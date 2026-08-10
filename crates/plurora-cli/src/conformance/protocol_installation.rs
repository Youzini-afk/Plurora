use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::sync::Arc;

use anyhow::Context;
use plurora_core::{
    canonical_json_bytes, ArtifactDescriptor, ComponentArtifactPayload, ComponentBoundaryClaims,
    ComponentClaimStatus, ComponentTrustClass, ContractMode, PackageEntry,
    COMPONENT_BEHAVIOR_TYPE_URI, COMPONENT_DESCRIPTOR_TYPE_URI,
};
use plurora_runtime::{
    EventStore, InMemoryEventStore, InMemoryObjectStore, InstallationCreateRequest,
    InstallationMutationResult, InstallationRemoveRequest, InstallationStateAction,
    InstallationStateSnapshot, InstallationStateSnapshotEntry, InstallationUpdateRequest,
    ObjectStore, ProtocolContext, ProtocolResourceSelector, Runtime, RuntimeConfig,
    StateDisposition, INSTALLATION_STATE_SNAPSHOT_MEDIA_TYPE, INSTALLATION_STATE_SNAPSHOT_TYPE_URI,
};
use plurora_service::InstallationRegistry;
use plurora_work::{
    AcquisitionKind, AcquisitionRecord, ArtifactModel, AssemblyId, AssemblyLock, AssemblyNode,
    AssemblyNodeSource, AssemblyRevision, BackupPolicy, EffectClass, InstallationSecretPolicy,
    InstallationStatus, InteractionModelId, NodeId, NodeLock, PortContract, PortDescriptor,
    PortEndpoint, PortId, PortMultiplicity, PortRole, StatePortability, StateScope,
    StateSlotDescriptor, StateSlotId, TransportRequirements, WorkId, WorkRevision,
    INTERACTION_SNAPSHOT,
};
use serde::Serialize;
use serde_json::{json, Value};
use tempfile::TempDir;

pub(crate) struct InstallationFixture {
    pub(crate) data: TempDir,
    pub(crate) store: Arc<InMemoryEventStore>,
    pub(crate) objects: Arc<InMemoryObjectStore>,
    pub(crate) registry: Arc<InstallationRegistry>,
    pub(crate) runtime: Runtime<InMemoryEventStore>,
    pub(crate) work_id: WorkId,
    pub(crate) work_revision: ArtifactDescriptor,
    pub(crate) assembly_lock: ArtifactDescriptor,
}

pub(crate) async fn fixture() -> anyhow::Result<InstallationFixture> {
    let data = tempfile::tempdir()?;
    let store = Arc::new(InMemoryEventStore::default());
    let objects = Arc::new(InMemoryObjectStore::default());
    let registry = InstallationRegistry::persistent(store.clone(), objects.clone(), data.path())?;
    registry.hydrate().await?;
    let runtime = Runtime::new(
        store.clone(),
        RuntimeConfig {
            object_store: objects.clone(),
            installation_control: registry.clone(),
            ..RuntimeConfig::default()
        },
    );
    let (work_revision, assembly_lock) = put_work_pair(&objects, "one").await?;
    Ok(InstallationFixture {
        data,
        store,
        objects,
        registry,
        runtime,
        work_id: WorkId::parse("conformance/one")?,
        work_revision,
        assembly_lock,
    })
}

async fn put_json<T: ArtifactModel>(
    objects: &InMemoryObjectStore,
    value: &T,
) -> anyhow::Result<ArtifactDescriptor> {
    let bytes = value.canonical_bytes()?;
    let descriptor = value.artifact_descriptor()?;
    let info = objects.put(bytes.into()).await?;
    anyhow::ensure!(info.digest == descriptor.digest);
    anyhow::ensure!(info.size_bytes == descriptor.size_bytes);
    Ok(descriptor)
}

async fn put_raw(
    objects: &InMemoryObjectStore,
    artifact_type_uri: &str,
    media_type: &str,
    bytes: Vec<u8>,
    references: Vec<String>,
) -> anyhow::Result<ArtifactDescriptor> {
    let info = objects.put(bytes.into()).await?;
    Ok(ArtifactDescriptor {
        artifact_type_uri: artifact_type_uri.to_string(),
        media_type: media_type.to_string(),
        digest: info.digest,
        size_bytes: info.size_bytes,
        references,
        annotations: BTreeMap::new(),
    })
}

