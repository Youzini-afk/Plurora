use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use plurora_core::{
    ArtifactDescriptor, COMPONENT_DESCRIPTOR_TYPE_URI, EVENT_REALIZATION_RECONCILED,
    PLATFORM_EVENT_KINDS,
};
use plurora_runtime::{
    InMemoryEventStore, InMemoryObjectStore, InstallationCreateRequest, InstallationListRequest,
    InstallationMutationResult, InstallationRemoveRequest, InstallationUpdateRequest,
    InstallationView, InstallationWorkSummary, ObjectStore, ProtocolContext, RealizationEffectKind,
    Runtime, RuntimeConfig,
};
use plurora_work::{
    AcquisitionKind, AcquisitionRecord, ArtifactModel, AssemblyLock, InstallationId,
    InstallationRecord, InstallationSecretPolicy, InstallationStatus, NodeId, NodeLock,
    OperationalIntent, ReplicaPolicy, ResourceRequirements, RestartPolicy, RightDisposition,
    RightsDeclaration, WorkId, WorkRevision, WorkloadImports, WorkloadIntent,
    ASSEMBLY_LOCK_TYPE_URI, ASSEMBLY_REVISION_TYPE_URI, OPERATIONAL_INTENT_TYPE_URI,
    WORK_REVISION_TYPE_URI,
};

use super::*;

#[derive(Clone)]
struct FakeInstallations {
    artifacts: Arc<tokio::sync::RwLock<RunInstallationArtifacts>>,
}

#[async_trait]
impl InstallationControl for FakeInstallations {
    async fn list(
        &self,
        _request: InstallationListRequest,
    ) -> anyhow::Result<Vec<InstallationView>> {
        Ok(vec![self.artifacts.read().await.installation.clone()])
    }

    async fn get(
        &self,
        installation_id: &InstallationId,
    ) -> anyhow::Result<Option<InstallationView>> {
        let view = self.artifacts.read().await.installation.clone();
        Ok((view.record.installation_id == *installation_id).then_some(view))
    }

    async fn create(
        &self,
        _request: InstallationCreateRequest,
    ) -> anyhow::Result<InstallationMutationResult> {
        anyhow::bail!("not used")
    }

    async fn update(
        &self,
        _request: InstallationUpdateRequest,
    ) -> anyhow::Result<InstallationMutationResult> {
        anyhow::bail!("not used")
    }

    async fn remove(
        &self,
        _request: InstallationRemoveRequest,
    ) -> anyhow::Result<InstallationMutationResult> {
        anyhow::bail!("not used")
    }

    fn installation_secret_store_path(
        &self,
        _installation_id: &InstallationId,
    ) -> anyhow::Result<PathBuf> {
        anyhow::bail!("not used")
    }

    async fn inspect_current_ready_for_run(
        &self,
        installation_id: &InstallationId,
    ) -> anyhow::Result<RunInstallationArtifacts> {
        let artifacts = self.artifacts.read().await.clone();
        ensure!(
            artifacts.installation.record.installation_id == *installation_id,
            "not found"
        );
        Ok(artifacts)
    }
}

#[derive(Default)]
struct FakeDriver {
    applies: AtomicUsize,
    stops: AtomicUsize,
    next_apply_outcome_unknown: AtomicBool,
    next_apply_partial: AtomicBool,
    next_stop_failure: AtomicBool,
}

