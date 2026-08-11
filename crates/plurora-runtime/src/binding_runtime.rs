use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use plurora_core::{
    CapHandle, CapHandleId, HandleLease, HandleProvenance, HandleScope, PackageId, SessionId,
    PLATFORM_RUNTIME_ID,
};
use plurora_work::{
    ActiveBindingRecord, BindingId, ExposureId, InstallationId, NodeId, PortId, RunId,
};
use serde_json::{json, Value};
use tokio::sync::{Notify, RwLock};

use crate::binding_control::{
    BindingAttachmentNotice, BindingCleanupNotice, BindingCurrentValidationRequest,
    BindingSelectionRecord, PowerboxControl,
};
use crate::runtime::{HandleTable, StreamRegistry};
use crate::{CapabilityFabric, RegisteredCapability};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingAttachFailureKind {
    DefinitiveDrift,
    Transient,
}

#[derive(Debug)]
pub struct BindingAttachError {
    kind: BindingAttachFailureKind,
    reason_code: &'static str,
    source: anyhow::Error,
}

impl BindingAttachError {
    pub fn definitive(reason_code: &'static str, source: impl Into<anyhow::Error>) -> Self {
        Self {
            kind: BindingAttachFailureKind::DefinitiveDrift,
            reason_code,
            source: source.into(),
        }
    }

    pub fn transient(reason_code: &'static str, source: impl Into<anyhow::Error>) -> Self {
        Self {
            kind: BindingAttachFailureKind::Transient,
            reason_code,
            source: source.into(),
        }
    }

    pub fn kind(&self) -> BindingAttachFailureKind {
        self.kind
    }

    pub fn reason_code(&self) -> &'static str {
        self.reason_code
    }

    pub fn is_definitive_drift(&self) -> bool {
        self.kind == BindingAttachFailureKind::DefinitiveDrift
    }

    fn from_control(reason_code: &'static str, error: anyhow::Error) -> Self {
        match error.downcast::<Self>() {
            Ok(error) => error,
            Err(error) => Self::transient(reason_code, error),
        }
    }
}

impl std::fmt::Display for BindingAttachError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.reason_code, self.source)
    }
}

impl std::error::Error for BindingAttachError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.source()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ComponentActivationIdentity {
    pub activation_id: String,
    pub installation_id: InstallationId,
    pub run_id: RunId,
    pub run_revision: u64,
    pub session_id: SessionId,
    pub package_id: PackageId,
    pub component_id: String,
    pub node_path: Vec<NodeId>,
    pub consumer_port: PortId,
}

impl ComponentActivationIdentity {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.activation_id.trim().is_empty(),
            "component activation identity is empty"
        );
        anyhow::ensure!(
            !self.package_id.trim().is_empty() && !self.component_id.trim().is_empty(),
            "component activation identity is missing package or component"
        );
        anyhow::ensure!(
            self.run_revision > 0 && !self.node_path.is_empty(),
            "component activation identity is missing Run revision or full NodePath"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ComponentActivationKey {
    installation_id: InstallationId,
    run_id: RunId,
    session_id: SessionId,
    package_id: PackageId,
    component_id: String,
    node_path: Vec<NodeId>,
}

impl ComponentActivationKey {
    fn from_identity(identity: &ComponentActivationIdentity) -> Self {
        Self {
            installation_id: identity.installation_id.clone(),
            run_id: identity.run_id.clone(),
            session_id: identity.session_id.clone(),
            package_id: identity.package_id.clone(),
            component_id: identity.component_id.clone(),
            node_path: identity.node_path.clone(),
        }
    }
}

#[derive(Clone)]
pub struct AttachedRunBinding {
    pub binding: ActiveBindingRecord,
}

impl std::fmt::Debug for AttachedRunBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AttachedRunBinding")
            .field("binding", &self.binding)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct InvocationBindingContext {
    pub handle_id: CapHandleId,
    pub activation: ComponentActivationIdentity,
}

#[derive(Debug)]
pub struct BindingInvocationPermit {
    pub binding_id: BindingId,
    pub generation: u64,
    /// Provider resolved and pinned during permit preparation. Execution must use
    /// this value and must not perform another global capability resolution.
    pub provider: RegisteredCapability,
    gate: Arc<GenerationGate>,
}

impl BindingInvocationPermit {
    pub fn stream_metadata(&self) -> Value {
        json!({
            "binding_id": self.binding_id.as_str(),
            "binding_generation": self.generation,
        })
    }

    pub(crate) fn ensure_stream_admission(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.gate
                .state
                .lock()
                .expect("binding generation gate poisoned")
                .admission_open,
            "binding authority generation is closed"
        );
        Ok(())
    }
}

impl Drop for BindingInvocationPermit {
    fn drop(&mut self) {
        let mut state = self
            .gate
            .state
            .lock()
            .expect("binding generation gate poisoned");
        debug_assert!(state.in_flight > 0);
        state.in_flight = state.in_flight.saturating_sub(1);
        drop(state);
        self.gate.settled.notify_waiters();
    }
}

#[derive(Debug, Default)]
struct GenerationGateState {
    admission_open: bool,
    in_flight: usize,
}

#[derive(Debug)]
struct GenerationGate {
    state: Mutex<GenerationGateState>,
    settled: Notify,
}

impl GenerationGate {
    fn open() -> Self {
        Self {
            state: Mutex::new(GenerationGateState {
                admission_open: true,
                in_flight: 0,
            }),
            settled: Notify::new(),
        }
    }

    async fn wait_settled(&self) {
        loop {
            let notified = self.settled.notified();
            if self
                .state
                .lock()
                .expect("binding generation gate poisoned")
                .in_flight
                == 0
            {
                return;
            }
            notified.await;
        }
    }
}

#[derive(Debug, Clone)]
struct BindingGeneration {
    generation: u64,
    open: bool,
    handle_id: CapHandleId,
    activation: ComponentActivationIdentity,
    selection: BindingSelectionRecord,
    provider: RegisteredCapability,
    gate: Arc<GenerationGate>,
}

#[derive(Default)]
struct BrokerState {
    next_generation: u64,
    activation_ids: HashMap<ComponentActivationKey, String>,
    by_handle: HashMap<CapHandleId, BindingGeneration>,
    /// Process-lifetime tombstones ensure a stale Binding token can never fall
    /// through to generic handle lookup or telemetry after generation close.
    issued_handles: HashSet<CapHandleId>,
    pending_cleanup: HashMap<BindingId, BindingCleanupNotice>,
    pending_run_cleanup: Vec<(InstallationId, RunId, SessionId)>,
}

/// Process-local Run authority sidecar. Durable Binding/Exposure decisions live
/// behind `PowerboxControl`; this broker owns only transient handle generations.
pub struct RunBindingBroker {
    control: Arc<dyn PowerboxControl>,
    handles: Arc<HandleTable>,
    capabilities: Arc<CapabilityFabric>,
    streams: Arc<StreamRegistry>,
    state: RwLock<BrokerState>,
    attach_serial: tokio::sync::Mutex<()>,
    close_serial: tokio::sync::Mutex<()>,
    cleanup_changed: Arc<Notify>,
    cleanup_retry_delay: std::time::Duration,
}

impl std::fmt::Debug for RunBindingBroker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RunBindingBroker(<transient authority>)")
    }
}

impl RunBindingBroker {
    pub fn new(
        control: Arc<dyn PowerboxControl>,
        handles: Arc<HandleTable>,
        capabilities: Arc<CapabilityFabric>,
        streams: Arc<StreamRegistry>,
        cleanup_retry_delay: std::time::Duration,
    ) -> Self {
        Self {
            control,
            handles,
            capabilities,
            streams,
            state: RwLock::new(BrokerState::default()),
            attach_serial: tokio::sync::Mutex::new(()),
            close_serial: tokio::sync::Mutex::new(()),
            cleanup_changed: Arc::new(Notify::new()),
            cleanup_retry_delay,
        }
    }

