# JSON Schema Annotator - Design Document

## Overview

A Rust library and CLI tool that annotates YAML and TOML configuration files with comments derived from JSON Schema `title` and `description` properties.

### Example

Given a JSON Schema:
```json
{
  "properties": {
    "server": {
      "title": "Server Configuration",
      "description": "Settings for the HTTP server",
      "properties": {
        "port": {
          "title": "Port",
          "description": "The port number to listen on"
        }
      }
    }
  }
}
```

And a TOML config:
```toml
[server]
port = 8080
```

The tool produces:
```toml
# Server Configuration
# Settings for the HTTP server
[server]
# Port
# The port number to listen on
port = 8080
```

## Requirements

| Requirement | Decision |
|-------------|----------|
| Schema input formats | JSON, YAML |
| Target file formats | YAML, TOML |
| Output mode | New file (preserves original) |
| Comment placement | Above the field |
| Interface | Library + CLI |

## Library Choices

| Purpose | Library | Rationale |
|---------|---------|-----------|
| Schema type | `schemars::Schema` | Type-safe JSON Schema wrapper, deserializable |
| TOML editing | `toml_edit` | Format-preserving, native comment support via `decor_mut()` |
| YAML editing | `yaml-edit` | Lossless parser using rowan syntax trees |
| CLI | `clap` | Derive-based, excellent UX |

### $ref Resolution Strategy

Schemars generates schemas with `$ref` but doesn't resolve them. We implement our own resolver:

```rust
use schemars::Schema;
use serde_json::Value;

/// Resolve all $ref pointers in a Schema
pub fn resolve_refs(schema: &Schema) -> Schema {
    let value = schema.as_value().clone();
    let root = value.clone();
    let resolved = resolve_refs_value(value, &root);
    Schema::from(resolved)
}

fn resolve_refs_value(mut value: Value, root: &Value) -> Value {
    match &mut value {
        Value::Object(map) => {
            if let Some(Value::String(ref_path)) = map.get("$ref") {
                // Parse JSON Pointer (e.g., "#/$defs/Address")
                if let Some(resolved) = resolve_json_pointer(root, ref_path) {
                    return resolved.clone();
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

/// Resolve a JSON Pointer like "#/$defs/Address"
fn resolve_json_pointer(root: &Value, pointer: &str) -> Option<&Value> {
    let pointer = pointer.strip_prefix("#")?;
    let mut current = root;
    for segment in pointer.split('/').filter(|s| !s.is_empty()) {
        let decoded = segment.replace("~1", "/").replace("~0", "~");
        current = current.get(&decoded)?;
    }
    Some(current)
}
```

**Scope**: Local `$ref` only (starting with `#`). External file/URL references are out of scope for v0.1.

### YAML Editing Strategy

The `yaml-edit` crate provides lossless parsing but may not support injecting new comments. Fallback strategy:

1. **Primary**: Use `yaml-edit` if it supports comment insertion
2. **Fallback**: String-based line injection:
   - Parse YAML structure to understand nesting
   - Track indentation to build path-to-line mapping
   - Insert comment lines at calculated positions

## Architecture

```
                    ┌─────────────────┐
                    │   JSON Schema   │
                    │  (.json/.yaml)  │
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  SchemaParser   │
                    │                 │
                    │ Extracts title/ │
                    │ description per │
                    │ property path   │
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  AnnotationMap  │
                    │                 │
                    │ path → {title,  │
                    │     description}│
                    └────────┬────────┘
                             │
              ┌──────────────┴──────────────┐
              │                             │
              ▼                             ▼
     ┌─────────────────┐           ┌─────────────────┐
     │  TomlAnnotator  │           │  YamlAnnotator  │
     │                 │           │                 │
     │  Uses toml_edit │           │ Uses yaml-edit  │
     │  decor API      │           │ or string manip │
     └────────┬────────┘           └────────┬────────┘
              │                             │
              ▼                             ▼
     ┌─────────────────┐           ┌─────────────────┐
     │ Annotated TOML  │           │ Annotated YAML  │
     └─────────────────┘           └─────────────────┘
```

## Module Structure

```
src/
├── lib.rs                 # Public API exports
├── main.rs                # CLI binary (clap)
├── error.rs               # SchemaError, AnnotatorError
├── format.rs              # FileFormat detection
├── schema/
│   ├── mod.rs
│   ├── annotation.rs      # Annotation, AnnotationMap
│   ├── refs.rs            # $ref resolution, JSON Pointer parsing
│   └── parser.rs          # Recursive JSON Schema walker
└── annotator/
    ├── mod.rs             # Annotator trait, AnnotatorConfig
    ├── toml.rs            # TomlAnnotator
    └── yaml.rs            # YamlAnnotator
```

## Core Types

### Annotation

```rust
/// Annotation data extracted from a JSON Schema property
#[derive(Debug, Clone)]
pub struct Annotation {
    /// Dot-separated path (e.g., "server.port")
    pub path: String,
    /// Schema `title` field
    pub title: Option<String>,
    /// Schema `description` field
    pub description: Option<String>,
}

impl Annotation {
    /// Format as comment lines
    pub fn to_comment_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(title) = &self.title {
            lines.push(format!("# {}", title));
        }
        if let Some(desc) = &self.description {
            for line in textwrap::wrap(desc, 78) {
                lines.push(format!("# {}", line));
            }
        }
        lines
    }
}
```

