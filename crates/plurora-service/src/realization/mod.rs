mod executor;
mod planner;
mod projection;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex as StdMutex, RwLock, Weak};

use anyhow::{anyhow, ensure};
use async_trait::async_trait;
use chrono::Utc;
use plurora_core::{
    ChangePrecondition, EventEnvelope, EVENT_REALIZATION_ACTIVE, EVENT_REALIZATION_APPLYING,
    EVENT_REALIZATION_FAILED, EVENT_REALIZATION_PLANNED, EVENT_REALIZATION_RECONCILED,
    EVENT_REALIZATION_ROLLED_BACK, EVENT_REALIZATION_STOPPED, PLATFORM_RUNTIME_ID,
};
use plurora_runtime::{
    is_managed_target_workload_outcome_unknown, load_rights_declaration, rights_policy_outcome,
    EventStore, ExecutionTarget, ExecutionTargetReachability, ExecutionTargetRegistry,
    ExecutionTargetStatusKind, InstallationControl, ObjectStore, RealizationApplyRequest,
    RealizationAuthoritySubject, RealizationBackendSelection, RealizationControl,
    RealizationEffectReceipt, RealizationGetRequest, RealizationListRequest,
    RealizationMutationAuthority, RealizationMutationResult, RealizationPlanRequest,
    RealizationPlanResult, RealizationReconcileRequest, RealizationRollbackRequest,
    RealizationStopRequest, RunInstallationArtifacts,
};
use plurora_work::{
    canonical_json_bytes, compile_realization_plan, ArtifactModel, HealthStatus, OperationalIntent,
    PlannedWorkloadInput, RealizationHealth, RealizationId, RealizationPlan,
    RealizationPlanningGap, RealizationRevision, RealizationStatus, RealizedResource,
    RightsOperation, REALIZATION_PLAN_TYPE_URI,
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::DevelopmentHostLease;
pub use executor::{
    reconcile_realization_backends, RealizationBackendReconcileSummary, ServiceRealizationExecutor,
};
use planner::{persist_target_inventory, prepare_backend_artifacts, put_json_artifact};
use projection::{apply_event, RealizationJournalEvent, RealizationProjection, RealizationRecord};

const JOURNAL_SESSION: &str = "host_realizations";
const JOURNAL_WRITER: &str = "host/control-plane";
const JOURNAL_SCHEMA: u16 = 1;
const OP_PLAN: &str = "plan";
const OP_APPLY: &str = "apply";
const OP_STOP: &str = "stop";
const OP_ROLLBACK: &str = "rollback";
const OP_RECONCILE: &str = "reconcile";
const PRIVATE_EVENT_REALIZATION_STOPPING: &str = "host-private/realization.stopping";
const PRIVATE_EVENT_REALIZATION_EFFECT_APPLIED: &str = "host-private/realization.effect-applied";
const PRIVATE_EVENT_REALIZATION_EFFECT_STOPPED: &str = "host-private/realization.effect-stopped";

#[derive(Debug, Clone)]
pub struct RealizationExecution {
    pub resources: Vec<RealizedResource>,
    pub receipts: Vec<RealizationEffectReceipt>,
}

#[derive(Debug)]
struct PartialRealizationExecution {
    reason_code: &'static str,
    resources: Vec<RealizedResource>,
    receipts: Vec<RealizationEffectReceipt>,
}

impl std::fmt::Display for PartialRealizationExecution {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}: Realization effect completed only partially",
            self.reason_code
        )
    }
}

impl std::error::Error for PartialRealizationExecution {}

fn partial_execution_error(
    reason_code: &'static str,
    resources: Vec<RealizedResource>,
    receipts: Vec<RealizationEffectReceipt>,
) -> anyhow::Error {
    PartialRealizationExecution {
        reason_code,
        resources,
        receipts,
    }
    .into()
}

#[derive(Debug, Clone)]
pub struct RealizationObservation {
    pub status: RealizationStatus,
    pub resources: Vec<RealizedResource>,
    pub receipts: Vec<RealizationEffectReceipt>,
    pub reason_code: Option<String>,
}

