# jsonschema-annotator

Annotate YAML and TOML configuration files with comments derived from JSON Schema `title` and `description` fields.

## Example

Given a schema:
```json
{
  "properties": {
    "server": {
      "title": "Server Configuration",
      "description": "HTTP server settings",
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

And a config:
```toml
[server]
port = 8080
```

Produces:
```toml
# Server Configuration
# HTTP server settings
[server]
# Port
# The port number to listen on
port = 8080
```

## Installation

```bash
cargo install jsonschema-annotator
```

Or build from source:
```bash
git clone https://github.com/kellpossible/jsonschema-annotator
cd jsonschema-annotator
cargo build --release
```

## CLI Usage

```bash
# Annotate a TOML file, output to stdout
jsonschema-annotator -s schema.json -i config.toml

# Annotate a YAML file, write to output file
jsonschema-annotator -s schema.json -i config.yaml -o config.annotated.yaml

# Read from stdin (defaults to YAML format)
cat config.yaml | jsonschema-annotator -s schema.json -i -

# Only include titles (no descriptions)
jsonschema-annotator -s schema.json -i config.toml --include title

# Custom line width for description wrapping
jsonschema-annotator -s schema.json -i config.toml --max-width 60
```

### CLI Options

```text
Options:
  -s, --schema <SCHEMA>        Path to JSON Schema file (JSON or YAML)
  -i, --input <INPUT>          Path to config file to annotate (YAML or TOML), or - for stdin
  -o, --output <OUTPUT>        Output path (default: stdout)
      --include <INCLUDE>      What to include in comments [default: both] [possible values: title, description, both]
      --max-width <MAX_WIDTH>  Maximum line width for description wrapping [default: 80]
      --force                  Overwrite output file if it exists
  -h, --help                   Print help
  -V, --version                Print version
```

## Library Usage

```rust,no_run
use jsonschema_annotator::{annotate, TargetFormat, AnnotatorConfig};
use schemars::Schema;

let schema_json = r#"{
    "properties": {
        "port": {
            "title": "Port",
            "description": "Server port number"
        }
    }
}"#;

let schema: Schema = serde_json::from_str(schema_json).unwrap();
let config_str = "port = 8080";

let annotated = annotate(
    &schema,
    config_str,
    TargetFormat::Toml,
    AnnotatorConfig::default(),
).unwrap();

assert!(annotated.contains("# Port"));
assert!(annotated.contains("# Server port number"));
```

### Configuration Options

```rust,ignore
let config = AnnotatorConfig {
    include_title: true,        // Include schema titles
    include_description: true,  // Include schema descriptions
    max_line_width: Some(80),   // Wrap descriptions at 80 chars
    preserve_existing: true,    // Keep existing comments
};
```

## Features

- **TOML & YAML support**: Annotate both formats with the same schema
- **$ref resolution**: Local JSON Schema `$ref` pointers are resolved automatically
- **Schema composition**: `oneOf`, `allOf`, and `anyOf` are supported
- **Format preservation**: Uses `toml_edit` and string-based YAML injection to preserve formatting
- **Configurable output**: Include title, description, or both
- **Line wrapping**: Long descriptions are wrapped at configurable width
- **Existing comments**: Optionally preserve existing comments in the file

## Limitations

- Only local `$ref` (starting with `#`) are supported; external file/URL references are not
- YAML inline tables and complex structures may not be annotated

## License

MIT License - see [LICENSE](LICENSE) for details.
