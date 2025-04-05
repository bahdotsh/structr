use anyhow::{anyhow, Result};
use convert_case::{Case, Casing};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

type StructMap = HashMap<String, String>;

#[derive(Debug, Clone)]
pub struct FrameworkOptions {
    pub actix: bool,
    pub axum: bool,
    pub rocket: bool,
    pub openapi: bool,
    pub graphql: bool,
    pub graphql_lib: String,
}

#[derive(Debug, Clone)]
pub struct GeneratorOptions {
    pub strict_option: bool, // Use Option<T> for all fields
    pub flatten: bool,       // Use #[serde(flatten)] for nested objects
    pub framework: FrameworkOptions,
}

/// Generate Rust struct code from multiple JSON samples
pub fn generate_struct_from_samples(
    name: &str,
    samples: &[Value],
    options: &GeneratorOptions,
) -> Result<String> {
    if samples.is_empty() {
        return Err(anyhow!("No JSON samples provided"));
    }

    let mut code = String::from("use serde::{Serialize, Deserialize};\n");

    // Add framework-specific imports
    if options.framework.openapi {
        code.push_str("#[cfg(feature = \"openapi\")]\nuse utoipa::ToSchema;\n");
    }

    if options.framework.actix {
        code.push_str("use actix_web::web;\n");
    }

    if options.framework.axum {
        code.push_str("use axum::{extract::FromRef, extract::Path, extract::Query, Json};\n");
    }

    if options.framework.rocket {
        code.push_str("use rocket::form::FromForm;\n");
        code.push_str("use rocket::serde::{Serialize as RocketSerialize, Deserialize as RocketDeserialize};\n");
    }

    if options.framework.graphql {
        if options.framework.graphql_lib == "juniper" {
            code.push_str(
                "#[cfg(feature = \"graphql\")]\nuse juniper::{GraphQLObject, graphql_object};\n",
            );
        } else {
            code.push_str("#[cfg(feature = \"graphql\")]\nuse async_graphql::{SimpleObject, Object, Context};\n");
        }
    }

    code.push_str("\n");

    let mut generated_structs = StructMap::new();
    let mut struct_names = HashSet::new();

    // Merge samples to create a schema
    let merged_schema = if samples.len() == 1 {
        samples[0].clone()
    } else {
        merge_json_samples(samples)?
    };

    generate_struct_recursive(
        name,
        &merged_schema,
        &mut generated_structs,
        &mut struct_names,
        options,
        samples,
    )?;

    // Add all generated structs to the output
    for struct_code in generated_structs.values() {
        code.push_str(struct_code);
        code.push_str("\n\n");
    }

    // Generate GraphQL resolvers if needed
    if options.framework.graphql {
        if options.framework.graphql_lib == "async-graphql" {
            let resolver_code = generate_async_graphql_resolver(name, &struct_names);
            code.push_str(&resolver_code);
            code.push_str("\n\n");
        }
    }

    Ok(code.trim().to_string())
}

/// Merge multiple JSON samples into a schema
fn merge_json_samples(samples: &[Value]) -> Result<Value> {
    // Start with the first sample as a base
    let mut merged = samples[0].clone();

    match &mut merged {
        Value::Object(map) => {
            for sample in samples.iter().skip(1) {
                if let Value::Object(sample_map) = sample {
                    // Collect all field names from all samples
                    for (key, value) in sample_map {
                        if map.contains_key(key) {
                            // Field exists in both, merge the values
                            let merged_value = merge_values(map.get(key).unwrap(), value)?;
                            map.insert(key.clone(), merged_value);
                        } else {
                            // Field only in this sample, add it as optional
                            map.insert(key.clone(), value.clone());
                        }
                    }
                }
            }
            Ok(merged)
        }
        Value::Array(items) => {
            // For arrays, we merge all array items to create a superset
            for sample in samples.iter().skip(1) {
                if let Value::Array(sample_items) = sample {
                    for item in sample_items {
                        items.push(item.clone());
                    }
                }
            }
            Ok(merged)
        }
        _ => Err(anyhow!("Root of first sample must be an object or array")),
    }
}