#[async_trait]
pub trait RealizationExecutionDriver: Send + Sync + 'static {
    async fn apply(
        &self,
        realization: &RealizationRevision,
        target_id: &str,
        plan: &RealizationPlan,
        backends: &[RealizationBackendSelection],
        authority: &RealizationMutationAuthority,
        subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<RealizationExecution>;

    async fn stop(
        &self,
        realization: &RealizationRevision,
        target_id: &str,
        authority: &RealizationMutationAuthority,
        subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<Vec<RealizationEffectReceipt>>;

    async fn observe(
        &self,
        realization: &RealizationRevision,
        target_id: &str,
    ) -> anyhow::Result<RealizationObservation>;
}

#[derive(Debug, Default)]
struct UnavailableRealizationExecutionDriver;

#[async_trait]
impl RealizationExecutionDriver for UnavailableRealizationExecutionDriver {
    async fn apply(
        &self,
        _realization: &RealizationRevision,
        _target_id: &str,
        _plan: &RealizationPlan,
        _backends: &[RealizationBackendSelection],
        _authority: &RealizationMutationAuthority,
        _subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<RealizationExecution> {
        anyhow::bail!("unsupported_backend: Realization execution driver is unavailable")
    }

    async fn stop(
        &self,
        _realization: &RealizationRevision,
        _target_id: &str,
        _authority: &RealizationMutationAuthority,
        _subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<Vec<RealizationEffectReceipt>> {
        anyhow::bail!("recovery_required: Realization execution driver is unavailable")
    }

    async fn observe(
        &self,
        _realization: &RealizationRevision,
        _target_id: &str,
    ) -> anyhow::Result<RealizationObservation> {
        Ok(RealizationObservation {
            status: RealizationStatus::RecoveryRequired,
            resources: Vec::new(),
            receipts: Vec::new(),
            reason_code: Some("recovery_required".to_string()),
        })
    }
}

pub struct RealizationRegistry {
    store: Arc<dyn EventStore>,
    public_store: Arc<dyn EventStore>,
    object_store: Arc<dyn ObjectStore>,
    installations: Arc<dyn InstallationControl>,
    targets: Arc<ExecutionTargetRegistry>,
    projection: tokio::sync::RwLock<RealizationProjection>,
    mutation: Mutex<()>,
    relay: Mutex<()>,
    effect_locks: StdMutex<BTreeMap<RealizationId, Weak<Mutex<()>>>>,
    owner_lease: RwLock<Option<DevelopmentHostLease>>,
    driver: RwLock<Arc<dyn RealizationExecutionDriver>>,
}

impl std::fmt::Debug for RealizationRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RealizationRegistry(<host-owned>)")
    }
}

impl RealizationRegistry {
    pub fn new(
        store: Arc<dyn EventStore>,
        public_store: Arc<dyn EventStore>,
        object_store: Arc<dyn ObjectStore>,
        installations: Arc<dyn InstallationControl>,
        targets: Arc<ExecutionTargetRegistry>,
    ) -> anyhow::Result<Arc<Self>> {
        ensure!(
            !Arc::ptr_eq(&store, &public_store),
            "Realization control journal must be physically distinct from the Runtime public journal"
        );
        Ok(Arc::new(Self {
            store,
            public_store,
            object_store,
            installations,
            targets,
            projection: tokio::sync::RwLock::new(RealizationProjection::default()),
            mutation: Mutex::new(()),
            relay: Mutex::new(()),
            effect_locks: StdMutex::new(BTreeMap::new()),
            owner_lease: RwLock::new(None),
            driver: RwLock::new(Arc::new(UnavailableRealizationExecutionDriver)),
        }))
    }

    pub fn install_owner_lease(&self, lease: DevelopmentHostLease) -> anyhow::Result<()> {
        lease.ensure_active()?;
        let mut slot = self.owner_lease.write().map_err(lock_error)?;
        ensure!(
            slot.is_none(),
            "Realization owner lease is already installed"
        );
        *slot = Some(lease);
        Ok(())
    }

    pub fn install_driver(&self, driver: Arc<dyn RealizationExecutionDriver>) {
        *self
            .driver
            .write()
            .expect("Realization driver lock poisoned") = driver;
    }

    fn driver(&self) -> Arc<dyn RealizationExecutionDriver> {
        self.driver
            .read()
            .expect("Realization driver lock poisoned")
            .clone()
    }

    fn effect_lock(&self, realization_id: &RealizationId) -> anyhow::Result<Arc<Mutex<()>>> {
        let mut locks = self.effect_locks.lock().map_err(lock_error)?;
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(realization_id).and_then(Weak::upgrade) {
            return Ok(lock);
        }
        let lock = Arc::new(Mutex::new(()));
        locks.insert(realization_id.clone(), Arc::downgrade(&lock));
        Ok(lock)
    }

    async fn ensure_owner(&self) -> anyhow::Result<()> {
        let lease = self.owner_lease.read().map_err(lock_error)?.clone();
        if let Some(lease) = lease {
            lease.ensure_durable_owner().await?;
        }
        Ok(())
    }

    async fn sync_journal(&self) -> anyhow::Result<usize> {
        let next = self.projection.read().await.next_sequence;
        let events = self
            .store
            .list_session_range(&JOURNAL_SESSION.to_string(), next.checked_sub(1), None)
            .await?;
        let mut state = self.projection.write().await;
        let mut loaded = 0usize;
        for event in events {
            if event.sequence < state.next_sequence {
                continue;
            }
            validate_envelope(&event, state.next_sequence)?;
            apply_event(&mut state, &event)?;
            loaded = loaded.saturating_add(1);
        }
        Ok(loaded)
    }

    pub async fn hydrate(&self) -> anyhow::Result<usize> {
        let _mutation = self.mutation.lock().await;
        self.ensure_owner().await?;
        let events = self
            .store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?;
        let mut rebuilt = RealizationProjection::default();
        for event in &events {
            validate_envelope(event, rebuilt.next_sequence)?;
            apply_event(&mut rebuilt, event)?;
        }
        *self.projection.write().await = rebuilt;
        drop(_mutation);
        self.relay_pending().await?;
        self.recover_incomplete().await?;
        Ok(events.len())
    }

    async fn recover_incomplete(&self) -> anyhow::Result<()> {
        let records = self
            .projection
            .read()
            .await
            .records
            .values()
            .filter(|record| {
                matches!(
                    record.revision.status,
                    RealizationStatus::Applying | RealizationStatus::Stopping
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        for record in records {
            let (apply_checkpoint, stop_checkpoint) = {
                let state = self.projection.read().await;
                (
                    state
                        .apply_checkpoints
                        .contains(&record.revision.realization_id),
                    state
                        .stop_checkpoints
                        .contains(&record.revision.realization_id),
                )
            };
            if record.revision.status == RealizationStatus::Stopping && stop_checkpoint {
                self.commit_stop_checkpoint(record, None, OP_RECONCILE, None)
                    .await?;
                continue;
            }
            let rollback_parent_is_stopped =
                if let Some(parent_id) = record.revision.parent_realization_id.as_ref() {
                    self.projection
                        .read()
                        .await
                        .records
                        .get(parent_id)
                        .is_some_and(|parent| parent.revision.status == RealizationStatus::Stopped)
                } else {
                    true
                };
            let observation = if record.revision.status == RealizationStatus::Applying
                && apply_checkpoint
                && rollback_parent_is_stopped
            {
                self.driver()
                    .observe(&record.revision, &record.target_id)
                    .await
                    .unwrap_or(RealizationObservation {
                        status: RealizationStatus::RecoveryRequired,
                        resources: record.revision.actual_resources.clone(),
                        receipts: Vec::new(),
                        reason_code: Some("recovery_required".to_string()),
                    })
            } else {
                RealizationObservation {
                    status: RealizationStatus::RecoveryRequired,
                    resources: record.revision.actual_resources.clone(),
                    receipts: Vec::new(),
                    reason_code: Some(
                        if record.revision.parent_realization_id.is_some()
                            && !rollback_parent_is_stopped
                        {
                            "rollback_incomplete"
                        } else {
                            "recovery_required"
                        }
                        .to_string(),
                    ),
                }
            };
            let status = match (record.revision.status, observation.status) {
                (RealizationStatus::Applying, RealizationStatus::Active) => {
                    RealizationStatus::Active
                }
                (RealizationStatus::Applying, RealizationStatus::OutcomeUnknown) => {
                    RealizationStatus::OutcomeUnknown
                }
                (RealizationStatus::Applying, RealizationStatus::Failed) if apply_checkpoint => {
                    RealizationStatus::Failed
                }
                _ => RealizationStatus::RecoveryRequired,
            };
            let mut receipts = record.revision.receipts.clone();
            let pending = self
                .projection
                .read()
                .await
                .pending_apply_receipts
                .get(&record.revision.realization_id)
                .cloned()
                .unwrap_or_default();
            receipts.extend(self.persist_receipts(pending).await?);
            receipts.extend(self.persist_receipts(observation.receipts).await?);
            let mut next = record.clone();
            next.revision = transition(
                &record.revision,
                status,
                observation.resources,
                receipts,
                observation.reason_code.as_deref(),
            );
            let _mutation = self.mutation.lock().await;
            self.sync_journal().await?;
            let current = self.current(&record.revision.realization_id).await?;
            if current.revision.revision != record.revision.revision {
                continue;
            }
            let event_kind = if status == RealizationStatus::Active
                && record.revision.parent_realization_id.is_some()
            {
                EVENT_REALIZATION_ROLLED_BACK
            } else {
                EVENT_REALIZATION_RECONCILED
            };
            let event_operation = if event_kind == EVENT_REALIZATION_ROLLED_BACK {
                OP_ROLLBACK
            } else {
                OP_RECONCILE
            };
            self.append(
                event_kind,
                next,
                event_operation,
                observation.reason_code,
                None,
                None,
            )
            .await?;
        }
        Ok(())
    }

    async fn append(
        &self,
        kind: &str,
        record: RealizationRecord,
        operation: &str,
        reason_code: Option<String>,
        idempotency: Option<(&str, &str)>,
        authority: Option<(&RealizationMutationAuthority, &RealizationAuthoritySubject)>,
    ) -> anyhow::Result<EventEnvelope> {
        let payload = RealizationJournalEvent {
            record,
            operation: operation.to_string(),
            reason_code,
            idempotency_key: idempotency.map(|(key, _)| hash_text(key)),
            request_fingerprint: idempotency.map(|(_, fingerprint)| fingerprint.to_string()),
            effect_receipts: Vec::new(),
        };
        self.append_payload(kind, payload, authority).await
    }

    async fn append_effect_checkpoint(
        &self,
        kind: &str,
        record: RealizationRecord,
        operation: &str,
        effect_receipts: Vec<RealizationEffectReceipt>,
    ) -> anyhow::Result<EventEnvelope> {
        let payload = RealizationJournalEvent {
            record,
            operation: operation.to_string(),
            reason_code: Some("effect_completed_pending_commit".to_string()),
            idempotency_key: None,
            request_fingerprint: None,
            effect_receipts,
        };
        self.append_payload(kind, payload, None).await
    }

    async fn append_payload(
        &self,
        kind: &str,
        payload: RealizationJournalEvent,
        authority: Option<(&RealizationMutationAuthority, &RealizationAuthoritySubject)>,
    ) -> anyhow::Result<EventEnvelope> {
        self.ensure_owner().await?;
        if let Some((authority, subject)) = authority {
            authority.refresh_current_for(subject).await?;
        }
        let mut state = self.projection.write().await;
        let event = self
            .store
            .append_with_sequence_if_next(
                JOURNAL_SESSION.to_string(),
                state.next_sequence,
                JOURNAL_WRITER.to_string(),
                kind.to_string(),
                JOURNAL_SCHEMA,
                serde_json::to_value(payload)?,
                json!({}),
            )
            .await?
            .ok_or_else(|| anyhow!("Realization journal compare-and-append conflict"))?;
        apply_event(&mut state, &event)?;
        drop(state);
        self.relay_event(&event).await?;
        Ok(event)
    }

    async fn replay(
        &self,
        operation: &str,
        key: &str,
        fingerprint: &str,
    ) -> anyhow::Result<Option<RealizationRecord>> {
        let state = self.projection.read().await;
        let Some((existing_fingerprint, id)) = state
            .idempotency
            .get(&(operation.to_string(), hash_text(key)))
        else {
            return Ok(None);
        };
        ensure!(
            existing_fingerprint == fingerprint,
            "idempotency_conflict: key was already used with another Realization request"
        );
        Ok(state.records.get(id).cloned())
    }

    async fn current(&self, id: &RealizationId) -> anyhow::Result<RealizationRecord> {
        self.projection
            .read()
            .await
            .records
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow!("Realization not found"))
    }

    async fn relay_pending(&self) -> anyhow::Result<usize> {
        let events = self
            .store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?;
        let mut count = 0usize;
        for event in events {
            if is_public_event(&event.kind) && self.relay_event(&event).await? {
                count = count.saturating_add(1);
            }
        }
        Ok(count)
    }

    async fn relay_event(&self, source: &EventEnvelope) -> anyhow::Result<bool> {
        if !is_public_event(&source.kind) {
            return Ok(false);
        }
        let _relay = self.relay.lock().await;
        let payload: RealizationJournalEvent = serde_json::from_value(source.payload.clone())?;
        let public = json!({
            "realization": payload.record.revision,
            "target_id": payload.record.target_id,
            "operation": payload.operation,
            "reason_code": payload.reason_code,
        });
        let digest = hash_bytes(&serde_json::to_vec(&public)?);
        loop {
            let existing = self
                .public_store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?;
            if existing.iter().any(|event| {
                event
                    .metadata
                    .get("source_event_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(source.id.as_str())
            }) {
                return Ok(false);
            }
            let expected = self
                .public_store
                .next_sequence(&JOURNAL_SESSION.to_string())
                .await?;
            if self
                .public_store
                .append_with_sequence_if_next(
                    JOURNAL_SESSION.to_string(),
                    expected,
                    PLATFORM_RUNTIME_ID.to_string(),
                    source.kind.clone(),
                    JOURNAL_SCHEMA,
                    public.clone(),
                    json!({
                        "authority": "none",
                        "source_event_id": source.id,
                        "source_event_sequence": source.sequence,
                        "source_payload_digest": digest,
                    }),
                )
                .await?
                .is_some()
            {
                return Ok(true);
            }
        }
    }

    async fn prepare_plan(
        &self,
        request: &RealizationPlanRequest,
    ) -> anyhow::Result<Result<(RealizationPlan, RealizationRecord), Vec<RealizationPlanningGap>>>
    {
        validate_key(&request.idempotency_key)?;
        ensure!(
            !request.backends.is_empty(),
            "target_unsatisfied: no backend was selected"
        );
        let artifacts = self
            .installations
            .inspect_current_ready_for_run(&request.installation_id)
            .await?;
        ensure!(
            artifacts.installation.revision == request.expected_installation_revision,
            "installation_revision_conflict: Realization plan parent is stale"
        );
        let target = self
            .targets
            .status(&request.target_id)
            .await
            .ok_or_else(|| anyhow!("target_unsatisfied: selected Target is unknown"))?;
        ensure_target_available(&target)?;
        if target.reachability != ExecutionTargetReachability::LocalHost {
            let rights =
                load_rights_declaration(self.object_store.as_ref(), &artifacts.work).await?;
            let outcome = rights_policy_outcome(rights.as_ref(), RightsOperation::CopyAcrossHosts);
            if outcome != plurora_runtime::RightsPolicyOutcome::Allowed {
                return Ok(Err(vec![RealizationPlanningGap {
                    reason_code: rights_gap_reason(outcome).to_string(),
                    next_step: "review copy_across_hosts Rights before planning artifact transfer to another Host"
                        .to_string(),
                    workload_id: None,
                    target_id: Some(request.target_id.clone()),
                }]));
            }
        }
        let intent_ref = artifacts
            .work
            .operational_intent
            .clone()
            .ok_or_else(|| anyhow!("target_unsatisfied: Work has no OperationalIntent"))?;
        let intent = self.load_model::<OperationalIntent>(&intent_ref).await?;
        if intent
            .annotations
            .get("plurora.intent/dedicated_server")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        {
            let rights =
                load_rights_declaration(self.object_store.as_ref(), &artifacts.work).await?;
            let outcome = rights_policy_outcome(rights.as_ref(), RightsOperation::DedicatedServer);
            if outcome != plurora_runtime::RightsPolicyOutcome::Allowed {
                return Ok(Err(vec![RealizationPlanningGap {
                    reason_code: rights_gap_reason(outcome).to_string(),
                    next_step:
                        "review the dedicated_server Right before planning this server workload"
                            .to_string(),
                    workload_id: None,
                    target_id: Some(request.target_id.clone()),
                }]));
            }
        }
        if intent.workloads.len() != 1 || request.backends.len() != 1 {
            return Ok(Err(vec![RealizationPlanningGap {
                reason_code: "unsupported_backend".to_string(),
                next_step: "the first Realization executor supports exactly one workload; split this intent or use a future multi-workload executor".to_string(),
                workload_id: None,
                target_id: Some(request.target_id.clone()),
            }]));
        }
        let root_lock = artifacts
            .locks
            .get(&artifacts.installation.record.assembly_lock.digest)
            .cloned()
            .ok_or_else(|| {
                anyhow!("artifact_missing: verified root AssemblyLock is unavailable")
            })?;
        let by_workload = intent
            .workloads
            .iter()
            .map(|workload| (workload.workload_id.as_str(), workload))
            .collect::<BTreeMap<_, _>>();
        let mut selected = BTreeSet::new();
        let mut route_ids = BTreeSet::new();
        let mut workloads: Vec<PlannedWorkloadInput> = Vec::new();
        for backend in &request.backends {
            let workload = by_workload
                .get(backend.workload_id())
                .ok_or_else(|| anyhow!("work_invalid: backend selects an unknown workload"))?;
            ensure!(
                selected.insert(backend.workload_id()),
                "work_invalid: a workload has more than one backend selection"
            );
            let route_id = match backend {
                RealizationBackendSelection::OciImage(selection) => &selection.route_id,
                RealizationBackendSelection::DockerBuild(selection) => &selection.route_id,
            };
            ensure!(
                route_ids.insert(route_id.as_str()),
                "work_invalid: backend selections must use distinct route ids"
            );
            workloads.push(
                prepare_backend_artifacts(
                    self.object_store.as_ref(),
                    backend,
                    &workload.node_id,
                    &request.target_id,
                )
                .await?,
            );
        }
        let (inventory_ref, inventory) =
            persist_target_inventory(self.object_store.as_ref(), &target).await?;
        let input = plurora_work::RealizationPlannerInput {
            installation_id: request.installation_id.clone(),
            work_revision: artifacts.installation.record.work_revision.clone(),
            assembly_lock_ref: artifacts.installation.record.assembly_lock.clone(),
            operational_intent_ref: intent_ref,
            inventory_refs: vec![inventory_ref.clone()],
            intent,
            assembly_lock: root_lock,
            inventories: vec![inventory],
            workloads,
            preconditions: plan_preconditions(&artifacts, &target, &inventory_ref),
            required_authority: vec![
                "realization.apply".to_string(),
                format!("host/installation/{}", request.installation_id),
                format!("host/target/{}", request.target_id),
            ],
            risk_summary: risk_summary(&request.backends),
        };
        let plan = match compile_realization_plan(input) {
            Ok(plan) => plan,
            Err(gaps) => return Ok(Err(gaps)),
        };
        let plan_ref = put_json_artifact(
            self.object_store.as_ref(),
            REALIZATION_PLAN_TYPE_URI,
            &plan,
            plan.referenced_artifacts()
                .iter()
                .map(|descriptor| descriptor.digest.clone())
                .collect(),
            BTreeMap::from([(
                "installation_id".to_string(),
                json!(request.installation_id),
            )]),
        )
        .await?;
        let now = Utc::now();
        let realization_id = RealizationId::new();
        let revision = RealizationRevision {
            realization_id,
            installation_id: request.installation_id.clone(),
            revision: 1,
            plan_ref,
            parent_realization_id: None,
            status: RealizationStatus::Planned,
            actual_resources: Vec::new(),
            receipts: Vec::new(),
            health: RealizationHealth {
                status: HealthStatus::Unknown,
                reason_code: None,
                evidence_refs: Vec::new(),
            },
            created_at: now,
            updated_at: now,
            activated_at: None,
            stopped_at: None,
        };
        revision
            .validate()
            .map_err(|error| anyhow!(error.to_string()))?;
        Ok(Ok((
            plan,
            RealizationRecord {
                revision,
                target_id: request.target_id.clone(),
                backends: request.backends.clone(),
                plan_idempotency_key: hash_text(&request.idempotency_key),
                plan_fingerprint: fingerprint(request)?,
            },
        )))
    }

    async fn load_model<T: serde::de::DeserializeOwned + ArtifactModel>(
        &self,
        descriptor: &plurora_core::ArtifactDescriptor,
    ) -> anyhow::Result<T> {
        ensure!(
            descriptor.artifact_type_uri == T::ARTIFACT_TYPE_URI,
            "artifact_digest_mismatch: artifact type is invalid"
        );
        let info = self.object_store.verify(&descriptor.digest).await?;
        ensure!(
            info.size_bytes == descriptor.size_bytes,
            "artifact_digest_mismatch: artifact size is stale"
        );
        let bytes = self.object_store.get(&descriptor.digest).await?;
        let value: T = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow!("work_invalid: artifact payload is malformed"))?;
        value
            .validate()
            .map_err(|error| anyhow!(error.to_string()))?;
        Ok(value)
    }

    async fn verify_apply_preconditions(
        &self,
        record: &RealizationRecord,
        plan_ref: &plurora_core::ArtifactDescriptor,
    ) -> anyhow::Result<RealizationPlan> {
        ensure!(
            &record.revision.plan_ref == plan_ref,
            "plan_digest_mismatch: request does not identify the persisted plan"
        );
        let plan = self.load_model::<RealizationPlan>(plan_ref).await?;
        ensure!(
            plan.installation_id == record.revision.installation_id,
            "plan_stale: plan Installation differs from its authority record"
        );
        let artifacts = self
            .installations
            .inspect_current_ready_for_run(&record.revision.installation_id)
            .await?;
        let expected_installation_revision = plan
            .preconditions
            .iter()
            .find(|precondition| precondition.kind == "installation_revision")
            .and_then(|precondition| precondition.expected.as_u64())
            .ok_or_else(|| anyhow!("plan_stale: plan lacks an Installation revision"))?;
        ensure!(
            artifacts.installation.revision == expected_installation_revision
                && artifacts.installation.record.work_revision == plan.work_revision
                && artifacts.installation.record.assembly_lock == plan.assembly_lock
                && artifacts.work.operational_intent.as_ref() == Some(&plan.operational_intent),
            "plan_stale: Installation Work, AssemblyLock, or OperationalIntent changed"
        );
        let target = self
            .targets
            .status(&record.target_id)
            .await
            .ok_or_else(|| anyhow!("target_unsatisfied: selected Target disappeared"))?;
        ensure_target_available(&target)?;
        let expected_epoch = plan
            .preconditions
            .iter()
            .find(|precondition| precondition.kind == "target_authority_epoch")
            .and_then(|precondition| {
                Some((
                    precondition.expected.get("lease")?.as_u64()?,
                    precondition.expected.get("policy")?.as_u64()?,
                ))
            })
            .ok_or_else(|| anyhow!("plan_stale: plan lacks a Target authority epoch"))?;
        ensure!(
            expected_epoch == (target.lease_epoch, target.policy_epoch),
            "plan_stale: Target lease or policy epoch changed"
        );
        Ok(plan)
    }

    async fn persist_receipts(
        &self,
        receipts: Vec<RealizationEffectReceipt>,
    ) -> anyhow::Result<Vec<plurora_core::ArtifactDescriptor>> {
        let mut descriptors = Vec::with_capacity(receipts.len());
        for receipt in receipts {
            descriptors.push(
                put_json_artifact(
                    self.object_store.as_ref(),
                    "urn:plurora:realization-effect-receipt:v1",
                    &receipt,
                    receipt
                        .diagnostic_ref
                        .iter()
                        .map(|descriptor| descriptor.digest.clone())
                        .collect(),
                    BTreeMap::from([("realization_id".to_string(), json!(receipt.realization_id))]),
                )
                .await?,
            );
        }
        Ok(descriptors)
    }

    async fn checkpoint_apply_effect(
        &self,
        record: &RealizationRecord,
        execution: RealizationExecution,
        operation: &str,
    ) -> anyhow::Result<RealizationRecord> {
        let mut checkpoint = record.clone();
        checkpoint.revision = transition(
            &record.revision,
            RealizationStatus::Applying,
            execution.resources,
            record.revision.receipts.clone(),
            Some("effect_completed_pending_commit"),
        );
        let _mutation = self.mutation.lock().await;
        self.sync_journal().await?;
        let current = self.current(&record.revision.realization_id).await?;
        ensure!(
            current.revision.revision == record.revision.revision,
            "realization_revision_conflict: apply checkpoint lost its parent revision"
        );
        self.append_effect_checkpoint(
            PRIVATE_EVENT_REALIZATION_EFFECT_APPLIED,
            checkpoint.clone(),
            operation,
            execution.receipts,
        )
        .await?;
        Ok(checkpoint)
    }

    async fn commit_apply_checkpoint(
        &self,
        record: RealizationRecord,
        authority: &RealizationMutationAuthority,
        subject: &RealizationAuthoritySubject,
        operation: &str,
        event_kind: &str,
        replayed: bool,
    ) -> anyhow::Result<RealizationMutationResult> {
        let pending_receipts = self
            .projection
            .read()
            .await
            .pending_apply_receipts
            .get(&record.revision.realization_id)
            .cloned()
            .unwrap_or_default();
        let mut receipts = record.revision.receipts.clone();
        receipts.extend(self.persist_receipts(pending_receipts).await?);
        let mut next = record.clone();
        next.revision = transition(
            &record.revision,
            RealizationStatus::Active,
            record.revision.actual_resources.clone(),
            receipts,
            None,
        );
        let _mutation = self.mutation.lock().await;
        self.sync_journal().await?;
        let current = self.current(&record.revision.realization_id).await?;
        ensure!(
            current.revision.revision == record.revision.revision,
            "realization_revision_conflict: apply completion lost its checkpoint"
        );
        self.append(
            event_kind,
            next.clone(),
            operation,
            None,
            None,
            Some((authority, subject)),
        )
        .await?;
        Ok(RealizationMutationResult {
            realization: next.revision,
            gaps: Vec::new(),
            replayed,
        })
    }

    async fn resume_apply_checkpoint(
        &self,
        realization_id: &RealizationId,
        authority: &RealizationMutationAuthority,
        subject: &RealizationAuthoritySubject,
        operation: &str,
        event_kind: &str,
    ) -> anyhow::Result<RealizationMutationResult> {
        let effect_lock = self.effect_lock(realization_id)?;
        let _effect = effect_lock.lock().await;
        let record = {
            let _mutation = self.mutation.lock().await;
            self.sync_journal().await?;
            self.current(realization_id).await?
        };
        if record.revision.status != RealizationStatus::Applying {
            return Ok(RealizationMutationResult {
                realization: record.revision,
                gaps: Vec::new(),
                replayed: true,
            });
        }
        ensure!(
            self.projection
                .read()
                .await
                .apply_checkpoints
                .contains(realization_id),
            "recovery_required: apply has no durable effect checkpoint"
        );
        self.commit_apply_checkpoint(record, authority, subject, operation, event_kind, true)
            .await
    }

    async fn execute_apply(
        &self,
        record: RealizationRecord,
        plan: RealizationPlan,
        authority: RealizationMutationAuthority,
        subject: RealizationAuthoritySubject,
    ) -> anyhow::Result<RealizationMutationResult> {
        let effect_lock = self.effect_lock(&record.revision.realization_id)?;
        let _effect = effect_lock.lock().await;
        let result = self
            .driver()
            .apply(
                &record.revision,
                &record.target_id,
                &plan,
                &record.backends,
                &authority,
                &subject,
            )
            .await;
        match result {
            Ok(execution) => {
                let checkpoint = self
                    .checkpoint_apply_effect(&record, execution, OP_APPLY)
                    .await?;
                self.commit_apply_checkpoint(
                    checkpoint,
                    &authority,
                    &subject,
                    OP_APPLY,
                    EVENT_REALIZATION_ACTIVE,
                    false,
                )
                .await
            }
            Err(error) => {
                let partial = error.downcast_ref::<PartialRealizationExecution>();
                let (status, reason, resources, mut receipts) = if let Some(partial) = partial {
                    (
                        if partial.reason_code == "outcome_unknown" {
                            RealizationStatus::OutcomeUnknown
                        } else {
                            RealizationStatus::RecoveryRequired
                        },
                        partial.reason_code,
                        partial.resources.clone(),
                        self.persist_receipts(partial.receipts.clone()).await?,
                    )
                } else if is_managed_target_workload_outcome_unknown(&error)
                    || error.to_string().contains("outcome_unknown")
                {
                    (
                        RealizationStatus::OutcomeUnknown,
                        "outcome_unknown",
                        Vec::new(),
                        Vec::new(),
                    )
                } else {
                    (
                        RealizationStatus::Failed,
                        stable_reason(&error),
                        Vec::new(),
                        Vec::new(),
                    )
                };
                receipts.splice(0..0, record.revision.receipts.clone());
                let mut next = record.clone();
                next.revision =
                    transition(&record.revision, status, resources, receipts, Some(reason));
                let _mutation = self.mutation.lock().await;
                self.sync_journal().await?;
                let current = self.current(&record.revision.realization_id).await?;
                ensure!(
                    current.revision.revision == record.revision.revision,
                    "realization_revision_conflict: apply failure lost its parent revision"
                );
                self.append(
                    EVENT_REALIZATION_FAILED,
                    next.clone(),
                    OP_APPLY,
                    Some(reason.to_string()),
                    None,
                    Some((&authority, &subject)),
                )
                .await?;
                Ok(RealizationMutationResult {
                    realization: next.revision,
                    gaps: vec![gap(reason, recovery_step(reason))],
                    replayed: false,
                })
            }
        }
    }

    async fn checkpoint_stop_effect(
        &self,
        record: &RealizationRecord,
        effect_receipts: Vec<RealizationEffectReceipt>,
        operation: &str,
    ) -> anyhow::Result<RealizationRecord> {
        let mut checkpoint = record.clone();
        checkpoint.revision = transition(
            &record.revision,
            RealizationStatus::Stopping,
            record.revision.actual_resources.clone(),
            record.revision.receipts.clone(),
            Some("effect_completed_pending_commit"),
        );
        let _mutation = self.mutation.lock().await;
        self.sync_journal().await?;
        let current = self.current(&record.revision.realization_id).await?;
        ensure!(
            current.revision.revision == record.revision.revision,
            "realization_revision_conflict: stop checkpoint lost its parent revision"
        );
        self.append_effect_checkpoint(
            PRIVATE_EVENT_REALIZATION_EFFECT_STOPPED,
            checkpoint.clone(),
            operation,
            effect_receipts,
        )
        .await?;
        Ok(checkpoint)
    }

    async fn commit_stop_checkpoint(
        &self,
        record: RealizationRecord,
        authority: Option<(&RealizationMutationAuthority, &RealizationAuthoritySubject)>,
        operation: &str,
        reason: Option<&str>,
    ) -> anyhow::Result<RealizationRecord> {
        let pending_receipts = self
            .projection
            .read()
            .await
            .pending_stop_receipts
            .get(&record.revision.realization_id)
            .cloned()
            .unwrap_or_default();
        let mut receipts = record.revision.receipts.clone();
        receipts.extend(self.persist_receipts(pending_receipts).await?);
        let mut next = record.clone();
        next.revision = transition(
            &record.revision,
            RealizationStatus::Stopped,
            Vec::new(),
            receipts,
            reason,
        );
        let _mutation = self.mutation.lock().await;
        self.sync_journal().await?;
        let current = self.current(&record.revision.realization_id).await?;
        ensure!(
            current.revision.revision == record.revision.revision,
            "realization_revision_conflict: stop completion lost its checkpoint"
        );
        self.append(
            EVENT_REALIZATION_STOPPED,
            next.clone(),
            operation,
            reason.map(str::to_string),
            None,
            authority,
        )
        .await?;
        Ok(next)
    }

    async fn complete_stop(
        &self,
        record: RealizationRecord,
        authority: RealizationMutationAuthority,
        subject: RealizationAuthoritySubject,
        replayed: bool,
    ) -> anyhow::Result<RealizationMutationResult> {
        let realization_id = record.revision.realization_id.clone();
        let effect_lock = self.effect_lock(&realization_id)?;
        let _effect = effect_lock.lock().await;
        let mut record = {
            let _mutation = self.mutation.lock().await;
            self.sync_journal().await?;
            self.current(&realization_id).await?
        };
        if record.revision.status == RealizationStatus::Stopped {
            return Ok(RealizationMutationResult {
                realization: record.revision,
                gaps: Vec::new(),
                replayed: true,
            });
        }
        ensure!(
            record.revision.status == RealizationStatus::Stopping,
            "realization_revision_conflict: stop no longer owns a Stopping revision"
        );
        let checkpointed = self
            .projection
            .read()
            .await
            .stop_checkpoints
            .contains(&realization_id);
        if !checkpointed {
            let receipts = self
                .driver()
                .stop(&record.revision, &record.target_id, &authority, &subject)
                .await?;
            record = self
                .checkpoint_stop_effect(&record, receipts, OP_STOP)
                .await?;
        }
        let next = self
            .commit_stop_checkpoint(record, Some((&authority, &subject)), OP_STOP, None)
            .await?;
        Ok(RealizationMutationResult {
            realization: next.revision,
            gaps: Vec::new(),
            replayed,
        })
    }

    async fn complete_rollback_checkpoint_locked(
        &self,
        replacement: RealizationRecord,
        authority: &RealizationMutationAuthority,
        subject: &RealizationAuthoritySubject,
        replayed: bool,
    ) -> anyhow::Result<RealizationMutationResult> {
        let parent_id = replacement
            .revision
            .parent_realization_id
            .clone()
            .ok_or_else(|| anyhow!("recovery_required: rollback replacement has no parent"))?;
        let parent_effect_lock = self.effect_lock(&parent_id)?;
        let _parent_effect = parent_effect_lock.lock().await;
        let mut parent = {
            let _mutation = self.mutation.lock().await;
            self.sync_journal().await?;
            self.current(&parent_id).await?
        };
        if matches!(
            parent.revision.status,
            RealizationStatus::Active | RealizationStatus::Degraded
        ) {
            let mut stopping = parent.clone();
            stopping.revision = transition(
                &parent.revision,
                RealizationStatus::Stopping,
                parent.revision.actual_resources.clone(),
                parent.revision.receipts.clone(),
                Some("rollback_pending"),
            );
            let _mutation = self.mutation.lock().await;
            self.sync_journal().await?;
            let current = self.current(&parent_id).await?;
            ensure!(
                current.revision.revision == parent.revision.revision,
                "realization_revision_conflict: rollback parent changed before stop"
            );
            self.append(
                PRIVATE_EVENT_REALIZATION_STOPPING,
                stopping.clone(),
                OP_ROLLBACK,
                Some("rollback_pending".to_string()),
                None,
                Some((authority, subject)),
            )
            .await?;
            parent = stopping;
        }
        if parent.revision.status == RealizationStatus::Stopping {
            let checkpointed = self
                .projection
                .read()
                .await
                .stop_checkpoints
                .contains(&parent_id);
            if !checkpointed {
                let receipts = self
                    .driver()
                    .stop(&parent.revision, &parent.target_id, authority, subject)
                    .await?;
                parent = self
                    .checkpoint_stop_effect(&parent, receipts, OP_ROLLBACK)
                    .await?;
            }
            parent = self
                .commit_stop_checkpoint(
                    parent,
                    Some((authority, subject)),
                    OP_ROLLBACK,
                    Some("rolled_back"),
                )
                .await?;
        }
        ensure!(
            parent.revision.status == RealizationStatus::Stopped,
            "recovery_required: rollback parent did not reach Stopped"
        );
        self.commit_apply_checkpoint(
            replacement,
            authority,
            subject,
            OP_ROLLBACK,
            EVENT_REALIZATION_ROLLED_BACK,
            replayed,
        )
        .await
    }

    async fn resume_rollback_checkpoint(
        &self,
        realization_id: &RealizationId,
        authority: &RealizationMutationAuthority,
        subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<RealizationMutationResult> {
        let effect_lock = self.effect_lock(realization_id)?;
        let _effect = effect_lock.lock().await;
        let replacement = {
            let _mutation = self.mutation.lock().await;
            self.sync_journal().await?;
            self.current(realization_id).await?
        };
        if replacement.revision.status != RealizationStatus::Applying {
            return Ok(RealizationMutationResult {
                realization: replacement.revision,
                gaps: Vec::new(),
                replayed: true,
            });
        }
        ensure!(
            self.projection
                .read()
                .await
                .apply_checkpoints
                .contains(realization_id),
            "recovery_required: rollback has no durable effect checkpoint"
        );
        self.complete_rollback_checkpoint_locked(replacement, authority, subject, true)
            .await
    }

    async fn execute_rollback(
        &self,
        replacement: RealizationRecord,
        plan: RealizationPlan,
        authority: RealizationMutationAuthority,
        subject: RealizationAuthoritySubject,
    ) -> anyhow::Result<RealizationMutationResult> {
        let realization_id = replacement.revision.realization_id.clone();
        let effect_lock = self.effect_lock(&realization_id)?;
        let _effect = effect_lock.lock().await;
        let result = self
            .driver()
            .apply(
                &replacement.revision,
                &replacement.target_id,
                &plan,
                &replacement.backends,
                &authority,
                &subject,
            )
            .await;
        let execution = match result {
            Ok(execution) => execution,
            Err(error) => {
                let partial = error.downcast_ref::<PartialRealizationExecution>();
                let (status, reason, resources, mut receipts) = if let Some(partial) = partial {
                    (
                        if partial.reason_code == "outcome_unknown" {
                            RealizationStatus::OutcomeUnknown
                        } else {
                            RealizationStatus::RecoveryRequired
                        },
                        partial.reason_code,
                        partial.resources.clone(),
                        self.persist_receipts(partial.receipts.clone()).await?,
                    )
                } else if is_managed_target_workload_outcome_unknown(&error)
                    || error.to_string().contains("outcome_unknown")
                {
                    (
                        RealizationStatus::OutcomeUnknown,
                        "outcome_unknown",
                        Vec::new(),
                        Vec::new(),
                    )
                } else {
                    (
                        RealizationStatus::Failed,
                        stable_reason(&error),
                        Vec::new(),
                        Vec::new(),
                    )
                };
                receipts.splice(0..0, replacement.revision.receipts.clone());
                let mut failed = replacement.clone();
                failed.revision = transition(
                    &replacement.revision,
                    status,
                    resources,
                    receipts,
                    Some(reason),
                );
                let _mutation = self.mutation.lock().await;
                self.sync_journal().await?;
                let current = self.current(&realization_id).await?;
                ensure!(
                    current.revision.revision == replacement.revision.revision,
                    "realization_revision_conflict: rollback failure lost its parent revision"
                );
                self.append(
                    EVENT_REALIZATION_FAILED,
                    failed.clone(),
                    OP_ROLLBACK,
                    Some(reason.to_string()),
                    None,
                    Some((&authority, &subject)),
                )
                .await?;
                return Ok(RealizationMutationResult {
                    realization: failed.revision,
                    gaps: vec![gap(reason, recovery_step(reason))],
                    replayed: false,
                });
            }
        };
        let checkpoint = self
            .checkpoint_apply_effect(&replacement, execution, OP_ROLLBACK)
            .await?;
        self.complete_rollback_checkpoint_locked(checkpoint, &authority, &subject, false)
            .await
    }
}

#[async_trait]
impl RealizationControl for RealizationRegistry {
    async fn plan(
        &self,
        request: RealizationPlanRequest,
        authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationPlanResult> {
        self.relay_pending().await?;
        let subject = RealizationAuthoritySubject::Plan {
            installation_id: request.installation_id.clone(),
            target_id: request.target_id.clone(),
        };
        authority.refresh_current_for(&subject).await?;
        let request_fingerprint = fingerprint(&request)?;
        {
            let _mutation = self.mutation.lock().await;
            self.sync_journal().await?;
            if let Some(record) = self
                .replay(OP_PLAN, &request.idempotency_key, &request_fingerprint)
                .await?
            {
                let plan = self
                    .load_model::<RealizationPlan>(&record.revision.plan_ref)
                    .await?;
                return Ok(RealizationPlanResult {
                    plan_ref: Some(record.revision.plan_ref.clone()),
                    realization: Some(record.revision),
                    plan: Some(plan),
                    gaps: Vec::new(),
                    replayed: true,
                });
            }
        }
        let prepared = self.prepare_plan(&request).await?;
        let Ok((plan, record)) = prepared else {
            return Ok(RealizationPlanResult {
                realization: None,
                plan_ref: None,
                plan: None,
                gaps: prepared.err().unwrap_or_default(),
                replayed: false,
            });
        };
        let _mutation = self.mutation.lock().await;
        self.sync_journal().await?;
        if let Some(existing) = self
            .replay(OP_PLAN, &request.idempotency_key, &request_fingerprint)
            .await?
        {
            let existing_plan = self
                .load_model::<RealizationPlan>(&existing.revision.plan_ref)
                .await?;
            return Ok(RealizationPlanResult {
                plan_ref: Some(existing.revision.plan_ref.clone()),
                realization: Some(existing.revision),
                plan: Some(existing_plan),
                gaps: Vec::new(),
                replayed: true,
            });
        }
        self.append(
            EVENT_REALIZATION_PLANNED,
            record.clone(),
            OP_PLAN,
            None,
            Some((&request.idempotency_key, &request_fingerprint)),
            Some((&authority, &subject)),
        )
        .await?;
        Ok(RealizationPlanResult {
            realization: Some(record.revision.clone()),
            plan_ref: Some(record.revision.plan_ref),
            plan: Some(plan),
            gaps: Vec::new(),
            replayed: false,
        })
    }

    async fn apply(
        &self,
        request: RealizationApplyRequest,
        authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationMutationResult> {
        self.relay_pending().await?;
        validate_key(&request.idempotency_key)?;
        let subject = RealizationAuthoritySubject::Apply {
            installation_id: request.installation_id.clone(),
            target_id: request.target_id.clone(),
            realization_ids: vec![request.realization_id.clone()],
        };
        authority.refresh_current_for(&subject).await?;
        let request_fingerprint = fingerprint(&request)?;
        {
            let _mutation = self.mutation.lock().await;
            self.sync_journal().await?;
            if let Some(record) = self
                .replay(OP_APPLY, &request.idempotency_key, &request_fingerprint)
                .await?
            {
                let has_checkpoint = record.revision.status == RealizationStatus::Applying
                    && self
                        .projection
                        .read()
                        .await
                        .apply_checkpoints
                        .contains(&record.revision.realization_id);
                if has_checkpoint {
                    let realization_id = record.revision.realization_id.clone();
                    drop(_mutation);
                    return self
                        .resume_apply_checkpoint(
                            &realization_id,
                            &authority,
                            &subject,
                            OP_APPLY,
                            EVENT_REALIZATION_ACTIVE,
                        )
                        .await;
                }
                return Ok(RealizationMutationResult {
                    realization: record.revision,
                    gaps: Vec::new(),
                    replayed: true,
                });
            }
        }
        let record = self.current(&request.realization_id).await?;
        validate_record_request(&record, &request.installation_id, &request.target_id)?;
        ensure!(
            record.revision.revision == request.expected_revision
                && record.revision.status == RealizationStatus::Planned,
            "realization_revision_conflict: apply requires the current Planned revision"
        );
        let plan = self
            .verify_apply_preconditions(&record, &request.plan_ref)
            .await?;
        validate_approval(&request.approval, &plan, &request.plan_ref.digest)?;
        let approval_ref = put_json_artifact(
            self.object_store.as_ref(),
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
        {
            let _mutation = self.mutation.lock().await;
            self.sync_journal().await?;
            ensure!(
                self.current(&request.realization_id)
                    .await?
                    .revision
                    .revision
                    == request.expected_revision,
                "realization_revision_conflict: apply parent changed"
            );
            self.append(
                EVENT_REALIZATION_APPLYING,
                applying.clone(),
                OP_APPLY,
                None,
                Some((&request.idempotency_key, &request_fingerprint)),
                Some((&authority, &subject)),
            )
            .await?;
        }
        self.execute_apply(applying, plan, authority, subject).await
    }

    async fn list(
        &self,
        request: RealizationListRequest,
    ) -> anyhow::Result<Vec<RealizationRevision>> {
        self.relay_pending().await?;
        self.sync_journal().await?;
        let mut values = self
            .projection
            .read()
            .await
            .records
            .values()
            .filter(|record| {
                request
                    .installation_id
                    .as_ref()
                    .is_none_or(|id| id == &record.revision.installation_id)
                    && request
                        .target_id
                        .as_ref()
                        .is_none_or(|id| id == &record.target_id)
            })
            .map(|record| record.revision.clone())
            .collect::<Vec<_>>();
        values.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.realization_id.cmp(&right.realization_id))
        });
        Ok(values)
    }

    async fn get(
        &self,
        request: RealizationGetRequest,
    ) -> anyhow::Result<Option<RealizationRevision>> {
        self.relay_pending().await?;
        self.sync_journal().await?;
        Ok(self
            .projection
            .read()
            .await
            .records
            .get(&request.realization_id)
            .filter(|record| record.revision.installation_id == request.installation_id)
            .map(|record| record.revision.clone()))
    }

    async fn stop(
        &self,
        request: RealizationStopRequest,
        authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationMutationResult> {
        self.relay_pending().await?;
        validate_key(&request.idempotency_key)?;
        let subject = RealizationAuthoritySubject::Apply {
            installation_id: request.installation_id.clone(),
            target_id: request.target_id.clone(),
            realization_ids: vec![request.realization_id.clone()],
        };
        authority.refresh_current_for(&subject).await?;
        let request_fingerprint = fingerprint(&request)?;
        {
            let _mutation = self.mutation.lock().await;
            self.sync_journal().await?;
            if let Some(record) = self
                .replay(OP_STOP, &request.idempotency_key, &request_fingerprint)
                .await?
            {
                if record.revision.status == RealizationStatus::Stopping {
                    drop(_mutation);
                    return self.complete_stop(record, authority, subject, true).await;
                }
                return Ok(RealizationMutationResult {
                    realization: record.revision,
                    gaps: Vec::new(),
                    replayed: true,
                });
            }
        }
        let record = self.current(&request.realization_id).await?;
        validate_record_request(&record, &request.installation_id, &request.target_id)?;
        ensure!(
            record.revision.revision == request.expected_revision
                && matches!(
                    record.revision.status,
                    RealizationStatus::Active | RealizationStatus::Degraded
                ),
            "realization_revision_conflict: stop requires the current active revision"
        );
        let mut stopping = record.clone();
        stopping.revision = transition(
            &record.revision,
            RealizationStatus::Stopping,
            record.revision.actual_resources.clone(),
            record.revision.receipts.clone(),
            None,
        );
        {
            let _mutation = self.mutation.lock().await;
            self.sync_journal().await?;
            self.append(
                PRIVATE_EVENT_REALIZATION_STOPPING,
                stopping.clone(),
                OP_STOP,
                None,
                Some((&request.idempotency_key, &request_fingerprint)),
                Some((&authority, &subject)),
            )
            .await?;
        }
        self.complete_stop(stopping, authority, subject, false)
            .await
    }

    async fn rollback(
        &self,
        request: RealizationRollbackRequest,
        authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationMutationResult> {
        self.relay_pending().await?;
        validate_key(&request.idempotency_key)?;
        let subject = RealizationAuthoritySubject::Apply {
            installation_id: request.installation_id.clone(),
            target_id: request.target_id.clone(),
            realization_ids: vec![
                request.realization_id.clone(),
                request.rollback_to_realization_id.clone(),
            ],
        };
        authority.refresh_current_for(&subject).await?;
        let request_fingerprint = fingerprint(&request)?;
        {
            let _mutation = self.mutation.lock().await;
            self.sync_journal().await?;
            if let Some(record) = self
                .replay(OP_ROLLBACK, &request.idempotency_key, &request_fingerprint)
                .await?
            {
                let has_checkpoint = record.revision.status == RealizationStatus::Applying
                    && self
                        .projection
                        .read()
                        .await
                        .apply_checkpoints
                        .contains(&record.revision.realization_id);
                if has_checkpoint {
                    let realization_id = record.revision.realization_id.clone();
                    drop(_mutation);
                    return self
                        .resume_rollback_checkpoint(&realization_id, &authority, &subject)
                        .await;
                }
                return Ok(RealizationMutationResult {
                    realization: record.revision,
                    gaps: Vec::new(),
                    replayed: true,
                });
            }
        }
        let current = self.current(&request.realization_id).await?;
        let historic = self.current(&request.rollback_to_realization_id).await?;
        validate_record_request(&current, &request.installation_id, &request.target_id)?;
        validate_record_request(&historic, &request.installation_id, &request.target_id)?;
        ensure!(
            current.revision.revision == request.expected_revision
                && matches!(
                    current.revision.status,
                    RealizationStatus::Active | RealizationStatus::Degraded
                ),
            "realization_revision_conflict: rollback parent is stale"
        );
        let plan = self
            .verify_apply_preconditions(&historic, &historic.revision.plan_ref)
            .await?;
        validate_approval(&request.approval, &plan, &historic.revision.plan_ref.digest)?;
        let approval_ref = put_json_artifact(
            self.object_store.as_ref(),
            planner::REALIZATION_APPROVAL_TYPE_URI,
            &request.approval,
            vec![historic.revision.plan_ref.digest.clone()],
            BTreeMap::new(),
        )
        .await?;
        let now = Utc::now();
        let mut replacement = historic.clone();
        replacement.revision = RealizationRevision {
            realization_id: RealizationId::new(),
            installation_id: request.installation_id.clone(),
            revision: 1,
            plan_ref: historic.revision.plan_ref.clone(),
            parent_realization_id: Some(current.revision.realization_id.clone()),
            status: RealizationStatus::Applying,
            actual_resources: Vec::new(),
            receipts: vec![approval_ref],
            health: RealizationHealth {
                status: HealthStatus::Unknown,
                reason_code: None,
                evidence_refs: Vec::new(),
            },
            created_at: now,
            updated_at: now,
            activated_at: None,
            stopped_at: None,
        };
        {
            let _mutation = self.mutation.lock().await;
            self.sync_journal().await?;
            let current_now = self.current(&request.realization_id).await?;
            ensure!(
                current_now.revision.revision == request.expected_revision
                    && matches!(
                        current_now.revision.status,
                        RealizationStatus::Active | RealizationStatus::Degraded
                    ),
                "realization_revision_conflict: rollback parent changed before reservation"
            );
            ensure!(
                !self
                    .projection
                    .read()
                    .await
                    .records
                    .values()
                    .any(|record| {
                        record.revision.parent_realization_id.as_ref()
                            == Some(&request.realization_id)
                            && record.revision.status == RealizationStatus::Applying
                    }),
                "realization_revision_conflict: rollback parent already has an applying replacement"
            );
            self.append(
                EVENT_REALIZATION_APPLYING,
                replacement.clone(),
                OP_ROLLBACK,
                None,
                Some((&request.idempotency_key, &request_fingerprint)),
                Some((&authority, &subject)),
            )
            .await?;
        }
        self.execute_rollback(replacement, plan, authority, subject)
            .await
    }

    async fn reconcile(
        &self,
        request: RealizationReconcileRequest,
        authority: RealizationMutationAuthority,
    ) -> anyhow::Result<RealizationMutationResult> {
        self.relay_pending().await?;
        validate_key(&request.idempotency_key)?;
        let subject = RealizationAuthoritySubject::Apply {
            installation_id: request.installation_id.clone(),
            target_id: request.target_id.clone(),
            realization_ids: vec![request.realization_id.clone()],
        };
        authority.refresh_current_for(&subject).await?;
        let request_fingerprint = fingerprint(&request)?;
        {
            let _mutation = self.mutation.lock().await;
            self.sync_journal().await?;
            if let Some(record) = self
                .replay(OP_RECONCILE, &request.idempotency_key, &request_fingerprint)
                .await?
            {
                return Ok(RealizationMutationResult {
                    realization: record.revision,
                    gaps: Vec::new(),
                    replayed: true,
                });
            }
        }
        let record = self.current(&request.realization_id).await?;
        validate_record_request(&record, &request.installation_id, &request.target_id)?;
        ensure!(
            record.revision.revision == request.expected_revision,
            "realization_revision_conflict: reconcile parent is stale"
        );
        let observation = self
            .driver()
            .observe(&record.revision, &record.target_id)
            .await?;
        let receipt_refs = self.persist_receipts(observation.receipts).await?;
        let mut receipts = record.revision.receipts.clone();
        receipts.extend(receipt_refs);
        let mut next = record.clone();
        next.revision = transition(
            &record.revision,
            observation.status,
            observation.resources,
            receipts,
            observation.reason_code.as_deref(),
        );
        let _mutation = self.mutation.lock().await;
        self.sync_journal().await?;
        self.append(
            EVENT_REALIZATION_RECONCILED,
            next.clone(),
            OP_RECONCILE,
            observation.reason_code,
            Some((&request.idempotency_key, &request_fingerprint)),
            Some((&authority, &subject)),
        )
        .await?;
        Ok(RealizationMutationResult {
            realization: next.revision,
            gaps: Vec::new(),
            replayed: false,
        })
    }
}

fn validate_envelope(event: &EventEnvelope, expected: u64) -> anyhow::Result<()> {
    ensure!(
        event.session_id == JOURNAL_SESSION
            && event.writer_package_id == JOURNAL_WRITER
            && event.schema_version == JOURNAL_SCHEMA
            && event.sequence == expected,
        "Realization journal identity, schema, or sequence is invalid"
    );
    Ok(())
}

fn is_public_event(kind: &str) -> bool {
    matches!(
        kind,
        EVENT_REALIZATION_PLANNED
            | EVENT_REALIZATION_APPLYING
            | EVENT_REALIZATION_ACTIVE
            | EVENT_REALIZATION_STOPPED
            | EVENT_REALIZATION_FAILED
            | EVENT_REALIZATION_ROLLED_BACK
            | EVENT_REALIZATION_RECONCILED
    )
}

fn plan_preconditions(
    artifacts: &RunInstallationArtifacts,
    target: &ExecutionTarget,
    inventory: &plurora_core::ArtifactDescriptor,
) -> Vec<ChangePrecondition> {
    vec![
        ChangePrecondition {
            kind: "installation_revision".to_string(),
            target: Some(artifacts.installation.record.installation_id.to_string()),
            expected: json!(artifacts.installation.revision),
        },
        ChangePrecondition {
            kind: "work_revision_digest".to_string(),
            target: None,
            expected: json!(artifacts.installation.record.work_revision.digest),
        },
        ChangePrecondition {
            kind: "assembly_lock_digest".to_string(),
            target: None,
            expected: json!(artifacts.installation.record.assembly_lock.digest),
        },
        ChangePrecondition {
            kind: "target_inventory_digest".to_string(),
            target: Some(target.id.clone()),
            expected: json!(inventory.digest),
        },
        ChangePrecondition {
            kind: "target_authority_epoch".to_string(),
            target: Some(target.id.clone()),
            expected: json!({"lease": target.lease_epoch, "policy": target.policy_epoch}),
        },
    ]
}

fn risk_summary(backends: &[RealizationBackendSelection]) -> Vec<String> {
    let mut risks = vec![
        "managed_target_effects".to_string(),
        "network_endpoint".to_string(),
    ];
    if backends
        .iter()
        .any(|backend| matches!(backend, RealizationBackendSelection::DockerBuild(_)))
    {
        risks.push("build_executes_dockerfile".to_string());
    }
    risks.sort();
    risks
}

fn validate_approval(
    approval: &plurora_runtime::RealizationApproval,
    plan: &RealizationPlan,
    plan_digest: &str,
) -> anyhow::Result<()> {
    ensure!(
        approval.decision == "approved"
            && approval.plan_digest == plan_digest
            && approval.decided_at <= Utc::now()
            && approval.expires_at.is_none_or(|expiry| expiry > Utc::now()),
        "approval_required: current explicit approval for this exact plan is required"
    );
    let accepted = approval.accepted_risks.iter().collect::<BTreeSet<_>>();
    ensure!(
        plan.risk_summary.iter().all(|risk| accepted.contains(risk)),
        "approval_required: approval does not accept every declared plan risk"
    );
    Ok(())
}

fn validate_record_request(
    record: &RealizationRecord,
    installation_id: &plurora_work::InstallationId,
    target_id: &str,
) -> anyhow::Result<()> {
    ensure!(
        &record.revision.installation_id == installation_id && record.target_id == target_id,
        "authority_denied: Realization parent resources do not match the request"
    );
    Ok(())
}

fn ensure_target_available(target: &ExecutionTarget) -> anyhow::Result<()> {
    ensure!(
        target.status == ExecutionTargetStatusKind::Available,
        "target_unsatisfied: selected Target is not currently available"
    );
    Ok(())
}

fn transition(
    previous: &RealizationRevision,
    status: RealizationStatus,
    resources: Vec<RealizedResource>,
    receipts: Vec<plurora_core::ArtifactDescriptor>,
    reason: Option<&str>,
) -> RealizationRevision {
    let now = Utc::now();
    let activated_at = if matches!(
        status,
        RealizationStatus::Active
            | RealizationStatus::Degraded
            | RealizationStatus::Stopping
            | RealizationStatus::Stopped
    ) {
        previous.activated_at.or(Some(now))
    } else {
        previous.activated_at
    };
    RealizationRevision {
        realization_id: previous.realization_id.clone(),
        installation_id: previous.installation_id.clone(),
        revision: previous.revision.saturating_add(1),
        plan_ref: previous.plan_ref.clone(),
        parent_realization_id: previous.parent_realization_id.clone(),
        status,
        actual_resources: resources,
        receipts,
        health: RealizationHealth {
            status: match status {
                RealizationStatus::Active => HealthStatus::Healthy,
                RealizationStatus::Degraded | RealizationStatus::RecoveryRequired => {
                    HealthStatus::Degraded
                }
                RealizationStatus::Failed | RealizationStatus::OutcomeUnknown => {
                    HealthStatus::Unhealthy
                }
                _ => HealthStatus::Unknown,
            },
            reason_code: reason.map(str::to_string),
            evidence_refs: Vec::new(),
        },
        created_at: previous.created_at,
        updated_at: now,
        activated_at,
        stopped_at: (status == RealizationStatus::Stopped).then_some(now),
    }
}

fn gap(reason: &str, next_step: &str) -> RealizationPlanningGap {
    RealizationPlanningGap {
        reason_code: reason.to_string(),
        next_step: next_step.to_string(),
        workload_id: None,
        target_id: None,
    }
}

fn recovery_step(reason: &str) -> &'static str {
    match reason {
        "outcome_unknown" => "reconcile Target truth before retrying or rolling back",
        "authority_denied" => "renew exact Realization authority",
        "target_unsatisfied" | "unsupported_backend" => "select a compatible available Target",
        _ => "inspect redacted receipts, repair the cause, then reconcile or roll back",
    }
}

fn stable_reason(error: &anyhow::Error) -> &'static str {
    let text = error.to_string();
    [
        "authority_denied",
        "target_unsatisfied",
        "unsupported_backend",
        "artifact_missing",
        "artifact_digest_mismatch",
        "plan_stale",
        "approval_required",
        "recovery_required",
    ]
    .into_iter()
    .find(|reason| text.contains(reason))
    .unwrap_or("realization_failed")
}

fn validate_key(key: &str) -> anyhow::Result<()> {
    ensure!(!key.trim().is_empty(), "idempotency key is required");
    Ok(())
}

fn fingerprint<T: Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(hash_bytes(
        &canonical_json_bytes(value).map_err(|error| anyhow!(error.to_string()))?,
    ))
}

fn hash_text(value: &str) -> String {
    hash_bytes(value.as_bytes())
}

fn hash_bytes(value: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(value))
}

fn lock_error<T>(error: std::sync::PoisonError<T>) -> anyhow::Error {
    anyhow!("Realization registry lock poisoned: {error}")
}

fn rights_gap_reason(outcome: plurora_runtime::RightsPolicyOutcome) -> &'static str {
    match outcome {
        plurora_runtime::RightsPolicyOutcome::RequiresEntitlement => "entitlement_required",
        plurora_runtime::RightsPolicyOutcome::Unspecified => "rights_unspecified",
        plurora_runtime::RightsPolicyOutcome::Denied => "rights_denied",
        plurora_runtime::RightsPolicyOutcome::Allowed => "rights_allowed",
    }
}

#[cfg(test)]
mod tests;