async fn put_stateful_work_pair(
    objects: &InMemoryObjectStore,
    marker: &str,
    schema_marker: &str,
    portability: StatePortability,
    scope: StateScope,
    migration_port: bool,
) -> anyhow::Result<(ArtifactDescriptor, ArtifactDescriptor)> {
    let behavior = put_raw(
        objects,
        COMPONENT_BEHAVIOR_TYPE_URI,
        "application/json",
        canonical_json_bytes(&json!({"capabilities": []}))?,
        Vec::new(),
    )
    .await?;
    let component_payload = ComponentArtifactPayload {
        component_id: format!("conformance/{marker}-component"),
        version: "1.0.0".to_string(),
        behavior: behavior.clone(),
        entry_kind: "wasm".to_string(),
        entry: PackageEntry::Wasm {
            module: "component.wasm".to_string(),
            abi_version: 1,
            memory_limit_mb: 64,
        },
        contract: ContractMode::V1,
        trust_class: ComponentTrustClass::SandboxedComponent,
        claim_status: ComponentClaimStatus::Declared,
        enforced_boundaries: ComponentBoundaryClaims::default(),
        protocol_artifacts: Vec::new(),
        content_roots: Vec::new(),
        surface_artifacts: Vec::new(),
        annotations: BTreeMap::new(),
    };
    let mut component = put_raw(
        objects,
        COMPONENT_DESCRIPTOR_TYPE_URI,
        "application/json",
        canonical_json_bytes(&component_payload)?,
        vec![behavior.digest.clone()],
    )
    .await?;
    component.annotations.insert(
        "component_id".to_string(),
        json!(component_payload.component_id),
    );
    let schema = put_raw(
        objects,
        "urn:plurora:test:state-schema:v1",
        "application/schema+json",
        canonical_json_bytes(&json!({"type": "object", "marker": schema_marker}))?,
        Vec::new(),
    )
    .await?;
    let node_id = NodeId::parse("state-owner")?;
    let port_id = PortId::parse("migrate")?;
    let migration = PortDescriptor {
        port_id: port_id.clone(),
        contract: PortContract {
            protocol_id: "plurora.state".to_string(),
            interface_id: "snapshot-migration".to_string(),
            version: "1.0.0".to_string(),
            profiles: Vec::new(),
        },
        interaction: InteractionModelId(INTERACTION_SNAPSHOT.to_string()),
        role: PortRole::Export {
            multiplicity: PortMultiplicity {
                min: 0,
                max: Some(1),
            },
            effect_class: EffectClass::DeterministicStateful,
        },
        transport: TransportRequirements::default(),
        annotations: BTreeMap::new(),
    };
    let assembly = AssemblyRevision {
        schema: AssemblyRevision::SCHEMA.to_string(),
        assembly_id: AssemblyId::parse(format!("conformance/{marker}-assembly"))?,
        nodes: vec![AssemblyNode {
            node_id: node_id.clone(),
            source: AssemblyNodeSource::Component {
                component: component.clone(),
            },
            ports: vec![migration],
            configuration: None,
            annotations: BTreeMap::new(),
        }],
        bindings: Vec::new(),
        exposed_ports: Vec::new(),
        state_slots: vec![StateSlotDescriptor {
            state_slot_id: StateSlotId::parse("save")?,
            owner_node_id: node_id.clone(),
            schema_ref: Some(schema),
            scope,
            portability,
            migration_port: migration_port.then_some(PortEndpoint {
                node_id: node_id.clone(),
                port_id,
            }),
            backup_policy: BackupPolicy::Required,
            annotations: BTreeMap::new(),
        }],
        annotations: BTreeMap::new(),
    };
    let assembly = put_json(objects, &assembly).await?;
    let work = WorkRevision {
        schema: WorkRevision::SCHEMA.to_string(),
        work_id: WorkId::parse(format!("conformance/{marker}"))?,
        title: format!("Conformance {marker}"),
        description: String::new(),
        assembly: assembly.clone(),
        content_roots: Vec::new(),
        entrypoints: Vec::new(),
        rights: None,
        transparency: None,
        operational_intent: None,
        annotations: BTreeMap::new(),
    };
    let lock = AssemblyLock {
        schema: AssemblyLock::SCHEMA.to_string(),
        assembly,
        nodes: vec![NodeLock {
            node_id,
            artifact: component,
            behavior_digest: Some(behavior.digest),
            trust_class: Some(ComponentTrustClass::SandboxedComponent),
        }],
        bindings: Vec::new(),
        protocol_profiles: Vec::new(),
        content_roots: Vec::new(),
    };
    Ok((
        put_json(objects, &work).await?,
        put_json(objects, &lock).await?,
    ))
}

