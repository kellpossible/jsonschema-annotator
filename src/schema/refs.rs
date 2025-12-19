use schemars::Schema;
use serde_json::Value;

/// Resolve all local $ref pointers in a Schema
///
/// This only handles local references starting with "#" (e.g., "#/$defs/Address").
/// External file or URL references are not supported.
pub fn resolve_refs(schema: &Schema) -> Schema {
    let value = schema.as_value().clone();
    let resolved = resolve_refs_value(value, schema);
    // The resolved value should always be an object (from a valid schema)
    resolved.try_into().unwrap_or_else(|_| schema.clone())
}

fn resolve_refs_value(mut value: Value, root: &Schema) -> Value {
    match &mut value {
        Value::Object(map) => {
            if let Some(Value::String(ref_path)) = map.get("$ref") {
                // Only handle local references starting with #
                if ref_path.starts_with('#') {
                    // Use schemars' built-in pointer method (handles percent-decoding)
                    if let Some(resolved) = root.pointer(ref_path) {
                        return resolved.clone();
                    }
                }
            }
            // Recurse into all values
            for v in map.values_mut() {
                *v = resolve_refs_value(v.clone(), root);
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                *item = resolve_refs_value(item.clone(), root);
            }
        }
        _ => {}
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_resolve_refs_simple() {
        let schema_json = json!({
            "$defs": {
                "Name": {
                    "type": "string",
                    "title": "Name",
                    "description": "A person's name"
                }
            },
            "properties": {
                "firstName": {"$ref": "#/$defs/Name"},
                "lastName": {"$ref": "#/$defs/Name"}
            }
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let resolved = resolve_refs(&schema);
        let value = resolved.as_value();

        // The $ref should be replaced with the actual definition
        let first_name = &value["properties"]["firstName"];
        assert_eq!(first_name["type"], "string");
        assert_eq!(first_name["title"], "Name");

        let last_name = &value["properties"]["lastName"];
        assert_eq!(last_name["type"], "string");
        assert_eq!(last_name["title"], "Name");
    }

    #[test]
    fn test_resolve_refs_nested() {
        let schema_json = json!({
            "$defs": {
                "Address": {
                    "type": "object",
                    "title": "Address",
                    "properties": {
                        "city": {"type": "string"}
                    }
                }
            },
            "properties": {
                "home": {"$ref": "#/$defs/Address"},
                "work": {"$ref": "#/$defs/Address"}
            }
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let resolved = resolve_refs(&schema);
        let value = resolved.as_value();

        let home = &value["properties"]["home"];
        assert_eq!(home["type"], "object");
        assert_eq!(home["title"], "Address");
        assert_eq!(home["properties"]["city"]["type"], "string");
    }

    #[test]
    fn test_resolve_refs_external_ignored() {
        // External refs are left unchanged
        let schema_json = json!({
            "properties": {
                "external": {"$ref": "http://example.com/schema.json"}
            }
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let resolved = resolve_refs(&schema);
        let value = resolved.as_value();

        // External $ref should remain unchanged
        assert_eq!(
            value["properties"]["external"]["$ref"],
            "http://example.com/schema.json"
        );
    }

    #[test]
    fn test_resolve_refs_not_found() {
        // Unresolvable local refs are left unchanged
        let schema_json = json!({
            "properties": {
                "missing": {"$ref": "#/$defs/DoesNotExist"}
            }
        });

        let schema: Schema = schema_json.try_into().unwrap();
        let resolved = resolve_refs(&schema);
        let value = resolved.as_value();

        // Unresolvable $ref should remain unchanged
        assert_eq!(
            value["properties"]["missing"]["$ref"],
            "#/$defs/DoesNotExist"
        );
    }
}
