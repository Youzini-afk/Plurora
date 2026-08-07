use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use plurora_core::{
    canonical_json_bytes, package_envelope_for_manifest, ArtifactDescriptor, ComponentLockPin,
    CompositionLock, ProtocolProfilePin, WorldBundleArchive, WORLD_BUNDLE_EXPERIMENTAL_PROFILE,
    WORLD_BUNDLE_PROTOCOL_ID, WORLD_BUNDLE_PROTOCOL_VERSION,
};
use plurora_runtime::{
    audit_world_bundle_archive, replay_world_bundle_archive, verify_world_bundle_archive,
    ArtifactCommitRequest, CapabilityInvocationRequest, EventStore, FilesystemObjectStore,
    InMemoryEventStore, Runtime, RuntimeConfig, SqliteEventStore, WorldBundleExportRequest,
    WorldJournalSelection,
};
use serde_json::{json, Value};
use tempfile::TempDir;

use super::fixtures::{echo_package, runtime};
use crate::commands::{manifest, world_bundle as world_bundle_command};

const PLAYABLE_BOARD_PACKAGE_ID: &str = "official/playable-creation-board";
const PLAYABLE_BOARD_CAPABILITY_ID: &str = "official/playable-creation-board/record_player_action";
const UNKNOWN_ARTIFACT_TYPE_URI: &str = "urn:example:opaque-board-extension:v1";

struct PortableBoardFixture {
    archive: WorldBundleArchive,
    source_sessions: Vec<String>,
    composition_lock: CompositionLock,
    unknown_artifact: ArtifactDescriptor,
}

pub(crate) async fn reference_closure() -> anyhow::Result<()> {
    let fixture = portable_board_fixture().await?;
    verify_world_bundle_archive(&fixture.archive)?;
    let audit = audit_world_bundle_archive(&fixture.archive)?;
    anyhow::ensure!(audit.reference_closure_verified);
    anyhow::ensure!(audit.effect_receipt_count >= 2);
    anyhow::ensure!(audit.non_plurora_namespace_artifact_count >= 1);
    anyhow::ensure!(
        fixture
            .archive
            .manifest
            .object_descriptors
            .iter()
            .any(|descriptor| descriptor == &fixture.unknown_artifact),
        "unknown artifacts must be preserved rather than discarded"
    );

    let mut missing = fixture.archive.clone();
    missing.objects.pop();
    anyhow::ensure!(
        verify_world_bundle_archive(&missing).is_err(),
        "missing closure object was accepted"
    );

    let mut tampered = fixture.archive.clone();
    tampered.objects[0].data_base64 = "dGFtcGVyZWQ=".to_string();
    anyhow::ensure!(
        verify_world_bundle_archive(&tampered).is_err(),
        "tampered object was accepted"
    );

    let mut host_path = fixture.archive.clone();
    let digest = fixture.unknown_artifact.digest.as_str();
    host_path
        .manifest
        .object_descriptors
        .iter_mut()
        .find(|descriptor| descriptor.digest == digest)
        .expect("unknown descriptor in inventory")
        .references
        .push("C:\\host-a\\world-state.bin".to_string());
    host_path
        .objects
        .iter_mut()
        .find(|object| object.descriptor.digest == digest)
        .expect("unknown inline object")
        .descriptor
        .references
        .push("C:\\host-a\\world-state.bin".to_string());
    anyhow::ensure!(
        verify_world_bundle_archive(&host_path).is_err(),
        "absolute host path was accepted as portable identity"
    );

    let mut spoofed_type = fixture.archive.clone();
    spoofed_type.manifest.world_head.artifact_type_uri =
        "urn:example:not-a-world-head:v1".to_string();
    anyhow::ensure!(
        verify_world_bundle_archive(&spoofed_type).is_err(),
        "a spoofed world-head descriptor type was accepted"
    );

    let mut broken_lineage = fixture.archive.clone();
    let self_parent = broken_lineage.manifest.lineage[0].head.clone();
    broken_lineage.manifest.lineage[0]
        .parent_heads
        .push(self_parent);
    anyhow::ensure!(
        verify_world_bundle_archive(&broken_lineage).is_err(),
        "a cyclic world lineage was accepted"
    );
    anyhow::ensure!(
        fixture
            .archive
            .manifest
            .journal_ranges
            .windows(2)
            .all(|window| window[0].session_id < window[1].session_id),
        "cross-session journal ranges are not canonically ordered"
    );
    Ok(())
}