/// Merge two JSON values
fn merge_values(v1: &Value, v2: &Value) -> Result<Value> {
    match (v1, v2) {
        (Value::Object(map1), Value::Object(map2)) => {
            let mut result = map1.clone();

            for (key, value) in map2 {
                if result.contains_key(key) {
                    let merged = merge_values(result.get(key).unwrap(), value)?;
                    result.insert(key.clone(), merged);
                } else {
                    result.insert(key.clone(), value.clone());
                }
            }

            Ok(Value::Object(result))
        }
        (Value::Array(_), Value::Array(_)) => {
            // For array types, we keep the first array but note the values may differ
            Ok(v1.clone())
        }
        (v1, v2) if v1.is_null() => Ok(v2.clone()),
        (v1, v2) if v2.is_null() => Ok(v1.clone()),
        // If types differ, prefer the non-null value or fallback to json Value
        (v1, v2)
            if v1.is_object() != v2.is_object()
                || v1.is_array() != v2.is_array()
                || v1.is_string() != v2.is_string()
                || v1.is_number() != v2.is_number()
                || v1.is_boolean() != v2.is_boolean() =>
        {
            // If one is null, use the other
            if v1.is_null() {
                Ok(v2.clone())
            } else if v2.is_null() {
                Ok(v1.clone())
            } else {
                // Types differ and neither is null, mark as generic Value
                Ok(Value::Null) // Use null as a marker for "any type"
            }
        }
        // Same types, keep first value
        _ => Ok(v1.clone()),
    }
}