#[async_trait]
impl RealizationExecutionDriver for FakeDriver {
    async fn apply(
        &self,
        realization: &RealizationRevision,
        target_id: &str,
        _plan: &RealizationPlan,
        _backends: &[RealizationBackendSelection],
        authority: &RealizationMutationAuthority,
        subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<RealizationExecution> {
        authority.refresh_current_for(subject).await?;
        self.applies.fetch_add(1, Ordering::SeqCst);
        if self
            .next_apply_outcome_unknown
            .swap(false, Ordering::SeqCst)
        {
            return Err(plurora_runtime::managed_target_workload_outcome_unknown(
                "fake target effect",
            ));
        }
        if self.next_apply_partial.swap(false, Ordering::SeqCst) {
            return Err(partial_execution_error(
                "recovery_required",
                vec![RealizedResource {
                    resource_id: "api".to_string(),
                    resource_type: "managed_workload".to_string(),
                    target_id: target_id.to_string(),
                    backend_id: "fake-partial-operation".to_string(),
                    properties: BTreeMap::new(),
                    receipt_ref: None,
                }],
                vec![receipt(
                    realization,
                    target_id,
                    RealizationEffectKind::Build,
                )],
            ));
        }
        Ok(RealizationExecution {
            resources: vec![RealizedResource {
                resource_id: "api".to_string(),
                resource_type: "managed_workload".to_string(),
                target_id: target_id.to_string(),
                backend_id: "fake-operation".to_string(),
                properties: BTreeMap::new(),
                receipt_ref: None,
            }],
            receipts: vec![receipt(
                realization,
                target_id,
                RealizationEffectKind::Launch,
            )],
        })
    }

    async fn stop(
        &self,
        realization: &RealizationRevision,
        target_id: &str,
        _authority: &RealizationMutationAuthority,
        _subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<Vec<RealizationEffectReceipt>> {
        self.stops.fetch_add(1, Ordering::SeqCst);
        if self.next_stop_failure.swap(false, Ordering::SeqCst) {
            anyhow::bail!("target_unsatisfied: fake stop failed before completion");
        }
        Ok(vec![receipt(
            realization,
            target_id,
            RealizationEffectKind::Stop,
        )])
    }

    async fn observe(
        &self,
        realization: &RealizationRevision,
        _target_id: &str,
    ) -> anyhow::Result<RealizationObservation> {
        Ok(RealizationObservation {
            status: RealizationStatus::Active,
            resources: realization.actual_resources.clone(),
            receipts: Vec::new(),
            reason_code: None,
        })
    }
}

struct Fixture {
    runtime: Runtime<InMemoryEventStore>,
    registry: Arc<RealizationRegistry>,
    control: Arc<InMemoryEventStore>,
    public: Arc<InMemoryEventStore>,
    installations: Arc<FakeInstallations>,
    driver: Arc<FakeDriver>,
    installation_id: InstallationId,
}

async fn fixture() -> anyhow::Result<Fixture> {
    let control = Arc::new(InMemoryEventStore::default());
    let public = Arc::new(InMemoryEventStore::default());
    let objects = Arc::new(InMemoryObjectStore::default());
    let intent = OperationalIntent {
        schema: OperationalIntent::SCHEMA.to_string(),
        workloads: vec![WorkloadIntent {
            workload_id: "api".to_string(),
            node_id: NodeId::parse("server")?,
            execution_classes: vec!["oci-container.v1".to_string()],
            resources: ResourceRequirements::default(),
            imports: WorkloadImports::default(),
            replicas: ReplicaPolicy { min: 1, max: 1 },
            restart_policy: RestartPolicy::OnFailure,
            health_port: None,
            annotations: BTreeMap::new(),
        }],
        endpoints: Vec::new(),
        state: Vec::new(),
        placement: Vec::new(),
        update_policy: Default::default(),
        annotations: BTreeMap::new(),
    };
    let intent_bytes = plurora_work::canonical_json_bytes(&intent)?;
    let intent_info = objects.put(intent_bytes.into()).await?;
    let intent_ref = ArtifactDescriptor {
        artifact_type_uri: OPERATIONAL_INTENT_TYPE_URI.to_string(),
        media_type: plurora_work::CANONICAL_JSON_MEDIA_TYPE.to_string(),
        digest: intent_info.digest,
        size_bytes: intent_info.size_bytes,
        references: Vec::new(),
        annotations: BTreeMap::new(),
    };
    let installation_id = InstallationId::new();
    let work_descriptor = descriptor(WORK_REVISION_TYPE_URI, 'a');
    let lock_descriptor = descriptor(ASSEMBLY_LOCK_TYPE_URI, 'b');
    let work = WorkRevision {
        schema: WorkRevision::SCHEMA.to_string(),
        work_id: WorkId::parse("vendor/realized-work")?,
        title: "Realized Work".to_string(),
        description: String::new(),
        assembly: descriptor(ASSEMBLY_REVISION_TYPE_URI, 'c'),
        content_roots: Vec::new(),
        entrypoints: Vec::new(),
        rights: None,
        transparency: None,
        operational_intent: Some(intent_ref),
        annotations: BTreeMap::new(),
    };
    let lock = AssemblyLock {
        schema: AssemblyLock::SCHEMA.to_string(),
        assembly: work.assembly.clone(),
        nodes: vec![NodeLock {
            node_id: NodeId::parse("server")?,
            artifact: descriptor(COMPONENT_DESCRIPTOR_TYPE_URI, 'd'),
            behavior_digest: Some(format!("sha256:{}", "e".repeat(64))),
            trust_class: Some(plurora_core::ComponentTrustClass::IsolatedProcess),
        }],
        bindings: Vec::new(),
        protocol_profiles: Vec::new(),
        content_roots: Vec::new(),
    };
    let now = Utc::now();
    let view = InstallationView {
        record: InstallationRecord {
            schema_version: InstallationRecord::SCHEMA_VERSION,
            installation_id: installation_id.clone(),
            work_revision: work_descriptor,
            assembly_lock: lock_descriptor.clone(),
            display_name: "Realized Work".to_string(),
            source: AcquisitionRecord {
                kind: AcquisitionKind::WorkBundle,
                source_ref: None,
                provenance_refs: Vec::new(),
                update_channel: None,
            },
            state_bindings: Vec::new(),
            secret_policy: InstallationSecretPolicy::default(),
            created_at: now,
            updated_at: now,
            status: InstallationStatus::Ready,
        },
        work_summary: InstallationWorkSummary::from_work_revision(&work),
        revision: 1,
        rollback: None,
    };
    let installations = Arc::new(FakeInstallations {
        artifacts: Arc::new(tokio::sync::RwLock::new(RunInstallationArtifacts {
            installation: view,
            work,
            assemblies: BTreeMap::new(),
            locks: BTreeMap::from([(lock_descriptor.digest.clone(), lock)]),
        })),
    });
    let targets = Arc::new(ExecutionTargetRegistry::default());
    let registry = RealizationRegistry::new(
        control.clone(),
        public.clone(),
        objects.clone(),
        installations.clone(),
        targets.clone(),
    )?;
    let driver = Arc::new(FakeDriver::default());
    registry.install_driver(driver.clone());
    let runtime = Runtime::new(
        public.clone(),
        RuntimeConfig {
            object_store: objects,
            installation_control: installations.clone(),
            realization_control: registry.clone(),
            target_registry: targets,
            ..RuntimeConfig::default()
        },
    );
    Ok(Fixture {
        runtime,
        registry,
        control,
        public,
        installations,
        driver,
        installation_id,
    })
}

fn rights(
    copy_across_hosts: RightDisposition,
    dedicated_server: RightDisposition,
) -> RightsDeclaration {
    RightsDeclaration {
        license_expression: Some("LicenseRef-realization-test".to_string()),
        terms_uri: None,
        install: RightDisposition::Allowed,
        execute: RightDisposition::Allowed,
        backup: RightDisposition::Allowed,
        export_state: RightDisposition::Denied,
        copy_across_hosts,
        redistribute_artifacts: RightDisposition::Denied,
        modify: RightDisposition::Denied,
        derive: RightDisposition::Denied,
        modding: RightDisposition::Denied,
        dedicated_server,
        entitlement_requirements: Vec::new(),
        evidence_refs: Vec::new(),
    }
}

async fn install_rights(fixture: &Fixture, declaration: &RightsDeclaration) -> anyhow::Result<()> {
    let descriptor = declaration.artifact_descriptor()?;
    fixture
        .runtime
        .object_store()
        .put(declaration.canonical_bytes()?.into())
        .await?;
    let mut artifacts = fixture.installations.artifacts.write().await;
    artifacts.work.rights = Some(descriptor);
    artifacts.installation.work_summary =
        InstallationWorkSummary::from_work_revision(&artifacts.work);
    Ok(())
}

#[tokio::test]
async fn plan_is_effect_free_idempotent_and_publicly_relayed() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let request = plan_request(&fixture.installation_id, "plan-key");
    let first: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &ProtocolContext::host_dev("realization-test"),
                "host.realization.plan",
                serde_json::to_value(&request)?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert!(first.gaps.is_empty());
    assert!(!first.replayed);
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 0);
    let replay: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &ProtocolContext::host_dev("realization-test"),
                "host.realization.plan",
                serde_json::to_value(&request)?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert!(replay.replayed);
    assert_eq!(first.plan_ref, replay.plan_ref);
    assert_eq!(
        fixture
            .control
            .list_session(&JOURNAL_SESSION.to_string())
            .await?
            .len(),
        1
    );
    assert_eq!(
        fixture
            .public
            .list_session(&JOURNAL_SESSION.to_string())
            .await?
            .len(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn remote_copy_requires_explicit_rights_without_persisting_a_plan() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let mut target = fixture
        .runtime
        .config()
        .target_registry
        .status("local")
        .await
        .expect("local target");
    target.reachability = ExecutionTargetReachability::Direct;
    target.last_seen_at_ms = Some(Utc::now().timestamp_millis());
    target.heartbeat_expires_at_ms = Some(Utc::now().timestamp_millis() + 60_000);
    fixture
        .runtime
        .config()
        .target_registry
        .replace_control_plane_projection(target)
        .await;

    for (disposition, expected, key) in [
        (RightDisposition::Denied, "rights_denied", "copy-denied"),
        (
            RightDisposition::Unspecified,
            "rights_unspecified",
            "copy-unspecified",
        ),
    ] {
        install_rights(&fixture, &rights(disposition, RightDisposition::Allowed)).await?;
        let result: RealizationPlanResult = serde_json::from_value(
            fixture
                .runtime
                .call_protocol(
                    &ProtocolContext::host_dev("rights-test"),
                    "host.realization.plan",
                    serde_json::to_value(plan_request(&fixture.installation_id, key))?,
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert!(result.realization.is_none());
        assert_eq!(result.gaps[0].reason_code, expected);
    }
    assert!(fixture.control.list_all().await?.is_empty());
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn dedicated_server_intent_requires_the_declared_right() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    install_rights(
        &fixture,
        &rights(RightDisposition::Allowed, RightDisposition::Denied),
    )
    .await?;
    let current_ref = fixture
        .installations
        .artifacts
        .read()
        .await
        .work
        .operational_intent
        .clone()
        .expect("intent ref");
    let bytes = fixture
        .runtime
        .object_store()
        .get(&current_ref.digest)
        .await?;
    let mut intent: OperationalIntent = serde_json::from_slice(&bytes)?;
    intent.annotations.insert(
        "plurora.intent/dedicated_server".to_string(),
        serde_json::Value::Bool(true),
    );
    let descriptor = intent.artifact_descriptor()?;
    fixture
        .runtime
        .object_store()
        .put(intent.canonical_bytes()?.into())
        .await?;
    {
        let mut artifacts = fixture.installations.artifacts.write().await;
        artifacts.work.operational_intent = Some(descriptor);
        artifacts.installation.work_summary =
            InstallationWorkSummary::from_work_revision(&artifacts.work);
    }
    let result: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &ProtocolContext::host_dev("rights-test"),
                "host.realization.plan",
                serde_json::to_value(plan_request(&fixture.installation_id, "dedicated-rights"))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert!(result.realization.is_none());
    assert_eq!(result.gaps[0].reason_code, "rights_denied");
    assert!(fixture.control.list_all().await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn apply_requires_exact_approval_and_stale_installation_has_zero_effects(
) -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let planned: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &ProtocolContext::host_dev("realization-test"),
                "host.realization.plan",
                serde_json::to_value(plan_request(&fixture.installation_id, "apply-plan"))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    fixture
        .installations
        .artifacts
        .write()
        .await
        .installation
        .revision = 2;
    let request = apply_request(&planned, &fixture.installation_id, "apply-stale");
    assert!(fixture
        .runtime
        .call_protocol(
            &ProtocolContext::host_dev("realization-test"),
            "host.realization.apply",
            serde_json::to_value(request)?,
        )
        .await
        .is_err());
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn apply_then_stop_persists_receipts_and_closes_before_stopped() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let context = ProtocolContext::host_dev("realization-test");
    let planned: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.plan",
                serde_json::to_value(plan_request(&fixture.installation_id, "lifecycle-plan"))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    let active: RealizationMutationResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.apply",
                serde_json::to_value(apply_request(
                    &planned,
                    &fixture.installation_id,
                    "apply-key",
                ))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert_eq!(active.realization.status, RealizationStatus::Active);
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 1);
    assert_eq!(active.realization.receipts.len(), 2);
    let stopped: RealizationMutationResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.stop",
                serde_json::to_value(RealizationStopRequest {
                    installation_id: fixture.installation_id.clone(),
                    target_id: "local".to_string(),
                    realization_id: active.realization.realization_id.clone(),
                    expected_revision: active.realization.revision,
                    idempotency_key: "stop-key".to_string(),
                })?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert_eq!(stopped.realization.status, RealizationStatus::Stopped);
    assert_eq!(fixture.driver.stops.load(Ordering::SeqCst), 1);
    assert!(stopped.realization.stopped_at.is_some());
    assert_eq!(stopped.realization.receipts.len(), 3);
    let control_kinds = fixture
        .control
        .list_session(&JOURNAL_SESSION.to_string())
        .await?
        .into_iter()
        .map(|event| event.kind)
        .collect::<Vec<_>>();
    assert!(control_kinds.contains(&PRIVATE_EVENT_REALIZATION_STOPPING.to_string()));
    assert!(!PLATFORM_EVENT_KINDS.contains(&PRIVATE_EVENT_REALIZATION_STOPPING));
    let public_kinds = fixture
        .public
        .list_session(&JOURNAL_SESSION.to_string())
        .await?
        .into_iter()
        .map(|event| event.kind)
        .collect::<Vec<_>>();
    assert!(!public_kinds.contains(&PRIVATE_EVENT_REALIZATION_STOPPING.to_string()));
    assert!(!public_kinds.contains(&EVENT_REALIZATION_RECONCILED.to_string()));
    Ok(())
}

#[tokio::test]
async fn hydrate_marks_unowned_incomplete_apply_recovery_required() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let context = ProtocolContext::host_dev("realization-test");
    let planned: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.plan",
                serde_json::to_value(plan_request(&fixture.installation_id, "recover-plan"))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    let record = fixture
        .registry
        .current(&planned.realization.as_ref().unwrap().realization_id)
        .await?;
    let mut applying = record.clone();
    applying.revision = transition(
        &record.revision,
        RealizationStatus::Applying,
        Vec::new(),
        Vec::new(),
        None,
    );
    fixture
        .registry
        .append(
            EVENT_REALIZATION_APPLYING,
            applying,
            OP_APPLY,
            None,
            None,
            None,
        )
        .await?;
    let rebuilt = RealizationRegistry::new(
        fixture.control.clone(),
        fixture.public.clone(),
        fixture.runtime.object_store(),
        fixture.installations.clone(),
        fixture.runtime.config().target_registry.clone(),
    )?;
    rebuilt.hydrate().await?;
    let recovered = rebuilt
        .get(RealizationGetRequest {
            installation_id: fixture.installation_id,
            realization_id: record.revision.realization_id,
        })
        .await?
        .expect("recovered");
    assert_eq!(recovered.status, RealizationStatus::RecoveryRequired);
    Ok(())
}

#[tokio::test]
async fn durable_apply_checkpoint_replays_without_repeating_the_external_effect(
) -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let context = ProtocolContext::host_dev("realization-test");
    let planned: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.plan",
                serde_json::to_value(plan_request(&fixture.installation_id, "checkpoint-plan"))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    let request = apply_request(&planned, &fixture.installation_id, "checkpoint-apply");
    let checkpoint = append_apply_checkpoint(&fixture, &request, OP_APPLY).await?;
    assert_eq!(checkpoint.revision.status, RealizationStatus::Applying);
    assert_eq!(checkpoint.revision.actual_resources.len(), 1);

    let resumed: RealizationMutationResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.apply",
                serde_json::to_value(request)?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert!(resumed.replayed);
    assert_eq!(resumed.realization.status, RealizationStatus::Active);
    assert_eq!(resumed.realization.receipts.len(), 2);
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 0);
    let control = fixture
        .control
        .list_session(&JOURNAL_SESSION.to_string())
        .await?;
    assert_eq!(
        control
            .iter()
            .filter(|event| event.kind == PRIVATE_EVENT_REALIZATION_EFFECT_APPLIED)
            .count(),
        1
    );
    assert!(!PLATFORM_EVENT_KINDS.contains(&PRIVATE_EVENT_REALIZATION_EFFECT_APPLIED));
    assert!(fixture
        .public
        .list_session(&JOURNAL_SESSION.to_string())
        .await?
        .iter()
        .all(|event| event.kind != PRIVATE_EVENT_REALIZATION_EFFECT_APPLIED));
    Ok(())
}

#[tokio::test]
async fn hydrate_recovers_apply_and_stop_effect_checkpoints_without_repeating_effects(
) -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let context = ProtocolContext::host_dev("realization-test");
    let planned: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.plan",
                serde_json::to_value(plan_request(
                    &fixture.installation_id,
                    "hydrate-checkpoint-plan",
                ))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    let request = apply_request(
        &planned,
        &fixture.installation_id,
        "hydrate-checkpoint-apply",
    );
    let checkpoint = append_apply_checkpoint(&fixture, &request, OP_APPLY).await?;
    let rebuilt = RealizationRegistry::new(
        fixture.control.clone(),
        fixture.public.clone(),
        fixture.runtime.object_store(),
        fixture.installations.clone(),
        fixture.runtime.config().target_registry.clone(),
    )?;
    rebuilt.install_driver(fixture.driver.clone());
    rebuilt.hydrate().await?;
    let active = rebuilt.current(&checkpoint.revision.realization_id).await?;
    assert_eq!(active.revision.status, RealizationStatus::Active);
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 0);

    let mut stopping = active.clone();
    stopping.revision = transition(
        &active.revision,
        RealizationStatus::Stopping,
        active.revision.actual_resources.clone(),
        active.revision.receipts.clone(),
        None,
    );
    rebuilt
        .append(
            PRIVATE_EVENT_REALIZATION_STOPPING,
            stopping.clone(),
            OP_STOP,
            None,
            None,
            None,
        )
        .await?;
    let stop_checkpoint = rebuilt
        .checkpoint_stop_effect(
            &stopping,
            vec![receipt(
                &stopping.revision,
                &stopping.target_id,
                RealizationEffectKind::Stop,
            )],
            OP_STOP,
        )
        .await?;
    let recovered = RealizationRegistry::new(
        fixture.control.clone(),
        fixture.public.clone(),
        fixture.runtime.object_store(),
        fixture.installations.clone(),
        fixture.runtime.config().target_registry.clone(),
    )?;
    recovered.install_driver(fixture.driver.clone());
    recovered.hydrate().await?;
    let stopped = recovered
        .current(&stop_checkpoint.revision.realization_id)
        .await?;
    assert_eq!(stopped.revision.status, RealizationStatus::Stopped);
    assert_eq!(stopped.revision.receipts.len(), 3);
    assert_eq!(fixture.driver.stops.load(Ordering::SeqCst), 0);
    assert!(!PLATFORM_EVENT_KINDS.contains(&PRIVATE_EVENT_REALIZATION_EFFECT_STOPPED));
    Ok(())
}

