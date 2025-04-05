mod generator;
mod parser;

use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

/// CLI tool to generate Rust structs from JSON files or stdin
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Structr {
    /// Input JSON file path(s)
    #[clap(short, long, value_parser)]
    input: Vec<PathBuf>,

    /// Output Rust file path (defaults to struct.rs)
    #[clap(short, long, default_value = "struct.rs")]
    output: PathBuf,

    /// Optional root struct name (defaults to RootStruct)
    #[clap(short, long, default_value = "RootStruct")]
    name: String,

    /// Parse all fields as Option<T>
    #[clap(long)]
    strict_option: bool,

    /// Flatten nested objects
    #[clap(long)]
    flatten: bool,

    /// Read JSON from stdin
    #[clap(long)]
    stdin: bool,

    /// Generate code compatible with Actix web framework
    #[clap(long)]
    actix: bool,

    /// Generate code compatible with Axum web framework
    #[clap(long)]
    axum: bool,

    /// Generate code compatible with Rocket web framework
    #[clap(long)]
    rocket: bool,

    /// Generate GraphQL schema
    #[clap(long)]
    graphql: bool,

    /// GraphQL library to use (juniper or async-graphql)
    #[clap(long, default_value = "async-graphql")]
    graphql_lib: String,

    /// Generate GraphQL schema file
    #[clap(long)]
    graphql_schema: bool,

    /// Output directory for additional schema files
    #[clap(long, default_value = ".")]
    schema_dir: PathBuf,
}

fn main() -> Result<()> {
    let args = Structr::parse();

    // Determine if we should read from stdin
    let use_stdin = args.stdin || (args.input.is_empty() && !atty::is(atty::Stream::Stdin));

    let mut json_values = Vec::new();

    if use_stdin {
        // Read from stdin
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;

        let value =
            parser::parse_json(&buffer).with_context(|| "Failed to parse JSON from stdin")?;
        json_values.push(value);
    } else if !args.input.is_empty() {
        // Read from files
        for input_path in &args.input {
            let json_content = fs::read_to_string(input_path)
                .with_context(|| format!("Failed to read input file: {}", input_path.display()))?;

            let value = parser::parse_json(&json_content)
                .with_context(|| format!("Failed to parse JSON from {}", input_path.display()))?;
            json_values.push(value);
        }
    } else {
        return Err(anyhow::anyhow!(
            "No input provided. Use --input or pipe JSON to stdin"
        ));
    }

    if json_values.is_empty() {
        return Err(anyhow::anyhow!("No valid JSON input found"));
    }

    // Create framework options
    let framework_options = generator::FrameworkOptions {
        actix: args.actix,
        axum: args.axum,
        rocket: args.rocket,
        openapi: false, // Simplified implementation
        graphql: args.graphql,
        graphql_lib: args.graphql_lib.clone(),
    };

    // Create generator options
    let generator_options = generator::GeneratorOptions {
        strict_option: args.strict_option,
        flatten: args.flatten,
        framework: framework_options,
    };

    // Generate Rust struct code
    let rust_code =
        generator::generate_struct_from_samples(&args.name, &json_values, &generator_options)
            .with_context(|| "Failed to generate struct code")?;

    // Write the struct code to the output file
    fs::write(&args.output, rust_code)
        .with_context(|| format!("Failed to write to output file: {}", args.output.display()))?;

    println!(
        "Successfully generated struct at: {}",
        args.output.display()
    );

    // Generate GraphQL schema if requested
    if args.graphql_schema {
        let graphql_schema =
            generator::generate_graphql_schema(&args.name, &json_values, &generator_options)?;
        let graphql_path = args
            .schema_dir
            .join(format!("{}_graphql.graphql", args.name));
        fs::write(&graphql_path, graphql_schema).with_context(|| {
            format!(
                "Failed to write GraphQL schema to: {}",
                graphql_path.display()
            )
        })?;
        println!(
            "Successfully generated GraphQL schema at: {}",
            graphql_path.display()
        );
    }

    Ok(())
}