pub(crate) async fn put_work_pair(
    objects: &InMemoryObjectStore,
    marker: &str,
) -> anyhow::Result<(ArtifactDescriptor, ArtifactDescriptor)> {
    let assembly = AssemblyRevision {
        schema: AssemblyRevision::SCHEMA.to_string(),
        assembly_id: AssemblyId::parse(format!("conformance/{marker}-assembly"))?,
        nodes: Vec::new(),
        bindings: Vec::new(),
        exposed_ports: Vec::new(),
        state_slots: Vec::new(),
        annotations: BTreeMap::new(),
    };
    let assembly = put_json(objects, &assembly).await?;
    let work = WorkRevision {
        schema: WorkRevision::SCHEMA.to_string(),
        work_id: WorkId::parse(format!("conformance/{marker}"))?,
        title: format!("Conformance {marker}"),
        description: String::new(),
        assembly: assembly.clone(),
        content_roots: Vec::new(),
        entrypoints: Vec::new(),
        rights: None,
        transparency: None,
        operational_intent: None,
        annotations: BTreeMap::new(),
    };
    let lock = AssemblyLock {
        schema: AssemblyLock::SCHEMA.to_string(),
        assembly,
        nodes: Vec::new(),
        bindings: Vec::new(),
        protocol_profiles: Vec::new(),
        content_roots: Vec::new(),
    };
    Ok((
        put_json(objects, &work).await?,
        put_json(objects, &lock).await?,
    ))
}

pub(crate) fn create_request(
    work_id: WorkId,
    work_revision: ArtifactDescriptor,
    assembly_lock: ArtifactDescriptor,
    display_name: &str,
    idempotency_key: &str,
    secret_policy: InstallationSecretPolicy,
) -> InstallationCreateRequest {
    InstallationCreateRequest {
        work_id,
        work_revision,
        assembly_lock,
        display_name: display_name.to_string(),
        source: AcquisitionRecord {
            kind: AcquisitionKind::WorkBundle,
            source_ref: None,
            provenance_refs: Vec::new(),
            update_channel: None,
        },
        state_bindings: Vec::new(),
        secret_policy,
        idempotency_key: idempotency_key.to_string(),
        authority: None,
    }
}

async fn call(
    runtime: &Runtime<InMemoryEventStore>,
    method: &str,
    params: impl Serialize,
) -> anyhow::Result<Value> {
    runtime
        .call_protocol(
            &ProtocolContext::host_dev("installation-conformance"),
            method,
            serde_json::to_value(params)?,
        )
        .await
        .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message))
}

async fn create_default(
    fixture: &InstallationFixture,
    key: &str,
) -> anyhow::Result<InstallationMutationResult> {
    let value = call(
        &fixture.runtime,
        "host.installation.create",
        create_request(
            fixture.work_id.clone(),
            fixture.work_revision.clone(),
            fixture.assembly_lock.clone(),
            "Installation",
            key,
            InstallationSecretPolicy::default(),
        ),
    )
    .await?;
    Ok(serde_json::from_value(value)?)
}