pub(crate) async fn cross_host_import() -> anyhow::Result<()> {
    let fixture = portable_board_fixture().await?;
    let source_replay = replay_world_bundle_archive(&fixture.archive)?;
    let temp = TempDir::new()?;
    let store = Arc::new(SqliteEventStore::open(temp.path().join("events.sqlite3"))?);
    let mut config = RuntimeConfig::default();
    config.object_store = Arc::new(FilesystemObjectStore::new(temp.path().join("objects")));
    let host_b = Runtime::new(store.clone(), config);

    let imported = host_b.import_world_bundle(&fixture.archive).await?;
    anyhow::ensure!(imported.bundle_digest == fixture.archive.bundle_descriptor.digest);
    anyhow::ensure!(imported.head_digest == fixture.archive.manifest.world_head.digest);
    anyhow::ensure!(imported.events_imported == source_replay.events.len());
    anyhow::ensure!(imported.sessions_imported == fixture.source_sessions.len());
    anyhow::ensure!(host_b.packages().list().await.is_empty());

    let imported_events = store.list_all().await?;
    let mut imported_by_position = BTreeMap::new();
    for event in &imported_events {
        imported_by_position.insert(
            (event.session_id.clone(), event.sequence),
            serde_json::to_value(event)?,
        );
    }
    let mut source_by_position = BTreeMap::new();
    for event in &source_replay.events {
        source_by_position.insert(
            (event.session_id.clone(), event.sequence),
            serde_json::to_value(event)?,
        );
    }
    anyhow::ensure!(
        imported_by_position == source_by_position,
        "cross-host import changed original v1 envelopes"
    );
    let unknown_bytes = host_b
        .object_store()
        .get(&fixture.unknown_artifact.digest)
        .await?;
    anyhow::ensure!(unknown_bytes.as_ref() == b"opaque-extension-v1");
    anyhow::ensure!(
        host_b
            .object_store()
            .has(&fixture.archive.bundle_descriptor.digest)
            .await?,
        "bundle manifest itself was not content-addressed on Host B"
    );

    let duplicate = host_b
        .import_world_bundle(&fixture.archive)
        .await
        .expect_err("non-empty import scope must be rejected");
    anyhow::ensure!(duplicate.to_string().contains("is not empty"));
    Ok(())
}

pub(crate) async fn offline_replay() -> anyhow::Result<()> {
    let fixture = portable_board_fixture().await?;
    let replay = replay_world_bundle_archive(&fixture.archive)?;
    anyhow::ensure!(replay.historical_only);
    anyhow::ensure!(replay.executor_invocations == 0);
    anyhow::ensure!(replay.receipts.len() >= 2);
    anyhow::ensure!(
        replay
            .receipts
            .iter()
            .all(|receipt| !receipt.outputs.is_empty()),
        "recorded effect outputs were not available offline"
    );
    anyhow::ensure!(
        replay
            .events
            .windows(2)
            .all(|window| window[0].session_id != window[1].session_id
                || window[0].sequence < window[1].sequence),
        "offline replay order is not deterministic"
    );
    Ok(())
}

