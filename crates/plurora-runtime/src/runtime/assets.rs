use std::collections::BTreeMap;

use bytes::Bytes;
use chrono::Utc;
use plurora_core::{
    canonical_json_bytes, new_id, ArtifactDescriptor, AssetRecord, EventEnvelope, PackageId,
    EVENT_ASSET_PUT, PLATFORM_RUNTIME_ID,
};
use plurora_work::{InstallationId, WorkId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{ArtifactCommitRequest, Runtime, StoredAsset, GENERIC_BLOB_ARTIFACT_TYPE_URI};
use crate::{
    redaction, sha256_digest, EventStore, InstallationStateAuthorityEvidence,
    InstallationStateDecision, InstallationStateDecisionAction, InstallationStateDecisionReceipt,
    INSTALLATION_STATE_AUTHORITY_EVIDENCE_MEDIA_TYPE, INSTALLATION_STATE_AUTHORITY_EVIDENCE_SCHEMA,
    INSTALLATION_STATE_AUTHORITY_EVIDENCE_TYPE_URI, INSTALLATION_STATE_OPERATION,
    INSTALLATION_STATE_RECEIPT_MEDIA_TYPE, INSTALLATION_STATE_REPLACEMENT_DECISION_RECEIPT_SCHEMA,
    INSTALLATION_STATE_REPLACEMENT_DECISION_RECEIPT_TYPE_URI,
    INSTALLATION_STATE_RESET_RECEIPT_SCHEMA, INSTALLATION_STATE_RESET_RECEIPT_TYPE_URI,
};

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

/// Encoding used by the public `object.put` and `object.get` content field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetContentEncoding {
    Utf8,
    Hex,
}

/// Explicit request contract for transporting an exact portable artifact.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExactArtifactUpload {
    pub descriptor: ArtifactDescriptor,
    pub content_encoding: AssetContentEncoding,
    pub scope: ObjectPutScope,
}

/// Build an exact-artifact upload declaration for arbitrary bytes encoded as hex.
pub fn exact_artifact_upload(
    descriptor: &ArtifactDescriptor,
    scope: ObjectPutScope,
) -> ExactArtifactUpload {
    ExactArtifactUpload {
        descriptor: descriptor.clone(),
        content_encoding: AssetContentEncoding::Hex,
        scope,
    }
}

/// Installation mutation which may consume an exact portable artifact upload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObjectPutScope {
    InstallationCreate {
        work_id: WorkId,
    },
    InstallationUpdate {
        installation_id: InstallationId,
        work_id: WorkId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssetPutRequest {
    #[serde(default)]
    pub origin_package_id: Option<PackageId>,
    pub mime: String,
    pub content: String,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<ExactArtifactUpload>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObjectPutResponse {
    #[serde(deserialize_with = "deserialize_object_put_asset")]
    #[schemars(required, schema_with = "object_put_asset_schema")]
    pub asset: Option<AssetRecord>,
    pub descriptor: ArtifactDescriptor,
}

fn object_put_asset_schema(
    generator: &mut schemars::r#gen::SchemaGenerator,
) -> schemars::schema::Schema {
    <Option<AssetRecord> as JsonSchema>::json_schema(generator)
}

fn deserialize_object_put_asset<'de, D>(deserializer: D) -> Result<Option<AssetRecord>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<AssetRecord>::deserialize(deserializer)
}

/// Original ordinary-Asset selector for `object.get`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssetGetParams {
    pub asset_id: String,
}

/// Audit-only selector for a journal-issued Installation state artifact.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstallationStateArtifactGetParams {
    pub installation_state_artifact: ArtifactDescriptor,
}

/// The two selectors deliberately have disjoint required fields. Branch-level
/// `deny_unknown_fields` keeps the untagged wire contract unambiguous and does
/// not turn the previously shipped `{ "asset_id": ... }` shape into an alias.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObjectGetRequest {
    Asset(AssetGetParams),
    InstallationStateArtifact(InstallationStateArtifactGetParams),
}

