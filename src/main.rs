use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use jsonschema_annotator::{annotate, AnnotatorConfig, TargetFormat};
use schemars::Schema;

#[derive(Parser)]
#[command(name = "jsonschema-annotator")]
#[command(about = "Annotate YAML and TOML files with comments from JSON Schema")]
#[command(version)]
struct Cli {
    /// Path to JSON Schema file (JSON or YAML)
    #[arg(short, long)]
    schema: PathBuf,

    /// Path to config file to annotate (YAML or TOML), or - for stdin
    #[arg(short, long)]
    input: String,

    /// Output path (default: stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// What to include in comments
    #[arg(long, value_enum, default_value = "both")]
    include: IncludeMode,

    /// Maximum line width for description wrapping
    #[arg(long, default_value = "80")]
    max_width: usize,

    /// Overwrite output file if it exists
    #[arg(long)]
    force: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum IncludeMode {
    Title,
    Description,
    Both,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Read and parse schema
    let schema_content = fs::read_to_string(&cli.schema)?;
    let schema: Schema = if cli.schema.extension().map(|e| e == "yaml" || e == "yml").unwrap_or(false) {
        serde_yaml::from_str(&schema_content)?
    } else {
        serde_json::from_str(&schema_content)?
    };

    // Read input content
    let (input_content, target_format) = if cli.input == "-" {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content)?;
        // Default to YAML for stdin, user can override by specifying output extension
        let format = cli.output
            .as_ref()
            .and_then(|p| TargetFormat::from_path(p))
            .unwrap_or(TargetFormat::Yaml);
        (content, format)
    } else {
        let path = PathBuf::from(&cli.input);
        let content = fs::read_to_string(&path)?;
        let format = TargetFormat::from_path(&path)
            .ok_or_else(|| format!("Unknown file format: {}", path.display()))?;
        (content, format)
    };

    // Build config
    let config = AnnotatorConfig {
        include_title: matches!(cli.include, IncludeMode::Title | IncludeMode::Both),
        include_description: matches!(cli.include, IncludeMode::Description | IncludeMode::Both),
        max_line_width: Some(cli.max_width),
        preserve_existing: true,
    };

    // Annotate
    let annotated = annotate(&schema, &input_content, target_format, config)?;

    // Write output
    if let Some(output_path) = cli.output {
        if output_path.exists() && !cli.force {
            return Err(format!(
                "Output file exists: {}. Use --force to overwrite.",
                output_path.display()
            ).into());
        }
        fs::write(&output_path, &annotated)?;
        eprintln!("Wrote annotated config to {}", output_path.display());
    } else {
        io::stdout().write_all(annotated.as_bytes())?;
    }

    Ok(())
}
