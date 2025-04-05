mod generator;
mod parser;

use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;

/// CLI tool to generate Rust structs from JSON files
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Input JSON file path
    #[clap(short, long)]
    input: PathBuf,

    /// Output Rust file path (defaults to struct.rs)
    #[clap(short, long, default_value = "struct.rs")]
    output: PathBuf,

    /// Optional root struct name (defaults to RootStruct)
    #[clap(short, long, default_value = "RootStruct")]
    name: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Read the JSON file
    let json_content = fs::read_to_string(&args.input)
        .with_context(|| format!("Failed to read input file: {}", args.input.display()))?;

    // Parse and validate the JSON
    let value = parser::parse_json(&json_content).with_context(|| "Failed to parse JSON")?;

    // Generate Rust struct code
    let rust_code = generator::generate_struct(&args.name, &value)
        .with_context(|| "Failed to generate struct code")?;

    // Write the struct code to the output file
    fs::write(&args.output, rust_code)
        .with_context(|| format!("Failed to write to output file: {}", args.output.display()))?;

    println!(
        "Successfully generated struct at: {}",
        args.output.display()
    );
    Ok(())
}
