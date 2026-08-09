use std::collections::{BTreeMap, BTreeSet};

use plurora_core::{
    canonical_json_bytes as core_canonical_json_bytes, is_secret_field_name, looks_like_raw_secret,
    validate_sha256, world_bundle_sha256_digest, ArtifactDescriptor, SecretRef,
};
use schemars::schema::{InstanceType, Schema, SchemaObject, SingleOrVec};
use serde::Serialize;
use serde_json::Value;

use crate::diagnostic::{DiagnosticCode, ModelError, ModelResult};

pub const CANONICAL_JSON_MEDIA_TYPE: &str = "application/json";
pub const MAX_CANONICAL_METADATA_BYTES: usize = 4 * 1024 * 1024;

pub(crate) fn fixed_string_schema(value: &str) -> Schema {
    Schema::Object(SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::String))),
        const_value: Some(Value::String(value.to_string())),
        ..SchemaObject::default()
    })
}

pub(crate) fn fixed_u16_schema(value: u16) -> Schema {
    Schema::Object(SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Integer))),
        format: Some("uint16".to_string()),
        const_value: Some(Value::from(value)),
        ..SchemaObject::default()
    })
}

pub fn canonical_json_bytes<T: Serialize>(value: &T) -> ModelResult<Vec<u8>> {
    let bytes = core_canonical_json_bytes(value).map_err(|_| {
        ModelError::new(
            DiagnosticCode::WorkInvalid,
            "model could not be serialized as canonical JSON",
        )
    })?;
    if bytes.len() > MAX_CANONICAL_METADATA_BYTES {
        return Err(ModelError::new(
            DiagnosticCode::ArtifactBudgetExceeded,
            "canonical metadata artifact exceeds the 4 MiB implementation budget",
        ));
    }
    Ok(bytes)
}

pub fn canonical_digest<T: Serialize>(value: &T) -> ModelResult<String> {
    validate_portable_model(value)?;
    validate_canonical_model(value)?;
    Ok(world_bundle_sha256_digest(&canonical_json_bytes(value)?))
}

pub fn validate_artifact_descriptor(descriptor: &ArtifactDescriptor) -> ModelResult<()> {
    if descriptor.artifact_type_uri.trim().is_empty() || descriptor.media_type.trim().is_empty() {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            "artifact type and media type must be present",
        ));
    }
    validate_sha256(&descriptor.digest).map_err(|_| {
        ModelError::new(
            DiagnosticCode::ArtifactDigestMismatch,
            "artifact descriptor does not contain a complete SHA-256 digest",
        )
    })?;
    let mut references = BTreeSet::new();
    for reference in &descriptor.references {
        validate_sha256(reference).map_err(|_| {
            ModelError::new(
                DiagnosticCode::ArtifactDigestMismatch,
                "artifact reference is not a complete SHA-256 digest",
            )
        })?;
        if reference == &descriptor.digest || !references.insert(reference.as_str()) {
            return Err(ModelError::new(
                DiagnosticCode::ArtifactDigestMismatch,
                "artifact descriptor contains a self-reference or duplicate reference",
            ));
        }
    }
    validate_portable_value(&Value::Object(
        descriptor.annotations.clone().into_iter().collect(),
    ))?;
    Ok(())
}

pub fn validate_descriptor_type(
    descriptor: &ArtifactDescriptor,
    expected_type_uri: &str,
) -> ModelResult<()> {
    validate_artifact_descriptor(descriptor)?;
    if descriptor.artifact_type_uri != expected_type_uri {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            "artifact descriptor has an unexpected artifact type",
        ));
    }
    Ok(())
}

pub fn validate_portable_model<T: Serialize>(value: &T) -> ModelResult<()> {
    let value = serde_json::to_value(value).map_err(|_| {
        ModelError::new(
            DiagnosticCode::WorkInvalid,
            "model could not be inspected for portable-value redlines",
        )
    })?;
    validate_portable_value(&value)
}