#[tokio::test]
async fn same_observed_inputs_produce_the_same_plan_digest_across_new_requests(
) -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let context = ProtocolContext::host_dev("realization-test");
    let first: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.plan",
                serde_json::to_value(plan_request(&fixture.installation_id, "stable-plan-a"))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    let second: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.plan",
                serde_json::to_value(plan_request(&fixture.installation_id, "stable-plan-b"))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert_ne!(
        first.realization.as_ref().unwrap().realization_id,
        second.realization.as_ref().unwrap().realization_id
    );
    assert_eq!(first.plan, second.plan);
    assert_eq!(first.plan_ref, second.plan_ref);
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn target_capability_mismatch_returns_gap_without_realization_or_effect() -> anyhow::Result<()>
{
    let fixture = fixture().await?;
    let mut request = plan_request(&fixture.installation_id, "unsupported-target");
    let current_intent_ref = fixture
        .installations
        .artifacts
        .read()
        .await
        .work
        .operational_intent
        .clone()
        .expect("intent ref");
    let mut intent: OperationalIntent = serde_json::from_slice(
        &fixture
            .runtime
            .object_store()
            .get(&current_intent_ref.digest)
            .await?,
    )?;
    intent.workloads[0]
        .execution_classes
        .push("unsupported-runtime.v1".to_string());
    let bytes = plurora_work::canonical_json_bytes(&intent)?;
    let info = fixture.runtime.object_store().put(bytes.into()).await?;
    fixture
        .installations
        .artifacts
        .write()
        .await
        .work
        .operational_intent = Some(ArtifactDescriptor {
        artifact_type_uri: OPERATIONAL_INTENT_TYPE_URI.to_string(),
        media_type: plurora_work::CANONICAL_JSON_MEDIA_TYPE.to_string(),
        digest: info.digest,
        size_bytes: info.size_bytes,
        references: Vec::new(),
        annotations: BTreeMap::new(),
    });
    let RealizationBackendSelection::OciImage(selection) = &mut request.backends[0] else {
        unreachable!()
    };
    selection.execution_class = "unsupported-runtime.v1".to_string();
    let result: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &ProtocolContext::host_dev("realization-test"),
                "host.realization.plan",
                serde_json::to_value(request)?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert!(result.realization.is_none());
    assert!(
        result
            .gaps
            .iter()
            .any(|gap| gap.reason_code == "unsupported_backend"),
        "{:?}",
        result.gaps
    );
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 0);
    assert!(fixture
        .control
        .list_session(&JOURNAL_SESSION.to_string())
        .await?
        .is_empty());
    Ok(())
}