/// Recursively generate struct definitions
fn generate_struct_recursive(
    name: &str,
    value: &Value,
    structs: &mut StructMap,
    struct_names: &mut HashSet<String>,
    options: &GeneratorOptions,
    samples: &[Value],
) -> Result<String> {
    let type_name = sanitize_struct_name(name);

    // Don't regenerate existing structs
    if struct_names.contains(&type_name) {
        return Ok(type_name);
    }

    struct_names.insert(type_name.clone());

    match value {
        Value::Object(map) => {
            let mut struct_code = String::new();

            // Add framework-specific derive macros
            let mut derive_macros = vec!["Debug", "Serialize", "Deserialize"];

            if options.framework.openapi {
                derive_macros.push("#[cfg(feature = \"openapi\")]");
                derive_macros.push("ToSchema");
            }

            if options.framework.actix {
                derive_macros.push("actix_web::web::Query");
                derive_macros.push("actix_web::web::Path");
                derive_macros.push("actix_web::web::Json");
            }

            if options.framework.axum {
                derive_macros.push("axum::extract::FromRef");
                derive_macros.push("axum::extract::Path");
                derive_macros.push("axum::extract::Query");
            }

            if options.framework.rocket {
                derive_macros.push("FromForm");
                derive_macros.push("RocketSerialize");
                derive_macros.push("RocketDeserialize");
            }

            if options.framework.graphql {
                if options.framework.graphql_lib == "juniper" {
                    derive_macros.push("#[cfg(feature = \"graphql\")]");
                    derive_macros.push("GraphQLObject");
                } else {
                    derive_macros.push("#[cfg(feature = \"graphql\")]");
                    derive_macros.push("SimpleObject");
                }
            }

            // Add the derive macros
            struct_code.push_str(&format!("#[derive({})]\n", derive_macros.join(", ")));

            // Add additional framework-specific attributes
            if options.framework.graphql && options.framework.graphql_lib == "async-graphql" {
                struct_code.push_str("#[cfg_attr(feature = \"graphql\", graphql(complex))]\n");
            }

            // Start the struct definition
            struct_code.push_str(&format!("pub struct {} {{\n", type_name));

            for (key, val) in map {
                let field_name = sanitize_field_name(key);

                // Determine if this field is optional by checking across samples
                let is_optional = is_field_optional(samples, key);

                // Get the field type
                let mut field_type = determine_type(
                    val,
                    &format!("{}_{}", type_name, key.to_case(Case::Pascal)),
                    structs,
                    struct_names,
                    options,
                    samples,
                )?;

                // Wrap in Option if needed
                if options.strict_option || is_optional {
                    field_type = format!("Option<{}>", field_type);
                }

                // Add serde and framework annotations
                let mut annotations = Vec::new();
                let mut field_attributes = Vec::new();

                // Serde annotations
                if field_name != *key || (options.flatten && val.is_object()) {
                    let mut serde_attrs = Vec::new();

                    if field_name != *key {
                        serde_attrs.push(format!("rename = \"{}\"", key));
                    }

                    if options.flatten && val.is_object() {
                        serde_attrs.push("flatten".to_string());
                    }

                    if is_optional || options.strict_option {
                        serde_attrs.push("skip_serializing_if = \"Option::is_none\"".to_string());
                    }

                    if !serde_attrs.is_empty() {
                        annotations.push(format!("serde({})", serde_attrs.join(", ")));
                    }
                }

                // GraphQL annotations
                if options.framework.graphql {
                    if options.framework.graphql_lib == "juniper" {
                        if field_name != *key {
                            annotations.push(format!("graphql(name = \"{}\")", key));
                        }
                    } else {
                        // async-graphql
                        if field_name != *key {
                            annotations.push(format!("name = \"{}\"", key));
                        }
                    }
                }

                // Add all annotations as attributes
                if !annotations.is_empty() {
                    for annotation in annotations {
                        field_attributes.push(format!("    #[{}]\n", annotation));
                    }
                }

                // Add attributes and field
                for attr in field_attributes {
                    struct_code.push_str(&attr);
                }
                struct_code.push_str(&format!("    pub {}: {},\n", field_name, field_type));
            }

            struct_code.push_str("}\n");

            // Add implementation blocks for frameworks if needed
            if options.framework.actix
                || options.framework.axum
                || options.framework.rocket
                || options.framework.graphql
            {
                struct_code.push_str("\n");

                // Actix implementation
                if options.framework.actix {
                    struct_code.push_str(&format!(
                        "impl {} {{\n    pub fn into_json(self) -> actix_web::web::Json<Self> {{\n        actix_web::web::Json(self)\n    }}\n}}\n\n",
                        type_name
                    ));
                }

                // Axum implementation
                if options.framework.axum {
                    struct_code.push_str(&format!(
                        "impl {} {{\n    pub fn into_json(self) -> axum::Json<Self> {{\n        axum::Json(self)\n    }}\n}}\n\n",
                        type_name
                    ));
                }

                // GraphQL implementation for Juniper
                if options.framework.graphql && options.framework.graphql_lib == "juniper" {
                    struct_code.push_str(&format!(
                        "#[cfg(feature = \"graphql\")]\ngraphql_object!({}: () |{{ }})\n\n",
                        type_name
                    ));
                }
            }

            structs.insert(type_name.clone(), struct_code);

            Ok(type_name)
        }
        Value::Array(items) => {
            if items.is_empty() {
                return Ok("Vec<serde_json::Value>".to_string());
            }

            // Try to determine the common type for array items
            let item_type = determine_array_item_type(
                items,
                &format!("{}Item", type_name),
                structs,
                struct_names,
                options,
                samples,
            )?;
            Ok(format!("Vec<{}>", item_type))
        }
        _ => Err(anyhow!(
            "Expected object or array at root level for struct generation"
        )),
    }
}

/// Check if a field is optional across samples
fn is_field_optional(samples: &[Value], field_name: &str) -> bool {
    // A field is optional if it's not present in at least one sample
    samples.iter().any(|sample| {
        if let Value::Object(map) = sample {
            !map.contains_key(field_name) || map[field_name].is_null()
        } else {
            false
        }
    })
}

