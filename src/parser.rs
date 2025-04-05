use anyhow::{anyhow, Result};
use serde_json::Value;

/// Parse and validate JSON string
pub fn parse_json(json_str: &str) -> Result<Value> {
    let parsed: Value =
        serde_json::from_str(json_str).map_err(|e| anyhow!("Invalid JSON: {}", e))?;

    // Ensure the JSON is an object for struct generation
    if !parsed.is_object() && !parsed.is_array() {
        return Err(anyhow!("Root JSON must be an object or array"));
    }

    Ok(parsed)
}