#[tokio::test]
async fn first_executor_rejects_multiple_workloads_before_persisting_a_plan() -> anyhow::Result<()>
{
    let fixture = fixture().await?;
    let mut request = plan_request(&fixture.installation_id, "multiple-workloads");
    let mut second = request.backends[0].clone();
    match &mut second {
        RealizationBackendSelection::OciImage(selection) => {
            selection.workload_id = "worker".to_string();
            selection.route_id = "worker".to_string();
            selection.port_name = "worker-http".to_string();
        }
        RealizationBackendSelection::DockerBuild(selection) => {
            selection.workload_id = "worker".to_string();
            selection.route_id = "worker".to_string();
            selection.port_name = "worker-http".to_string();
        }
    }
    request.backends.push(second);
    let result: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &ProtocolContext::host_dev("realization-test"),
                "host.realization.plan",
                serde_json::to_value(request)?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert!(result.plan.is_none());
    assert!(result.realization.is_none());
    assert_eq!(result.gaps.len(), 1);
    assert_eq!(result.gaps[0].reason_code, "unsupported_backend");
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 0);
    assert!(fixture
        .control
        .list_session(&JOURNAL_SESSION.to_string())
        .await?
        .is_empty());
    Ok(())
}

#[tokio::test]
async fn apply_approval_and_idempotency_are_exact_and_unknown_outcome_is_durable(
) -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let context = ProtocolContext::host_dev("realization-test");
    let planned: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.plan",
                serde_json::to_value(plan_request(&fixture.installation_id, "approval-plan"))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    let mut denied = apply_request(&planned, &fixture.installation_id, "approval-denied");
    denied.approval.accepted_risks.clear();
    assert!(fixture
        .runtime
        .call_protocol(
            &context,
            "host.realization.apply",
            serde_json::to_value(denied)?,
        )
        .await
        .is_err());
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 0);

    fixture
        .driver
        .next_apply_outcome_unknown
        .store(true, Ordering::SeqCst);
    let request = apply_request(&planned, &fixture.installation_id, "approval-apply");
    let first: RealizationMutationResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.apply",
                serde_json::to_value(&request)?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert_eq!(first.realization.status, RealizationStatus::OutcomeUnknown);
    assert_eq!(
        first.realization.health.reason_code.as_deref(),
        Some("outcome_unknown")
    );
    let replay: RealizationMutationResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.apply",
                serde_json::to_value(request)?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert!(replay.replayed);
    assert_eq!(replay.realization.status, RealizationStatus::OutcomeUnknown);
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn partial_apply_persists_resources_and_receipts_as_recovery_required() -> anyhow::Result<()>
{
    let fixture = fixture().await?;
    let context = ProtocolContext::host_dev("realization-test");
    let planned: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.plan",
                serde_json::to_value(plan_request(&fixture.installation_id, "partial-plan"))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    fixture
        .driver
        .next_apply_partial
        .store(true, Ordering::SeqCst);
    let request = apply_request(&planned, &fixture.installation_id, "partial-apply");
    let first: RealizationMutationResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.apply",
                serde_json::to_value(&request)?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert_eq!(
        first.realization.status,
        RealizationStatus::RecoveryRequired
    );
    assert_eq!(
        first.realization.health.reason_code.as_deref(),
        Some("recovery_required")
    );
    assert_eq!(first.realization.actual_resources.len(), 1);
    assert_eq!(first.realization.receipts.len(), 2);

    let replay: RealizationMutationResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.apply",
                serde_json::to_value(request)?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert!(replay.replayed);
    assert_eq!(replay.realization, first.realization);
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn rollback_uses_persisted_historic_plan_and_host_generates_child_id() -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let context = ProtocolContext::host_dev("realization-test");
    let current_plan: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.plan",
                serde_json::to_value(plan_request(
                    &fixture.installation_id,
                    "rollback-current-plan",
                ))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    let current: RealizationMutationResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.apply",
                serde_json::to_value(apply_request(
                    &current_plan,
                    &fixture.installation_id,
                    "rollback-current-apply",
                ))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    let historic: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.plan",
                serde_json::to_value(plan_request(
                    &fixture.installation_id,
                    "rollback-historic-plan",
                ))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    let historic_revision = historic.realization.as_ref().expect("historic realization");
    let historic_plan = historic.plan.as_ref().expect("historic plan");
    let result: RealizationMutationResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.rollback",
                serde_json::to_value(RealizationRollbackRequest {
                    installation_id: fixture.installation_id.clone(),
                    target_id: "local".to_string(),
                    realization_id: current.realization.realization_id.clone(),
                    expected_revision: current.realization.revision,
                    rollback_to_realization_id: historic_revision.realization_id.clone(),
                    approval: plurora_runtime::RealizationApproval {
                        plan_digest: historic.plan_ref.as_ref().unwrap().digest.clone(),
                        decision: "approved".to_string(),
                        accepted_risks: historic_plan.risk_summary.clone(),
                        decided_at: Utc::now(),
                        expires_at: None,
                    },
                    idempotency_key: "rollback-exact".to_string(),
                })?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert_eq!(result.realization.status, RealizationStatus::Active);
    assert_ne!(
        result.realization.realization_id,
        historic_revision.realization_id
    );
    assert_eq!(
        result.realization.parent_realization_id.as_ref(),
        Some(&current.realization.realization_id)
    );
    assert_eq!(result.realization.plan_ref, historic_revision.plan_ref);
    assert_eq!(result.realization.receipts.len(), 2);
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.driver.stops.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn rollback_retry_resumes_durable_child_and_parent_checkpoints_without_reapplying(
) -> anyhow::Result<()> {
    let fixture = fixture().await?;
    let context = ProtocolContext::host_dev("realization-test");
    let current_plan: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.plan",
                serde_json::to_value(plan_request(
                    &fixture.installation_id,
                    "rollback-resume-current-plan",
                ))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    let current: RealizationMutationResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.apply",
                serde_json::to_value(apply_request(
                    &current_plan,
                    &fixture.installation_id,
                    "rollback-resume-current-apply",
                ))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    let historic: RealizationPlanResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.plan",
                serde_json::to_value(plan_request(
                    &fixture.installation_id,
                    "rollback-resume-historic-plan",
                ))?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    let request = rollback_request(
        &fixture.installation_id,
        &current.realization,
        &historic,
        "rollback-resume",
    );
    fixture
        .driver
        .next_stop_failure
        .store(true, Ordering::SeqCst);
    assert!(fixture
        .runtime
        .call_protocol(
            &context,
            "host.realization.rollback",
            serde_json::to_value(&request)?,
        )
        .await
        .is_err());
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.driver.stops.load(Ordering::SeqCst), 1);
    let parent = fixture
        .registry
        .current(&current.realization.realization_id)
        .await?;
    assert_eq!(parent.revision.status, RealizationStatus::Stopping);
    let child = fixture
        .registry
        .projection
        .read()
        .await
        .records
        .values()
        .find(|record| {
            record.revision.parent_realization_id.as_ref()
                == Some(&current.realization.realization_id)
        })
        .cloned()
        .expect("durable rollback child");
    assert_eq!(child.revision.status, RealizationStatus::Applying);
    assert_eq!(child.revision.actual_resources.len(), 1);

    let resumed: RealizationMutationResult = serde_json::from_value(
        fixture
            .runtime
            .call_protocol(
                &context,
                "host.realization.rollback",
                serde_json::to_value(request)?,
            )
            .await
            .map_err(protocol_error)?,
    )?;
    assert!(resumed.replayed);
    assert_eq!(resumed.realization.status, RealizationStatus::Active);
    assert_eq!(fixture.driver.applies.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.driver.stops.load(Ordering::SeqCst), 2);
    assert_eq!(
        fixture
            .registry
            .current(&current.realization.realization_id)
            .await?
            .revision
            .status,
        RealizationStatus::Stopped
    );
    Ok(())
}

