//! Raw-secret redaction and blocking utilities.
//!
//! This module provides conservative scanning of trusted payload paths
//! for leaked raw secrets. It checks known secret field names and
//! value patterns in:
//!
//! - Proposal payloads (operations and expected_effects)
//! - Asset metadata
//! - Audit-like payloads (permission events, platform events)
//!
//! The general scanner is intentionally conservative: it targets obvious
//! API-key field names and high-entropy value patterns, while excluding
//! arbitrary user-content fields to avoid false positives. Effect evidence
//! uses the strict scanner and redactor, which includes those fields.

use plurora_core::{is_secret_field_name, looks_like_raw_secret};
use serde_json::Value;

/// Result of a raw-secret scan.
#[derive(Debug, Clone)]
pub struct SecretScanResult {
    /// Fields where raw secrets were detected.
    pub findings: Vec<SecretFinding>,
}

/// A single finding of a raw secret in a payload.
#[derive(Debug, Clone)]
pub struct SecretFinding {
    /// The JSON path where the secret was found (dot-separated).
    pub path: String,
    /// The field name that triggered the finding.
    pub field_name: String,
    /// The kind of detection that triggered.
    pub detection: SecretDetection,
}

/// How the secret was detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretDetection {
    /// The field name matches a known secret field name.
    FieldName,
    /// The value looks like a raw secret (API key pattern).
    ValuePattern,
}

impl SecretScanResult {
    /// Whether any raw secrets were found.
    pub fn has_findings(&self) -> bool {
        !self.findings.is_empty()
    }
}

/// Scan a JSON value for raw secrets.
///
/// This is the entry point for conservative secret scanning.
/// It recursively walks the value, checking field names and
/// string values against known patterns.
///
/// Paths that are excluded from scanning (to avoid false positives):
/// - `content` fields (asset content is arbitrary user data)
/// - `description` fields
/// - `title` fields
/// - `reason` fields
/// - `message` fields
/// - `label` fields
/// - `summary` fields
pub fn scan_value_for_raw_secrets(value: &Value, root_path: &str) -> SecretScanResult {
    let mut result = SecretScanResult {
        findings: Vec::new(),
    };
    scan_recursive(value, root_path, &mut result, true);
    result
}

/// Strict scan for persisted effect evidence. Unlike the general proposal/asset
/// scanner, this checks arbitrary content-like fields because receipts must not
/// persist raw credential-shaped material by reference.
pub fn scan_effect_value_for_raw_secrets(value: &Value, root_path: &str) -> SecretScanResult {
    let mut result = SecretScanResult {
        findings: Vec::new(),
    };
    scan_recursive(value, root_path, &mut result, false);
    result
}

/// Fields that are excluded from value-pattern scanning to avoid false positives.
/// Field-name detection still applies, but we don't run looks_like_raw_secret
/// on the values of these fields.
const EXCLUDED_VALUE_SCAN_FIELDS: &[&str] = &[
    "content",
    "description",
    "title",
    "reason",
    "message",
    "label",
    "summary",
    "text",
    "body",
    "note",
    "comment",
    "display_name",
    "author",
];

fn scan_recursive(
    value: &Value,
    path: &str,
    result: &mut SecretScanResult,
    honor_value_exclusions: bool,
) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };
                scan_recursive(child, &child_path, result, honor_value_exclusions);
            }
        }
        Value::Array(arr) => {
            for (i, child) in arr.iter().enumerate() {
                let child_path = format!("{}[{}]", path, i);
                scan_recursive(child, &child_path, result, honor_value_exclusions);
            }
        }
        Value::String(string) => {
            let field_name = path
                .rsplit('.')
                .next()
                .unwrap_or(path)
                .split('[')
                .next()
                .unwrap_or_default();
            if is_secret_field_name(field_name) && !plurora_core::SecretRef::is_valid_ref(string) {
                result.findings.push(SecretFinding {
                    path: path.to_string(),
                    field_name: field_name.to_string(),
                    detection: SecretDetection::FieldName,
                });
            }
            let is_excluded = honor_value_exclusions
                && EXCLUDED_VALUE_SCAN_FIELDS
                    .iter()
                    .any(|excluded| *excluded == field_name);
            if !is_excluded
                && contains_raw_secret_pattern(string)
                && !plurora_core::SecretRef::is_valid_ref(string)
            {
                result.findings.push(SecretFinding {
                    path: path.to_string(),
                    field_name: field_name.to_string(),
                    detection: SecretDetection::ValuePattern,
                });
            }
        }
        _ => {}
    }
}

/// Redact a JSON value by replacing detected raw secret values
/// with `<secret:redacted>`.
///
/// Returns the redacted value and the scan result.
pub fn redact_secrets_in_value(value: &Value) -> (Value, SecretScanResult) {
    let scan = scan_value_for_raw_secrets(value, "");
    if scan.findings.is_empty() {
        return (value.clone(), scan);
    }
    let mut redacted = value.clone();
    for finding in &scan.findings {
        apply_redaction(&mut redacted, &finding.path);
    }
    (redacted, scan)
}