### AnnotationMap

```rust
/// Collection of annotations indexed by path
pub struct AnnotationMap {
    inner: HashMap<String, Annotation>,
}

impl AnnotationMap {
    pub fn get(&self, path: &str) -> Option<&Annotation>;
    pub fn insert(&mut self, annotation: Annotation);
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Annotation)>;
}
```

### Annotator Trait

```rust
/// Common interface for format-specific annotators
pub trait Annotator {
    fn annotate(
        &self,
        content: &str,
        annotations: &AnnotationMap,
    ) -> Result<String, AnnotatorError>;
}

/// Configuration for annotation behavior
#[derive(Debug, Clone)]
pub struct AnnotatorConfig {
    /// Include title in comments
    pub include_title: bool,
    /// Include description in comments
    pub include_description: bool,
    /// Maximum line width for wrapping (None = no wrap)
    pub max_line_width: Option<usize>,
    /// Preserve existing comments
    pub preserve_existing: bool,
}
```

## Schema Parsing

The parser recursively walks the JSON Schema, extracting annotations:

```rust
fn walk_schema(
    value: &serde_json::Value,
    current_path: &mut Vec<String>,
    annotations: &mut AnnotationMap,
) {
    if let Some(obj) = value.as_object() {
        // Extract title/description at current level
        let title = obj.get("title").and_then(|v| v.as_str());
        let desc = obj.get("description").and_then(|v| v.as_str());

        if title.is_some() || desc.is_some() {
            annotations.insert(Annotation {
                path: current_path.join("."),
                title: title.map(String::from),
                description: desc.map(String::from),
            });
        }

        // Recurse into properties
        if let Some(props) = obj.get("properties").and_then(|v| v.as_object()) {
            for (key, val) in props {
                current_path.push(key.clone());
                walk_schema(val, current_path, annotations);
                current_path.pop();
            }
        }

        // Handle array items (annotation applies to array key)
        if let Some(items) = obj.get("items") {
            walk_schema(items, current_path, annotations);
        }
    }
}
```

## TOML Annotator

Uses `toml_edit`'s decor API to add prefix comments:

```rust
impl Annotator for TomlAnnotator {
    fn annotate(&self, content: &str, annotations: &AnnotationMap) -> Result<String, AnnotatorError> {
        let mut doc: DocumentMut = content.parse()?;
        self.annotate_table(doc.as_table_mut(), &[], annotations)?;
        Ok(doc.to_string())
    }
}

fn annotate_table(&self, table: &mut Table, path: &[&str], annotations: &AnnotationMap) {
    for (key, item) in table.iter_mut() {
        let current_path = [path, &[key.get()]].concat().join(".");

        if let Some(ann) = annotations.get(&current_path) {
            let comment = ann.to_comment_lines().join("\n") + "\n";
            key.leaf_decor_mut().set_prefix(comment);
        }

        // Recurse into nested tables
        if let Item::Table(nested) = item {
            self.annotate_table(nested, &[path, &[key.get()]].concat(), annotations);
        }
    }
}
```

## YAML Annotator

Primary approach using `yaml-edit`, with string-based fallback:

```rust
impl Annotator for YamlAnnotator {
    fn annotate(&self, content: &str, annotations: &AnnotationMap) -> Result<String, AnnotatorError> {
        // Build map of line numbers to paths
        let line_paths = self.build_line_path_map(content)?;

        // Collect insertions (line_num, comment, indent)
        let mut insertions: Vec<(usize, String, usize)> = Vec::new();

        for (line_num, (path, indent)) in &line_paths {
            if let Some(ann) = annotations.get(path) {
                let comment = ann.to_comment_lines().join("\n");
                insertions.push((*line_num, comment, *indent));
            }
        }

        // Insert in reverse order to preserve line numbers
        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        insertions.sort_by(|a, b| b.0.cmp(&a.0));

        for (line_num, comment, indent) in insertions {
            let indented: Vec<String> = comment
                .lines()
                .map(|l| format!("{}{}", " ".repeat(indent), l))
                .collect();

            for (i, line) in indented.into_iter().rev().enumerate() {
                lines.insert(line_num, line);
            }
        }

        Ok(lines.join("\n"))
    }
}
```

## CLI Interface

```
jsonschema-annotator [OPTIONS] --schema <FILE> --input <FILE>

Options:
  -s, --schema <FILE>     Path to JSON Schema file (JSON or YAML)
  -i, --input <FILE>      Path to config file to annotate (YAML or TOML)
  -o, --output <FILE>     Output path [default: <input>.annotated.<ext>]
      --include <MODE>    What to include: title, description, both [default: both]
      --max-width <N>     Max line width for descriptions [default: 80]
      --preserve-comments Keep existing comments [default: true]
      --stdout            Print to stdout instead of file
      --force             Overwrite output if exists
  -h, --help              Print help
  -V, --version           Print version
```