async fn append_apply_checkpoint(
    fixture: &Fixture,
    request: &RealizationApplyRequest,
    operation: &str,
) -> anyhow::Result<RealizationRecord> {
    let record = fixture.registry.current(&request.realization_id).await?;
    let approval_ref = put_json_artifact(
        fixture.runtime.object_store().as_ref(),
        planner::REALIZATION_APPROVAL_TYPE_URI,
        &request.approval,
        vec![request.plan_ref.digest.clone()],
        BTreeMap::new(),
    )
    .await?;
    let mut applying = record.clone();
    applying.revision = transition(
        &record.revision,
        RealizationStatus::Applying,
        Vec::new(),
        vec![approval_ref],
        None,
    );
    let request_fingerprint = fingerprint(request)?;
    fixture
        .registry
        .append(
            EVENT_REALIZATION_APPLYING,
            applying.clone(),
            operation,
            None,
            Some((&request.idempotency_key, &request_fingerprint)),
            None,
        )
        .await?;
    fixture
        .registry
        .checkpoint_apply_effect(
            &applying,
            RealizationExecution {
                resources: vec![RealizedResource {
                    resource_id: "api".to_string(),
                    resource_type: "managed_workload".to_string(),
                    target_id: "local".to_string(),
                    backend_id: "fake-operation".to_string(),
                    properties: BTreeMap::new(),
                    receipt_ref: None,
                }],
                receipts: vec![receipt(
                    &applying.revision,
                    "local",
                    RealizationEffectKind::Launch,
                )],
            },
            operation,
        )
        .await
}

