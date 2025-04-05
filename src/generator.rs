use anyhow::{anyhow, Result};
use convert_case::{Case, Casing};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

type StructMap = HashMap<String, String>;

/// Generate Rust struct code from JSON Value
pub fn generate_struct(name: &str, value: &Value) -> Result<String> {
    let mut code = String::from("use serde::{Serialize, Deserialize};\n\n");
    let mut generated_structs = StructMap::new();
    let mut struct_names = HashSet::new();

    generate_struct_recursive(name, value, &mut generated_structs, &mut struct_names)?;

    // Add all generated structs to the output
    for struct_code in generated_structs.values() {
        code.push_str(struct_code);
        code.push_str("\n\n");
    }

    Ok(code.trim().to_string())
}

/// Recursively generate struct definitions
fn generate_struct_recursive(
    name: &str,
    value: &Value,
    structs: &mut StructMap,
    struct_names: &mut HashSet<String>,
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
                let field_type = determine_type(
                    val,
                    &format!("{}_{}", type_name, key.to_case(Case::Pascal)),
                    structs,
                    struct_names,
                )?;

                // Add serde rename if field name differs from JSON key
                if field_name != *key {
                    struct_code.push_str(&format!("    #[serde(rename = \"{}\")]\n", key));
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
            )?;
            Ok(format!("Vec<{}>", item_type))
        }
        _ => Err(anyhow!(
            "Expected object or array at root level for struct generation"
        )),
    }
}

/// Determine Rust type from JSON value
fn determine_type(
    value: &Value,
    name: &str,
    structs: &mut StructMap,
    struct_names: &mut HashSet<String>,
) -> Result<String> {
    match value {
        Value::Null => Ok("Option<serde_json::Value>".to_string()),
        Value::Bool(_) => Ok("bool".to_string()),
        Value::Number(n) => {
            if n.is_i64() {
                Ok("i64".to_string())
            } else {
                Ok("f64".to_string())
            }
        }
        Value::String(_) => Ok("String".to_string()),
        Value::Array(items) => {
            if items.is_empty() {
                return Ok("Vec<serde_json::Value>".to_string());
            }

            let item_type =
                determine_array_item_type(items, &format!("{}Item", name), structs, struct_names)?;
            Ok(format!("Vec<{}>", item_type))
        }
        Value::Object(_) => {
            let type_name = generate_struct_recursive(name, value, structs, struct_names)?;
            Ok(type_name)
        }
    }
}

/// Determine common type for array items
fn determine_array_item_type(
    items: &[Value],
    name: &str,
    structs: &mut StructMap,
    struct_names: &mut HashSet<String>,
) -> Result<String> {
    // If any item is an object, we'll need a custom struct
    if items.iter().any(|item| item.is_object()) {
        // Get the first object to use as a template
        if let Some(obj) = items.iter().find(|&item| item.is_object()) {
            return generate_struct_recursive(name, obj, structs, struct_names);
        }
    }

    // For arrays of arrays
    if items.iter().any(|item| item.is_array()) {
        if let Some(arr) = items.iter().find(|&item| item.is_array()) {
            let inner_type = determine_array_item_type(
                arr.as_array().unwrap(),
                &format!("{}Inner", name),
                structs,
                struct_names,
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
