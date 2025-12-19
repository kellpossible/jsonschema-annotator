mod toml;
mod yaml;

pub use self::toml::TomlAnnotator;
pub use self::yaml::YamlAnnotator;

use crate::error::AnnotatorError;
use crate::schema::AnnotationMap;

/// Configuration for annotation behavior
#[derive(Debug, Clone)]
pub struct AnnotatorConfig {
    /// Include title in comments
    pub include_title: bool,
    /// Include description in comments
    pub include_description: bool,
    /// Maximum line width for wrapping descriptions (None = no wrap)
    pub max_line_width: Option<usize>,
    /// Preserve existing comments in the document
    pub preserve_existing: bool,
}

impl Default for AnnotatorConfig {
    fn default() -> Self {
        Self {
            include_title: true,
            include_description: true,
            max_line_width: Some(80),
            preserve_existing: true,
        }
    }
}

impl AnnotatorConfig {
    /// Create a config that only includes titles
    pub fn titles_only() -> Self {
        Self {
            include_title: true,
            include_description: false,
            ..Default::default()
        }
    }

    /// Create a config that only includes descriptions
    pub fn descriptions_only() -> Self {
        Self {
            include_title: false,
            include_description: true,
            ..Default::default()
        }
    }
}

/// Common interface for format-specific annotators
pub trait Annotator {
    /// Annotate a document with comments from the annotation map
    fn annotate(
        &self,
        content: &str,
        annotations: &AnnotationMap,
    ) -> Result<String, AnnotatorError>;
}
