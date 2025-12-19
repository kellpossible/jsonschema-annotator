use toml_edit::{DocumentMut, Item, Table};

use super::{Annotator, AnnotatorConfig, ExistingCommentBehavior};
use crate::error::{AnnotatorError, AnnotatorErrorKind, Error};
use crate::schema::{Annotation, AnnotationMap};

/// TOML document annotator using toml_edit
pub struct TomlAnnotator {
    config: AnnotatorConfig,
}

impl TomlAnnotator {
    pub fn new(config: AnnotatorConfig) -> Self {
        Self { config }
    }

    fn format_comment(&self, annotation: &Annotation) -> Option<String> {
        let mut lines = Vec::new();

        if self.config.include_title {
            if let Some(title) = &annotation.title {
                lines.push(format!("# {}", title));
            }
        }

        if self.config.include_description {
            if let Some(desc) = &annotation.description {
                let width = self.config.max_line_width.unwrap_or(78);
                for line in textwrap::wrap(desc, width.saturating_sub(2)) {
                    lines.push(format!("# {}", line));
                }
            }
        }

        if self.config.include_default {
            if let Some(default) = &annotation.default {
                lines.push(format!("# Default: {}", default));
            }
        }

        if lines.is_empty() {
            None
        } else {
            // Add newline after comments so it appears before the key
            Some(lines.join("\n") + "\n")
        }
    }