/// Try to create an enum for a field
fn try_create_enum(field_name: &str, samples: &[Value], structs: &mut StructMap) -> Option<String> {
    // Collect all string values for this field
    let mut values = HashSet::new();
    let mut all_strings = true;

    for sample in samples {
        if let Value::Object(map) = sample {
            if let Some(value) = map.get(field_name) {
                if let Value::String(s) = value {
                    values.insert(s.clone());
                } else {
                    all_strings = false;
                    break;
                }
            }
        }
    }

    // If we have a reasonable number of distinct string values (2-10), create an enum
    if all_strings && values.len() >= 2 && values.len() <= 10 {
        let enum_name = sanitize_struct_name(&format!("{}Enum", field_name.to_case(Case::Pascal)));

        let mut enum_code = format!(
            "#[derive(Debug, Serialize, Deserialize)]\npub enum {} {{\n",
            enum_name
        );

        for value in values {
            let variant_name = sanitize_struct_name(&value);

            // Add rename if variant name differs from value
            if variant_name != value {
                enum_code.push_str(&format!("    #[serde(rename = \"{}\")]\n", value));
            }

            enum_code.push_str(&format!("    {},\n", variant_name));
        }

        enum_code.push_str("}\n");
        structs.insert(enum_name.clone(), enum_code);

        return Some(enum_name);
    }

    None
}

/// Determine Rust type from JSON value
fn determine_type(
    value: &Value,
    name: &str,
    structs: &mut StructMap,
    struct_names: &mut HashSet<String>,
    options: &GeneratorOptions,
    samples: &[Value],
) -> Result<String> {
    match value {
        Value::Null => Ok("serde_json::Value".to_string()),
        Value::Bool(_) => Ok("bool".to_string()),
        Value::Number(n) => {
            if n.is_i64() {
                Ok("i64".to_string())
            } else {
                Ok("f64".to_string())
            }
        }
        Value::String(_) => {
            // Try to create an enum if this is a field in multiple samples
            if let Some(enum_name) = extract_field_name(name)
                .and_then(|field_name| try_create_enum(field_name, samples, structs))
            {
                return Ok(enum_name);
            }

            Ok("String".to_string())
        }
        Value::Array(items) => {
            if items.is_empty() {
                return Ok("Vec<serde_json::Value>".to_string());
            }

            let item_type = determine_array_item_type(
                items,
                &format!("{}Item", name),
                structs,
                struct_names,
                options,
                samples,
            )?;
            Ok(format!("Vec<{}>", item_type))
        }
        Value::Object(_) => {
            let type_name =
                generate_struct_recursive(name, value, structs, struct_names, options, samples)?;
            Ok(type_name)
        }
    }
}

/// Extract field name from a generated name
fn extract_field_name(name: &str) -> Option<&str> {
    name.split('_').nth(1)
}

/// Determine common type for array items
fn determine_array_item_type(
    items: &[Value],
    name: &str,
    structs: &mut StructMap,
    struct_names: &mut HashSet<String>,
    options: &GeneratorOptions,
    samples: &[Value],
) -> Result<String> {
    // If any item is an object, we'll need a custom struct
    if items.iter().any(|item| item.is_object()) {
        // Merge all object items to create a schema
        let mut merged_obj = serde_json::Map::new();
        for item in items.iter().filter(|i| i.is_object()) {
            if let Value::Object(map) = item {
                for (key, val) in map {
                    if !merged_obj.contains_key(key) {
                        merged_obj.insert(key.clone(), val.clone());
                    }
                }
            }
        }

        let merged_value = Value::Object(merged_obj);
        return generate_struct_recursive(
            name,
            &merged_value,
            structs,
            struct_names,
            options,
            samples,
        );
    }

    // For arrays of arrays
    if items.iter().any(|item| item.is_array()) {
        if let Some(arr) = items.iter().find(|&item| item.is_array()) {
            let inner_type = determine_array_item_type(
                arr.as_array().unwrap(),
                &format!("{}Inner", name),
                structs,
                struct_names,
                options,
                samples,
            )?;
            return Ok(format!("Vec<{}>", inner_type));
        }
    }

    // For primitive types
    let mut types = HashSet::new();
    for item in items {
        match item {
            Value::Null => {}
            Value::Bool(_) => {
                types.insert("bool");
            }
            Value::Number(n) => {
                if n.is_i64() {
                    types.insert("i64");
                } else {
                    types.insert("f64");
                }
            }
            Value::String(_) => {
                types.insert("String");
            }
            _ => {} // Already handled complex types above
        }
    }

    if types.len() == 1 {
        return Ok(types.into_iter().next().unwrap().to_string());
    }

    // Mixed types or can't determine
    Ok("serde_json::Value".to_string())
}

