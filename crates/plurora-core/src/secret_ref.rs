//! Generic secret reference types and validation utilities.
//!
//! This module provides the contract for referencing secrets without
//! embedding raw secret values. Packages and profiles use `secret_ref`
//! identifiers; the host resolves them at runtime. Raw secrets must
//! never appear in events, proposals, logs, or audit records.

use serde::{Deserialize, Serialize};

/// The canonical prefix for a secret reference string.
///
/// A valid secret reference is of the form `secret_ref:<vault>:<key>`,
/// e.g. `secret_ref:env:OPENAI_API_KEY` or `secret_ref:vault:prod/openai`.
pub const SECRET_REF_PREFIX: &str = "secret_ref:";

/// Alternative prefixes that are recognized as secret references
/// for compatibility with common naming conventions.
pub const SECRET_REF_ALT_PREFIXES: &[&str] = &["secretRef:", "secret-ref:"];

/// Field names commonly associated with raw secret values.
/// Used by the redaction scanner to detect leaked secrets.
pub const SECRET_FIELD_NAMES: &[&str] = &[
    "api_key",
    "apikey",
    "api_secret",
    "apisecret",
    "secret_key",
    "secretkey",
    "secret",
    "token",
    "access_token",
    "access_secret",
    "auth_token",
    "password",
    "passwd",
    "private_key",
    "privatekey",
    "credential",
    "credentials",
    "bearer_token",
    "x-api-key",
];

/// A structured secret reference that can be included in payloads
/// instead of raw secret values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRef {
    /// The reference identifier, e.g. `"secret_ref:env:OPENAI_API_KEY"`.
    pub ref_id: String,
    /// Optional human-readable label for the target secret.
    #[serde(default)]
    pub label: Option<String>,
}

impl SecretRef {
    /// Create a new secret reference with the given ref_id.
    pub fn new(ref_id: impl Into<String>) -> Self {
        Self {
            ref_id: ref_id.into(),
            label: None,
        }
    }

    /// Create a secret reference with a human-readable label.
    pub fn with_label(ref_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            ref_id: ref_id.into(),
            label: Some(label.into()),
        }
    }

    /// Check whether a string looks like a valid secret reference.
    ///
    /// Valid forms:
    /// - `secret_ref:<vault>:<key>` (canonical)
    /// - `secretRef:<vault>:<key>` (camelCase variant)
    /// - `secret-ref:<vault>:<key>` (kebab-case variant)
    /// - `host:xxx` references for host-injected secrets
    pub fn is_valid_ref(s: &str) -> bool {
        if s.starts_with(SECRET_REF_PREFIX)
            || SECRET_REF_ALT_PREFIXES.iter().any(|p| s.starts_with(p))
        {
            // Must have at least vault:key after the prefix
            let after_prefix = s.find(':').map(|i| &s[i + 1..]).unwrap_or("");
            after_prefix.contains(':') && after_prefix.len() > 2
        } else if s.starts_with("host:") {
            // Host-injected secret references: `host:<key>`
            s.len() > 5
        } else {
            false
        }
    }
}

/// Check whether a string is a valid **env-backed** secret reference.
///
/// This is stricter than [`SecretRef::is_valid_ref`]: it only accepts
/// references that resolve via environment variables. Currently, only
/// the `env` vault is supported; other vault types (e.g. `vault:`,
/// `aws:`) are rejected.
///
/// Valid forms:
/// - `secret_ref:env:NAME` (canonical)
/// - `secretRef:env:NAME` (camelCase)
/// - `secret-ref:env:NAME` (kebab-case)
/// - `host:env:NAME` (host env compat)
///
/// Returns `false` for:
/// - Non-env vaults: `secret_ref:vault:key`
/// - Bare host refs: `host:my_secret` (no `env:` prefix)
/// - Malformed or unrecognized strings
pub fn is_env_backed_ref(s: &str) -> bool {
    // Canonical: secret_ref:env:NAME
    if let Some(rest) = s.strip_prefix("secret_ref:") {
        if let Some(name) = rest.strip_prefix("env:") {
            return !name.is_empty();
        }
        return false;
    }
    // camelCase: secretRef:env:NAME
    if let Some(rest) = s.strip_prefix("secretRef:") {
        if let Some(name) = rest.strip_prefix("env:") {
            return !name.is_empty();
        }
        return false;
    }
    // kebab-case: secret-ref:env:NAME
    if let Some(rest) = s.strip_prefix("secret-ref:") {
        if let Some(name) = rest.strip_prefix("env:") {
            return !name.is_empty();
        }
        return false;
    }
    // host:env:NAME (but NOT host:<other-key>)
    if let Some(rest) = s.strip_prefix("host:") {
        if let Some(name) = rest.strip_prefix("env:") {
            return !name.is_empty();
        }
        return false;
    }
    false
}