pub(crate) async fn installation_crud_uses_service_registry() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let created = create_default(&fixture, "crud-create").await?;
    uuid::Uuid::parse_str(created.installation.record.installation_id.as_str())?;

    let listed = call(
        &fixture.runtime,
        "host.installation.list",
        plurora_runtime::InstallationListRequest::default(),
    )
    .await?;
    anyhow::ensure!(listed.as_array().is_some_and(|items| items.len() == 1));

    let got = call(
        &fixture.runtime,
        "host.installation.get",
        plurora_runtime::InstallationGetRequest {
            installation_id: created.installation.record.installation_id.clone(),
        },
    )
    .await?;
    anyhow::ensure!(got == serde_json::to_value(&created.installation)?);

    let (work_revision, assembly_lock) = put_work_pair(&fixture.objects, "two").await?;
    let updated: InstallationMutationResult = serde_json::from_value(
        call(
            &fixture.runtime,
            "host.installation.update",
            InstallationUpdateRequest {
                installation_id: created.installation.record.installation_id.clone(),
                expected_revision: created.installation.revision,
                work_revision: work_revision.clone(),
                assembly_lock: assembly_lock.clone(),
                display_name: Some("Updated installation".to_string()),
                source: None,
                state_bindings: None,
                secret_policy: None,
                state_action: InstallationStateAction::Preserve,
                idempotency_key: "crud-update".to_string(),
                authority: None,
            },
        )
        .await?,
    )?;
    let diff = updated.diff.as_ref().expect("update must publish a diff");
    anyhow::ensure!(
        diff.work_revision_changed && diff.assembly_lock_changed && diff.display_name_changed
    );

    let removed: InstallationMutationResult = serde_json::from_value(
        call(
            &fixture.runtime,
            "host.installation.remove",
            InstallationRemoveRequest {
                installation_id: updated.installation.record.installation_id.clone(),
                expected_revision: updated.installation.revision,
                state_disposition: StateDisposition::Keep,
                idempotency_key: "crud-remove".to_string(),
                authority: None,
            },
        )
        .await?,
    )?;
    anyhow::ensure!(removed.installation.record.status == InstallationStatus::Removed);
    Ok(())
}

pub(crate) async fn installation_idempotency_replay_and_conflict() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let request = create_request(
        fixture.work_id.clone(),
        fixture.work_revision.clone(),
        fixture.assembly_lock.clone(),
        "Idempotent",
        "same-create-key",
        InstallationSecretPolicy::default(),
    );
    let first: InstallationMutationResult = serde_json::from_value(
        call(&fixture.runtime, "host.installation.create", &request).await?,
    )?;
    let replay: InstallationMutationResult = serde_json::from_value(
        call(&fixture.runtime, "host.installation.create", &request).await?,
    )?;
    anyhow::ensure!(!first.idempotent && replay.idempotent);
    anyhow::ensure!(first.installation == replay.installation);

    let mut conflicting = request;
    conflicting.display_name = "Different fingerprint".to_string();
    let error = fixture
        .runtime
        .call_protocol(
            &ProtocolContext::host_dev("installation-conformance"),
            "host.installation.create",
            serde_json::to_value(conflicting)?,
        )
        .await
        .expect_err("same key with a different fingerprint must conflict");
    anyhow::ensure!(error.message.contains("idempotency_conflict"));
    Ok(())
}

pub(crate) async fn installation_stale_revision_is_rejected() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let created = create_default(&fixture, "stale-create").await?;
    let (work_revision, assembly_lock) = put_work_pair(&fixture.objects, "stale").await?;
    let error = fixture
        .runtime
        .call_protocol(
            &ProtocolContext::host_dev("installation-conformance"),
            "host.installation.update",
            serde_json::to_value(InstallationUpdateRequest {
                installation_id: created.installation.record.installation_id,
                expected_revision: 0,
                work_revision: work_revision.clone(),
                assembly_lock: assembly_lock.clone(),
                display_name: None,
                source: None,
                state_bindings: None,
                secret_policy: None,
                state_action: InstallationStateAction::Preserve,
                idempotency_key: "stale-update".to_string(),
                authority: None,
            })?,
        )
        .await
        .expect_err("a stale update must fail");
    anyhow::ensure!(error.message.contains("revision_conflict"));
    Ok(())
}