fn rollback_request(
    installation_id: &InstallationId,
    current: &RealizationRevision,
    historic: &RealizationPlanResult,
    key: &str,
) -> RealizationRollbackRequest {
    let historic_revision = historic.realization.as_ref().expect("historic realization");
    let historic_plan = historic.plan.as_ref().expect("historic plan");
    RealizationRollbackRequest {
        installation_id: installation_id.clone(),
        target_id: "local".to_string(),
        realization_id: current.realization_id.clone(),
        expected_revision: current.revision,
        rollback_to_realization_id: historic_revision.realization_id.clone(),
        approval: plurora_runtime::RealizationApproval {
            plan_digest: historic.plan_ref.as_ref().unwrap().digest.clone(),
            decision: "approved".to_string(),
            accepted_risks: historic_plan.risk_summary.clone(),
            decided_at: Utc::now(),
            expires_at: None,
        },
        idempotency_key: key.to_string(),
    }
}

fn plan_request(installation_id: &InstallationId, key: &str) -> RealizationPlanRequest {
    RealizationPlanRequest {
        installation_id: installation_id.clone(),
        expected_installation_revision: 1,
        target_id: "local".to_string(),
        backends: vec![RealizationBackendSelection::OciImage(
            plurora_runtime::OciImageBackendSelection {
                workload_id: "api".to_string(),
                execution_class: "oci-container.v1".to_string(),
                image: format!("registry.example/app@sha256:{}", "f".repeat(64)),
                container_port: 8080,
                port_name: "http".to_string(),
                route_id: "realized-api".to_string(),
                route_access: plurora_runtime::ProxyRouteAccess::HostAuthenticated,
                health_path: Some("/health".to_string()),
                pull_if_missing: false,
            },
        )],
        idempotency_key: key.to_string(),
    }
}