### Usage Examples

```bash
# Basic usage
jsonschema-annotator -s schema.json -i config.toml

# YAML schema and config
jsonschema-annotator -s schema.yaml -i config.yaml -o annotated.yaml

# Only descriptions, print to stdout
jsonschema-annotator -s schema.json -i config.toml --include description --stdout
```

## Error Handling

Generic error type with context chaining (inspired by offline-docs pattern):

```rust
use std::borrow::Cow;

/// Generic error with kind, context chain, and hidden source
#[derive(Debug)]
pub struct Error<K> {
    pub kind: K,
    context: Vec<Cow<'static, str>>,
    source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

impl<K> Error<K> {
    pub fn new(kind: K) -> Self { /* ... */ }
    pub fn add_context(self, ctx: impl Into<Cow<'static, str>>) -> Self { /* ... */ }
    pub fn with_source(self, src: impl std::error::Error + Send + Sync + 'static) -> Self { /* ... */ }
    pub fn map_kind<NK>(self, f: impl Fn(K) -> NK) -> Error<NK> { /* ... */ }
}

/// Extension trait for adding context to Results
pub trait ResultExt<T, K> {
    fn add_context(self, ctx: impl Into<Cow<'static, str>>) -> Result<T, Error<K>>;
}

// Error kinds
#[derive(Debug)]
pub enum SchemaErrorKind {
    Io,
    ValueParse,      // JSON or YAML parsing failure
    InvalidSchema,
    RefResolution,
}

#[derive(Debug)]
pub enum AnnotatorErrorKind {
    Parse,           // TOML or YAML parsing failure
    Io,
}

pub type SchemaError = Error<SchemaErrorKind>;
pub type AnnotatorError = Error<AnnotatorErrorKind>;
```

Usage:
```rust
fn parse_schema(path: &Path) -> Result<Schema, SchemaError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| SchemaError::new(SchemaErrorKind::Io).with_source(e))
        .add_context_fn(|| format!("reading schema from {}", path.display()))?;
    // ...
}
```

## Public Library API

```rust
use schemars::Schema;

// Re-exports
pub use schema::{Annotation, AnnotationMap};
pub use annotator::{Annotator, AnnotatorConfig, TomlAnnotator, YamlAnnotator};
pub use error::{SchemaError, AnnotatorError};
pub use format::TargetFormat;

/// Format of the target file to annotate
#[derive(Debug, Clone, Copy)]
pub enum TargetFormat {
    Toml,
    Yaml,
}

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
/// use schemars::schema_for;
///
/// #[derive(schemars::JsonSchema)]
/// struct Config {
///     /// Server port number
///     port: u16,
/// }
///
/// let schema = schema_for!(Config);
/// let config_str = "port = 8080";
///
/// let annotated = annotate(
///     &schema,
///     config_str,
///     TargetFormat::Toml,
///     AnnotatorConfig::default(),
/// )?;
///
/// assert!(annotated.contains("# Server port number"));
/// ```
pub fn annotate(
    schema: &Schema,
    target: &str,
    target_format: TargetFormat,
    config: AnnotatorConfig,
) -> Result<String, AnnotatorError>;

/// Extract annotations from a schema (with $ref resolution)
pub fn extract_annotations(schema: &Schema) -> Result<AnnotationMap, SchemaError>;
```

## Dependencies

```toml
[package]
name = "jsonschema-annotator"
version = "0.1.0"
edition = "2021"

[dependencies]
schemars = "1.1"            # JSON Schema type (Schema struct)
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"          # JSON parsing, $ref resolution internals
serde_yaml = "0.9"          # YAML schema file parsing (CLI)
toml_edit = "0.22"          # TOML editing with comments
yaml-edit = "0.1"           # YAML editing (lossless)
clap = { version = "4.5", features = ["derive"] }  # CLI
textwrap = "0.16"           # Comment line wrapping
```

## Implementation Phases

### Phase 1: Core Types
- [ ] `error.rs` - Error types
- [ ] `format.rs` - FileFormat enum
- [ ] `schema/annotation.rs` - Annotation, AnnotationMap

### Phase 2: Schema Parsing & $ref Resolution
- [ ] `schema/refs.rs` - JSON Pointer resolution, $ref dereferencing
- [ ] `schema/parser.rs` - Recursive walker extracting title/description
- [ ] Unit tests for nested schemas and $ref

### Phase 3: TOML Annotator
- [ ] `annotator/toml.rs` - Using toml_edit
- [ ] Integration tests

### Phase 4: YAML Annotator
- [ ] `annotator/yaml.rs` - Using yaml-edit or string manipulation
- [ ] Integration tests

### Phase 5: CLI & Polish
- [ ] `main.rs` - CLI with clap
- [ ] `lib.rs` - Public API
- [ ] End-to-end tests

## Out of Scope (v0.1)

- External `$ref` (file:// or http:// URLs) - only local `#/...` refs supported
- `allOf`/`oneOf`/`anyOf` composition
- In-place file modification
- Watch mode / file monitoring
- IDE integration