impl JsonSchema for ObjectGetRequest {
    fn schema_name() -> String {
        "ObjectGetRequest".to_string()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        schemars::schema::SchemaObject {
            subschemas: Some(Box::new(schemars::schema::SubschemaValidation {
                one_of: Some(vec![
                    generator.subschema_for::<AssetGetParams>(),
                    generator.subschema_for::<InstallationStateArtifactGetParams>(),
                ]),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

/// Original ordinary-Asset result for `object.get`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssetGetResponse {
    pub record: AssetRecord,
    pub content: String,
}

/// Audit-only result for a journal-issued Installation state artifact.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstallationStateArtifactGetResponse {
    pub descriptor: ArtifactDescriptor,
    pub content: String,
    pub content_encoding: AssetContentEncoding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObjectGetResponse {
    Asset(AssetGetResponse),
    InstallationStateArtifact(InstallationStateArtifactGetResponse),
}

impl JsonSchema for ObjectGetResponse {
    fn schema_name() -> String {
        "ObjectGetResponse".to_string()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        schemars::schema::SchemaObject {
            subschemas: Some(Box::new(schemars::schema::SubschemaValidation {
                one_of: Some(vec![
                    generator.subschema_for::<AssetGetResponse>(),
                    generator.subschema_for::<InstallationStateArtifactGetResponse>(),
                ]),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub async fn put_object(
        &self,
        mut request: AssetPutRequest,
    ) -> anyhow::Result<ObjectPutResponse> {
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

        if let Some(upload) = request.artifact.take() {
            let descriptor = self.put_exact_artifact(request, upload).await?;
            return Ok(ObjectPutResponse {
                asset: None,
                descriptor,
            });
        }

        let asset = self.put_ordinary_asset(request).await?;
        let descriptor = asset
            .descriptor
            .clone()
            .ok_or_else(|| anyhow::anyhow!("object.put asset descriptor is unavailable"))?;
        Ok(ObjectPutResponse {
            asset: Some(asset),
            descriptor,
        })
    }

    /// Store an ordinary user-visible Asset record. Exact portable artifacts must
    /// use `put_object` so they cannot acquire an Asset identity or event.
    pub async fn put_asset(&self, request: AssetPutRequest) -> anyhow::Result<AssetRecord> {
        anyhow::ensure!(
            request.artifact.is_none(),
            "exact artifact uploads must use object.put"
        );
        let response = self.put_object(request).await?;
        response
            .asset
            .ok_or_else(|| anyhow::anyhow!("object.put did not create an Asset record"))
    }

    async fn put_ordinary_asset(
        &self,
        mut request: AssetPutRequest,
    ) -> anyhow::Result<AssetRecord> {
        let origin_package_id = request
            .origin_package_id
            .take()
            .unwrap_or_else(|| PLATFORM_RUNTIME_ID.to_string());
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
        self.append_platform_event_with_metadata(
            &format!("platform_asset_{}", record.id),
            EVENT_ASSET_PUT,
            serde_json::to_value(&record)?,
            json!({
                "artifact_digest": descriptor.digest,
                "size_bytes": descriptor.size_bytes,
                "content_included": false,
                "content_encoding": AssetContentEncoding::Utf8,
            }),
        )
        .await?;
        assets.insert(
            record.id.clone(),
            StoredAsset {
                record: record.clone(),
                content_encoding: AssetContentEncoding::Utf8,
            },
        );
        Ok(record)
    }

    async fn put_exact_artifact(
        &self,
        request: AssetPutRequest,
        upload: ExactArtifactUpload,
    ) -> anyhow::Result<ArtifactDescriptor> {
        plurora_work::validate_artifact_descriptor(&upload.descriptor)
            .map_err(|_| anyhow::anyhow!("object.put artifact descriptor is invalid"))?;
        anyhow::ensure!(
            request.mime == upload.descriptor.media_type,
            "object.put artifact media type does not match its descriptor"
        );
        let bytes = decode_content(&request.content, upload.content_encoding)?;
        anyhow::ensure!(
            bytes.len() as u64 == upload.descriptor.size_bytes,
            "object.put artifact size does not match its descriptor"
        );
        anyhow::ensure!(
            sha256_digest(&bytes) == upload.descriptor.digest,
            "object.put artifact digest does not match its descriptor"
        );

        let info = self.config.object_store.put(Bytes::from(bytes)).await?;
        anyhow::ensure!(
            info.digest == upload.descriptor.digest
                && info.size_bytes == upload.descriptor.size_bytes,
            "object.put artifact store verification did not preserve its descriptor"
        );
        Ok(upload.descriptor)
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
        let content = match stored.content_encoding {
            AssetContentEncoding::Hex => encode_hex(&bytes),
            AssetContentEncoding::Utf8 => String::from_utf8(bytes.to_vec())
                .map_err(|_| anyhow::anyhow!("asset '{asset_id}' is not valid UTF-8"))?,
        };
        Ok(AssetGetResponse {
            record: stored.record,
            content,
        })
    }

    pub async fn get_object(&self, request: ObjectGetRequest) -> anyhow::Result<ObjectGetResponse> {
        match request {
            ObjectGetRequest::Asset(AssetGetParams { asset_id }) => self
                .get_asset(&asset_id)
                .await
                .map(ObjectGetResponse::Asset),
            ObjectGetRequest::InstallationStateArtifact(InstallationStateArtifactGetParams {
                installation_state_artifact,
            }) => self
                .get_installation_state_artifact(&installation_state_artifact)
                .await
                .map(ObjectGetResponse::InstallationStateArtifact),
        }
    }

    async fn get_installation_state_artifact(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> anyhow::Result<InstallationStateArtifactGetResponse> {
        validate_state_artifact_descriptor(descriptor)?;
        self.verify_artifact(descriptor)
            .await
            .map_err(|error| anyhow::anyhow!("state artifact verification failed: {error}"))?;
        let bytes = self
            .read_artifact(descriptor)
            .await
            .map_err(|error| anyhow::anyhow!("state artifact read failed: {error}"))?;
        let text = String::from_utf8(bytes.to_vec())
            .map_err(|_| anyhow::anyhow!("state artifact is not valid UTF-8"))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("state artifact is not valid JSON"))?;
        let canonical = canonical_json_bytes(&value)
            .map_err(|_| anyhow::anyhow!("state artifact canonical JSON encoding failed"))?;
        anyhow::ensure!(
            canonical.as_slice() == bytes.as_ref(),
            "state artifact is not canonical JSON"
        );
        let installation_id = validate_state_artifact_payload(descriptor, &value)?;
        self.config
            .installation_control
            .validate_issued_state_artifact(&installation_id, descriptor)
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "state artifact is not issued by the authoritative Installation journal"
                )
            })?;
        Ok(InstallationStateArtifactGetResponse {
            descriptor: descriptor.clone(),
            content: text,
            content_encoding: AssetContentEncoding::Utf8,
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
    ) -> anyhow::Result<StoredAsset> {
        let mut record: AssetRecord = serde_json::from_value(event.payload.clone())?;
        let content_encoding = event
            .metadata
            .get("content_encoding")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| anyhow::anyhow!("asset event content encoding is invalid"))?
            .unwrap_or(AssetContentEncoding::Utf8);
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
            return Ok(StoredAsset {
                record,
                content_encoding: AssetContentEncoding::Utf8,
            });
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
        Ok(StoredAsset {
            record,
            content_encoding,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateArtifactKind {
    DecisionReceipt,
    AuthorityEvidence,
}

fn validate_state_artifact_descriptor(
    descriptor: &ArtifactDescriptor,
) -> anyhow::Result<StateArtifactKind> {
    plurora_work::validate_artifact_descriptor(descriptor)
        .map_err(|_| anyhow::anyhow!("state artifact descriptor is invalid"))?;
    anyhow::ensure!(
        descriptor.media_type == INSTALLATION_STATE_RECEIPT_MEDIA_TYPE,
        "state artifact descriptor media type is not application/json"
    );
    match descriptor.artifact_type_uri.as_str() {
        INSTALLATION_STATE_RESET_RECEIPT_TYPE_URI
        | INSTALLATION_STATE_REPLACEMENT_DECISION_RECEIPT_TYPE_URI => {
            Ok(StateArtifactKind::DecisionReceipt)
        }
        INSTALLATION_STATE_AUTHORITY_EVIDENCE_TYPE_URI => Ok(StateArtifactKind::AuthorityEvidence),
        _ => anyhow::bail!(
            "object.get does not permit installation state artifact type '{}'",
            descriptor.artifact_type_uri
        ),
    }
}

fn validate_state_artifact_payload(
    descriptor: &ArtifactDescriptor,
    value: &Value,
) -> anyhow::Result<InstallationId> {
    let installation_id = match validate_state_artifact_descriptor(descriptor)? {
        StateArtifactKind::AuthorityEvidence => {
            let evidence: InstallationStateAuthorityEvidence =
                serde_json::from_value(value.clone())
                    .map_err(|_| anyhow::anyhow!("authority evidence payload is invalid"))?;
            anyhow::ensure!(
                evidence.schema == INSTALLATION_STATE_AUTHORITY_EVIDENCE_SCHEMA,
                "authority evidence schema does not match its type"
            );
            anyhow::ensure!(
                evidence.action == "installation.manage",
                "authority evidence action is invalid"
            );
            anyhow::ensure!(
                descriptor.references.is_empty(),
                "authority evidence descriptor references do not match its payload"
            );
            evidence.installation_id
        }
        StateArtifactKind::DecisionReceipt => {
            let receipt: InstallationStateDecisionReceipt =
                serde_json::from_value(value.clone())
                    .map_err(|_| anyhow::anyhow!("state decision receipt payload is invalid"))?;
            let expected_schema = match descriptor.artifact_type_uri.as_str() {
                INSTALLATION_STATE_RESET_RECEIPT_TYPE_URI => {
                    INSTALLATION_STATE_RESET_RECEIPT_SCHEMA
                }
                INSTALLATION_STATE_REPLACEMENT_DECISION_RECEIPT_TYPE_URI => {
                    INSTALLATION_STATE_REPLACEMENT_DECISION_RECEIPT_SCHEMA
                }
                _ => unreachable!("descriptor type checked above"),
            };
            anyhow::ensure!(
                receipt.schema == expected_schema,
                "state decision receipt schema does not match its type"
            );
            anyhow::ensure!(
                receipt.operation == INSTALLATION_STATE_OPERATION,
                "state decision receipt operation is invalid"
            );
            plurora_core::validate_sha256(&receipt.candidate_work_digest)
                .map_err(|_| anyhow::anyhow!("state decision receipt work digest is invalid"))?;
            plurora_core::validate_sha256(&receipt.candidate_lock_digest)
                .map_err(|_| anyhow::anyhow!("state decision receipt lock digest is invalid"))?;
            if let Some(snapshot_digest) = &receipt.replacement_snapshot_digest {
                plurora_core::validate_sha256(snapshot_digest).map_err(|_| {
                    anyhow::anyhow!("state decision receipt snapshot digest is invalid")
                })?;
            }
            anyhow::ensure!(
                receipt.decision == InstallationStateDecision::Allow,
                "state decision receipt is not an allowed Host decision"
            );
            anyhow::ensure!(
                !receipt.authority_evidence.is_empty(),
                "state decision receipt has no authority evidence"
            );
            let references: Vec<String> = receipt
                .authority_evidence
                .iter()
                .map(|evidence| {
                    anyhow::ensure!(
                        evidence.artifact_type_uri
                            == INSTALLATION_STATE_AUTHORITY_EVIDENCE_TYPE_URI,
                        "state decision receipt authority evidence type is invalid"
                    );
                    anyhow::ensure!(
                        evidence.media_type == INSTALLATION_STATE_AUTHORITY_EVIDENCE_MEDIA_TYPE,
                        "state decision receipt authority evidence media type is invalid"
                    );
                    plurora_work::validate_artifact_descriptor(evidence).map_err(|_| {
                        anyhow::anyhow!(
                            "state decision receipt authority evidence descriptor is invalid"
                        )
                    })?;
                    anyhow::ensure!(
                        evidence.references.is_empty(),
                        "state decision receipt authority evidence references are invalid"
                    );
                    Ok(evidence.digest.clone())
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            anyhow::ensure!(
                descriptor.references == references,
                "state decision receipt descriptor references do not match its payload"
            );
            anyhow::ensure!(
                receipt.action == InstallationStateDecisionAction::Reset
                    || receipt.action == InstallationStateDecisionAction::Replace,
                "state decision receipt action is invalid"
            );
            match receipt.action {
                InstallationStateDecisionAction::Reset => anyhow::ensure!(
                    receipt.replacement_snapshot_digest.is_none(),
                    "reset state decision receipt unexpectedly references a replacement snapshot"
                ),
                InstallationStateDecisionAction::Replace => anyhow::ensure!(
                    receipt.replacement_snapshot_digest.is_some(),
                    "replace state decision receipt has no replacement snapshot"
                ),
            }
            receipt.installation_id
        }
    };
    Ok(installation_id)
}

fn decode_content(content: &str, encoding: AssetContentEncoding) -> anyhow::Result<Vec<u8>> {
    match encoding {
        AssetContentEncoding::Utf8 => Ok(content.as_bytes().to_vec()),
        AssetContentEncoding::Hex => decode_hex(content),
    }
}

fn decode_hex(content: &str) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(
        content.len().is_multiple_of(2),
        "object.put artifact content is not valid hex"
    );
    content
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0])?;
            let low = hex_nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn hex_nibble(value: u8) -> anyhow::Result<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => anyhow::bail!("object.put artifact content is not valid hex"),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{InMemoryEventStore, InMemoryObjectStore, ObjectStore, RuntimeConfig};
    use plurora_work::InstallationId;

    fn descriptor(bytes: &[u8]) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: "urn:plurora:test-portable-object:v1".to_string(),
            media_type: "application/octet-stream".to_string(),
            digest: sha256_digest(bytes),
            size_bytes: bytes.len() as u64,
            references: vec![format!("sha256:{}", "a".repeat(64))],
            annotations: BTreeMap::new(),
        }
    }

    fn upload_request(descriptor: &ArtifactDescriptor, bytes: &[u8]) -> AssetPutRequest {
        AssetPutRequest {
            origin_package_id: None,
            mime: descriptor.media_type.clone(),
            content: encode_hex(bytes),
            metadata: json!({}),
            artifact: Some(exact_artifact_upload(
                descriptor,
                ObjectPutScope::InstallationCreate {
                    work_id: WorkId::parse("tests/object-put").unwrap(),
                },
            )),
        }
    }

    #[tokio::test]
    async fn object_put_preserves_exact_descriptor_and_is_idempotent() {
        let events = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(InMemoryObjectStore::default());
        let runtime = Runtime::new(
            events.clone(),
            RuntimeConfig {
                object_store: objects.clone(),
                ..RuntimeConfig::default()
            },
        );
        let bytes = b"\0portable\xffartifact";
        let descriptor = descriptor(bytes);

        let first = runtime
            .put_object(upload_request(&descriptor, bytes))
            .await
            .expect("first exact upload");
        let replay = runtime
            .put_object(upload_request(&descriptor, bytes))
            .await
            .expect("idempotent exact upload");
        let restarted = Runtime::new(
            events.clone(),
            RuntimeConfig {
                object_store: objects.clone(),
                ..RuntimeConfig::default()
            },
        );
        restarted
            .hydrate_substrate_from_events()
            .await
            .expect("rehydrate exact upload");
        let replay_after_restart = restarted
            .put_object(upload_request(&descriptor, bytes))
            .await
            .expect("idempotent exact upload after restart");

        assert!(first.asset.is_none());
        assert!(replay.asset.is_none());
        assert!(replay_after_restart.asset.is_none());
        assert_eq!(first.descriptor, descriptor);
        assert_eq!(replay.descriptor, descriptor);
        assert_eq!(replay_after_restart.descriptor, descriptor);
        assert_eq!(
            objects.get(&descriptor.digest).await.unwrap().as_ref(),
            bytes
        );
        assert!(runtime.list_assets().await.is_empty());
        assert!(restarted.list_assets().await.is_empty());
        assert_eq!(
            events
                .list_kind_prefix(EVENT_ASSET_PUT)
                .await
                .unwrap()
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn object_put_rejects_tampered_size_digest_and_media_type() {
        let objects = Arc::new(InMemoryObjectStore::default());
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig {
                object_store: objects.clone(),
                ..RuntimeConfig::default()
            },
        );
        let bytes = b"portable artifact";
        let descriptor = descriptor(bytes);

        let mut wrong_size = descriptor.clone();
        wrong_size.size_bytes += 1;
        assert!(runtime
            .put_object(upload_request(&wrong_size, bytes))
            .await
            .unwrap_err()
            .to_string()
            .contains("size"));

        let mut wrong_digest = descriptor.clone();
        wrong_digest.digest = format!("sha256:{}", "b".repeat(64));
        assert!(runtime
            .put_object(upload_request(&wrong_digest, bytes))
            .await
            .unwrap_err()
            .to_string()
            .contains("digest"));

        let mut wrong_media = upload_request(&descriptor, bytes);
        wrong_media.mime = "application/json".to_string();
        assert!(runtime
            .put_object(wrong_media)
            .await
            .unwrap_err()
            .to_string()
            .contains("media type"));
        assert!(!objects.has(&descriptor.digest).await.unwrap());
    }

    fn state_artifact_descriptor<T: Serialize>(
        artifact_type_uri: &str,
        value: &T,
        references: Vec<String>,
    ) -> (ArtifactDescriptor, Vec<u8>) {
        let bytes = canonical_json_bytes(value).unwrap();
        let descriptor = ArtifactDescriptor {
            artifact_type_uri: artifact_type_uri.to_string(),
            media_type: INSTALLATION_STATE_RECEIPT_MEDIA_TYPE.to_string(),
            digest: sha256_digest(&bytes),
            size_bytes: bytes.len() as u64,
            references,
            annotations: BTreeMap::new(),
        };
        (descriptor, bytes)
    }

    #[tokio::test]
    async fn object_get_preserves_original_asset_wire_contract() {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        let record = runtime
            .put_asset(AssetPutRequest {
                origin_package_id: None,
                mime: "text/plain".to_string(),
                content: "selector".to_string(),
                metadata: json!({}),
                artifact: None,
            })
            .await
            .unwrap();
        let request: ObjectGetRequest =
            serde_json::from_value(json!({"asset_id": record.id})).unwrap();
        let response = runtime.get_object(request).await.unwrap();
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            json!({"record": record, "content": "selector"})
        );

        let direct = runtime.get_asset(&record.id).await.unwrap();
        assert_eq!(direct.record.id, record.id);
        assert_eq!(direct.content, "selector");

        assert!(serde_json::from_value::<ObjectGetRequest>(
            json!({"kind": "asset", "asset_id": record.id})
        )
        .is_err());
        assert!(serde_json::from_value::<ObjectGetRequest>(json!({
            "asset_id": record.id,
            "installation_state_artifact": record.descriptor
        }))
        .is_err());
    }

    #[tokio::test]
    async fn object_get_rejects_structurally_valid_state_artifacts_not_issued_by_host() {
        let objects = Arc::new(InMemoryObjectStore::default());
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig {
                object_store: objects.clone(),
                ..RuntimeConfig::default()
            },
        );
        let installation_id = InstallationId::new();
        let evidence_payload = InstallationStateAuthorityEvidence {
            schema: INSTALLATION_STATE_AUTHORITY_EVIDENCE_SCHEMA.to_string(),
            action: "installation.manage".to_string(),
            installation_id: installation_id.clone(),
            grant_id: Some("grant-test".to_string()),
            expires_at_ms: Some(4_000_000_000),
        };
        let (evidence_descriptor, evidence_bytes) = state_artifact_descriptor(
            INSTALLATION_STATE_AUTHORITY_EVIDENCE_TYPE_URI,
            &evidence_payload,
            Vec::new(),
        );
        objects.put(evidence_bytes.into()).await.unwrap();
        let receipt_payload = InstallationStateDecisionReceipt {
            schema: INSTALLATION_STATE_RESET_RECEIPT_SCHEMA.to_string(),
            installation_id,
            expected_revision: 7,
            candidate_work_digest: format!("sha256:{}", "a".repeat(64)),
            candidate_lock_digest: format!("sha256:{}", "b".repeat(64)),
            replacement_snapshot_digest: None,
            operation: INSTALLATION_STATE_OPERATION.to_string(),
            action: InstallationStateDecisionAction::Reset,
            decision: InstallationStateDecision::Allow,
            authority_evidence: vec![evidence_descriptor.clone()],
        };
        let (receipt_descriptor, receipt_bytes) = state_artifact_descriptor(
            INSTALLATION_STATE_RESET_RECEIPT_TYPE_URI,
            &receipt_payload,
            vec![evidence_descriptor.digest.clone()],
        );
        objects.put(receipt_bytes.into()).await.unwrap();

        for descriptor in [evidence_descriptor, receipt_descriptor] {
            let error = runtime
                .get_object(ObjectGetRequest::InstallationStateArtifact(
                    InstallationStateArtifactGetParams {
                        installation_state_artifact: descriptor,
                    },
                ))
                .await
                .expect_err("CAS presence and a valid payload are not Host issuance");
            assert!(error
                .to_string()
                .contains("authoritative Installation journal"));
        }
    }

    #[tokio::test]
    async fn object_get_rejects_snapshot_generic_and_tampered_state_artifacts() {
        let objects = Arc::new(InMemoryObjectStore::default());
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig {
                object_store: objects.clone(),
                ..RuntimeConfig::default()
            },
        );
        let evidence_payload = InstallationStateAuthorityEvidence {
            schema: INSTALLATION_STATE_AUTHORITY_EVIDENCE_SCHEMA.to_string(),
            action: "installation.manage".to_string(),
            installation_id: InstallationId::new(),
            grant_id: None,
            expires_at_ms: None,
        };
        let (evidence_descriptor, evidence_bytes) = state_artifact_descriptor(
            INSTALLATION_STATE_AUTHORITY_EVIDENCE_TYPE_URI,
            &evidence_payload,
            Vec::new(),
        );
        objects.put(evidence_bytes.into()).await.unwrap();
        let mut snapshot_descriptor = evidence_descriptor.clone();
        snapshot_descriptor.artifact_type_uri =
            crate::INSTALLATION_STATE_SNAPSHOT_TYPE_URI.to_string();
        assert!(runtime
            .get_object(ObjectGetRequest::InstallationStateArtifact(
                InstallationStateArtifactGetParams {
                    installation_state_artifact: snapshot_descriptor,
                },
            ))
            .await
            .is_err());
        let mut generic_descriptor = evidence_descriptor.clone();
        generic_descriptor.artifact_type_uri = GENERIC_BLOB_ARTIFACT_TYPE_URI.to_string();
        assert!(runtime
            .get_object(ObjectGetRequest::InstallationStateArtifact(
                InstallationStateArtifactGetParams {
                    installation_state_artifact: generic_descriptor,
                },
            ))
            .await
            .is_err());
        let mut wrong_size = evidence_descriptor.clone();
        wrong_size.size_bytes += 1;
        assert!(runtime
            .get_object(ObjectGetRequest::InstallationStateArtifact(
                InstallationStateArtifactGetParams {
                    installation_state_artifact: wrong_size,
                },
            ))
            .await
            .is_err());
        let mut wrong_digest = evidence_descriptor;
        wrong_digest.digest = format!("sha256:{}", "f".repeat(64));
        assert!(runtime
            .get_object(ObjectGetRequest::InstallationStateArtifact(
                InstallationStateArtifactGetParams {
                    installation_state_artifact: wrong_digest,
                },
            ))
            .await
            .is_err());
    }
}
