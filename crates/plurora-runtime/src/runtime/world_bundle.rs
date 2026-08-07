use std::collections::{BTreeMap, BTreeSet, VecDeque};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use bytes::Bytes;
use chrono::Utc;
use plurora_core::{
    canonical_json_bytes, ArtifactDescriptor, CompositionLock, EffectReceipt, EventEnvelope,
    KernelSession, ProtocolProfilePin, SessionStatus, WorldBundleArchive, WorldBundleManifest,
    WorldBundleObject, WorldHead, WorldJournalRange, WorldLineageEntry, EFFECT_RECEIPT_TYPE_URI,
    EVENT_SESSION_CLOSED, EVENT_SESSION_OPENED, WORLD_BUNDLE_ARCHIVE_FORMAT,
    WORLD_BUNDLE_EXPERIMENTAL_PROFILE, WORLD_BUNDLE_PROTOCOL_ID, WORLD_BUNDLE_PROTOCOL_VERSION,
    WORLD_BUNDLE_TYPE_URI, WORLD_COMPOSITION_LOCK_MEDIA_TYPE, WORLD_COMPOSITION_LOCK_TYPE_URI,
    WORLD_EVENT_ENVELOPE_MEDIA_TYPE, WORLD_EVENT_ENVELOPE_TYPE_URI, WORLD_HEAD_MEDIA_TYPE,
    WORLD_HEAD_TYPE_URI, WORLD_JOURNAL_INDEX_MEDIA_TYPE, WORLD_JOURNAL_INDEX_TYPE_URI,
    WORLD_POLICY_INDEX_MEDIA_TYPE, WORLD_POLICY_INDEX_TYPE_URI, WORLD_PROVENANCE_MEDIA_TYPE,
    WORLD_PROVENANCE_TYPE_URI,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{ArtifactCommitRequest, Runtime};
use crate::{sha256_digest, EventStore};

const MAX_WORLD_BUNDLE_OBJECTS: usize = 100_000;
const MAX_WORLD_BUNDLE_TOTAL_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldJournalSelection {
    pub session_id: String,
    pub first_sequence: Option<u64>,
    pub last_sequence: Option<u64>,
}

impl WorldJournalSelection {
    pub fn all(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            first_sequence: None,
            last_sequence: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorldBundleExportRequest {
    pub world_id: String,
    pub state_root: ArtifactDescriptor,
    pub journal_selections: Vec<WorldJournalSelection>,
    pub composition_lock: CompositionLock,
    pub protocol_profiles: Vec<ProtocolProfilePin>,
    pub policy_refs: Vec<ArtifactDescriptor>,
    pub effect_receipts: Vec<ArtifactDescriptor>,
    pub parent_heads: Vec<ArtifactDescriptor>,
    pub prior_lineage: Vec<WorldLineageEntry>,
    pub additional_roots: Vec<ArtifactDescriptor>,
    pub relation: String,
    pub annotations: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorldBundleAuditReport {
    pub bundle_digest: String,
    pub world_id: String,
    pub head_digest: String,
    pub object_count: usize,
    pub journal_range_count: usize,
    pub event_count: usize,
    pub session_count: usize,
    pub effect_receipt_count: usize,
    pub lineage_entry_count: usize,
    pub non_plurora_namespace_artifact_count: usize,
    pub protocol_profile: String,
    pub reference_closure_verified: bool,
    pub original_envelopes_verified: bool,
    pub offline_replay_safe: bool,
    pub shell_independent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WorldBundleReceiptReplay {
    pub receipt_ref: ArtifactDescriptor,
    pub receipt: EffectReceipt,
    pub outputs: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorldBundleReplayResult {
    pub bundle_digest: String,
    pub world_id: String,
    pub head: WorldHead,
    pub events: Vec<EventEnvelope>,
    pub receipts: Vec<WorldBundleReceiptReplay>,
    pub historical_only: bool,
    pub executor_invocations: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorldBundleImportResult {
    pub bundle_digest: String,
    pub world_id: String,
    pub head_digest: String,
    pub objects_imported: usize,
    pub events_imported: usize,
    pub sessions_imported: usize,
}

struct VerifiedWorldBundle {
    objects: BTreeMap<String, (ArtifactDescriptor, Bytes)>,
    events: Vec<EventEnvelope>,
    head: WorldHead,
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub async fn export_world_bundle(
        &self,
        mut request: WorldBundleExportRequest,
    ) -> anyhow::Result<WorldBundleArchive> {
        anyhow::ensure!(
            !request.journal_selections.is_empty(),
            "World Bundle export requires at least one journal selection"
        );
        anyhow::ensure!(
            !request.relation.trim().is_empty(),
            "World Bundle lineage relation is empty"
        );
        normalize_composition_lock(&mut request.composition_lock);
        request.composition_lock.validate()?;
        normalize_profiles(&mut request.protocol_profiles);
        request.journal_selections.sort_by(|left, right| {
            (&left.session_id, left.first_sequence, left.last_sequence).cmp(&(
                &right.session_id,
                right.first_sequence,
                right.last_sequence,
            ))
        });
        let mut selected_sessions = BTreeSet::new();
        for selection in &request.journal_selections {
            anyhow::ensure!(
                selected_sessions.insert(selection.session_id.as_str()),
                "World Bundle export currently accepts one journal range per session"
            );
        }
        ensure_world_bundle_profile(&request.protocol_profiles)?;
        anyhow::ensure!(
            request.composition_lock.protocol_profiles == request.protocol_profiles,
            "composition lock and world head protocol profile pins differ"
        );
        self.verify_artifact(&request.state_root).await?;

        let composition_lock = self
            .commit_json_artifact(
                WORLD_COMPOSITION_LOCK_TYPE_URI,
                WORLD_COMPOSITION_LOCK_MEDIA_TYPE,
                &request.composition_lock,
                request
                    .composition_lock
                    .content_roots
                    .iter()
                    .map(|root| root.digest.clone())
                    .collect(),
                BTreeMap::new(),
            )
            .await?;

        let mut journal_ranges = Vec::new();
        let mut original_v1_envelopes = Vec::new();
        let mut discovered_receipts = BTreeMap::new();
        for selection in &request.journal_selections {
            let events = self.selected_events(selection).await?;
            let mut envelope_refs = Vec::with_capacity(events.len());
            for event in events {
                let mut embedded = Vec::new();
                collect_artifact_descriptors(&event.payload, &mut embedded);
                collect_artifact_descriptors(&event.metadata, &mut embedded);
                let references = unique_descriptor_digests(&embedded)?;
                for descriptor in embedded {
                    if descriptor.artifact_type_uri == EFFECT_RECEIPT_TYPE_URI {
                        insert_descriptor(&mut discovered_receipts, descriptor)?;
                    }
                }
                let descriptor = self
                    .commit_artifact(ArtifactCommitRequest {
                        artifact_type_uri: WORLD_EVENT_ENVELOPE_TYPE_URI.to_string(),
                        media_type: WORLD_EVENT_ENVELOPE_MEDIA_TYPE.to_string(),
                        bytes: Bytes::from(canonical_json_bytes(&event)?),
                        references,
                        annotations: BTreeMap::from([
                            ("session_id".to_string(), json!(event.session_id)),
                            ("sequence".to_string(), json!(event.sequence)),
                            ("event_kind".to_string(), json!(event.kind)),
                        ]),
                    })
                    .await?;
                envelope_refs.push(descriptor.clone());
                original_v1_envelopes.push(descriptor);
            }
            let first_sequence = envelope_refs
                .first()
                .and_then(|descriptor| descriptor.annotations.get("sequence"))
                .and_then(Value::as_u64)
                .expect("selected event descriptors retain sequence annotations");
            let last_sequence = envelope_refs
                .last()
                .and_then(|descriptor| descriptor.annotations.get("sequence"))
                .and_then(Value::as_u64)
                .expect("selected event descriptors retain sequence annotations");
            journal_ranges.push(WorldJournalRange {
                session_id: selection.session_id.clone(),
                first_sequence,
                last_sequence,
                envelope_refs,
            });
        }

        let history_root = self
            .commit_json_artifact(
                WORLD_JOURNAL_INDEX_TYPE_URI,
                WORLD_JOURNAL_INDEX_MEDIA_TYPE,
                &journal_ranges,
                original_v1_envelopes
                    .iter()
                    .map(|descriptor| descriptor.digest.clone())
                    .collect(),
                BTreeMap::new(),
            )
            .await?;

        let mut effect_receipts = BTreeMap::new();
        for descriptor in request.effect_receipts {
            anyhow::ensure!(
                descriptor.artifact_type_uri == EFFECT_RECEIPT_TYPE_URI,
                "effect_receipts contains a non-receipt artifact"
            );
            insert_descriptor(&mut effect_receipts, descriptor)?;
        }
        for descriptor in discovered_receipts.into_values() {
            insert_descriptor(&mut effect_receipts, descriptor)?;
        }
        let effect_receipts = effect_receipts.into_values().collect::<Vec<_>>();

        let mut policy_refs = unique_descriptors(request.policy_refs)?;
        let policy_root = match policy_refs.len() {
            0 => None,
            1 => policy_refs.first().cloned(),
            _ => Some(
                self.commit_json_artifact(
                    WORLD_POLICY_INDEX_TYPE_URI,
                    WORLD_POLICY_INDEX_MEDIA_TYPE,
                    &policy_refs,
                    policy_refs
                        .iter()
                        .map(|descriptor| descriptor.digest.clone())
                        .collect(),
                    BTreeMap::new(),
                )
                .await?,
            ),
        };
        if let Some(policy_root) = &policy_root {
            if !policy_refs
                .iter()
                .any(|descriptor| descriptor.digest == policy_root.digest)
            {
                policy_refs.push(policy_root.clone());
                policy_refs.sort_by(|left, right| left.digest.cmp(&right.digest));
            }
        }

        let provenance_material = json!({
            "schema_version": 1,
            "world_id": request.world_id,
            "state_root": request.state_root,
            "history_root": history_root,
            "composition_lock": composition_lock,
            "protocol_profiles": request.protocol_profiles,
            "policy_refs": policy_refs,
            "effect_receipts": effect_receipts,
            "parent_heads": request.parent_heads,
        });
        let mut provenance_descriptors = Vec::new();
        collect_artifact_descriptors(&provenance_material, &mut provenance_descriptors);
        let provenance_root = self
            .commit_json_artifact(
                WORLD_PROVENANCE_TYPE_URI,
                WORLD_PROVENANCE_MEDIA_TYPE,
                &provenance_material,
                unique_descriptor_digests(&provenance_descriptors)?,
                BTreeMap::new(),
            )
            .await?;

        let head = WorldHead {
            schema_version: 1,
            head_type_uri: WORLD_HEAD_TYPE_URI.to_string(),
            world_id: request.world_id.clone(),
            state_root: request.state_root.clone(),
            history_root: history_root.clone(),
            composition_lock: composition_lock.clone(),
            protocol_profiles: request.protocol_profiles.clone(),
            policy_root,
            provenance_root: provenance_root.clone(),
            effect_receipts: effect_receipts.clone(),
            parent_heads: unique_descriptors(request.parent_heads)?,
            annotations: request.annotations.clone(),
        };
        head.validate()?;
        let head_ref = self
            .commit_json_artifact(
                WORLD_HEAD_TYPE_URI,
                WORLD_HEAD_MEDIA_TYPE,
                &head,
                unique_descriptor_digests(
                    &head
                        .referenced_descriptors()
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>(),
                )?,
                BTreeMap::from([("world_id".to_string(), json!(request.world_id))]),
            )
            .await?;

        let mut lineage = request.prior_lineage;
        lineage.push(WorldLineageEntry {
            head: head_ref.clone(),
            parent_heads: head.parent_heads.clone(),
            relation: request.relation,
            effect_receipts: effect_receipts.clone(),
            annotations: request.annotations.clone(),
        });

        let mut roots = vec![
            head_ref.clone(),
            request.state_root,
            history_root,
            composition_lock.clone(),
            provenance_root,
        ];
        roots.extend(original_v1_envelopes.iter().cloned());
        roots.extend(policy_refs.iter().cloned());
        roots.extend(effect_receipts.iter().cloned());
        roots.extend(request.additional_roots);
        for entry in &lineage {
            roots.push(entry.head.clone());
            roots.extend(entry.parent_heads.iter().cloned());
            roots.extend(entry.effect_receipts.iter().cloned());
        }
        let closure = self.collect_object_closure(roots).await?;
        let object_descriptors = closure
            .values()
            .map(|(descriptor, _)| descriptor.clone())
            .collect::<Vec<_>>();

        let manifest = WorldBundleManifest {
            schema_version: 1,
            bundle_type_uri: WORLD_BUNDLE_TYPE_URI.to_string(),
            protocol_id: WORLD_BUNDLE_PROTOCOL_ID.to_string(),
            protocol_version: WORLD_BUNDLE_PROTOCOL_VERSION.to_string(),
            protocol_profile: WORLD_BUNDLE_EXPERIMENTAL_PROFILE.to_string(),
            world_id: request.world_id,
            world_head: head_ref,
            journal_ranges,
            object_descriptors,
            composition_lock,
            protocol_profiles: request.protocol_profiles,
            policy_refs,
            effect_receipts,
            lineage,
            original_v1_envelopes,
            annotations: request.annotations,
        };
        let bundle_descriptor = manifest.descriptor()?;
        let manifest_bytes = manifest.canonical_bytes()?;
        let stored_manifest = self
            .config
            .object_store
            .put(Bytes::from(manifest_bytes))
            .await?;
        anyhow::ensure!(
            stored_manifest.digest == bundle_descriptor.digest
                && stored_manifest.size_bytes == bundle_descriptor.size_bytes,
            "stored World Bundle manifest differs from its descriptor"
        );

        let archive = WorldBundleArchive {
            archive_format: WORLD_BUNDLE_ARCHIVE_FORMAT.to_string(),
            bundle_descriptor,
            manifest,
            objects: closure
                .into_values()
                .map(|(descriptor, bytes)| WorldBundleObject {
                    descriptor,
                    data_base64: BASE64_STANDARD.encode(bytes),
                })
                .collect(),
        };
        verify_world_bundle_archive(&archive)?;
        Ok(archive)
    }

    pub async fn import_world_bundle(
        &self,
        archive: &WorldBundleArchive,
    ) -> anyhow::Result<WorldBundleImportResult> {
        let _import_guard = self.world_bundle_import_lock.lock().await;
        anyhow::ensure!(
            self.store.supports_atomic_empty_session_batch_append(),
            "World Bundle import requires an EventStore with atomic empty-session batch append"
        );
        let verified = verify_archive_internal(archive)?;
        let session_ids = verified
            .events
            .iter()
            .map(|event| event.session_id.clone())
            .collect::<BTreeSet<_>>();
        let mut runtime_sessions = self.sessions.write().await;
        for session_id in &session_ids {
            anyhow::ensure!(
                self.store.list_session(session_id).await?.is_empty(),
                "World Bundle import target session '{session_id}' is not empty"
            );
            anyhow::ensure!(
                !runtime_sessions.contains_key(session_id),
                "World Bundle import target runtime already has session '{session_id}'"
            );
        }
        let existing_events = self.store.list_all().await?;
        let existing_event_ids = existing_events
            .iter()
            .map(|event| event.id.as_str())
            .collect::<BTreeSet<_>>();
        anyhow::ensure!(
            verified
                .events
                .iter()
                .all(|event| !existing_event_ids.contains(event.id.as_str())),
            "World Bundle import contains an event id already present in the destination"
        );
        let sessions = imported_sessions(&verified.events, &archive.bundle_descriptor.digest)?;
        let sessions_imported = sessions.len();

        let manifest_bytes = archive.manifest.canonical_bytes()?;
        let manifest_info = self
            .config
            .object_store
            .put(Bytes::from(manifest_bytes))
            .await?;
        anyhow::ensure!(manifest_info.digest == archive.bundle_descriptor.digest);
        for (_, bytes) in verified.objects.values() {
            let info = self.config.object_store.put(bytes.clone()).await?;
            anyhow::ensure!(
                info.digest == sha256_digest(bytes),
                "destination ObjectStore changed an imported digest"
            );
        }
        let substrate_state = self.build_substrate_state(&verified.events).await?;
        let required_empty_sessions = session_ids.into_iter().collect::<Vec<_>>();
        self.store
            .append_batch_atomic_if_sessions_empty(&verified.events, &required_empty_sessions)
            .await?;

        runtime_sessions.extend(sessions);
        drop(runtime_sessions);
        self.merge_substrate_state(substrate_state).await;

        Ok(WorldBundleImportResult {
            bundle_digest: archive.bundle_descriptor.digest.clone(),
            world_id: archive.manifest.world_id.clone(),
            head_digest: archive.manifest.world_head.digest.clone(),
            objects_imported: verified.objects.len(),
            events_imported: verified.events.len(),
            sessions_imported,
        })
    }

    async fn selected_events(
        &self,
        selection: &WorldJournalSelection,
    ) -> anyhow::Result<Vec<EventEnvelope>> {
        anyhow::ensure!(
            !selection.session_id.trim().is_empty(),
            "journal session id is empty"
        );
        if let (Some(first), Some(last)) = (selection.first_sequence, selection.last_sequence) {
            anyhow::ensure!(first <= last, "journal selection start exceeds end");
        }
        let events = self.store.list_session(&selection.session_id).await?;
        let events = events
            .into_iter()
            .filter(|event| {
                selection
                    .first_sequence
                    .is_none_or(|first| event.sequence >= first)
                    && selection
                        .last_sequence
                        .is_none_or(|last| event.sequence <= last)
            })
            .collect::<Vec<_>>();
        anyhow::ensure!(
            !events.is_empty(),
            "journal selection '{}' is empty",
            selection.session_id
        );
        for window in events.windows(2) {
            anyhow::ensure!(
                window[1].sequence == window[0].sequence + 1,
                "journal selection '{}' is not contiguous",
                selection.session_id
            );
        }
        if let Some(first) = selection.first_sequence {
            anyhow::ensure!(
                events[0].sequence == first,
                "journal selection start is missing"
            );
        }
        if let Some(last) = selection.last_sequence {
            anyhow::ensure!(
                events.last().is_some_and(|event| event.sequence == last),
                "journal selection end is missing"
            );
        }
        Ok(events)
    }

    async fn commit_json_artifact<T: Serialize>(
        &self,
        artifact_type_uri: &str,
        media_type: &str,
        value: &T,
        references: Vec<String>,
        annotations: BTreeMap<String, Value>,
    ) -> anyhow::Result<ArtifactDescriptor> {
        Ok(self
            .commit_artifact(ArtifactCommitRequest {
                artifact_type_uri: artifact_type_uri.to_string(),
                media_type: media_type.to_string(),
                bytes: Bytes::from(canonical_json_bytes(value)?),
                references,
                annotations,
            })
            .await?)
    }

    async fn collect_object_closure(
        &self,
        roots: Vec<ArtifactDescriptor>,
    ) -> anyhow::Result<BTreeMap<String, (ArtifactDescriptor, Bytes)>> {
        let mut descriptors = BTreeMap::new();
        let mut queue = VecDeque::new();
        for descriptor in roots {
            enqueue_descriptor(&mut descriptors, &mut queue, descriptor)?;
        }
        let mut objects = BTreeMap::new();
        while let Some(digest) = queue.pop_front() {
            let descriptor = descriptors
                .get(&digest)
                .cloned()
                .expect("queued descriptor exists");
            let bytes = self.read_artifact(&descriptor).await.map_err(|error| {
                anyhow::anyhow!(
                    "incomplete World Bundle closure at '{}': {error}",
                    descriptor.digest
                )
            })?;
            if is_json_media_type(&descriptor.media_type) {
                let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
                    anyhow::anyhow!(
                        "portable JSON artifact '{}' cannot be decoded: {error}",
                        descriptor.digest
                    )
                })?;
                let mut embedded = Vec::new();
                collect_artifact_descriptors(&value, &mut embedded);
                for embedded in embedded {
                    enqueue_descriptor(&mut descriptors, &mut queue, embedded)?;
                }
            }
            objects.insert(digest, (descriptor, bytes));
        }
        for descriptor in descriptors.values() {
            for reference in &descriptor.references {
                anyhow::ensure!(
                    descriptors.contains_key(reference),
                    "incomplete World Bundle closure: '{}' references missing object '{reference}'",
                    descriptor.digest
                );
            }
        }
        anyhow::ensure!(
            objects.len() == descriptors.len(),
            "World Bundle closure contains unread objects"
        );
        let mut closure = BTreeMap::new();
        for (digest, descriptor) in descriptors {
            let (_, bytes) = objects
                .remove(&digest)
                .expect("every final descriptor has verified bytes");
            closure.insert(digest, (descriptor, bytes));
        }
        Ok(closure)
    }
}

pub fn verify_world_bundle_archive(archive: &WorldBundleArchive) -> anyhow::Result<()> {
    verify_archive_internal(archive).map(|_| ())
}

pub fn audit_world_bundle_archive(
    archive: &WorldBundleArchive,
) -> anyhow::Result<WorldBundleAuditReport> {
    let verified = verify_archive_internal(archive)?;
    let session_count = verified
        .events
        .iter()
        .map(|event| event.session_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let non_plurora_namespace_artifact_count = archive
        .manifest
        .object_descriptors
        .iter()
        .filter(|descriptor| !descriptor.artifact_type_uri.starts_with("urn:plurora:"))
        .count();
    Ok(WorldBundleAuditReport {
        bundle_digest: archive.bundle_descriptor.digest.clone(),
        world_id: archive.manifest.world_id.clone(),
        head_digest: archive.manifest.world_head.digest.clone(),
        object_count: verified.objects.len(),
        journal_range_count: archive.manifest.journal_ranges.len(),
        event_count: verified.events.len(),
        session_count,
        effect_receipt_count: archive.manifest.effect_receipts.len(),
        lineage_entry_count: archive.manifest.lineage.len(),
        non_plurora_namespace_artifact_count,
        protocol_profile: archive.manifest.protocol_profile.clone(),
        reference_closure_verified: true,
        original_envelopes_verified: true,
        offline_replay_safe: true,
        shell_independent: true,
    })
}

pub fn replay_world_bundle_archive(
    archive: &WorldBundleArchive,
) -> anyhow::Result<WorldBundleReplayResult> {
    let verified = verify_archive_internal(archive)?;
    let mut receipts = Vec::new();
    for receipt_ref in &archive.manifest.effect_receipts {
        let (_, bytes) = verified
            .objects
            .get(&receipt_ref.digest)
            .expect("verified receipt object exists");
        let receipt: EffectReceipt = serde_json::from_slice(bytes)?;
        let mut outputs = Vec::new();
        for output_ref in &receipt.output_refs {
            let (_, bytes) = verified.objects.get(&output_ref.digest).ok_or_else(|| {
                anyhow::anyhow!("receipt output '{}' is missing", output_ref.digest)
            })?;
            outputs.push(serde_json::from_slice(bytes).map_err(|error| {
                anyhow::anyhow!(
                    "receipt output '{}' is invalid JSON: {error}",
                    output_ref.digest
                )
            })?);
        }
        receipts.push(WorldBundleReceiptReplay {
            receipt_ref: receipt_ref.clone(),
            receipt,
            outputs,
        });
    }
    Ok(WorldBundleReplayResult {
        bundle_digest: archive.bundle_descriptor.digest.clone(),
        world_id: archive.manifest.world_id.clone(),
        head: verified.head,
        events: verified.events,
        receipts,
        historical_only: true,
        executor_invocations: 0,
    })
}

fn verify_archive_internal(archive: &WorldBundleArchive) -> anyhow::Result<VerifiedWorldBundle> {
    archive.validate_shape()?;
    anyhow::ensure!(
        archive.objects.len() <= MAX_WORLD_BUNDLE_OBJECTS,
        "World Bundle exceeds the object-count safety limit"
    );
    ensure_decoded_size_within_limit(&archive.objects, MAX_WORLD_BUNDLE_TOTAL_BYTES)?;
    let mut objects = BTreeMap::new();
    let mut decoded_total = 0_u64;
    for object in &archive.objects {
        let bytes = BASE64_STANDARD
            .decode(&object.data_base64)
            .map_err(|error| {
                anyhow::anyhow!(
                    "object '{}' is not valid base64: {error}",
                    object.descriptor.digest
                )
            })?;
        decoded_total = decoded_total
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| anyhow::anyhow!("World Bundle decoded object sizes overflow"))?;
        anyhow::ensure!(
            decoded_total <= MAX_WORLD_BUNDLE_TOTAL_BYTES,
            "World Bundle exceeds the decoded-size safety limit"
        );
        anyhow::ensure!(
            bytes.len() as u64 == object.descriptor.size_bytes,
            "object '{}' size differs from its descriptor",
            object.descriptor.digest
        );
        anyhow::ensure!(
            sha256_digest(&bytes) == object.descriptor.digest,
            "object '{}' failed digest verification",
            object.descriptor.digest
        );
        objects.insert(
            object.descriptor.digest.clone(),
            (object.descriptor.clone(), Bytes::from(bytes)),
        );
    }
    for (descriptor, _) in objects.values() {
        for reference in &descriptor.references {
            anyhow::ensure!(
                objects.contains_key(reference),
                "incomplete World Bundle closure: '{}' references missing object '{reference}'",
                descriptor.digest
            );
        }
    }

    for (_, bytes) in objects.values() {
        if let Ok(value) = serde_json::from_slice::<Value>(bytes) {
            let mut embedded = Vec::new();
            collect_artifact_descriptors(&value, &mut embedded);
            for descriptor in embedded {
                verify_descriptor_present(&objects, &descriptor, "embedded artifact")?;
            }
        }
    }

    let head: WorldHead = decode_json_object(&objects, &archive.manifest.world_head, "world head")?;
    head.validate()?;
    anyhow::ensure!(
        head.world_id == archive.manifest.world_id,
        "world head id differs"
    );
    anyhow::ensure!(
        head.composition_lock == archive.manifest.composition_lock,
        "world head composition lock differs from the bundle manifest"
    );
    anyhow::ensure!(
        head.protocol_profiles == archive.manifest.protocol_profiles,
        "world head protocol profiles differ from the bundle manifest"
    );
    anyhow::ensure!(
        head.effect_receipts == archive.manifest.effect_receipts,
        "world head receipts differ from the bundle manifest"
    );
    for descriptor in head.referenced_descriptors() {
        verify_descriptor_present(&objects, descriptor, "world head reference")?;
    }
    anyhow::ensure!(
        head.policy_root.as_ref().is_none_or(|policy_root| {
            archive
                .manifest
                .policy_refs
                .iter()
                .any(|descriptor| descriptor == policy_root)
        }),
        "world head policy root is not present in policy_refs"
    );
    let _: Value = decode_json_object(&objects, &head.provenance_root, "world provenance")?;

    let composition_lock: CompositionLock = decode_json_object(
        &objects,
        &archive.manifest.composition_lock,
        "composition lock",
    )?;
    composition_lock.validate()?;
    anyhow::ensure!(
        composition_lock.protocol_profiles == archive.manifest.protocol_profiles,
        "composition lock protocol profiles differ from the bundle manifest"
    );
    for root in &composition_lock.content_roots {
        verify_descriptor_present(&objects, root, "composition content root")?;
    }

    let journal_ranges: Vec<WorldJournalRange> =
        decode_json_object(&objects, &head.history_root, "journal index")?;
    anyhow::ensure!(
        journal_ranges == archive.manifest.journal_ranges,
        "world head history root differs from journal_ranges"
    );
    let mut events = Vec::new();
    let mut event_ids = BTreeSet::new();
    let mut event_positions = BTreeSet::new();
    for range in &archive.manifest.journal_ranges {
        range.validate()?;
        for (offset, descriptor) in range.envelope_refs.iter().enumerate() {
            let event: EventEnvelope = decode_json_object(&objects, descriptor, "event envelope")?;
            let expected_sequence = range.first_sequence + offset as u64;
            anyhow::ensure!(
                event.session_id == range.session_id && event.sequence == expected_sequence,
                "original event envelope does not match its journal range"
            );
            anyhow::ensure!(
                event_ids.insert(event.id.clone()),
                "duplicate event id in bundle"
            );
            anyhow::ensure!(
                event_positions.insert((event.session_id.clone(), event.sequence)),
                "duplicate journal position in bundle"
            );
            events.push(event);
        }
    }

    let current_lineage = archive
        .manifest
        .lineage
        .last()
        .expect("manifest validation requires lineage");
    anyhow::ensure!(
        current_lineage.head == archive.manifest.world_head,
        "current lineage entry does not identify world_head"
    );
    anyhow::ensure!(
        current_lineage.parent_heads == head.parent_heads,
        "current lineage parents differ from world_head"
    );
    for entry in &archive.manifest.lineage {
        let lineage_head: WorldHead =
            decode_json_object(&objects, &entry.head, "lineage world head")?;
        lineage_head.validate()?;
        anyhow::ensure!(
            lineage_head.world_id == archive.manifest.world_id,
            "lineage contains a head for another world"
        );
        anyhow::ensure!(
            lineage_head.parent_heads == entry.parent_heads,
            "lineage entry parents differ from the referenced head"
        );
        for receipt in &entry.effect_receipts {
            verify_descriptor_present(&objects, receipt, "lineage receipt")?;
        }
    }

    for receipt_ref in &archive.manifest.effect_receipts {
        anyhow::ensure!(
            receipt_ref.artifact_type_uri == EFFECT_RECEIPT_TYPE_URI,
            "effect_receipts contains a non-receipt artifact"
        );
        let receipt: EffectReceipt = decode_json_object(&objects, receipt_ref, "effect receipt")?;
        anyhow::ensure!(
            receipt.receipt_type_uri == EFFECT_RECEIPT_TYPE_URI,
            "unsupported effect receipt type"
        );
        let declared = receipt_ref
            .references
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let actual = receipt
            .referenced_digests()
            .into_iter()
            .collect::<BTreeSet<_>>();
        anyhow::ensure!(
            declared == actual,
            "effect receipt '{}' reference list was altered",
            receipt_ref.digest
        );
        for output_ref in &receipt.output_refs {
            verify_descriptor_present(&objects, output_ref, "effect receipt output")?;
            let (_, bytes) = objects
                .get(&output_ref.digest)
                .expect("verified output object exists");
            serde_json::from_slice::<Value>(bytes).map_err(|error| {
                anyhow::anyhow!(
                    "effect receipt output '{}' is invalid JSON: {error}",
                    output_ref.digest
                )
            })?;
        }
    }

    Ok(VerifiedWorldBundle {
        objects,
        events,
        head,
    })
}

fn ensure_decoded_size_within_limit(
    objects: &[WorldBundleObject],
    max_bytes: u64,
) -> anyhow::Result<()> {
    let estimated_total = objects.iter().try_fold(0_u64, |total, object| {
        let estimate = u64::try_from(base64::decoded_len_estimate(object.data_base64.len()))
            .map_err(|_| anyhow::anyhow!("World Bundle decoded-size estimate overflow"))?;
        total
            .checked_add(estimate)
            .ok_or_else(|| anyhow::anyhow!("World Bundle decoded-size estimate overflow"))
    })?;
    anyhow::ensure!(
        estimated_total <= max_bytes,
        "World Bundle exceeds the decoded-size safety limit"
    );
    Ok(())
}

fn decode_json_object<T: for<'de> Deserialize<'de>>(
    objects: &BTreeMap<String, (ArtifactDescriptor, Bytes)>,
    descriptor: &ArtifactDescriptor,
    label: &str,
) -> anyhow::Result<T> {
    verify_descriptor_present(objects, descriptor, label)?;
    let (_, bytes) = objects
        .get(&descriptor.digest)
        .expect("verified descriptor object exists");
    serde_json::from_slice(bytes)
        .map_err(|error| anyhow::anyhow!("{label} '{}' is invalid: {error}", descriptor.digest))
}

fn verify_descriptor_present(
    objects: &BTreeMap<String, (ArtifactDescriptor, Bytes)>,
    descriptor: &ArtifactDescriptor,
    label: &str,
) -> anyhow::Result<()> {
    let (inventory, bytes) = objects
        .get(&descriptor.digest)
        .ok_or_else(|| anyhow::anyhow!("{label} '{}' is missing", descriptor.digest))?;
    anyhow::ensure!(
        !descriptor.artifact_type_uri.trim().is_empty() && !descriptor.media_type.trim().is_empty(),
        "{label} '{}' has an empty artifact type or media type",
        descriptor.digest
    );
    anyhow::ensure!(
        descriptor.size_bytes == bytes.len() as u64,
        "{label} '{}' has an incorrect descriptor size",
        descriptor.digest
    );
    // Type, media type, and annotations describe the role in which identical content is used.
    // A digest-keyed inventory keeps one canonical view, while every role-local view must keep
    // its closure references covered by that canonical entry.
    for reference in &descriptor.references {
        anyhow::ensure!(
            inventory.references.contains(reference),
            "{label} '{}' contains reference '{reference}' that is absent from the object inventory descriptor",
            descriptor.digest
        );
    }
    Ok(())
}

fn collect_artifact_descriptors(value: &Value, descriptors: &mut Vec<ArtifactDescriptor>) {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_artifact_descriptors(value, descriptors);
            }
        }
        Value::Object(object) => {
            if object.contains_key("artifact_type_uri")
                && object.contains_key("media_type")
                && object.contains_key("digest")
                && object.contains_key("size_bytes")
            {
                if let Ok(descriptor) =
                    serde_json::from_value::<ArtifactDescriptor>(Value::Object(object.clone()))
                {
                    descriptors.push(descriptor);
                }
            }
            for value in object.values() {
                collect_artifact_descriptors(value, descriptors);
            }
        }
        _ => {}
    }
}

fn unique_descriptor_digests(descriptors: &[ArtifactDescriptor]) -> anyhow::Result<Vec<String>> {
    Ok(unique_descriptors(descriptors.to_vec())?
        .into_iter()
        .map(|descriptor| descriptor.digest)
        .collect())
}

fn unique_descriptors(
    descriptors: Vec<ArtifactDescriptor>,
) -> anyhow::Result<Vec<ArtifactDescriptor>> {
    let mut unique = BTreeMap::new();
    for descriptor in descriptors {
        insert_descriptor(&mut unique, descriptor)?;
    }
    Ok(unique.into_values().collect())
}

fn insert_descriptor(
    descriptors: &mut BTreeMap<String, ArtifactDescriptor>,
    descriptor: ArtifactDescriptor,
) -> anyhow::Result<()> {
    merge_descriptor(descriptors, descriptor).map(|_| ())
}

fn merge_descriptor(
    descriptors: &mut BTreeMap<String, ArtifactDescriptor>,
    descriptor: ArtifactDescriptor,
) -> anyhow::Result<bool> {
    if let Some(existing) = descriptors.get_mut(&descriptor.digest) {
        anyhow::ensure!(
            existing.size_bytes == descriptor.size_bytes,
            "digest '{}' has conflicting descriptor sizes",
            descriptor.digest
        );
        let references = existing
            .references
            .iter()
            .chain(descriptor.references.iter())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut changed = references != existing.references;
        existing.references = references;
        if !is_json_media_type(&existing.media_type) && is_json_media_type(&descriptor.media_type) {
            existing.media_type = descriptor.media_type.clone();
            existing.artifact_type_uri = descriptor.artifact_type_uri.clone();
            changed = true;
        }
        for (key, value) in descriptor.annotations {
            if let std::collections::btree_map::Entry::Vacant(entry) =
                existing.annotations.entry(key)
            {
                entry.insert(value);
                changed = true;
            }
        }
        Ok(changed)
    } else {
        descriptors.insert(descriptor.digest.clone(), descriptor);
        Ok(true)
    }
}

fn enqueue_descriptor(
    descriptors: &mut BTreeMap<String, ArtifactDescriptor>,
    queue: &mut VecDeque<String>,
    descriptor: ArtifactDescriptor,
) -> anyhow::Result<()> {
    let digest = descriptor.digest.clone();
    let changed = merge_descriptor(descriptors, descriptor)?;
    if changed {
        queue.push_back(digest);
    }
    Ok(())
}

fn normalize_profiles(profiles: &mut Vec<ProtocolProfilePin>) {
    profiles.sort_by(|left, right| {
        (&left.protocol_id, &left.version, &left.profile).cmp(&(
            &right.protocol_id,
            &right.version,
            &right.profile,
        ))
    });
    profiles.dedup();
}

fn normalize_composition_lock(lock: &mut CompositionLock) {
    lock.components
        .sort_by(|left, right| left.component_id.cmp(&right.component_id));
    normalize_profiles(&mut lock.protocol_profiles);
    lock.content_roots
        .sort_by(|left, right| left.digest.cmp(&right.digest));
}

fn ensure_world_bundle_profile(profiles: &[ProtocolProfilePin]) -> anyhow::Result<()> {
    anyhow::ensure!(
        profiles.iter().any(|profile| {
            profile.protocol_id == WORLD_BUNDLE_PROTOCOL_ID
                && profile.version == WORLD_BUNDLE_PROTOCOL_VERSION
                && profile.profile == WORLD_BUNDLE_EXPERIMENTAL_PROFILE
        }),
        "World Bundle export requires the Experimental World Bundle profile pin"
    );
    Ok(())
}

fn imported_sessions(
    events: &[EventEnvelope],
    bundle_digest: &str,
) -> anyhow::Result<BTreeMap<String, KernelSession>> {
    let mut grouped = BTreeMap::<String, Vec<&EventEnvelope>>::new();
    for event in events {
        grouped
            .entry(event.session_id.clone())
            .or_default()
            .push(event);
    }
    let mut sessions = BTreeMap::new();
    for (session_id, mut events) in grouped {
        events.sort_by_key(|event| event.sequence);
        let first = events.first().expect("grouped session has events");
        let last = events.last().expect("grouped session has events");
        let opened = events
            .iter()
            .find(|event| event.kind == EVENT_SESSION_OPENED)
            .copied();
        let labels = opened
            .and_then(|event| event.payload.get("labels"))
            .cloned()
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or_default();
        let active_package_set = opened
            .and_then(|event| event.payload.get("active_package_set"))
            .cloned()
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or_default();
        let principal_scope = opened
            .and_then(|event| event.payload.get("principal_scope"))
            .cloned()
            .map(serde_json::from_value)
            .transpose()?
            .flatten();
        let status = if events
            .iter()
            .any(|event| event.kind == EVENT_SESSION_CLOSED)
        {
            SessionStatus::Closed
        } else {
            SessionStatus::Open
        };
        sessions.insert(
            session_id.clone(),
            KernelSession {
                id: session_id,
                labels,
                active_package_set,
                principal_scope,
                status,
                created_at: first.timestamp,
                updated_at: last.timestamp,
                metadata: json!({
                    "imported_world_bundle": bundle_digest,
                    "imported_at": Utc::now(),
                }),
            },
        );
    }
    Ok(sessions)
}

fn is_json_media_type(media_type: &str) -> bool {
    media_type == "application/json"
        || media_type.starts_with("application/json;")
        || media_type.contains("+json")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{InMemoryEventStore, RuntimeConfig};
    use plurora_core::{new_id, ComponentLockPin, ComponentTrustClass};

    #[tokio::test]
    async fn archive_verification_rejects_tampered_object() -> anyhow::Result<()> {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        let session = runtime
            .open_session(super::super::OpenSessionRequest::default())
            .await?;
        let state_root = runtime
            .commit_artifact(ArtifactCommitRequest::blob(
                "application/json",
                br#"{"ok":true}"#.to_vec(),
            ))
            .await?;
        let profile = ProtocolProfilePin {
            protocol_id: WORLD_BUNDLE_PROTOCOL_ID.to_string(),
            version: WORLD_BUNDLE_PROTOCOL_VERSION.to_string(),
            profile: WORLD_BUNDLE_EXPERIMENTAL_PROFILE.to_string(),
        };
        let lock = CompositionLock::new(
            vec![ComponentLockPin {
                component_id: "example/component".to_string(),
                digest: sha256_digest(b"component"),
                behavior_digest: sha256_digest(b"behavior"),
                trust_class: ComponentTrustClass::SandboxedComponent,
            }],
            vec![profile.clone()],
            Vec::new(),
        )?;
        let mut archive = runtime
            .export_world_bundle(WorldBundleExportRequest {
                world_id: "example/world".to_string(),
                state_root,
                journal_selections: vec![WorldJournalSelection::all(session.id)],
                composition_lock: lock,
                protocol_profiles: vec![profile],
                policy_refs: Vec::new(),
                effect_receipts: Vec::new(),
                parent_heads: Vec::new(),
                prior_lineage: Vec::new(),
                additional_roots: Vec::new(),
                relation: "exported".to_string(),
                annotations: BTreeMap::new(),
            })
            .await?;
        archive.objects[0].data_base64 = BASE64_STANDARD.encode(b"tampered");
        let error = verify_world_bundle_archive(&archive).expect_err("tampering must fail");
        assert!(
            error.to_string().contains("size differs")
                || error.to_string().contains("digest verification")
        );
        Ok(())
    }

    #[test]
    fn descriptor_roles_share_content_when_inventory_covers_references() -> anyhow::Result<()> {
        let bytes = Bytes::from_static(br#"{"ok":true}"#);
        let digest = sha256_digest(&bytes);
        let child_digest = sha256_digest(b"child");
        let inventory = ArtifactDescriptor {
            artifact_type_uri: "urn:example:state:v1".to_string(),
            media_type: "application/json".to_string(),
            digest: digest.clone(),
            size_bytes: bytes.len() as u64,
            references: vec![child_digest.clone()],
            annotations: BTreeMap::from([
                ("board_id".to_string(), json!("board:portable")),
                ("effect_role".to_string(), json!("original")),
            ]),
        };
        let output_view = ArtifactDescriptor {
            artifact_type_uri: "urn:example:effect-output:v1".to_string(),
            media_type: "application/json".to_string(),
            digest: digest.clone(),
            size_bytes: bytes.len() as u64,
            references: vec![child_digest],
            annotations: BTreeMap::from([("effect_role".to_string(), json!("replacement"))]),
        };
        let objects = BTreeMap::from([(digest.clone(), (inventory, bytes))]);
        verify_descriptor_present(&objects, &output_view, "effect output")?;

        let mut altered = output_view;
        altered.references.push(sha256_digest(b"undeclared-child"));
        verify_descriptor_present(&objects, &altered, "effect output")
            .expect_err("a descriptor view with an uncovered reference must fail");
        Ok(())
    }

    #[test]
    fn declared_size_cannot_bypass_decoded_size_limit() {
        let object = WorldBundleObject {
            descriptor: ArtifactDescriptor {
                artifact_type_uri: "urn:example:opaque:v1".to_string(),
                media_type: "application/octet-stream".to_string(),
                digest: sha256_digest(b"four"),
                size_bytes: 0,
                references: Vec::new(),
                annotations: BTreeMap::new(),
            },
            data_base64: BASE64_STANDARD.encode(b"four"),
        };
        ensure_decoded_size_within_limit(&[object], 3)
            .expect_err("encoded payload size, not the declared size, must enforce the limit");
    }

    #[test]
    fn imported_session_reconstruction_preserves_status() -> anyhow::Result<()> {
        let now = Utc::now();
        let session_id = "session-a".to_string();
        let events = vec![
            EventEnvelope {
                id: new_id("evt"),
                session_id: session_id.clone(),
                sequence: 0,
                writer_package_id: "kernel".to_string(),
                kind: EVENT_SESSION_OPENED.to_string(),
                schema_version: 1,
                timestamp: now,
                payload: json!({"labels":["portable"],"active_package_set":[]}),
                metadata: Value::Null,
            },
            EventEnvelope {
                id: new_id("evt"),
                session_id: session_id.clone(),
                sequence: 1,
                writer_package_id: "kernel".to_string(),
                kind: EVENT_SESSION_CLOSED.to_string(),
                schema_version: 1,
                timestamp: now,
                payload: json!({}),
                metadata: Value::Null,
            },
        ];
        let sessions = imported_sessions(&events, &sha256_digest(b"bundle"))?;
        assert_eq!(sessions[&session_id].status, SessionStatus::Closed);
        assert_eq!(sessions[&session_id].labels, vec!["portable"]);
        Ok(())
    }

    #[test]
    fn imported_session_reconstruction_rejects_invalid_open_payload() {
        let event = EventEnvelope {
            id: new_id("evt"),
            session_id: "session-invalid".to_string(),
            sequence: 0,
            writer_package_id: "kernel".to_string(),
            kind: EVENT_SESSION_OPENED.to_string(),
            schema_version: 1,
            timestamp: Utc::now(),
            payload: json!({"labels":"not-an-array","active_package_set":[]}),
            metadata: Value::Null,
        };
        imported_sessions(&[event], &sha256_digest(b"bundle"))
            .expect_err("invalid session payload must fail before journal commit");
    }

    #[tokio::test]
    async fn sqlite_atomic_batch_rolls_back_all_events() -> anyhow::Result<()> {
        let temp = tempfile::TempDir::new()?;
        let store = crate::SqliteEventStore::open(temp.path().join("events.sqlite3"))?;
        let existing = EventEnvelope {
            id: new_id("evt"),
            session_id: "existing-session".to_string(),
            sequence: 0,
            writer_package_id: "kernel".to_string(),
            kind: EVENT_SESSION_OPENED.to_string(),
            schema_version: 1,
            timestamp: Utc::now(),
            payload: json!({}),
            metadata: Value::Null,
        };
        store.append(existing.clone()).await?;
        let batch = vec![
            EventEnvelope {
                id: new_id("evt"),
                session_id: "new-session".to_string(),
                sequence: 0,
                writer_package_id: "kernel".to_string(),
                kind: EVENT_SESSION_OPENED.to_string(),
                schema_version: 1,
                timestamp: Utc::now(),
                payload: json!({}),
                metadata: Value::Null,
            },
            EventEnvelope {
                id: new_id("evt"),
                session_id: existing.session_id.clone(),
                sequence: existing.sequence,
                writer_package_id: "kernel".to_string(),
                kind: EVENT_SESSION_CLOSED.to_string(),
                schema_version: 1,
                timestamp: Utc::now(),
                payload: json!({}),
                metadata: Value::Null,
            },
        ];
        store
            .append_batch_atomic(&batch)
            .await
            .expect_err("batch conflict must roll back");
        assert!(store
            .list_session(&"new-session".to_string())
            .await?
            .is_empty());
        assert_eq!(store.list_session(&existing.session_id).await?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn sqlite_empty_session_gate_is_part_of_the_atomic_batch() -> anyhow::Result<()> {
        let temp = tempfile::TempDir::new()?;
        let store = crate::SqliteEventStore::open(temp.path().join("events.sqlite3"))?;
        let existing = EventEnvelope {
            id: new_id("evt"),
            session_id: "occupied-session".to_string(),
            sequence: 0,
            writer_package_id: "kernel".to_string(),
            kind: EVENT_SESSION_OPENED.to_string(),
            schema_version: 1,
            timestamp: Utc::now(),
            payload: json!({}),
            metadata: Value::Null,
        };
        store.append(existing.clone()).await?;
        let batch = vec![
            EventEnvelope {
                id: new_id("evt"),
                session_id: "new-session".to_string(),
                sequence: 0,
                writer_package_id: "kernel".to_string(),
                kind: EVENT_SESSION_OPENED.to_string(),
                schema_version: 1,
                timestamp: Utc::now(),
                payload: json!({}),
                metadata: Value::Null,
            },
            EventEnvelope {
                id: new_id("evt"),
                session_id: existing.session_id.clone(),
                sequence: 1,
                writer_package_id: "kernel".to_string(),
                kind: EVENT_SESSION_CLOSED.to_string(),
                schema_version: 1,
                timestamp: Utc::now(),
                payload: json!({}),
                metadata: Value::Null,
            },
        ];
        store
            .append_batch_atomic_if_sessions_empty(&batch, &[existing.session_id.clone()])
            .await
            .expect_err("occupied import session must reject the entire batch");
        assert!(store
            .list_session(&"new-session".to_string())
            .await?
            .is_empty());
        assert_eq!(store.list_session(&existing.session_id).await?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn in_memory_empty_session_gate_is_part_of_the_atomic_batch() -> anyhow::Result<()> {
        let store = InMemoryEventStore::default();
        let existing = EventEnvelope {
            id: new_id("evt"),
            session_id: "occupied-session".to_string(),
            sequence: 0,
            writer_package_id: "kernel".to_string(),
            kind: EVENT_SESSION_OPENED.to_string(),
            schema_version: 1,
            timestamp: Utc::now(),
            payload: json!({}),
            metadata: Value::Null,
        };
        store.append(existing.clone()).await?;
        let batch = vec![EventEnvelope {
            id: new_id("evt"),
            session_id: "new-session".to_string(),
            sequence: 0,
            writer_package_id: "kernel".to_string(),
            kind: EVENT_SESSION_OPENED.to_string(),
            schema_version: 1,
            timestamp: Utc::now(),
            payload: json!({}),
            metadata: Value::Null,
        }];
        store
            .append_batch_atomic_if_sessions_empty(&batch, &[existing.session_id.clone()])
            .await
            .expect_err("occupied import session must reject the entire batch");
        assert!(store
            .list_session(&"new-session".to_string())
            .await?
            .is_empty());
        assert_eq!(store.list_session(&existing.session_id).await?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn imported_substrate_merge_preserves_existing_entries() -> anyhow::Result<()> {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        let existing = crate::ProjectionDefinition {
            id: "projection-existing".to_string(),
            session_id: "session-existing".to_string(),
            source_kind_prefix: None,
            state: json!({"value":"existing"}),
        };
        runtime
            .projections
            .write()
            .await
            .insert(existing.id.clone(), existing.clone());
        let imported = crate::ProjectionDefinition {
            id: "projection-imported".to_string(),
            session_id: "session-imported".to_string(),
            source_kind_prefix: None,
            state: json!({"value":"imported"}),
        };
        let event = EventEnvelope {
            id: new_id("evt"),
            session_id: imported.session_id.clone(),
            sequence: 0,
            writer_package_id: "kernel".to_string(),
            kind: plurora_core::EVENT_PROJECTION_UPDATED.to_string(),
            schema_version: 1,
            timestamp: Utc::now(),
            payload: serde_json::to_value(&imported)?,
            metadata: Value::Null,
        };
        let imported_state = runtime.build_substrate_state(&[event]).await?;
        runtime.merge_substrate_state(imported_state).await;

        let projections = runtime.projections.read().await;
        assert_eq!(projections.len(), 2);
        assert_eq!(projections[&existing.id].state, existing.state);
        assert_eq!(projections[&imported.id].state, imported.state);
        Ok(())
    }
}