pub(crate) async fn reexecution_branch() -> anyhow::Result<()> {
    let fixture = portable_board_fixture().await?;
    let temp = TempDir::new()?;
    let store = Arc::new(SqliteEventStore::open(temp.path().join("events.sqlite3"))?);
    let mut config = RuntimeConfig::default();
    config.object_store = Arc::new(FilesystemObjectStore::new(temp.path().join("objects")));
    let host_b = Runtime::new(store.clone(), config);
    host_b.import_world_bundle(&fixture.archive).await?;

    let replacement_capability = "thirdparty/playable-board-reference/record_player_action";
    let replacement_manifest = echo_package(
        "thirdparty/playable-board-reference",
        replacement_capability,
    );
    host_b.load_package(replacement_manifest.clone()).await?;

    let parent_session = fixture
        .source_sessions
        .last()
        .cloned()
        .expect("fixture has source sessions");
    let parent_events = store.list_session(&parent_session).await?;
    let parent_sequence = parent_events.last().expect("parent has events").sequence;
    let branch = host_b
        .fork_session(
            parent_session,
            parent_sequence,
            json!({"reason":"World Bundle re-execution with replacement component"}),
        )
        .await?;
    let replacement = host_b
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(replacement_capability.to_string()),
            caller_package_id: None,
            provider_package_id: Some("thirdparty/playable-board-reference".to_string()),
            version: None,
            session_id: Some(branch.child_session_id.clone()),
            input: json!({
                "board_id": "board:portable",
                "action_kind": "move_marker",
                "sequence": 3,
                "payload": {"marker_id":"m1","x":9,"y":4},
            }),
        })
        .await?;
    let replacement_receipt = replacement
        .receipt
        .clone()
        .ok_or_else(|| anyhow::anyhow!("replacement invocation produced no receipt"))?;
    let state_root = host_b
        .commit_artifact(ArtifactCommitRequest {
            artifact_type_uri: "urn:example:playable-board-state:v1".to_string(),
            media_type: "application/json".to_string(),
            bytes: canonical_json_bytes(&replacement.output)?.into(),
            references: Vec::new(),
            annotations: BTreeMap::new(),
        })
        .await?;
    let replacement_envelope = package_envelope_for_manifest(&replacement_manifest)?;
    let replacement_lock = CompositionLock::new(
        replacement_envelope
            .components
            .iter()
            .map(ComponentLockPin::from_descriptor)
            .collect(),
        fixture.composition_lock.protocol_profiles.clone(),
        fixture.composition_lock.content_roots.clone(),
    )?;

    let mut journal_selections = fixture
        .source_sessions
        .iter()
        .cloned()
        .map(WorldJournalSelection::all)
        .collect::<Vec<_>>();
    journal_selections.push(WorldJournalSelection::all(branch.child_session_id));
    let derived = host_b
        .export_world_bundle(WorldBundleExportRequest {
            world_id: fixture.archive.manifest.world_id.clone(),
            state_root,
            journal_selections,
            composition_lock: replacement_lock.clone(),
            protocol_profiles: replacement_lock.protocol_profiles.clone(),
            policy_refs: fixture.archive.manifest.policy_refs.clone(),
            effect_receipts: vec![replacement_receipt],
            parent_heads: vec![fixture.archive.manifest.world_head.clone()],
            prior_lineage: fixture.archive.manifest.lineage.clone(),
            additional_roots: vec![fixture.unknown_artifact.clone()],
            relation: "reexecuted_with_replacement".to_string(),
            annotations: BTreeMap::from([(
                "branch_id".to_string(),
                Value::String(branch.id.clone()),
            )]),
        })
        .await?;

    anyhow::ensure!(
        derived.manifest.world_head.digest != fixture.archive.manifest.world_head.digest,
        "re-execution did not create a new head"
    );
    let current = derived.manifest.lineage.last().expect("derived lineage");
    anyhow::ensure!(
        current.parent_heads == vec![fixture.archive.manifest.world_head.clone()],
        "derived head did not retain its parent"
    );
    anyhow::ensure!(current.annotations["branch_id"] == branch.id);
    anyhow::ensure!(
        replacement_lock.content_roots == fixture.composition_lock.content_roots,
        "component replacement changed content roots"
    );
    anyhow::ensure!(
        replacement_lock.components != fixture.composition_lock.components,
        "re-execution did not use a different implementation"
    );
    anyhow::ensure!(
        fixture.archive.manifest.lineage.len() == 1,
        "derivation mutated the imported archive"
    );
    Ok(())
}

pub(crate) async fn shell_independence() -> anyhow::Result<()> {
    let fixture = portable_board_fixture().await?;
    let temp = TempDir::new()?;
    let archive_path = temp.path().join("portable-board.plurora-world.json");
    tokio::fs::write(&archive_path, serde_json::to_vec_pretty(&fixture.archive)?).await?;

    let direct = audit_world_bundle_archive(&fixture.archive)?;
    let headless = world_bundle_command::audit_file(&archive_path).await?;
    anyhow::ensure!(direct == headless);
    let replay = world_bundle_command::replay_file(&archive_path).await?;
    anyhow::ensure!(replay.world_id == direct.world_id);
    anyhow::ensure!(
        replay.head.state_root.digest
            == fixture
                .archive
                .manifest
                .object_descriptors
                .iter()
                .find(|descriptor| descriptor.digest == replay.head.state_root.digest)
                .expect("state root remains in portable inventory")
                .digest
    );
    anyhow::ensure!(headless.shell_independent);
    Ok(())
}

