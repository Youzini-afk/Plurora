use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{ArtifactDescriptor, EventSequence, ProtocolProfilePin, SessionId};

pub const WORLD_BUNDLE_ARCHIVE_FORMAT: &str = "plurora.world-bundle.archive.v1";
pub const WORLD_BUNDLE_TYPE_URI: &str = "urn:plurora:world-bundle:v1";
pub const WORLD_BUNDLE_MEDIA_TYPE: &str = "application/vnd.plurora.world-bundle+json;version=1";
pub const WORLD_HEAD_TYPE_URI: &str = "urn:plurora:world-head:v1";
pub const WORLD_HEAD_MEDIA_TYPE: &str = "application/vnd.plurora.world-head+json;version=1";
pub const WORLD_JOURNAL_INDEX_TYPE_URI: &str = "urn:plurora:world-journal-index:v1";
pub const WORLD_JOURNAL_INDEX_MEDIA_TYPE: &str =
    "application/vnd.plurora.world-journal-index+json;version=1";
pub const WORLD_PROVENANCE_TYPE_URI: &str = "urn:plurora:world-provenance:v1";
pub const WORLD_PROVENANCE_MEDIA_TYPE: &str =
    "application/vnd.plurora.world-provenance+json;version=1";
pub const WORLD_POLICY_INDEX_TYPE_URI: &str = "urn:plurora:world-policy-index:v1";
pub const WORLD_POLICY_INDEX_MEDIA_TYPE: &str =
    "application/vnd.plurora.world-policy-index+json;version=1";
pub const WORLD_EVENT_ENVELOPE_TYPE_URI: &str = "urn:plurora:event-envelope:v1";
pub const WORLD_EVENT_ENVELOPE_MEDIA_TYPE: &str =
    "application/vnd.plurora.event-envelope+json;version=1";
pub const WORLD_COMPOSITION_LOCK_TYPE_URI: &str = "urn:plurora:composition-lock:v1";
pub const WORLD_COMPOSITION_LOCK_MEDIA_TYPE: &str =
    "application/vnd.plurora.composition-lock+json;version=1";
pub const WORLD_BUNDLE_PROTOCOL_ID: &str = "plurora.world.bundle";
pub const WORLD_BUNDLE_PROTOCOL_VERSION: &str = "1.0.0";
pub const WORLD_BUNDLE_EXPERIMENTAL_PROFILE: &str = "plurora.world.bundle/experimental/v1";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorldJournalRange {
    pub session_id: SessionId,
    pub first_sequence: EventSequence,
    pub last_sequence: EventSequence,
    pub envelope_refs: Vec<ArtifactDescriptor>,
}

