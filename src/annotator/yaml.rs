use super::{Annotator, AnnotatorConfig};
use crate::error::{AnnotatorError, AnnotatorErrorKind, Error};
use crate::schema::{Annotation, AnnotationMap};

/// YAML document annotator using string-based line injection
///
/// Since yaml-edit doesn't support comment injection, we use a string-based
/// approach that tracks indentation to map lines to paths.
pub struct YamlAnnotator {
    config: AnnotatorConfig,
}

impl YamlAnnotator {
    pub fn new(config: AnnotatorConfig) -> Self {
        Self { config }
    }

    fn format_comment(&self, annotation: &Annotation, indent: usize) -> Option<String> {
        let mut lines = Vec::new();
        let indent_str = " ".repeat(indent);

        if self.config.include_title {
            if let Some(title) = &annotation.title {
                lines.push(format!("{}# {}", indent_str, title));
            }
        }

        if self.config.include_description {
            if let Some(desc) = &annotation.description {
                let width = self.config.max_line_width.unwrap_or(78).saturating_sub(indent + 2);
                for line in textwrap::wrap(desc, width) {
                    lines.push(format!("{}# {}", indent_str, line));
                }
            }
        }

        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    /// Build a map of line numbers to (path, indent) for YAML content
    fn build_line_path_map(&self, content: &str) -> Vec<(usize, String, usize)> {
        let mut result = Vec::new();
        let mut path_stack: Vec<(String, usize)> = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            // Skip empty lines and comments
            if line.trim().is_empty() || line.trim().starts_with('#') {
                continue;
            }

            // Calculate indentation
            let indent = line.len() - line.trim_start().len();

            // Pop path components that are at same or deeper indentation
            while let Some((_, prev_indent)) = path_stack.last() {
                if indent <= *prev_indent {
                    path_stack.pop();
                } else {
                    break;
                }
            }

            // Extract key from line (handle "key:" and "key: value" formats)
            if let Some(key) = extract_yaml_key(line) {
                // Build current path
                let path = if path_stack.is_empty() {
                    key.clone()
                } else {
                    let parent_path: Vec<_> = path_stack.iter().map(|(k, _)| k.as_str()).collect();
                    format!("{}.{}", parent_path.join("."), key)
                };

                result.push((line_num, path.clone(), indent));

                // Check if this line starts a nested object (ends with ":" or has nested content)
                if line.trim().ends_with(':') || is_mapping_start(line) {
                    path_stack.push((key, indent));
                }
            }
        }

        result
    }
}

/// Extract the key from a YAML line like "key: value" or "key:"
fn extract_yaml_key(line: &str) -> Option<String> {
    let trimmed = line.trim();

    // Skip list items for now (lines starting with -)
    if trimmed.starts_with('-') {
        return None;
    }

    // Find the colon
    let colon_pos = trimmed.find(':')?;
    let key = trimmed[..colon_pos].trim();

    // Skip if key is empty or quoted (complex keys)
    if key.is_empty() {
        return None;
    }

    Some(key.to_string())
}

/// Check if a line is a mapping start (key with no inline value)
fn is_mapping_start(line: &str) -> bool {
    let trimmed = line.trim();
    if let Some(colon_pos) = trimmed.find(':') {
        let after_colon = trimmed[colon_pos + 1..].trim();
        after_colon.is_empty() || after_colon.starts_with('#')
    } else {
        false
    }
}

impl Annotator for YamlAnnotator {
    fn annotate(
        &self,
        content: &str,
        annotations: &AnnotationMap,
    ) -> Result<String, AnnotatorError> {
        // Validate YAML syntax by attempting to parse
        let _: serde_yaml::Value = serde_yaml::from_str(content)
            .map_err(|e| Error::new(AnnotatorErrorKind::Parse).with_source(e))?;

        let line_paths = self.build_line_path_map(content);

        // Collect insertions: (line_num, comment, indent)
        let mut insertions: Vec<(usize, String, usize)> = Vec::new();

        for (line_num, path, indent) in &line_paths {
            if let Some(ann) = annotations.get(path) {
                if let Some(comment) = self.format_comment(ann, *indent) {
                    insertions.push((*line_num, comment, *indent));
                }
            }
        }

        // Sort by line number descending to insert from bottom up
        insertions.sort_by(|a, b| b.0.cmp(&a.0));

        // Insert comments
        let mut lines: Vec<String> = content.lines().map(String::from).collect();

        for (line_num, comment, _indent) in insertions {
            // Insert comment lines before the target line
            let comment_lines: Vec<String> = comment.lines().map(String::from).collect();
            for (i, comment_line) in comment_lines.into_iter().enumerate() {
                lines.insert(line_num + i, comment_line);
            }
        }

        // Preserve trailing newline if original had one
        let mut result = lines.join("\n");
        if content.ends_with('\n') {
            result.push('\n');
        }

        Ok(result)
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
        let content = "port: 8080\n";
        let annotations = make_annotations(&[("port", Some("Port"), Some("Server port number"))]);

        let annotator = YamlAnnotator::new(AnnotatorConfig::default());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_nested_mapping() {
        let content = r#"server:
  port: 8080
  host: localhost
"#;
        let annotations = make_annotations(&[
            ("server", Some("Server Config"), None),
            ("server.port", Some("Port"), Some("The port to listen on")),
            ("server.host", Some("Host"), None),
        ]);

        let annotator = YamlAnnotator::new(AnnotatorConfig::default());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_title_only() {
        let content = "name: test\n";
        let annotations = make_annotations(&[("name", Some("Name"), Some("Full description"))]);

        let annotator = YamlAnnotator::new(AnnotatorConfig::titles_only());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_description_only() {
        let content = "name: test\n";
        let annotations = make_annotations(&[("name", Some("Name"), Some("Full description"))]);

        let annotator = YamlAnnotator::new(AnnotatorConfig::descriptions_only());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_preserve_existing_comments() {
        let content = "# Existing comment\nport: 8080\n";
        let annotations = make_annotations(&[("port", Some("Port"), None)]);

        let annotator = YamlAnnotator::new(AnnotatorConfig::default());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_no_matching_annotations() {
        let content = "name: test\nage: 30\n";
        let annotations = make_annotations(&[("other", Some("Other"), None)]);

        let annotator = YamlAnnotator::new(AnnotatorConfig::default());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_deeply_nested() {
        let content = r#"database:
  connection:
    host: localhost
    port: 5432
"#;
        let annotations = make_annotations(&[
            ("database", Some("Database"), None),
            ("database.connection", Some("Connection Settings"), None),
            ("database.connection.host", Some("Host"), Some("Database server hostname")),
            ("database.connection.port", Some("Port"), None),
        ]);

        let annotator = YamlAnnotator::new(AnnotatorConfig::default());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_inline_values() {
        let content = r#"server:
  host: localhost
  port: 8080
  enabled: true
"#;
        let annotations = make_annotations(&[
            ("server.host", Some("Hostname"), None),
            ("server.port", Some("Port Number"), None),
            ("server.enabled", Some("Enabled"), Some("Whether the server is enabled")),
        ]);

        let annotator = YamlAnnotator::new(AnnotatorConfig::default());
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }
}