async fn portable_board_fixture() -> anyhow::Result<PortableBoardFixture> {
    let (_store, host_a) = runtime();
    let manifest_path = PathBuf::from("packages/official/playable-creation-board/manifest.yaml");
    let package_manifest = manifest::read_manifest(manifest_path).await?;
    host_a.load_package(package_manifest.clone()).await?;

    let parent = host_a
        .open_session(plurora_runtime::OpenSessionRequest {
            labels: vec![
                "playable-creation-board".to_string(),
                "portable".to_string(),
            ],
            active_package_set: vec![PLAYABLE_BOARD_PACKAGE_ID.to_string()],
            metadata: json!({"world_id":"official/playable-creation-board/portable-world"}),
        })
        .await?;
    let first = invoke_board_action(&host_a, &parent.id, 1, "place_marker").await?;
    let parent_sequence = host_a
        .store()
        .list_session(&parent.id)
        .await?
        .last()
        .expect("parent has invocation events")
        .sequence;
    let branch = host_a
        .fork_session(
            parent.id.clone(),
            parent_sequence,
            json!({"reason":"portable branch proof"}),
        )
        .await?;
    let second = invoke_board_action(&host_a, &branch.child_session_id, 2, "move_marker").await?;

    let state_root = host_a
        .commit_artifact(ArtifactCommitRequest {
            artifact_type_uri: "urn:example:playable-board-state:v1".to_string(),
            media_type: "application/json".to_string(),
            bytes: canonical_json_bytes(&second.output)?.into(),
            references: Vec::new(),
            annotations: BTreeMap::from([("board_id".to_string(), json!("board:portable"))]),
        })
        .await?;
    let unknown_artifact = host_a
        .commit_artifact(ArtifactCommitRequest {
            artifact_type_uri: UNKNOWN_ARTIFACT_TYPE_URI.to_string(),
            media_type: "application/octet-stream".to_string(),
            bytes: b"opaque-extension-v1".as_slice().to_vec().into(),
            references: Vec::new(),
            annotations: BTreeMap::from([("copy_semantics".to_string(), json!("preserve"))]),
        })
        .await?;
    let profile = world_bundle_profile();
    let envelope = package_envelope_for_manifest(&package_manifest)?;
    let composition_lock = CompositionLock::new(
        envelope
            .components
            .iter()
            .map(ComponentLockPin::from_descriptor)
            .collect(),
        vec![profile.clone()],
        vec![unknown_artifact.clone()],
    )?;
    let receipts = [first.receipt, second.receipt]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    anyhow::ensure!(
        receipts.len() == 2,
        "controlled effects did not produce receipts"
    );

    let source_sessions = vec![parent.id, branch.child_session_id];
    let archive = host_a
        .export_world_bundle(WorldBundleExportRequest {
            world_id: "official/playable-creation-board/portable-world".to_string(),
            state_root,
            journal_selections: source_sessions
                .iter()
                .cloned()
                .map(WorldJournalSelection::all)
                .collect(),
            composition_lock: composition_lock.clone(),
            protocol_profiles: vec![profile],
            policy_refs: Vec::new(),
            effect_receipts: receipts,
            parent_heads: Vec::new(),
            prior_lineage: Vec::new(),
            additional_roots: vec![unknown_artifact.clone()],
            relation: "exported".to_string(),
            annotations: BTreeMap::from([(
                "pressure_source".to_string(),
                json!(PLAYABLE_BOARD_PACKAGE_ID),
            )]),
        })
        .await?;
    Ok(PortableBoardFixture {
        archive,
        source_sessions,
        composition_lock,
        unknown_artifact,
    })
}

async fn invoke_board_action(
    runtime: &Runtime<InMemoryEventStore>,
    session_id: &str,
    sequence: u64,
    action_kind: &str,
) -> anyhow::Result<plurora_runtime::CapabilityInvocationResult> {
    Ok(runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(PLAYABLE_BOARD_CAPABILITY_ID.to_string()),
            caller_package_id: None,
            provider_package_id: Some(PLAYABLE_BOARD_PACKAGE_ID.to_string()),
            version: None,
            session_id: Some(session_id.to_string()),
            input: json!({
                "board_id": "board:portable",
                "action_kind": action_kind,
                "sequence": sequence,
                "payload": {"marker_id":"m1","x":sequence,"y":sequence + 1},
            }),
        })
        .await?)
}

fn world_bundle_profile() -> ProtocolProfilePin {
    ProtocolProfilePin {
        protocol_id: WORLD_BUNDLE_PROTOCOL_ID.to_string(),
        version: WORLD_BUNDLE_PROTOCOL_VERSION.to_string(),
        profile: WORLD_BUNDLE_EXPERIMENTAL_PROFILE.to_string(),
    }
}
