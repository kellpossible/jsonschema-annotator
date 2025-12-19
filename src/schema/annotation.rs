use std::collections::HashMap;

/// Annotation data extracted from a JSON Schema property
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    /// Dot-separated path (e.g., "server.port")
    pub path: String,
    /// Schema `title` field
    pub title: Option<String>,
    /// Schema `description` field
    pub description: Option<String>,
}

impl Annotation {
    /// Create a new annotation
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            title: None,
            description: None,
        }
    }

    /// Set the title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Format as comment lines
    pub fn to_comment_lines(&self, max_width: Option<usize>) -> Vec<String> {
        let mut lines = Vec::new();

        if let Some(title) = &self.title {
            lines.push(format!("# {}", title));
        }

        if let Some(desc) = &self.description {
            let width = max_width.unwrap_or(78);
            for line in textwrap::wrap(desc, width) {
                lines.push(format!("# {}", line));
            }
        }

        lines
    }

    /// Check if this annotation has any content
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.description.is_none()
    }
}

/// Collection of annotations indexed by path
#[derive(Debug, Clone, Default)]
pub struct AnnotationMap {
    inner: HashMap<String, Annotation>,
}

impl AnnotationMap {
    /// Create a new empty annotation map
    pub fn new() -> Self {
        Self::default()
    }

    /// Get an annotation by path
    pub fn get(&self, path: &str) -> Option<&Annotation> {
        self.inner.get(path)
    }

    /// Insert an annotation
    pub fn insert(&mut self, annotation: Annotation) {
        if !annotation.is_empty() {
            self.inner.insert(annotation.path.clone(), annotation);
        }
    }

    /// Iterate over all annotations
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Annotation)> {
        self.inner.iter()
    }

    /// Get the number of annotations
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the map is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotation_builder() {
        let ann = Annotation::new("server.port")
            .with_title("Port")
            .with_description("The server port number");

        assert_eq!(ann.path, "server.port");
        assert_eq!(ann.title, Some("Port".to_string()));
        assert_eq!(ann.description, Some("The server port number".to_string()));
    }

    #[test]
    fn test_annotation_to_comment_lines() {
        let ann = Annotation::new("test")
            .with_title("Title")
            .with_description("Description");

        let lines = ann.to_comment_lines(None);
        assert_eq!(lines, vec!["# Title", "# Description"]);
    }

    #[test]
    fn test_annotation_to_comment_lines_wrapping() {
        let ann = Annotation::new("test")
            .with_description("This is a very long description that should be wrapped");

        let lines = ann.to_comment_lines(Some(30));
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(line.len() <= 32); // 30 + "# " prefix
        }
    }

    #[test]
    fn test_annotation_map() {
        let mut map = AnnotationMap::new();

        map.insert(Annotation::new("a").with_title("A"));
        map.insert(Annotation::new("b").with_title("B"));

        assert_eq!(map.len(), 2);
        assert_eq!(map.get("a").unwrap().title, Some("A".to_string()));
        assert_eq!(map.get("b").unwrap().title, Some("B".to_string()));
        assert!(map.get("c").is_none());
    }

    #[test]
    fn test_empty_annotation_not_inserted() {
        let mut map = AnnotationMap::new();
        map.insert(Annotation::new("empty"));
        assert!(map.is_empty());
    }
}