/// Check whether a string is a valid **store-backed** secret reference.
///
/// Valid forms:
/// - `secret_ref:store:NAME` (canonical)
/// - `secretRef:store:NAME` (camelCase)
/// - `secret-ref:store:NAME` (kebab-case)
pub fn is_store_backed_ref(s: &str) -> bool {
    extract_store_name(s).is_some()
}

/// Extract the store secret name from a store-backed reference.
pub fn extract_store_name(ref_id: &str) -> Option<&str> {
    for prefix in &["secret_ref:", "secretRef:", "secret-ref:"] {
        if let Some(rest) = ref_id.strip_prefix(prefix) {
            if let Some(name) = rest.strip_prefix("store:") {
                if !name.is_empty() {
                    return Some(name);
                }
            }
            return None;
        }
    }
    None
}

/// Check whether a string is a valid **project-backed** secret reference.
///
/// Valid forms:
/// - `secret_ref:project:NAME` (canonical)
/// - `secretRef:project:NAME` (camelCase)
/// - `secret-ref:project:NAME` (kebab-case)
pub fn is_project_backed_ref(s: &str) -> bool {
    extract_project_name(s).is_some()
}

/// Extract the project secret name from a project-backed reference.
pub fn extract_project_name(ref_id: &str) -> Option<&str> {
    for prefix in &["secret_ref:", "secretRef:", "secret-ref:"] {
        if let Some(rest) = ref_id.strip_prefix(prefix) {
            if let Some(name) = rest.strip_prefix("project:") {
                if !name.is_empty() {
                    return Some(name);
                }
            }
            return None;
        }
    }
    None
}

/// Check whether a string value looks like a raw secret (not a reference).
///
/// This uses heuristic patterns to detect values that look like API keys
/// or bearer tokens. It is intentionally conservative to avoid false
/// positives on ordinary text.
pub fn looks_like_raw_secret(value: &str) -> bool {
    // Common API key patterns: long base64-like or hex-like strings
    // that don't start with known secret_ref prefixes
    if SecretRef::is_valid_ref(value) {
        return false;
    }

    // Bearer token pattern
    if value.starts_with("Bearer ") || value.starts_with("bearer ") {
        return true;
    }

    // Long alphanumeric strings typical of API keys (>= 32 chars, high entropy)
    if value.len() >= 32
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        // Check for mixed-case/digit pattern typical of API keys
        let has_upper = value.chars().any(|c| c.is_ascii_uppercase());
        let has_lower = value.chars().any(|c| c.is_ascii_lowercase());
        let has_digit = value.chars().any(|c| c.is_ascii_digit());
        if has_upper && has_lower && has_digit {
            return true;
        }
        // Pure hex-like string of length >= 32
        if value.len() >= 32 && value.chars().all(|c| c.is_ascii_hexdigit()) {
            return true;
        }
    }

    // sk- prefixed keys (OpenAI, etc.)
    if value.starts_with("sk-") || value.starts_with("sk_") {
        return true;
    }

    false
}

