use schemars::Schema;
use serde_json::Value;

use super::annotation::{Annotation, AnnotationMap};
use super::refs::resolve_refs;

/// Format a JSON value as a human-readable string for display in comments
fn format_default_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("\"{}\"", s),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_default_value).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(obj) => {
            let items: Vec<String> = obj
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_default_value(v)))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
    }
}

/// Extract annotations from a JSON Schema
///
/// This resolves $refs and walks the schema recursively,
/// extracting title/description for each property path.
pub fn extract_annotations(schema: &Schema) -> AnnotationMap {
    let resolved = resolve_refs(schema);
    let mut annotations = AnnotationMap::new();
    let mut path = Vec::new();

    walk_schema(resolved.as_value(), &mut path, &mut annotations);

    annotations
}

fn walk_schema(value: &Value, current_path: &mut Vec<String>, annotations: &mut AnnotationMap) {
    let Some(obj) = value.as_object() else {
        return;
    };

    // Extract title/description/default at current level
    let title = obj.get("title").and_then(|v| v.as_str());
    let desc = obj.get("description").and_then(|v| v.as_str());
    let default = obj.get("default").map(format_default_value);

    if title.is_some() || desc.is_some() || default.is_some() {
        let mut ann = Annotation::new(current_path.join("."));
        if let Some(t) = title {
            ann = ann.with_title(t);
        }
        if let Some(d) = desc {
            ann = ann.with_description(d);
        }
        if let Some(d) = default {
            ann = ann.with_default(d);
        }
        annotations.insert(ann);
    }

    // Recurse into properties
    if let Some(props) = obj.get("properties").and_then(|v| v.as_object()) {
        for (key, val) in props {
            current_path.push(key.clone());
            walk_schema(val, current_path, annotations);
            current_path.pop();
        }
    }

    // Handle array items (annotation applies to the array key itself)
    if let Some(items) = obj.get("items") {
        walk_schema(items, current_path, annotations);
    }

    // Handle additionalProperties if it's a schema object
    if let Some(additional) = obj.get("additionalProperties") {
        if additional.is_object() {
            walk_schema(additional, current_path, annotations);
        }
    }

    // Handle oneOf/allOf/anyOf composition
    for keyword in ["oneOf", "allOf", "anyOf"] {
        if let Some(schemas) = obj.get(keyword).and_then(|v| v.as_array()) {
            for schema in schemas {
                walk_schema(schema, current_path, annotations);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_simple() {
        let schema_json = json!({
            "properties": {
                "name": {
                    "title": "Name",
                    "description": "User's full name"
                },
                "age": {
                    "title": "Age"
                }
            }
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let annotations = extract_annotations(&schema);

        assert_eq!(annotations.len(), 2);

        let name = annotations.get("name").unwrap();
        assert_eq!(name.title, Some("Name".to_string()));
        assert_eq!(name.description, Some("User's full name".to_string()));

        let age = annotations.get("age").unwrap();
        assert_eq!(age.title, Some("Age".to_string()));
        assert_eq!(age.description, None);
    }

    #[test]
    fn test_extract_nested() {
        let schema_json = json!({
            "properties": {
                "server": {
                    "title": "Server",
                    "description": "Server configuration",
                    "properties": {
                        "host": {
                            "title": "Host",
                            "description": "Server hostname"
                        },
                        "port": {
                            "title": "Port"
                        }
                    }
                }
            }
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let annotations = extract_annotations(&schema);

        assert_eq!(annotations.len(), 3);

        let server = annotations.get("server").unwrap();
        assert_eq!(server.title, Some("Server".to_string()));

        let host = annotations.get("server.host").unwrap();
        assert_eq!(host.title, Some("Host".to_string()));
        assert_eq!(host.description, Some("Server hostname".to_string()));

        let port = annotations.get("server.port").unwrap();
        assert_eq!(port.title, Some("Port".to_string()));
    }

    #[test]
    fn test_extract_with_refs() {
        let schema_json = json!({
            "$defs": {
                "Address": {
                    "title": "Address",
                    "description": "A physical address",
                    "properties": {
                        "city": {
                            "title": "City"
                        }
                    }
                }
            },
            "properties": {
                "home": {"$ref": "#/$defs/Address"},
                "work": {"$ref": "#/$defs/Address"}
            }
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let annotations = extract_annotations(&schema);

        // Both home and work should have annotations from the Address $def
        let home = annotations.get("home").unwrap();
        assert_eq!(home.title, Some("Address".to_string()));

        let home_city = annotations.get("home.city").unwrap();
        assert_eq!(home_city.title, Some("City".to_string()));

        let work = annotations.get("work").unwrap();
        assert_eq!(work.title, Some("Address".to_string()));
    }

    #[test]
    fn test_extract_root_annotation() {
        let schema_json = json!({
            "title": "Config",
            "description": "Application configuration",
            "properties": {
                "debug": {
                    "title": "Debug Mode"
                }
            }
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let annotations = extract_annotations(&schema);

        // Root level annotation has empty path
        let root = annotations.get("").unwrap();
        assert_eq!(root.title, Some("Config".to_string()));
        assert_eq!(root.description, Some("Application configuration".to_string()));

        let debug = annotations.get("debug").unwrap();
        assert_eq!(debug.title, Some("Debug Mode".to_string()));
    }

    #[test]
    fn test_extract_no_annotations() {
        let schema_json = json!({
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "number"}
            }
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let annotations = extract_annotations(&schema);

        assert!(annotations.is_empty());
    }

    #[test]
    fn test_extract_array_items() {
        let schema_json = json!({
            "properties": {
                "users": {
                    "title": "Users",
                    "description": "List of users",
                    "items": {
                        "properties": {
                            "name": {
                                "title": "User Name"
                            }
                        }
                    }
                }
            }
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let annotations = extract_annotations(&schema);

        let users = annotations.get("users").unwrap();
        assert_eq!(users.title, Some("Users".to_string()));

        // Items inherit the parent path
        let user_name = annotations.get("users.name").unwrap();
        assert_eq!(user_name.title, Some("User Name".to_string()));
    }

    #[test]
    fn test_extract_oneof() {
        let schema_json = json!({
            "properties": {
                "value": {
                    "title": "Value",
                    "oneOf": [
                        {
                            "properties": {
                                "string_val": {
                                    "title": "String Value",
                                    "description": "A string value"
                                }
                            }
                        },
                        {
                            "properties": {
                                "number_val": {
                                    "title": "Number Value"
                                }
                            }
                        }
                    ]
                }
            }
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let annotations = extract_annotations(&schema);

        let value = annotations.get("value").unwrap();
        assert_eq!(value.title, Some("Value".to_string()));

        let string_val = annotations.get("value.string_val").unwrap();
        assert_eq!(string_val.title, Some("String Value".to_string()));
        assert_eq!(string_val.description, Some("A string value".to_string()));

        let number_val = annotations.get("value.number_val").unwrap();
        assert_eq!(number_val.title, Some("Number Value".to_string()));
    }

    #[test]
    fn test_extract_allof() {
        let schema_json = json!({
            "allOf": [
                {
                    "properties": {
                        "base": {
                            "title": "Base Property"
                        }
                    }
                },
                {
                    "properties": {
                        "extended": {
                            "title": "Extended Property"
                        }
                    }
                }
            ]
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let annotations = extract_annotations(&schema);

        let base = annotations.get("base").unwrap();
        assert_eq!(base.title, Some("Base Property".to_string()));

        let extended = annotations.get("extended").unwrap();
        assert_eq!(extended.title, Some("Extended Property".to_string()));
    }

    #[test]
    fn test_extract_anyof() {
        let schema_json = json!({
            "properties": {
                "config": {
                    "title": "Config",
                    "anyOf": [
                        {
                            "properties": {
                                "simple": {
                                    "title": "Simple Mode"
                                }
                            }
                        },
                        {
                            "properties": {
                                "advanced": {
                                    "title": "Advanced Mode",
                                    "description": "For power users"
                                }
                            }
                        }
                    ]
                }
            }
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let annotations = extract_annotations(&schema);

        let config = annotations.get("config").unwrap();
        assert_eq!(config.title, Some("Config".to_string()));

        let simple = annotations.get("config.simple").unwrap();
        assert_eq!(simple.title, Some("Simple Mode".to_string()));

        let advanced = annotations.get("config.advanced").unwrap();
        assert_eq!(advanced.title, Some("Advanced Mode".to_string()));
        assert_eq!(advanced.description, Some("For power users".to_string()));
    }

    #[test]
    fn test_extract_default_values() {
        let schema_json = json!({
            "properties": {
                "port": {
                    "title": "Port",
                    "description": "The port number",
                    "default": 8080
                },
                "host": {
                    "title": "Host",
                    "default": "localhost"
                },
                "enabled": {
                    "title": "Enabled",
                    "default": true
                },
                "tags": {
                    "title": "Tags",
                    "default": ["web", "api"]
                }
            }
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let annotations = extract_annotations(&schema);

        let port = annotations.get("port").unwrap();
        assert_eq!(port.title, Some("Port".to_string()));
        assert_eq!(port.default, Some("8080".to_string()));

        let host = annotations.get("host").unwrap();
        assert_eq!(host.default, Some("\"localhost\"".to_string()));

        let enabled = annotations.get("enabled").unwrap();
        assert_eq!(enabled.default, Some("true".to_string()));

        let tags = annotations.get("tags").unwrap();
        assert_eq!(tags.default, Some("[\"web\", \"api\"]".to_string()));
    }

    #[test]
    fn test_extract_only_default() {
        // Test that a property with only a default value still gets extracted
        let schema_json = json!({
            "properties": {
                "timeout": {
                    "default": 30
                }
            }
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let annotations = extract_annotations(&schema);

        let timeout = annotations.get("timeout").unwrap();
        assert_eq!(timeout.title, None);
        assert_eq!(timeout.description, None);
        assert_eq!(timeout.default, Some("30".to_string()));
    }
}