fn apply_request(
    planned: &RealizationPlanResult,
    installation_id: &InstallationId,
    key: &str,
) -> RealizationApplyRequest {
    let plan = planned.plan.as_ref().expect("plan");
    let realization = planned.realization.as_ref().expect("realization");
    RealizationApplyRequest {
        installation_id: installation_id.clone(),
        target_id: "local".to_string(),
        realization_id: realization.realization_id.clone(),
        expected_revision: realization.revision,
        plan_ref: planned.plan_ref.clone().expect("plan ref"),
        approval: plurora_runtime::RealizationApproval {
            plan_digest: planned.plan_ref.as_ref().unwrap().digest.clone(),
            decision: "approved".to_string(),
            accepted_risks: plan.risk_summary.clone(),
            decided_at: Utc::now(),
            expires_at: None,
        },
        idempotency_key: key.to_string(),
    }
}

fn receipt(
    realization: &RealizationRevision,
    target_id: &str,
    effect: RealizationEffectKind,
) -> RealizationEffectReceipt {
    let now = Utc::now();
    RealizationEffectReceipt {
        realization_id: realization.realization_id.clone(),
        action_id: format!("fake-{effect:?}"),
        target_id: target_id.to_string(),
        effect,
        request_digest: format!("sha256:{}", "9".repeat(64)),
        status: "succeeded".to_string(),
        started_at: now,
        finished_at: now,
        observed_resources: Vec::new(),
        diagnostic_ref: None,
    }
}

fn descriptor(kind: &str, marker: char) -> ArtifactDescriptor {
    ArtifactDescriptor {
        artifact_type_uri: kind.to_string(),
        media_type: plurora_work::CANONICAL_JSON_MEDIA_TYPE.to_string(),
        digest: format!("sha256:{}", marker.to_string().repeat(64)),
        size_bytes: 1,
        references: Vec::new(),
        annotations: BTreeMap::new(),
    }
}

fn protocol_error(error: plurora_runtime::ProtocolError) -> anyhow::Error {
    anyhow!("{}: {}", error.code, error.message)
}
