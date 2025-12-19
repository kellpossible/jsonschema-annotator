use std::path::Path;

/// Format of the target file to annotate
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetFormat {
    Toml,
    Yaml,
}

impl TargetFormat {
    /// Detect format from file extension
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        Self::from_extension(ext)
    }

    /// Detect format from extension string
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "toml" => Some(Self::Toml),
            "yaml" | "yml" => Some(Self::Yaml),
            _ => None,
        }
    }

    /// Get the canonical file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Toml => "toml",
            Self::Yaml => "yaml",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_extension() {
        assert_eq!(TargetFormat::from_extension("toml"), Some(TargetFormat::Toml));
        assert_eq!(TargetFormat::from_extension("yaml"), Some(TargetFormat::Yaml));
        assert_eq!(TargetFormat::from_extension("yml"), Some(TargetFormat::Yaml));
        assert_eq!(TargetFormat::from_extension("TOML"), Some(TargetFormat::Toml));
        assert_eq!(TargetFormat::from_extension("json"), None);
    }

    #[test]
    fn test_from_path() {
        assert_eq!(TargetFormat::from_path(Path::new("config.toml")), Some(TargetFormat::Toml));
        assert_eq!(TargetFormat::from_path(Path::new("config.yaml")), Some(TargetFormat::Yaml));
        assert_eq!(TargetFormat::from_path(Path::new("config.yml")), Some(TargetFormat::Yaml));
        assert_eq!(TargetFormat::from_path(Path::new("config.json")), None);
        assert_eq!(TargetFormat::from_path(Path::new("noext")), None);
    }
}
