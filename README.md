# jsonschema-annotator

Annotate YAML and TOML configuration files with comments derived from JSON Schema `title` and `description` fields.

## Example

Given a schema:
```json
{
  "properties": {
    "port": { "title": "Port", "description": "Server port number" }
  }
}
```

And a config:
```toml
port = 8080
```

Produces:
```toml
# Port
# Server port number
port = 8080
```

## Usage

### CLI
```bash
jsonschema-annotator -s schema.json -i config.toml -o config.annotated.toml
```

### Library
```rust
use jsonschema_annotator::{annotate, TargetFormat, AnnotatorConfig};
use schemars::Schema;

let schema: Schema = serde_json::from_str(schema_json)?;
let annotated = annotate(&schema, config_str, TargetFormat::Toml, AnnotatorConfig::default())?;
```

## TODO

### Phase 1: Core Types
- [ ] Set up Cargo.toml with dependencies
- [ ] `src/error.rs` - Generic error type with context chaining
- [ ] `src/format.rs` - TargetFormat enum (Toml, Yaml)
- [ ] `src/schema/annotation.rs` - Annotation, AnnotationMap

### Phase 2: Schema Parsing & $ref Resolution
- [ ] `src/schema/refs.rs` - JSON Pointer parsing, $ref dereferencing
- [ ] `src/schema/parser.rs` - Recursive walker extracting title/description

### Phase 3: TOML Annotator
- [ ] `src/annotator/toml.rs` - Using toml_edit decor API
- [ ] Unit tests for TOML annotation

### Phase 4: YAML Annotator
- [ ] `src/annotator/yaml.rs` - Using yaml-edit or string manipulation
- [ ] Unit tests for YAML annotation

### Phase 5: CLI & Library API
- [ ] `src/main.rs` - CLI with clap
- [ ] `src/lib.rs` - Public API exports
- [ ] Integration tests (`tests/integration.rs`)
- [ ] Test fixtures (sample schemas and configs)