pub(crate) async fn installation_state_migration_is_explicit() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let (current_work, current_lock) = put_stateful_work_pair(
        &fixture.objects,
        "migration-current",
        "v1",
        StatePortability::Portable,
        StateScope::Installation,
        false,
    )
    .await?;
    let created: InstallationMutationResult = serde_json::from_value(
        call(
            &fixture.runtime,
            "host.installation.create",
            create_request(
                WorkId::parse("conformance/migration-current")?,
                current_work,
                current_lock,
                "Stateful installation",
                "migration-create",
                InstallationSecretPolicy::default(),
            ),
        )
        .await?,
    )?;
    let state = fixture
        .data
        .path()
        .join("installations")
        .join(created.installation.record.installation_id.as_str())
        .join("state");
    fs::write(state.join("save.bin"), b"durable-state")?;
    let (work_revision, assembly_lock) = put_stateful_work_pair(
        &fixture.objects,
        "migration-candidate",
        "v2",
        StatePortability::OpaqueExportable,
        StateScope::Installation,
        true,
    )
    .await?;
    let error = fixture
        .runtime
        .call_protocol(
            &ProtocolContext::host_dev("installation-conformance"),
            "host.installation.update",
            serde_json::to_value(InstallationUpdateRequest {
                installation_id: created.installation.record.installation_id.clone(),
                expected_revision: created.installation.revision,
                work_revision: work_revision.clone(),
                assembly_lock: assembly_lock.clone(),
                display_name: None,
                source: None,
                state_bindings: None,
                secret_policy: None,
                state_action: InstallationStateAction::Preserve,
                idempotency_key: "migration-update".to_string(),
                authority: None,
            })?,
        )
        .await
        .expect_err("changing a lock with durable state must require migration");
    anyhow::ensure!(error.message.contains("state_migration_required"));
    anyhow::ensure!(fs::read(state.join("save.bin"))? == b"durable-state");

    let replacement = InstallationStateSnapshot {
        schema: plurora_runtime::INSTALLATION_STATE_SNAPSHOT_SCHEMA.to_string(),
        entries: vec![InstallationStateSnapshotEntry {
            path: "save.bin".to_string(),
            bytes: b"migrated-state".to_vec(),
        }],
    };
    let replacement = put_raw(
        &fixture.objects,
        INSTALLATION_STATE_SNAPSHOT_TYPE_URI,
        INSTALLATION_STATE_SNAPSHOT_MEDIA_TYPE,
        replacement.canonical_bytes()?,
        Vec::new(),
    )
    .await?;
    let migrated: InstallationMutationResult = serde_json::from_value(
        call(
            &fixture.runtime,
            "host.installation.update",
            InstallationUpdateRequest {
                installation_id: created.installation.record.installation_id,
                expected_revision: created.installation.revision,
                work_revision,
                assembly_lock,
                display_name: None,
                source: None,
                state_bindings: None,
                secret_policy: None,
                state_action: InstallationStateAction::Replace {
                    replacement_snapshot: replacement,
                },
                idempotency_key: "migration-replace".to_string(),
                authority: None,
            },
        )
        .await?,
    )?;
    anyhow::ensure!(migrated.diff.as_ref().is_some_and(|diff| {
        diff.state_slots.iter().any(|slot| {
            slot.required_action == plurora_runtime::InstallationStateSlotRequirement::Replace
        })
    }));
    anyhow::ensure!(fs::read(state.join("save.bin"))? == b"migrated-state");
    Ok(())
}

pub(crate) async fn installation_remove_keep_and_delete_are_distinct() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let keep = create_default(&fixture, "keep-create").await?;
    let keep_state = fixture
        .data
        .path()
        .join("installations")
        .join(keep.installation.record.installation_id.as_str())
        .join("state");
    fs::write(keep_state.join("save.bin"), b"keep")?;
    call(
        &fixture.runtime,
        "host.installation.remove",
        InstallationRemoveRequest {
            installation_id: keep.installation.record.installation_id,
            expected_revision: keep.installation.revision,
            state_disposition: StateDisposition::Keep,
            idempotency_key: "keep-remove".to_string(),
            authority: None,
        },
    )
    .await?;
    anyhow::ensure!(fs::read(keep_state.join("save.bin"))? == b"keep");

    let delete = create_default(&fixture, "delete-create").await?;
    let delete_state = fixture
        .data
        .path()
        .join("installations")
        .join(delete.installation.record.installation_id.as_str())
        .join("state");
    fs::write(delete_state.join("save.bin"), b"delete")?;
    call(
        &fixture.runtime,
        "host.installation.remove",
        InstallationRemoveRequest {
            installation_id: delete.installation.record.installation_id,
            expected_revision: delete.installation.revision,
            state_disposition: StateDisposition::Delete,
            idempotency_key: "delete-remove".to_string(),
            authority: None,
        },
    )
    .await?;
    anyhow::ensure!(fs::read_dir(delete_state)?.next().is_none());
    Ok(())
}

