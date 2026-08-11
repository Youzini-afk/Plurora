use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock as StdRwLock, Weak};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use plurora_core::{EventEnvelope, EventSequence, PLATFORM_RUNTIME_ID};
use plurora_runtime::{
    runtime_outcome_unknown, BindingAttachError, BindingAttachmentNotice, BindingCandidate,
    BindingCandidatesRequest, BindingCandidatesResult, BindingCleanupNotice,
    BindingCurrentValidationRequest, BindingDecisionStatus, BindingEffectiveStatus,
    BindingEndpointPin, BindingGap, BindingListRequest, BindingMutationResult,
    BindingRevokeRequest, BindingSelectRequest, BindingSelectionRecord, BindingView, CapabilityPin,
    ComponentPin, EventStore, ExposureCreateRequest, ExposureListRequest, ExposureMutationResult,
    ExposureRevokeRequest, ExposureView, InstallationControl, InstallationRevisionPin,
    PowerboxAuthorityBasis, PowerboxAuthorityRefresh, PowerboxAuthoritySubject, PowerboxControl,
    PowerboxEndpointInspection, PowerboxInvalidationResult, PowerboxMutationAuthority,
    ResolvedPortPin, RunBindingPreparation, RunBindingPreparationRequest, RunControl,
    RunGetRequest, RunRevisionPin, Runtime,
};
use plurora_work::{
    check_port_compatibility, select_transport, BindingId, BindingPhase, ExposureId,
    ExposureRecord, ExposureStatus, InstallationId, NodeId, PortDirection, PortEndpoint, PortId,
    PortRole, RunId, RunStatus, TransportPolicy,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, Notify, RwLock};

use crate::development::DevelopmentHostLease;

