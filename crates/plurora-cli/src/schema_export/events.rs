use plurora_core::*;
use plurora_runtime::*;
use serde_json::{json, Value};

use super::defs::*;
use super::SCHEMA;

pub(crate) fn event_schema(kind: &str, payload: Value) -> Value {
    json!({
        "$schema": SCHEMA,
        "$id": format!("urn:plurora:schema:event:{kind}:v1"),
        "title": kind,
        "description": format!("Payload schema for event kind {kind}."),
        "type": "object",
        "properties": { "kind": { "const": kind }, "payload": { "$ref": "#/$defs/Payload" } },
        "$defs": { "Payload": payload }
    })
}

fn with_optional_receipt(mut payload: Value) -> Value {
    let properties = payload
        .as_object_mut()
        .expect("event payload schema is an object")
        .entry("properties")
        .or_insert_with(|| json!({}));
    properties
        .as_object_mut()
        .expect("event payload properties is an object")
        .insert(
            "receipt".to_string(),
            json!({
                "anyOf": [schema_value::<ArtifactDescriptor>(), {"type": "null"}],
                "default": null,
            }),
        );
    payload
}

pub(crate) fn event_schemas() -> Vec<(&'static str, Value)> {
    vec![
        (EVENT_SESSION_OPENED, json!({"type":"object"})),
        (EVENT_SESSION_CLOSED, json!({"type":"object"})),
        (EVENT_SESSION_FORKED, schema_value::<BranchRecord>()),
        (
            EVENT_PACKAGE_LOADED,
            schema_value::<PackageLifecyclePayload>(),
        ),
        (
            EVENT_PACKAGE_LOADING,
            schema_value::<PackageLifecyclePayload>(),
        ),
        (
            EVENT_PACKAGE_STARTING,
            schema_value::<PackageLifecyclePayload>(),
        ),
        (
            EVENT_PACKAGE_READY,
            schema_value::<PackageLifecyclePayload>(),
        ),
        (
            EVENT_PACKAGE_STOPPING,
            schema_value::<PackageLifecyclePayload>(),
        ),
        (
            EVENT_PACKAGE_STOPPED,
            schema_value::<PackageLifecyclePayload>(),
        ),
        (
            EVENT_PACKAGE_UNLOADED,
            schema_value::<PackageLifecyclePayload>(),
        ),
        (
            EVENT_PACKAGE_DEGRADED,
            schema_value::<PackageLifecyclePayload>(),
        ),
        (EVENT_PACKAGE_LOG, schema_value::<SubprocessLogLine>()),
        (
            INSTALLATION_CREATED,
            schema_value::<InstallationCreatedPayloadSchema>(),
        ),
        (
            INSTALLATION_UPDATED,
            schema_value::<InstallationUpdatedPayloadSchema>(),
        ),
        (
            INSTALLATION_REMOVED,
            schema_value::<InstallationRemovedPayloadSchema>(),
        ),
        (EVENT_ASSET_PUT, schema_value::<AssetRecord>()),
        (
            EVENT_PROJECTION_UPDATED,
            schema_value::<ProjectionDefinition>(),
        ),
        (EVENT_PROPOSAL_CREATED, schema_value::<ProposalRecord>()),
        (EVENT_PROPOSAL_APPROVED, schema_value::<ProposalRecord>()),
        (EVENT_PROPOSAL_REJECTED, schema_value::<ProposalRecord>()),
        (EVENT_PROPOSAL_APPLIED, schema_value::<ProposalRecord>()),
        (EVENT_PROPOSAL_FAILED, schema_value::<ProposalRecord>()),
        (EVENT_CAPABILITY_INVOKED, json!({"type":"object"})),
        (
            EVENT_CAPABILITY_COMPLETED,
            schema_value::<CapabilityInvocationResult>(),
        ),
        (
            EVENT_CAPABILITY_FAILED,
            with_optional_receipt(json!({"type":"object"})),
        ),
        (
            EVENT_PERMISSION_DENIED,
            json!({"type":"object","properties":{"package_id":{"type":"string"},"operation":{"type":"string"}}}),
        ),
        (
            EVENT_PERMISSION_GRANTED,
            schema_value::<PermissionGrantRecord>(),
        ),
        (
            EVENT_PERMISSION_REVOKED,
            schema_value::<PermissionGrantRecord>(),
        ),
        (EVENT_ERROR, schema_value::<ErrorShape>()),
        (
            EVENT_OUTBOUND_REQUEST,
            schema_value::<OutboundAuditRecord>(),
        ),
        (
            EVENT_OUTBOUND_DENIED,
            with_optional_receipt(schema_value::<OutboundAuditRecord>()),
        ),
        (
            EVENT_OUTBOUND_EXECUTE_COMPLETED,
            with_optional_receipt(json!({"type":"object"})),
        ),
        (
            EVENT_OUTBOUND_STREAM_COMPLETED,
            with_optional_receipt(schema_value::<OutboundStreamSummary>()),
        ),
        (EVENT_STREAM_STARTED, json!({"type":"object"})),
        (EVENT_STREAM_CHUNK, schema_value::<StreamFrameEnvelope>()),
        (EVENT_STREAM_PROGRESS, schema_value::<StreamFrameEnvelope>()),
        (
            EVENT_STREAM_ENDED,
            with_optional_receipt(schema_value::<StreamFrameEnvelope>()),
        ),
        (
            EVENT_STREAM_ERROR,
            with_optional_receipt(schema_value::<StreamFrameEnvelope>()),
        ),
        (
            EVENT_STREAM_CANCELLED,
            with_optional_receipt(schema_value::<StreamFrameEnvelope>()),
        ),
        (
            EVENT_STREAM_TIMEOUT,
            with_optional_receipt(schema_value::<StreamFrameEnvelope>()),
        ),
        (EVENT_OUTBOUND_WEBSOCKET_OPENED, json!({"type":"object"})),
        (EVENT_OUTBOUND_WEBSOCKET_FRAME, json!({"type":"object"})),
        (EVENT_OUTBOUND_WEBSOCKET_ERROR, json!({"type":"object"})),
        (
            EVENT_OUTBOUND_WEBSOCKET_COMPLETED,
            with_optional_receipt(json!({"type":"object"})),
        ),
        (EVENT_EXEC_REQUEST, json!({"type":"object"})),
        (
            EVENT_EXEC_DENIED,
            with_optional_receipt(json!({"type":"object"})),
        ),
        (EVENT_EXEC_STARTED, json!({"type":"object"})),
        (
            EVENT_EXEC_STOPPED,
            with_optional_receipt(json!({"type":"object"})),
        ),
        (
            EVENT_EXEC_COMPLETED,
            with_optional_receipt(json!({"type":"object"})),
        ),
        (
            EVENT_EXEC_FAILED,
            with_optional_receipt(json!({"type":"object"})),
        ),
        (EVENT_PORT_LEASED, json!({"type":"object"})),
        (EVENT_PORT_RELEASED, json!({"type":"object"})),
        (EVENT_PORT_DENIED, json!({"type":"object"})),
        (EVENT_PROXY_REGISTERED, json!({"type":"object"})),
        (EVENT_PROXY_UNREGISTERED, json!({"type":"object"})),
        (EVENT_PROXY_DENIED, json!({"type":"object"})),
        (
            EVENT_DEPLOYMENT_RECONCILED,
            schema_value::<DeploymentReconcileSummary>(),
        ),
        (
            EVENT_DEPLOYMENT_HEALTH,
            schema_value::<DeploymentHealthEventPayload>(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use jsonschema::JSONSchema;
    use plurora_core::{ArtifactDescriptor, INSTALLATION_REMOVED, INSTALLATION_UPDATED};
    use plurora_runtime::{
        EventStore, InMemoryEventStore, InMemoryObjectStore, InstallationCreateRequest,
        InstallationMutationResult, InstallationRemoveRequest, InstallationStateAction,
        InstallationUpdateRequest, ObjectStore, ProtocolContext, Runtime, RuntimeConfig,
        StateDisposition,
    };
    use plurora_service::InstallationRegistry;
    use plurora_work::{
        AcquisitionKind, AcquisitionRecord, ArtifactModel, AssemblyId, AssemblyLock,
        AssemblyRevision, WorkId, WorkRevision,
    };
    use serde_json::{json, Value};

    use crate::schema_export::defs::normalize_schema;

    use super::{event_schema, event_schemas};

    async fn put_model<T: ArtifactModel>(
        objects: &InMemoryObjectStore,
        value: &T,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let bytes = value.canonical_bytes()?;
        let descriptor = value.artifact_descriptor()?;
        let info = objects.put(bytes.into()).await?;
        anyhow::ensure!(
            info.digest == descriptor.digest && info.size_bytes == descriptor.size_bytes,
            "object store changed the canonical model identity"
        );
        Ok(descriptor)
    }

    async fn put_work_pair(
        objects: &InMemoryObjectStore,
        marker: &str,
    ) -> anyhow::Result<(ArtifactDescriptor, ArtifactDescriptor)> {
        let assembly = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse(format!("schema-tests/{marker}-assembly"))?,
            nodes: Vec::new(),
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let assembly = put_model(objects, &assembly).await?;
        let work = WorkRevision {
            schema: WorkRevision::SCHEMA.to_string(),
            work_id: WorkId::parse(format!("schema-tests/{marker}"))?,
            title: format!("Schema test {marker}"),
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
            put_model(objects, &work).await?,
            put_model(objects, &lock).await?,
        ))
    }

    #[tokio::test]
    async fn installation_terminal_schemas_accept_real_required_null_operation_ids(
    ) -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(InMemoryObjectStore::default());
        let registry = InstallationRegistry::ephemeral(store.clone(), objects.clone())?;
        let runtime = Runtime::new(
            store.clone(),
            RuntimeConfig {
                object_store: objects.clone(),
                installation_control: registry,
                ..RuntimeConfig::default()
            },
        );
        let (work_one, lock_one) = put_work_pair(objects.as_ref(), "one").await?;
        let created: InstallationMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("schema-event-test"),
                    "host.installation.create",
                    serde_json::to_value(InstallationCreateRequest {
                        work_id: WorkId::parse("schema-tests/one")?,
                        work_revision: work_one,
                        assembly_lock: lock_one,
                        display_name: "Schema test installation".to_string(),
                        source: AcquisitionRecord {
                            kind: AcquisitionKind::WorkBundle,
                            source_ref: None,
                            provenance_refs: Vec::new(),
                            update_channel: None,
                        },
                        state_bindings: Vec::new(),
                        secret_policy: Default::default(),
                        idempotency_key: "schema-create".to_string(),
                        authority: None,
                    })?,
                )
                .await
                .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message))?,
        )?;
        let created = created.installation;
        let (work_two, lock_two) = put_work_pair(objects.as_ref(), "two").await?;
        let updated: InstallationMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("schema-event-test"),
                    "host.installation.update",
                    serde_json::to_value(InstallationUpdateRequest {
                        installation_id: created.record.installation_id.clone(),
                        expected_revision: created.revision,
                        work_revision: work_two,
                        assembly_lock: lock_two,
                        display_name: None,
                        source: None,
                        state_bindings: None,
                        secret_policy: None,
                        state_action: InstallationStateAction::Preserve,
                        idempotency_key: "schema-update".to_string(),
                        authority: None,
                    })?,
                )
                .await
                .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message))?,
        )?;
        let updated = updated.installation;
        runtime
            .call_protocol(
                &ProtocolContext::host_dev("schema-event-test"),
                "host.installation.remove",
                serde_json::to_value(InstallationRemoveRequest {
                    installation_id: updated.record.installation_id,
                    expected_revision: updated.revision,
                    state_disposition: StateDisposition::Keep,
                    idempotency_key: "schema-remove".to_string(),
                    authority: None,
                })?,
            )
            .await
            .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message))?;

        let payload_schemas = event_schemas().into_iter().collect::<BTreeMap<_, _>>();
        let emitted = store
            .list_session_range(&"host_installations".to_string(), None, None)
            .await?;
        for kind in [INSTALLATION_UPDATED, INSTALLATION_REMOVED] {
            let event = emitted
                .iter()
                .find(|event| event.kind == kind)
                .unwrap_or_else(|| panic!("Installation registry did not emit {kind}"));
            assert_eq!(event.payload.get("operation_id"), Some(&Value::Null));

            let mut schema = event_schema(
                kind,
                payload_schemas
                    .get(kind)
                    .unwrap_or_else(|| panic!("missing exported payload schema for {kind}"))
                    .clone(),
            );
            normalize_schema(&mut schema);
            let compiled = JSONSchema::compile(&schema)
                .map_err(|error| anyhow::anyhow!("compile {kind} schema: {error}"))?;
            let instance = json!({"kind": kind, "payload": event.payload});
            if let Err(errors) = compiled.validate(&instance) {
                let details = errors.map(|error| error.to_string()).collect::<Vec<_>>();
                anyhow::bail!("real {kind} event did not validate: {details:?}");
            }

            let mut missing_operation_id = instance;
            missing_operation_id["payload"]
                .as_object_mut()
                .expect("emitted Installation payload is an object")
                .remove("operation_id");
            anyhow::ensure!(
                compiled.validate(&missing_operation_id).is_err(),
                "{kind} operation_id must remain required even when nullable"
            );
        }
        Ok(())
    }
}
