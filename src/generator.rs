use anyhow::{anyhow, Result};
use convert_case::{Case, Casing};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

type StructMap = HashMap<String, String>;

#[derive(Debug, Clone)]
pub struct GeneratorOptions {
    pub strict_option: bool, // Use Option<T> for all fields
    pub flatten: bool,       // Use #[serde(flatten)] for nested objects
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

    let mut code = String::from("use serde::{Serialize, Deserialize};\n\n");
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
        &samples,
    )?;

    // Add all generated structs to the output
    for struct_code in generated_structs.values() {
        code.push_str(struct_code);
        code.push_str("\n\n");
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
            let mut struct_code = format!(
                "#[derive(Debug, Serialize, Deserialize)]\npub struct {} {{\n",
                type_name
            );

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

                // Add serde annotations
                if field_name != *key || options.flatten && val.is_object() {
                    struct_code.push_str("    #[serde(");

                    let mut annotations = Vec::new();

                    if field_name != *key {
                        annotations.push(format!("rename = \"{}\"", key));
                    }

                    if options.flatten && val.is_object() {
                        annotations.push("flatten".to_string());
                    }

                    if is_optional || options.strict_option {
                        annotations.push("skip_serializing_if = \"Option::is_none\"".to_string());
                    }

                    struct_code.push_str(&annotations.join(", "));
                    struct_code.push_str(")]\n");
                }

                struct_code.push_str(&format!("    pub {}: {},\n", field_name, field_type));
            }

            struct_code.push_str("}\n");
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

/// Determine if we can create an enum for a field
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
