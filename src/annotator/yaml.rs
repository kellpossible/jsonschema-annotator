use super::{Annotator, AnnotatorConfig, ExistingCommentBehavior};
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

    /// Build a map of line numbers to (path, indent, has_existing_comment) for YAML content
    fn build_line_path_map(&self, content: &str) -> Vec<(usize, String, usize, bool)> {
        let mut result = Vec::new();
        let mut path_stack: Vec<(String, usize)> = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (line_num, line) in lines.iter().enumerate() {
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

                // Check if there's an existing comment immediately before this line
                let has_existing_comment = self.has_preceding_comment(&lines, line_num, indent);

                result.push((line_num, path.clone(), indent, has_existing_comment));

                // Check if this line starts a nested object (ends with ":" or has nested content)
                if line.trim().ends_with(':') || is_mapping_start(line) {
                    path_stack.push((key, indent));
                }
            }
        }

        result
    }

    /// Check if there's a comment line immediately preceding the given line
    /// that belongs to this key (at the same or appropriate indentation)
    fn has_preceding_comment(&self, lines: &[&str], line_num: usize, key_indent: usize) -> bool {
        if line_num == 0 {
            return false;
        }

        // Look at the line immediately before
        let prev_line = lines[line_num - 1];
        let prev_trimmed = prev_line.trim();

        // If it's a comment, check if it's at the same indentation level
        if prev_trimmed.starts_with('#') {
            let prev_indent = prev_line.len() - prev_line.trim_start().len();
            // Comment belongs to this key if it's at the same indentation
            return prev_indent == key_indent;
        }

        false
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

/// Represents an operation to perform on the YAML lines
enum YamlOperation {
    /// Insert comment lines before the target line
    Insert { line_num: usize, comment: String },
    /// Replace the existing comment line with a new one
    Replace { line_num: usize, comment: String },
    /// Insert comment lines after an existing comment (before the key)
    Append { line_num: usize, comment: String },
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

        // Collect operations
        let mut operations: Vec<YamlOperation> = Vec::new();

        for (line_num, path, indent, has_existing_comment) in &line_paths {
            if let Some(ann) = annotations.get(path) {
                if let Some(comment) = self.format_comment(ann, *indent) {
                    let op = match (self.config.existing_comments, *has_existing_comment) {
                        (ExistingCommentBehavior::Skip, true) => None,
                        (ExistingCommentBehavior::Replace, true) => {
                            Some(YamlOperation::Replace {
                                line_num: *line_num,
                                comment,
                            })
                        }
                        (ExistingCommentBehavior::Append, true) => {
                            Some(YamlOperation::Append {
                                line_num: *line_num,
                                comment,
                            })
                        }
                        _ => {
                            // Prepend (default) or no existing comment
                            Some(YamlOperation::Insert {
                                line_num: *line_num,
                                comment,
                            })
                        }
                    };

                    if let Some(operation) = op {
                        operations.push(operation);
                    }
                }
            }
        }

        // Sort by line number descending to process from bottom up
        operations.sort_by(|a, b| {
            let line_a = match a {
                YamlOperation::Insert { line_num, .. }
                | YamlOperation::Replace { line_num, .. }
                | YamlOperation::Append { line_num, .. } => *line_num,
            };
            let line_b = match b {
                YamlOperation::Insert { line_num, .. }
                | YamlOperation::Replace { line_num, .. }
                | YamlOperation::Append { line_num, .. } => *line_num,
            };
            line_b.cmp(&line_a)
        });

        // Apply operations
        let mut lines: Vec<String> = content.lines().map(String::from).collect();

        for op in operations {
            match op {
                YamlOperation::Insert { line_num, comment } => {
                    let comment_lines: Vec<String> = comment.lines().map(String::from).collect();
                    for (i, comment_line) in comment_lines.into_iter().enumerate() {
                        lines.insert(line_num + i, comment_line);
                    }
                }
                YamlOperation::Replace { line_num, comment } => {
                    // Find and count existing comment lines before this key
                    let mut start_line = line_num - 1;
                    while start_line > 0 && lines[start_line - 1].trim().starts_with('#') {
                        start_line -= 1;
                    }
                    // Remove old comments
                    for _ in start_line..line_num {
                        lines.remove(start_line);
                    }
                    // Insert new comments at the start position
                    let comment_lines: Vec<String> = comment.lines().map(String::from).collect();
                    for (i, comment_line) in comment_lines.into_iter().enumerate() {
                        lines.insert(start_line + i, comment_line);
                    }
                }
                YamlOperation::Append { line_num, comment } => {
                    // Insert after existing comments (right before the key)
                    let comment_lines: Vec<String> = comment.lines().map(String::from).collect();
                    for (i, comment_line) in comment_lines.into_iter().enumerate() {
                        lines.insert(line_num + i, comment_line);
                    }
                }
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

    #[test]
    fn test_skip_existing_comments() {
        let content = "# Existing comment\nport: 8080\nhost: localhost\n";
        let annotations = make_annotations(&[
            ("port", Some("Port"), None),
            ("host", Some("Host"), None),
        ]);

        let mut config = AnnotatorConfig::default();
        config.existing_comments = ExistingCommentBehavior::Skip;
        let annotator = YamlAnnotator::new(config);
        let result = annotator.annotate(content, &annotations).unwrap();

        // port should keep its existing comment, host should get the annotation
        assert_snapshot!(result);
    }

    #[test]
    fn test_append_to_existing_comments() {
        let content = "# Existing comment\nport: 8080\n";
        let annotations = make_annotations(&[("port", Some("Port"), None)]);

        let mut config = AnnotatorConfig::default();
        config.existing_comments = ExistingCommentBehavior::Append;
        let annotator = YamlAnnotator::new(config);
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }

    #[test]
    fn test_replace_existing_comments() {
        let content = "# Existing comment\nport: 8080\n";
        let annotations = make_annotations(&[("port", Some("Port"), None)]);

        let mut config = AnnotatorConfig::default();
        config.existing_comments = ExistingCommentBehavior::Replace;
        let annotator = YamlAnnotator::new(config);
        let result = annotator.annotate(content, &annotations).unwrap();

        assert_snapshot!(result);
    }
}
