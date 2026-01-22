//! Parameter merging utilities for connectors
//!
//! This module provides unified parameter merging logic used across
//! source and sink connectors.

use crate::connectors::ParamMap;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_error::{ToStructError, UvsValidationFrom};

/// Merge connector parameters with user overrides, respecting whitelist.
///
/// This function combines base parameters from a connector definition with
/// user-provided overrides, ensuring that only whitelisted parameters can
/// be overridden.
///
/// # Arguments
///
/// * `base` - Base parameters from connector definition (default_params)
/// * `overrides` - User-provided parameter overrides (params_override)
/// * `allow_override` - Whitelist of parameter names that can be overridden
///
/// # Returns
///
/// * `Ok(ParamMap)` - Merged parameters with overrides applied
/// * `Err(...)` - If any override parameter is not in the whitelist
///
/// # Example
///
/// ```rust,ignore
/// use std::collections::BTreeMap;
/// use wp_conf::connectors::merge_params;
///
/// let mut base = BTreeMap::new();
/// base.insert("fmt".to_string(), "json".into());
/// base.insert("compression".to_string(), "none".into());
///
/// let mut overrides = BTreeMap::new();
/// overrides.insert("path".to_string(), "/data/logs".into());
///
/// let allow = vec!["path".to_string(), "fmt".to_string()];
///
/// let merged = merge_params(&base, &overrides, &allow)?;
/// // merged now contains: fmt=json, compression=none, path=/data/logs
/// ```
///
/// # Errors
///
/// Returns an error if any key in `overrides` is not present in `allow_override`.
/// The error message will include the rejected parameter name and the list of
/// allowed parameters.
pub fn merge_params(
    base: &ParamMap,
    overrides: &ParamMap,
    allow_override: &[String],
) -> OrionConfResult<ParamMap> {
    let mut result = base.clone();

    for (key, value) in overrides.iter() {
        // Check if parameter is in whitelist
        if !allow_override.iter().any(|allowed| allowed == key) {
            return ConfIOReason::from_validation(format!(
                "Parameter override '{}' not allowed. Permitted overrides: [{}]",
                key,
                allow_override.join(", ")
            ))
            .err_result();
        }

        // Merge the override
        result.insert(key.clone(), value.clone());
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_params_allows_whitelisted_override() {
        let mut base = ParamMap::new();
        base.insert("fmt".into(), json!("json"));
        base.insert("compression".into(), json!("gzip"));

        let mut overrides = ParamMap::new();
        overrides.insert("path".into(), json!("/data/test.log"));

        let allow = vec!["path".to_string()];

        let result = merge_params(&base, &overrides, &allow);
        assert!(result.is_ok(), "Should allow whitelisted override");

        let merged = result.unwrap();
        assert_eq!(merged.len(), 3);
        assert_eq!(merged.get("fmt").and_then(|v| v.as_str()), Some("json"));
        assert_eq!(
            merged.get("path").and_then(|v| v.as_str()),
            Some("/data/test.log")
        );
    }

    #[test]
    fn merge_params_rejects_non_whitelisted_override() {
        let base = ParamMap::new();
        let mut overrides = ParamMap::new();
        overrides.insert("dangerous_param".into(), json!("value"));

        let allow = vec!["safe_param".to_string()];

        let result = merge_params(&base, &overrides, &allow);
        assert!(result.is_err(), "Should reject non-whitelisted parameter");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("dangerous_param"),
            "Error should mention rejected parameter"
        );
        assert!(
            err_msg.contains("not allowed"),
            "Error should indicate parameter not allowed"
        );
    }

    #[test]
    fn merge_params_overwrites_base_values() {
        let mut base = ParamMap::new();
        base.insert("path".into(), json!("/default"));

        let mut overrides = ParamMap::new();
        overrides.insert("path".into(), json!("/custom"));

        let allow = vec!["path".to_string()];

        let merged = merge_params(&base, &overrides, &allow).unwrap();
        assert_eq!(
            merged.get("path").and_then(|v| v.as_str()),
            Some("/custom"),
            "Override should replace base value"
        );
    }

    #[test]
    fn merge_params_empty_overrides_returns_base() {
        let mut base = ParamMap::new();
        base.insert("key".into(), json!("value"));

        let overrides = ParamMap::new();
        let allow = vec!["key".to_string()];

        let merged = merge_params(&base, &overrides, &allow).unwrap();
        assert_eq!(merged, base, "Empty overrides should return base unchanged");
    }

    #[test]
    fn merge_params_handles_multiple_overrides() {
        let mut base = ParamMap::new();
        base.insert("a".into(), json!("1"));
        base.insert("b".into(), json!("2"));

        let mut overrides = ParamMap::new();
        overrides.insert("b".into(), json!("new_b"));
        overrides.insert("c".into(), json!("new_c"));

        let allow = vec!["b".to_string(), "c".to_string()];

        let merged = merge_params(&base, &overrides, &allow).unwrap();
        assert_eq!(merged.len(), 3, "Should have 3 parameters");
        assert_eq!(merged.get("a").and_then(|v| v.as_str()), Some("1"));
        assert_eq!(merged.get("b").and_then(|v| v.as_str()), Some("new_b"));
        assert_eq!(merged.get("c").and_then(|v| v.as_str()), Some("new_c"));
    }

    #[test]
    fn merge_params_preserves_base_params_not_in_overrides() {
        let mut base = ParamMap::new();
        base.insert("keep1".into(), json!("value1"));
        base.insert("keep2".into(), json!("value2"));
        base.insert("override_me".into(), json!("old"));

        let mut overrides = ParamMap::new();
        overrides.insert("override_me".into(), json!("new"));

        let allow = vec!["override_me".to_string()];

        let merged = merge_params(&base, &overrides, &allow).unwrap();
        assert_eq!(merged.len(), 3);
        assert_eq!(merged.get("keep1").and_then(|v| v.as_str()), Some("value1"));
        assert_eq!(merged.get("keep2").and_then(|v| v.as_str()), Some("value2"));
        assert_eq!(
            merged.get("override_me").and_then(|v| v.as_str()),
            Some("new")
        );
    }
}