    pub fn spawn_cleanup_supervisor(self: &Arc<Self>) {
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let broker = Arc::downgrade(self);
        let cleanup_changed = self.cleanup_changed.clone();
        tokio::spawn(async move {
            loop {
                cleanup_changed.notified().await;
                let Some(current) = broker.upgrade() else {
                    return;
                };
                loop {
                    current.retry_pending_cleanup().await;
                    let pending = {
                        let state = current.state.read().await;
                        !state.pending_cleanup.is_empty() || !state.pending_run_cleanup.is_empty()
                    };
                    if !pending {
                        break;
                    }
                    tokio::time::sleep(current.cleanup_retry_delay).await;
                }
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn register_component_activation(
        &self,
        installation_id: InstallationId,
        run_id: RunId,
        session_id: SessionId,
        package_id: PackageId,
        component_id: String,
        node_path: Vec<NodeId>,
    ) -> anyhow::Result<String> {
        anyhow::ensure!(
            !session_id.trim().is_empty()
                && !package_id.trim().is_empty()
                && !component_id.trim().is_empty()
                && !node_path.is_empty(),
            "component activation registration is incomplete"
        );
        let key = ComponentActivationKey {
            installation_id,
            run_id,
            session_id,
            package_id,
            component_id,
            node_path,
        };
        let mut state = self.state.write().await;
        Ok(state
            .activation_ids
            .entry(key)
            .or_insert_with(|| plurora_core::new_id("act"))
            .clone())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn component_activation_identity(
        &self,
        installation_id: InstallationId,
        run_id: RunId,
        run_revision: u64,
        session_id: SessionId,
        package_id: PackageId,
        component_id: String,
        node_path: Vec<NodeId>,
        consumer_port: PortId,
    ) -> anyhow::Result<ComponentActivationIdentity> {
        let key = ComponentActivationKey {
            installation_id: installation_id.clone(),
            run_id: run_id.clone(),
            session_id: session_id.clone(),
            package_id: package_id.clone(),
            component_id: component_id.clone(),
            node_path: node_path.clone(),
        };
        let activation_id = self
            .state
            .read()
            .await
            .activation_ids
            .get(&key)
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "consumer component has no exact current Host-issued activation identity"
                )
            })?;
        let identity = ComponentActivationIdentity {
            activation_id,
            installation_id,
            run_id,
            run_revision,
            session_id,
            package_id,
            component_id,
            node_path,
            consumer_port,
        };
        identity.validate()?;
        Ok(identity)
    }

    pub async fn attach(
        self: &Arc<Self>,
        selection: BindingSelectionRecord,
        activation: ComponentActivationIdentity,
        provider: RegisteredCapability,
    ) -> Result<AttachedRunBinding, BindingAttachError> {
        selection
            .validate_selected()
            .map_err(|error| BindingAttachError::definitive("binding_selection_drift", error))?;
        activation
            .validate()
            .map_err(|error| BindingAttachError::definitive("consumer_activation_drift", error))?;
        ensure_activation_matches_selection(&activation, &selection)
            .map_err(|error| BindingAttachError::definitive("consumer_activation_drift", error))?;
        ensure_registered_provider_matches_pin(&provider, &selection)
            .map_err(|error| BindingAttachError::definitive("provider_pin_drift", error))?;
        let current = self
            .control
            .validate_binding_current(BindingCurrentValidationRequest {
                binding_id: selection.binding_id.clone(),
                expected_binding: selection.clone(),
                run_id: activation.run_id.clone(),
                session_id: activation.session_id.clone(),
                require_attachment: false,
            })
            .await
            .map_err(|error| {
                BindingAttachError::from_control("binding_validation_failed", error)
            })?;
        current
            .validate_selected()
            .map_err(|error| BindingAttachError::definitive("binding_current_drift", error))?;
        if current != selection {
            return Err(BindingAttachError::definitive(
                "binding_current_drift",
                anyhow::anyhow!("binding or Exposure pins changed before handle attachment"),
            ));
        }
        let _attach = self.attach_serial.lock().await;

        {
            let key = ComponentActivationKey::from_identity(&activation);
            let state = self.state.read().await;
            match state.activation_ids.get(&key) {
                Some(activation_id) if activation_id == &activation.activation_id => {}
                Some(_) => {
                    return Err(BindingAttachError::definitive(
                        "consumer_activation_drift",
                        anyhow::anyhow!(
                            "component node already has another Host-issued activation identity"
                        ),
                    ));
                }
                None => {
                    return Err(BindingAttachError::definitive(
                        "consumer_activation_drift",
                        anyhow::anyhow!(
                            "component node has no current Host-issued activation identity"
                        ),
                    ));
                }
            }
        }

        let current = self
            .control
            .validate_binding_current(BindingCurrentValidationRequest {
                binding_id: selection.binding_id.clone(),
                expected_binding: selection.clone(),
                run_id: activation.run_id.clone(),
                session_id: activation.session_id.clone(),
                require_attachment: false,
            })
            .await
            .map_err(|error| {
                BindingAttachError::from_control("binding_validation_failed", error)
            })?;
        current
            .validate_selected()
            .map_err(|error| BindingAttachError::definitive("binding_current_drift", error))?;
        if current != selection {
            return Err(BindingAttachError::definitive(
                "binding_current_drift",
                anyhow::anyhow!("binding or Exposure pins changed at generation admission"),
            ));
        }

        if let Some(existing) = {
            let state = self.state.read().await;
            state
                .by_handle
                .values()
                .find(|generation| {
                    generation.open
                        && generation.selection == selection
                        && generation.activation == activation
                        && same_registered_provider(&generation.provider, &provider)
                })
                .cloned()
        } {
            return Ok(attached_record(&existing.selection, &existing.activation));
        }

        let handle = CapHandle {
            id: CapHandleId::new(),
            cap_type: provider.descriptor.id.clone(),
            cap_version: provider.descriptor.version.clone(),
            scope: HandleScope {
                holder_package_id: activation.package_id.clone(),
                session_id: Some(activation.session_id.clone()),
            },
            constraints: json!({}),
            lease: HandleLease {
                expires_at: selection.effective_expires_at,
                max_invocations: None,
                invocations_used: 0,
            },
            provenance: HandleProvenance {
                granted_at: Utc::now(),
                granted_by_package_id: PLATFORM_RUNTIME_ID.to_string(),
                via_method: "run_binding".to_string(),
            },
            parent: None,
            revoked: false,
        };
        let handle_id = self.handles.mint_private(handle).await;
        {
            let mut state = self.state.write().await;
            state.next_generation = state.next_generation.saturating_add(1);
            let generation = state.next_generation;
            state.by_handle.insert(
                handle_id,
                BindingGeneration {
                    generation,
                    open: true,
                    handle_id,
                    activation: activation.clone(),
                    selection: selection.clone(),
                    provider: provider.clone(),
                    gate: Arc::new(GenerationGate::open()),
                },
            );
            state.issued_handles.insert(handle_id);
        }

        let notice = BindingAttachmentNotice {
            binding_id: selection.binding_id.clone(),
            run_id: activation.run_id.clone(),
            session_id: activation.session_id.clone(),
            component_activation_id: activation.activation_id.clone(),
            consumer_package_id: activation.package_id.clone(),
            consumer_node_id: activation
                .node_path
                .last()
                .expect("validated component activation has a NodePath")
                .clone(),
            consumer_port: activation.consumer_port.clone(),
        };
        if let Err(error) = self.control.binding_attached(notice).await {
            drop(_attach);
            self.close_handle_generation(handle_id, "attachment_failed")
                .await;
            return Err(BindingAttachError::from_control(
                "binding_attachment_failed",
                error,
            ));
        }

        let providers = self.capabilities.describe(&provider.descriptor.id).await;
        if !providers.iter().any(|current| {
            registered_provider_matches(current, &selection)
                && same_registered_provider(current, &provider)
        }) {
            drop(_attach);
            self.close_handle_generation(handle_id, "provider_pin_drift")
                .await;
            return Err(BindingAttachError::definitive(
                "provider_pin_drift",
                anyhow::anyhow!(
                    "binding provider exact artifact, behavior, trust, capability, or version pin drifted after generation creation"
                ),
            ));
        }

        if let Some(expires_at) = selection.effective_expires_at {
            let broker = Arc::downgrade(self);
            tokio::spawn(async move {
                let delay = (expires_at - Utc::now())
                    .to_std()
                    .unwrap_or(std::time::Duration::ZERO);
                tokio::time::sleep(delay).await;
                if let Some(broker) = broker.upgrade() {
                    broker
                        .close_handle_generation(handle_id, "binding_expired")
                        .await;
                }
            });
        }

        Ok(AttachedRunBinding {
            binding: ActiveBindingRecord {
                binding_id: selection.binding_id,
                consumer_installation_id: activation.installation_id,
                consumer_port: activation.consumer_port,
                exposure_id: selection.exposure_id,
                transport: selection.transport,
                expires_at: selection.effective_expires_at,
            },
        })
    }

    pub async fn is_binding_handle(&self, handle_id: CapHandleId) -> bool {
        self.state.read().await.issued_handles.contains(&handle_id)
    }

    #[doc(hidden)]
    pub async fn active_generation_count(&self) -> usize {
        self.state.read().await.by_handle.len()
    }

    pub async fn validate_permit(
        &self,
        handle_id: CapHandleId,
        caller: &ComponentActivationIdentity,
    ) -> anyhow::Result<BindingInvocationPermit> {
        caller.validate()?;
        let prepared = {
            let state = self.state.read().await;
            let generation = state
                .by_handle
                .get(&handle_id)
                .ok_or_else(|| anyhow::anyhow!("binding authority handle not found"))?;
            anyhow::ensure!(generation.open, "binding authority generation is closed");
            anyhow::ensure!(
                &generation.activation == caller,
                "binding authority does not match the exact component activation and consumer Port"
            );
            generation.clone()
        };

        let handle = self
            .handles
            .lookup_private(handle_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("binding authority handle not found"))?;
        anyhow::ensure!(!handle.revoked, "binding authority handle is revoked");
        if handle
            .lease
            .expires_at
            .is_some_and(|expiry| expiry <= Utc::now())
        {
            self.close_binding(&prepared.selection.binding_id, "binding_expired")
                .await;
            anyhow::bail!("binding authority handle is expired");
        }
        anyhow::ensure!(
            handle.scope.holder_package_id == caller.package_id
                && handle.scope.session_id.as_ref() == Some(&caller.session_id)
                && handle.cap_type == prepared.provider.descriptor.id
                && handle.cap_version == prepared.provider.descriptor.version,
            "binding authority handle scope or expiry is invalid"
        );

        let current = match self
            .control
            .validate_binding_current(BindingCurrentValidationRequest {
                binding_id: prepared.selection.binding_id.clone(),
                expected_binding: prepared.selection.clone(),
                run_id: caller.run_id.clone(),
                session_id: caller.session_id.clone(),
                require_attachment: true,
            })
            .await
        {
            Ok(current) => current,
            Err(error) => {
                let error = BindingAttachError::from_control("binding_validation_failed", error);
                if error.is_definitive_drift() {
                    let terminal = self
                        .control
                        .binding_drifted(&prepared.selection.binding_id, error.reason_code())
                        .await;
                    self.close_binding(&prepared.selection.binding_id, error.reason_code())
                        .await;
                    terminal?;
                } else {
                    self.close_binding(&prepared.selection.binding_id, "binding_broken")
                        .await;
                }
                return Err(error.into());
            }
        };
        if current.validate_selected().is_err() {
            let reason = match current.status {
                crate::BindingDecisionStatus::Revoked => "binding_revoked",
                crate::BindingDecisionStatus::Expired => "binding_expired",
                crate::BindingDecisionStatus::Selected => "binding_broken",
            };
            let terminal = self
                .control
                .binding_drifted(&prepared.selection.binding_id, reason)
                .await;
            self.close_binding(&prepared.selection.binding_id, reason)
                .await;
            terminal?;
            anyhow::bail!("binding is no longer current and selected");
        }
        if current != prepared.selection {
            let terminal = self
                .control
                .binding_drifted(&prepared.selection.binding_id, "binding_pin_drift")
                .await;
            self.close_binding(&prepared.selection.binding_id, "binding_pin_drift")
                .await;
            terminal?;
            anyhow::bail!("binding or Exposure pins changed after handle attachment");
        }

        let providers = self
            .capabilities
            .describe(&prepared.provider.descriptor.id)
            .await;
        if !providers.iter().any(|provider| {
            registered_provider_matches(provider, &prepared.selection)
                && same_registered_provider(provider, &prepared.provider)
        }) {
            if let Err(error) = self
                .control
                .binding_drifted(&prepared.selection.binding_id, "provider_pin_drift")
                .await
            {
                self.close_binding(&prepared.selection.binding_id, "provider_pin_drift")
                    .await;
                return Err(error.context(
                    "provider drift was detected but its durable close obligation could not complete",
                ));
            }
            anyhow::bail!(
                "binding provider exact artifact, behavior, trust, capability, or version pin drifted"
            );
        }

        {
            let state = self.state.read().await;
            let current_generation = state
                .by_handle
                .get(&handle_id)
                .ok_or_else(|| anyhow::anyhow!("binding authority generation is closed"))?;
            anyhow::ensure!(
                current_generation.open && current_generation.generation == prepared.generation,
                "binding authority generation is closed"
            );
            let mut gate = current_generation
                .gate
                .state
                .lock()
                .expect("binding generation gate poisoned");
            anyhow::ensure!(
                gate.admission_open,
                "binding authority generation is closed"
            );
            gate.in_flight = gate.in_flight.saturating_add(1);
        }
        if let Err(error) = self.handles.record_private_invocation(handle_id).await {
            let mut gate = prepared
                .gate
                .state
                .lock()
                .expect("binding generation gate poisoned");
            gate.in_flight = gate.in_flight.saturating_sub(1);
            drop(gate);
            prepared.gate.settled.notify_waiters();
            return Err(error);
        }
        Ok(BindingInvocationPermit {
            binding_id: prepared.selection.binding_id,
            generation: prepared.generation,
            provider: prepared.provider,
            gate: prepared.gate,
        })
    }

    /// Close every transient generation for one durable Binding before any new
    /// invocation permit can be issued, then cancel streams and attempt cleanup.
    pub async fn close_binding(&self, binding_id: &BindingId, reason_code: &str) {
        let _ = self.close_binding_barrier(binding_id, reason_code).await;
    }

    pub async fn close_binding_barrier(
        &self,
        binding_id: &BindingId,
        reason_code: &str,
    ) -> anyhow::Result<()> {
        let _close = self.close_serial.lock().await;
        let _generation = self.attach_serial.lock().await;
        let closed = self
            .close_matching(|generation| &generation.selection.binding_id == binding_id)
            .await;
        self.finish_close(closed, reason_code).await?;
        if let Some(notice) = self
            .state
            .read()
            .await
            .pending_cleanup
            .get(binding_id)
            .cloned()
        {
            if self.control.binding_detached(notice.clone()).await.is_ok() {
                self.state.write().await.pending_cleanup.remove(binding_id);
            }
        }
        anyhow::ensure!(
            !self
                .state
                .read()
                .await
                .by_handle
                .values()
                .any(|generation| {
                    generation.open && &generation.selection.binding_id == binding_id
                })
                && !self
                    .state
                    .read()
                    .await
                    .pending_cleanup
                    .contains_key(binding_id),
            "Binding cleanup remains pending after the close barrier"
        );
        Ok(())
    }

    pub async fn close_exposure(&self, exposure_id: &ExposureId, reason_code: &str) {
        let _close = self.close_serial.lock().await;
        let _generation = self.attach_serial.lock().await;
        let closed = self
            .close_matching(|generation| &generation.selection.exposure_id == exposure_id)
            .await;
        let _ = self.finish_close(closed, reason_code).await;
    }

    pub async fn close_provider_run(&self, run_id: &RunId, reason_code: &str) {
        let _close = self.close_serial.lock().await;
        let _generation = self.attach_serial.lock().await;
        let closed = self
            .close_matching(|generation| {
                generation
                    .selection
                    .provider
                    .run
                    .as_ref()
                    .is_some_and(|pin| &pin.run_id == run_id)
            })
            .await;
        let _ = self.finish_close(closed, reason_code).await;
    }

    pub async fn close_consumer_run(&self, run_id: &RunId, reason_code: &str) {
        let _close = self.close_serial.lock().await;
        let _generation = self.attach_serial.lock().await;
        let closed = self
            .close_matching(|generation| &generation.activation.run_id == run_id)
            .await;
        let _ = self.finish_close(closed, reason_code).await;
    }

    /// Run stop/break closes all generations in that exact Run before cleanup.
    pub async fn stop_run(
        &self,
        installation_id: &InstallationId,
        run_id: &RunId,
        session_id: &SessionId,
        reason_code: &str,
    ) -> anyhow::Result<()> {
        {
            let _close = self.close_serial.lock().await;
            let _generation = self.attach_serial.lock().await;
            let closed = self
                .close_matching(|generation| {
                    &generation.activation.installation_id == installation_id
                        && &generation.activation.run_id == run_id
                        && &generation.activation.session_id == session_id
                })
                .await;
            self.finish_close(closed, reason_code).await?;
        }
        match self
            .control
            .run_stopped(installation_id, run_id, session_id)
            .await
        {
            Ok(result) => {
                for binding_id in result.affected_binding_ids {
                    self.close_binding_barrier(&binding_id, reason_code).await?;
                }
            }
            Err(_) => {
                let mut state = self.state.write().await;
                let notice = (installation_id.clone(), run_id.clone(), session_id.clone());
                if !state.pending_run_cleanup.contains(&notice) {
                    state.pending_run_cleanup.push(notice);
                }
                drop(state);
                self.cleanup_changed.notify_one();
                anyhow::bail!("Powerbox Run cleanup remains pending");
            }
        }
        self.state.write().await.activation_ids.retain(|key, _| {
            !(key.installation_id == *installation_id
                && key.run_id == *run_id
                && key.session_id == *session_id)
        });
        Ok(())
    }

    pub async fn retry_pending_cleanup(&self) {
        let pending = self
            .state
            .read()
            .await
            .pending_cleanup
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for notice in pending {
            if self.control.binding_detached(notice.clone()).await.is_ok() {
                self.state
                    .write()
                    .await
                    .pending_cleanup
                    .remove(&notice.binding_id);
            }
        }
        let pending_runs = self.state.read().await.pending_run_cleanup.clone();
        for (installation_id, run_id, session_id) in pending_runs {
            if let Ok(result) = self
                .control
                .run_stopped(&installation_id, &run_id, &session_id)
                .await
            {
                for binding_id in result.affected_binding_ids {
                    self.close_binding(&binding_id, "run_stopped").await;
                }
                self.state
                    .write()
                    .await
                    .pending_run_cleanup
                    .retain(|candidate| {
                        candidate != &(installation_id.clone(), run_id.clone(), session_id.clone())
                    });
                self.state.write().await.activation_ids.retain(|key, _| {
                    !(key.installation_id == installation_id
                        && key.run_id == run_id
                        && key.session_id == session_id)
                });
            }
        }
    }

    pub async fn active_handle_count_for_run(&self, run_id: &RunId) -> usize {
        self.state
            .read()
            .await
            .by_handle
            .values()
            .filter(|generation| generation.open && &generation.activation.run_id == run_id)
            .count()
    }

    /// Return only bindings belonging to one unambiguous component activation.
    /// A shared process with no exact activation match receives no Run authority.
    pub(crate) async fn invocation_bindings_for_component(
        &self,
        session_id: &SessionId,
        package_id: &PackageId,
        component_id: &str,
    ) -> anyhow::Result<HashMap<PortId, InvocationBindingContext>> {
        let state = self.state.read().await;
        let activation_keys = state
            .activation_ids
            .iter()
            .filter(|(key, _)| {
                &key.session_id == session_id
                    && &key.package_id == package_id
                    && key.component_id == component_id
            })
            .collect::<Vec<_>>();
        anyhow::ensure!(
            activation_keys.len() <= 1,
            "component invocation has multiple possible Run activations"
        );
        let exact_activation = activation_keys.first().map(|(key, id)| (*key, id.as_str()));
        let matching = state
            .by_handle
            .values()
            .filter(|generation| {
                generation.open
                    && &generation.activation.session_id == session_id
                    && &generation.activation.package_id == package_id
                    && generation.activation.component_id == component_id
                    && exact_activation.is_none_or(|(key, id)| {
                        ComponentActivationKey::from_identity(&generation.activation) == *key
                            && generation.activation.activation_id == id
                    })
            })
            .collect::<Vec<_>>();
        let mut bindings = HashMap::new();
        for generation in matching {
            anyhow::ensure!(
                bindings
                    .insert(
                        generation.activation.consumer_port.clone(),
                        InvocationBindingContext {
                            handle_id: generation.handle_id,
                            activation: generation.activation.clone(),
                        },
                    )
                    .is_none(),
                "component activation has multiple selected bindings for one consumer Port"
            );
        }
        Ok(bindings)
    }

    async fn close_handle_generation(&self, handle_id: CapHandleId, reason_code: &str) {
        let _close = self.close_serial.lock().await;
        let _generation = self.attach_serial.lock().await;
        let closed = self
            .close_matching(|generation| generation.handle_id == handle_id)
            .await;
        let _ = self.finish_close(closed, reason_code).await;
    }

    async fn close_matching<F>(&self, predicate: F) -> Vec<BindingGeneration>
    where
        F: Fn(&BindingGeneration) -> bool,
    {
        let mut state = self.state.write().await;
        let ids = state
            .by_handle
            .iter()
            .filter_map(|(handle_id, generation)| predicate(generation).then_some(*handle_id))
            .collect::<Vec<_>>();
        let mut closed = Vec::with_capacity(ids.len());
        for handle_id in ids {
            if let Some(mut generation) = state.by_handle.remove(&handle_id) {
                generation.open = false;
                generation
                    .gate
                    .state
                    .lock()
                    .expect("binding generation gate poisoned")
                    .admission_open = false;
                closed.push(generation);
            }
        }
        closed
    }

    async fn finish_close(
        &self,
        closed: Vec<BindingGeneration>,
        reason_code: &str,
    ) -> anyhow::Result<()> {
        let mut cleanup_failed = false;
        for generation in closed {
            let _ = self.handles.revoke_private(generation.handle_id).await;
            self.streams
                .cancel_binding_generation(
                    generation.selection.binding_id.as_str(),
                    generation.generation,
                )
                .await;
            generation.gate.wait_settled().await;
            let _ = self.handles.remove_private(generation.handle_id).await;
            let notice = BindingCleanupNotice {
                binding_id: generation.selection.binding_id,
                run_id: generation.activation.run_id,
                session_id: generation.activation.session_id,
                reason_code: reason_code.to_string(),
            };
            if self.control.binding_detached(notice.clone()).await.is_err() {
                self.state
                    .write()
                    .await
                    .pending_cleanup
                    .insert(notice.binding_id.clone(), notice);
                self.cleanup_changed.notify_one();
                cleanup_failed = true;
            }
        }
        anyhow::ensure!(
            !cleanup_failed,
            "Binding detach acknowledgement remains pending"
        );
        Ok(())
    }
}

fn attached_record(
    selection: &BindingSelectionRecord,
    activation: &ComponentActivationIdentity,
) -> AttachedRunBinding {
    AttachedRunBinding {
        binding: ActiveBindingRecord {
            binding_id: selection.binding_id.clone(),
            consumer_installation_id: activation.installation_id.clone(),
            consumer_port: activation.consumer_port.clone(),
            exposure_id: selection.exposure_id.clone(),
            transport: selection.transport.clone(),
            expires_at: selection.effective_expires_at,
        },
    }
}

fn ensure_activation_matches_selection(
    activation: &ComponentActivationIdentity,
    selection: &BindingSelectionRecord,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        activation.installation_id == selection.consumer.installation.installation_id
            && selection.consumer.run.as_ref().is_none_or(|run| {
                run.run_id == activation.run_id
                    && run.run_revision == activation.run_revision
                    && run.context_id == activation.session_id
            })
            && activation.package_id == selection.consumer.component.package_id
            && activation.component_id == selection.consumer.component.component_id
            && activation.node_path == selection.consumer.component.node_path
            && activation.consumer_port == selection.consumer.port.root_port,
        "binding selection does not match the exact consumer activation"
    );
    Ok(())
}

fn ensure_registered_provider_matches_pin(
    provider: &RegisteredCapability,
    selection: &BindingSelectionRecord,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        registered_provider_matches(provider, selection),
        "registered provider does not match exact Binding pins"
    );
    Ok(())
}