fn contains_raw_secret_pattern(value: &str) -> bool {
    looks_like_raw_secret(value)
        || value.contains("Bearer ")
        || value.contains("bearer ")
        || value
            .split(|character: char| {
                !(character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
            })
            .any(|candidate| {
                !candidate
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
                    && looks_like_raw_secret(candidate)
            })
}

pub fn redact_effect_value(value: &Value) -> (Value, SecretScanResult) {
    let scan = scan_effect_value_for_raw_secrets(value, "");
    if scan.findings.is_empty() {
        return (value.clone(), scan);
    }
    let mut redacted = value.clone();
    for finding in &scan.findings {
        apply_redaction(&mut redacted, &finding.path);
    }
    (redacted, scan)
}

fn apply_redaction(value: &mut Value, path: &str) {
    let parts: Vec<&str> = path.split('.').collect();
    redact_path_recursive(value, &parts);
}

fn redact_path_recursive(value: &mut Value, parts: &[&str]) {
    let Some((segment, rest)) = parts.split_first() else {
        return;
    };
    let Some(target) = descend_path_segment_mut(value, segment) else {
        return;
    };
    if rest.is_empty() {
        *target = Value::String("<secret:redacted>".to_string());
    } else {
        redact_path_recursive(target, rest);
    }
}

fn descend_path_segment_mut<'a>(value: &'a mut Value, segment: &str) -> Option<&'a mut Value> {
    let (key, index) = match segment.split_once('[') {
        Some((key, index)) if index.ends_with(']') => {
            (key, index.trim_end_matches(']').parse::<usize>().ok())
        }
        _ => (segment, None),
    };
    let target = if key.is_empty() {
        value
    } else {
        value.as_object_mut()?.get_mut(key)?
    };
    match index {
        Some(index) => target.as_array_mut()?.get_mut(index),
        None => Some(target),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_api_key_field_name() {
        let value = json!({"api_key": "somevalue"});
        let result = scan_value_for_raw_secrets(&value, "");
        assert!(result.has_findings());
        assert_eq!(result.findings[0].detection, SecretDetection::FieldName);
    }

    #[test]
    fn allows_secret_ref_in_secret_field() {
        let value = json!({"secret": "secret_ref:env:MY_KEY"});
        let result = scan_value_for_raw_secrets(&value, "");
        assert!(!result.has_findings());
    }

    #[test]
    fn detects_raw_secret_value_pattern() {
        let value = json!({"config": {"bearer": "Bearer sk-abc123def456ghi789jkl012mno345pqr"}});
        let result = scan_value_for_raw_secrets(&value, "");
        assert!(result.has_findings());
    }

    #[test]
    fn detects_raw_secret_embedded_in_diagnostic_text() {
        let value = json!({
            "diagnostic": "deployment failed with sk-Abcdefghijklmnopqrstuvwxyz123456"
        });
        assert!(scan_effect_value_for_raw_secrets(&value, "receipt").has_findings());
    }

    #[test]
    fn scans_root_and_array_strings() {
        let secret = "sk-Abcdefghijklmnopqrstuvwxyz123456";
        assert!(scan_effect_value_for_raw_secrets(
            &Value::String(secret.to_string()),
            "diagnostic"
        )
        .has_findings());
        assert!(scan_effect_value_for_raw_secrets(&json!([secret]), "diagnostics").has_findings());
    }

    #[test]
    fn content_digests_and_typed_container_refs_are_not_secrets() {
        let value = json!({
            "digest": format!("sha256:{}", "a".repeat(64)),
            "image": format!("registry.example/app@sha256:{}", "b".repeat(64)),
            "container_id": format!("docker:{}", "c".repeat(64))
        });
        assert!(!scan_effect_value_for_raw_secrets(&value, "receipt").has_findings());
    }

    #[test]
    fn excludes_content_field_from_value_scan() {
        // Content fields hold arbitrary user data — don't scan for value patterns
        let value = json!({"content": "sk-abc123def456ghi789jkl012mno345pqr678stu901vwx"});
        let result = scan_value_for_raw_secrets(&value, "");
        // Field-name detection wouldn't fire either since "content" isn't a secret field name
        assert!(!result.has_findings());
    }

    #[test]
    fn effect_scan_includes_content_fields() {
        let value = json!({
            "items": [{"content": "sk-abc123def456ghi789jkl012mno345pqr678stu901vwx"}]
        });
        let result = scan_effect_value_for_raw_secrets(&value, "");
        assert!(result.has_findings());
        let (redacted, _) = redact_effect_value(&value);
        assert_eq!(redacted["items"][0]["content"], "<secret:redacted>");
    }

    #[test]
    fn detects_nested_secret() {
        let value = json!({
            "operations": [
                {"op": "asset.put", "payload": {"api_key": "sk-real-key-abc123def456ghi789"}}
            ]
        });
        let result = scan_value_for_raw_secrets(&value, "");
        assert!(result.has_findings());
    }

    #[test]
    fn redaction_replaces_value() {
        let value = json!({"api_key": "sk-abc123", "title": "My Package"});
        let (redacted, scan) = redact_secrets_in_value(&value);
        assert!(scan.has_findings());
        assert_eq!(redacted["api_key"], "<secret:redacted>");
        assert_eq!(redacted["title"], "My Package");
    }

    #[test]
    fn no_false_positive_on_normal_text() {
        let value = json!({
            "title": "My Package",
            "description": "A useful package",
            "version": "0.1.0",
            "reason": "needed for work",
            "expected_effects": {"summary": "writes an asset"}
        });
        let result = scan_value_for_raw_secrets(&value, "");
        assert!(!result.has_findings());
    }
}
