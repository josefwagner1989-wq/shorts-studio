//! Tolerant JSON extraction from model replies (code fences, a sentence around the value).

use crate::{Error, Result};
use serde::de::DeserializeOwned;

pub fn extract<T: DeserializeOwned>(reply: &str) -> Result<T> {
    let trimmed = reply.trim();
    if let Ok(v) = serde_json::from_str(trimmed) {
        return Ok(v);
    }
    if let Some(start) = trimmed.find("```") {
        let after = &trimmed[start + 3..];
        let after = after.strip_prefix("json").unwrap_or(after);
        if let Some(end) = after.find("```") {
            if let Ok(v) = serde_json::from_str(after[..end].trim()) {
                return Ok(v);
            }
        }
    }
    let first = trimmed.find(['{', '[']);
    let last = trimmed.rfind(['}', ']']);
    if let (Some(a), Some(b)) = (first, last) {
        if b > a {
            if let Ok(v) = serde_json::from_str(&trimmed[a..=b]) {
                return Ok(v);
            }
        }
    }
    Err(Error::Parse(trimmed.chars().take(200).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn plain_fenced_and_wrapped() {
        assert!(extract::<Value>("{\"a\":1}").is_ok());
        assert!(extract::<Value>("```json\n[1,2]\n```").is_ok());
        assert!(extract::<Value>("Here you go: {\"a\": [1]} hope it helps").is_ok());
        assert!(extract::<Value>("no json here").is_err());
    }
}
