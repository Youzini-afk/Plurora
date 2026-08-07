use std::collections::BTreeMap;

use bytes::Bytes;
use chrono::Utc;
use plurora_core::{
    new_id, ArtifactDescriptor, AssetRecord, EventEnvelope, PackageId, EVENT_ASSET_PUT,
    KERNEL_PACKAGE_ID,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{ArtifactCommitRequest, Runtime, StoredAsset, GENERIC_BLOB_ARTIFACT_TYPE_URI};
use crate::{redaction, sha256_digest, EventStore};

// ---------------------------------------------------------------------------
// Legacy content-address helper (FNV-1a 64-bit, deterministic across runs)
// ---------------------------------------------------------------------------
//
// DefaultHasher is explicitly NOT used because its output is not guaranteed
// stable across Rust versions or platforms. FNV-1a is a simple, well-known,
// deterministic hash retained only for importing and addressing v1 records.
// Canonical object identity uses SHA-256.

const FNV1A_64_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A_64_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Compute a deterministic FNV-1a 64-bit hash of the input bytes.
fn fnv1a_64(data: &[u8]) -> u64 {
    let mut hash = FNV1A_64_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV1A_64_PRIME);
    }
    hash
}

/// Compute the legacy v1 FNV-1a address for arbitrary string content.
pub fn legacy_content_address(content: &str) -> String {
    let hash = fnv1a_64(content.as_bytes());
    format!("fnv1a64:{:016x}", hash)
}

/// Compute the canonical SHA-256 content address for arbitrary string content.
pub fn content_address(content: &str) -> String {
    sha256_digest(content.as_bytes())
}