const POWERBOX_SESSION: &str = "host_powerbox";
const POWERBOX_WRITER: &str = "host/control-plane";
const POWERBOX_SCHEMA: u16 = 1;
const EVENT_EXPOSURE_CREATED: &str = "host/exposure.created";
const EVENT_EXPOSURE_REVOKED: &str = "host/exposure.revoked";
const EVENT_EXPOSURE_EXPIRED: &str = "host/exposure.expired";
const EVENT_BINDING_SELECTED: &str = "host/binding.selected";
const EVENT_BINDING_REVOKED: &str = "host/binding.revoked";
const EVENT_BINDING_EXPIRED: &str = "host/binding.expired";
const PRIVATE_EXPOSURE_CLOSING: &str = "host-private/powerbox.exposure-closing";
const PRIVATE_BINDING_CLOSING: &str = "host-private/powerbox.binding-closing";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExposureProjection {
    view: ExposureView,
    provider: BindingEndpointPin,
    provider_descriptor: plurora_work::PortDescriptor,
    capability: CapabilityPin,
    idempotency_key: String,
    request_fingerprint: String,
    authority_basis: PowerboxAuthorityBasis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BindingProjection {
    view: BindingView,
    idempotency_key: String,
    request_fingerprint: String,
    authority_basis: PowerboxAuthorityBasis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExposureEvent {
    exposure: ExposureProjection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BindingEvent {
    binding: BindingProjection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct MutationClaim {
    idempotency_key: String,
    request_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ExposureClosing {
    exposure_id: ExposureId,
    terminal_status: ExposureStatus,
    reason_code: String,
    affected_binding_ids: Vec<BindingId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    claim: Option<MutationClaim>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct BindingClosing {
    binding_id: BindingId,
    terminal_status: BindingDecisionStatus,
    reason_code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    claim: Option<MutationClaim>,
}

#[derive(Debug, Clone)]
struct Attachment {
    run_id: RunId,
    session_id: String,
    component_activation_id: String,
}

#[cfg(test)]
#[derive(Debug, Default)]
struct TestCloseBarrier {
    entered: std::sync::atomic::AtomicBool,
    released: std::sync::atomic::AtomicBool,
    entered_changed: Notify,
    released_changed: Notify,
}

#[cfg(test)]
impl TestCloseBarrier {
    async fn pause(&self) {
        self.entered
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.entered_changed.notify_waiters();
        loop {
            let notified = self.released_changed.notified();
            if self.released.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            notified.await;
        }
    }

    async fn wait_entered(&self) {
        loop {
            let notified = self.entered_changed.notified();
            if self.entered.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            notified.await;
        }
    }

    fn release(&self) {
        self.released
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.released_changed.notify_waiters();
    }
}

#[derive(Debug, Default)]
struct Projection {
    next_sequence: EventSequence,
    exposures: BTreeMap<ExposureId, ExposureProjection>,
    bindings: BTreeMap<BindingId, BindingProjection>,
    exposure_idempotency: HashMap<String, (String, ExposureId)>,
    binding_idempotency: HashMap<String, (String, BindingId)>,
    attachments: HashMap<BindingId, Attachment>,
    exposure_closing: BTreeMap<ExposureId, ExposureClosing>,
    binding_closing: BTreeMap<BindingId, BindingClosing>,
}

#[async_trait]
pub trait PowerboxEndpointInspector: Send + Sync + 'static {
    async fn inspect(
        &self,
        installation_id: &InstallationId,
        expected_revision: u64,
        run: Option<RunRevisionPin>,
        port: &PortId,
        direction: PortDirection,
    ) -> anyhow::Result<PowerboxEndpointInspection>;
}

pub struct RuntimePowerboxInspector<S>
where
    S: EventStore,
{
    runtime: Weak<Runtime<S>>,
}

impl<S> RuntimePowerboxInspector<S>
where
    S: EventStore,
{
    pub fn new(runtime: Weak<Runtime<S>>) -> Self {
        Self { runtime }
    }
}

#[async_trait]
impl<S> PowerboxEndpointInspector for RuntimePowerboxInspector<S>
where
    S: EventStore,
{
    async fn inspect(
        &self,
        installation_id: &InstallationId,
        expected_revision: u64,
        run: Option<RunRevisionPin>,
        port: &PortId,
        direction: PortDirection,
    ) -> anyhow::Result<PowerboxEndpointInspection> {
        let runtime = self
            .runtime
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Powerbox Runtime inspector is unavailable"))?;
        let artifacts = runtime
            .config()
            .installation_control
            .inspect_current_ready_for_run(installation_id)
            .await?;
        anyhow::ensure!(
            artifacts.installation.revision == expected_revision,
            "stale Installation revision"
        );
        runtime
            .inspect_powerbox_endpoint(&artifacts, run, port, direction)
            .await
    }
}

pub struct PowerboxRegistry {
    store: Arc<dyn EventStore>,
    runtime_public_store: Arc<dyn EventStore>,
    installations: Arc<dyn InstallationControl>,
    runs: Arc<dyn RunControl>,
    projection: RwLock<Projection>,
    mutation: Mutex<()>,
    relay: Mutex<()>,
    owner_lease: StdRwLock<Option<DevelopmentHostLease>>,
    inspector: StdRwLock<Option<Arc<dyn PowerboxEndpointInspector>>>,
    authority_refresh: StdRwLock<Option<PowerboxAuthorityRefresh>>,
    runtime_broker: StdRwLock<Option<Weak<plurora_runtime::RunBindingBroker>>>,
    expiry_changed: Notify,
    #[cfg(test)]
    test_close_barrier: StdRwLock<Option<Arc<TestCloseBarrier>>>,
}

impl std::fmt::Debug for PowerboxRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PowerboxRegistry(<host-owned>)")
    }
}

impl PowerboxRegistry {
    pub fn new(
        store: Arc<dyn EventStore>,
        runtime_public_store: Arc<dyn EventStore>,
        installations: Arc<dyn InstallationControl>,
        runs: Arc<dyn RunControl>,
    ) -> anyhow::Result<Arc<Self>> {
        anyhow::ensure!(
            !Arc::ptr_eq(&store, &runtime_public_store),
            "Powerbox control journal must be physically distinct from the Runtime public journal"
        );
        Ok(Arc::new(Self {
            store,
            runtime_public_store,
            installations,
            runs,
            projection: RwLock::new(Projection::default()),
            mutation: Mutex::new(()),
            relay: Mutex::new(()),
            owner_lease: StdRwLock::new(None),
            inspector: StdRwLock::new(None),
            authority_refresh: StdRwLock::new(None),
            runtime_broker: StdRwLock::new(None),
            expiry_changed: Notify::new(),
            #[cfg(test)]
            test_close_barrier: StdRwLock::new(None),
        }))
    }

    pub fn install_owner_lease(&self, lease: DevelopmentHostLease) -> anyhow::Result<()> {
        let mut slot = self
            .owner_lease
            .write()
            .expect("Powerbox owner lock poisoned");
        anyhow::ensure!(slot.is_none(), "Powerbox owner lease is already installed");
        *slot = Some(lease);
        Ok(())
    }

    pub fn install_inspector(
        &self,
        inspector: Arc<dyn PowerboxEndpointInspector>,
    ) -> anyhow::Result<()> {
        let mut slot = self
            .inspector
            .write()
            .expect("Powerbox inspector lock poisoned");
        anyhow::ensure!(
            slot.is_none(),
            "Powerbox Runtime inspector is already installed"
        );
        *slot = Some(inspector);
        Ok(())
    }

    pub fn install_authority_refresh(
        &self,
        refresh: PowerboxAuthorityRefresh,
    ) -> anyhow::Result<()> {
        let mut slot = self
            .authority_refresh
            .write()
            .expect("Powerbox authority validator lock poisoned");
        anyhow::ensure!(
            slot.is_none(),
            "Powerbox authority validator is already installed"
        );
        *slot = Some(refresh);
        Ok(())
    }

    fn authority_refresh(&self) -> Option<PowerboxAuthorityRefresh> {
        self.authority_refresh
            .read()
            .expect("Powerbox authority validator lock poisoned")
            .clone()
    }

    async fn validate_basis(&self, basis: &PowerboxAuthorityBasis) -> anyhow::Result<()> {
        basis
            .validate_current(self.authority_refresh().as_ref())
            .await
    }

    async fn ensure_owner(&self) -> anyhow::Result<()> {
        let lease = self
            .owner_lease
            .read()
            .expect("Powerbox owner lock poisoned")
            .clone();
        if let Some(lease) = lease {
            lease.ensure_durable_owner().await?;
        }
        Ok(())
    }

    fn inspector(&self) -> anyhow::Result<Arc<dyn PowerboxEndpointInspector>> {
        self.inspector
            .read()
            .expect("Powerbox inspector lock poisoned")
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Powerbox Runtime inspector is unavailable"))
    }

    fn runtime_broker(&self) -> Option<Arc<plurora_runtime::RunBindingBroker>> {
        self.runtime_broker
            .read()
            .expect("Powerbox Runtime broker lock poisoned")
            .as_ref()
            .and_then(Weak::upgrade)
    }

    #[cfg(test)]
    async fn pause_test_close_barrier(&self) {
        let barrier = self
            .test_close_barrier
            .read()
            .expect("Powerbox test close barrier lock poisoned")
            .clone();
        if let Some(barrier) = barrier {
            barrier.pause().await;
        }
    }

    async fn sync_journal(&self) -> anyhow::Result<usize> {
        let next = self.projection.read().await.next_sequence;
        let events = self
            .store
            .list_session_range(&POWERBOX_SESSION.to_string(), next.checked_sub(1), None)
            .await?;
        let mut state = self.projection.write().await;
        let mut loaded = 0;
        for event in events {
            if event.sequence < state.next_sequence {
                continue;
            }
            anyhow::ensure!(
                event.session_id == POWERBOX_SESSION
                    && event.sequence == state.next_sequence
                    && event.writer_package_id == POWERBOX_WRITER
                    && event.schema_version == POWERBOX_SCHEMA,
                "Powerbox journal identity, schema, or sequence is invalid"
            );
            apply_event(&mut state, &event)?;
            loaded += 1;
        }
        Ok(loaded)
    }

    async fn relay_pending_public_events(&self) -> anyhow::Result<usize> {
        let _relay = self.relay.lock().await;
        let sources = self
            .store
            .list_session(&POWERBOX_SESSION.to_string())
            .await?;
        let mut published = 0usize;
        for source in sources
            .iter()
            .filter(|event| is_public_powerbox_kind(&event.kind))
        {
            if self.relay_source_event_locked(source).await? {
                published = published.saturating_add(1);
            }
        }
        Ok(published)
    }

    async fn ensure_public_relay_caught_up(&self) -> anyhow::Result<()> {
        self.relay_pending_public_events()
            .await
            .map(|_| ())
            .map_err(|_| runtime_outcome_unknown("Powerbox public event relay"))
    }

    async fn relay_source_event(&self, source: &EventEnvelope) -> anyhow::Result<()> {
        let _relay = self.relay.lock().await;
        self.relay_source_event_locked(source).await.map(|_| ())
    }

    async fn relay_source_event_locked(&self, source: &EventEnvelope) -> anyhow::Result<bool> {
        anyhow::ensure!(
            source.session_id == POWERBOX_SESSION
                && source.writer_package_id == POWERBOX_WRITER
                && is_public_powerbox_kind(&source.kind),
            "Powerbox relay accepts only public authority source events"
        );
        let payload = public_payload(source)?;
        let payload_digest = sha256(&serde_json::to_vec(&payload)?);
        loop {
            let existing = self
                .runtime_public_store
                .list_session(&POWERBOX_SESSION.to_string())
                .await?;
            if let Some(published) = existing.iter().find(|event| {
                event
                    .metadata
                    .get("source_event_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(source.id.as_str())
            }) {
                anyhow::ensure!(
                    published.kind == source.kind
                        && published.payload == payload
                        && published
                            .metadata
                            .get("source_event_sequence")
                            .and_then(serde_json::Value::as_u64)
                            == Some(source.sequence)
                        && published
                            .metadata
                            .get("source_payload_digest")
                            .and_then(serde_json::Value::as_str)
                            == Some(payload_digest.as_str()),
                    "Runtime public Powerbox event conflicts with its durable source"
                );
                return Ok(false);
            }
            let expected = self
                .runtime_public_store
                .next_sequence(&POWERBOX_SESSION.to_string())
                .await?;
            let appended = self
                .runtime_public_store
                .append_with_sequence_if_next(
                    POWERBOX_SESSION.to_string(),
                    expected,
                    PLATFORM_RUNTIME_ID.to_string(),
                    source.kind.clone(),
                    POWERBOX_SCHEMA,
                    payload.clone(),
                    serde_json::json!({
                        "authority": "none",
                        "source_session_id": POWERBOX_SESSION,
                        "source_event_id": source.id.clone(),
                        "source_event_sequence": source.sequence,
                        "source_payload_digest": payload_digest.clone(),
                    }),
                )
                .await?;
            if appended.is_some() {
                return Ok(true);
            }
        }
    }

    pub async fn hydrate(&self) -> anyhow::Result<usize> {
        let _guard = self.mutation.lock().await;
        self.ensure_owner().await?;
        let events = self
            .store
            .list_session(&POWERBOX_SESSION.to_string())
            .await?;
        let mut rebuilt = Projection::default();
        for event in &events {
            anyhow::ensure!(
                event.session_id == POWERBOX_SESSION
                    && event.sequence == rebuilt.next_sequence
                    && event.writer_package_id == POWERBOX_WRITER
                    && event.schema_version == POWERBOX_SCHEMA,
                "Powerbox journal identity, schema, or sequence is invalid"
            );
            apply_event(&mut rebuilt, event)?;
        }
        *self.projection.write().await = rebuilt;
        drop(_guard);
        self.ensure_public_relay_caught_up().await?;
        self.resume_closings().await?;
        let _ = self.sweep_expired().await?;
        Ok(events.len())
    }

    async fn append_exposure(
        &self,
        state: &mut Projection,
        kind: &str,
        exposure: ExposureProjection,
        authority: Option<(&PowerboxMutationAuthority, &PowerboxAuthoritySubject)>,
    ) -> anyhow::Result<()> {
        self.ensure_owner().await?;
        let payload = serde_json::to_value(ExposureEvent { exposure })?;
        if let Some((authority, subject)) = authority {
            authority.refresh_current_for(subject).await?;
        }
        let event = self
            .store
            .append_with_sequence_if_next(
                POWERBOX_SESSION.to_string(),
                state.next_sequence,
                POWERBOX_WRITER.to_string(),
                kind.to_string(),
                POWERBOX_SCHEMA,
                payload,
                serde_json::json!({}),
            )
            .await?
            .ok_or_else(|| anyhow::anyhow!("Powerbox journal compare-and-append conflict"))?;
        apply_event(state, &event)?;
        self.expiry_changed.notify_one();
        self.relay_source_event(&event)
            .await
            .map_err(|_| runtime_outcome_unknown("Powerbox public event relay"))
    }

    async fn append_binding(
        &self,
        state: &mut Projection,
        kind: &str,
        binding: BindingProjection,
        authority: Option<(&PowerboxMutationAuthority, &PowerboxAuthoritySubject)>,
    ) -> anyhow::Result<()> {
        self.ensure_owner().await?;
        let payload = serde_json::to_value(BindingEvent { binding })?;
        if let Some((authority, subject)) = authority {
            authority.refresh_current_for(subject).await?;
        }
        let event = self
            .store
            .append_with_sequence_if_next(
                POWERBOX_SESSION.to_string(),
                state.next_sequence,
                POWERBOX_WRITER.to_string(),
                kind.to_string(),
                POWERBOX_SCHEMA,
                payload,
                serde_json::json!({}),
            )
            .await?
            .ok_or_else(|| anyhow::anyhow!("Powerbox journal compare-and-append conflict"))?;
        apply_event(state, &event)?;
        self.expiry_changed.notify_one();
        self.relay_source_event(&event)
            .await
            .map_err(|_| runtime_outcome_unknown("Powerbox public event relay"))
    }

    async fn append_private<T: Serialize>(
        &self,
        state: &mut Projection,
        kind: &str,
        payload: &T,
        authority: Option<(&PowerboxMutationAuthority, &PowerboxAuthoritySubject)>,
    ) -> anyhow::Result<()> {
        self.ensure_owner().await?;
        if let Some((authority, subject)) = authority {
            authority.refresh_current_for(subject).await?;
        }
        let event = self
            .store
            .append_with_sequence_if_next(
                POWERBOX_SESSION.to_string(),
                state.next_sequence,
                POWERBOX_WRITER.to_string(),
                kind.to_string(),
                POWERBOX_SCHEMA,
                serde_json::to_value(payload)?,
                serde_json::json!({"visibility": "host_private", "credentials": "none"}),
            )
            .await?
            .ok_or_else(|| anyhow::anyhow!("Powerbox journal compare-and-append conflict"))?;
        apply_event(state, &event).map(|()| self.expiry_changed.notify_one())
    }

    async fn current_run_pin(
        &self,
        installation_id: &InstallationId,
        run_id: &RunId,
        expected_revision: Option<u64>,
    ) -> anyhow::Result<RunRevisionPin> {
        let view = self
            .runs
            .get(RunGetRequest {
                installation_id: installation_id.clone(),
                run_id: run_id.clone(),
            })
            .await?
            .ok_or_else(|| anyhow::anyhow!("Run not found"))?;
        anyhow::ensure!(
            matches!(view.record.status, RunStatus::Running | RunStatus::Degraded)
                && expected_revision.is_none_or(|revision| revision == view.revision),
            "Run is not current and active"
        );
        let context_id = view
            .record
            .context_id
            .filter(|context| !context.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("Run has no exact Host context"))?;
        Ok(RunRevisionPin {
            run_id: view.record.run_id,
            run_revision: view.revision,
            context_id,
        })
    }

    async fn current_run_pin_for_attachment(
        &self,
        installation_id: &InstallationId,
        run_id: &RunId,
        expected_revision: Option<u64>,
    ) -> Result<RunRevisionPin, BindingAttachError> {
        let view = self
            .runs
            .get(RunGetRequest {
                installation_id: installation_id.clone(),
                run_id: run_id.clone(),
            })
            .await
            .map_err(|error| BindingAttachError::transient("run_lookup_failed", error))?
            .ok_or_else(|| {
                BindingAttachError::definitive(
                    "consumer_activation_drift",
                    anyhow::anyhow!("Run not found"),
                )
            })?;
        if !matches!(view.record.status, RunStatus::Running | RunStatus::Degraded)
            || expected_revision.is_some_and(|revision| revision != view.revision)
        {
            return Err(BindingAttachError::definitive(
                "consumer_activation_drift",
                anyhow::anyhow!("Run is not current and active"),
            ));
        }
        let context_id = view
            .record
            .context_id
            .filter(|context| !context.trim().is_empty())
            .ok_or_else(|| {
                BindingAttachError::definitive(
                    "consumer_activation_drift",
                    anyhow::anyhow!("Run has no exact Host context"),
                )
            })?;
        Ok(RunRevisionPin {
            run_id: view.record.run_id,
            run_revision: view.revision,
            context_id,
        })
    }

    async fn inspect_endpoint(
        &self,
        installation_id: &InstallationId,
        expected_revision: u64,
        run: Option<RunRevisionPin>,
        port: &PortId,
        direction: PortDirection,
    ) -> anyhow::Result<PowerboxEndpointInspection> {
        self.inspector()?
            .inspect(installation_id, expected_revision, run, port, direction)
            .await
    }

    async fn calculate_candidates(
        &self,
        request: &BindingCandidatesRequest,
    ) -> anyhow::Result<BindingCandidatesResult> {
        request.validate()?;
        let query = request
            .query
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("trusted Powerbox query context is missing"))?;
        let consumer_run = match &request.consumer_run {
            Some(pin) => {
                let current = self
                    .current_run_pin(
                        &request.consumer_installation_id,
                        &pin.run_id,
                        Some(pin.run_revision),
                    )
                    .await?;
                anyhow::ensure!(current == *pin, "consumer Run pin is stale");
                Some(current)
            }
            None => None,
        };
        let consumer = self
            .inspect_endpoint(
                &request.consumer_installation_id,
                request.expected_consumer_installation_revision,
                consumer_run,
                &request.import_port,
                PortDirection::Import,
            )
            .await?;
        validate_dynamic_phase(request.phase, request.consumer_run.as_ref())?;
        anyhow::ensure!(
            consumer.phase == request.phase,
            "requested Binding phase does not match the import Port latest_binding_phase"
        );
        let consumer_max_bindings = match &consumer.descriptor.role {
            PortRole::Import { multiplicity, .. } => multiplicity.max,
            PortRole::Export { .. } => anyhow::bail!("consumer endpoint is not an import Port"),
        };
        let (exposures, selected_bindings) = {
            let _guard = self.mutation.lock().await;
            self.sync_journal().await?;
            let state = self.projection.read().await;
            (
                state
                    .exposures
                    .values()
                    .filter(|entry| {
                        !state
                            .exposure_closing
                            .contains_key(&entry.view.record.exposure_id)
                    })
                    .cloned()
                    .collect::<Vec<_>>(),
                state
                    .bindings
                    .values()
                    .filter(|entry| entry.view.record.status == BindingDecisionStatus::Selected)
                    .map(|entry| entry.view.record.clone())
                    .collect::<Vec<_>>(),
            )
        };
        if !consumer_binding_capacity_available(
            &selected_bindings,
            &consumer.endpoint,
            consumer_max_bindings,
        ) {
            return Ok(BindingCandidatesResult {
                candidates: Vec::new(),
                gaps: vec![capacity_gap(
                    request,
                    &consumer,
                    "consumer_import_capacity_exhausted",
                )],
            });
        }
        let now = Utc::now();
        let mut candidates = Vec::new();
        let mut provider_capacity_exhausted = false;
        for exposure in exposures {
            let record = &exposure.view.record;
            if record.status != ExposureStatus::Active
                || record.expires_at.is_some_and(|expiry| now >= expiry)
                || !query.is_in_audience(&record.audience)
            {
                continue;
            }
            if self
                .validate_basis(&exposure.authority_basis)
                .await
                .is_err()
            {
                continue;
            }
            let Some(provider_run) = exposure.provider.run.clone() else {
                continue;
            };
            let Ok(current_run) = self
                .current_run_pin(
                    &record.installation_id,
                    &provider_run.run_id,
                    Some(provider_run.run_revision),
                )
                .await
            else {
                continue;
            };
            if current_run != provider_run {
                continue;
            }
            let Ok(provider) = self
                .inspect_endpoint(
                    &record.installation_id,
                    exposure.provider.installation.installation_revision,
                    Some(provider_run),
                    &record.export_port,
                    PortDirection::Export,
                )
                .await
            else {
                continue;
            };
            if provider.endpoint != exposure.provider
                || provider.descriptor != exposure.provider_descriptor
                || provider.capability.as_ref() != Some(&exposure.capability)
                || check_port_compatibility(&provider.descriptor, &consumer.descriptor).is_err()
            {
                continue;
            }
            let transport = match select_transport(
                &provider.descriptor.transport,
                &consumer.descriptor.transport,
                &TransportPolicy::default(),
            ) {
                Ok(transport) => transport,
                Err(_) => continue,
            };
            let provider_max_bindings = match &provider.descriptor.role {
                PortRole::Export { multiplicity, .. } => multiplicity.max,
                PortRole::Import { .. } => continue,
            };
            if !provider_binding_capacity_available(
                &selected_bindings,
                &provider.endpoint,
                &exposure.capability,
                provider_max_bindings,
            ) {
                provider_capacity_exhausted = true;
                continue;
            }
            let mut candidate = BindingCandidate {
                candidate_digest: String::new(),
                exposure: exposure.view.clone(),
                consumer: consumer.endpoint.clone(),
                consumer_port: consumer.descriptor.clone(),
                provider: provider.endpoint,
                provider_port: provider.descriptor,
                provider_work: provider.work,
                provider_installation: provider.installation,
                provider_component: provider.component,
                capability: exposure.capability.clone(),
                transport,
                phase: request.phase,
                availability: consumer.availability,
                effective_expires_at: earliest_expiry(
                    record.expires_at,
                    exposure
                        .authority_basis
                        .expires_at_ms()
                        .and_then(DateTime::<Utc>::from_timestamp_millis),
                ),
            };
            candidate.candidate_digest = candidate_digest(&candidate)?;
            candidates.push(candidate);
        }
        let mut result = BindingCandidatesResult {
            candidates,
            gaps: Vec::new(),
        }
        .stable()?;
        if result.candidates.is_empty() && provider_capacity_exhausted {
            result.gaps.push(capacity_gap(
                request,
                &consumer,
                "provider_export_capacity_exhausted",
            ));
        } else if result.candidates.is_empty() {
            result.gaps.push(BindingGap {
                reason_code: "binding_unavailable".to_string(),
                next_step: "create or select one visible compatible Exposure".to_string(),
                installation_id: Some(request.consumer_installation_id.clone()),
                run_id: request.consumer_run.as_ref().map(|run| run.run_id.clone()),
                node_id: Some(consumer.endpoint.port.leaf_port.node_id),
                port_id: Some(request.import_port.clone()),
                interaction_model: Some(consumer.descriptor.interaction.0),
            });
        } else if result.candidates.len() > 1 {
            result.gaps.push(BindingGap {
                reason_code: "binding_ambiguous".to_string(),
                next_step: "select one candidate explicitly by its candidate_digest".to_string(),
                installation_id: Some(request.consumer_installation_id.clone()),
                run_id: request.consumer_run.as_ref().map(|run| run.run_id.clone()),
                node_id: Some(consumer.endpoint.port.leaf_port.node_id),
                port_id: Some(request.import_port.clone()),
                interaction_model: Some(consumer.descriptor.interaction.0),
            });
        }
        Ok(result)
    }

    pub async fn sweep_expired(&self) -> anyhow::Result<PowerboxInvalidationResult> {
        self.ensure_owner().await?;
        let now = Utc::now();
        let (expired_exposures, expired_bindings) = {
            let _guard = self.mutation.lock().await;
            self.sync_journal().await?;
            let state = self.projection.read().await;
            let exposures = state
                .exposures
                .values()
                .filter(|entry| {
                    entry.view.record.status == ExposureStatus::Active
                        && entry
                            .view
                            .record
                            .expires_at
                            .is_some_and(|expiry| now >= expiry)
                })
                .map(|entry| entry.view.record.exposure_id.clone())
                .collect::<Vec<_>>();
            let bindings = state
                .bindings
                .values()
                .filter(|entry| {
                    entry.view.record.status == BindingDecisionStatus::Selected
                        && entry
                            .view
                            .record
                            .effective_expires_at
                            .is_some_and(|expiry| now >= expiry)
                })
                .map(|entry| entry.view.record.binding_id.clone())
                .collect::<Vec<_>>();
            (exposures, bindings)
        };
        let mut affected = Vec::new();
        for exposure_id in expired_exposures {
            if let Some(closing) = self
                .begin_exposure_closing(
                    &exposure_id,
                    ExposureStatus::Expired,
                    "exposure_expired",
                    None,
                    None,
                )
                .await?
            {
                affected.extend(
                    self.complete_exposure_closing(closing)
                        .await?
                        .affected_binding_ids,
                );
            }
        }
        for binding_id in expired_bindings {
            if let Some(closing) = self
                .begin_binding_closing(
                    &binding_id,
                    BindingDecisionStatus::Expired,
                    "binding_expired",
                    None,
                    None,
                )
                .await?
            {
                if self.complete_binding_closing(closing).await?.is_some() {
                    affected.push(binding_id);
                }
            }
        }
        affected.sort();
        affected.dedup();
        Ok(PowerboxInvalidationResult {
            affected_binding_ids: affected,
        })
    }

    pub async fn next_expiry(&self) -> Option<DateTime<Utc>> {
        let state = self.projection.read().await;
        state
            .exposures
            .values()
            .filter(|entry| entry.view.record.status == ExposureStatus::Active)
            .filter_map(|entry| entry.view.record.expires_at)
            .chain(
                state
                    .bindings
                    .values()
                    .filter(|entry| entry.view.record.status == BindingDecisionStatus::Selected)
                    .filter_map(|entry| entry.view.record.effective_expires_at),
            )
            .min()
    }

    async fn validate_exposure_snapshot(&self, view: &ExposureView) -> anyhow::Result<()> {
        let (exposure, closing) = {
            let state = self.projection.read().await;
            (
                state
                    .exposures
                    .get(&view.record.exposure_id)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Exposure not found"))?,
                state
                    .exposure_closing
                    .contains_key(&view.record.exposure_id),
            )
        };
        anyhow::ensure!(
            exposure.view == *view
                && exposure.view.record.status == ExposureStatus::Active
                && !closing,
            "Exposure is no longer current"
        );
        self.validate_basis(&exposure.authority_basis).await?;
        let run = exposure
            .provider
            .run
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Exposure provider Run pin is missing"))?;
        anyhow::ensure!(
            self.current_run_pin(
                &exposure.provider.installation.installation_id,
                &run.run_id,
                Some(run.run_revision),
            )
            .await?
                == run,
            "Exposure provider Run pin is stale"
        );
        let inspected = self
            .inspect_endpoint(
                &exposure.provider.installation.installation_id,
                exposure.provider.installation.installation_revision,
                Some(run),
                &exposure.provider.port.root_port,
                PortDirection::Export,
            )
            .await?;
        anyhow::ensure!(
            inspected.endpoint == exposure.provider
                && inspected.descriptor == exposure.provider_descriptor
                && inspected.capability.as_ref() == Some(&exposure.capability),
            "Exposure provider endpoint drifted"
        );
        Ok(())
    }

    async fn validate_binding_snapshot(
        &self,
        selection: &BindingSelectionRecord,
    ) -> Result<(), BindingAttachError> {
        let (binding, exposure) = {
            let state = self.projection.read().await;
            let binding = state
                .bindings
                .get(&selection.binding_id)
                .cloned()
                .ok_or_else(|| {
                    BindingAttachError::definitive(
                        "binding_current_drift",
                        anyhow::anyhow!("Binding not found"),
                    )
                })?;
            let exposure = state
                .exposures
                .get(&selection.exposure_id)
                .cloned()
                .ok_or_else(|| {
                    BindingAttachError::definitive(
                        "binding_current_drift",
                        anyhow::anyhow!("Exposure not found"),
                    )
                })?;
            if state.binding_closing.contains_key(&selection.binding_id)
                || state.exposure_closing.contains_key(&selection.exposure_id)
            {
                return Err(BindingAttachError::definitive(
                    "binding_current_drift",
                    anyhow::anyhow!("Binding or Exposure has a durable closing obligation"),
                ));
            }
            (binding, exposure)
        };
        if binding.view.record != *selection
            || selection.status != BindingDecisionStatus::Selected
            || exposure.view.record.status != ExposureStatus::Active
            || exposure.view.revision != selection.exposure_revision
        {
            return Err(BindingAttachError::definitive(
                "binding_current_drift",
                anyhow::anyhow!("Binding or Exposure is no longer current"),
            ));
        }
        self.validate_basis(&binding.authority_basis)
            .await
            .map_err(|error| BindingAttachError::transient("authority_validation_failed", error))?;
        self.validate_basis(&exposure.authority_basis)
            .await
            .map_err(|error| BindingAttachError::transient("authority_validation_failed", error))?;
        if let Some(run) = selection.consumer.run.as_ref() {
            let current = self
                .current_run_pin_for_attachment(
                    &selection.consumer.installation.installation_id,
                    &run.run_id,
                    Some(run.run_revision),
                )
                .await?;
            if current != *run {
                return Err(BindingAttachError::definitive(
                    "consumer_activation_drift",
                    anyhow::anyhow!("consumer Run pin is stale"),
                ));
            }
        }
        let consumer = self
            .inspect_endpoint(
                &selection.consumer.installation.installation_id,
                selection.consumer.installation.installation_revision,
                selection.consumer.run.clone(),
                &selection.consumer.port.root_port,
                PortDirection::Import,
            )
            .await
            .map_err(|error| BindingAttachError::transient("endpoint_inspection_failed", error))?;
        let provider_run = selection.provider.run.clone().ok_or_else(|| {
            BindingAttachError::definitive(
                "provider_activation_drift",
                anyhow::anyhow!("provider Run pin is missing"),
            )
        })?;
        let current_provider = self
            .current_run_pin_for_attachment(
                &selection.provider.installation.installation_id,
                &provider_run.run_id,
                Some(provider_run.run_revision),
            )
            .await?;
        if current_provider != provider_run {
            return Err(BindingAttachError::definitive(
                "provider_activation_drift",
                anyhow::anyhow!("provider Run pin is stale"),
            ));
        }
        let provider = self
            .inspect_endpoint(
                &selection.provider.installation.installation_id,
                selection.provider.installation.installation_revision,
                Some(provider_run),
                &selection.provider.port.root_port,
                PortDirection::Export,
            )
            .await
            .map_err(|error| BindingAttachError::transient("endpoint_inspection_failed", error))?;
        if consumer.endpoint != selection.consumer
            || provider.endpoint != selection.provider
            || provider.capability.as_ref() != Some(&selection.capability)
            || check_port_compatibility(&provider.descriptor, &consumer.descriptor).is_err()
        {
            return Err(BindingAttachError::definitive(
                "provider_pin_drift",
                anyhow::anyhow!("Binding endpoint identity or compatibility drifted"),
            ));
        }
        Ok(())
    }

    async fn begin_binding_closing(
        &self,
        binding_id: &BindingId,
        terminal_status: BindingDecisionStatus,
        reason_code: &str,
        claim: Option<MutationClaim>,
        authority: Option<(&PowerboxMutationAuthority, &PowerboxAuthoritySubject)>,
    ) -> anyhow::Result<Option<BindingClosing>> {
        let _guard = self.mutation.lock().await;
        self.ensure_owner().await?;
        self.sync_journal().await?;
        let mut state = self.projection.write().await;
        let Some(binding) = state.bindings.get(binding_id) else {
            return Ok(None);
        };
        if binding.view.record.status != BindingDecisionStatus::Selected {
            return Ok(None);
        }
        if let Some(existing) = state.binding_closing.get(binding_id) {
            return Ok(Some(existing.clone()));
        }
        let closing = BindingClosing {
            binding_id: binding_id.clone(),
            terminal_status,
            reason_code: reason_code.to_string(),
            claim,
        };
        self.append_private(&mut state, PRIVATE_BINDING_CLOSING, &closing, authority)
            .await?;
        Ok(Some(closing))
    }

    async fn complete_binding_closing(
        &self,
        closing: BindingClosing,
    ) -> anyhow::Result<Option<BindingView>> {
        #[cfg(test)]
        self.pause_test_close_barrier().await;
        if let Some(broker) = self.runtime_broker() {
            broker
                .close_binding_barrier(&closing.binding_id, &closing.reason_code)
                .await?;
        }
        let _guard = self.mutation.lock().await;
        self.ensure_owner().await?;
        self.sync_journal().await?;
        let mut state = self.projection.write().await;
        let Some(mut binding) = state.bindings.get(&closing.binding_id).cloned() else {
            return Ok(None);
        };
        if binding.view.record.status != BindingDecisionStatus::Selected {
            return Ok(Some(binding.view));
        }
        anyhow::ensure!(
            state.binding_closing.get(&closing.binding_id) == Some(&closing),
            "Binding closing obligation changed before completion"
        );
        binding.view.revision = binding.view.revision.saturating_add(1);
        binding.view.record.status = closing.terminal_status;
        binding.view.effective_status = BindingEffectiveStatus::Broken {
            reason_code: closing.reason_code.clone(),
        };
        if let Some(claim) = &closing.claim {
            binding.idempotency_key = claim.idempotency_key.clone();
            binding.request_fingerprint = claim.request_fingerprint.clone();
        }
        let kind = match closing.terminal_status {
            BindingDecisionStatus::Revoked => EVENT_BINDING_REVOKED,
            BindingDecisionStatus::Expired => EVENT_BINDING_EXPIRED,
            BindingDecisionStatus::Selected => {
                anyhow::bail!("selected is not a terminal Binding status")
            }
        };
        let view = binding.view.clone();
        self.append_binding(&mut state, kind, binding, None).await?;
        Ok(Some(view))
    }

    async fn terminalize_binding(
        &self,
        binding_id: &BindingId,
        reason_code: &str,
    ) -> anyhow::Result<bool> {
        let Some(closing) = self
            .begin_binding_closing(
                binding_id,
                BindingDecisionStatus::Revoked,
                reason_code,
                None,
                None,
            )
            .await?
        else {
            return Ok(false);
        };
        self.complete_binding_closing(closing).await?;
        Ok(true)
    }

    async fn begin_exposure_closing(
        &self,
        exposure_id: &ExposureId,
        terminal_status: ExposureStatus,
        reason_code: &str,
        claim: Option<MutationClaim>,
        authority: Option<(&PowerboxMutationAuthority, &PowerboxAuthoritySubject)>,
    ) -> anyhow::Result<Option<ExposureClosing>> {
        let _guard = self.mutation.lock().await;
        self.ensure_owner().await?;
        self.sync_journal().await?;
        let mut state = self.projection.write().await;
        let Some(exposure) = state.exposures.get(exposure_id) else {
            return Ok(None);
        };
        if exposure.view.record.status != ExposureStatus::Active {
            return Ok(None);
        }
        if let Some(existing) = state.exposure_closing.get(exposure_id) {
            return Ok(Some(existing.clone()));
        }
        let closing = ExposureClosing {
            exposure_id: exposure_id.clone(),
            terminal_status,
            reason_code: reason_code.to_string(),
            affected_binding_ids: active_bindings_for_exposure(&state, exposure_id),
            claim,
        };
        self.append_private(&mut state, PRIVATE_EXPOSURE_CLOSING, &closing, authority)
            .await?;
        Ok(Some(closing))
    }

    async fn complete_exposure_closing(
        &self,
        closing: ExposureClosing,
    ) -> anyhow::Result<PowerboxInvalidationResult> {
        #[cfg(test)]
        self.pause_test_close_barrier().await;
        let binding_status = match closing.terminal_status {
            ExposureStatus::Revoked => BindingDecisionStatus::Revoked,
            ExposureStatus::Expired => BindingDecisionStatus::Expired,
            ExposureStatus::Active => anyhow::bail!("active is not a terminal Exposure status"),
        };
        let binding_kind = match binding_status {
            BindingDecisionStatus::Revoked => EVENT_BINDING_REVOKED,
            BindingDecisionStatus::Expired => EVENT_BINDING_EXPIRED,
            BindingDecisionStatus::Selected => unreachable!(),
        };
        let exposure_kind = match closing.terminal_status {
            ExposureStatus::Revoked => EVENT_EXPOSURE_REVOKED,
            ExposureStatus::Expired => EVENT_EXPOSURE_EXPIRED,
            ExposureStatus::Active => unreachable!(),
        };
        let mut affected_binding_ids = closing.affected_binding_ids.clone();
        let mut barriered = std::collections::BTreeSet::new();
        loop {
            if let Some(broker) = self.runtime_broker() {
                let pending = affected_binding_ids
                    .iter()
                    .filter(|binding_id| !barriered.contains(*binding_id))
                    .cloned()
                    .collect::<Vec<_>>();
                for binding_id in pending {
                    broker
                        .close_binding_barrier(&binding_id, &closing.reason_code)
                        .await?;
                    barriered.insert(binding_id);
                }
            } else {
                barriered.extend(affected_binding_ids.iter().cloned());
            }

            let _guard = self.mutation.lock().await;
            self.ensure_owner().await?;
            self.sync_journal().await?;
            let mut state = self.projection.write().await;
            let exposure = state
                .exposures
                .get(&closing.exposure_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Exposure disappeared during close completion"))?;
            if exposure.view.record.status != ExposureStatus::Active {
                anyhow::ensure!(
                    exposure.view.record.status == closing.terminal_status,
                    "Exposure completed with a different terminal status"
                );
                let mut known = state
                    .bindings
                    .values()
                    .filter(|binding| {
                        binding.view.record.exposure_id == closing.exposure_id
                            && binding.view.record.status == binding_status
                            && matches!(
                                &binding.view.effective_status,
                                BindingEffectiveStatus::Broken { reason_code }
                                    if reason_code == &closing.reason_code
                            )
                    })
                    .map(|binding| binding.view.record.binding_id.clone())
                    .collect::<Vec<_>>();
                known.sort();
                known.dedup();
                return Ok(PowerboxInvalidationResult {
                    affected_binding_ids: known,
                });
            }
            anyhow::ensure!(
                state.exposure_closing.get(&closing.exposure_id) == Some(&closing),
                "Exposure closing obligation changed before completion"
            );

            // Admission rejects this Exposure as soon as the private intent is
            // durable. This final locked rescan is defense-in-depth for a
            // historical or otherwise abnormal association that was not in the
            // intent's original snapshot.
            let discovered = active_bindings_for_exposure(&state, &closing.exposure_id);
            let mut found_new = false;
            for binding_id in discovered {
                if !affected_binding_ids.contains(&binding_id) {
                    affected_binding_ids.push(binding_id);
                    found_new = true;
                }
            }
            affected_binding_ids.sort();
            affected_binding_ids.dedup();
            if found_new {
                continue;
            }

            for binding_id in &affected_binding_ids {
                let Some(mut binding) = state.bindings.get(binding_id).cloned() else {
                    continue;
                };
                if binding.view.record.status != BindingDecisionStatus::Selected {
                    continue;
                }
                binding.view.revision = binding.view.revision.saturating_add(1);
                binding.view.record.status = binding_status;
                binding.view.effective_status = BindingEffectiveStatus::Broken {
                    reason_code: closing.reason_code.clone(),
                };
                self.append_binding(&mut state, binding_kind, binding, None)
                    .await?;
            }
            let mut exposure = state
                .exposures
                .get(&closing.exposure_id)
                .cloned()
                .expect("active Exposure remained present under the mutation lock");
            exposure.view.revision = exposure.view.revision.saturating_add(1);
            exposure.view.record.status = closing.terminal_status;
            if let Some(claim) = &closing.claim {
                exposure.idempotency_key = claim.idempotency_key.clone();
                exposure.request_fingerprint = claim.request_fingerprint.clone();
            }
            self.append_exposure(&mut state, exposure_kind, exposure, None)
                .await?;
            return Ok(PowerboxInvalidationResult {
                affected_binding_ids,
            });
        }
    }

    async fn terminalize_exposure(
        &self,
        exposure_id: &ExposureId,
        reason_code: &str,
    ) -> anyhow::Result<PowerboxInvalidationResult> {
        let Some(closing) = self
            .begin_exposure_closing(
                exposure_id,
                ExposureStatus::Revoked,
                reason_code,
                None,
                None,
            )
            .await?
        else {
            return Ok(PowerboxInvalidationResult::default());
        };
        self.complete_exposure_closing(closing).await
    }

    async fn resume_closings(&self) -> anyhow::Result<()> {
        let (bindings, exposures) = {
            let state = self.projection.read().await;
            (
                state.binding_closing.values().cloned().collect::<Vec<_>>(),
                state.exposure_closing.values().cloned().collect::<Vec<_>>(),
            )
        };
        for closing in bindings {
            self.complete_binding_closing(closing).await?;
        }
        for closing in exposures {
            self.complete_exposure_closing(closing).await?;
        }
        Ok(())
    }

    pub async fn reconcile_runs(&self) -> anyhow::Result<PowerboxInvalidationResult> {
        let runs = self
            .runs
            .list(plurora_runtime::RunListRequest::default())
            .await?;
        let current = runs
            .into_iter()
            .filter(|run| matches!(run.record.status, RunStatus::Running | RunStatus::Degraded))
            .filter_map(|run| {
                let context_id = run.record.context_id.clone()?;
                Some((
                    (
                        run.record.installation_id.clone(),
                        run.record.run_id.clone(),
                    ),
                    RunRevisionPin {
                        run_id: run.record.run_id,
                        run_revision: run.revision,
                        context_id,
                    },
                ))
            })
            .collect::<HashMap<_, _>>();
        let (exposures, bindings) = {
            let state = self.projection.read().await;
            (
                state.exposures.values().cloned().collect::<Vec<_>>(),
                state.bindings.values().cloned().collect::<Vec<_>>(),
            )
        };
        let mut affected = Vec::new();
        for exposure in exposures {
            let stale = exposure.view.record.status == ExposureStatus::Active
                && exposure.provider.run.as_ref().is_none_or(|pin| {
                    current.get(&(
                        exposure.provider.installation.installation_id.clone(),
                        pin.run_id.clone(),
                    )) != Some(pin)
                });
            if stale {
                affected.extend(
                    self.terminalize_exposure(&exposure.view.record.exposure_id, "run_interrupted")
                        .await?
                        .affected_binding_ids,
                );
            }
        }
        for binding in bindings {
            if binding.view.record.status != BindingDecisionStatus::Selected {
                continue;
            }
            let consumer_stale = binding
                .view
                .record
                .consumer
                .run
                .as_ref()
                .is_some_and(|pin| {
                    current.get(&(
                        binding
                            .view
                            .record
                            .consumer
                            .installation
                            .installation_id
                            .clone(),
                        pin.run_id.clone(),
                    )) != Some(pin)
                });
            if consumer_stale
                && self
                    .terminalize_binding(&binding.view.record.binding_id, "run_interrupted")
                    .await?
            {
                affected.push(binding.view.record.binding_id);
            }
        }
        affected.sort();
        affected.dedup();
        Ok(PowerboxInvalidationResult {
            affected_binding_ids: affected,
        })
    }

    pub async fn expiry_changed(&self) {
        self.expiry_changed.notified().await;
    }
}

#[async_trait]
impl PowerboxControl for PowerboxRegistry {
    fn install_runtime_broker(
        &self,
        broker: Weak<plurora_runtime::RunBindingBroker>,
    ) -> anyhow::Result<()> {
        let mut slot = self
            .runtime_broker
            .write()
            .expect("Powerbox Runtime broker lock poisoned");
        anyhow::ensure!(
            slot.is_none(),
            "Powerbox Runtime broker is already installed"
        );
        *slot = Some(broker);
        Ok(())
    }

    async fn exposure_list(
        &self,
        request: ExposureListRequest,
    ) -> anyhow::Result<Vec<ExposureView>> {
        let _ = self.sweep_expired().await?;
        let mut views = self
            .projection
            .read()
            .await
            .exposures
            .values()
            .filter(|entry| {
                request
                    .installation_id
                    .as_ref()
                    .is_none_or(|id| id == &entry.view.record.installation_id)
                    && request
                        .run_id
                        .as_ref()
                        .is_none_or(|id| entry.view.record.run_id.as_ref() == Some(id))
                    && request
                        .status
                        .is_none_or(|status| status == entry.view.record.status)
            })
            .map(|entry| entry.view.clone())
            .collect::<Vec<_>>();
        views.sort_by(|left, right| left.record.exposure_id.cmp(&right.record.exposure_id));
        Ok(views)
    }

    async fn exposure_create(
        &self,
        request: ExposureCreateRequest,
    ) -> anyhow::Result<ExposureMutationResult> {
        request.validate()?;
        let subject = PowerboxAuthoritySubject::ExposureCreate {
            installation_id: request.installation_id.clone(),
            run_id: request.run_id.clone(),
            export_port: request.export_port.clone(),
        };
        let authority = request
            .authority
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("current Powerbox mutation authority is required"))?;
        let fingerprint = fingerprint(&request)?;
        authority.refresh_current_for(&subject).await?;
        self.ensure_public_relay_caught_up().await?;
        let authority_basis = authority.durable_basis();
        let replay = {
            let _guard = self.mutation.lock().await;
            self.ensure_owner().await?;
            self.sync_journal().await?;
            let state = self.projection.read().await;
            state
                .exposure_idempotency
                .get(&request.idempotency_key)
                .map(|(known, id)| {
                    anyhow::ensure!(
                        known == &fingerprint,
                        "idempotency key was already used for a different Exposure mutation"
                    );
                    Ok(state.exposures[id].view.clone())
                })
                .transpose()?
        };
        if let Some(exposure) = replay {
            return Ok(ExposureMutationResult {
                exposure,
                affected_binding_ids: Vec::new(),
                idempotent: true,
            });
        }
        let installation = self
            .installations
            .acquire_ready_for_run(
                &request.installation_id,
                request.expected_installation_revision,
            )
            .await?;
        let provider_run = self
            .current_run_pin(
                &request.installation_id,
                &request.run_id,
                Some(request.expected_run_revision),
            )
            .await?;
        let inspected = self
            .inspector()?
            .inspect(
                &request.installation_id,
                request.expected_installation_revision,
                Some(provider_run.clone()),
                &request.export_port,
                PortDirection::Export,
            )
            .await?;
        anyhow::ensure!(
            inspected.endpoint.installation.work_revision
                == installation.artifacts().installation.record.work_revision
                && inspected.endpoint.installation.assembly_lock
                    == installation.artifacts().installation.record.assembly_lock,
            "Powerbox inspector drifted from the held Installation revision"
        );
        drop(installation);
        let capability = inspected
            .capability
            .clone()
            .ok_or_else(|| anyhow::anyhow!("export Port has no exact Runtime capability"))?;
        let record = ExposureRecord {
            exposure_id: ExposureId::new(),
            installation_id: request.installation_id.clone(),
            run_id: Some(request.run_id.clone()),
            export_port: request.export_port.clone(),
            audience: request.audience.clone(),
            expires_at: earliest_expiry(
                request.expires_at,
                authority_basis
                    .expires_at_ms()
                    .and_then(DateTime::<Utc>::from_timestamp_millis),
            ),
            status: ExposureStatus::Active,
        };
        record.validate()?;
        let projection = ExposureProjection {
            view: ExposureView {
                record,
                revision: 1,
            },
            provider: inspected.endpoint,
            provider_descriptor: inspected.descriptor,
            capability,
            idempotency_key: request.idempotency_key,
            request_fingerprint: fingerprint,
            authority_basis,
        };
        let view = projection.view.clone();
        let _guard = self.mutation.lock().await;
        self.ensure_owner().await?;
        self.sync_journal().await?;
        let mut state = self.projection.write().await;
        if let Some((replay_fingerprint, id)) =
            state.exposure_idempotency.get(&projection.idempotency_key)
        {
            let existing = &state.exposures[id];
            anyhow::ensure!(
                replay_fingerprint == &projection.request_fingerprint,
                "idempotency key was already used for a different Exposure mutation"
            );
            return Ok(ExposureMutationResult {
                exposure: existing.view.clone(),
                affected_binding_ids: Vec::new(),
                idempotent: true,
            });
        }
        self.append_exposure(
            &mut state,
            EVENT_EXPOSURE_CREATED,
            projection,
            Some((authority, &subject)),
        )
        .await?;
        drop(state);
        drop(_guard);
        if self.validate_exposure_snapshot(&view).await.is_err() {
            let invalidated = self
                .terminalize_exposure(&view.record.exposure_id, "exposure_drifted")
                .await?;
            anyhow::bail!(
                "Exposure ceased to be current before create completed ({} bindings invalidated)",
                invalidated.affected_binding_ids.len()
            );
        }
        Ok(ExposureMutationResult {
            exposure: view,
            affected_binding_ids: Vec::new(),
            idempotent: false,
        })
    }

    async fn exposure_revoke(
        &self,
        request: ExposureRevokeRequest,
    ) -> anyhow::Result<ExposureMutationResult> {
        request.validate()?;
        let subject = PowerboxAuthoritySubject::ExposureRevoke {
            installation_id: request.installation_id.clone(),
            run_id: request.run_id.clone(),
            export_port: request.export_port.clone(),
            exposure_id: request.exposure_id.clone(),
        };
        let authority = request
            .authority
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("current Powerbox mutation authority is required"))?;
        let fingerprint = fingerprint(&request)?;
        authority.refresh_current_for(&subject).await?;
        self.ensure_public_relay_caught_up().await?;

        let replay = {
            let _guard = self.mutation.lock().await;
            self.ensure_owner().await?;
            self.sync_journal().await?;
            let state = self.projection.read().await;
            state
                .exposure_idempotency
                .get(&request.idempotency_key)
                .map(|(known, id)| {
                    anyhow::ensure!(
                        known == &fingerprint,
                        "idempotency key was already used for a different Exposure mutation"
                    );
                    Ok((
                        id.clone(),
                        state.exposures[id].view.clone(),
                        state.exposure_closing.get(id).cloned(),
                    ))
                })
                .transpose()?
        };
        if let Some((id, _, closing)) = replay {
            if let Some(closing) = closing {
                self.complete_exposure_closing(closing).await?;
            }
            let state = self.projection.read().await;
            return Ok(ExposureMutationResult {
                exposure: state.exposures[&id].view.clone(),
                affected_binding_ids: revoked_bindings_for_exposure(&state, &id),
                idempotent: true,
            });
        }

        let installation = self
            .installations
            .acquire_ready_for_run(
                &request.installation_id,
                request.expected_installation_revision,
            )
            .await?;
        drop(installation);
        let current_run = self
            .current_run_pin(
                &request.installation_id,
                &request.run_id,
                Some(request.expected_run_revision),
            )
            .await?;
        let _guard = self.mutation.lock().await;
        self.ensure_owner().await?;
        self.sync_journal().await?;
        let mut state = self.projection.write().await;
        if let Some((replay_fingerprint, id)) =
            state.exposure_idempotency.get(&request.idempotency_key)
        {
            anyhow::ensure!(
                replay_fingerprint == &fingerprint,
                "idempotency key was already used for a different Exposure mutation"
            );
            let id = id.clone();
            let closing = state.exposure_closing.get(&id).cloned();
            drop(state);
            drop(_guard);
            if let Some(closing) = closing {
                self.complete_exposure_closing(closing).await?;
            }
            let state = self.projection.read().await;
            return Ok(ExposureMutationResult {
                exposure: state.exposures[&id].view.clone(),
                affected_binding_ids: revoked_bindings_for_exposure(&state, &id),
                idempotent: true,
            });
        }
        let exposure = state
            .exposures
            .get(&request.exposure_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Exposure not found"))?;
        anyhow::ensure!(
            exposure.view.revision == request.expected_exposure_revision
                && exposure.view.record.status == ExposureStatus::Active
                && exposure.view.record.installation_id == request.installation_id
                && exposure.view.record.run_id.as_ref() == Some(&request.run_id)
                && exposure.view.record.export_port == request.export_port,
            "stale or terminal Exposure revision"
        );
        anyhow::ensure!(
            exposure.provider.run.as_ref() == Some(&current_run),
            "Exposure provider Run pin is stale"
        );
        let closing = ExposureClosing {
            exposure_id: request.exposure_id.clone(),
            terminal_status: ExposureStatus::Revoked,
            reason_code: "exposure_revoked".to_string(),
            affected_binding_ids: active_bindings_for_exposure(&state, &request.exposure_id),
            claim: Some(MutationClaim {
                idempotency_key: request.idempotency_key,
                request_fingerprint: fingerprint,
            }),
        };
        self.append_private(
            &mut state,
            PRIVATE_EXPOSURE_CLOSING,
            &closing,
            Some((authority, &subject)),
        )
        .await?;
        drop(state);
        drop(_guard);
        let affected = self
            .complete_exposure_closing(closing)
            .await?
            .affected_binding_ids;
        let exposure = self.projection.read().await.exposures[&request.exposure_id]
            .view
            .clone();
        Ok(ExposureMutationResult {
            exposure,
            affected_binding_ids: affected,
            idempotent: false,
        })
    }

    async fn binding_list(&self, request: BindingListRequest) -> anyhow::Result<Vec<BindingView>> {
        let _ = self.sweep_expired().await?;
        let query = request.query.as_ref().ok_or_else(|| {
            anyhow::anyhow!("binding list requires trusted caller visibility context")
        })?;
        let state = self.projection.read().await;
        let mut views = state
            .bindings
            .values()
            .filter(|entry| {
                state
                    .exposures
                    .get(&entry.view.record.exposure_id)
                    .is_some_and(|exposure| query.is_in_audience(&exposure.view.record.audience))
                    && request.consumer_installation_id.as_ref().is_none_or(|id| {
                        id == &entry.view.record.consumer.installation.installation_id
                    })
                    && request.run_id.as_ref().is_none_or(|id| {
                        entry
                            .view
                            .record
                            .consumer
                            .run
                            .as_ref()
                            .map(|run| &run.run_id)
                            == Some(id)
                            || state
                                .attachments
                                .get(&entry.view.record.binding_id)
                                .map(|a| &a.run_id)
                                == Some(id)
                    })
                    && request
                        .status
                        .is_none_or(|status| status == entry.view.record.status)
            })
            .map(|entry| {
                let mut view = entry.view.clone();
                if view.record.status == BindingDecisionStatus::Selected {
                    view.effective_status =
                        if state.attachments.contains_key(&view.record.binding_id) {
                            BindingEffectiveStatus::Active
                        } else {
                            BindingEffectiveStatus::Detached
                        };
                }
                view
            })
            .collect::<Vec<_>>();
        views.sort_by(|left, right| left.record.binding_id.cmp(&right.record.binding_id));
        Ok(views)
    }

    async fn binding_candidates(
        &self,
        request: BindingCandidatesRequest,
    ) -> anyhow::Result<BindingCandidatesResult> {
        let _ = self.sweep_expired().await?;
        self.calculate_candidates(&request).await
    }

    async fn binding_select(
        &self,
        request: BindingSelectRequest,
    ) -> anyhow::Result<BindingMutationResult> {
        request.validate()?;
        let _ = self.sweep_expired().await?;
        let subject = PowerboxAuthoritySubject::BindingSelect {
            consumer_installation_id: request.consumer_installation_id.clone(),
            consumer_run: request.consumer_run.clone(),
            import_port: request.import_port.clone(),
            exposure_id: request.exposure_id.clone(),
        };
        let authority = request
            .authority
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("current Powerbox mutation authority is required"))?;
        let fingerprint = fingerprint(&request)?;
        authority.refresh_current_for(&subject).await?;
        self.ensure_public_relay_caught_up().await?;
        let authority_basis = authority.durable_basis();
        let replay = {
            let _guard = self.mutation.lock().await;
            self.ensure_owner().await?;
            self.sync_journal().await?;
            let state = self.projection.read().await;
            state
                .binding_idempotency
                .get(&request.idempotency_key)
                .map(|(known, id)| {
                    anyhow::ensure!(
                        known == &fingerprint,
                        "idempotency key was already used for a different Binding mutation"
                    );
                    let binding = &state.bindings[id];
                    Ok((
                        id.clone(),
                        state.binding_closing.get(id).cloned(),
                        state
                            .exposure_closing
                            .get(&binding.view.record.exposure_id)
                            .cloned(),
                    ))
                })
                .transpose()?
        };
        if let Some((binding_id, binding_closing, exposure_closing)) = replay {
            if let Some(closing) = binding_closing {
                self.complete_binding_closing(closing).await?;
            } else if let Some(closing) = exposure_closing {
                self.complete_exposure_closing(closing).await?;
            }
            let binding = self.projection.read().await.bindings[&binding_id]
                .view
                .clone();
            return Ok(BindingMutationResult {
                binding,
                affected_binding_ids: Vec::new(),
                idempotent: true,
            });
        }
        let candidates = self
            .calculate_candidates(&BindingCandidatesRequest {
                consumer_installation_id: request.consumer_installation_id.clone(),
                expected_consumer_installation_revision: request
                    .expected_consumer_installation_revision,
                phase: request.phase,
                consumer_run: request.consumer_run.clone(),
                import_port: request.import_port.clone(),
                preferences: Vec::new(),
                query: request.query.clone(),
            })
            .await?;
        let chosen = candidates
            .candidates
            .into_iter()
            .find(|candidate| {
                candidate.exposure.record.exposure_id == request.exposure_id
                    && candidate.exposure.revision == request.expected_exposure_revision
                    && candidate.provider.installation.installation_id
                        == request.provider_installation_id
                    && candidate.provider.installation.installation_revision
                        == request.expected_provider_installation_revision
                    && candidate.candidate_digest == request.candidate_digest
            })
            .ok_or_else(|| anyhow::anyhow!("chosen Binding candidate is no longer current"))?;
        anyhow::ensure!(
            chosen.phase == request.phase,
            "chosen Binding candidate phase is stale"
        );
        let inspected_consumer = self
            .inspect_endpoint(
                &request.consumer_installation_id,
                request.expected_consumer_installation_revision,
                request.consumer_run.clone(),
                &request.import_port,
                PortDirection::Import,
            )
            .await?;
        anyhow::ensure!(
            inspected_consumer.endpoint == chosen.consumer
                && inspected_consumer.descriptor == chosen.consumer_port
                && inspected_consumer.phase == request.phase,
            "consumer endpoint disclosure drifted before Binding selection append"
        );
        let max_consumer_bindings = match &inspected_consumer.descriptor.role {
            PortRole::Import { multiplicity, .. } => multiplicity.max,
            PortRole::Export { .. } => anyhow::bail!("consumer endpoint is not an import Port"),
        };
        let inspected_provider = self
            .inspect_endpoint(
                &chosen.provider.installation.installation_id,
                chosen.provider.installation.installation_revision,
                chosen.provider.run.clone(),
                &chosen.provider.port.root_port,
                PortDirection::Export,
            )
            .await?;
        anyhow::ensure!(
            inspected_provider.endpoint == chosen.provider
                && inspected_provider.descriptor == chosen.provider_port
                && inspected_provider.work == chosen.provider_work
                && inspected_provider.installation == chosen.provider_installation
                && inspected_provider.component == chosen.provider_component
                && inspected_provider.capability.as_ref() == Some(&chosen.capability),
            "provider endpoint disclosure drifted before Binding selection append"
        );
        let max_provider_bindings = match &inspected_provider.descriptor.role {
            PortRole::Export { multiplicity, .. } => multiplicity.max,
            PortRole::Import { .. } => anyhow::bail!("provider endpoint is not an export Port"),
        };
        let _guard = self.mutation.lock().await;
        self.ensure_owner().await?;
        self.sync_journal().await?;
        let mut state = self.projection.write().await;
        if let Some((replay_fingerprint, id)) =
            state.binding_idempotency.get(&request.idempotency_key)
        {
            let binding_id = id.clone();
            let existing = &state.bindings[&binding_id];
            anyhow::ensure!(
                replay_fingerprint == &fingerprint,
                "idempotency key was already used for a different Binding mutation"
            );
            let binding_closing = state.binding_closing.get(&binding_id).cloned();
            let exposure_closing = state
                .exposure_closing
                .get(&existing.view.record.exposure_id)
                .cloned();
            drop(state);
            drop(_guard);
            if let Some(closing) = binding_closing {
                self.complete_binding_closing(closing).await?;
            } else if let Some(closing) = exposure_closing {
                self.complete_exposure_closing(closing).await?;
            }
            return Ok(BindingMutationResult {
                binding: self.projection.read().await.bindings[&binding_id]
                    .view
                    .clone(),
                affected_binding_ids: Vec::new(),
                idempotent: true,
            });
        }
        anyhow::ensure!(
            state
                .exposures
                .get(&request.exposure_id)
                .is_some_and(|entry| {
                    entry.view.revision == request.expected_exposure_revision
                        && entry.view.record.status == ExposureStatus::Active
                        && entry
                            .view
                            .record
                            .expires_at
                            .is_none_or(|expiry| Utc::now() < expiry)
                }),
            "Exposure changed before Binding selection append"
        );
        anyhow::ensure!(
            !state.exposure_closing.contains_key(&request.exposure_id),
            "Exposure is closing before Binding selection append"
        );
        ensure_consumer_binding_capacity(&state, &chosen.consumer, max_consumer_bindings)?;
        ensure_provider_binding_capacity(
            &state,
            &chosen.provider,
            &chosen.capability,
            max_provider_bindings,
        )?;
        let record = BindingSelectionRecord {
            binding_id: BindingId::new(),
            candidate_digest: chosen.candidate_digest,
            exposure_id: chosen.exposure.record.exposure_id,
            exposure_revision: chosen.exposure.revision,
            consumer: chosen.consumer,
            provider: chosen.provider,
            capability: chosen.capability,
            transport: chosen.transport,
            phase: chosen.phase,
            availability: chosen.availability,
            effective_expires_at: earliest_expiry(
                chosen.effective_expires_at,
                authority_basis
                    .expires_at_ms()
                    .and_then(DateTime::<Utc>::from_timestamp_millis),
            ),
            status: BindingDecisionStatus::Selected,
        };
        record.validate_selected()?;
        let projection = BindingProjection {
            view: BindingView {
                record,
                revision: 1,
                effective_status: BindingEffectiveStatus::Detached,
            },
            idempotency_key: request.idempotency_key,
            request_fingerprint: fingerprint,
            authority_basis,
        };
        let view = projection.view.clone();
        self.append_binding(
            &mut state,
            EVENT_BINDING_SELECTED,
            projection,
            Some((authority, &subject)),
        )
        .await?;
        drop(state);
        drop(_guard);
        if let Err(error) = self.validate_binding_snapshot(&view.record).await {
            if error.is_definitive_drift() {
                self.terminalize_binding(&view.record.binding_id, error.reason_code())
                    .await?;
            }
            return Err(error.into());
        }
        Ok(BindingMutationResult {
            binding: view,
            affected_binding_ids: Vec::new(),
            idempotent: false,
        })
    }

    async fn binding_revoke(
        &self,
        request: BindingRevokeRequest,
    ) -> anyhow::Result<BindingMutationResult> {
        request.validate()?;
        let subject = PowerboxAuthoritySubject::BindingRevoke {
            consumer_installation_id: request.consumer_installation_id.clone(),
            consumer_run: request.consumer_run.clone(),
            import_port: request.import_port.clone(),
            exposure_id: request.exposure_id.clone(),
            binding_id: request.binding_id.clone(),
        };
        let authority = request
            .authority
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("current Powerbox mutation authority is required"))?;
        let fingerprint = fingerprint(&request)?;
        authority.refresh_current_for(&subject).await?;
        self.ensure_public_relay_caught_up().await?;

        let replay = {
            let _guard = self.mutation.lock().await;
            self.ensure_owner().await?;
            self.sync_journal().await?;
            let state = self.projection.read().await;
            state
                .binding_idempotency
                .get(&request.idempotency_key)
                .map(|(known, id)| {
                    anyhow::ensure!(
                        known == &fingerprint,
                        "idempotency key was already used for a different Binding mutation"
                    );
                    Ok((id.clone(), state.binding_closing.get(id).cloned()))
                })
                .transpose()?
        };
        if let Some((id, closing)) = replay {
            if let Some(closing) = closing {
                self.complete_binding_closing(closing).await?;
            }
            return Ok(BindingMutationResult {
                binding: self.projection.read().await.bindings[&id].view.clone(),
                affected_binding_ids: vec![id],
                idempotent: true,
            });
        }

        let installation = self
            .installations
            .acquire_ready_for_run(
                &request.consumer_installation_id,
                request.expected_consumer_installation_revision,
            )
            .await?;
        drop(installation);
        if let Some(run) = request.consumer_run.as_ref() {
            let current = self
                .current_run_pin(
                    &request.consumer_installation_id,
                    &run.run_id,
                    Some(run.run_revision),
                )
                .await?;
            anyhow::ensure!(current == *run, "consumer Run pin is stale");
        }
        let _guard = self.mutation.lock().await;
        self.ensure_owner().await?;
        self.sync_journal().await?;
        let mut state = self.projection.write().await;
        if let Some((replay_fingerprint, id)) =
            state.binding_idempotency.get(&request.idempotency_key)
        {
            anyhow::ensure!(
                replay_fingerprint == &fingerprint,
                "idempotency key was already used for a different Binding mutation"
            );
            let id = id.clone();
            let closing = state.binding_closing.get(&id).cloned();
            drop(state);
            drop(_guard);
            if let Some(closing) = closing {
                self.complete_binding_closing(closing).await?;
            }
            return Ok(BindingMutationResult {
                binding: self.projection.read().await.bindings[&id].view.clone(),
                affected_binding_ids: vec![id.clone()],
                idempotent: true,
            });
        }
        let binding = state
            .bindings
            .get(&request.binding_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Binding not found"))?;
        anyhow::ensure!(
            binding.view.revision == request.expected_binding_revision
                && binding.view.record.status == BindingDecisionStatus::Selected
                && binding.view.record.consumer.installation.installation_id
                    == request.consumer_installation_id
                && binding.view.record.consumer.port.root_port == request.import_port
                && binding.view.record.exposure_id == request.exposure_id
                && binding.view.record.consumer.run == request.consumer_run,
            "stale or terminal Binding revision"
        );
        let closing = BindingClosing {
            binding_id: request.binding_id.clone(),
            terminal_status: BindingDecisionStatus::Revoked,
            reason_code: "binding_revoked".to_string(),
            claim: Some(MutationClaim {
                idempotency_key: request.idempotency_key,
                request_fingerprint: fingerprint,
            }),
        };
        self.append_private(
            &mut state,
            PRIVATE_BINDING_CLOSING,
            &closing,
            Some((authority, &subject)),
        )
        .await?;
        drop(state);
        drop(_guard);
        let view = self
            .complete_binding_closing(closing)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Binding disappeared during revoke completion"))?;
        Ok(BindingMutationResult {
            binding: view,
            affected_binding_ids: vec![request.binding_id],
            idempotent: false,
        })
    }

    async fn prepare_run_bindings(
        &self,
        request: RunBindingPreparationRequest,
    ) -> anyhow::Result<RunBindingPreparation> {
        let _ = self.sweep_expired().await?;
        let state = self.projection.read().await;
        let mut prepared = RunBindingPreparation::default();
        for required in request.required_imports {
            let matches = state
                .bindings
                .values()
                .filter(|entry| {
                    entry.view.record.status == BindingDecisionStatus::Selected
                        && !state
                            .binding_closing
                            .contains_key(&entry.view.record.binding_id)
                        && !state
                            .exposure_closing
                            .contains_key(&entry.view.record.exposure_id)
                        && entry.view.record.consumer.installation.installation_id
                            == request.consumer_installation_id
                        && entry
                            .view
                            .record
                            .consumer
                            .installation
                            .installation_revision
                            == request.consumer_installation_revision
                        && entry.view.record.consumer.port == required
                        && entry.view.record.consumer.run == request.consumer_run
                        && !state
                            .attachments
                            .contains_key(&entry.view.record.binding_id)
                })
                .map(|entry| entry.view.record.clone())
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [selection] => prepared.selected.push(selection.clone()),
                [] => prepared.gaps.push(BindingGap {
                    reason_code: "binding_unavailable".to_string(),
                    next_step: "select one explicit compatible Exposure".to_string(),
                    installation_id: Some(request.consumer_installation_id.clone()),
                    run_id: request.consumer_run.as_ref().map(|run| run.run_id.clone()),
                    node_id: Some(required.leaf_port.node_id),
                    port_id: Some(required.root_port),
                    interaction_model: None,
                }),
                _ => prepared.gaps.push(BindingGap {
                    reason_code: "binding_ambiguous".to_string(),
                    next_step: "leave exactly one selected Binding for this import".to_string(),
                    installation_id: Some(request.consumer_installation_id.clone()),
                    run_id: request.consumer_run.as_ref().map(|run| run.run_id.clone()),
                    node_id: Some(required.leaf_port.node_id),
                    port_id: Some(required.root_port),
                    interaction_model: None,
                }),
            }
        }
        Ok(prepared)
    }

    async fn binding_attached(&self, notice: BindingAttachmentNotice) -> anyhow::Result<()> {
        let selection = {
            let state = self.projection.read().await;
            state
                .bindings
                .get(&notice.binding_id)
                .map(|binding| binding.view.record.clone())
                .ok_or_else(|| {
                    BindingAttachError::definitive(
                        "binding_current_drift",
                        anyhow::anyhow!("Binding not found"),
                    )
                })?
        };
        self.validate_binding_snapshot(&selection).await?;
        if !(match selection.phase {
            BindingPhase::Launch => selection.consumer.run.is_none(),
            BindingPhase::Runtime => selection.consumer.run.as_ref().is_some_and(|run| {
                run.run_id == notice.run_id && run.context_id == notice.session_id
            }),
            _ => false,
        } && selection.consumer.component.package_id == notice.consumer_package_id
            && selection.consumer.component.node_path.last() == Some(&notice.consumer_node_id)
            && selection.consumer.port.root_port == notice.consumer_port)
        {
            return Err(BindingAttachError::definitive(
                "consumer_activation_drift",
                anyhow::anyhow!(
                    "Binding attachment does not match the exact selected consumer activation"
                ),
            )
            .into());
        }
        let _guard = self.mutation.lock().await;
        self.ensure_owner().await.map_err(|error| {
            BindingAttachError::transient("powerbox_owner_validation_failed", error)
        })?;
        let mut state = self.projection.write().await;
        let binding = state.bindings.get(&notice.binding_id).ok_or_else(|| {
            BindingAttachError::definitive(
                "binding_current_drift",
                anyhow::anyhow!("Binding not found"),
            )
        })?;
        if binding.view.record.status != BindingDecisionStatus::Selected
            || state.binding_closing.contains_key(&notice.binding_id)
            || state
                .exposure_closing
                .contains_key(&binding.view.record.exposure_id)
        {
            return Err(BindingAttachError::definitive(
                "binding_current_drift",
                anyhow::anyhow!("terminal or closing Binding cannot attach"),
            )
            .into());
        }
        if let Some(existing) = state.attachments.get(&notice.binding_id) {
            if existing.run_id != notice.run_id
                || existing.session_id != notice.session_id
                || existing.component_activation_id != notice.component_activation_id
            {
                return Err(BindingAttachError::definitive(
                    "consumer_activation_drift",
                    anyhow::anyhow!("Binding is already attached to another Run"),
                )
                .into());
            }
        } else {
            state.attachments.insert(
                notice.binding_id.clone(),
                Attachment {
                    run_id: notice.run_id.clone(),
                    session_id: notice.session_id.clone(),
                    component_activation_id: notice.component_activation_id.clone(),
                },
            );
        }
        drop(state);
        drop(_guard);
        self.validate_binding_snapshot(&selection).await?;
        Ok(())
    }

    async fn validate_binding_current(
        &self,
        request: BindingCurrentValidationRequest,
    ) -> anyhow::Result<BindingSelectionRecord> {
        let validation: Result<BindingSelectionRecord, BindingAttachError> = async {
            self.ensure_owner().await.map_err(|error| {
                BindingAttachError::transient("powerbox_owner_validation_failed", error)
            })?;
            let invalidated = self.sweep_expired().await.map_err(|error| {
                BindingAttachError::transient("powerbox_expiry_sweep_failed", error)
            })?;
            if invalidated
                .affected_binding_ids
                .contains(&request.binding_id)
            {
                return Err(BindingAttachError::definitive(
                    "binding_expired",
                    anyhow::anyhow!("Binding expired"),
                ));
            }
            let (selection, attachment, exposure) = {
                let state = self.projection.read().await;
                let binding = state.bindings.get(&request.binding_id).ok_or_else(|| {
                    BindingAttachError::definitive(
                        "binding_current_drift",
                        anyhow::anyhow!("Binding not found"),
                    )
                })?;
                let exposure = state
                    .exposures
                    .get(&binding.view.record.exposure_id)
                    .ok_or_else(|| {
                        BindingAttachError::definitive(
                            "binding_current_drift",
                            anyhow::anyhow!("Exposure not found"),
                        )
                    })?;
                if state.binding_closing.contains_key(&request.binding_id)
                    || state
                        .exposure_closing
                        .contains_key(&binding.view.record.exposure_id)
                {
                    return Err(BindingAttachError::definitive(
                        "binding_current_drift",
                        anyhow::anyhow!("Binding or Exposure has a durable closing obligation"),
                    ));
                }
                (
                    binding.view.record.clone(),
                    state.attachments.get(&request.binding_id).cloned(),
                    exposure.clone(),
                )
            };
            if !(selection == request.expected_binding
                && selection.status == BindingDecisionStatus::Selected
                && (if request.require_attachment {
                    attachment.as_ref().is_some_and(|attached| {
                        attached.run_id == request.run_id
                            && attached.session_id == request.session_id
                    })
                } else {
                    attachment.as_ref().is_none_or(|attached| {
                        attached.run_id == request.run_id
                            && attached.session_id == request.session_id
                    })
                })
                && exposure.view.record.status == ExposureStatus::Active
                && exposure.view.revision == selection.exposure_revision)
            {
                return Err(BindingAttachError::definitive(
                    "binding_current_drift",
                    anyhow::anyhow!("Binding, Exposure, or attachment is no longer current"),
                ));
            }
            if request.require_attachment || selection.phase == BindingPhase::Runtime {
                let current_consumer = self
                    .current_run_pin_for_attachment(
                        &selection.consumer.installation.installation_id,
                        &request.run_id,
                        selection.consumer.run.as_ref().map(|run| run.run_revision),
                    )
                    .await?;
                if !(current_consumer.context_id == request.session_id
                    && selection
                        .consumer
                        .run
                        .as_ref()
                        .is_none_or(|run| run == &current_consumer))
                {
                    return Err(BindingAttachError::definitive(
                        "consumer_activation_drift",
                        anyhow::anyhow!("consumer Run activation is no longer current"),
                    ));
                }
            }
            self.validate_binding_snapshot(&selection).await?;
            Ok(selection)
        }
        .await;
        validation.map_err(Into::into)
    }

    async fn binding_drifted(
        &self,
        binding_id: &BindingId,
        reason_code: &str,
    ) -> anyhow::Result<PowerboxInvalidationResult> {
        let mut result = PowerboxInvalidationResult::default();
        if self.terminalize_binding(binding_id, reason_code).await? {
            result.affected_binding_ids.push(binding_id.clone());
        }
        Ok(result)
    }

    async fn binding_detached(&self, notice: BindingCleanupNotice) -> anyhow::Result<()> {
        let _guard = self.mutation.lock().await;
        self.ensure_owner().await?;
        let mut state = self.projection.write().await;
        if state
            .attachments
            .get(&notice.binding_id)
            .is_some_and(|attached| {
                attached.run_id == notice.run_id && attached.session_id == notice.session_id
            })
        {
            state.attachments.remove(&notice.binding_id);
        }
        Ok(())
    }

    async fn run_stopped(
        &self,
        installation_id: &InstallationId,
        run_id: &RunId,
        _session_id: &String,
    ) -> anyhow::Result<PowerboxInvalidationResult> {
        let (exposure_ids, binding_ids) = {
            let _guard = self.mutation.lock().await;
            self.ensure_owner().await?;
            self.sync_journal().await?;
            let state = self.projection.read().await;
            let exposures = state
                .exposures
                .values()
                .filter(|entry| {
                    entry.view.record.status == ExposureStatus::Active
                        && entry.view.record.installation_id == *installation_id
                        && entry.view.record.run_id.as_ref() == Some(run_id)
                })
                .map(|entry| entry.view.record.exposure_id.clone())
                .collect::<Vec<_>>();
            let bindings = state
                .bindings
                .values()
                .filter(|entry| {
                    entry.view.record.status == BindingDecisionStatus::Selected
                        && (entry.view.record.provider.run.as_ref().is_some_and(|run| {
                            run.run_id == *run_id
                                && entry.view.record.provider.installation.installation_id
                                    == *installation_id
                        }) || entry.view.record.consumer.run.as_ref().is_some_and(|run| {
                            run.run_id == *run_id
                                && entry.view.record.consumer.installation.installation_id
                                    == *installation_id
                        }) || state
                            .attachments
                            .get(&entry.view.record.binding_id)
                            .is_some_and(|attachment| attachment.run_id == *run_id))
                })
                .map(|entry| entry.view.record.binding_id.clone())
                .collect::<Vec<_>>();
            (exposures, bindings)
        };
        let mut affected = Vec::new();
        for exposure_id in exposure_ids {
            affected.extend(
                self.terminalize_exposure(&exposure_id, "run_stopped")
                    .await?
                    .affected_binding_ids,
            );
        }
        for binding_id in binding_ids {
            if self.terminalize_binding(&binding_id, "run_stopped").await? {
                affected.push(binding_id);
            }
        }
        affected.sort();
        affected.dedup();
        Ok(PowerboxInvalidationResult {
            affected_binding_ids: affected,
        })
    }

    async fn reconcile_authority(&self) -> anyhow::Result<PowerboxInvalidationResult> {
        let expired = self.sweep_expired().await?;
        let (exposures, bindings) = {
            let state = self.projection.read().await;
            (
                state.exposures.values().cloned().collect::<Vec<_>>(),
                state.bindings.values().cloned().collect::<Vec<_>>(),
            )
        };
        let mut affected = expired.affected_binding_ids;
        for exposure in exposures {
            if exposure.view.record.status == ExposureStatus::Active
                && self
                    .validate_basis(&exposure.authority_basis)
                    .await
                    .is_err()
            {
                affected.extend(
                    self.terminalize_exposure(
                        &exposure.view.record.exposure_id,
                        "exposure_authority_revoked",
                    )
                    .await?
                    .affected_binding_ids,
                );
            }
        }
        for binding in bindings {
            if binding.view.record.status == BindingDecisionStatus::Selected
                && self.validate_basis(&binding.authority_basis).await.is_err()
                && self
                    .terminalize_binding(
                        &binding.view.record.binding_id,
                        "binding_authority_revoked",
                    )
                    .await?
            {
                affected.push(binding.view.record.binding_id);
            }
        }
        affected.sort();
        affected.dedup();
        Ok(PowerboxInvalidationResult {
            affected_binding_ids: affected,
        })
    }

    async fn installation_changed(
        &self,
        installation_id: &InstallationId,
        current_revision: Option<u64>,
    ) -> anyhow::Result<PowerboxInvalidationResult> {
        let (exposures, bindings) = {
            let state = self.projection.read().await;
            (
                state.exposures.values().cloned().collect::<Vec<_>>(),
                state.bindings.values().cloned().collect::<Vec<_>>(),
            )
        };
        let mut affected = Vec::new();
        for exposure in exposures {
            if exposure.view.record.status == ExposureStatus::Active
                && exposure.provider.installation.installation_id == *installation_id
                && current_revision != Some(exposure.provider.installation.installation_revision)
            {
                affected.extend(
                    self.terminalize_exposure(
                        &exposure.view.record.exposure_id,
                        "installation_changed",
                    )
                    .await?
                    .affected_binding_ids,
                );
            }
        }
        for binding in bindings {
            if binding.view.record.status != BindingDecisionStatus::Selected {
                continue;
            }
            let consumer_changed = binding.view.record.consumer.installation.installation_id
                == *installation_id
                && current_revision
                    != Some(
                        binding
                            .view
                            .record
                            .consumer
                            .installation
                            .installation_revision,
                    );
            let provider_changed = binding.view.record.provider.installation.installation_id
                == *installation_id
                && current_revision
                    != Some(
                        binding
                            .view
                            .record
                            .provider
                            .installation
                            .installation_revision,
                    );
            if (consumer_changed || provider_changed)
                && self
                    .terminalize_binding(&binding.view.record.binding_id, "installation_changed")
                    .await?
            {
                affected.push(binding.view.record.binding_id);
            }
        }
        affected.sort();
        affected.dedup();
        Ok(PowerboxInvalidationResult {
            affected_binding_ids: affected,
        })
    }
}

fn apply_event(state: &mut Projection, event: &EventEnvelope) -> anyhow::Result<()> {
    match event.kind.as_str() {
        EVENT_EXPOSURE_CREATED | EVENT_EXPOSURE_REVOKED | EVENT_EXPOSURE_EXPIRED => {
            let payload: ExposureEvent = serde_json::from_value(event.payload.clone())?;
            let expected_status = match event.kind.as_str() {
                EVENT_EXPOSURE_CREATED => ExposureStatus::Active,
                EVENT_EXPOSURE_REVOKED => ExposureStatus::Revoked,
                EVENT_EXPOSURE_EXPIRED => ExposureStatus::Expired,
                _ => unreachable!(),
            };
            anyhow::ensure!(
                payload.exposure.view.record.status == expected_status,
                "Powerbox Exposure event kind and status disagree"
            );
            let id = payload.exposure.view.record.exposure_id.clone();
            if let Some(previous) = state.exposures.get(&id) {
                anyhow::ensure!(
                    payload.exposure.view.revision == previous.view.revision + 1
                        && previous.view.record.status == ExposureStatus::Active,
                    "Powerbox Exposure journal revision or terminal transition is invalid"
                );
            } else {
                anyhow::ensure!(
                    event.kind == EVENT_EXPOSURE_CREATED
                        && payload.exposure.view.revision == 1
                        && payload.exposure.view.record.status == ExposureStatus::Active,
                    "Powerbox Exposure journal does not begin with created revision 1"
                );
            }
            state.exposure_idempotency.insert(
                payload.exposure.idempotency_key.clone(),
                (payload.exposure.request_fingerprint.clone(), id.clone()),
            );
            if expected_status != ExposureStatus::Active {
                state.exposure_closing.remove(&id);
            }
            state.exposures.insert(id, payload.exposure);
        }
        EVENT_BINDING_SELECTED | EVENT_BINDING_REVOKED | EVENT_BINDING_EXPIRED => {
            let payload: BindingEvent = serde_json::from_value(event.payload.clone())?;
            let expected_status = match event.kind.as_str() {
                EVENT_BINDING_SELECTED => BindingDecisionStatus::Selected,
                EVENT_BINDING_REVOKED => BindingDecisionStatus::Revoked,
                EVENT_BINDING_EXPIRED => BindingDecisionStatus::Expired,
                _ => unreachable!(),
            };
            anyhow::ensure!(
                payload.binding.view.record.status == expected_status,
                "Powerbox Binding event kind and status disagree"
            );
            let id = payload.binding.view.record.binding_id.clone();
            if let Some(previous) = state.bindings.get(&id) {
                anyhow::ensure!(
                    payload.binding.view.revision == previous.view.revision + 1
                        && previous.view.record.status == BindingDecisionStatus::Selected,
                    "Powerbox Binding journal revision or terminal transition is invalid"
                );
            } else {
                anyhow::ensure!(
                    event.kind == EVENT_BINDING_SELECTED
                        && payload.binding.view.revision == 1
                        && payload.binding.view.record.status == BindingDecisionStatus::Selected,
                    "Powerbox Binding journal does not begin with selected revision 1"
                );
            }
            state.binding_idempotency.insert(
                payload.binding.idempotency_key.clone(),
                (payload.binding.request_fingerprint.clone(), id.clone()),
            );
            if expected_status != BindingDecisionStatus::Selected {
                state.binding_closing.remove(&id);
            }
            state.bindings.insert(id, payload.binding);
        }
        PRIVATE_EXPOSURE_CLOSING => {
            let closing: ExposureClosing = serde_json::from_value(event.payload.clone())?;
            anyhow::ensure!(
                matches!(
                    closing.terminal_status,
                    ExposureStatus::Revoked | ExposureStatus::Expired
                ),
                "private Exposure closing intent must name a terminal status"
            );
            let exposure = state.exposures.get(&closing.exposure_id).ok_or_else(|| {
                anyhow::anyhow!("private Exposure closing intent has no active object")
            })?;
            anyhow::ensure!(
                exposure.view.record.status == ExposureStatus::Active,
                "private Exposure closing intent cannot follow a public terminal"
            );
            let mut affected = closing.affected_binding_ids.clone();
            affected.sort();
            affected.dedup();
            anyhow::ensure!(
                affected == closing.affected_binding_ids,
                "private Exposure closing intent Binding ids are not canonical"
            );
            if let Some(previous) = state.exposure_closing.get(&closing.exposure_id) {
                anyhow::ensure!(previous == &closing, "Exposure closing intent changed");
            } else {
                if let Some(claim) = &closing.claim {
                    state.exposure_idempotency.insert(
                        claim.idempotency_key.clone(),
                        (
                            claim.request_fingerprint.clone(),
                            closing.exposure_id.clone(),
                        ),
                    );
                }
                state
                    .exposure_closing
                    .insert(closing.exposure_id.clone(), closing);
            }
        }
        PRIVATE_BINDING_CLOSING => {
            let closing: BindingClosing = serde_json::from_value(event.payload.clone())?;
            anyhow::ensure!(
                matches!(
                    closing.terminal_status,
                    BindingDecisionStatus::Revoked | BindingDecisionStatus::Expired
                ),
                "private Binding closing intent must name a terminal status"
            );
            let binding = state.bindings.get(&closing.binding_id).ok_or_else(|| {
                anyhow::anyhow!("private Binding closing intent has no selected object")
            })?;
            anyhow::ensure!(
                binding.view.record.status == BindingDecisionStatus::Selected,
                "private Binding closing intent cannot follow a public terminal"
            );
            if let Some(previous) = state.binding_closing.get(&closing.binding_id) {
                anyhow::ensure!(previous == &closing, "Binding closing intent changed");
            } else {
                if let Some(claim) = &closing.claim {
                    state.binding_idempotency.insert(
                        claim.idempotency_key.clone(),
                        (
                            claim.request_fingerprint.clone(),
                            closing.binding_id.clone(),
                        ),
                    );
                }
                state
                    .binding_closing
                    .insert(closing.binding_id.clone(), closing);
            }
        }
        _ => anyhow::bail!("Powerbox journal contains an unsupported public event kind"),
    }
    state.next_sequence = event.sequence + 1;
    Ok(())
}

fn is_public_powerbox_kind(kind: &str) -> bool {
    matches!(
        kind,
        EVENT_EXPOSURE_CREATED
            | EVENT_EXPOSURE_REVOKED
            | EVENT_EXPOSURE_EXPIRED
            | EVENT_BINDING_SELECTED
            | EVENT_BINDING_REVOKED
            | EVENT_BINDING_EXPIRED
    )
}

fn public_payload(source: &EventEnvelope) -> anyhow::Result<serde_json::Value> {
    match source.kind.as_str() {
        EVENT_EXPOSURE_CREATED | EVENT_EXPOSURE_REVOKED | EVENT_EXPOSURE_EXPIRED => {
            let source: ExposureEvent = serde_json::from_value(source.payload.clone())?;
            Ok(serde_json::json!({"exposure": source.exposure.view}))
        }
        EVENT_BINDING_SELECTED | EVENT_BINDING_REVOKED | EVENT_BINDING_EXPIRED => {
            let source: BindingEvent = serde_json::from_value(source.payload.clone())?;
            Ok(serde_json::json!({"binding": source.binding.view}))
        }
        _ => anyhow::bail!("Powerbox private event cannot be relayed publicly"),
    }
}

fn active_bindings_for_exposure(state: &Projection, exposure_id: &ExposureId) -> Vec<BindingId> {
    let mut ids = state
        .bindings
        .values()
        .filter(|entry| {
            entry.view.record.exposure_id == *exposure_id
                && entry.view.record.status == BindingDecisionStatus::Selected
        })
        .map(|entry| entry.view.record.binding_id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

fn capacity_gap(
    request: &BindingCandidatesRequest,
    consumer: &PowerboxEndpointInspection,
    reason_code: &str,
) -> BindingGap {
    BindingGap {
        reason_code: reason_code.to_string(),
        next_step:
            "wait for a selected Binding to become terminal or choose another compatible endpoint"
                .to_string(),
        installation_id: Some(request.consumer_installation_id.clone()),
        run_id: request.consumer_run.as_ref().map(|run| run.run_id.clone()),
        node_id: Some(consumer.endpoint.port.leaf_port.node_id.clone()),
        port_id: Some(request.import_port.clone()),
        interaction_model: Some(consumer.descriptor.interaction.0.clone()),
    }
}

/// Identity used for Port multiplicity admission.
///
/// A resolved Port may be reached through more than one root/exposed Port. The
/// resolver counts the resulting leaf `(node_path, leaf_port_id)` endpoint, not
/// the root alias or the binding/exposure record that led to it. Powerbox must
/// use the same leaf identity while retaining the Host-local Installation and
/// Run context, plus the concrete component (and, for providers, capability)
/// pin that supplies the endpoint.
/// `root_port` and the Binding's `exposure_id` are deliberately not part of
/// this key: aliases and separate Exposures still address the same leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CapacityEndpointIdentity {
    installation: InstallationRevisionPin,
    run: Option<RunRevisionPin>,
    node_path: Vec<NodeId>,
    leaf_port: PortEndpoint,
    component: ComponentPin,
    capability: Option<CapabilityPin>,
}

fn capacity_endpoint_identity(
    endpoint: &BindingEndpointPin,
    capability: Option<&CapabilityPin>,
) -> CapacityEndpointIdentity {
    let ResolvedPortPin {
        node_path,
        leaf_port,
        ..
    } = &endpoint.port;
    CapacityEndpointIdentity {
        installation: endpoint.installation.clone(),
        run: endpoint.run.clone(),
        node_path: node_path.clone(),
        leaf_port: leaf_port.clone(),
        component: endpoint.component.clone(),
        capability: capability.cloned(),
    }
}

fn consumer_binding_capacity_available(
    selected: &[BindingSelectionRecord],
    consumer: &BindingEndpointPin,
    max_bindings: Option<u16>,
) -> bool {
    max_bindings.is_none_or(|max| {
        let identity = capacity_endpoint_identity(consumer, None);
        selected
            .iter()
            .filter(|binding| capacity_endpoint_identity(&binding.consumer, None) == identity)
            .count()
            < usize::from(max)
    })
}

fn provider_binding_capacity_available(
    selected: &[BindingSelectionRecord],
    provider: &BindingEndpointPin,
    capability: &CapabilityPin,
    max_bindings: Option<u16>,
) -> bool {
    max_bindings.is_none_or(|max| {
        let identity = capacity_endpoint_identity(provider, Some(capability));
        selected
            .iter()
            .filter(|binding| {
                capacity_endpoint_identity(&binding.provider, Some(&binding.capability)) == identity
            })
            .count()
            < usize::from(max)
    })
}

fn ensure_consumer_binding_capacity(
    state: &Projection,
    consumer: &BindingEndpointPin,
    max_bindings: Option<u16>,
) -> anyhow::Result<()> {
    let selected = state
        .bindings
        .values()
        .filter(|entry| entry.view.record.status == BindingDecisionStatus::Selected)
        .map(|entry| entry.view.record.clone())
        .collect::<Vec<_>>();
    anyhow::ensure!(
        consumer_binding_capacity_available(&selected, consumer, max_bindings),
        "consumer import Port multiplicity is already satisfied"
    );
    Ok(())
}

fn ensure_provider_binding_capacity(
    state: &Projection,
    provider: &BindingEndpointPin,
    capability: &CapabilityPin,
    max_bindings: Option<u16>,
) -> anyhow::Result<()> {
    let selected = state
        .bindings
        .values()
        .filter(|entry| entry.view.record.status == BindingDecisionStatus::Selected)
        .map(|entry| entry.view.record.clone())
        .collect::<Vec<_>>();
    anyhow::ensure!(
        provider_binding_capacity_available(&selected, provider, capability, max_bindings),
        "provider export Port multiplicity is already satisfied"
    );
    Ok(())
}

fn earliest_expiry(
    left: Option<DateTime<Utc>>,
    right: Option<DateTime<Utc>>,
) -> Option<DateTime<Utc>> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(expiry), None) | (None, Some(expiry)) => Some(expiry),
        (None, None) => None,
    }
}

