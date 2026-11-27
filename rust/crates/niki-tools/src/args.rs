use serde_json::{Map, Value};

use crate::ToolError;

fn object(args: &Value) -> Result<&Map<String, Value>, ToolError> {
    args.as_object().ok_or_else(|| {
        ToolError::InvalidArguments("tool arguments must be a JSON object".to_owned())
    })
}

pub(crate) fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    match object(args)?.get(key) {
        Some(Value::String(value)) => Ok(value),
        Some(_) => Err(ToolError::InvalidArguments(format!(
            "`{key}` must be a string"
        ))),
        None => Err(ToolError::InvalidArguments(format!(
            "missing required argument `{key}`"
        ))),
    }
}

/// The first of `keys` present as a string.
///
/// The wire vocabulary calls the file argument `file_path` (and the edit
/// strings `old_string`/`new_string`), while this crate's tools take
/// `path`/`old`/`new`. Accept either spelling so an instruction, skill, or hook
/// keeps working.
pub(crate) fn required_str_any<'a>(args: &'a Value, keys: &[&str]) -> Result<&'a str, ToolError> {
    let map = object(args)?;
    for key in keys {
        match map.get(*key) {
            Some(Value::String(value)) => return Ok(value),
            Some(_) => {
                return Err(ToolError::InvalidArguments(format!(
                    "`{key}` must be a string"
                )));
            }
            None => {}
        }
    }
    let accepted = if keys.len() > 1 {
        format!(" (also accepted: {})", keys[1..].join(", "))
    } else {
        String::new()
    };
    Err(ToolError::InvalidArguments(format!(
        "missing required argument `{}`{accepted}",
        keys[0]
    )))
}

pub(crate) fn optional_str<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>, ToolError> {
    match object(args)?.get(key) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(ToolError::InvalidArguments(format!(
            "`{key}` must be a string"
        ))),
    }
}

pub(crate) fn optional_u32(args: &Value, key: &str) -> Result<Option<u32>, ToolError> {
    match object(args)?.get(key) {
        None => Ok(None),
        Some(Value::Number(number)) => {
            let value = number
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| {
                    ToolError::InvalidArguments(format!(
                        "`{key}` must be a non-negative integer below 2^32"
                    ))
                })?;
            Ok(Some(value))
        }
        Some(_) => Err(ToolError::InvalidArguments(format!(
            "`{key}` must be an integer"
        ))),
    }
}

pub(crate) fn optional_bool(args: &Value, key: &str) -> Result<Option<bool>, ToolError> {
    match object(args)?.get(key) {
        None => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(ToolError::InvalidArguments(format!(
            "`{key}` must be a boolean"
        ))),
    }
}