/// Check whether a field name is a known secret field name.
pub fn is_secret_field_name(field_name: &str) -> bool {
    let lower = field_name.to_lowercase();
    SECRET_FIELD_NAMES.iter().any(|name| lower == *name)
        || lower.contains("secret")
            && !lower.contains("secret_ref")
            && !lower.contains("secretref")
            && !lower.contains("secret-ref")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_secret_ref_forms() {
        assert!(SecretRef::is_valid_ref("secret_ref:env:OPENAI_API_KEY"));
        assert!(SecretRef::is_valid_ref("secretRef:env:MY_KEY"));
        assert!(SecretRef::is_valid_ref("secret-ref:vault:prod/key"));
        assert!(SecretRef::is_valid_ref("host:my_secret"));
    }

    #[test]
    fn invalid_secret_ref_forms() {
        assert!(!SecretRef::is_valid_ref("secret_ref:"));
        assert!(!SecretRef::is_valid_ref("secret_ref:x"));
        assert!(!SecretRef::is_valid_ref("not_a_secret_ref"));
        assert!(!SecretRef::is_valid_ref(""));
        assert!(!SecretRef::is_valid_ref("host:"));
    }

    #[test]
    fn raw_secret_detection() {
        assert!(looks_like_raw_secret("sk-abc123def456ghi789jkl012mno345"));
        assert!(looks_like_raw_secret("Bearer abc123def456"));
        let stripe_like = ["sk", "_live_", "abcdefghijklmnopqrstuvwx"].concat();
        assert!(looks_like_raw_secret(&stripe_like));
        assert!(!looks_like_raw_secret("secret_ref:env:MY_KEY"));
        assert!(!looks_like_raw_secret("hello world"));
        assert!(!looks_like_raw_secret("short"));
        assert!(!looks_like_raw_secret("regular-text-no-secret"));
    }

    #[test]
    fn secret_field_name_detection() {
        assert!(is_secret_field_name("api_key"));
        assert!(is_secret_field_name("API_KEY"));
        assert!(is_secret_field_name("token"));
        assert!(is_secret_field_name("password"));
        assert!(is_secret_field_name("x-api-key"));
        assert!(!is_secret_field_name("secret_ref"));
        assert!(!is_secret_field_name("secretRef"));
        assert!(!is_secret_field_name("title"));
        assert!(!is_secret_field_name("content"));
    }

    #[test]
    fn secret_ref_roundtrip() {
        let r = SecretRef::new("secret_ref:env:MY_KEY");
        assert_eq!(r.ref_id, "secret_ref:env:MY_KEY");
        assert!(r.label.is_none());

        let r = SecretRef::with_label("secret_ref:env:MY_KEY", "OpenAI key");
        assert_eq!(r.label, Some("OpenAI key".to_string()));
    }

    #[test]
    fn env_backed_ref_accepts_canonical_forms() {
        assert!(is_env_backed_ref("secret_ref:env:OPENAI_API_KEY"));
        assert!(is_env_backed_ref("secretRef:env:MY_KEY"));
        assert!(is_env_backed_ref("secret-ref:env:SOME_VAR"));
        assert!(is_env_backed_ref("host:env:DEEPSEEK_KEY"));
    }

    #[test]
    fn env_backed_ref_rejects_non_env_vaults() {
        assert!(!is_env_backed_ref("secret_ref:vault:prod/key"));
        assert!(!is_env_backed_ref("secretRef:vault:my-secret"));
        assert!(!is_env_backed_ref("secret-ref:aws:secret_name"));
    }

    #[test]
    fn env_backed_ref_rejects_bare_host_ref() {
        assert!(!is_env_backed_ref("host:my_secret"));
        assert!(!is_env_backed_ref("host:token_value"));
    }

    #[test]
    fn env_backed_ref_rejects_malformed() {
        assert!(!is_env_backed_ref("not_a_secret_ref"));
        assert!(!is_env_backed_ref(""));
        assert!(!is_env_backed_ref("secret_ref:env:"));
        assert!(!is_env_backed_ref("host:env:"));
        assert!(!is_env_backed_ref("secret_ref:"));
    }

    #[test]
    fn store_backed_ref_accepts_canonical_form() {
        assert!(is_store_backed_ref("secret_ref:store:OPENAI_API_KEY"));
    }

    #[test]
    fn store_backed_ref_rejects_env_and_empty_name() {
        assert!(!is_store_backed_ref("secret_ref:env:OPENAI_API_KEY"));
        assert!(!is_store_backed_ref("secret_ref:store:"));
    }

    #[test]
    fn extract_store_name_returns_name() {
        assert_eq!(
            extract_store_name("secret_ref:store:MY_KEY"),
            Some("MY_KEY")
        );
    }

    #[test]
    fn store_backed_ref_accepts_all_prefix_variants() {
        assert!(is_store_backed_ref("secret_ref:store:MY_KEY"));
        assert!(is_store_backed_ref("secretRef:store:MY_KEY"));
        assert!(is_store_backed_ref("secret-ref:store:MY_KEY"));
        assert_eq!(extract_store_name("secretRef:store:MY_KEY"), Some("MY_KEY"));
        assert_eq!(
            extract_store_name("secret-ref:store:MY_KEY"),
            Some("MY_KEY")
        );
    }

    #[test]
    fn project_backed_ref_accepts_all_prefix_variants() {
        assert!(is_project_backed_ref("secret_ref:project:MY_KEY"));
        assert!(is_project_backed_ref("secretRef:project:MY_KEY"));
        assert!(is_project_backed_ref("secret-ref:project:MY_KEY"));
        assert_eq!(
            extract_project_name("secret_ref:project:MY_KEY"),
            Some("MY_KEY")
        );
        assert_eq!(
            extract_project_name("secretRef:project:MY_KEY"),
            Some("MY_KEY")
        );
        assert_eq!(
            extract_project_name("secret-ref:project:MY_KEY"),
            Some("MY_KEY")
        );
    }

    #[test]
    fn project_backed_ref_rejects_non_project_vault_and_empty_name() {
        assert!(!is_project_backed_ref("secret_ref:env:OPENAI_API_KEY"));
        assert!(!is_project_backed_ref("secret_ref:store:OPENAI_API_KEY"));
        assert!(!is_project_backed_ref("secret_ref:vault:OPENAI_API_KEY"));
        assert!(!is_project_backed_ref("secret_ref:project:"));
    }

    #[test]
    fn project_backed_ref_rejects_malformed() {
        assert!(!is_project_backed_ref("not_a_secret_ref"));
        assert!(!is_project_backed_ref(""));
        assert!(!is_project_backed_ref("secret_ref:"));
        assert!(!is_project_backed_ref("project:MY_KEY"));
        assert_eq!(extract_project_name("secret_ref:project:"), None);
    }
}
