#![doc = include_str!("../README.md")]

mod annotator;
mod error;
mod format;
mod schema;

pub use annotator::{Annotator, AnnotatorConfig, ExistingCommentBehavior, TomlAnnotator, YamlAnnotator};
pub use error::{AnnotatorError, AnnotatorErrorKind, Error, ResultExt, SchemaError, SchemaErrorKind};
pub use format::TargetFormat;
pub use schema::{extract_annotations, Annotation, AnnotationMap};

use schemars::Schema;

/// Annotate a target document with schema descriptions
///
/// # Arguments
/// * `schema` - JSON Schema as a `schemars::Schema`
/// * `target` - Target document as a string (TOML or YAML)
/// * `target_format` - Format of the target document
/// * `config` - Annotation configuration options
///
/// # Example
/// ```rust
/// use jsonschema_annotator::{annotate, TargetFormat, AnnotatorConfig};
/// use schemars::Schema;
///
/// let schema_json = r#"{"properties": {"port": {"title": "Port"}}}"#;
/// let schema: Schema = serde_json::from_str(schema_json).unwrap();
///
/// let annotated = annotate(
///     &schema,
///     "port = 8080",
///     TargetFormat::Toml,
///     AnnotatorConfig::default(),
/// ).unwrap();
///
/// assert!(annotated.contains("# Port"));
/// ```
pub fn annotate(
    schema: &Schema,
    target: &str,
    target_format: TargetFormat,
    config: AnnotatorConfig,
) -> Result<String, AnnotatorError> {
    let annotations = extract_annotations(schema);

    match target_format {
        TargetFormat::Toml => {
            let annotator = TomlAnnotator::new(config);
            annotator.annotate(target, &annotations)
        }
        TargetFormat::Yaml => {
            let annotator = YamlAnnotator::new(config);
            annotator.annotate(target, &annotations)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn test_annotate_toml() {
        let schema_json = r#"{
            "properties": {
                "server": {
                    "title": "Server",
                    "description": "Server configuration",
                    "properties": {
                        "port": {
                            "title": "Port",
                            "description": "The port to listen on"
                        }
                    }
                }
            }
        }"#;

        let schema: Schema = serde_json::from_str(schema_json).unwrap();
        let config = r#"[server]
port = 8080
"#;

        let result = annotate(&schema, config, TargetFormat::Toml, AnnotatorConfig::default()).unwrap();
        assert_snapshot!(result);
    }

    #[test]
    fn test_annotate_yaml() {
        let schema_json = r#"{
            "properties": {
                "server": {
                    "title": "Server",
                    "description": "Server configuration",
                    "properties": {
                        "port": {
                            "title": "Port",
                            "description": "The port to listen on"
                        }
                    }
                }
            }
        }"#;

        let schema: Schema = serde_json::from_str(schema_json).unwrap();
        let config = r#"server:
  port: 8080
"#;

        let result = annotate(&schema, config, TargetFormat::Yaml, AnnotatorConfig::default()).unwrap();
        assert_snapshot!(result);
    }

    #[test]
    fn test_annotate_with_refs() {
        let schema_json = r##"{
            "$defs": {
                "Port": {
                    "title": "Port",
                    "description": "A network port number"
                }
            },
            "properties": {
                "http_port": { "$ref": "#/$defs/Port" },
                "https_port": { "$ref": "#/$defs/Port" }
            }
        }"##;

        let schema: Schema = serde_json::from_str(schema_json).unwrap();
        let config = "http_port = 80\nhttps_port = 443\n";

        let result = annotate(&schema, config, TargetFormat::Toml, AnnotatorConfig::default()).unwrap();
        assert_snapshot!(result);
    }
}
