use anyhow::Result;
use serde_json::Value;

pub(super) fn extract_verbose_option(value: &Value) -> Result<String> {
    match value {
        Value::Number(verbose) => Ok(verbose.to_string()),
        _ => Ok("0".to_string()),
    }
}