pub(crate) async fn installation_journal_rehydrates_after_restart() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let created = create_default(&fixture, "restart-create").await?;
    let projection = fixture
        .data
        .path()
        .join("installations")
        .join(created.installation.record.installation_id.as_str())
        .join("installation.json");
    fs::write(&projection, b"projection is not authority")?;

    let restarted = InstallationRegistry::persistent(
        fixture.store.clone(),
        fixture.objects.clone(),
        fixture.data.path(),
    )?;
    anyhow::ensure!(restarted.hydrate().await? == 1);
    let runtime = Runtime::new(
        fixture.store,
        RuntimeConfig {
            object_store: fixture.objects,
            installation_control: restarted,
            ..RuntimeConfig::default()
        },
    );
    let restored = call(
        &runtime,
        "host.installation.get",
        plurora_runtime::InstallationGetRequest {
            installation_id: created.installation.record.installation_id.clone(),
        },
    )
    .await?;
    anyhow::ensure!(restored == serde_json::to_value(created.installation)?);
    Ok(())
}

pub(crate) async fn installation_authority_is_exact() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let first = create_default(&fixture, "authority-first").await?;
    let second = create_default(&fixture, "authority-second").await?;
    let context = ProtocolContext::host_device(
        "grant-installation-one",
        vec!["observe".to_string()],
        vec![ProtocolResourceSelector {
            owner: "host".to_string(),
            kind: "installation".to_string(),
            id: Some(first.installation.record.installation_id.to_string()),
        }],
        Vec::new(),
        "conformance",
    );
    fixture
        .runtime
        .call_protocol(
            &context,
            "host.installation.get",
            json!({"installation_id": first.installation.record.installation_id}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let denied = fixture
        .runtime
        .call_protocol(
            &context,
            "host.installation.get",
            json!({"installation_id": second.installation.record.installation_id}),
        )
        .await
        .expect_err("authority for one installation must not match another");
    anyhow::ensure!(denied.code == "runtime/error/permission_denied");

    let listed = fixture
        .runtime
        .call_protocol(&context, "host.installation.list", json!({}))
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(listed.as_array().is_some_and(|items| items.len() == 1));

    let manage_without_work = ProtocolContext::host_device(
        "grant-manage-no-work",
        vec!["installation.manage".to_string()],
        vec![ProtocolResourceSelector {
            owner: "host".to_string(),
            kind: "installation".to_string(),
            id: Some(first.installation.record.installation_id.to_string()),
        }],
        Vec::new(),
        "conformance",
    );
    let denied = fixture
        .runtime
        .call_protocol(
            &manage_without_work,
            "host.installation.create",
            serde_json::to_value(create_request(
                fixture.work_id,
                fixture.work_revision,
                fixture.assembly_lock,
                "Denied",
                "authority-create-denied",
                InstallationSecretPolicy::default(),
            ))?,
        )
        .await
        .expect_err("create requires exact Work authority");
    anyhow::ensure!(denied.code == "runtime/error/permission_denied");
    Ok(())
}

pub(crate) async fn installation_events_match_public_lifecycle() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let raw_key = "raw-event-idempotency-sentinel";
    let created = create_default(&fixture, raw_key).await?;
    let (work_revision, assembly_lock) = put_work_pair(&fixture.objects, "events").await?;
    let updated: InstallationMutationResult = serde_json::from_value(
        call(
            &fixture.runtime,
            "host.installation.update",
            InstallationUpdateRequest {
                installation_id: created.installation.record.installation_id,
                expected_revision: created.installation.revision,
                work_revision,
                assembly_lock,
                display_name: None,
                source: None,
                state_bindings: None,
                secret_policy: None,
                state_action: InstallationStateAction::Preserve,
                idempotency_key: "event-update".to_string(),
                authority: None,
            },
        )
        .await?,
    )?;
    call(
        &fixture.runtime,
        "host.installation.remove",
        InstallationRemoveRequest {
            installation_id: updated.installation.record.installation_id,
            expected_revision: updated.installation.revision,
            state_disposition: StateDisposition::Keep,
            idempotency_key: "event-remove".to_string(),
            authority: None,
        },
    )
    .await?;

    let events = fixture
        .store
        .list_session(&"host_installations".to_string())
        .await?;
    for kind in [
        plurora_core::INSTALLATION_CREATED,
        plurora_core::INSTALLATION_UPDATED,
        plurora_core::INSTALLATION_REMOVED,
    ] {
        anyhow::ensure!(events.iter().filter(|event| event.kind == kind).count() == 1);
    }
    for (kind, expected) in [
        (
            plurora_core::INSTALLATION_CREATED,
            BTreeSet::from(["claim", "view"]),
        ),
        (
            plurora_core::INSTALLATION_UPDATED,
            BTreeSet::from(["claim", "operation_id", "previous_revision", "view"]),
        ),
        (
            plurora_core::INSTALLATION_REMOVED,
            BTreeSet::from(["claim", "operation_id", "previous_revision", "view"]),
        ),
    ] {
        let payload = events
            .iter()
            .find(|event| event.kind == kind)
            .and_then(|event| event.payload.as_object())
            .with_context(|| format!("{kind} payload must be an object"))?;
        anyhow::ensure!(payload.keys().map(String::as_str).collect::<BTreeSet<_>>() == expected);
        let claim = payload["claim"]
            .as_object()
            .with_context(|| format!("{kind} claim must be an object"))?;
        anyhow::ensure!(
            claim.keys().map(String::as_str).collect::<BTreeSet<_>>()
                == BTreeSet::from(["fingerprint", "key_hash", "result"])
        );
    }
    let public_payloads = events
        .iter()
        .filter(|event| {
            [
                plurora_core::INSTALLATION_CREATED,
                plurora_core::INSTALLATION_UPDATED,
                plurora_core::INSTALLATION_REMOVED,
            ]
            .contains(&event.kind.as_str())
        })
        .map(|event| event.payload.clone())
        .collect::<Vec<_>>();
    let text = serde_json::to_string(&public_payloads)?;
    anyhow::ensure!(!text.contains(raw_key));
    anyhow::ensure!(!text.contains("secrets.dat"));
    Ok(())
}