/// Generate async-graphql resolver
fn generate_async_graphql_resolver(name: &str, struct_names: &HashSet<String>) -> String {
    let type_name = sanitize_struct_name(name);

    let mut code = String::new();
    code.push_str("#[cfg(feature = \"graphql\")]\n#[derive(Default)]\npub struct Query;\n\n");

    code.push_str("#[cfg(feature = \"graphql\")]\n#[Object]\nimpl Query {\n");
    for struct_name in struct_names {
        let field_name = struct_name.to_case(Case::Snake);
        code.push_str(&format!(
            "    async fn get_{}(&self, ctx: &Context<'_>) -> Result<{}, async_graphql::Error> {{\n        // Add your resolver implementation here\n        unimplemented!()\n    }}\n\n",
            field_name, struct_name
        ));
    }
    code.push_str("}\n\n");

    code.push_str("#[cfg(feature = \"graphql\")]\n#[derive(Default)]\npub struct Mutation;\n\n");

    code.push_str("#[cfg(feature = \"graphql\")]\n#[Object]\nimpl Mutation {\n");
    for struct_name in struct_names {
        let field_name = struct_name.to_case(Case::Snake);
        code.push_str(&format!(
            "    async fn create_{}(&self, ctx: &Context<'_>, input: {}) -> Result<{}, async_graphql::Error> {{\n        // Add your resolver implementation here\n        unimplemented!()\n    }}\n\n",
            field_name, struct_name, struct_name
        ));

        code.push_str(&format!(
            "    async fn update_{}(&self, ctx: &Context<'_>, id: String, input: {}) -> Result<{}, async_graphql::Error> {{\n        // Add your resolver implementation here\n        unimplemented!()\n    }}\n\n",
            field_name, struct_name, struct_name
        ));

        code.push_str(&format!(
            "    async fn delete_{}(&self, ctx: &Context<'_>, id: String) -> Result<bool, async_graphql::Error> {{\n        // Add your resolver implementation here\n        unimplemented!()\n    }}\n\n",
            field_name
        ));
    }
    code.push_str("}\n\n");

    // Add schema creation helper
    code.push_str("#[cfg(feature = \"graphql\")]\n");
    code.push_str(&format!(
        "pub type {}Schema = async_graphql::Schema<Query, Mutation, async_graphql::EmptySubscription>;\n\n",
        type_name
    ));

    code.push_str("#[cfg(feature = \"graphql\")]\n");
    code.push_str(&format!(
        "pub fn create_{}_schema() -> {}Schema {{\n    {}Schema::build(Query::default(), Mutation::default(), async_graphql::EmptySubscription::default()).finish()\n}}\n",
        type_name.to_case(Case::Snake), type_name, type_name
    ));

    code
}