fn registered_provider_matches(
    provider: &RegisteredCapability,
    selection: &BindingSelectionRecord,
) -> bool {
    let pin = &selection.provider.component;
    let capability = &selection.capability;
    provider.provider_package_id == pin.package_id
        && provider.provider_component_id == pin.component_id
        && provider.provider_component_digest == pin.component_artifact.digest
        && provider.provider_behavior_digest == pin.behavior_digest
        && provider.provider_trust_class == pin.trust_class
        && provider.descriptor.id == capability.capability_id
        && provider.descriptor.version == capability.capability_version
}

fn same_registered_provider(left: &RegisteredCapability, right: &RegisteredCapability) -> bool {
    left.provider_package_id == right.provider_package_id
        && left.provider_component_id == right.provider_component_id
        && left.provider_component_digest == right.provider_component_digest
        && left.provider_behavior_digest == right.provider_behavior_digest
        && left.provider_trust_class == right.provider_trust_class
        && left.descriptor.id == right.descriptor.id
        && left.descriptor.version == right.descriptor.version
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use async_trait::async_trait;
    use plurora_core::{
        ArtifactDescriptor, CapabilityDescriptor, ComponentBoundaryClaims, ComponentClaimStatus,
        ComponentTrustClass,
    };
    use plurora_work::{AvailabilityPolicy, BindingPhase, ExposureId, SelectedTransport};

    use super::*;
    use crate::{
        BindingDecisionStatus, BindingEndpointPin, CapabilityPin, ComponentPin, EventStore,
        InMemoryEventStore, InstallationRevisionPin, ResolvedPortPin, Runtime, RuntimeConfig,
    };

    struct CurrentControl {
        current: std::sync::Mutex<BindingSelectionRecord>,
        cleanup_fails: AtomicBool,
        authority_current: AtomicBool,
    }

    struct MultiCurrentControl {
        current: std::sync::Mutex<HashMap<BindingId, BindingSelectionRecord>>,
    }

    #[async_trait]
    impl PowerboxControl for MultiCurrentControl {
        async fn binding_attached(&self, _notice: BindingAttachmentNotice) -> anyhow::Result<()> {
            Ok(())
        }

        async fn validate_binding_current(
            &self,
            request: BindingCurrentValidationRequest,
        ) -> anyhow::Result<BindingSelectionRecord> {
            self.current
                .lock()
                .expect("current mutex poisoned")
                .get(&request.binding_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("binding was replaced"))
        }

        async fn binding_detached(&self, _notice: BindingCleanupNotice) -> anyhow::Result<()> {
            Ok(())
        }
    }

    struct GenerationDriftControl {
        current: std::sync::Mutex<BindingSelectionRecord>,
        capabilities: std::sync::Mutex<Option<Arc<CapabilityFabric>>>,
        provider_package_id: PackageId,
        select_calls: AtomicUsize,
        attach_calls: AtomicUsize,
        drift_calls: AtomicUsize,
    }

    #[async_trait]
    impl PowerboxControl for GenerationDriftControl {
        async fn binding_select(
            &self,
            _request: crate::BindingSelectRequest,
        ) -> anyhow::Result<crate::BindingMutationResult> {
            let call = self.select_calls.fetch_add(1, Ordering::SeqCst);
            let current = self.current.lock().expect("current mutex poisoned").clone();
            Ok(crate::BindingMutationResult {
                binding: crate::BindingView {
                    revision: if current.status == BindingDecisionStatus::Selected {
                        1
                    } else {
                        2
                    },
                    effective_status: if current.status == BindingDecisionStatus::Selected {
                        crate::BindingEffectiveStatus::Detached
                    } else {
                        crate::BindingEffectiveStatus::Broken {
                            reason_code: "provider_pin_drift".to_string(),
                        }
                    },
                    record: current,
                },
                affected_binding_ids: Vec::new(),
                idempotent: call > 0,
            })
        }

        async fn binding_attached(&self, _notice: BindingAttachmentNotice) -> anyhow::Result<()> {
            self.attach_calls.fetch_add(1, Ordering::SeqCst);
            let capabilities = self
                .capabilities
                .lock()
                .expect("capabilities mutex poisoned")
                .clone()
                .expect("Runtime capabilities installed");
            capabilities
                .unregister_package(&self.provider_package_id)
                .await;
            Ok(())
        }

        async fn validate_binding_current(
            &self,
            _request: BindingCurrentValidationRequest,
        ) -> anyhow::Result<BindingSelectionRecord> {
            Ok(self.current.lock().expect("current mutex poisoned").clone())
        }

        async fn binding_drifted(
            &self,
            binding_id: &BindingId,
            _reason_code: &str,
        ) -> anyhow::Result<crate::PowerboxInvalidationResult> {
            let mut current = self.current.lock().expect("current mutex poisoned");
            if current.binding_id == *binding_id
                && current.status == BindingDecisionStatus::Selected
            {
                current.status = BindingDecisionStatus::Revoked;
                self.drift_calls.fetch_add(1, Ordering::SeqCst);
                return Ok(crate::PowerboxInvalidationResult {
                    affected_binding_ids: vec![binding_id.clone()],
                });
            }
            Ok(crate::PowerboxInvalidationResult::default())
        }

        async fn binding_detached(&self, _notice: BindingCleanupNotice) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl PowerboxControl for CurrentControl {
        async fn binding_select(
            &self,
            _request: crate::BindingSelectRequest,
        ) -> anyhow::Result<crate::BindingMutationResult> {
            Ok(crate::BindingMutationResult {
                binding: crate::BindingView {
                    record: self.current.lock().expect("current mutex poisoned").clone(),
                    revision: 1,
                    effective_status: crate::BindingEffectiveStatus::Detached,
                },
                affected_binding_ids: Vec::new(),
                idempotent: false,
            })
        }

        async fn binding_attached(&self, _notice: BindingAttachmentNotice) -> anyhow::Result<()> {
            Ok(())
        }

        async fn validate_binding_current(
            &self,
            request: BindingCurrentValidationRequest,
        ) -> anyhow::Result<BindingSelectionRecord> {
            anyhow::ensure!(
                self.authority_current.load(Ordering::SeqCst),
                "durable grant or parent delegation is revoked"
            );
            let current = self.current.lock().expect("current mutex poisoned").clone();
            anyhow::ensure!(
                current.binding_id == request.binding_id,
                "binding was replaced"
            );
            Ok(current)
        }

        async fn binding_detached(&self, _notice: BindingCleanupNotice) -> anyhow::Result<()> {
            anyhow::ensure!(
                !self.cleanup_fails.load(Ordering::SeqCst),
                "cleanup unavailable"
            );
            Ok(())
        }

        async fn run_stopped(
            &self,
            _installation_id: &InstallationId,
            _run_id: &RunId,
            _session_id: &SessionId,
        ) -> anyhow::Result<crate::PowerboxInvalidationResult> {
            Ok(crate::PowerboxInvalidationResult::default())
        }

        async fn reconcile_authority(&self) -> anyhow::Result<crate::PowerboxInvalidationResult> {
            Ok(if self.authority_current.load(Ordering::SeqCst) {
                crate::PowerboxInvalidationResult::default()
            } else {
                crate::PowerboxInvalidationResult {
                    affected_binding_ids: vec![self
                        .current
                        .lock()
                        .expect("current mutex poisoned")
                        .binding_id
                        .clone()],
                }
            })
        }
    }

    fn artifact(byte: char) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: plurora_core::COMPONENT_DESCRIPTOR_TYPE_URI.to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", byte.to_string().repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    fn installation_pin(id: InstallationId, revision: u64) -> InstallationRevisionPin {
        InstallationRevisionPin {
            installation_id: id,
            installation_revision: revision,
            work_revision: artifact('a'),
            assembly_lock: artifact('b'),
        }
    }

    fn port(node: &str, port: &str) -> ResolvedPortPin {
        ResolvedPortPin {
            root_port: PortId::parse(port).unwrap(),
            node_path: vec![NodeId::parse(node).unwrap()],
            leaf_port: plurora_work::PortEndpoint {
                node_id: NodeId::parse(node).unwrap(),
                port_id: PortId::parse(port).unwrap(),
            },
            canonical_contract_digest: format!("sha256:{}", "c".repeat(64)),
        }
    }

    fn registered(package_id: &str, component_id: &str) -> RegisteredCapability {
        RegisteredCapability {
            descriptor: CapabilityDescriptor {
                id: "example/save/write".to_string(),
                version: "1.2.3".to_string(),
                input_schema: Value::Null,
                output_schema: Value::Null,
                streaming: true,
                side_effects: Vec::new(),
                description: None,
            },
            provider_package_id: package_id.to_string(),
            provider_component_id: component_id.to_string(),
            provider_component_digest: artifact('c').digest,
            provider_behavior_digest: format!("sha256:{}", "d".repeat(64)),
            provider_trust_class: ComponentTrustClass::IsolatedProcess,
            provider_claim_status: ComponentClaimStatus::Declared,
            provider_enforced_boundaries: ComponentBoundaryClaims::default(),
        }
    }

    async fn register_exact_provider(
        capabilities: &CapabilityFabric,
        provider: &RegisteredCapability,
    ) {
        let component = plurora_core::ComponentDescriptor {
            component_id: provider.provider_component_id.clone(),
            version: "1.0.0".to_string(),
            artifact: artifact('c'),
            behavior: ArtifactDescriptor {
                digest: provider.provider_behavior_digest.clone(),
                ..artifact('d')
            },
            trust_class: provider.provider_trust_class,
            claim_status: provider.provider_claim_status,
            entry_kind: "subprocess".to_string(),
            capability_ids: vec![provider.descriptor.id.clone()],
            enforced_boundaries: ComponentBoundaryClaims::default(),
            protocol_implementations: Vec::new(),
            content_roots: Vec::new(),
            surfaces: Vec::new(),
            annotations: BTreeMap::new(),
        };
        capabilities
            .register_component(
                &provider.provider_package_id,
                &component,
                std::slice::from_ref(&provider.descriptor),
            )
            .await
            .unwrap();
    }

    async fn register_selection_activation(
        broker: &RunBindingBroker,
        selection: &BindingSelectionRecord,
    ) -> anyhow::Result<String> {
        let run = selection
            .consumer
            .run
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Runtime selection requires a Run pin"))?;
        broker
            .register_component_activation(
                selection.consumer.installation.installation_id.clone(),
                run.run_id.clone(),
                run.context_id.clone(),
                selection.consumer.component.package_id.clone(),
                selection.consumer.component.component_id.clone(),
                selection.consumer.component.node_path.clone(),
            )
            .await
    }

    fn selection(provider_package_id: &str) -> BindingSelectionRecord {
        let consumer_installation = InstallationId::new();
        let provider_installation = InstallationId::new();
        let run_id = RunId::new();
        BindingSelectionRecord {
            binding_id: BindingId::new(),
            candidate_digest: format!("sha256:{}", "e".repeat(64)),
            exposure_id: ExposureId::new(),
            exposure_revision: 3,
            consumer: BindingEndpointPin {
                installation: installation_pin(consumer_installation, 7),
                run: Some(crate::RunRevisionPin {
                    run_id,
                    run_revision: 2,
                    context_id: "session-1".to_string(),
                }),
                port: port("consumer-node", "save-import"),
                component: ComponentPin {
                    package_id: "example/consumer".to_string(),
                    component_id: "consumer-component".to_string(),
                    node_path: vec![NodeId::parse("consumer-node").unwrap()],
                    component_artifact: artifact('e'),
                    behavior_digest: format!("sha256:{}", "f".repeat(64)),
                    trust_class: ComponentTrustClass::TrustedNative,
                },
            },
            provider: BindingEndpointPin {
                installation: installation_pin(provider_installation, 11),
                run: Some(crate::RunRevisionPin {
                    run_id: RunId::new(),
                    run_revision: 4,
                    context_id: "provider-session".to_string(),
                }),
                port: port("provider-node", "save-export"),
                component: ComponentPin {
                    package_id: provider_package_id.to_string(),
                    component_id: "provider-component".to_string(),
                    node_path: vec![NodeId::parse("provider-node").unwrap()],
                    component_artifact: artifact('c'),
                    behavior_digest: format!("sha256:{}", "d".repeat(64)),
                    trust_class: ComponentTrustClass::IsolatedProcess,
                },
            },
            capability: CapabilityPin {
                capability_id: "example/save/write".to_string(),
                capability_version: "1.2.3".to_string(),
            },
            transport: SelectedTransport {
                class_id: "plurora.transport.capability/v1".to_string(),
                properties: BTreeMap::new(),
            },
            phase: BindingPhase::Runtime,
            availability: AvailabilityPolicy::Required,
            effective_expires_at: Some(Utc::now() + chrono::Duration::minutes(5)),
            status: BindingDecisionStatus::Selected,
        }
    }

    fn activation(selection: &BindingSelectionRecord) -> ComponentActivationIdentity {
        ComponentActivationIdentity {
            activation_id: "activation-1".to_string(),
            installation_id: selection.consumer.installation.installation_id.clone(),
            run_id: selection.consumer.run.as_ref().unwrap().run_id.clone(),
            run_revision: selection.consumer.run.as_ref().unwrap().run_revision,
            session_id: "session-1".to_string(),
            package_id: selection.consumer.component.package_id.clone(),
            component_id: selection.consumer.component.component_id.clone(),
            node_path: selection.consumer.component.node_path.clone(),
            consumer_port: selection.consumer.port.root_port.clone(),
        }
    }

    async fn broker_fixture(
        provider_package_id: &str,
    ) -> (
        Arc<RunBindingBroker>,
        Arc<CurrentControl>,
        Arc<CapabilityFabric>,
        BindingSelectionRecord,
        RegisteredCapability,
    ) {
        let selection = selection(provider_package_id);
        let provider = registered(provider_package_id, "provider-component");
        let control = Arc::new(CurrentControl {
            current: std::sync::Mutex::new(selection.clone()),
            cleanup_fails: AtomicBool::new(false),
            authority_current: AtomicBool::new(true),
        });
        let capabilities = Arc::new(CapabilityFabric::default());
        register_exact_provider(capabilities.as_ref(), &provider).await;
        let broker = Arc::new(RunBindingBroker::new(
            control.clone(),
            Arc::new(HandleTable::default()),
            capabilities.clone(),
            Arc::new(StreamRegistry::default()),
            std::time::Duration::from_millis(10),
        ));
        let identity = activation(&selection);
        broker.state.write().await.activation_ids.insert(
            ComponentActivationKey::from_identity(&identity),
            identity.activation_id,
        );
        (broker, control, capabilities, selection, provider)
    }

    async fn multi_broker_fixture(
        selections: &[BindingSelectionRecord],
        provider: &RegisteredCapability,
    ) -> Arc<RunBindingBroker> {
        let control = Arc::new(MultiCurrentControl {
            current: std::sync::Mutex::new(
                selections
                    .iter()
                    .map(|selection| (selection.binding_id.clone(), selection.clone()))
                    .collect(),
            ),
        });
        let capabilities = Arc::new(CapabilityFabric::default());
        register_exact_provider(capabilities.as_ref(), provider).await;
        Arc::new(RunBindingBroker::new(
            control,
            Arc::new(HandleTable::default()),
            capabilities,
            Arc::new(StreamRegistry::default()),
            std::time::Duration::from_millis(10),
        ))
    }

    async fn wait_until_generation_is_closed(broker: &RunBindingBroker, handle_id: CapHandleId) {
        loop {
            if !broker.state.read().await.by_handle.contains_key(&handle_id) {
                return;
            }
            tokio::task::yield_now().await;
        }
    }

    async fn only_active_handle(broker: &RunBindingBroker) -> CapHandleId {
        let state = broker.state.read().await;
        assert_eq!(state.by_handle.len(), 1);
        *state.by_handle.keys().next().unwrap()
    }

    #[tokio::test]
    async fn binding_handle_is_exact_to_session_package_component_node_and_port() {
        let (broker, _control, _capabilities, selection, provider) =
            broker_fixture("example/provider").await;
        let identity = activation(&selection);
        broker
            .attach(selection, identity.clone(), provider)
            .await
            .unwrap();
        let handle_id = only_active_handle(&broker).await;
        broker.validate_permit(handle_id, &identity).await.unwrap();

        for mismatch in ["session", "package", "component", "node", "port"] {
            let mut changed = identity.clone();
            match mismatch {
                "session" => changed.session_id = "session-other".to_string(),
                "package" => changed.package_id = "example/other".to_string(),
                "component" => changed.component_id = "other-component".to_string(),
                "node" => changed.node_path = vec![NodeId::parse("other-node").unwrap()],
                "port" => changed.consumer_port = PortId::parse("other-port").unwrap(),
                _ => unreachable!(),
            }
            assert!(
                broker.validate_permit(handle_id, &changed).await.is_err(),
                "{mismatch}"
            );
        }
    }

    #[tokio::test]
    async fn two_launch_ports_on_one_node_share_activation_and_authorize_both_providers(
    ) -> anyhow::Result<()> {
        let mut first = selection("example/provider");
        let run = first.consumer.run.take().expect("fixture Run pin");
        first.phase = BindingPhase::Launch;
        let mut second = first.clone();
        second.binding_id = BindingId::new();
        second.exposure_id = ExposureId::new();
        second.consumer.port = port("consumer-node", "settings-import");
        let provider = registered("example/provider", "provider-component");
        let broker = multi_broker_fixture(&[first.clone(), second.clone()], &provider).await;

        let activation_id = broker
            .register_component_activation(
                first.consumer.installation.installation_id.clone(),
                run.run_id.clone(),
                run.context_id.clone(),
                first.consumer.component.package_id.clone(),
                first.consumer.component.component_id.clone(),
                first.consumer.component.node_path.clone(),
            )
            .await?;
        let first_identity = broker
            .component_activation_identity(
                first.consumer.installation.installation_id.clone(),
                run.run_id.clone(),
                run.run_revision,
                run.context_id.clone(),
                first.consumer.component.package_id.clone(),
                first.consumer.component.component_id.clone(),
                first.consumer.component.node_path.clone(),
                first.consumer.port.root_port.clone(),
            )
            .await?;
        let second_identity = broker
            .component_activation_identity(
                second.consumer.installation.installation_id.clone(),
                run.run_id.clone(),
                run.run_revision,
                run.context_id.clone(),
                second.consumer.component.package_id.clone(),
                second.consumer.component.component_id.clone(),
                second.consumer.component.node_path.clone(),
                second.consumer.port.root_port.clone(),
            )
            .await?;
        assert_eq!(first_identity.activation_id, activation_id);
        assert_eq!(second_identity.activation_id, activation_id);
        assert_ne!(first_identity.consumer_port, second_identity.consumer_port);

        broker
            .attach(first.clone(), first_identity, provider.clone())
            .await?;
        broker
            .attach(second.clone(), second_identity, provider.clone())
            .await?;
        let bindings = broker
            .invocation_bindings_for_component(
                &run.context_id,
                &first.consumer.component.package_id,
                &first.consumer.component.component_id,
            )
            .await?;
        assert_eq!(bindings.len(), 2);
        for port in [
            &first.consumer.port.root_port,
            &second.consumer.port.root_port,
        ] {
            let exact = bindings.get(port).expect("both Launch ports attached");
            let permit = broker
                .validate_permit(exact.handle_id, &exact.activation)
                .await?;
            assert!(same_registered_provider(&permit.provider, &provider));
        }

        broker
            .stop_run(
                &first.consumer.installation.installation_id,
                &run.run_id,
                &run.context_id,
                "test_complete",
            )
            .await?;
        assert_eq!(broker.active_generation_count().await, 0);
        Ok(())
    }

    #[tokio::test]
    async fn launch_and_runtime_ports_reuse_the_current_node_activation() -> anyhow::Result<()> {
        let mut launch = selection("example/provider");
        let run = launch.consumer.run.take().expect("fixture Run pin");
        launch.phase = BindingPhase::Launch;
        let mut runtime = launch.clone();
        runtime.binding_id = BindingId::new();
        runtime.exposure_id = ExposureId::new();
        runtime.phase = BindingPhase::Runtime;
        runtime.consumer.port = port("consumer-node", "runtime-import");
        runtime.consumer.run = Some(crate::RunRevisionPin {
            run_id: run.run_id.clone(),
            run_revision: run.run_revision,
            context_id: run.context_id.clone(),
        });
        let provider = registered("example/provider", "provider-component");
        let broker = multi_broker_fixture(&[launch.clone(), runtime.clone()], &provider).await;
        let activation_id = broker
            .register_component_activation(
                launch.consumer.installation.installation_id.clone(),
                run.run_id.clone(),
                run.context_id.clone(),
                launch.consumer.component.package_id.clone(),
                launch.consumer.component.component_id.clone(),
                launch.consumer.component.node_path.clone(),
            )
            .await?;

        let launch_identity = broker
            .component_activation_identity(
                launch.consumer.installation.installation_id.clone(),
                run.run_id.clone(),
                run.run_revision,
                run.context_id.clone(),
                launch.consumer.component.package_id.clone(),
                launch.consumer.component.component_id.clone(),
                launch.consumer.component.node_path.clone(),
                launch.consumer.port.root_port.clone(),
            )
            .await?;
        broker
            .attach(launch.clone(), launch_identity, provider.clone())
            .await?;
        let runtime_identity = broker
            .component_activation_identity(
                runtime.consumer.installation.installation_id.clone(),
                run.run_id.clone(),
                run.run_revision,
                run.context_id.clone(),
                runtime.consumer.component.package_id.clone(),
                runtime.consumer.component.component_id.clone(),
                runtime.consumer.component.node_path.clone(),
                runtime.consumer.port.root_port.clone(),
            )
            .await?;
        assert_eq!(runtime_identity.activation_id, activation_id);
        broker
            .attach(runtime.clone(), runtime_identity, provider)
            .await?;

        let bindings = broker
            .invocation_bindings_for_component(
                &run.context_id,
                &launch.consumer.component.package_id,
                &launch.consumer.component.component_id,
            )
            .await?;
        assert_eq!(bindings.len(), 2);
        assert!(bindings.contains_key(&launch.consumer.port.root_port));
        assert!(bindings.contains_key(&runtime.consumer.port.root_port));
        broker
            .stop_run(
                &launch.consumer.installation.installation_id,
                &run.run_id,
                &run.context_id,
                "test_complete",
            )
            .await?;
        assert_eq!(broker.active_generation_count().await, 0);
        Ok(())
    }

    #[tokio::test]
    async fn same_component_on_different_nodes_never_shares_ambient_activation(
    ) -> anyhow::Result<()> {
        let first = selection("example/provider");
        let run = first
            .consumer
            .run
            .as_ref()
            .expect("fixture Run pin")
            .clone();
        let mut second = first.clone();
        second.binding_id = BindingId::new();
        second.exposure_id = ExposureId::new();
        second.consumer.port = port("other-consumer-node", "other-import");
        second.consumer.component.node_path = vec![NodeId::parse("other-consumer-node").unwrap()];
        let provider = registered("example/provider", "provider-component");
        let broker = multi_broker_fixture(&[first.clone(), second.clone()], &provider).await;

        let mut identities = Vec::new();
        for selected in [&first, &second] {
            let activation_id = broker
                .register_component_activation(
                    selected.consumer.installation.installation_id.clone(),
                    run.run_id.clone(),
                    run.context_id.clone(),
                    selected.consumer.component.package_id.clone(),
                    selected.consumer.component.component_id.clone(),
                    selected.consumer.component.node_path.clone(),
                )
                .await?;
            let identity = broker
                .component_activation_identity(
                    selected.consumer.installation.installation_id.clone(),
                    run.run_id.clone(),
                    run.run_revision,
                    run.context_id.clone(),
                    selected.consumer.component.package_id.clone(),
                    selected.consumer.component.component_id.clone(),
                    selected.consumer.component.node_path.clone(),
                    selected.consumer.port.root_port.clone(),
                )
                .await?;
            assert_eq!(identity.activation_id, activation_id);
            broker
                .attach(selected.clone(), identity.clone(), provider.clone())
                .await?;
            identities.push(identity);
        }
        assert_ne!(identities[0].activation_id, identities[1].activation_id);
        assert!(broker
            .invocation_bindings_for_component(
                &run.context_id,
                &first.consumer.component.package_id,
                &first.consumer.component.component_id,
            )
            .await
            .is_err());
        broker
            .stop_run(
                &first.consumer.installation.installation_id,
                &run.run_id,
                &run.context_id,
                "test_complete",
            )
            .await?;
        assert_eq!(broker.active_generation_count().await, 0);
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_idempotent_attach_mints_one_private_generation() -> anyhow::Result<()> {
        let (broker, _control, _capabilities, selection, provider) =
            broker_fixture("example/provider").await;
        let identity = activation(&selection);
        let (left, right) = tokio::join!(
            broker.attach(selection.clone(), identity.clone(), provider.clone()),
            broker.attach(selection, identity, provider),
        );
        assert_eq!(left?.binding, right?.binding);
        assert_eq!(broker.state.read().await.by_handle.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn transient_current_validation_failure_keeps_selection_retryable() {
        let (broker, control, _capabilities, selection, provider) =
            broker_fixture("example/provider").await;
        control.authority_current.store(false, Ordering::SeqCst);
        let error = broker
            .attach(selection.clone(), activation(&selection), provider)
            .await
            .expect_err("unavailable current validation must fail attach");
        assert_eq!(error.kind(), BindingAttachFailureKind::Transient);
        assert_eq!(
            control
                .current
                .lock()
                .expect("current mutex poisoned")
                .status,
            BindingDecisionStatus::Selected
        );
        assert_eq!(broker.active_generation_count().await, 0);
    }

    #[tokio::test]
    async fn two_installation_runtime_phase_attach_immediately_authorizes_exact_provider() {
        let mut selection = selection("example/provider");
        selection.phase = BindingPhase::Runtime;
        assert_ne!(
            selection.consumer.installation.installation_id,
            selection.provider.installation.installation_id
        );
        let provider = registered("example/provider", "provider-component");
        let control = Arc::new(CurrentControl {
            current: std::sync::Mutex::new(selection.clone()),
            cleanup_fails: AtomicBool::new(false),
            authority_current: AtomicBool::new(true),
        });
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(
            store.clone(),
            RuntimeConfig {
                powerbox_control: control,
                ..RuntimeConfig::default()
            },
        );
        register_exact_provider(runtime.capabilities().as_ref(), &provider).await;
        register_selection_activation(runtime.run_binding_broker().as_ref(), &selection)
            .await
            .unwrap();

        let response = runtime
            .call_protocol(
                &crate::ProtocolContext::host_dev("binding-select-e2e"),
                "host.binding.select",
                serde_json::json!({
                    "consumer_installation_id": selection.consumer.installation.installation_id.clone(),
                    "expected_consumer_installation_revision": selection.consumer.installation.installation_revision,
                    "phase": "runtime",
                    "consumer_run": selection.consumer.run.clone(),
                    "import_port": selection.consumer.port.root_port.clone(),
                    "exposure_id": selection.exposure_id.clone(),
                    "expected_exposure_revision": selection.exposure_revision,
                    "provider_installation_id": selection.provider.installation.installation_id.clone(),
                    "expected_provider_installation_revision": selection.provider.installation.installation_revision,
                    "candidate_digest": selection.candidate_digest.clone(),
                    "idempotency_key": "binding-select-e2e",
                }),
            )
            .await
            .unwrap();
        assert_eq!(response["binding"]["effective_status"]["kind"], "active");
        let response = serde_json::to_string(&response).unwrap();
        assert!(!response.contains("handle"));
        assert!(store.list_all().await.unwrap().is_empty());

        let run = selection.consumer.run.as_ref().unwrap();
        let bindings = runtime
            .run_binding_broker()
            .invocation_bindings_for_component(
                &run.context_id,
                &selection.consumer.component.package_id,
                &selection.consumer.component.component_id,
            )
            .await
            .unwrap();
        let exact = bindings
            .get(&selection.consumer.port.root_port)
            .expect("Runtime select must attach before returning");
        let permit = runtime
            .run_binding_broker()
            .validate_permit(exact.handle_id, &exact.activation)
            .await
            .unwrap();
        assert!(same_registered_provider(&permit.provider, &provider));
    }

    #[tokio::test]
    async fn provider_pin_drift_fails_and_prepared_permit_keeps_exact_provider() {
        let (broker, _control, capabilities, selection, provider) =
            broker_fixture("example/provider").await;
        let identity = activation(&selection);
        broker
            .attach(selection, identity.clone(), provider.clone())
            .await
            .unwrap();
        let handle_id = only_active_handle(&broker).await;
        let permit = broker.validate_permit(handle_id, &identity).await.unwrap();
        assert_eq!(
            permit.provider.provider_component_digest,
            provider.provider_component_digest
        );
        assert_eq!(permit.provider.descriptor.version, "1.2.3");
        drop(permit);
        capabilities
            .unregister_package(&provider.provider_package_id)
            .await;
        assert!(broker.validate_permit(handle_id, &identity).await.is_err());
    }

    #[tokio::test]
    async fn post_generation_provider_drift_terminalizes_once_without_replay_attach(
    ) -> anyhow::Result<()> {
        for round in 0..20 {
            let mut selected = selection("example/provider");
            selected.phase = BindingPhase::Runtime;
            let provider = registered("example/provider", "provider-component");
            let control = Arc::new(GenerationDriftControl {
                current: std::sync::Mutex::new(selected.clone()),
                capabilities: std::sync::Mutex::new(None),
                provider_package_id: provider.provider_package_id.clone(),
                select_calls: AtomicUsize::new(0),
                attach_calls: AtomicUsize::new(0),
                drift_calls: AtomicUsize::new(0),
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
                .lock()
                .expect("capabilities mutex poisoned") = Some(runtime.capabilities());
            register_exact_provider(runtime.capabilities().as_ref(), &provider).await;
            register_selection_activation(runtime.run_binding_broker().as_ref(), &selected).await?;
            let params = serde_json::json!({
                "consumer_installation_id": selected.consumer.installation.installation_id,
                "expected_consumer_installation_revision": selected.consumer.installation.installation_revision,
                "phase": "runtime",
                "consumer_run": selected.consumer.run,
                "import_port": selected.consumer.port.root_port,
                "exposure_id": selected.exposure_id,
                "expected_exposure_revision": selected.exposure_revision,
                "provider_installation_id": selected.provider.installation.installation_id,
                "expected_provider_installation_revision": selected.provider.installation.installation_revision,
                "candidate_digest": selected.candidate_digest,
                "idempotency_key": format!("post-generation-drift-{round}"),
            });
            let first = runtime
                .call_protocol(
                    &crate::ProtocolContext::host_dev("post-generation-drift"),
                    "host.binding.select",
                    params.clone(),
                )
                .await
                .unwrap();
            assert_eq!(first["binding"]["record"]["status"], "revoked");
            let replay = runtime
                .call_protocol(
                    &crate::ProtocolContext::host_dev("post-generation-drift"),
                    "host.binding.select",
                    params,
                )
                .await
                .unwrap();
            assert_eq!(replay["binding"]["record"]["status"], "revoked");
            assert_eq!(control.attach_calls.load(Ordering::SeqCst), 1);
            assert_eq!(control.drift_calls.load(Ordering::SeqCst), 1);
            assert!(runtime
                .run_binding_broker()
                .invocation_bindings_for_component(
                    &selected.consumer.run.as_ref().unwrap().context_id,
                    &selected.consumer.component.package_id,
                    &selected.consumer.component.component_id,
                )
                .await?
                .is_empty());
            assert!(runtime
                .stream_registry()
                .list_invocations()
                .await
                .is_empty());
        }
        Ok(())
    }

    #[tokio::test]
    async fn revoke_cancels_binding_stream_and_run_stop_releases_handle() {
        let (broker, _control, _capabilities, selection, provider) =
            broker_fixture("example/provider").await;
        let identity = activation(&selection);
        let binding_id = selection.binding_id.clone();
        broker
            .attach(selection, identity.clone(), provider)
            .await
            .unwrap();
        let handle_id = only_active_handle(&broker).await;
        let permit = broker.validate_permit(handle_id, &identity).await.unwrap();
        let stream = broker
            .streams
            .start_binding_invocation(
                permit.provider.descriptor.id.clone(),
                permit.provider.provider_package_id.clone(),
                identity.session_id.clone(),
                permit.stream_metadata(),
                permit,
            )
            .await
            .unwrap();

        broker.close_binding(&binding_id, "binding_revoked").await;
        assert!(broker.validate_permit(handle_id, &identity).await.is_err());
        assert_eq!(
            broker
                .streams
                .get_invocation(&stream.invocation_id)
                .await
                .unwrap()
                .state,
            plurora_core::StreamInvocationState::Cancelled
        );
        assert_eq!(
            broker.active_handle_count_for_run(&identity.run_id).await,
            0
        );
    }

    #[tokio::test]
    async fn grant_or_parent_revoke_reconciliation_cancels_active_stream() {
        let (broker, control, _capabilities, selection, provider) =
            broker_fixture("example/provider").await;
        let identity = activation(&selection);
        broker
            .attach(selection, identity.clone(), provider)
            .await
            .unwrap();
        let handle_id = only_active_handle(&broker).await;
        let permit = broker.validate_permit(handle_id, &identity).await.unwrap();
        let stream = broker
            .streams
            .start_binding_invocation(
                permit.provider.descriptor.id.clone(),
                permit.provider.provider_package_id.clone(),
                identity.session_id.clone(),
                permit.stream_metadata(),
                permit,
            )
            .await
            .unwrap();

        control.authority_current.store(false, Ordering::SeqCst);
        let invalidated = control.reconcile_authority().await.unwrap();
        assert_eq!(invalidated.affected_binding_ids.len(), 1);
        for binding_id in invalidated.affected_binding_ids {
            broker
                .close_binding(&binding_id, "binding_authority_revoked")
                .await;
        }
        assert!(broker.validate_permit(handle_id, &identity).await.is_err());
        assert_eq!(
            broker
                .streams
                .get_invocation(&stream.invocation_id)
                .await
                .unwrap()
                .state,
            plurora_core::StreamInvocationState::Cancelled
        );
    }

    #[tokio::test]
    async fn concurrent_close_is_one_barrier_for_an_accepted_unary_effect_for_twenty_rounds() {
        for _ in 0..20 {
            let (broker, _control, _capabilities, selection, provider) =
                broker_fixture("example/provider").await;
            let identity = activation(&selection);
            let binding_id = selection.binding_id.clone();
            broker
                .attach(selection, identity.clone(), provider)
                .await
                .unwrap();
            let handle_id = only_active_handle(&broker).await;
            let permit = broker.validate_permit(handle_id, &identity).await.unwrap();

            let close = |binding_id: BindingId| {
                let broker = broker.clone();
                tokio::spawn(async move {
                    broker
                        .close_binding_barrier(&binding_id, "binding_revoked")
                        .await
                })
            };
            let first = close(binding_id.clone());
            let second = close(binding_id);
            wait_until_generation_is_closed(&broker, handle_id).await;
            assert!(!first.is_finished());
            assert!(!second.is_finished());
            assert!(broker.validate_permit(handle_id, &identity).await.is_err());
            drop(permit);
            first.await.unwrap().unwrap();
            second.await.unwrap().unwrap();
        }
    }

    #[tokio::test]
    async fn stream_registration_cannot_cross_a_closed_generation() {
        let (broker, _control, _capabilities, selection, provider) =
            broker_fixture("example/provider").await;
        let identity = activation(&selection);
        let binding_id = selection.binding_id.clone();
        broker
            .attach(selection, identity.clone(), provider)
            .await
            .unwrap();
        let handle_id = only_active_handle(&broker).await;
        let permit = broker.validate_permit(handle_id, &identity).await.unwrap();

        let closing = {
            let broker = broker.clone();
            tokio::spawn(async move {
                broker.close_binding(&binding_id, "binding_revoked").await;
            })
        };
        wait_until_generation_is_closed(&broker, handle_id).await;
        let stream = broker
            .streams
            .start_binding_invocation(
                permit.provider.descriptor.id.clone(),
                permit.provider.provider_package_id.clone(),
                identity.session_id.clone(),
                permit.stream_metadata(),
                permit,
            )
            .await;
        assert!(stream.is_err());
        closing.await.unwrap();
        assert!(broker.streams.list_invocations().await.is_empty());
    }

    #[tokio::test]
    async fn binding_handle_is_absent_from_generic_handle_authority() {
        let (broker, _control, _capabilities, selection, provider) =
            broker_fixture("example/provider").await;
        let identity = activation(&selection);
        let binding_id = selection.binding_id.clone();
        broker
            .attach(selection, identity.clone(), provider)
            .await
            .unwrap();
        let handle_id = only_active_handle(&broker).await;
        assert!(broker.handles.lookup(handle_id).await.is_none());
        assert!(broker
            .handles
            .list_for(&identity.package_id)
            .await
            .is_empty());
        assert!(broker.handles.revoke(handle_id).await.is_err());
        assert!(broker
            .handles
            .attenuate(handle_id, json!({}))
            .await
            .is_err());
        broker.close_binding(&binding_id, "test_complete").await;
        assert!(broker.is_binding_handle(handle_id).await);
        assert!(broker.handles.lookup(handle_id).await.is_none());
    }

    #[tokio::test]
    async fn durable_expiry_closes_generation_before_the_next_permit() {
        let (broker, control, _capabilities, selection, provider) =
            broker_fixture("example/provider").await;
        let identity = activation(&selection);
        broker
            .attach(selection, identity.clone(), provider)
            .await
            .unwrap();
        let handle_id = only_active_handle(&broker).await;
        control.current.lock().unwrap().status = BindingDecisionStatus::Expired;
        assert!(broker.validate_permit(handle_id, &identity).await.is_err());
        assert_eq!(
            broker.active_handle_count_for_run(&identity.run_id).await,
            0
        );
    }

    #[tokio::test]
    async fn effective_expiry_timer_cancels_an_active_binding_stream() {
        let (broker, control, _capabilities, mut selection, provider) =
            broker_fixture("example/provider").await;
        selection.effective_expires_at = Some(Utc::now() + chrono::Duration::milliseconds(30));
        *control.current.lock().unwrap() = selection.clone();
        let identity = activation(&selection);
        broker
            .attach(selection, identity.clone(), provider)
            .await
            .unwrap();
        let handle_id = only_active_handle(&broker).await;
        let permit = broker.validate_permit(handle_id, &identity).await.unwrap();
        let stream = broker
            .streams
            .start_binding_invocation(
                permit.provider.descriptor.id.clone(),
                permit.provider.provider_package_id.clone(),
                identity.session_id.clone(),
                permit.stream_metadata(),
                permit,
            )
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        assert_eq!(
            broker
                .streams
                .get_invocation(&stream.invocation_id)
                .await
                .unwrap()
                .state,
            plurora_core::StreamInvocationState::Cancelled
        );
    }

    #[tokio::test]
    async fn cleanup_failure_is_retained_for_idempotent_retry() {
        let (broker, control, _capabilities, selection, provider) =
            broker_fixture("example/provider").await;
        broker.spawn_cleanup_supervisor();
        let identity = activation(&selection);
        let binding_id = selection.binding_id.clone();
        broker.attach(selection, identity, provider).await.unwrap();
        control.cleanup_fails.store(true, Ordering::SeqCst);
        broker.close_binding(&binding_id, "binding_revoked").await;
        assert!(broker
            .state
            .read()
            .await
            .pending_cleanup
            .contains_key(&binding_id));
        control.cleanup_fails.store(false, Ordering::SeqCst);
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                if !broker
                    .state
                    .read()
                    .await
                    .pending_cleanup
                    .contains_key(&binding_id)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("production cleanup supervisor must retry retained cleanup");
        assert!(!broker
            .state
            .read()
            .await
            .pending_cleanup
            .contains_key(&binding_id));
    }

    #[tokio::test]
    async fn provider_publisher_prefix_never_changes_pin_validation() {
        for package_id in ["plurora/provider", "third-party/provider"] {
            let (broker, _control, _capabilities, selection, provider) =
                broker_fixture(package_id).await;
            let identity = activation(&selection);
            broker
                .attach(selection, identity.clone(), provider)
                .await
                .unwrap();
            let handle_id = only_active_handle(&broker).await;
            broker.validate_permit(handle_id, &identity).await.unwrap();
        }
    }
}