fn validate_dynamic_phase(
    phase: BindingPhase,
    consumer_run: Option<&RunRevisionPin>,
) -> anyhow::Result<()> {
    match phase {
        BindingPhase::Launch => anyhow::ensure!(
            consumer_run.is_none(),
            "Launch-phase Binding selection must not carry a consumer Run pin"
        ),
        BindingPhase::Runtime => anyhow::ensure!(
            consumer_run.is_some(),
            "Runtime-phase Binding selection requires an exact consumer Run pin"
        ),
        _ => anyhow::bail!("dynamic Binding selection supports only Launch or Runtime phase"),
    }
    Ok(())
}

fn revoked_bindings_for_exposure(state: &Projection, exposure_id: &ExposureId) -> Vec<BindingId> {
    let mut ids = state
        .bindings
        .values()
        .filter(|entry| {
            entry.view.record.exposure_id == *exposure_id
                && entry.view.record.status == BindingDecisionStatus::Revoked
                && matches!(
                    &entry.view.effective_status,
                    BindingEffectiveStatus::Broken { reason_code }
                        if reason_code == "exposure_revoked"
                )
        })
        .map(|entry| entry.view.record.binding_id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

fn fingerprint<T: Serialize>(value: &T) -> anyhow::Result<String> {
    let bytes = serde_json::to_vec(value)?;
    Ok(sha256(&bytes))
}

fn candidate_digest(candidate: &BindingCandidate) -> anyhow::Result<String> {
    // Every disclosed fact is bound, including verified title/display text. That
    // makes any stale UI review detectable; candidate ordering independently
    // ignores presentation and publisher identity.
    let mut value = serde_json::to_value(candidate)?;
    value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Binding candidate did not serialize as an object"))?
        .remove("candidate_digest");
    Ok(sha256(&serde_json::to_vec(&value)?))
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::from("sha256:");
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use plurora_core::{
        ArtifactDescriptor, CapabilityDescriptor, ComponentBoundaryClaims, ComponentClaimStatus,
        ComponentDescriptor, ComponentTrustClass, COMPONENT_DESCRIPTOR_TYPE_URI,
    };
    use plurora_runtime::{
        BindingComponentDisclosure, BindingEffectiveStatus, BindingInstallationDisclosure,
        BindingWorkDisclosure, ComponentPin, InMemoryEventStore, InstallationRevisionPin,
        ProtocolContext, ResolvedPortPin, RunControl, Runtime, RuntimeConfig,
        UnavailableInstallationControl, UnavailableRunControl,
    };
    use plurora_work::{
        AcquisitionKind, AcquisitionRecord, AvailabilityPolicy, BindingPhase, EffectClass,
        InteractionModelId, NodeId, PortContract, PortEndpoint, PortMultiplicity, PortRole,
        SelectedTransport, TransportRequirements, WorkId, INTERACTION_CAPABILITY_UNARY,
    };
    use std::sync::atomic::{AtomicBool, Ordering};

    struct RemoveProviderAfterSelect {
        inner: Arc<PowerboxRegistry>,
        capabilities: StdRwLock<Option<Arc<plurora_runtime::CapabilityFabric>>>,
        provider_package_id: String,
        removed: AtomicBool,
    }

    struct FailOnceEventStore {
        inner: Arc<InMemoryEventStore>,
        fail_next_compare_append: AtomicBool,
    }

    #[async_trait]
    impl EventStore for FailOnceEventStore {
        async fn append(&self, event: EventEnvelope) -> anyhow::Result<()> {
            self.inner.append(event).await
        }

        async fn list_all(&self) -> anyhow::Result<Vec<EventEnvelope>> {
            self.inner.list_all().await
        }

        async fn list_session(
            &self,
            session_id: &plurora_core::SessionId,
        ) -> anyhow::Result<Vec<EventEnvelope>> {
            self.inner.list_session(session_id).await
        }

        async fn list_session_range(
            &self,
            session_id: &plurora_core::SessionId,
            after_sequence: Option<EventSequence>,
            limit: Option<usize>,
        ) -> anyhow::Result<Vec<EventEnvelope>> {
            self.inner
                .list_session_range(session_id, after_sequence, limit)
                .await
        }

        async fn next_sequence(
            &self,
            session_id: &plurora_core::SessionId,
        ) -> anyhow::Result<EventSequence> {
            self.inner.next_sequence(session_id).await
        }

        fn subscribe(&self) -> tokio::sync::broadcast::Receiver<EventEnvelope> {
            self.inner.subscribe()
        }

        async fn append_with_sequence_if_next(
            &self,
            session_id: plurora_core::SessionId,
            expected_next_sequence: EventSequence,
            writer_package_id: plurora_core::PackageId,
            kind: plurora_core::EventKind,
            schema_version: u16,
            payload_json: serde_json::Value,
            metadata_json: serde_json::Value,
        ) -> anyhow::Result<Option<EventEnvelope>> {
            if self.fail_next_compare_append.swap(false, Ordering::SeqCst) {
                anyhow::bail!("injected Runtime public append failure")
            }
            self.inner
                .append_with_sequence_if_next(
                    session_id,
                    expected_next_sequence,
                    writer_package_id,
                    kind,
                    schema_version,
                    payload_json,
                    metadata_json,
                )
                .await
        }
    }

    #[async_trait]
    impl PowerboxControl for RemoveProviderAfterSelect {
        fn install_runtime_broker(
            &self,
            broker: Weak<plurora_runtime::RunBindingBroker>,
        ) -> anyhow::Result<()> {
            self.inner.install_runtime_broker(broker)
        }

        async fn binding_select(
            &self,
            request: BindingSelectRequest,
        ) -> anyhow::Result<BindingMutationResult> {
            let result = self.inner.binding_select(request).await?;
            if !result.idempotent
                && result.binding.record.status == BindingDecisionStatus::Selected
                && !self.removed.swap(true, Ordering::SeqCst)
            {
                let capabilities = self
                    .capabilities
                    .read()
                    .expect("capability slot poisoned")
                    .clone()
                    .expect("Runtime capabilities installed");
                capabilities
                    .unregister_package(&self.provider_package_id)
                    .await;
            }
            Ok(result)
        }

        async fn binding_attached(&self, notice: BindingAttachmentNotice) -> anyhow::Result<()> {
            self.inner.binding_attached(notice).await
        }

        async fn validate_binding_current(
            &self,
            request: BindingCurrentValidationRequest,
        ) -> anyhow::Result<BindingSelectionRecord> {
            self.inner.validate_binding_current(request).await
        }

        async fn binding_drifted(
            &self,
            binding_id: &BindingId,
            reason_code: &str,
        ) -> anyhow::Result<PowerboxInvalidationResult> {
            self.inner.binding_drifted(binding_id, reason_code).await
        }

        async fn binding_detached(&self, notice: BindingCleanupNotice) -> anyhow::Result<()> {
            self.inner.binding_detached(notice).await
        }
    }

    async fn register_candidate_provider(
        runtime: &Runtime<InMemoryEventStore>,
        provider: &BindingEndpointPin,
    ) -> anyhow::Result<()> {
        let capability = CapabilityDescriptor {
            id: "tests/powerbox/save".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::Value::Null,
            output_schema: serde_json::Value::Null,
            streaming: true,
            side_effects: Vec::new(),
            description: None,
        };
        runtime
            .capabilities()
            .register_component(
                &provider.component.package_id,
                &ComponentDescriptor {
                    component_id: provider.component.component_id.clone(),
                    version: "1.0.0".to_string(),
                    artifact: provider.component.component_artifact.clone(),
                    behavior: ArtifactDescriptor {
                        digest: provider.component.behavior_digest.clone(),
                        ..descriptor('e')
                    },
                    trust_class: provider.component.trust_class,
                    claim_status: ComponentClaimStatus::Declared,
                    entry_kind: "subprocess".to_string(),
                    capability_ids: vec![capability.id.clone()],
                    enforced_boundaries: ComponentBoundaryClaims::default(),
                    protocol_implementations: Vec::new(),
                    content_roots: Vec::new(),
                    surfaces: Vec::new(),
                    annotations: BTreeMap::new(),
                },
                &[capability],
            )
            .await?;
        Ok(())
    }

    fn descriptor(byte: char) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: COMPONENT_DESCRIPTOR_TYPE_URI.to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", byte.to_string().repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    fn endpoint(
        installation_id: InstallationId,
        run_id: RunId,
        root_port: PortId,
        node_id: NodeId,
    ) -> BindingEndpointPin {
        BindingEndpointPin {
            installation: InstallationRevisionPin {
                installation_id,
                installation_revision: 1,
                work_revision: descriptor('a'),
                assembly_lock: descriptor('b'),
            },
            run: Some(RunRevisionPin {
                run_id,
                run_revision: 1,
                context_id: "run-context".to_string(),
            }),
            port: ResolvedPortPin {
                root_port: root_port.clone(),
                node_path: vec![node_id.clone()],
                leaf_port: PortEndpoint {
                    node_id: node_id.clone(),
                    port_id: root_port,
                },
                canonical_contract_digest: format!("sha256:{}", "c".repeat(64)),
            },
            component: ComponentPin {
                package_id: "tests/powerbox-provider".to_string(),
                component_id: "provider".to_string(),
                node_path: vec![node_id],
                component_artifact: descriptor('d'),
                behavior_digest: format!("sha256:{}", "e".repeat(64)),
                trust_class: ComponentTrustClass::SandboxedComponent,
            },
        }
    }

    fn export_descriptor(port_id: PortId) -> plurora_work::PortDescriptor {
        plurora_work::PortDescriptor {
            port_id,
            contract: PortContract {
                protocol_id: "plurora.capability".to_string(),
                interface_id: "tests/powerbox/save".to_string(),
                version: "1.0.0".to_string(),
                profiles: Vec::new(),
            },
            interaction: InteractionModelId(INTERACTION_CAPABILITY_UNARY.to_string()),
            role: PortRole::Export {
                multiplicity: PortMultiplicity { min: 0, max: None },
                effect_class: EffectClass::ExternalEffecting,
            },
            transport: TransportRequirements::default(),
            annotations: BTreeMap::new(),
        }
    }

    fn import_descriptor(port_id: PortId, max: Option<u16>) -> plurora_work::PortDescriptor {
        let mut descriptor = export_descriptor(port_id);
        descriptor.role = PortRole::Import {
            multiplicity: PortMultiplicity { min: 0, max },
            latest_binding_phase: BindingPhase::Runtime,
            availability: AvailabilityPolicy::Required,
            accepted_effects: vec![EffectClass::ExternalEffecting],
        };
        descriptor
    }

    fn work_disclosure(title: &str) -> BindingWorkDisclosure {
        BindingWorkDisclosure {
            work_id: WorkId::parse("tests/powerbox-work").unwrap(),
            title: title.to_string(),
        }
    }

    fn installation_disclosure(
        endpoint: &BindingEndpointPin,
        display_name: &str,
    ) -> BindingInstallationDisclosure {
        BindingInstallationDisclosure {
            installation_id: endpoint.installation.installation_id.clone(),
            installation_revision: endpoint.installation.installation_revision,
            display_name: display_name.to_string(),
            source: AcquisitionRecord {
                kind: AcquisitionKind::Package,
                source_ref: Some(descriptor('f')),
                provenance_refs: vec![descriptor('1')],
                update_channel: None,
            },
        }
    }

    fn component_disclosure(endpoint: &BindingEndpointPin) -> BindingComponentDisclosure {
        BindingComponentDisclosure {
            package_id: endpoint.component.package_id.clone(),
            component_id: endpoint.component.component_id.clone(),
            version: "1.0.0".to_string(),
            entry_kind: "subprocess".to_string(),
            trust_class: endpoint.component.trust_class,
            claim_status: ComponentClaimStatus::Declared,
            enforced_boundaries: ComponentBoundaryClaims::default(),
            component_artifact: endpoint.component.component_artifact.clone(),
            behavior: ArtifactDescriptor {
                digest: endpoint.component.behavior_digest.clone(),
                ..descriptor('e')
            },
            protocol_implementations: Vec::new(),
        }
    }

    #[derive(Clone)]
    struct CandidateRuns {
        runs: Vec<plurora_runtime::RunView>,
    }

    #[async_trait]
    impl RunControl for CandidateRuns {
        async fn list(
            &self,
            _request: plurora_runtime::RunListRequest,
        ) -> anyhow::Result<Vec<plurora_runtime::RunView>> {
            Ok(self.runs.clone())
        }

        async fn get(
            &self,
            request: plurora_runtime::RunGetRequest,
        ) -> anyhow::Result<Option<plurora_runtime::RunView>> {
            Ok(self
                .runs
                .iter()
                .find(|run| {
                    run.record.installation_id == request.installation_id
                        && run.record.run_id == request.run_id
                })
                .cloned())
        }

        async fn status(
            &self,
            _request: plurora_runtime::RunStatusRequest,
        ) -> anyhow::Result<plurora_runtime::RunStatusView> {
            anyhow::bail!("unused candidate test operation")
        }

        async fn start(
            &self,
            _request: plurora_runtime::RunStartRequest,
        ) -> anyhow::Result<plurora_runtime::RunStartResult> {
            anyhow::bail!("unused candidate test operation")
        }

        async fn stop(
            &self,
            _request: plurora_runtime::RunStopRequest,
        ) -> anyhow::Result<plurora_runtime::RunMutationResult> {
            anyhow::bail!("unused candidate test operation")
        }

        async fn package_activation_lost(
            &self,
            _package_id: &plurora_core::PackageId,
            _run_ids: Vec<RunId>,
        ) -> anyhow::Result<()> {
            anyhow::bail!("unused candidate test operation")
        }
    }

    struct CandidateInspector {
        consumer_installation_id: InstallationId,
        consumer: PowerboxEndpointInspection,
        provider: PowerboxEndpointInspection,
    }

    #[async_trait]
    impl PowerboxEndpointInspector for CandidateInspector {
        async fn inspect(
            &self,
            installation_id: &InstallationId,
            expected_installation_revision: u64,
            run: Option<RunRevisionPin>,
            port: &PortId,
            direction: PortDirection,
        ) -> anyhow::Result<PowerboxEndpointInspection> {
            let inspected = if installation_id == &self.consumer_installation_id {
                &self.consumer
            } else {
                &self.provider
            };
            anyhow::ensure!(
                inspected.endpoint.installation.installation_revision
                    == expected_installation_revision
                    && inspected.endpoint.run == run
                    && inspected.endpoint.port.root_port == *port
                    && matches!(
                        (&inspected.descriptor.role, direction),
                        (PortRole::Import { .. }, PortDirection::Import)
                            | (PortRole::Export { .. }, PortDirection::Export)
                    ),
                "candidate test endpoint mismatch"
            );
            Ok(inspected.clone())
        }
    }

    fn running_view(endpoint: &BindingEndpointPin) -> plurora_runtime::RunView {
        let run = endpoint.run.as_ref().unwrap();
        plurora_runtime::RunView {
            record: plurora_work::RunRecord {
                run_id: run.run_id.clone(),
                installation_id: endpoint.installation.installation_id.clone(),
                context_id: Some(run.context_id.clone()),
                status: RunStatus::Running,
                node_instances: Vec::new(),
                bindings: Vec::new(),
                started_at: Utc::now(),
                stopped_at: None,
                health: plurora_work::RunHealth {
                    status: plurora_work::HealthStatus::Healthy,
                    reason_code: None,
                    diagnostic_refs: Vec::new(),
                },
            },
            revision: run.run_revision,
            installation_revision: endpoint.installation.installation_revision,
            entrypoint_id: "default".to_string(),
        }
    }

    async fn candidate_fixture() -> anyhow::Result<(
        Arc<PowerboxRegistry>,
        BindingEndpointPin,
        BindingEndpointPin,
    )> {
        let (registry, consumer, provider, _, _) = candidate_fixture_with_store().await?;
        Ok((registry, consumer, provider))
    }

    async fn candidate_fixture_with_store() -> anyhow::Result<(
        Arc<PowerboxRegistry>,
        BindingEndpointPin,
        BindingEndpointPin,
        Arc<InMemoryEventStore>,
        Arc<InMemoryEventStore>,
    )> {
        let runtime_public_store = Arc::new(InMemoryEventStore::default());
        candidate_fixture_with_public_store(runtime_public_store.clone())
            .await
            .map(|(registry, consumer, provider, store)| {
                (registry, consumer, provider, store, runtime_public_store)
            })
    }

    async fn candidate_fixture_with_public_store(
        runtime_public_store: Arc<dyn EventStore>,
    ) -> anyhow::Result<(
        Arc<PowerboxRegistry>,
        BindingEndpointPin,
        BindingEndpointPin,
        Arc<InMemoryEventStore>,
    )> {
        candidate_fixture_with_public_store_and_consumer_max(runtime_public_store, Some(1)).await
    }

    async fn candidate_fixture_with_public_store_and_consumer_max(
        runtime_public_store: Arc<dyn EventStore>,
        consumer_max: Option<u16>,
    ) -> anyhow::Result<(
        Arc<PowerboxRegistry>,
        BindingEndpointPin,
        BindingEndpointPin,
        Arc<InMemoryEventStore>,
    )> {
        let consumer = endpoint(
            InstallationId::new(),
            RunId::new(),
            PortId::parse("save-import")?,
            NodeId::parse("consumer")?,
        );
        let provider = endpoint(
            InstallationId::new(),
            RunId::new(),
            PortId::parse("save-export")?,
            NodeId::parse("provider")?,
        );
        let runs = Arc::new(CandidateRuns {
            runs: vec![running_view(&consumer), running_view(&provider)],
        });
        let store = Arc::new(InMemoryEventStore::default());
        let registry = PowerboxRegistry::new(
            store.clone(),
            runtime_public_store,
            Arc::new(UnavailableInstallationControl),
            runs,
        )?;
        registry.install_inspector(Arc::new(CandidateInspector {
            consumer_installation_id: consumer.installation.installation_id.clone(),
            consumer: PowerboxEndpointInspection {
                endpoint: consumer.clone(),
                descriptor: import_descriptor(consumer.port.root_port.clone(), consumer_max),
                work: work_disclosure("Consumer Work"),
                installation: installation_disclosure(&consumer, "Consumer Installation"),
                component: component_disclosure(&consumer),
                phase: BindingPhase::Runtime,
                availability: AvailabilityPolicy::Required,
                capability: None,
            },
            provider: PowerboxEndpointInspection {
                endpoint: provider.clone(),
                descriptor: export_descriptor(provider.port.root_port.clone()),
                work: work_disclosure("Provider Work"),
                installation: installation_disclosure(&provider, "Provider Installation"),
                component: component_disclosure(&provider),
                phase: BindingPhase::Runtime,
                availability: AvailabilityPolicy::Required,
                capability: Some(CapabilityPin {
                    capability_id: "tests/powerbox/save".to_string(),
                    capability_version: "1.0.0".to_string(),
                }),
            },
        }))?;
        Ok((registry, consumer, provider, store))
    }

    fn candidate_exposure(
        provider: &BindingEndpointPin,
        audience: Vec<plurora_work::ResourceSelector>,
    ) -> ExposureProjection {
        let exposure_id = ExposureId::new();
        ExposureProjection {
            view: ExposureView {
                record: ExposureRecord {
                    exposure_id,
                    installation_id: provider.installation.installation_id.clone(),
                    run_id: provider.run.as_ref().map(|run| run.run_id.clone()),
                    export_port: provider.port.root_port.clone(),
                    audience,
                    expires_at: None,
                    status: ExposureStatus::Active,
                },
                revision: 1,
            },
            provider: provider.clone(),
            provider_descriptor: export_descriptor(provider.port.root_port.clone()),
            capability: CapabilityPin {
                capability_id: "tests/powerbox/save".to_string(),
                capability_version: "1.0.0".to_string(),
            },
            idempotency_key: uuid::Uuid::new_v4().to_string(),
            request_fingerprint: format!("sha256:{}", "9".repeat(64)),
            authority_basis: PowerboxAuthorityBasis::host(
                PowerboxAuthoritySubject::ExposureCreate {
                    installation_id: provider.installation.installation_id.clone(),
                    run_id: provider.run.as_ref().unwrap().run_id.clone(),
                    export_port: provider.port.root_port.clone(),
                },
            ),
        }
    }

    fn candidate_request(
        consumer: &BindingEndpointPin,
        visible_installation: &InstallationId,
    ) -> BindingCandidatesRequest {
        BindingCandidatesRequest {
            consumer_installation_id: consumer.installation.installation_id.clone(),
            expected_consumer_installation_revision: consumer.installation.installation_revision,
            phase: BindingPhase::Runtime,
            consumer_run: consumer.run.clone(),
            import_port: consumer.port.root_port.clone(),
            preferences: Vec::new(),
            query: Some(plurora_runtime::PowerboxQueryContext {
                unrestricted: false,
                visible_resources: vec![plurora_work::ResourceSelector {
                    kind: "installation".to_string(),
                    id: visible_installation.to_string(),
                }],
                audience_principals: Vec::new(),
            }),
        }
    }

    #[tokio::test]
    async fn audience_only_consumer_sees_provider_without_provider_ownership() -> anyhow::Result<()>
    {
        let (registry, consumer, provider) = candidate_fixture().await?;
        let audience = vec![plurora_work::ResourceSelector {
            kind: "installation".to_string(),
            id: consumer.installation.installation_id.to_string(),
        }];
        let exposure = candidate_exposure(&provider, audience);
        registry
            .projection
            .write()
            .await
            .exposures
            .insert(exposure.view.record.exposure_id.clone(), exposure);
        let result = registry
            .calculate_candidates(&candidate_request(
                &consumer,
                &consumer.installation.installation_id,
            ))
            .await?;
        assert_eq!(result.candidates.len(), 1);
        let candidate = &result.candidates[0];
        assert_eq!(candidate.exposure.record.audience.len(), 1);
        assert_eq!(
            candidate.consumer_port.contract.protocol_id,
            "plurora.capability"
        );
        assert_eq!(
            candidate.provider_port.contract.interface_id,
            "tests/powerbox/save"
        );
        assert_eq!(candidate.provider_work.title, "Provider Work");
        assert_eq!(
            candidate.provider_installation.source.provenance_refs.len(),
            1
        );
        assert_eq!(
            candidate.provider_component.claim_status,
            ComponentClaimStatus::Declared
        );
        let wire = serde_json::to_string(candidate)?;
        for private in [
            "authority_basis",
            "grant_reference",
            "raw_handle",
            "host_path",
            "secret_value",
            "stderr",
        ] {
            assert!(!wire.contains(private), "candidate leaked {private}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn candidate_digest_binds_phase_exposure_ports_origin_and_evidence() -> anyhow::Result<()>
    {
        let (registry, consumer, provider) = candidate_fixture().await?;
        let exposure = candidate_exposure(
            &provider,
            vec![plurora_work::ResourceSelector {
                kind: "installation".to_string(),
                id: consumer.installation.installation_id.to_string(),
            }],
        );
        registry
            .projection
            .write()
            .await
            .exposures
            .insert(exposure.view.record.exposure_id.clone(), exposure);
        let candidate = registry
            .calculate_candidates(&candidate_request(
                &consumer,
                &consumer.installation.installation_id,
            ))
            .await?
            .candidates
            .into_iter()
            .next()
            .expect("one candidate");
        let original = candidate.candidate_digest.clone();
        let mut variants = Vec::new();
        let mut changed = candidate.clone();
        changed.phase = BindingPhase::Launch;
        variants.push(changed);
        let mut changed = candidate.clone();
        changed.exposure.record.audience[0].id.push_str("-changed");
        variants.push(changed);
        let mut changed = candidate.clone();
        changed
            .provider_port
            .contract
            .profiles
            .push("tests/profile/v2".to_string());
        variants.push(changed);
        let mut changed = candidate.clone();
        changed.provider_work.title.push_str(" changed");
        variants.push(changed);
        let mut changed = candidate.clone();
        changed.provider_component.behavior.digest = format!("sha256:{}", "2".repeat(64));
        variants.push(changed);
        for changed in variants {
            assert_ne!(candidate_digest(&changed)?, original);
        }
        Ok(())
    }

    #[tokio::test]
    async fn candidate_order_ignores_publisher_and_presentation() -> anyhow::Result<()> {
        let (registry, consumer, provider) = candidate_fixture().await?;
        let exposure = candidate_exposure(
            &provider,
            vec![plurora_work::ResourceSelector {
                kind: "installation".to_string(),
                id: consumer.installation.installation_id.to_string(),
            }],
        );
        registry
            .projection
            .write()
            .await
            .exposures
            .insert(exposure.view.record.exposure_id.clone(), exposure);
        let base = registry
            .calculate_candidates(&candidate_request(
                &consumer,
                &consumer.installation.installation_id,
            ))
            .await?
            .candidates
            .into_iter()
            .next()
            .expect("one candidate");
        let mut candidates = Vec::new();
        for (publisher, title) in [
            ("plurora/first-party", "First Party"),
            ("third-party/provider", "Third Party"),
            ("another/provider", "Another Provider"),
        ] {
            let mut candidate = base.clone();
            candidate.exposure.record.exposure_id = ExposureId::new();
            candidate.provider.component.package_id = publisher.to_string();
            candidate.provider_component.package_id = publisher.to_string();
            candidate.provider_work.title = title.to_string();
            candidate.provider_installation.display_name = title.to_string();
            candidate.candidate_digest = candidate_digest(&candidate)?;
            candidates.push(candidate);
        }
        candidates.reverse();
        let mut expected = candidates
            .iter()
            .map(|candidate| candidate.exposure.record.exposure_id.clone())
            .collect::<Vec<_>>();
        expected.sort();
        let actual = plurora_runtime::sort_provider_candidates(candidates)?
            .into_iter()
            .map(|candidate| candidate.exposure.record.exposure_id)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        Ok(())
    }

    #[tokio::test]
    async fn tampered_phase_or_candidate_digest_never_selects() -> anyhow::Result<()> {
        let (registry, consumer, provider, source, _) = candidate_fixture_with_store().await?;
        let exposure = candidate_exposure(
            &provider,
            vec![plurora_work::ResourceSelector {
                kind: "installation".to_string(),
                id: consumer.installation.installation_id.to_string(),
            }],
        );
        let exposure_id = exposure.view.record.exposure_id.clone();
        registry
            .projection
            .write()
            .await
            .exposures
            .insert(exposure_id.clone(), exposure);
        let candidate = registry
            .calculate_candidates(&candidate_request(
                &consumer,
                &consumer.installation.installation_id,
            ))
            .await?
            .candidates
            .into_iter()
            .next()
            .expect("one candidate");
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig {
                powerbox_control: registry,
                ..RuntimeConfig::default()
            },
        );
        let params = serde_json::json!({
            "consumer_installation_id": consumer.installation.installation_id,
            "expected_consumer_installation_revision": consumer.installation.installation_revision,
            "phase": "runtime",
            "consumer_run": consumer.run,
            "import_port": consumer.port.root_port,
            "exposure_id": exposure_id,
            "expected_exposure_revision": 1,
            "provider_installation_id": provider.installation.installation_id,
            "expected_provider_installation_revision": provider.installation.installation_revision,
            "candidate_digest": candidate.candidate_digest,
            "idempotency_key": "tamper-test",
        });
        let mut wrong_digest = params.clone();
        wrong_digest["candidate_digest"] = serde_json::json!(format!("sha256:{}", "0".repeat(64)));
        assert!(runtime
            .call_protocol(
                &ProtocolContext::host_dev("tampered-digest"),
                "host.binding.select",
                wrong_digest,
            )
            .await
            .is_err());
        let mut wrong_phase = params;
        wrong_phase["phase"] = serde_json::json!("launch");
        wrong_phase
            .as_object_mut()
            .expect("request object")
            .remove("consumer_run");
        assert!(runtime
            .call_protocol(
                &ProtocolContext::host_dev("tampered-phase"),
                "host.binding.select",
                wrong_phase,
            )
            .await
            .is_err());
        assert!(source
            .list_session(&POWERBOX_SESSION.to_string())
            .await?
            .iter()
            .all(|event| event.kind != EVENT_BINDING_SELECTED));
        Ok(())
    }

    #[tokio::test]
    async fn saturated_candidate_query_returns_stable_capacity_gap() -> anyhow::Result<()> {
        let (registry, consumer, provider) = candidate_fixture().await?;
        let exposure = candidate_exposure(
            &provider,
            vec![plurora_work::ResourceSelector {
                kind: "installation".to_string(),
                id: consumer.installation.installation_id.to_string(),
            }],
        );
        let exposure_id = exposure.view.record.exposure_id.clone();
        let mut selected = selected_projection(consumer.clone(), provider);
        selected.view.record.exposure_id = exposure_id.clone();
        let binding_id = selected.view.record.binding_id.clone();
        {
            let mut state = registry.projection.write().await;
            state.exposures.insert(exposure_id, exposure);
            state.bindings.insert(binding_id, selected);
        }
        let result = registry
            .calculate_candidates(&candidate_request(
                &consumer,
                &consumer.installation.installation_id,
            ))
            .await?;
        assert!(result.candidates.is_empty());
        assert_eq!(
            result.gaps[0].reason_code,
            "consumer_import_capacity_exhausted"
        );
        Ok(())
    }

    #[tokio::test]
    async fn binding_list_never_discloses_provider_pins_across_grant_audiences(
    ) -> anyhow::Result<()> {
        let (registry, consumer, provider) = candidate_fixture().await?;
        let exposure_a = candidate_exposure(
            &provider,
            vec![plurora_work::ResourceSelector {
                kind: "grant".to_string(),
                id: "grant-a".to_string(),
            }],
        );
        let exposure_b = candidate_exposure(
            &provider,
            vec![plurora_work::ResourceSelector {
                kind: "grant".to_string(),
                id: "grant-b".to_string(),
            }],
        );
        let mut binding_a = selected_projection(consumer.clone(), provider.clone());
        binding_a.view.record.exposure_id = exposure_a.view.record.exposure_id.clone();
        let mut binding_b = selected_projection(consumer, provider);
        binding_b.view.record.exposure_id = exposure_b.view.record.exposure_id.clone();
        let binding_a_id = binding_a.view.record.binding_id.clone();
        let binding_b_id = binding_b.view.record.binding_id.clone();
        {
            let mut state = registry.projection.write().await;
            state
                .exposures
                .insert(exposure_a.view.record.exposure_id.clone(), exposure_a);
            state
                .exposures
                .insert(exposure_b.view.record.exposure_id.clone(), exposure_b);
            state.bindings.insert(binding_a_id.clone(), binding_a);
            state.bindings.insert(binding_b_id.clone(), binding_b);
        }

        let list_for = |grant: &str| BindingListRequest {
            query: Some(plurora_runtime::PowerboxQueryContext {
                unrestricted: false,
                visible_resources: Vec::new(),
                audience_principals: vec![plurora_work::ResourceSelector {
                    kind: "grant".to_string(),
                    id: grant.to_string(),
                }],
            }),
            ..BindingListRequest::default()
        };
        let visible_a = registry.binding_list(list_for("grant-a")).await?;
        assert_eq!(visible_a.len(), 1);
        assert_eq!(visible_a[0].record.binding_id, binding_a_id);
        let visible_b = registry.binding_list(list_for("grant-b")).await?;
        assert_eq!(visible_b.len(), 1);
        assert_eq!(visible_b[0].record.binding_id, binding_b_id);
        Ok(())
    }

    #[tokio::test]
    async fn selected_provider_lookup_drift_closes_real_powerbox_claim_once() -> anyhow::Result<()>
    {
        for round in 0..20 {
            let (registry, consumer, provider, store, runtime_public_store) =
                candidate_fixture_with_store().await?;
            let exposure = candidate_exposure(
                &provider,
                vec![plurora_work::ResourceSelector {
                    kind: "installation".to_string(),
                    id: consumer.installation.installation_id.to_string(),
                }],
            );
            let exposure_id = exposure.view.record.exposure_id.clone();
            registry
                .projection
                .write()
                .await
                .exposures
                .insert(exposure_id.clone(), exposure);
            let candidates = registry
                .calculate_candidates(&candidate_request(
                    &consumer,
                    &consumer.installation.installation_id,
                ))
                .await?;
            let candidate = candidates
                .candidates
                .into_iter()
                .next()
                .expect("one exact provider candidate");
            let control = Arc::new(RemoveProviderAfterSelect {
                inner: registry.clone(),
                capabilities: StdRwLock::new(None),
                provider_package_id: provider.component.package_id.clone(),
                removed: AtomicBool::new(false),
            });
            let runtime = Runtime::new(
                Arc::new(InMemoryEventStore::default()),
                RuntimeConfig {
                    powerbox_control: control.clone(),
                    ..RuntimeConfig::default()
                },
            );
            *control
                .capabilities
                .write()
                .expect("capability slot poisoned") = Some(runtime.capabilities());
            register_candidate_provider(&runtime, &provider).await?;
            let params = serde_json::json!({
                "consumer_installation_id": consumer.installation.installation_id.clone(),
                "expected_consumer_installation_revision": consumer.installation.installation_revision,
                "phase": "runtime",
                "consumer_run": consumer.run.clone(),
                "import_port": consumer.port.root_port.clone(),
                "exposure_id": exposure_id,
                "expected_exposure_revision": 1,
                "provider_installation_id": provider.installation.installation_id.clone(),
                "expected_provider_installation_revision": provider.installation.installation_revision,
                "candidate_digest": candidate.candidate_digest,
                "idempotency_key": format!("provider-lookup-drift-{round}"),
            });
            let first = runtime
                .call_protocol(
                    &ProtocolContext::host_dev("provider-lookup-drift"),
                    "host.binding.select",
                    params.clone(),
                )
                .await
                .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message))?;
            assert_eq!(first["binding"]["record"]["status"], "revoked");
            let replay = runtime
                .call_protocol(
                    &ProtocolContext::host_dev("provider-lookup-drift"),
                    "host.binding.select",
                    params,
                )
                .await
                .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message))?;
            assert_eq!(replay["binding"]["record"]["status"], "revoked");

            let events = store.list_session(&POWERBOX_SESSION.to_string()).await?;
            assert_eq!(
                events
                    .iter()
                    .filter(|event| event.kind == EVENT_BINDING_SELECTED)
                    .count(),
                1
            );
            assert!(registry.projection.read().await.binding_closing.is_empty());
            assert_eq!(
                events
                    .iter()
                    .filter(|event| event.kind == PRIVATE_BINDING_CLOSING)
                    .count(),
                1
            );
            let public = runtime_public_store
                .list_session(&POWERBOX_SESSION.to_string())
                .await?;
            assert_eq!(
                public
                    .iter()
                    .filter(|event| event.kind == EVENT_BINDING_SELECTED)
                    .count(),
                1
            );
            assert_eq!(
                public
                    .iter()
                    .filter(|event| event.kind == EVENT_BINDING_REVOKED)
                    .count(),
                1
            );
            assert!(public.iter().all(|event| {
                event.writer_package_id == PLATFORM_RUNTIME_ID
                    && event.metadata["authority"] == "none"
                    && !event.payload.to_string().contains("authority_basis")
                    && !event.payload.to_string().contains("idempotency_key")
                    && !event.payload.to_string().contains("request_fingerprint")
            }));
            assert_eq!(
                events
                    .iter()
                    .filter(|event| event.kind == EVENT_BINDING_REVOKED)
                    .count(),
                1
            );
            assert_eq!(
                runtime.run_binding_broker().active_generation_count().await,
                0
            );
            assert!(runtime
                .stream_registry()
                .list_invocations()
                .await
                .is_empty());
        }
        Ok(())
    }

    #[tokio::test]
    async fn all_powerbox_mutations_replay_before_world_validation_after_terminal_drift(
    ) -> anyhow::Result<()> {
        let (registry, consumer, provider) = candidate_fixture().await?;
        let audience = vec![plurora_work::ResourceSelector {
            kind: "installation".to_string(),
            id: consumer.installation.installation_id.to_string(),
        }];
        let create = ExposureCreateRequest {
            installation_id: provider.installation.installation_id.clone(),
            expected_installation_revision: provider.installation.installation_revision,
            run_id: provider.run.as_ref().unwrap().run_id.clone(),
            expected_run_revision: provider.run.as_ref().unwrap().run_revision,
            export_port: provider.port.root_port.clone(),
            audience: audience.clone(),
            expires_at: None,
            idempotency_key: "replay-create".to_string(),
            authority: None,
        };
        let mut exposure = candidate_exposure(&provider, audience);
        exposure.view.revision = 2;
        exposure.view.record.status = ExposureStatus::Revoked;
        exposure.idempotency_key = create.idempotency_key.clone();
        exposure.request_fingerprint = fingerprint(&create)?;
        let exposure_id = exposure.view.record.exposure_id.clone();
        let revoke = ExposureRevokeRequest {
            installation_id: provider.installation.installation_id.clone(),
            expected_installation_revision: provider.installation.installation_revision,
            run_id: provider.run.as_ref().unwrap().run_id.clone(),
            expected_run_revision: provider.run.as_ref().unwrap().run_revision,
            export_port: provider.port.root_port.clone(),
            exposure_id: exposure_id.clone(),
            expected_exposure_revision: 1,
            idempotency_key: "replay-exposure-revoke".to_string(),
            authority: None,
        };

        let mut binding = selected_projection(consumer.clone(), provider.clone());
        binding.view.record.exposure_id = exposure_id.clone();
        binding.view.revision = 2;
        binding.view.record.status = BindingDecisionStatus::Revoked;
        binding.view.effective_status = BindingEffectiveStatus::Broken {
            reason_code: "world_drifted".to_string(),
        };
        let select = BindingSelectRequest {
            consumer_installation_id: consumer.installation.installation_id.clone(),
            expected_consumer_installation_revision: consumer.installation.installation_revision,
            phase: BindingPhase::Runtime,
            consumer_run: consumer.run.clone(),
            import_port: consumer.port.root_port.clone(),
            exposure_id: exposure_id.clone(),
            expected_exposure_revision: 1,
            provider_installation_id: provider.installation.installation_id.clone(),
            expected_provider_installation_revision: provider.installation.installation_revision,
            candidate_digest: binding.view.record.candidate_digest.clone(),
            idempotency_key: "replay-select".to_string(),
            query: None,
            authority: None,
        };
        binding.idempotency_key = select.idempotency_key.clone();
        binding.request_fingerprint = fingerprint(&select)?;
        let binding_id = binding.view.record.binding_id.clone();
        let binding_revoke = BindingRevokeRequest {
            consumer_installation_id: consumer.installation.installation_id.clone(),
            expected_consumer_installation_revision: consumer.installation.installation_revision,
            consumer_run: consumer.run.clone(),
            import_port: consumer.port.root_port.clone(),
            exposure_id: exposure_id.clone(),
            binding_id: binding_id.clone(),
            expected_binding_revision: 1,
            idempotency_key: "replay-binding-revoke".to_string(),
            authority: None,
        };
        {
            let mut state = registry.projection.write().await;
            state.exposure_idempotency.insert(
                create.idempotency_key.clone(),
                (fingerprint(&create)?, exposure_id.clone()),
            );
            state.exposure_idempotency.insert(
                revoke.idempotency_key.clone(),
                (fingerprint(&revoke)?, exposure_id.clone()),
            );
            state.exposures.insert(exposure_id.clone(), exposure);
            state.binding_idempotency.insert(
                select.idempotency_key.clone(),
                (fingerprint(&select)?, binding_id.clone()),
            );
            state.binding_idempotency.insert(
                binding_revoke.idempotency_key.clone(),
                (fingerprint(&binding_revoke)?, binding_id.clone()),
            );
            state.bindings.insert(binding_id, binding);
        }
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig {
                powerbox_control: registry,
                ..RuntimeConfig::default()
            },
        );
        let context = ProtocolContext::host_dev("powerbox-replay-world-drift");
        for (method, request) in [
            ("host.exposure.create", serde_json::to_value(&create)?),
            ("host.exposure.revoke", serde_json::to_value(&revoke)?),
            ("host.binding.select", serde_json::to_value(&select)?),
            (
                "host.binding.revoke",
                serde_json::to_value(&binding_revoke)?,
            ),
        ] {
            let result = runtime
                .call_protocol(&context, method, request)
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            assert_eq!(result["idempotent"], true, "{method} did not replay");
        }
        let mut conflict = serde_json::to_value(create)?;
        conflict["expected_run_revision"] = serde_json::json!(99);
        assert!(runtime
            .call_protocol(&context, "host.exposure.create", conflict)
            .await
            .unwrap_err()
            .message
            .contains("idempotency"));
        Ok(())
    }

    #[tokio::test]
    async fn durable_exposure_close_intent_blocks_candidates_select_and_replay_attach_for_twenty_rounds(
    ) -> anyhow::Result<()> {
        for round in 0..20 {
            let runtime_public_store = Arc::new(InMemoryEventStore::default());
            let (registry, consumer, provider, source_store) =
                candidate_fixture_with_public_store_and_consumer_max(
                    runtime_public_store.clone(),
                    None,
                )
                .await?;
            let exposure = candidate_exposure(
                &provider,
                vec![plurora_work::ResourceSelector {
                    kind: "installation".to_string(),
                    id: consumer.installation.installation_id.to_string(),
                }],
            );
            let exposure_id = exposure.view.record.exposure_id.clone();
            {
                let _mutation = registry.mutation.lock().await;
                let mut state = registry.projection.write().await;
                registry
                    .append_exposure(&mut state, EVENT_EXPOSURE_CREATED, exposure, None)
                    .await?;
            }
            let candidate_request =
                candidate_request(&consumer, &consumer.installation.installation_id);
            let candidate = registry
                .calculate_candidates(&candidate_request)
                .await?
                .candidates
                .into_iter()
                .next()
                .expect("active Exposure must initially be selectable");
            let idempotency_key = format!("closing-select-replay-{round}");
            let select_request = BindingSelectRequest {
                consumer_installation_id: consumer.installation.installation_id.clone(),
                expected_consumer_installation_revision: consumer
                    .installation
                    .installation_revision,
                phase: BindingPhase::Runtime,
                consumer_run: consumer.run.clone(),
                import_port: consumer.port.root_port.clone(),
                exposure_id: exposure_id.clone(),
                expected_exposure_revision: 1,
                provider_installation_id: provider.installation.installation_id.clone(),
                expected_provider_installation_revision: provider
                    .installation
                    .installation_revision,
                candidate_digest: candidate.candidate_digest.clone(),
                idempotency_key: idempotency_key.clone(),
                query: Some(plurora_runtime::PowerboxQueryContext {
                    unrestricted: true,
                    ..plurora_runtime::PowerboxQueryContext::default()
                }),
                authority: None,
            };
            let binding_id = BindingId::new();
            let selected = BindingProjection {
                view: BindingView {
                    record: BindingSelectionRecord {
                        binding_id: binding_id.clone(),
                        candidate_digest: candidate.candidate_digest,
                        exposure_id: candidate.exposure.record.exposure_id,
                        exposure_revision: candidate.exposure.revision,
                        consumer: candidate.consumer,
                        provider: candidate.provider,
                        capability: candidate.capability,
                        transport: candidate.transport,
                        phase: candidate.phase,
                        availability: candidate.availability,
                        effective_expires_at: candidate.effective_expires_at,
                        status: BindingDecisionStatus::Selected,
                    },
                    revision: 1,
                    effective_status: BindingEffectiveStatus::Detached,
                },
                idempotency_key,
                request_fingerprint: fingerprint(&select_request)?,
                authority_basis: PowerboxAuthorityBasis::host(
                    PowerboxAuthoritySubject::BindingSelect {
                        consumer_installation_id: consumer.installation.installation_id.clone(),
                        consumer_run: consumer.run.clone(),
                        import_port: consumer.port.root_port.clone(),
                        exposure_id: exposure_id.clone(),
                    },
                ),
            };
            {
                let _mutation = registry.mutation.lock().await;
                let mut state = registry.projection.write().await;
                registry
                    .append_binding(&mut state, EVENT_BINDING_SELECTED, selected, None)
                    .await?;
            }

            let runtime = Arc::new(Runtime::new(
                runtime_public_store,
                RuntimeConfig {
                    powerbox_control: registry.clone(),
                    ..RuntimeConfig::default()
                },
            ));
            let close_barrier = Arc::new(TestCloseBarrier::default());
            *registry
                .test_close_barrier
                .write()
                .expect("Powerbox test close barrier lock poisoned") = Some(close_barrier.clone());
            let closing = registry
                .begin_exposure_closing(
                    &exposure_id,
                    ExposureStatus::Revoked,
                    "exposure_revoked",
                    None,
                    None,
                )
                .await?
                .expect("active Exposure must create a durable closing obligation");
            let completing = {
                let registry = registry.clone();
                tokio::spawn(async move { registry.complete_exposure_closing(closing).await })
            };
            close_barrier.wait_entered().await;
            assert!(!completing.is_finished());

            let rejected_candidates = registry.calculate_candidates(&candidate_request).await?;
            assert!(rejected_candidates.candidates.is_empty());
            assert_eq!(
                rejected_candidates.gaps[0].reason_code,
                "binding_unavailable"
            );

            let context = ProtocolContext::host_dev("closing-admission");
            let mut new_select = serde_json::to_value(&select_request)?;
            new_select["idempotency_key"] =
                serde_json::json!(format!("closing-new-select-{round}"));
            assert!(runtime
                .call_protocol(&context, "host.binding.select", new_select)
                .await
                .is_err());
            assert_eq!(
                runtime.run_binding_broker().active_generation_count().await,
                0
            );
            assert_eq!(
                source_store
                    .list_session(&POWERBOX_SESSION.to_string())
                    .await?
                    .iter()
                    .filter(|event| event.kind == EVENT_BINDING_SELECTED)
                    .count(),
                1
            );

            let replay = {
                let runtime = runtime.clone();
                let params = serde_json::to_value(&select_request)?;
                tokio::spawn(async move {
                    runtime
                        .call_protocol(&context, "host.binding.select", params)
                        .await
                })
            };
            tokio::task::yield_now().await;
            assert!(!replay.is_finished());
            assert_eq!(
                runtime.run_binding_broker().active_generation_count().await,
                0
            );
            close_barrier.release();

            let affected = completing.await??;
            assert_eq!(affected.affected_binding_ids, vec![binding_id.clone()]);
            let replay = replay
                .await?
                .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message))?;
            assert_eq!(replay["idempotent"], true);
            assert_eq!(replay["binding"]["record"]["status"], "revoked");
            assert_eq!(
                runtime.run_binding_broker().active_generation_count().await,
                0
            );
            let state = registry.projection.read().await;
            assert_eq!(
                state.exposures[&exposure_id].view.record.status,
                ExposureStatus::Revoked
            );
            assert_eq!(
                state.bindings[&binding_id].view.record.status,
                BindingDecisionStatus::Revoked
            );
            drop(state);
            assert_eq!(
                source_store
                    .list_session(&POWERBOX_SESSION.to_string())
                    .await?
                    .iter()
                    .filter(|event| event.kind == EVENT_BINDING_SELECTED)
                    .count(),
                1
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn exposure_terminalization_rescans_an_abnormal_post_intent_association(
    ) -> anyhow::Result<()> {
        let (registry, consumer, provider, source_store, _) =
            candidate_fixture_with_store().await?;
        let exposure = candidate_exposure(
            &provider,
            vec![plurora_work::ResourceSelector {
                kind: "installation".to_string(),
                id: consumer.installation.installation_id.to_string(),
            }],
        );
        let exposure_id = exposure.view.record.exposure_id.clone();
        let mut first = selected_projection(consumer.clone(), provider.clone());
        first.view.record.exposure_id = exposure_id.clone();
        let first_id = first.view.record.binding_id.clone();
        {
            let _mutation = registry.mutation.lock().await;
            let mut state = registry.projection.write().await;
            registry
                .append_exposure(&mut state, EVENT_EXPOSURE_CREATED, exposure, None)
                .await?;
            registry
                .append_binding(&mut state, EVENT_BINDING_SELECTED, first, None)
                .await?;
        }
        let closing = registry
            .begin_exposure_closing(
                &exposure_id,
                ExposureStatus::Revoked,
                "exposure_revoked",
                None,
                None,
            )
            .await?
            .expect("active Exposure must begin closing");
        assert_eq!(closing.affected_binding_ids, vec![first_id.clone()]);

        // Inject the journal shape that correct admission now prevents. Close
        // completion must still discover and terminalize it before the Exposure.
        let mut late = selected_projection(consumer, provider);
        late.view.record.exposure_id = exposure_id.clone();
        let late_id = late.view.record.binding_id.clone();
        {
            let _mutation = registry.mutation.lock().await;
            let mut state = registry.projection.write().await;
            registry
                .append_binding(&mut state, EVENT_BINDING_SELECTED, late, None)
                .await?;
        }

        let result = registry.complete_exposure_closing(closing).await?;
        let mut expected = vec![first_id.clone(), late_id.clone()];
        expected.sort();
        assert_eq!(result.affected_binding_ids, expected);
        let state = registry.projection.read().await;
        assert_eq!(
            state.exposures[&exposure_id].view.record.status,
            ExposureStatus::Revoked
        );
        assert_eq!(
            state.bindings[&first_id].view.record.status,
            BindingDecisionStatus::Revoked
        );
        assert_eq!(
            state.bindings[&late_id].view.record.status,
            BindingDecisionStatus::Revoked
        );
        drop(state);
        assert_eq!(
            source_store
                .list_session(&POWERBOX_SESSION.to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_BINDING_REVOKED)
                .count(),
            2
        );
        Ok(())
    }

    #[tokio::test]
    async fn candidate_overflow_counts_only_visible_exposures() -> anyhow::Result<()> {
        let (registry, consumer, provider) = candidate_fixture().await?;
        let visible = vec![plurora_work::ResourceSelector {
            kind: "installation".to_string(),
            id: consumer.installation.installation_id.to_string(),
        }];
        let hidden = vec![plurora_work::ResourceSelector {
            kind: "installation".to_string(),
            id: InstallationId::new().to_string(),
        }];
        {
            let mut state = registry.projection.write().await;
            for _ in 0..257 {
                let exposure = candidate_exposure(&provider, hidden.clone());
                state
                    .exposures
                    .insert(exposure.view.record.exposure_id.clone(), exposure);
            }
            let exposure = candidate_exposure(&provider, visible.clone());
            state
                .exposures
                .insert(exposure.view.record.exposure_id.clone(), exposure);
        }
        let request = candidate_request(&consumer, &consumer.installation.installation_id);
        assert_eq!(
            registry
                .calculate_candidates(&request)
                .await?
                .candidates
                .len(),
            1
        );

        let (overflow, overflow_consumer, overflow_provider) = candidate_fixture().await?;
        let overflow_visible = vec![plurora_work::ResourceSelector {
            kind: "installation".to_string(),
            id: overflow_consumer.installation.installation_id.to_string(),
        }];
        {
            let mut state = overflow.projection.write().await;
            for _ in 0..257 {
                let exposure = candidate_exposure(&overflow_provider, overflow_visible.clone());
                state
                    .exposures
                    .insert(exposure.view.record.exposure_id.clone(), exposure);
            }
        }
        assert!(overflow
            .calculate_candidates(&candidate_request(
                &overflow_consumer,
                &overflow_consumer.installation.installation_id,
            ))
            .await
            .unwrap_err()
            .to_string()
            .contains("MAX_PROVIDER_CANDIDATES"));
        Ok(())
    }

    fn selected_projection(
        consumer: BindingEndpointPin,
        provider: BindingEndpointPin,
    ) -> BindingProjection {
        let exposure_id = ExposureId::new();
        let binding_id = BindingId::new();
        BindingProjection {
            view: BindingView {
                record: BindingSelectionRecord {
                    binding_id,
                    candidate_digest: format!("sha256:{}", "7".repeat(64)),
                    exposure_id: exposure_id.clone(),
                    exposure_revision: 1,
                    consumer: consumer.clone(),
                    provider,
                    capability: CapabilityPin {
                        capability_id: "tests/powerbox/save".to_string(),
                        capability_version: "1.0.0".to_string(),
                    },
                    transport: SelectedTransport {
                        class_id: "plurora.transport.capability/v1".to_string(),
                        properties: BTreeMap::new(),
                    },
                    phase: BindingPhase::Runtime,
                    availability: AvailabilityPolicy::Required,
                    effective_expires_at: None,
                    status: BindingDecisionStatus::Selected,
                },
                revision: 1,
                effective_status: BindingEffectiveStatus::Detached,
            },
            idempotency_key: uuid::Uuid::new_v4().to_string(),
            request_fingerprint: format!("sha256:{}", "8".repeat(64)),
            authority_basis: PowerboxAuthorityBasis::host(
                PowerboxAuthoritySubject::BindingSelect {
                    consumer_installation_id: consumer.installation.installation_id,
                    consumer_run: consumer.run,
                    import_port: consumer.port.root_port,
                    exposure_id,
                },
            ),
        }
    }

    #[tokio::test]
    async fn exact_provider_max_one_is_shared_across_exposures_for_twenty_races(
    ) -> anyhow::Result<()> {
        for _ in 0..20 {
            let (registry, consumer, provider) = candidate_fixture().await?;
            let mut other_consumer = consumer.clone();
            other_consumer.installation.installation_id = InstallationId::new();
            other_consumer.run.as_mut().unwrap().run_id = RunId::new();
            let mut left_provider = provider.clone();
            left_provider.port.root_port = PortId::parse("provider-export-alias-left")?;
            let mut right_provider = provider.clone();
            right_provider.port.root_port = PortId::parse("provider-export-alias-right")?;
            let left_selection = selected_projection(consumer, left_provider);
            let right_selection = selected_projection(other_consumer, right_provider);
            assert_ne!(
                left_selection.view.record.exposure_id,
                right_selection.view.record.exposure_id
            );
            let capability = left_selection.view.record.capability.clone();
            let barrier = Arc::new(tokio::sync::Barrier::new(3));
            let reserve = |selection: BindingProjection| {
                let registry = registry.clone();
                let capability = capability.clone();
                let barrier = barrier.clone();
                tokio::spawn(async move {
                    barrier.wait().await;
                    let _mutation = registry.mutation.lock().await;
                    let mut state = registry.projection.write().await;
                    let provider = selection.view.record.provider.clone();
                    if ensure_provider_binding_capacity(&state, &provider, &capability, Some(1))
                        .is_err()
                    {
                        return false;
                    }
                    state
                        .bindings
                        .insert(selection.view.record.binding_id.clone(), selection);
                    true
                })
            };
            let left = reserve(left_selection);
            let right = reserve(right_selection);
            barrier.wait().await;
            let (left, right) = tokio::join!(left, right);
            assert_eq!(usize::from(left?) + usize::from(right?), 1);
            assert_eq!(registry.projection.read().await.bindings.len(), 1);
        }
        Ok(())
    }

    #[tokio::test]
    async fn exact_consumer_max_one_is_shared_across_root_aliases_for_twenty_races(
    ) -> anyhow::Result<()> {
        for _ in 0..20 {
            let (registry, consumer, provider) = candidate_fixture().await?;
            let mut left_consumer = consumer.clone();
            left_consumer.port.root_port = PortId::parse("consumer-import-alias-left")?;
            let mut right_consumer = consumer.clone();
            right_consumer.port.root_port = PortId::parse("consumer-import-alias-right")?;
            let left_selection = selected_projection(left_consumer, provider.clone());
            let right_selection = selected_projection(right_consumer, provider);
            assert_ne!(
                left_selection.view.record.exposure_id,
                right_selection.view.record.exposure_id
            );
            let barrier = Arc::new(tokio::sync::Barrier::new(3));
            let reserve = |selection: BindingProjection| {
                let registry = registry.clone();
                let barrier = barrier.clone();
                tokio::spawn(async move {
                    barrier.wait().await;
                    let _mutation = registry.mutation.lock().await;
                    let mut state = registry.projection.write().await;
                    let consumer = selection.view.record.consumer.clone();
                    if ensure_consumer_binding_capacity(&state, &consumer, Some(1)).is_err() {
                        return false;
                    }
                    state
                        .bindings
                        .insert(selection.view.record.binding_id.clone(), selection);
                    true
                })
            };
            let left = reserve(left_selection);
            let right = reserve(right_selection);
            barrier.wait().await;
            let (left, right) = tokio::join!(left, right);
            assert_eq!(usize::from(left?) + usize::from(right?), 1);
            assert_eq!(registry.projection.read().await.bindings.len(), 1);
        }
        Ok(())
    }

    #[test]
    fn provider_capacity_supports_larger_unbounded_and_terminal_release() -> anyhow::Result<()> {
        let consumer = endpoint(
            InstallationId::new(),
            RunId::new(),
            PortId::parse("save-import")?,
            NodeId::parse("consumer")?,
        );
        let provider = endpoint(
            InstallationId::new(),
            RunId::new(),
            PortId::parse("save-export")?,
            NodeId::parse("provider")?,
        );
        let first = selected_projection(consumer.clone(), provider.clone());
        let mut other_consumer = consumer;
        other_consumer.installation.installation_id = InstallationId::new();
        other_consumer.run.as_mut().unwrap().run_id = RunId::new();
        let second = selected_projection(other_consumer, provider.clone());
        assert_ne!(
            first.view.record.exposure_id,
            second.view.record.exposure_id
        );
        let first_id = first.view.record.binding_id.clone();
        let second_id = second.view.record.binding_id.clone();
        let capability = first.view.record.capability.clone();
        let mut state = Projection::default();
        state.bindings.insert(first_id.clone(), first);
        ensure_provider_binding_capacity(&state, &provider, &capability, Some(2))?;
        ensure_provider_binding_capacity(&state, &provider, &capability, None)?;
        assert!(ensure_provider_binding_capacity(&state, &provider, &capability, Some(1)).is_err());
        state.bindings.insert(second_id.clone(), second);
        assert!(ensure_provider_binding_capacity(&state, &provider, &capability, Some(2)).is_err());
        ensure_provider_binding_capacity(&state, &provider, &capability, None)?;

        let mut other_endpoint = provider.clone();
        let mut aliased_endpoint = provider.clone();
        aliased_endpoint.port.root_port = PortId::parse("provider-export-alias")?;
        assert!(
            ensure_provider_binding_capacity(&state, &aliased_endpoint, &capability, Some(1))
                .is_err()
        );
        other_endpoint.port.root_port = PortId::parse("other-export")?;
        other_endpoint.port.leaf_port.port_id = PortId::parse("other-export")?;
        ensure_provider_binding_capacity(&state, &other_endpoint, &capability, Some(1))?;

        state
            .bindings
            .get_mut(&first_id)
            .unwrap()
            .view
            .record
            .status = BindingDecisionStatus::Revoked;
        state
            .bindings
            .get_mut(&second_id)
            .unwrap()
            .view
            .record
            .status = BindingDecisionStatus::Expired;
        ensure_provider_binding_capacity(&state, &provider, &capability, Some(1))?;
        Ok(())
    }

    #[tokio::test]
    async fn hydrate_catches_up_expiry_once_and_restart_keeps_terminal_projection(
    ) -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime_public_store = Arc::new(InMemoryEventStore::default());
        let mut live = runtime_public_store.subscribe();
        let installation_id = InstallationId::new();
        let provider_run_id = RunId::new();
        let export_port = PortId::parse("save")?;
        let node_id = NodeId::parse("provider")?;
        let exposure_id = ExposureId::new();
        let provider = endpoint(
            installation_id.clone(),
            provider_run_id.clone(),
            export_port.clone(),
            node_id,
        );
        let exposure = ExposureProjection {
            view: ExposureView {
                record: ExposureRecord {
                    exposure_id: exposure_id.clone(),
                    installation_id,
                    run_id: Some(provider_run_id),
                    export_port: export_port.clone(),
                    audience: vec![plurora_work::ResourceSelector {
                        kind: "installation".to_string(),
                        id: InstallationId::new().to_string(),
                    }],
                    expires_at: Some(DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")?.into()),
                    status: ExposureStatus::Active,
                },
                revision: 1,
            },
            provider: provider.clone(),
            provider_descriptor: export_descriptor(export_port.clone()),
            capability: CapabilityPin {
                capability_id: "tests/powerbox/save".to_string(),
                capability_version: "1.0.0".to_string(),
            },
            idempotency_key: "create-exposure".to_string(),
            request_fingerprint: format!("sha256:{}", "f".repeat(64)),
            authority_basis: PowerboxAuthorityBasis::host(
                PowerboxAuthoritySubject::ExposureCreate {
                    installation_id: provider.installation.installation_id.clone(),
                    run_id: provider.run.as_ref().unwrap().run_id.clone(),
                    export_port: export_port.clone(),
                },
            ),
        };
        store
            .append_with_sequence(
                POWERBOX_SESSION.to_string(),
                POWERBOX_WRITER.to_string(),
                EVENT_EXPOSURE_CREATED.to_string(),
                POWERBOX_SCHEMA,
                serde_json::to_value(ExposureEvent { exposure })?,
                serde_json::json!({}),
            )
            .await?;
        let binding_id = BindingId::new();
        let consumer_installation_id = provider.installation.installation_id.clone();
        let consumer_run = provider.run.clone();
        let consumer_import_port = provider.port.root_port.clone();
        let binding = BindingProjection {
            view: BindingView {
                record: BindingSelectionRecord {
                    binding_id: binding_id.clone(),
                    candidate_digest: format!("sha256:{}", "1".repeat(64)),
                    exposure_id: exposure_id.clone(),
                    exposure_revision: 1,
                    consumer: provider.clone(),
                    provider,
                    capability: CapabilityPin {
                        capability_id: "tests/powerbox/save".to_string(),
                        capability_version: "1.0.0".to_string(),
                    },
                    transport: SelectedTransport {
                        class_id: "plurora.transport.capability/v1".to_string(),
                        properties: BTreeMap::new(),
                    },
                    phase: BindingPhase::Runtime,
                    availability: AvailabilityPolicy::Required,
                    effective_expires_at: None,
                    status: BindingDecisionStatus::Selected,
                },
                revision: 1,
                effective_status: BindingEffectiveStatus::Detached,
            },
            idempotency_key: "select-binding".to_string(),
            request_fingerprint: format!("sha256:{}", "2".repeat(64)),
            authority_basis: PowerboxAuthorityBasis::host(
                PowerboxAuthoritySubject::BindingSelect {
                    consumer_installation_id,
                    consumer_run,
                    import_port: consumer_import_port,
                    exposure_id: exposure_id.clone(),
                },
            ),
        };
        store
            .append_with_sequence(
                POWERBOX_SESSION.to_string(),
                POWERBOX_WRITER.to_string(),
                EVENT_BINDING_SELECTED.to_string(),
                POWERBOX_SCHEMA,
                serde_json::to_value(BindingEvent { binding })?,
                serde_json::json!({}),
            )
            .await?;
        store
            .append_with_sequence(
                POWERBOX_SESSION.to_string(),
                POWERBOX_WRITER.to_string(),
                PRIVATE_EXPOSURE_CLOSING.to_string(),
                POWERBOX_SCHEMA,
                serde_json::to_value(ExposureClosing {
                    exposure_id: exposure_id.clone(),
                    terminal_status: ExposureStatus::Expired,
                    reason_code: "exposure_expired".to_string(),
                    affected_binding_ids: vec![binding_id.clone()],
                    claim: None,
                })?,
                serde_json::json!({"visibility": "host_private", "credentials": "none"}),
            )
            .await?;

        let registry = PowerboxRegistry::new(
            store.clone(),
            runtime_public_store.clone(),
            Arc::new(UnavailableInstallationControl),
            Arc::new(UnavailableRunControl),
        )?;
        assert_eq!(registry.hydrate().await?, 3);
        assert_eq!(
            store
                .list_session(&POWERBOX_SESSION.to_string())
                .await?
                .len(),
            5
        );
        assert!(registry
            .sweep_expired()
            .await?
            .affected_binding_ids
            .is_empty());
        assert_eq!(
            store
                .list_session(&POWERBOX_SESSION.to_string())
                .await?
                .len(),
            5
        );

        let persisted = store.list_session(&POWERBOX_SESSION.to_string()).await?;
        assert_eq!(
            persisted
                .iter()
                .filter(|event| plurora_core::is_platform_event_kind(&event.kind))
                .count(),
            4
        );
        assert_eq!(
            persisted
                .iter()
                .filter(|event| event.kind == PRIVATE_EXPOSURE_CLOSING)
                .count(),
            1
        );
        assert!(!plurora_core::is_platform_event_kind(
            PRIVATE_EXPOSURE_CLOSING
        ));

        let public = runtime_public_store
            .list_session(&POWERBOX_SESSION.to_string())
            .await?;
        assert_eq!(public.len(), 4);
        assert_eq!(
            public
                .iter()
                .filter(|event| event.kind == EVENT_EXPOSURE_CREATED)
                .count(),
            1
        );
        assert_eq!(
            public
                .iter()
                .filter(|event| event.kind == EVENT_BINDING_SELECTED)
                .count(),
            1
        );
        assert_eq!(
            public
                .iter()
                .filter(|event| event.kind == EVENT_BINDING_EXPIRED)
                .count(),
            1
        );
        assert_eq!(
            public
                .iter()
                .filter(|event| event.kind == EVENT_EXPOSURE_EXPIRED)
                .count(),
            1
        );
        assert!(public.iter().all(|event| {
            is_public_powerbox_kind(&event.kind)
                && event.writer_package_id == PLATFORM_RUNTIME_ID
                && event.metadata["authority"] == "none"
                && event.metadata.get("source_event_id").is_some()
                && event.metadata.get("source_event_sequence").is_some()
                && event.metadata.get("source_payload_digest").is_some()
                && (event.payload.get("exposure").is_some()
                    ^ event.payload.get("binding").is_some())
                && !event.payload.to_string().contains("authority_basis")
                && !event.payload.to_string().contains("idempotency_key")
                && !event.payload.to_string().contains("request_fingerprint")
                && !event.kind.starts_with("host-private/")
        }));
        let live_events = (0..4)
            .map(|_| live.try_recv().expect("public live relay event"))
            .collect::<Vec<_>>();
        assert_eq!(
            live_events
                .iter()
                .map(|event| (&event.id, event.sequence, &event.kind, &event.payload))
                .collect::<Vec<_>>(),
            public
                .iter()
                .map(|event| (&event.id, event.sequence, &event.kind, &event.payload))
                .collect::<Vec<_>>()
        );
        assert!(live.try_recv().is_err());

        let runtime = Runtime::new(runtime_public_store.clone(), RuntimeConfig::default());
        let replay = runtime.list_events(&POWERBOX_SESSION.to_string()).await?;
        assert_eq!(
            replay.iter().map(|event| &event.id).collect::<Vec<_>>(),
            public.iter().map(|event| &event.id).collect::<Vec<_>>()
        );
        let range = runtime
            .list_events_range(&plurora_runtime::EventListRequest {
                session_id: POWERBOX_SESSION.to_string(),
                after_sequence: Some(1),
                limit: None,
                kind_prefix: None,
                writer_package_id: None,
            })
            .await?;
        assert_eq!(
            range.iter().map(|event| &event.id).collect::<Vec<_>>(),
            public[2..]
                .iter()
                .map(|event| &event.id)
                .collect::<Vec<_>>()
        );

        let restarted = PowerboxRegistry::new(
            store.clone(),
            runtime_public_store.clone(),
            Arc::new(UnavailableInstallationControl),
            Arc::new(UnavailableRunControl),
        )?;
        assert_eq!(restarted.hydrate().await?, 5);
        let exposures = restarted
            .exposure_list(ExposureListRequest::default())
            .await?;
        assert_eq!(exposures[0].record.status, ExposureStatus::Expired);
        let bindings = restarted
            .binding_list(BindingListRequest {
                query: Some(plurora_runtime::PowerboxQueryContext {
                    unrestricted: true,
                    ..plurora_runtime::PowerboxQueryContext::default()
                }),
                ..BindingListRequest::default()
            })
            .await?;
        assert_eq!(bindings[0].record.status, BindingDecisionStatus::Expired);
        assert_eq!(bindings[0].record.binding_id, binding_id);
        assert_eq!(
            runtime_public_store
                .list_session(&POWERBOX_SESSION.to_string())
                .await?
                .len(),
            4
        );

        let empty_after_restart = Arc::new(InMemoryEventStore::default());
        let recovered_public = PowerboxRegistry::new(
            store,
            empty_after_restart.clone(),
            Arc::new(UnavailableInstallationControl),
            Arc::new(UnavailableRunControl),
        )?;
        assert_eq!(recovered_public.hydrate().await?, 5);
        assert_eq!(
            empty_after_restart
                .list_session(&POWERBOX_SESSION.to_string())
                .await?
                .len(),
            4
        );
        Ok(())
    }

    #[test]
    fn public_event_kind_must_match_terminal_status() -> anyhow::Result<()> {
        let installation_id = InstallationId::new();
        let run_id = RunId::new();
        let port = PortId::parse("save")?;
        let exposure = ExposureProjection {
            view: ExposureView {
                record: ExposureRecord {
                    exposure_id: ExposureId::new(),
                    installation_id: installation_id.clone(),
                    run_id: Some(run_id.clone()),
                    export_port: port.clone(),
                    audience: vec![plurora_work::ResourceSelector {
                        kind: "installation".to_string(),
                        id: InstallationId::new().to_string(),
                    }],
                    expires_at: None,
                    status: ExposureStatus::Revoked,
                },
                revision: 1,
            },
            provider: endpoint(
                installation_id.clone(),
                run_id.clone(),
                port.clone(),
                NodeId::parse("provider")?,
            ),
            provider_descriptor: export_descriptor(port.clone()),
            capability: CapabilityPin {
                capability_id: "tests/powerbox/save".to_string(),
                capability_version: "1.0.0".to_string(),
            },
            idempotency_key: "invalid".to_string(),
            request_fingerprint: format!("sha256:{}", "3".repeat(64)),
            authority_basis: PowerboxAuthorityBasis::host(
                PowerboxAuthoritySubject::ExposureCreate {
                    installation_id,
                    run_id,
                    export_port: port,
                },
            ),
        };
        let event = EventEnvelope {
            id: "evt-invalid".to_string(),
            session_id: POWERBOX_SESSION.to_string(),
            sequence: 0,
            timestamp: Utc::now(),
            writer_package_id: POWERBOX_WRITER.to_string(),
            kind: EVENT_EXPOSURE_CREATED.to_string(),
            schema_version: POWERBOX_SCHEMA,
            payload: serde_json::to_value(ExposureEvent { exposure })?,
            metadata: serde_json::json!({}),
        };
        assert!(apply_event(&mut Projection::default(), &event).is_err());
        Ok(())
    }

    #[tokio::test]
    async fn failed_public_append_is_recovered_once_from_the_durable_source() -> anyhow::Result<()>
    {
        let store = Arc::new(InMemoryEventStore::default());
        let provider = endpoint(
            InstallationId::new(),
            RunId::new(),
            PortId::parse("save-export")?,
            NodeId::parse("provider")?,
        );
        let exposure = candidate_exposure(&provider, Vec::new());
        store
            .append_with_sequence(
                POWERBOX_SESSION.to_string(),
                POWERBOX_WRITER.to_string(),
                EVENT_EXPOSURE_CREATED.to_string(),
                POWERBOX_SCHEMA,
                serde_json::to_value(ExposureEvent { exposure })?,
                serde_json::json!({}),
            )
            .await?;
        let public_inner = Arc::new(InMemoryEventStore::default());
        let public = Arc::new(FailOnceEventStore {
            inner: public_inner.clone(),
            fail_next_compare_append: AtomicBool::new(true),
        });
        let registry = PowerboxRegistry::new(
            store.clone(),
            public,
            Arc::new(UnavailableInstallationControl),
            Arc::new(UnavailableRunControl),
        )?;

        let error = registry
            .hydrate()
            .await
            .expect_err("injected public append must make the outcome retryable");
        assert_eq!(
            plurora_runtime::ProtocolError::from_anyhow(error).code,
            "runtime/error/outcome_unknown"
        );
        assert!(public_inner.list_all().await?.is_empty());
        assert_eq!(store.list_all().await?.len(), 1);

        assert_eq!(registry.hydrate().await?, 1);
        let (left, right) = tokio::join!(
            registry.ensure_public_relay_caught_up(),
            registry.ensure_public_relay_caught_up(),
        );
        left?;
        right?;
        let source = &store.list_all().await?[0];
        let published = public_inner.list_all().await?;
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].metadata["source_event_id"], source.id);
        assert_eq!(
            published[0].metadata["source_event_sequence"],
            source.sequence
        );
        assert_eq!(published[0].payload, public_payload(source)?);
        Ok(())
    }

    #[tokio::test]
    async fn binding_select_public_relay_failure_is_typed_and_idempotent_retry_is_known(
    ) -> anyhow::Result<()> {
        let public_inner = Arc::new(InMemoryEventStore::default());
        let public = Arc::new(FailOnceEventStore {
            inner: public_inner.clone(),
            fail_next_compare_append: AtomicBool::new(true),
        });
        let (registry, consumer, provider, source) =
            candidate_fixture_with_public_store(public).await?;
        let exposure = candidate_exposure(
            &provider,
            vec![plurora_work::ResourceSelector {
                kind: "installation".to_string(),
                id: consumer.installation.installation_id.to_string(),
            }],
        );
        let exposure_id = exposure.view.record.exposure_id.clone();
        registry
            .projection
            .write()
            .await
            .exposures
            .insert(exposure_id.clone(), exposure);
        let candidate = registry
            .calculate_candidates(&candidate_request(
                &consumer,
                &consumer.installation.installation_id,
            ))
            .await?
            .candidates
            .into_iter()
            .next()
            .expect("one exact provider candidate");
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig {
                powerbox_control: registry,
                ..RuntimeConfig::default()
            },
        );
        let params = serde_json::json!({
            "consumer_installation_id": consumer.installation.installation_id,
            "expected_consumer_installation_revision": consumer.installation.installation_revision,
            "phase": "runtime",
            "consumer_run": consumer.run,
            "import_port": consumer.port.root_port,
            "exposure_id": exposure_id,
            "expected_exposure_revision": 1,
            "provider_installation_id": provider.installation.installation_id,
            "expected_provider_installation_revision": provider.installation.installation_revision,
            "candidate_digest": candidate.candidate_digest,
            "idempotency_key": "relay-outcome-unknown-binding",
        });
        let first = runtime
            .call_protocol(
                &ProtocolContext::host_dev("relay-outcome-unknown"),
                "host.binding.select",
                params.clone(),
            )
            .await
            .expect_err("the first public relay is injected to fail");
        assert_eq!(first.code, "runtime/error/outcome_unknown");
        assert_eq!(
            source
                .list_session(&POWERBOX_SESSION.to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_BINDING_SELECTED)
                .count(),
            1
        );
        assert_eq!(
            public_inner
                .list_session(&POWERBOX_SESSION.to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_BINDING_SELECTED)
                .count(),
            0
        );

        let retry = runtime
            .call_protocol(
                &ProtocolContext::host_dev("relay-outcome-unknown"),
                "host.binding.select",
                params,
            )
            .await
            .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message))?;
        assert_eq!(retry["idempotent"], true);
        assert!(retry["binding"]["record"]["binding_id"].is_string());
        assert_eq!(
            source
                .list_session(&POWERBOX_SESSION.to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_BINDING_SELECTED)
                .count(),
            1
        );
        assert_eq!(
            public_inner
                .list_session(&POWERBOX_SESSION.to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_BINDING_SELECTED)
                .count(),
            1
        );
        Ok(())
    }

    #[test]
    fn powerbox_rejects_runtime_public_store_alias() {
        let shared: Arc<dyn EventStore> = Arc::new(InMemoryEventStore::default());
        let error = PowerboxRegistry::new(
            shared.clone(),
            shared,
            Arc::new(UnavailableInstallationControl),
            Arc::new(UnavailableRunControl),
        )
        .expect_err("Powerbox must reject a Runtime public store alias");
        assert!(error.to_string().contains("physically distinct"));
    }

    #[tokio::test]
    async fn owner_loss_fails_closed_and_takeover_rehydrates_same_journal() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let first_owner =
            crate::acquire_development_host_lease(store.clone(), crate::development_registry())
                .await?;
        let first = PowerboxRegistry::new(
            store.clone(),
            Arc::new(InMemoryEventStore::default()),
            Arc::new(UnavailableInstallationControl),
            Arc::new(UnavailableRunControl),
        )?;
        first.install_owner_lease(first_owner.clone())?;
        assert_eq!(first.hydrate().await?, 0);
        crate::release_development_host_lease(store.clone(), &first_owner).await?;
        assert!(first.sweep_expired().await.is_err());

        let takeover =
            crate::acquire_development_host_lease(store.clone(), crate::development_registry())
                .await?;
        let second = PowerboxRegistry::new(
            store.clone(),
            Arc::new(InMemoryEventStore::default()),
            Arc::new(UnavailableInstallationControl),
            Arc::new(UnavailableRunControl),
        )?;
        second.install_owner_lease(takeover.clone())?;
        assert_eq!(second.hydrate().await?, 0);
        crate::release_development_host_lease(store, &takeover).await?;
        Ok(())
    }
}
