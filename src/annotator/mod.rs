mod toml;
mod yaml;

pub use self::toml::TomlAnnotator;
pub use self::yaml::YamlAnnotator;

use crate::error::AnnotatorError;
use crate::schema::AnnotationMap;

/// How to handle fields that already have comments
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExistingCommentBehavior {
    /// Skip annotating fields that already have comments
    Skip,
    /// Add annotation before existing comment
    #[default]
    Prepend,
    /// Add annotation after existing comment
    Append,
    /// Replace existing comment with annotation
    Replace,
}

/// Configuration for annotation behavior
#[derive(Debug, Clone)]
pub struct AnnotatorConfig {
    /// Include title in comments
    pub include_title: bool,
    /// Include description in comments
    pub include_description: bool,
    /// Maximum line width for wrapping descriptions (None = no wrap)
    pub max_line_width: Option<usize>,
    /// How to handle fields that already have comments
    pub existing_comments: ExistingCommentBehavior,
}

impl Default for AnnotatorConfig {
    fn default() -> Self {
        Self {
            include_title: true,
            include_description: true,
            max_line_width: Some(80),
            existing_comments: ExistingCommentBehavior::default(),
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