    fn annotate_table(
        &self,
        table: &mut Table,
        path: &[String],
        annotations: &AnnotationMap,
    ) {
        // Collect keys first to avoid borrow issues
        // Use deref to str to get the key string (Key derefs to str)
        let keys: Vec<String> = table.iter().map(|(k, _)| (*k).to_string()).collect();

        for key_string in keys {
            let mut current_path = path.to_vec();
            current_path.push(key_string.clone());
            let path_string = current_path.join(".");

            // Get mutable access to the key-value pair
            if let Some((mut key, item)) = table.get_key_value_mut(&key_string) {
                // Handle tables vs regular values differently
                match item {
                    Item::Table(nested) => {
                        // For tables, use the table's own decor (appears before the [header])
                        if let Some(ann) = annotations.get(&path_string) {
                            if let Some(comment) = self.format_comment(ann) {
                                let decor = nested.decor_mut();
                                let existing = decor.prefix().map(|s| s.as_str().unwrap_or("")).unwrap_or("");
                                let has_existing = existing.trim().starts_with('#');

                                let new_prefix = match self.config.existing_comments {
                                    ExistingCommentBehavior::Skip if has_existing => None,
                                    ExistingCommentBehavior::Prepend if has_existing => {
                                        Some(format!("{}{}", comment, existing))
                                    }
                                    ExistingCommentBehavior::Append if has_existing => {
                                        Some(format!("{}{}", existing, comment))
                                    }
                                    _ => Some(comment), // Replace or no existing comment
                                };

                                if let Some(prefix) = new_prefix {
                                    decor.set_prefix(prefix);
                                }
                            }
                        }
                        // Recurse into nested tables
                        self.annotate_table(nested, &current_path, annotations);
                    }
                    Item::Value(toml_edit::Value::InlineTable(_)) => {
                        // Can't easily modify inline tables, skip for now
                    }
                    _ => {
                        // For regular values, use the key's decor
                        if let Some(ann) = annotations.get(&path_string) {
                            if let Some(comment) = self.format_comment(ann) {
                                let decor = key.leaf_decor_mut();
                                let existing = decor.prefix().map(|s| s.as_str().unwrap_or("")).unwrap_or("");
                                let has_existing = existing.trim().starts_with('#');

                                let new_prefix = match self.config.existing_comments {
                                    ExistingCommentBehavior::Skip if has_existing => None,
                                    ExistingCommentBehavior::Prepend if has_existing => {
                                        Some(format!("{}{}", comment, existing))
                                    }
                                    ExistingCommentBehavior::Append if has_existing => {
                                        Some(format!("{}{}", existing, comment))
                                    }
                                    _ => Some(comment), // Replace or no existing comment
                                };

                                if let Some(prefix) = new_prefix {
                                    decor.set_prefix(prefix);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Annotator for TomlAnnotator {
    fn annotate(
        &self,
        content: &str,
        annotations: &AnnotationMap,
    ) -> Result<String, AnnotatorError> {
        let mut doc: DocumentMut = content
            .parse()
            .map_err(|e| Error::new(AnnotatorErrorKind::Parse).with_source(e))?;

        self.annotate_table(doc.as_table_mut(), &Vec::new(), annotations);

        Ok(doc.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Annotation;
    use insta::assert_snapshot;

    fn make_annotations(items: &[(&str, Option<&str>, Option<&str>)]) -> AnnotationMap {
        let mut map = AnnotationMap::new();
        for (path, title, desc) in items {
            let mut ann = Annotation::new(*path);
            if let Some(t) = title {
                ann = ann.with_title(*t);
            }
            if let Some(d) = desc {
                ann = ann.with_description(*d);
            }
            map.insert(ann);
        }
        map
    }

    #[test]
    fn test_simple_annotation() {
        let content = "port = 8080\n";
        let annotations = make_annotations(&[("port", Some("Port"), Some("Server port number"))]);

        let annotator = TomlAnnotator::new(AnnotatorConfig::default());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_nested_table() {
        let content = r#"[server]
port = 8080
host = "localhost"
"#;
        let annotations = make_annotations(&[
            ("server", Some("Server Config"), None),
            ("server.port", Some("Port"), Some("The port to listen on")),
            ("server.host", Some("Host"), None),
        ]);

        let annotator = TomlAnnotator::new(AnnotatorConfig::default());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_title_only() {
        let content = "name = \"test\"\n";
        let annotations = make_annotations(&[("name", Some("Name"), Some("Full description"))]);

        let annotator = TomlAnnotator::new(AnnotatorConfig::titles_only());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_description_only() {
        let content = "name = \"test\"\n";
        let annotations = make_annotations(&[("name", Some("Name"), Some("Full description"))]);

        let annotator = TomlAnnotator::new(AnnotatorConfig::descriptions_only());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_preserve_existing_comments() {
        let content = "# Existing comment\nport = 8080\n";
        let annotations = make_annotations(&[("port", Some("Port"), None)]);

        let annotator = TomlAnnotator::new(AnnotatorConfig::default());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_no_matching_annotations() {
        let content = "name = \"test\"\nage = 30\n";
        let annotations = make_annotations(&[("other", Some("Other"), None)]);

        let annotator = TomlAnnotator::new(AnnotatorConfig::default());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_deeply_nested() {
        let content = r#"[database]
[database.connection]
host = "localhost"
port = 5432
"#;
        let annotations = make_annotations(&[
            ("database", Some("Database"), None),
            ("database.connection", Some("Connection Settings"), None),
            ("database.connection.host", Some("Host"), Some("Database server hostname")),
            ("database.connection.port", Some("Port"), None),
        ]);

        let annotator = TomlAnnotator::new(AnnotatorConfig::default());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_long_description_wrapping() {
        let content = "name = \"test\"\n";
        let long_desc = "This is a very long description that should be wrapped across multiple lines when the max line width is set to a reasonable value";
        let annotations = make_annotations(&[("name", None, Some(long_desc))]);

        let config = AnnotatorConfig {
            max_line_width: Some(40),
            ..Default::default()
        };
        let annotator = TomlAnnotator::new(config);
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_skip_existing_comments() {
        let content = "# Existing comment\nport = 8080\nhost = \"localhost\"\n";
        let annotations = make_annotations(&[
            ("port", Some("Port"), None),
            ("host", Some("Host"), None),
        ]);

        let config = AnnotatorConfig {
            existing_comments: ExistingCommentBehavior::Skip,
            ..Default::default()
        };
        let annotator = TomlAnnotator::new(config);
        let result = annotator.annotate(content, &annotations).unwrap();

        // port should keep its existing comment, host should get the annotation
        assert_snapshot!(result);
    }

    #[test]
    fn test_append_to_existing_comments() {
        let content = "# Existing comment\nport = 8080\n";
        let annotations = make_annotations(&[("port", Some("Port"), None)]);

        let config = AnnotatorConfig {
            existing_comments: ExistingCommentBehavior::Append,
            ..Default::default()
        };
        let annotator = TomlAnnotator::new(config);
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_replace_existing_comments() {
        let content = "# Existing comment\nport = 8080\n";
        let annotations = make_annotations(&[("port", Some("Port"), None)]);

        let config = AnnotatorConfig {
            existing_comments: ExistingCommentBehavior::Replace,
            ..Default::default()
        };
        let annotator = TomlAnnotator::new(config);
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_include_default_value() {
        let content = "port = 8080\n";

        let mut map = AnnotationMap::new();
        map.insert(
            Annotation::new("port")
                .with_title("Port")
                .with_description("The port number")
                .with_default("3000"),
        );

        let config = AnnotatorConfig {
            include_default: true,
            ..Default::default()
        };
        let annotator = TomlAnnotator::new(config);
        let result = annotator.annotate(content, &map).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_default_value_disabled_by_default() {
        let content = "port = 8080\n";

        let mut map = AnnotationMap::new();
        map.insert(
            Annotation::new("port")
                .with_title("Port")
                .with_default("3000"),
        );

        // Default config has include_default = false
        let annotator = TomlAnnotator::new(AnnotatorConfig::default());
        let result = annotator.annotate(content, &map).unwrap();

        assert_snapshot!(result);
    }
}
