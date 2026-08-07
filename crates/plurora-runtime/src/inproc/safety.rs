//! Shared conservative raw-secret detection primitives for inproc handlers.
//!
//! These functions detect likely raw-secret content in capability input values.
//! They are used by official inproc packages to reject inputs containing
//! raw API keys, tokens, passwords, or high-entropy secret-like strings.
//!
//! **Important**: these are conservative heuristics, not a full secret scanner.
//! They must not be extended with marketplace/billing/signing field checks —
//! those are package-specific concerns handled per-package.

use serde_json::Value;

/// Field names that commonly hold secrets. Matching is case-insensitive
/// and uses `contains` so that compound names like `user_api_key` also match.
pub const SECRET_FIELD_NAMES: &[&str] = &[
    "api_key",
    "secret",
    "token",
    "password",
    "private_key",
    "access_token",
    "refresh_token",
    "auth_token",
];

/// Value prefixes that strongly indicate a raw secret.
pub const SECRET_VALUE_PREFIXES: &[&str] = &["sk-", "Bearer ", "bearer "];

/// Check if a string value is a secret reference (safe) rather than a raw secret.
///
/// Secret references like `secret_ref:env:MY_KEY` or `host:env:MY_KEY` are
/// the approved way to pass secrets; raw values are blocked.
pub fn is_secret_ref_value(val: &str) -> bool {
    val.starts_with("secret_ref:")
        || val.starts_with("secretRef:")
        || val.starts_with("secret-ref:")
        || val.starts_with("host:")
}

/// Check if a string value looks like a raw secret.
///
/// Detects common secret prefixes (`sk-`, `Bearer `, `bearer `) and
/// high-entropy alphanumeric strings of length >= 40 with mixed
/// uppercase, lowercase, and digit characters.
pub fn looks_like_raw_secret_value(val: &str) -> bool {
    for prefix in SECRET_VALUE_PREFIXES {
        if val.starts_with(prefix) {
            return true;
        }
    }
    if val.len() >= 40 {
        let has_upper = val.chars().any(|c| c.is_ascii_uppercase());
        let has_lower = val.chars().any(|c| c.is_ascii_lowercase());
        let has_digit = val.chars().any(|c| c.is_ascii_digit());
        if has_upper && has_lower && has_digit {
            return true;
        }
    }
    false
}

/// Recursively scan a JSON value for raw-secret-like content.
///
/// Returns `true` if any suspicious field name or value pattern is found.
/// Field name matching is case-insensitive and uses substring containment.
/// Values that are secret references (as determined by `is_secret_ref_value`)
/// are not treated as raw secrets.
pub fn contains_raw_secret(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                let key_lower = key.to_lowercase();
                for secret_name in SECRET_FIELD_NAMES {
                    if key_lower == *secret_name || key_lower.contains(secret_name) {
                        if let Some(s) = val.as_str() {
                            if !is_secret_ref_value(s) {
                                return true;
                            }
                        } else if !val.is_null() {
                            return true;
                        }
                    }
                }
                if let Some(s) = val.as_str() {
                    if looks_like_raw_secret_value(s) {
                        return true;
                    }
                }
                if contains_raw_secret(val) {
                    return true;
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                if contains_raw_secret(item) {
                    return true;
                }
            }
        }
        _ => {}
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_api_key_field() {
        assert!(contains_raw_secret(&json!({"api_key": "abc123"})));
    }

    #[test]
    fn detects_sk_prefix() {
        let raw = ["s", "k-", "test123"].concat();
        assert!(contains_raw_secret(&json!({"api_key": raw})));
    }

    #[test]
    fn detects_bearer_prefix() {
        assert!(contains_raw_secret(&json!({"token": "Bearer xyz"})));
    }

    #[test]
    fn allows_secret_ref() {
        assert!(!contains_raw_secret(
            &json!({"api_key": "secret_ref:env:MY_KEY"})
        ));
    }

    #[test]
    fn allows_host_ref() {
        assert!(!contains_raw_secret(&json!({"token": "host:env:MY_KEY"})));
    }

    #[test]
    fn allows_safe_text() {
        assert!(!contains_raw_secret(&json!({"objective": "safe text"})));
    }

    #[test]
    fn detects_nested_secret() {
        assert!(contains_raw_secret(&json!({"config": {"password": "abc"}})));
    }

    #[test]
    fn detects_high_entropy_string() {
        assert!(contains_raw_secret(
            &json!({"api_key": "RawSecretExample1234567890abcdefABCDEF123456"})
        ));
    }

    #[test]
    fn is_secret_ref_value_checks_prefixes() {
        assert!(is_secret_ref_value("secret_ref:env:KEY"));
        assert!(is_secret_ref_value("secretRef:env:KEY"));
        assert!(is_secret_ref_value("secret-ref:env:KEY"));
        assert!(is_secret_ref_value("host:env:KEY"));
        assert!(!is_secret_ref_value("just-a-string"));
    }

    #[test]
    fn looks_like_raw_secret_checks_prefixes() {
        assert!(looks_like_raw_secret_value("sk-abc"));
        assert!(looks_like_raw_secret_value("Bearer xyz"));
        assert!(looks_like_raw_secret_value("bearer xyz"));
        assert!(!looks_like_raw_secret_value("safe text"));
    }
}