pub(crate) async fn retired_host_methods_are_not_aliases() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let retired_owner = ["host", "project"].join(".");
    for suffix in ["list", "get", "start", "stop", "status"] {
        let method = format!("{retired_owner}.{suffix}");
        let error = fixture
            .runtime
            .call_protocol(
                &ProtocolContext::host_dev("installation-conformance"),
                &method,
                json!({}),
            )
            .await
            .expect_err("a retired method must not resolve");
        anyhow::ensure!(error.code == "runtime/error/invalid_request");
    }
    Ok(())
}

pub(crate) async fn surface_resolve_via_dev_path() -> anyhow::Result<()> {
    let store = Arc::new(InMemoryEventStore::default());
    let mut config = RuntimeConfig::default();
    config.surface_dev_paths.insert(
        "ydltavern".to_string(),
        "/tmp/ydltavern-surface-dist".to_string(),
    );
    let runtime = Runtime::new(store, config);
    let value = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.surface.bundle.resolve",
            json!({"surface_id":"ydltavern/play"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(value["source"] == json!("dev_path"));
    anyhow::ensure!(value["package_id"].is_null());
    anyhow::ensure!(value["export_name"] == json!("mountTavernPlaySurface"));
    Ok(())
}

pub(crate) async fn surface_resolve_unknown_fails() -> anyhow::Result<()> {
    let (_store, runtime) = super::fixtures::runtime();
    let error = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.surface.bundle.resolve",
            json!({"surface_id":"unknown/surface"}),
        )
        .await
        .expect_err("unknown surface should fail closed");
    anyhow::ensure!(error.message.contains("surface_not_found"));
    Ok(())
}

pub(crate) async fn surface_resolve_admin_principal_required() -> anyhow::Result<()> {
    let (_store, runtime) = super::fixtures::runtime();
    let error = runtime
        .call_protocol(
            &ProtocolContext::package("example/not-admin", "conformance"),
            "host.surface.bundle.resolve",
            json!({"surface_id":"ydltavern/play"}),
        )
        .await
        .expect_err("package principal should be denied");
    anyhow::ensure!(error.code == "runtime/error/permission_denied");
    Ok(())
}