pub fn validate_portable_value(value: &Value) -> ModelResult<()> {
    validate_portable_value_at(value, None)
}

fn validate_portable_value_at(value: &Value, field_name: Option<&str>) -> ModelResult<()> {
    match value {
        Value::Array(values) => {
            for value in values {
                validate_portable_value_at(value, field_name)?;
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                if looks_like_raw_secret(key) || looks_like_host_path(key, None) {
                    return Err(ModelError::new(
                        if looks_like_host_path(key, None) {
                            DiagnosticCode::RawPath
                        } else {
                            DiagnosticCode::RawSecret
                        },
                        "portable model contains a non-portable object key",
                    ));
                }
                if secret_value_field(key) {
                    match value {
                        Value::Null => {}
                        Value::String(reference) if SecretRef::is_valid_ref(reference) => {}
                        Value::Array(references)
                            if references.iter().all(|value| {
                                value.as_str().is_some_and(SecretRef::is_valid_ref)
                            }) => {}
                        _ => {
                            return Err(ModelError::new(
                                DiagnosticCode::RawSecret,
                                "portable model contains a raw secret-bearing field",
                            ));
                        }
                    }
                }
                validate_portable_value_at(value, Some(key))?;
            }
        }
        Value::String(value) => {
            if looks_like_raw_secret(value) {
                return Err(ModelError::new(
                    DiagnosticCode::RawSecret,
                    "portable model contains a value that looks like a raw secret",
                ));
            }
            if looks_like_host_path(value, field_name) {
                return Err(ModelError::new(
                    DiagnosticCode::RawPath,
                    "portable model contains a host-local filesystem path",
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn validate_canonical_model<T: Serialize>(value: &T) -> ModelResult<()> {
    let value = serde_json::to_value(value).map_err(|_| {
        ModelError::new(
            DiagnosticCode::WorkInvalid,
            "model could not be inspected for canonical metadata redlines",
        )
    })?;
    validate_canonical_value(&value, false)
}

fn validate_canonical_value(value: &Value, open_metadata: bool) -> ModelResult<()> {
    match value {
        Value::Array(values) => {
            for value in values {
                validate_canonical_value(value, open_metadata)?;
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                let child_is_open_metadata = open_metadata
                    || matches!(
                        key.to_ascii_lowercase().as_str(),
                        "annotations" | "extensions" | "properties"
                    );
                if child_is_open_metadata && is_volatile_metadata_key(key) {
                    return Err(ModelError::new(
                        DiagnosticCode::WorkInvalid,
                        "canonical metadata contains host-observed or nondeterministic state",
                    ));
                }
                validate_canonical_value(value, child_is_open_metadata)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_volatile_metadata_key(key: &str) -> bool {
    let normalized = key
        .rsplit(['/', ':'])
        .next()
        .unwrap_or(key)
        .replace('-', "_")
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "generated_at"
            | "build_time"
            | "current_time"
            | "pid"
            | "process_id"
            | "actual_port"
            | "host_port"
            | "temporary_id"
            | "temp_id"
            | "random_id"
    )
}

fn secret_value_field(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    is_secret_field_name(key)
        && !lower.ends_with("_id")
        && !lower.ends_with("_ids")
        && !lower.ends_with("_policy")
        && !lower.ends_with("_requirements")
        && !lower.ends_with("_imports")
        && !lower.ends_with("_scope")
}

fn looks_like_host_path(value: &str, field_name: Option<&str>) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    let bytes = trimmed.as_bytes();
    let drive_absolute = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\');
    let lower = trimmed.to_ascii_lowercase();
    let is_json_pointer = field_name.is_some_and(|field| {
        let field = field.to_ascii_lowercase();
        (field.contains("json_pointer") || field.contains("json-pointer"))
            && trimmed.starts_with('/')
    });
    drive_absolute
        || (trimmed.starts_with('/') && !is_json_pointer)
        || trimmed.starts_with("\\\\")
        || trimmed.starts_with("~/")
        || trimmed.starts_with("~\\")
        || trimmed.eq("..")
        || trimmed.starts_with("../")
        || trimmed.starts_with("..\\")
        || trimmed.contains("/../")
        || trimmed.contains("\\..\\")
        || lower.starts_with("file:")
        || lower.starts_with("unix:")
}

pub trait ArtifactModel: Serialize + Sized {
    const ARTIFACT_TYPE_URI: &'static str;

    fn validate(&self) -> ModelResult<()>;

    fn referenced_artifacts(&self) -> Vec<&ArtifactDescriptor> {
        Vec::new()
    }

    fn descriptor_annotations(&self) -> BTreeMap<String, Value> {
        BTreeMap::new()
    }

    fn canonical_bytes(&self) -> ModelResult<Vec<u8>> {
        self.validate()?;
        validate_portable_model(self)?;
        validate_canonical_model(self)?;
        canonical_json_bytes(self)
    }

    fn digest(&self) -> ModelResult<String> {
        Ok(world_bundle_sha256_digest(&self.canonical_bytes()?))
    }

    fn artifact_descriptor(&self) -> ModelResult<ArtifactDescriptor> {
        let bytes = self.canonical_bytes()?;
        let digest = world_bundle_sha256_digest(&bytes);
        let mut references = BTreeSet::new();
        for descriptor in self.referenced_artifacts() {
            validate_artifact_descriptor(descriptor)?;
            references.insert(descriptor.digest.clone());
        }
        Ok(ArtifactDescriptor {
            artifact_type_uri: Self::ARTIFACT_TYPE_URI.to_string(),
            media_type: CANONICAL_JSON_MEDIA_TYPE.to_string(),
            digest,
            size_bytes: bytes.len() as u64,
            references: references.into_iter().collect(),
            annotations: self.descriptor_annotations(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_bytes_do_not_depend_on_map_order() {
        let left = json!({"b": 2, "a": {"z": 1, "y": 0}});
        let right = json!({"a": {"y": 0, "z": 1}, "b": 2});
        assert_eq!(
            canonical_json_bytes(&left).unwrap(),
            canonical_json_bytes(&right).unwrap()
        );
        assert_eq!(
            canonical_digest(&left).unwrap(),
            canonical_digest(&right).unwrap()
        );
    }

    #[test]
    fn portable_values_reject_raw_secret_and_host_path_without_echoing_them() {
        let secret = ["sk-", "Example1234567890Example1234567890"].concat();
        let error = validate_portable_value(&json!({"token": secret})).unwrap_err();
        assert_eq!(error.code, DiagnosticCode::RawSecret);
        assert!(!error.to_string().contains("Example123"));

        let error =
            validate_portable_value(&json!({"source": "C:\\Users\\creator\\work"})).unwrap_err();
        assert_eq!(error.code, DiagnosticCode::RawPath);
        assert!(!error.to_string().contains("creator"));
    }

    #[test]
    fn portable_values_allow_namespaced_ids_urls_and_secret_references() {
        validate_portable_value(&json!({
            "protocol": "plurora.work/experimental/v1",
            "docs": "https://plurora.dev/spec/work",
            "secret_ref": "secret_ref:store:MODEL_KEY"
        }))
        .unwrap();
    }

    #[test]
    fn portable_values_distinguish_json_pointers_from_host_socket_paths() {
        validate_portable_value(&json!({"json_pointer": "/components/0"})).unwrap();
        assert_eq!(
            validate_portable_value(&json!({"endpoint": "unix:///tmp/service.sock"}))
                .unwrap_err()
                .code,
            DiagnosticCode::RawPath
        );
    }

    #[test]
    fn canonical_metadata_rejects_host_observed_values() {
        assert_eq!(
            validate_canonical_model(&json!({
                "annotations": {"generated_at": "2026-08-09T00:00:00Z"}
            }))
            .unwrap_err()
            .code,
            DiagnosticCode::WorkInvalid
        );

        validate_canonical_model(&json!({
            "annotations": {"thirdparty.example/timestamp": 0}
        }))
        .unwrap();
    }
}