/// Generate GraphQL schema from JSON samples
pub fn generate_graphql_schema(
    name: &str,
    samples: &[Value],
    options: &GeneratorOptions,
) -> Result<String> {
    // Merge samples to get a complete schema
    let merged_schema = if samples.len() == 1 {
        samples[0].clone()
    } else {
        merge_json_samples(samples)?
    };

    let mut graphql_schema = String::new();
    let type_name = sanitize_struct_name(name);

    match merged_schema {
        Value::Object(map) => {
            // Create GraphQL type
            graphql_schema.push_str(&format!("type {} {{\n", type_name));

            for (key, val) in map {
                let field_name = sanitize_field_name(&key);
                let field_type = graphql_type_from_json(
                    &val,
                    &format!("{}_{}", type_name, key.to_case(Case::Pascal)),
                    options,
                )?;

                // Determine if this field is optional by checking across samples
                let is_optional = is_field_optional(samples, &key);

                // In GraphQL, required fields have "!" at the end
                let type_str = if !is_optional && !options.strict_option {
                    format!("{field_type}!")
                } else {
                    field_type
                };

                graphql_schema.push_str(&format!("  {}: {}\n", field_name, type_str));
            }

            graphql_schema.push_str("}\n\n");

            // Generate query
            graphql_schema.push_str("type Query {\n");
            graphql_schema.push_str(&format!("  get{}: {}\n", type_name, type_name));
            graphql_schema.push_str("}\n\n");

            // Generate mutation if appropriate
            graphql_schema.push_str("type Mutation {\n");
            graphql_schema.push_str(&format!("  create{0}: {0}\n", type_name));
            graphql_schema.push_str(&format!("  update{0}(id: ID!): {0}\n", type_name));
            graphql_schema.push_str(&format!("  delete{0}(id: ID!): Boolean!\n", type_name));
            graphql_schema.push_str("}\n\n");

            // Generate schema declaration
            graphql_schema.push_str("schema {\n");
            graphql_schema.push_str("  query: Query\n");
            graphql_schema.push_str("  mutation: Mutation\n");
            graphql_schema.push_str("}\n");

            Ok(graphql_schema)
        }
        _ => Err(anyhow!(
            "Root schema must be an object for GraphQL schema generation"
        )),
    }
}

/// Determine GraphQL type from JSON value
fn graphql_type_from_json(value: &Value, name: &str, options: &GeneratorOptions) -> Result<String> {
    match value {
        Value::Null => Ok("String".to_string()),
        Value::Bool(_) => Ok("Boolean".to_string()),
        Value::Number(n) => {
            if n.is_i64() {
                Ok("Int".to_string())
            } else {
                Ok("Float".to_string())
            }
        }
        Value::String(_) => Ok("String".to_string()),
        Value::Array(items) => {
            if items.is_empty() {
                return Ok("[String]".to_string());
            }

            // Use first item to determine type
            let item_type = graphql_type_from_json(&items[0], &format!("{}Item", name), options)?;

            Ok(format!("[{}]", item_type))
        }
        Value::Object(_) => {
            // For objects, use the name as the type
            let type_name = sanitize_struct_name(name);

            // Recursively define this type (handled separately)
            Ok(type_name)
        }
    }
}

/// Sanitize struct names to be valid Rust identifiers
fn sanitize_struct_name(name: &str) -> String {
    let pascal_case = name.to_case(Case::Pascal);

    // Ensure the name starts with a letter
    if pascal_case
        .chars()
        .next()
        .map_or(true, |c| !c.is_alphabetic())
    {
        return format!("S{}", pascal_case);
    }

    pascal_case
}

/// Sanitize field names to be valid Rust identifiers
fn sanitize_field_name(name: &str) -> String {
    let snake_case = name.to_case(Case::Snake);

    // If the field name is a Rust keyword, append an underscore
    if is_rust_keyword(&snake_case) {
        return format!("{}_", snake_case);
    }

    // Ensure the name starts with a letter or underscore
    if snake_case
        .chars()
        .next()
        .map_or(true, |c| !c.is_alphabetic() && c != '_')
    {
        return format!("f_{}", snake_case);
    }

    snake_case
}

/// Check if a string is a Rust keyword
fn is_rust_keyword(word: &str) -> bool {
    let keywords = [
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do",
        "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
    ];

    keywords.contains(&word)
}