impl WorldJournalRange {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.session_id.trim().is_empty(),
            "journal session id is empty"
        );
        anyhow::ensure!(
            self.first_sequence <= self.last_sequence,
            "journal range start exceeds end"
        );
        let expected = self
            .last_sequence
            .checked_sub(self.first_sequence)
            .and_then(|span| span.checked_add(1))
            .context("journal range length overflow")?;
        anyhow::ensure!(
            self.envelope_refs.len() as u64 == expected,
            "journal range envelope count does not match its sequence range"
        );
        for descriptor in &self.envelope_refs {
            anyhow::ensure!(
                descriptor.artifact_type_uri == WORLD_EVENT_ENVELOPE_TYPE_URI,
                "journal range contains a non-event artifact"
            );
            anyhow::ensure!(
                descriptor.media_type == WORLD_EVENT_ENVELOPE_MEDIA_TYPE,
                "journal range contains an unsupported event-envelope media type"
            );
            validate_descriptor(descriptor)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorldHead {
    pub schema_version: u16,
    pub head_type_uri: String,
    pub world_id: String,
    pub state_root: ArtifactDescriptor,
    pub history_root: ArtifactDescriptor,
    pub composition_lock: ArtifactDescriptor,
    pub protocol_profiles: Vec<ProtocolProfilePin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_root: Option<ArtifactDescriptor>,
    pub provenance_root: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effect_receipts: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parent_heads: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl WorldHead {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.schema_version == 1,
            "unsupported world head schema version"
        );
        anyhow::ensure!(
            self.head_type_uri == WORLD_HEAD_TYPE_URI,
            "unsupported world head type"
        );
        validate_portable_identity(&self.world_id, "world id")?;
        ensure_artifact_type(
            &self.history_root,
            WORLD_JOURNAL_INDEX_TYPE_URI,
            WORLD_JOURNAL_INDEX_MEDIA_TYPE,
            "world history root",
        )?;
        ensure_artifact_type(
            &self.composition_lock,
            WORLD_COMPOSITION_LOCK_TYPE_URI,
            WORLD_COMPOSITION_LOCK_MEDIA_TYPE,
            "world composition lock",
        )?;
        ensure_artifact_type(
            &self.provenance_root,
            WORLD_PROVENANCE_TYPE_URI,
            WORLD_PROVENANCE_MEDIA_TYPE,
            "world provenance root",
        )?;
        for receipt in &self.effect_receipts {
            anyhow::ensure!(
                receipt.artifact_type_uri == crate::EFFECT_RECEIPT_TYPE_URI,
                "world head contains a non-receipt effect reference"
            );
        }
        for parent in &self.parent_heads {
            ensure_artifact_type(
                parent,
                WORLD_HEAD_TYPE_URI,
                WORLD_HEAD_MEDIA_TYPE,
                "parent world head",
            )?;
        }
        for descriptor in self.referenced_descriptors() {
            validate_descriptor(descriptor)?;
        }
        validate_profile_pins(&self.protocol_profiles)?;
        anyhow::ensure!(
            self.protocol_profiles.iter().any(|profile| {
                profile.protocol_id == WORLD_BUNDLE_PROTOCOL_ID
                    && profile.version == WORLD_BUNDLE_PROTOCOL_VERSION
                    && profile.profile == WORLD_BUNDLE_EXPERIMENTAL_PROFILE
            }),
            "world head does not pin the Experimental World Bundle profile"
        );
        Ok(())
    }

    pub fn referenced_descriptors(&self) -> Vec<&ArtifactDescriptor> {
        let mut descriptors = vec![
            &self.state_root,
            &self.history_root,
            &self.composition_lock,
            &self.provenance_root,
        ];
        descriptors.extend(self.policy_root.iter());
        descriptors.extend(self.effect_receipts.iter());
        descriptors.extend(self.parent_heads.iter());
        descriptors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorldLineageEntry {
    pub head: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parent_heads: Vec<ArtifactDescriptor>,
    pub relation: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effect_receipts: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl WorldLineageEntry {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.relation.trim().is_empty(),
            "lineage relation is empty"
        );
        validate_descriptor(&self.head)?;
        ensure_artifact_type(
            &self.head,
            WORLD_HEAD_TYPE_URI,
            WORLD_HEAD_MEDIA_TYPE,
            "lineage head",
        )?;
        for descriptor in self.parent_heads.iter().chain(self.effect_receipts.iter()) {
            validate_descriptor(descriptor)?;
        }
        for parent in &self.parent_heads {
            ensure_artifact_type(
                parent,
                WORLD_HEAD_TYPE_URI,
                WORLD_HEAD_MEDIA_TYPE,
                "lineage parent head",
            )?;
        }
        for receipt in &self.effect_receipts {
            anyhow::ensure!(
                receipt.artifact_type_uri == crate::EFFECT_RECEIPT_TYPE_URI,
                "lineage contains a non-receipt effect reference"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorldBundleManifest {
    pub schema_version: u16,
    pub bundle_type_uri: String,
    pub protocol_id: String,
    pub protocol_version: String,
    pub protocol_profile: String,
    pub world_id: String,
    pub world_head: ArtifactDescriptor,
    pub journal_ranges: Vec<WorldJournalRange>,
    pub object_descriptors: Vec<ArtifactDescriptor>,
    pub composition_lock: ArtifactDescriptor,
    pub protocol_profiles: Vec<ProtocolProfilePin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_refs: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effect_receipts: Vec<ArtifactDescriptor>,
    pub lineage: Vec<WorldLineageEntry>,
    pub original_v1_envelopes: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl WorldBundleManifest {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.schema_version == 1,
            "unsupported world bundle schema version"
        );
        anyhow::ensure!(
            self.bundle_type_uri == WORLD_BUNDLE_TYPE_URI,
            "unsupported world bundle type"
        );
        anyhow::ensure!(
            self.protocol_id == WORLD_BUNDLE_PROTOCOL_ID
                && self.protocol_version == WORLD_BUNDLE_PROTOCOL_VERSION
                && self.protocol_profile == WORLD_BUNDLE_EXPERIMENTAL_PROFILE,
            "unsupported World Bundle protocol/profile"
        );
        validate_portable_identity(&self.world_id, "world id")?;
        anyhow::ensure!(
            !self.journal_ranges.is_empty(),
            "world bundle has no journal ranges"
        );
        anyhow::ensure!(!self.lineage.is_empty(), "world bundle has no lineage");
        validate_descriptor(&self.world_head)?;
        validate_descriptor(&self.composition_lock)?;
        ensure_artifact_type(
            &self.world_head,
            WORLD_HEAD_TYPE_URI,
            WORLD_HEAD_MEDIA_TYPE,
            "bundle world head",
        )?;
        ensure_artifact_type(
            &self.composition_lock,
            WORLD_COMPOSITION_LOCK_TYPE_URI,
            WORLD_COMPOSITION_LOCK_MEDIA_TYPE,
            "bundle composition lock",
        )?;
        validate_profile_pins(&self.protocol_profiles)?;
        for range in &self.journal_ranges {
            range.validate()?;
        }
        let mut prior_heads = BTreeSet::new();
        for entry in &self.lineage {
            entry.validate()?;
            for parent in &entry.parent_heads {
                anyhow::ensure!(
                    prior_heads.contains(parent.digest.as_str()),
                    "world lineage parent '{}' does not precede its child",
                    parent.digest
                );
            }
            anyhow::ensure!(
                prior_heads.insert(entry.head.digest.as_str()),
                "duplicate head in world lineage"
            );
        }
        anyhow::ensure!(
            self.lineage
                .last()
                .is_some_and(|entry| entry.head == self.world_head),
            "world_head is not the final lineage entry"
        );
        let mut descriptors = BTreeSet::new();
        for descriptor in &self.object_descriptors {
            validate_descriptor(descriptor)?;
            anyhow::ensure!(
                descriptors.insert(descriptor.digest.as_str()),
                "duplicate world bundle object descriptor '{}'",
                descriptor.digest
            );
        }
        for descriptor in self.required_root_descriptors() {
            anyhow::ensure!(
                descriptors.contains(descriptor.digest.as_str()),
                "world bundle root '{}' is missing from object_descriptors",
                descriptor.digest
            );
        }
        let flattened = self
            .journal_ranges
            .iter()
            .flat_map(|range| range.envelope_refs.iter().map(|item| item.digest.as_str()))
            .collect::<Vec<_>>();
        let originals = self
            .original_v1_envelopes
            .iter()
            .map(|item| item.digest.as_str())
            .collect::<Vec<_>>();
        anyhow::ensure!(
            flattened == originals,
            "original_v1_envelopes does not exactly preserve journal range order"
        );
        Ok(())
    }

    pub fn canonical_bytes(&self) -> anyhow::Result<Vec<u8>> {
        canonical_json_bytes(self)
    }

    pub fn descriptor(&self) -> anyhow::Result<ArtifactDescriptor> {
        self.validate()?;
        let bytes = self.canonical_bytes()?;
        Ok(ArtifactDescriptor {
            artifact_type_uri: WORLD_BUNDLE_TYPE_URI.to_string(),
            media_type: WORLD_BUNDLE_MEDIA_TYPE.to_string(),
            digest: sha256_digest(&bytes),
            size_bytes: bytes.len() as u64,
            references: self
                .object_descriptors
                .iter()
                .map(|descriptor| descriptor.digest.clone())
                .collect(),
            annotations: BTreeMap::from([
                ("world_id".to_string(), Value::String(self.world_id.clone())),
                (
                    "protocol_profile".to_string(),
                    Value::String(self.protocol_profile.clone()),
                ),
            ]),
        })
    }

    pub fn required_root_descriptors(&self) -> Vec<&ArtifactDescriptor> {
        let mut descriptors = vec![&self.world_head, &self.composition_lock];
        descriptors.extend(
            self.journal_ranges
                .iter()
                .flat_map(|range| range.envelope_refs.iter()),
        );
        descriptors.extend(self.policy_refs.iter());
        descriptors.extend(self.effect_receipts.iter());
        descriptors.extend(self.original_v1_envelopes.iter());
        for entry in &self.lineage {
            descriptors.push(&entry.head);
            descriptors.extend(entry.parent_heads.iter());
            descriptors.extend(entry.effect_receipts.iter());
        }
        descriptors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorldBundleObject {
    pub descriptor: ArtifactDescriptor,
    pub data_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorldBundleArchive {
    pub archive_format: String,
    pub bundle_descriptor: ArtifactDescriptor,
    pub manifest: WorldBundleManifest,
    pub objects: Vec<WorldBundleObject>,
}

impl WorldBundleArchive {
    pub fn validate_shape(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.archive_format == WORLD_BUNDLE_ARCHIVE_FORMAT,
            "unsupported World Bundle archive format"
        );
        self.manifest.validate()?;
        anyhow::ensure!(
            self.bundle_descriptor == self.manifest.descriptor()?,
            "World Bundle descriptor does not match its manifest"
        );
        let inventory = self
            .manifest
            .object_descriptors
            .iter()
            .map(|descriptor| (descriptor.digest.as_str(), descriptor))
            .collect::<BTreeMap<_, _>>();
        let mut objects = BTreeSet::new();
        for object in &self.objects {
            validate_descriptor(&object.descriptor)?;
            anyhow::ensure!(
                objects.insert(object.descriptor.digest.as_str()),
                "duplicate inline object '{}'",
                object.descriptor.digest
            );
            anyhow::ensure!(
                inventory.get(object.descriptor.digest.as_str()).copied()
                    == Some(&object.descriptor),
                "inline object descriptor '{}' differs from the manifest inventory",
                object.descriptor.digest
            );
        }
        anyhow::ensure!(
            objects.len() == inventory.len()
                && inventory.keys().all(|digest| objects.contains(digest)),
            "inline object set does not exactly match object_descriptors"
        );
        Ok(())
    }
}

pub fn canonical_json_bytes<T: Serialize>(value: &T) -> anyhow::Result<Vec<u8>> {
    let mut value = serde_json::to_value(value)?;
    sort_json_value(&mut value);
    Ok(serde_json::to_vec(&value)?)
}

pub fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub fn validate_sha256(digest: &str) -> anyhow::Result<()> {
    let Some(value) = digest.strip_prefix("sha256:") else {
        anyhow::bail!("digest '{digest}' is not SHA-256");
    };
    anyhow::ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "digest '{digest}' must contain exactly 64 lowercase hexadecimal characters"
    );
    Ok(())
}

fn validate_descriptor(descriptor: &ArtifactDescriptor) -> anyhow::Result<()> {
    anyhow::ensure!(
        !descriptor.artifact_type_uri.trim().is_empty(),
        "artifact type URI is empty"
    );
    anyhow::ensure!(
        !descriptor.media_type.trim().is_empty(),
        "artifact media type is empty"
    );
    validate_sha256(&descriptor.digest)?;
    let mut references = BTreeSet::new();
    for reference in &descriptor.references {
        validate_sha256(reference).with_context(|| {
            format!(
                "portable artifact '{}' contains a non-content-addressed reference",
                descriptor.digest
            )
        })?;
        anyhow::ensure!(
            references.insert(reference.as_str()),
            "artifact '{}' repeats reference '{reference}'",
            descriptor.digest
        );
        anyhow::ensure!(
            reference != &descriptor.digest,
            "artifact '{}' references itself",
            descriptor.digest
        );
    }
    Ok(())
}

fn ensure_artifact_type(
    descriptor: &ArtifactDescriptor,
    artifact_type_uri: &str,
    media_type: &str,
    label: &str,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        descriptor.artifact_type_uri == artifact_type_uri && descriptor.media_type == media_type,
        "{label} has an unsupported artifact type or media type"
    );
    Ok(())
}

fn validate_profile_pins(profiles: &[ProtocolProfilePin]) -> anyhow::Result<()> {
    let mut seen = BTreeSet::new();
    for profile in profiles {
        anyhow::ensure!(
            !profile.protocol_id.trim().is_empty()
                && !profile.version.trim().is_empty()
                && !profile.profile.trim().is_empty(),
            "protocol profile pin fields must not be empty"
        );
        anyhow::ensure!(
            seen.insert((
                profile.protocol_id.as_str(),
                profile.version.as_str(),
                profile.profile.as_str(),
            )),
            "duplicate protocol profile pin"
        );
    }
    Ok(())
}

fn validate_portable_identity(value: &str, label: &str) -> anyhow::Result<()> {
    let trimmed = value.trim();
    anyhow::ensure!(!trimmed.is_empty(), "{label} is empty");
    let bytes = trimmed.as_bytes();
    let drive_absolute = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\');
    anyhow::ensure!(
        !drive_absolute
            && !trimmed.starts_with('/')
            && !trimmed.starts_with("\\\\")
            && !trimmed.starts_with("file:")
            && !trimmed.contains("://"),
        "{label} must not be a host path or URL"
    );
    Ok(())
}

fn sort_json_value(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                sort_json_value(value);
            }
        }
        Value::Object(object) => {
            let mut sorted = BTreeMap::new();
            for (key, mut value) in std::mem::take(object) {
                sort_json_value(&mut value);
                sorted.insert(key, value);
            }
            object.extend(sorted);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(kind: &str, bytes: &[u8], references: Vec<String>) -> ArtifactDescriptor {
        let media_type = match kind {
            WORLD_EVENT_ENVELOPE_TYPE_URI => WORLD_EVENT_ENVELOPE_MEDIA_TYPE,
            WORLD_JOURNAL_INDEX_TYPE_URI => WORLD_JOURNAL_INDEX_MEDIA_TYPE,
            WORLD_COMPOSITION_LOCK_TYPE_URI => WORLD_COMPOSITION_LOCK_MEDIA_TYPE,
            WORLD_PROVENANCE_TYPE_URI => WORLD_PROVENANCE_MEDIA_TYPE,
            WORLD_HEAD_TYPE_URI => WORLD_HEAD_MEDIA_TYPE,
            _ => "application/json",
        };
        ArtifactDescriptor {
            artifact_type_uri: kind.to_string(),
            media_type: media_type.to_string(),
            digest: sha256_digest(bytes),
            size_bytes: bytes.len() as u64,
            references,
            annotations: BTreeMap::new(),
        }
    }

    #[test]
    fn canonical_json_is_key_order_independent() -> anyhow::Result<()> {
        let left: Value = serde_json::from_str(r#"{"b":2,"a":{"z":1,"y":0}}"#)?;
        let right: Value = serde_json::from_str(r#"{"a":{"y":0,"z":1},"b":2}"#)?;
        assert_eq!(canonical_json_bytes(&left)?, canonical_json_bytes(&right)?);
        Ok(())
    }

    #[test]
    fn portable_references_must_be_sha256() {
        let descriptor = ArtifactDescriptor {
            artifact_type_uri: "urn:test:artifact".to_string(),
            media_type: "application/octet-stream".to_string(),
            digest: sha256_digest(b"content"),
            size_bytes: 7,
            references: vec!["C:\\host-a\\state.bin".to_string()],
            annotations: BTreeMap::new(),
        };
        let error = validate_descriptor(&descriptor).expect_err("absolute path must fail");
        assert!(error
            .to_string()
            .contains("non-content-addressed reference"));
    }

    #[test]
    fn bundle_descriptor_covers_manifest_and_inventory() -> anyhow::Result<()> {
        let event = descriptor(WORLD_EVENT_ENVELOPE_TYPE_URI, b"event", Vec::new());
        let state = descriptor("urn:test:state", b"state", Vec::new());
        let history = descriptor(
            WORLD_JOURNAL_INDEX_TYPE_URI,
            b"history",
            vec![event.digest.clone()],
        );
        let lock = descriptor(WORLD_COMPOSITION_LOCK_TYPE_URI, b"lock", Vec::new());
        let provenance = descriptor(WORLD_PROVENANCE_TYPE_URI, b"provenance", Vec::new());
        let head = descriptor(
            WORLD_HEAD_TYPE_URI,
            b"head",
            vec![
                state.digest.clone(),
                history.digest.clone(),
                lock.digest.clone(),
                provenance.digest.clone(),
            ],
        );
        let profile = ProtocolProfilePin {
            protocol_id: WORLD_BUNDLE_PROTOCOL_ID.to_string(),
            version: WORLD_BUNDLE_PROTOCOL_VERSION.to_string(),
            profile: WORLD_BUNDLE_EXPERIMENTAL_PROFILE.to_string(),
        };
        let manifest = WorldBundleManifest {
            schema_version: 1,
            bundle_type_uri: WORLD_BUNDLE_TYPE_URI.to_string(),
            protocol_id: WORLD_BUNDLE_PROTOCOL_ID.to_string(),
            protocol_version: WORLD_BUNDLE_PROTOCOL_VERSION.to_string(),
            protocol_profile: WORLD_BUNDLE_EXPERIMENTAL_PROFILE.to_string(),
            world_id: "example/world".to_string(),
            world_head: head.clone(),
            journal_ranges: vec![WorldJournalRange {
                session_id: "session-a".to_string(),
                first_sequence: 0,
                last_sequence: 0,
                envelope_refs: vec![event.clone()],
            }],
            object_descriptors: vec![
                event.clone(),
                head.clone(),
                history,
                lock.clone(),
                provenance,
                state,
            ],
            composition_lock: lock,
            protocol_profiles: vec![profile],
            policy_refs: Vec::new(),
            effect_receipts: Vec::new(),
            lineage: vec![WorldLineageEntry {
                head,
                parent_heads: Vec::new(),
                relation: "exported".to_string(),
                effect_receipts: Vec::new(),
                annotations: BTreeMap::new(),
            }],
            original_v1_envelopes: vec![event],
            annotations: BTreeMap::new(),
        };
        let descriptor = manifest.descriptor()?;
        assert_eq!(descriptor.references.len(), 6);
        assert_eq!(
            descriptor.digest,
            sha256_digest(&manifest.canonical_bytes()?)
        );
        Ok(())
    }
}