/// Build standard Beta 2 metadata convention fields for an asset record.
///
/// Callers should merge this into their `AssetPutRequest.metadata`.
/// No raw secrets are included; secret fields use `secret_ref:` references.
pub fn standard_asset_metadata(origin_package_id: &str, disclosure: &str) -> Value {
    json!({
        "content_address_scheme": "sha256",
        "provenance": {
            "origin_package_id": origin_package_id,
        },
        "disclosure": disclosure,
        "source_refs": [],
        "derived_refs": [],
        "large_output_policy": "object_ref",
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssetPutRequest {
    #[serde(default)]
    pub origin_package_id: Option<PackageId>,
    pub mime: String,
    pub content: String,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssetGetResponse {
    pub record: AssetRecord,
    pub content: String,
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub async fn put_asset(&self, mut request: AssetPutRequest) -> anyhow::Result<AssetRecord> {
        // Scan asset metadata for raw secrets (content is arbitrary user data — excluded)
        let metadata_scan = redaction::scan_value_for_raw_secrets(&request.metadata, "metadata");
        if metadata_scan.has_findings() {
            let findings: Vec<String> = metadata_scan
                .findings
                .iter()
                .map(|f| format!("{} ({:?})", f.path, f.detection))
                .collect();
            anyhow::bail!(
                "asset metadata contains raw secret(s) in field(s): {}; use secret_ref references instead",
                findings.join(", ")
            );
        }

        let origin_package_id = request
            .origin_package_id
            .take()
            .unwrap_or_else(|| KERNEL_PACKAGE_ID.to_string());
        let asset_id = new_id("ast");
        let references = artifact_references(&request.metadata);
        let annotations =
            asset_annotations(&asset_id, &origin_package_id, request.metadata.clone());
        let descriptor = self
            .commit_artifact(ArtifactCommitRequest {
                artifact_type_uri: GENERIC_BLOB_ARTIFACT_TYPE_URI.to_string(),
                media_type: request.mime.clone(),
                bytes: Bytes::from(request.content.into_bytes()),
                references,
                annotations,
            })
            .await?;
        let record = AssetRecord {
            id: asset_id,
            origin_package_id,
            mime: request.mime,
            hash: descriptor.digest.clone(),
            size_bytes: descriptor.size_bytes,
            created_at: Utc::now(),
            metadata: request.metadata,
            descriptor: Some(descriptor.clone()),
        };
        let mut assets = self.assets.write().await;
        self.append_kernel_event_with_metadata(
            &format!("kernel_asset_{}", record.id),
            EVENT_ASSET_PUT,
            serde_json::to_value(&record)?,
            json!({
                "artifact_digest": descriptor.digest,
                "size_bytes": descriptor.size_bytes,
                "content_included": false,
            }),
        )
        .await?;
        assets.insert(
            record.id.clone(),
            StoredAsset {
                record: record.clone(),
            },
        );
        Ok(record)
    }

    pub async fn get_asset(&self, asset_id: &str) -> anyhow::Result<AssetGetResponse> {
        let stored = self
            .assets
            .read()
            .await
            .get(asset_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("asset '{asset_id}' not found"))?;
        let descriptor = stored
            .record
            .descriptor
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("asset '{asset_id}' has no artifact descriptor"))?;
        let bytes = self.read_artifact(descriptor).await?;
        let content = String::from_utf8(bytes.to_vec())
            .map_err(|_| anyhow::anyhow!("asset '{asset_id}' is not valid UTF-8"))?;
        Ok(AssetGetResponse {
            record: stored.record,
            content,
        })
    }

    pub async fn list_assets(&self) -> Vec<AssetRecord> {
        let mut assets: Vec<_> = self
            .assets
            .read()
            .await
            .values()
            .map(|stored| stored.record.clone())
            .collect();
        assets.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        assets
    }

    pub(super) async fn hydrate_asset_event(
        &self,
        event: &EventEnvelope,
    ) -> anyhow::Result<AssetRecord> {
        let mut record: AssetRecord = serde_json::from_value(event.payload.clone())?;
        if let Some(content) = event.metadata.get("content").and_then(Value::as_str) {
            let legacy_hash = record.hash.clone();
            let mut annotations = asset_annotations(
                &record.id,
                &record.origin_package_id,
                record.metadata.clone(),
            );
            annotations.insert("legacy_asset_id".to_string(), json!(record.id));
            annotations.insert("legacy_hash".to_string(), json!(legacy_hash));
            annotations.insert("legacy_event_id".to_string(), json!(event.id));
            annotations.insert("legacy_event_sequence".to_string(), json!(event.sequence));
            annotations.insert(
                "legacy_event_session_id".to_string(),
                json!(event.session_id),
            );
            let mut references = artifact_references(&record.metadata);
            references.push(format!("urn:plurora:event:{}", event.id));
            let descriptor = self
                .commit_artifact(ArtifactCommitRequest {
                    artifact_type_uri: GENERIC_BLOB_ARTIFACT_TYPE_URI.to_string(),
                    media_type: record.mime.clone(),
                    bytes: Bytes::copy_from_slice(content.as_bytes()),
                    references,
                    annotations,
                })
                .await?;
            record.hash = descriptor.digest.clone();
            record.size_bytes = descriptor.size_bytes;
            record.descriptor = Some(descriptor);
            return Ok(record);
        }

        let descriptor = match record.descriptor.clone() {
            Some(descriptor) => descriptor,
            None if record.hash.starts_with("sha256:") => ArtifactDescriptor {
                artifact_type_uri: GENERIC_BLOB_ARTIFACT_TYPE_URI.to_string(),
                media_type: record.mime.clone(),
                digest: record.hash.clone(),
                size_bytes: record.size_bytes,
                references: artifact_references(&record.metadata),
                annotations: asset_annotations(
                    &record.id,
                    &record.origin_package_id,
                    record.metadata.clone(),
                ),
            },
            None => anyhow::bail!(
                "asset '{}' uses legacy digest '{}' but its event has no inline content to migrate",
                record.id,
                record.hash
            ),
        };
        if descriptor.digest != record.hash {
            anyhow::bail!(
                "asset '{}' digest '{}' does not match descriptor digest '{}'",
                record.id,
                record.hash,
                descriptor.digest
            );
        }
        if descriptor.size_bytes != record.size_bytes {
            anyhow::bail!(
                "asset '{}' size {} does not match descriptor size {}",
                record.id,
                record.size_bytes,
                descriptor.size_bytes
            );
        }
        if descriptor.media_type != record.mime {
            anyhow::bail!(
                "asset '{}' media type '{}' does not match descriptor media type '{}'",
                record.id,
                record.mime,
                descriptor.media_type
            );
        }
        self.verify_artifact(&descriptor).await?;
        record.descriptor = Some(descriptor);
        Ok(record)
    }
}

fn artifact_references(metadata: &Value) -> Vec<String> {
    ["source_refs", "derived_refs"]
        .into_iter()
        .filter_map(|field| metadata.get(field).and_then(Value::as_array))
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn asset_annotations(
    asset_id: &str,
    origin_package_id: &str,
    metadata: Value,
) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("asset_id".to_string(), json!(asset_id)),
        ("origin_package_id".to_string(), json!(origin_package_id)),
        ("asset_metadata".to_string(), metadata),
    ])
}
