use std::collections::BTreeSet;
use std::sync::{Arc, Weak};

use crate::{
    activate_foreign_launch,
    package::{PackageRunClaim, PackageRunLease},
    resolve_foreign_launch_binding, rights_policy_outcome, ActiveForeignLaunch,
    BindingComponentDisclosure, BindingEndpointPin, BindingInstallationDisclosure,
    BindingSelectionRecord, BindingWorkDisclosure, CapabilityPin, ComponentActivationIdentity,
    ComponentPin, EventStore, ForeignLaunchBinding, InstallationRevisionPin, OpenSessionRequest,
    PackageRecord, PackageState, PowerboxEndpointInspection, ResolvedPortPin, RightsPolicyOutcome,
    RunActivation, RunBindingPreparationRequest, RunGap, RunInstallationArtifacts,
    RunInstallationGuard, RunLifecycleDriver, RunPreparation, RunRevisionPin, RunStartRequest,
    RunStatusInspection, RunStatusRequest, Runtime,
};
use async_trait::async_trait;
use plurora_core::{
    ComponentDescriptor, ComponentTrustClass, ContractMode, PackageEntry, PackageId, SessionStatus,
    SubprocessTransport, COMPONENT_DESCRIPTOR_TYPE_URI,
};
use plurora_work::{
    canonical_digest, AssemblyLock, AssemblyNodeSource, AssemblyRevision, AvailabilityPolicy,
    BindingPhase, NodeInstanceRecord, NodeInstanceStatus, NodeLock, PortDescriptor, PortDirection,
    PortEndpoint, PortId, PortRole, RightsDeclaration, RightsOperation, RunId,
    WorkEntrypointTarget, ASSEMBLY_LOCK_TYPE_URI, FOREIGN_DEDICATED_SERVER_INTENT_URI,
    INTERACTION_CAPABILITY_STREAM, INTERACTION_CAPABILITY_UNARY,
};

pub struct AssemblyRuntimeDriver<S>
where
    S: EventStore,
{
    runtime: Weak<Runtime<S>>,
}

impl<S> AssemblyRuntimeDriver<S>
where
    S: EventStore,
{
    pub fn new(runtime: Weak<Runtime<S>>) -> Self {
        Self { runtime }
    }

    fn runtime(&self) -> anyhow::Result<Arc<Runtime<S>>> {
        self.runtime
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Run runtime is unavailable"))
    }
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    /// Resolve a root Assembly Port using the same verified closure and exact
    /// loaded-component rules as Run preflight. The Host Powerbox uses this
    /// immediately before candidate construction and every effect validation.
    pub async fn inspect_powerbox_endpoint(
        &self,
        artifacts: &RunInstallationArtifacts,
        run: Option<RunRevisionPin>,
        root_port: &PortId,
        direction: PortDirection,
    ) -> anyhow::Result<PowerboxEndpointInspection> {
        let root_lock = artifacts
            .locks
            .get(&artifacts.installation.record.assembly_lock.digest)
            .ok_or_else(|| anyhow::anyhow!("verified root AssemblyLock is unavailable"))?;
        let root_assembly = artifacts
            .assemblies
            .get(&root_lock.assembly.digest)
            .ok_or_else(|| anyhow::anyhow!("verified root AssemblyRevision is unavailable"))?;
        let packages = self.packages().list().await;
        let resolved = resolve_powerbox_leaf(
            root_assembly,
            root_lock,
            &artifacts.assemblies,
            &artifacts.locks,
            &packages,
            &[],
            root_port,
            direction,
        )?;
        let installation = InstallationRevisionPin {
            installation_id: artifacts.installation.record.installation_id.clone(),
            installation_revision: artifacts.installation.revision,
            work_revision: artifacts.installation.record.work_revision.clone(),
            assembly_lock: artifacts.installation.record.assembly_lock.clone(),
        };
        let endpoint = BindingEndpointPin {
            installation,
            run,
            port: ResolvedPortPin {
                root_port: root_port.clone(),
                node_path: resolved.node_path.clone(),
                leaf_port: PortEndpoint {
                    node_id: resolved
                        .node_path
                        .last()
                        .expect("resolved Powerbox leaf has a node")
                        .clone(),
                    port_id: resolved.descriptor.port_id.clone(),
                },
                canonical_contract_digest: canonical_digest(&resolved.descriptor.contract)?,
            },
            component: ComponentPin {
                package_id: resolved.component.claim.package_id().clone(),
                component_id: resolved.component.claim.component_id().to_string(),
                node_path: resolved.node_path,
                component_artifact: resolved.component.claim.component_artifact().clone(),
                behavior_digest: resolved.component.claim.behavior_digest().to_string(),
                trust_class: resolved.component.claim.trust_class(),
            },
        };
        let (phase, availability) = match &resolved.descriptor.role {
            PortRole::Import {
                latest_binding_phase,
                availability,
                ..
            } => (*latest_binding_phase, *availability),
            PortRole::Export { .. } => (BindingPhase::Runtime, AvailabilityPolicy::Required),
        };
        let capability = if direction == PortDirection::Export {
            let capability_id = resolved
                .descriptor
                .annotations
                .get("plurora.projection/capability_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(resolved.descriptor.contract.interface_id.as_str());
            anyhow::ensure!(
                !capability_id.trim().is_empty(),
                "resolved export Port has no exact capability identity"
            );
            let matches = self
                .capabilities()
                .describe(&capability_id.to_string())
                .await
                .into_iter()
                .filter(|provider| {
                    provider.provider_package_id == endpoint.component.package_id
                        && provider.provider_component_id == endpoint.component.component_id
                        && provider.provider_component_digest
                            == endpoint.component.component_artifact.digest
                        && provider.provider_behavior_digest == endpoint.component.behavior_digest
                        && provider.provider_trust_class == endpoint.component.trust_class
                        && provider.descriptor.version == resolved.descriptor.contract.version
                })
                .collect::<Vec<_>>();
            let [provider] = matches.as_slice() else {
                anyhow::bail!(
                    "resolved export Port does not have one exact current Runtime capability provider"
                );
            };
            Some(CapabilityPin {
                capability_id: provider.descriptor.id.clone(),
                capability_version: provider.descriptor.version.clone(),
            })
        } else {
            None
        };
        let work = BindingWorkDisclosure {
            work_id: artifacts.work.work_id.clone(),
            title: artifacts.work.title.clone(),
        };
        let installation = BindingInstallationDisclosure {
            installation_id: artifacts.installation.record.installation_id.clone(),
            installation_revision: artifacts.installation.revision,
            display_name: artifacts.installation.record.display_name.clone(),
            source: artifacts.installation.record.source.clone(),
        };
        Ok(PowerboxEndpointInspection {
            endpoint,
            descriptor: resolved.descriptor,
            work,
            installation,
            component: resolved.component.disclosure,
            phase,
            availability,
            capability,
        })
    }

    /// Attach a Runtime-phase durable selection to the exact already-running
    /// consumer component. The private capability handle remains exclusively in
    /// `RunBindingBroker`; callers receive only the handle-free binding record.
    pub async fn attach_runtime_binding(
        &self,
        selection: BindingSelectionRecord,
    ) -> Result<crate::AttachedRunBinding, crate::BindingAttachError> {
        if selection.phase != BindingPhase::Runtime {
            return Err(crate::BindingAttachError::definitive(
                "binding_phase_drift",
                anyhow::anyhow!("only Runtime-phase selections attach after Run activation"),
            ));
        }
        let run = selection.consumer.run.clone().ok_or_else(|| {
            crate::BindingAttachError::definitive(
                "consumer_activation_drift",
                anyhow::anyhow!("Runtime-phase Binding is missing its exact consumer Run"),
            )
        })?;
        let activation = self
            .run_binding_broker()
            .component_activation_identity(
                selection.consumer.installation.installation_id.clone(),
                run.run_id,
                run.run_revision,
                run.context_id,
                selection.consumer.component.package_id.clone(),
                selection.consumer.component.component_id.clone(),
                selection.consumer.component.node_path.clone(),
                selection.consumer.port.root_port.clone(),
            )
            .await
            .map_err(|error| {
                crate::BindingAttachError::definitive("consumer_activation_drift", error)
            })?;
        self.attach_selected_binding(selection, activation).await
    }

    async fn attach_selected_binding(
        &self,
        selection: BindingSelectionRecord,
        activation: ComponentActivationIdentity,
    ) -> Result<crate::AttachedRunBinding, crate::BindingAttachError> {
        let providers = self
            .capabilities()
            .describe(&selection.capability.capability_id)
            .await
            .into_iter()
            .filter(|provider| {
                provider.provider_package_id == selection.provider.component.package_id
                    && provider.provider_component_id == selection.provider.component.component_id
                    && provider.provider_component_digest
                        == selection.provider.component.component_artifact.digest
                    && provider.provider_behavior_digest
                        == selection.provider.component.behavior_digest
                    && provider.provider_trust_class == selection.provider.component.trust_class
                    && provider.descriptor.version == selection.capability.capability_version
            })
            .collect::<Vec<_>>();
        let [provider] = providers.as_slice() else {
            return Err(crate::BindingAttachError::definitive(
                "provider_pin_drift",
                anyhow::anyhow!(
                    "selected Binding provider no longer has one exact registered capability"
                ),
            ));
        };
        self.run_binding_broker()
            .attach(selection, activation, provider.clone())
            .await
    }
}

struct ResolvedPowerboxLeaf {
    node_path: Vec<plurora_work::NodeId>,
    descriptor: PortDescriptor,
    component: SelectedComponent,
}

#[allow(clippy::too_many_arguments)]
fn resolve_powerbox_leaf(
    assembly: &AssemblyRevision,
    lock: &AssemblyLock,
    assemblies: &std::collections::BTreeMap<String, AssemblyRevision>,
    locks: &std::collections::BTreeMap<String, AssemblyLock>,
    packages: &[PackageRecord],
    path_prefix: &[plurora_work::NodeId],
    root_port: &PortId,
    direction: PortDirection,
) -> anyhow::Result<ResolvedPowerboxLeaf> {
    let exposure = assembly
        .exposed_ports
        .iter()
        .find(|exposure| exposure.port_id == *root_port && exposure.direction == direction)
        .ok_or_else(|| {
            anyhow::anyhow!("root Assembly Port is unavailable or has the wrong direction")
        })?;
    let node = assembly
        .nodes
        .iter()
        .find(|node| node.node_id == exposure.target.node_id)
        .ok_or_else(|| anyhow::anyhow!("root Assembly Port targets an unknown node"))?;
    let node_lock = lock
        .nodes
        .iter()
        .find(|candidate| candidate.node_id == node.node_id)
        .ok_or_else(|| anyhow::anyhow!("verified AssemblyLock is missing the Port target node"))?;
    let mut node_path = path_prefix.to_vec();
    node_path.push(node.node_id.clone());
    match (&node.source, node_lock.artifact.artifact_type_uri.as_str()) {
        (AssemblyNodeSource::Component { component }, COMPONENT_DESCRIPTOR_TYPE_URI) => {
            anyhow::ensure!(
                component == &node_lock.artifact,
                "AssemblyLock component pin differs from the AssemblyRevision"
            );
            let descriptor = node
                .ports
                .iter()
                .find(|port| port.port_id == exposure.target.port_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("root Assembly Port leaf is unavailable"))?;
            let role_matches = matches!(
                (&descriptor.role, direction),
                (PortRole::Import { .. }, PortDirection::Import)
                    | (PortRole::Export { .. }, PortDirection::Export)
            );
            anyhow::ensure!(
                role_matches,
                "root Assembly Port leaf role disagrees with direction"
            );
            let component = exact_loaded_component(node_lock, packages)
                .map_err(|gap| anyhow::anyhow!("{}: {}", gap.reason_code, gap.next_step))?;
            Ok(ResolvedPowerboxLeaf {
                node_path,
                descriptor,
                component,
            })
        }
        (AssemblyNodeSource::Assembly { assembly: nested }, ASSEMBLY_LOCK_TYPE_URI) => {
            let nested_lock = locks
                .get(&node_lock.artifact.digest)
                .ok_or_else(|| anyhow::anyhow!("verified nested AssemblyLock is unavailable"))?;
            anyhow::ensure!(
                &nested_lock.assembly == nested,
                "nested AssemblyLock does not pin the declared AssemblyRevision"
            );
            let nested_assembly = assemblies.get(&nested.digest).ok_or_else(|| {
                anyhow::anyhow!("verified nested AssemblyRevision is unavailable")
            })?;
            resolve_powerbox_leaf(
                nested_assembly,
                nested_lock,
                assemblies,
                locks,
                packages,
                &node_path,
                &exposure.target.port_id,
                direction,
            )
        }
        _ => anyhow::bail!("verified Assembly and AssemblyLock node kinds disagree"),
    }
}

struct PreparedAssemblyRun {
    installation: RunInstallationGuard,
    package_claims: Vec<PackageRunClaim>,
    package_ids: Vec<PackageId>,
    nodes: Vec<Vec<plurora_work::NodeId>>,
    component_activations: Vec<PreparedComponentActivation>,
    selected_bindings: Vec<BindingSelectionRecord>,
    foreign: Option<PreparedForeignLaunch>,
}

struct AssemblyPreflight {
    entrypoint_id: String,
    package_claims: Vec<PackageRunClaim>,
    package_ids: Vec<PackageId>,
    nodes: Vec<Vec<plurora_work::NodeId>>,
    component_activations: Vec<PreparedComponentActivation>,
    gaps: Vec<RunGap>,
    selected_bindings: Vec<BindingSelectionRecord>,
    foreign: Option<PreparedForeignLaunch>,
}

struct ActiveAssemblyRun {
    installation_id: plurora_work::InstallationId,
    session_id: String,
    _installation: RunInstallationGuard,
    _packages: PackageRunLease,
    foreign: Option<ActiveForeignLaunch>,
}

#[derive(Debug, Clone)]
struct PreparedForeignLaunch {
    binding: ForeignLaunchBinding,
    rights: Option<RightsDeclaration>,
}

#[derive(Debug, Clone)]
struct PreparedComponentActivation {
    package_id: PackageId,
    component_id: String,
    node_path: Vec<plurora_work::NodeId>,
}

#[derive(Debug, Clone)]
struct SelectedComponent {
    claim: PackageRunClaim,
    activates_node: bool,
    disclosure: BindingComponentDisclosure,
}

#[async_trait]
impl<S> RunLifecycleDriver for AssemblyRuntimeDriver<S>
where
    S: EventStore,
{
    async fn inspect_status(
        &self,
        request: &RunStatusRequest,
    ) -> anyhow::Result<RunStatusInspection> {
        let runtime = self.runtime()?;
        let installation = runtime
            .config()
            .installation_control
            .inspect_current_ready_for_run(&request.installation_id)
            .await?;
        let artifacts = &installation;
        let installation_revision = artifacts.installation.revision;
        let work_revision = artifacts.installation.record.work_revision.clone();
        let preflight = match request.entrypoint_id.as_deref() {
            Some(entrypoint_id) => {
                let inspected = inspect_entrypoint(&runtime, artifacts, entrypoint_id).await?;
                Some(crate::RunEntrypointPreflight {
                    entrypoint_id: inspected.entrypoint_id,
                    gaps: inspected.gaps,
                })
            }
            None => None,
        };
        Ok(RunStatusInspection {
            installation_revision,
            work_revision,
            preflight,
        })
    }

    async fn prepare_start(&self, request: &RunStartRequest) -> anyhow::Result<RunPreparation> {
        let runtime = self.runtime()?;
        let installation = runtime
            .config()
            .installation_control
            .acquire_ready_for_run(
                &request.installation_id,
                request.expected_installation_revision,
            )
            .await?;
        let artifacts = installation.artifacts();
        let revision = artifacts.installation.revision;
        let preflight = inspect_entrypoint(&runtime, artifacts, &request.entrypoint_id).await?;
        if !preflight.gaps.is_empty() {
            return Ok(RunPreparation::blocked(
                revision,
                preflight.entrypoint_id,
                preflight.gaps,
            ));
        }
        Ok(RunPreparation::ready(
            revision,
            preflight.entrypoint_id,
            Box::new(PreparedAssemblyRun {
                installation,
                package_claims: preflight.package_claims,
                package_ids: preflight.package_ids,
                nodes: preflight.nodes,
                component_activations: preflight.component_activations,
                selected_bindings: preflight.selected_bindings,
                foreign: preflight.foreign,
            }),
        ))
    }

    async fn activate(
        &self,
        run_id: &RunId,
        preparation: RunPreparation,
    ) -> anyhow::Result<RunActivation> {
        let runtime = self.runtime()?;
        let prepared: PreparedAssemblyRun = preparation.take()?;
        // This is the final Package precondition before creating any Run
        // context. Acquisition and exact identity validation share the same
        // registry critical section as unload/restart exclusion.
        let package_lease = runtime
            .packages()
            .acquire_run_lease(run_id, &prepared.package_claims)
            .await?;
        let installation_id = prepared
            .installation
            .artifacts()
            .installation
            .record
            .installation_id
            .clone();
        let session = runtime
            .open_session(OpenSessionRequest {
                labels: vec!["host:run".to_string()],
                active_package_set: prepared.package_ids,
                metadata: serde_json::json!({
                    "kind": "run",
                    "run_id": run_id,
                    "installation_id": installation_id,
                }),
            })
            .await?;
        let foreign = match prepared.foreign.as_ref() {
            Some(prepared_foreign) => match activate_foreign_launch(
                runtime.as_ref(),
                &session.id,
                &prepared_foreign.binding,
                prepared_foreign.rights.as_ref(),
            )
            .await
            {
                Ok(active) => Some(active),
                Err(error) => {
                    let _ = runtime.close_session(session.id.clone()).await;
                    return Err(error);
                }
            },
            None => None,
        };
        let mut active_bindings = Vec::new();
        for component in &prepared.component_activations {
            runtime
                .run_binding_broker()
                .register_component_activation(
                    installation_id.clone(),
                    run_id.clone(),
                    session.id.clone(),
                    component.package_id.clone(),
                    component.component_id.clone(),
                    component.node_path.clone(),
                )
                .await?;
        }
        for selection in prepared.selected_bindings {
            let claim = prepared
                .package_claims
                .iter()
                .find(|claim| {
                    claim.package_id() == &selection.consumer.component.package_id
                        && claim.component_id() == selection.consumer.component.component_id
                        && claim.component_artifact()
                            == &selection.consumer.component.component_artifact
                        && claim.behavior_digest()
                            == selection.consumer.component.behavior_digest
                        && claim.trust_class() == selection.consumer.component.trust_class
                })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "selected Binding consumer does not match the verified Assembly/Lock Package claim"
                    )
                })?;
            let activation = runtime
                .run_binding_broker()
                .component_activation_identity(
                    installation_id.clone(),
                    run_id.clone(),
                    1,
                    session.id.clone(),
                    claim.package_id().clone(),
                    claim.component_id().to_string(),
                    selection.consumer.component.node_path.clone(),
                    selection.consumer.port.root_port.clone(),
                )
                .await?;
            let attached = runtime
                .attach_selected_binding(selection.clone(), activation)
                .await;
            match attached {
                Ok(attached) => active_bindings.push(attached.binding),
                Err(error) => {
                    let terminal_error = if error.is_definitive_drift() {
                        runtime
                            .config()
                            .powerbox_control
                            .binding_drifted(&selection.binding_id, error.reason_code())
                            .await
                            .err()
                    } else {
                        None
                    };
                    let _ = runtime
                        .run_binding_broker()
                        .stop_run(
                            &installation_id,
                            run_id,
                            &session.id,
                            "binding_attachment_failed",
                        )
                        .await;
                    let _ = runtime.close_session(session.id.clone()).await;
                    if let Some(terminal_error) = terminal_error {
                        return Err(anyhow::anyhow!(
                            "binding drift was detected during Run activation but its durable close obligation could not complete: {terminal_error}"
                        ));
                    }
                    return Err(error.into());
                }
            }
        }
        let node_instances = prepared
            .nodes
            .into_iter()
            .map(|node_path| NodeInstanceRecord {
                instance_id: plurora_core::new_id("rni"),
                node_id: node_path
                    .last()
                    .expect("prepared Assembly node has a full NodePath")
                    .clone(),
                node_path,
                status: NodeInstanceStatus::Running,
                realization_id: None,
            })
            .collect();
        Ok(RunActivation::new(
            Some(session.id.clone()),
            node_instances,
            active_bindings,
            Box::new(ActiveAssemblyRun {
                installation_id,
                session_id: session.id,
                _installation: prepared.installation,
                _packages: package_lease,
                foreign,
            }),
        ))
    }

    async fn stop(&self, run_id: &RunId, activation: &mut RunActivation) -> anyhow::Result<()> {
        let runtime = self.runtime()?;
        let active = activation.get_mut::<ActiveAssemblyRun>()?;
        let installation_id = active.installation_id.clone();
        let session_id = active.session_id.clone();
        if let Some(foreign) = active.foreign.as_mut() {
            foreign.stop().await?;
        }
        runtime
            .run_binding_broker()
            .stop_run(&installation_id, run_id, &session_id, "run_stopped")
            .await?;
        let context_id = activation
            .context_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Run activation has no Host context"))?;
        if runtime
            .get_session(context_id)
            .await
            .is_some_and(|session| session.status == SessionStatus::Open)
        {
            runtime.close_session(context_id.to_string()).await?;
        }
        let _active: ActiveAssemblyRun = activation.take()?;
        // Dropping `active` releases only this Run's Installation and Package
        // leases. Shared processes/capabilities remain loaded for other Runs.
        Ok(())
    }
}

async fn inspect_entrypoint<S>(
    runtime: &Runtime<S>,
    artifacts: &RunInstallationArtifacts,
    entrypoint_id: &str,
) -> anyhow::Result<AssemblyPreflight>
where
    S: EventStore,
{
    let Some(entrypoint) = artifacts
        .work
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.id == entrypoint_id)
    else {
        return Ok(AssemblyPreflight {
            entrypoint_id: entrypoint_id.to_string(),
            package_claims: Vec::new(),
            package_ids: Vec::new(),
            nodes: Vec::new(),
            component_activations: Vec::new(),
            selected_bindings: Vec::new(),
            foreign: None,
            gaps: vec![RunGap::new(
                "target_unsatisfied",
                "select an entrypoint declared by this fixed WorkRevision",
            )],
        });
    };

    let mut gaps = Vec::new();
    let rights = crate::load_rights_declaration(runtime.object_store().as_ref(), &artifacts.work)
        .await
        .map_err(|_| anyhow::anyhow!("declared Rights artifact is unavailable or invalid"))?;
    let execute_outcome = rights_policy_outcome(rights.as_ref(), RightsOperation::Execute);
    if matches!(
        execute_outcome,
        RightsPolicyOutcome::Denied | RightsPolicyOutcome::Unspecified
    ) {
        gaps.push(RunGap::new(
            if execute_outcome == RightsPolicyOutcome::Denied {
                "rights_denied"
            } else {
                "rights_unspecified"
            },
            "review the Work Rights declaration before executing this entrypoint",
        ));
    }
    if artifacts.work.operational_intent.is_some() {
        gaps.push(RunGap::new(
            "target_unsatisfied",
            "plan and apply the required Realization before starting this Run",
        ));
    }
    let mut foreign = None;
    match &entrypoint.target {
        WorkEntrypointTarget::ForeignLaunch { launch_id } => {
            let capsule =
                crate::load_foreign_capsule(runtime.object_store().as_ref(), &artifacts.work)
                    .await
                    .map_err(|_| {
                        anyhow::anyhow!("ForeignCapsule artifact is unavailable or invalid")
                    })?;
            let requirement = capsule.as_ref().and_then(|capsule| {
                capsule
                    .launch_requirements
                    .iter()
                    .find(|requirement| &requirement.launch_id == launch_id)
            });
            let Some(requirement) = requirement else {
                gaps.push(RunGap::new(
                    "artifact_missing",
                    "restore the exact ForeignCapsule launch requirement referenced by this entrypoint",
                ));
                return finish_preflight(entrypoint.id.clone(), gaps, foreign);
            };
            if entrypoint.intent_uri == FOREIGN_DEDICATED_SERVER_INTENT_URI {
                let outcome =
                    rights_policy_outcome(rights.as_ref(), RightsOperation::DedicatedServer);
                if outcome != RightsPolicyOutcome::Allowed {
                    gaps.push(RunGap::new(
                        if outcome == RightsPolicyOutcome::RequiresEntitlement {
                            "entitlement_required"
                        } else if outcome == RightsPolicyOutcome::Denied {
                            "rights_denied"
                        } else {
                            "rights_unspecified"
                        },
                        "review the dedicated-server Right before launching this entrypoint",
                    ));
                }
            }
            match resolve_foreign_launch_binding(
                runtime,
                &artifacts.installation.record.installation_id,
                &artifacts.installation.record.secret_policy,
                requirement,
            )
            .await
            {
                Ok(binding) => {
                    if execute_outcome == RightsPolicyOutcome::RequiresEntitlement
                        && binding.entitlement.is_none()
                        && !matches!(
                            binding.target,
                            crate::ForeignLaunchTarget::EntitlementAdapter { .. }
                        )
                    {
                        gaps.push(RunGap::new(
                            "entitlement_required",
                            "bind an ordinary entitlement adapter before executing this entrypoint",
                        ));
                    }
                    foreign = Some(PreparedForeignLaunch { binding, rights });
                }
                Err(error) => {
                    let message = error.to_string();
                    let reason = if message.contains("binding_invalid") {
                        "binding_incompatible"
                    } else {
                        "binding_unavailable"
                    };
                    gaps.push(RunGap::new(
                        reason,
                        format!(
                            "store the Installation-local binding at {}",
                            crate::foreign_launch_secret_ref(launch_id)
                        ),
                    ));
                }
            }
        }
        WorkEntrypointTarget::AssemblyPort { port_id } => {
            let root = artifacts
                .assemblies
                .get(&artifacts.work.assembly.digest)
                .ok_or_else(|| anyhow::anyhow!("verified root AssemblyRevision is unavailable"))?;
            if !assembly_port_resolves(root, port_id, &artifacts.assemblies) {
                gaps.push(
                    RunGap::new(
                        "target_unsatisfied",
                        "repair the exposed Assembly port in a new immutable revision",
                    )
                    .for_port(port_id),
                );
            }
        }
        WorkEntrypointTarget::Surface { .. } => {}
    }

    let packages = runtime.packages().list().await;
    let root_lock = artifacts
        .locks
        .get(&artifacts.installation.record.assembly_lock.digest)
        .ok_or_else(|| anyhow::anyhow!("verified root AssemblyLock is unavailable"))?;
    let root_assembly = artifacts
        .assemblies
        .get(&root_lock.assembly.digest)
        .ok_or_else(|| anyhow::anyhow!("verified root AssemblyRevision is unavailable"))?;
    let mut selected = Vec::new();
    let mut required_imports = Vec::new();
    let root_imports = root_assembly
        .exposed_ports
        .iter()
        .filter(|exposure| exposure.direction == plurora_work::PortDirection::Import)
        .map(|exposure| (exposure.port_id.clone(), Some(exposure.port_id.clone())))
        .collect::<std::collections::BTreeMap<_, _>>();
    collect_locked_nodes(
        root_assembly,
        root_lock,
        &artifacts.assemblies,
        &artifacts.locks,
        &packages,
        &[],
        &root_imports,
        &mut selected,
        &mut required_imports,
        &mut gaps,
    )?;

    let mut selected_bindings = Vec::new();
    if !required_imports.is_empty() {
        match runtime
            .config()
            .powerbox_control
            .prepare_run_bindings(RunBindingPreparationRequest {
                consumer_installation_id: artifacts.installation.record.installation_id.clone(),
                consumer_installation_revision: artifacts.installation.revision,
                consumer_run: None,
                required_imports: required_imports.clone(),
            })
            .await
        {
            Ok(prepared) => {
                for gap in prepared.gaps {
                    let mut run_gap = RunGap::new(gap.reason_code, gap.next_step);
                    if let Some(node_id) = gap.node_id {
                        run_gap = run_gap.for_node(node_id);
                    }
                    if let Some(port_id) = gap.port_id {
                        run_gap = run_gap.for_port(port_id);
                    }
                    gaps.push(run_gap);
                }
                for required in &required_imports {
                    let matching = prepared
                        .selected
                        .iter()
                        .filter(|binding| {
                            binding.consumer.installation.installation_id
                                == artifacts.installation.record.installation_id
                                && binding.consumer.installation.installation_revision
                                    == artifacts.installation.revision
                                && binding.consumer.port == *required
                        })
                        .collect::<Vec<_>>();
                    match matching.as_slice() {
                        [binding] if binding.validate_selected().is_ok() => {
                            selected_bindings.push((*binding).clone())
                        }
                        [] => gaps.push(
                            RunGap::new(
                                "binding_unavailable",
                                "select one explicit Exposure provider for this required import",
                            )
                            .for_node(&required.leaf_port.node_id)
                            .for_port(&required.root_port),
                        ),
                        _ => gaps.push(
                            RunGap::new(
                                "binding_ambiguous",
                                "leave exactly one selected Binding for this required import",
                            )
                            .for_node(&required.leaf_port.node_id)
                            .for_port(&required.root_port),
                        ),
                    }
                }
            }
            Err(_) => {
                for required in &required_imports {
                    gaps.push(
                        RunGap::new(
                            "binding_unavailable",
                            "Powerbox control is unavailable or no explicit Binding is selected",
                        )
                        .for_node(&required.leaf_port.node_id)
                        .for_port(&required.root_port),
                    );
                }
            }
        }
    }

    gaps.sort_by(|left, right| {
        (&left.reason_code, &left.node_id, &left.port_id).cmp(&(
            &right.reason_code,
            &right.node_id,
            &right.port_id,
        ))
    });
    gaps.dedup();
    let mut package_ids = selected
        .iter()
        .map(|(_, component)| component.claim.package_id().clone())
        .collect::<Vec<_>>();
    package_ids.sort();
    package_ids.dedup();
    let nodes = selected
        .iter()
        .filter(|(_, component)| component.activates_node)
        .map(|(node_id, _)| node_id.clone())
        .collect();
    let component_activations = selected
        .iter()
        .filter(|(_, component)| component.activates_node)
        .map(|(node_path, component)| PreparedComponentActivation {
            package_id: component.claim.package_id().clone(),
            component_id: component.claim.component_id().to_string(),
            node_path: node_path.clone(),
        })
        .collect();
    let package_claims = selected
        .into_iter()
        .map(|(_, component)| component.claim)
        .collect();
    Ok(AssemblyPreflight {
        entrypoint_id: entrypoint.id.clone(),
        package_claims,
        package_ids,
        nodes,
        component_activations,
        gaps,
        selected_bindings,
        foreign,
    })
}

fn finish_preflight(
    entrypoint_id: String,
    mut gaps: Vec<RunGap>,
    foreign: Option<PreparedForeignLaunch>,
) -> anyhow::Result<AssemblyPreflight> {
    gaps.sort_by(|left, right| {
        (&left.reason_code, &left.node_id, &left.port_id).cmp(&(
            &right.reason_code,
            &right.node_id,
            &right.port_id,
        ))
    });
    gaps.dedup();
    Ok(AssemblyPreflight {
        entrypoint_id,
        package_claims: Vec::new(),
        package_ids: Vec::new(),
        nodes: Vec::new(),
        component_activations: Vec::new(),
        gaps,
        selected_bindings: Vec::new(),
        foreign,
    })
}

fn assembly_port_resolves(
    assembly: &AssemblyRevision,
    port_id: &plurora_work::PortId,
    assemblies: &std::collections::BTreeMap<String, AssemblyRevision>,
) -> bool {
    let Some(exposure) = assembly
        .exposed_ports
        .iter()
        .find(|exposure| &exposure.port_id == port_id)
    else {
        return false;
    };
    let Some(node) = assembly
        .nodes
        .iter()
        .find(|node| node.node_id == exposure.target.node_id)
    else {
        return false;
    };
    match &node.source {
        AssemblyNodeSource::Component { .. } => node
            .ports
            .iter()
            .any(|port| port.port_id == exposure.target.port_id),
        AssemblyNodeSource::Assembly { assembly: nested } => {
            assemblies.get(&nested.digest).is_some_and(|nested| {
                assembly_port_resolves(nested, &exposure.target.port_id, assemblies)
            })
        }
    }
}

fn collect_locked_nodes(
    assembly: &AssemblyRevision,
    lock: &AssemblyLock,
    assemblies: &std::collections::BTreeMap<String, AssemblyRevision>,
    locks: &std::collections::BTreeMap<String, AssemblyLock>,
    packages: &[PackageRecord],
    path_prefix: &[plurora_work::NodeId],
    available_imports: &std::collections::BTreeMap<
        plurora_work::PortId,
        Option<plurora_work::PortId>,
    >,
    selected: &mut Vec<(Vec<plurora_work::NodeId>, SelectedComponent)>,
    required_imports: &mut Vec<ResolvedPortPin>,
    gaps: &mut Vec<RunGap>,
) -> anyhow::Result<()> {
    let locked_ids = lock
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        locked_ids.len() == assembly.nodes.len(),
        "verified AssemblyLock node set is inconsistent"
    );

    for node in &assembly.nodes {
        let mut node_path = path_prefix.to_vec();
        node_path.push(node.node_id.clone());
        let node_lock = lock
            .nodes
            .iter()
            .find(|candidate| candidate.node_id == node.node_id)
            .ok_or_else(|| anyhow::anyhow!("verified AssemblyLock is missing a node"))?;
        match (&node.source, node_lock.artifact.artifact_type_uri.as_str()) {
            (AssemblyNodeSource::Assembly { assembly: nested }, ASSEMBLY_LOCK_TYPE_URI) => {
                let nested_lock = locks.get(&node_lock.artifact.digest).ok_or_else(|| {
                    anyhow::anyhow!("verified nested AssemblyLock is unavailable")
                })?;
                anyhow::ensure!(
                    &nested_lock.assembly == nested,
                    "nested AssemblyLock does not pin the declared AssemblyRevision"
                );
                let nested_assembly = assemblies.get(&nested.digest).ok_or_else(|| {
                    anyhow::anyhow!("verified nested AssemblyRevision is unavailable")
                })?;
                let mut nested_imports = std::collections::BTreeMap::new();
                for exposure in nested_assembly
                    .exposed_ports
                    .iter()
                    .filter(|exposure| exposure.direction == plurora_work::PortDirection::Import)
                {
                    let fixed = lock.bindings.iter().any(|binding| {
                        binding.consumer.node_id == node.node_id
                            && binding.consumer.port_id == exposure.port_id
                            && matches!(
                                binding.phase,
                                BindingPhase::Authoring | BindingPhase::Installation
                            )
                    });
                    if fixed {
                        nested_imports.insert(exposure.port_id.clone(), None);
                        continue;
                    }
                    if let Some(parent_exposure) = assembly.exposed_ports.iter().find(|parent| {
                        parent.direction == plurora_work::PortDirection::Import
                            && parent.target.node_id == node.node_id
                            && parent.target.port_id == exposure.port_id
                    }) {
                        if let Some(root_port) = available_imports.get(&parent_exposure.port_id) {
                            nested_imports.insert(exposure.port_id.clone(), root_port.clone());
                        }
                    }
                }
                collect_locked_nodes(
                    nested_assembly,
                    nested_lock,
                    assemblies,
                    locks,
                    packages,
                    &node_path,
                    &nested_imports,
                    selected,
                    required_imports,
                    gaps,
                )?;
            }
            (AssemblyNodeSource::Component { component }, COMPONENT_DESCRIPTOR_TYPE_URI) => {
                anyhow::ensure!(
                    component == &node_lock.artifact,
                    "AssemblyLock component pin differs from the AssemblyRevision"
                );
                validate_required_bindings(
                    assembly,
                    node,
                    &node_path,
                    available_imports,
                    lock,
                    required_imports,
                    gaps,
                )?;
                match exact_loaded_component(node_lock, packages) {
                    Ok(component) => selected.push((node_path, component)),
                    Err(gap) => gaps.push(gap.for_node(&node.node_id)),
                }
            }
            _ => anyhow::bail!("verified Assembly and AssemblyLock node kinds disagree"),
        }
    }
    Ok(())
}

fn validate_required_bindings(
    assembly: &AssemblyRevision,
    node: &plurora_work::AssemblyNode,
    node_path: &[plurora_work::NodeId],
    available_imports: &std::collections::BTreeMap<
        plurora_work::PortId,
        Option<plurora_work::PortId>,
    >,
    lock: &AssemblyLock,
    required_imports: &mut Vec<ResolvedPortPin>,
    gaps: &mut Vec<RunGap>,
) -> anyhow::Result<()> {
    for port in &node.ports {
        let PortRole::Import {
            latest_binding_phase,
            availability: AvailabilityPolicy::Required,
            ..
        } = &port.role
        else {
            continue;
        };
        if *latest_binding_phase != BindingPhase::Launch {
            continue;
        }
        let fixed = lock.bindings.iter().any(|binding| {
            binding.consumer.node_id == node.node_id
                && binding.consumer.port_id == port.port_id
                && matches!(
                    binding.phase,
                    BindingPhase::Authoring | BindingPhase::Installation
                )
        });
        if !fixed {
            let exposed = assembly.exposed_ports.iter().find(|exposure| {
                exposure.direction == plurora_work::PortDirection::Import
                    && exposure.target.node_id == node.node_id
                    && exposure.target.port_id == port.port_id
            });
            let Some(root_port) =
                exposed.and_then(|exposure| available_imports.get(&exposure.port_id))
            else {
                gaps.push(
                    RunGap::new(
                        "target_unsatisfied",
                        "expose this deferred import through the root Assembly Port",
                    )
                    .for_node(&node.node_id)
                    .for_port(&port.port_id),
                );
                continue;
            };
            let Some(root_port) = root_port else {
                continue;
            };
            if !matches!(
                port.interaction.0.as_str(),
                INTERACTION_CAPABILITY_UNARY | INTERACTION_CAPABILITY_STREAM
            ) {
                gaps.push(
                    RunGap::new(
                        "unsupported_interaction",
                        "select or install an adapter for this required interaction model",
                    )
                    .for_node(&node.node_id)
                    .for_port(&port.port_id),
                );
                continue;
            }
            required_imports.push(ResolvedPortPin {
                root_port: root_port.clone(),
                node_path: node_path.to_vec(),
                leaf_port: PortEndpoint {
                    node_id: node.node_id.clone(),
                    port_id: port.port_id.clone(),
                },
                canonical_contract_digest: canonical_digest(&port.contract)?,
            });
        }
    }
    Ok(())
}

fn exact_loaded_component(
    node_lock: &NodeLock,
    packages: &[PackageRecord],
) -> Result<SelectedComponent, RunGap> {
    let mut matches = Vec::new();
    for package in packages
        .iter()
        .filter(|package| package.state == PackageState::Ready)
    {
        for component in &package.components {
            if component.artifact == node_lock.artifact
                && Some(component.behavior.digest.as_str()) == node_lock.behavior_digest.as_deref()
                && Some(component.trust_class) == node_lock.trust_class
                && component.entry_kind == package.entry_kind
            {
                matches.push((package, component));
            }
        }
    }
    if matches.is_empty() {
        return Err(RunGap::new(
            "artifact_missing",
            "load exactly one Package whose manifest-derived component matches the AssemblyLock",
        ));
    }
    if matches.len() > 1 {
        return Err(RunGap::new(
            "binding_ambiguous",
            "leave exactly one loaded Package matching the locked component",
        ));
    }
    let (package, component) = matches[0];
    if component.trust_class == ComponentTrustClass::ForeignCapsule
        || package.manifest.entry.contract == ContractMode::None
    {
        return Err(RunGap::new(
            "unsupported_backend",
            "use a contract-enforced local component for this Run",
        ));
    }
    match &package.manifest.entry.kind {
        PackageEntry::RustInproc { .. } => selected_component(package, component, true),
        PackageEntry::Subprocess {
            transport: SubprocessTransport::JsonRpcStdio,
            ..
        } => selected_component(package, component, true),
        PackageEntry::SurfaceBundle { .. } => selected_component(package, component, false),
        PackageEntry::Subprocess { .. }
        | PackageEntry::Wasm { .. }
        | PackageEntry::Remote { .. } => Err(RunGap::new(
            "unsupported_backend",
            "load the locked component with rust_inproc or json_rpc_stdio support",
        )),
    }
}

fn selected_component(
    package: &PackageRecord,
    component: &ComponentDescriptor,
    activates_node: bool,
) -> Result<SelectedComponent, RunGap> {
    Ok(SelectedComponent {
        claim: PackageRunClaim::exact(package, component).map_err(|_| {
            RunGap::new(
                "artifact_digest_mismatch",
                "reload the exact Package selected by the AssemblyLock",
            )
        })?,
        activates_node,
        disclosure: BindingComponentDisclosure {
            package_id: package.id.clone(),
            component_id: component.component_id.clone(),
            version: component.version.clone(),
            entry_kind: component.entry_kind.clone(),
            trust_class: component.trust_class,
            claim_status: component.claim_status,
            enforced_boundaries: component.enforced_boundaries.clone(),
            component_artifact: component.artifact.clone(),
            behavior: component.behavior.clone(),
            protocol_implementations: component.protocol_implementations.clone(),
        },
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use plurora_core::{
        ArtifactDescriptor, EntryDescriptor, PackageContributions, PackageManifest, PermissionSet,
        SandboxPolicy,
    };
    use plurora_work::{
        AcquisitionKind, AcquisitionRecord, ArtifactModel, AssemblyId, AssemblyNode,
        AssemblyPortExposure, ClaimStatus, EffectClass, ForeignCapsuleDescriptor,
        ForeignLaunchKind, ForeignLaunchRequirement, InstallationId, InstallationRecord,
        InstallationSecretPolicy, InstallationStatus, InteractionModelId, NodeId, PortContract,
        PortDescriptor, PortDirection, PortId, PortMultiplicity, RightDisposition,
        RightsDeclaration, SourceVisibility, StatePortability, TransparencyDeclaration,
        TransportRequirements, WorkEntrypoint, WorkId, WorkRevision, ASSEMBLY_REVISION_TYPE_URI,
        OPERATIONAL_INTENT_TYPE_URI, WORK_REVISION_TYPE_URI,
    };

    use super::*;
    use crate::{
        HostSecretResolver, InMemoryEventStore, InMemoryObjectStore, InstallationControl,
        InstallationView, InstallationWorkSummary, ObjectStore, RuntimeConfig,
        SecretResolverConfig, FOREIGN_LAUNCH_BINDING_SCHEMA,
    };

    struct StaticSecretResolver(String);

    #[async_trait]
    impl HostSecretResolver for StaticSecretResolver {
        async fn resolve(&self, _ref_id: &str) -> anyhow::Result<String> {
            Ok(self.0.clone())
        }
    }

    #[derive(Clone)]
    struct StaticInstallationControl {
        artifacts: RunInstallationArtifacts,
    }

    #[async_trait]
    impl InstallationControl for StaticInstallationControl {
        async fn list(
            &self,
            _request: crate::InstallationListRequest,
        ) -> anyhow::Result<Vec<InstallationView>> {
            Ok(vec![self.artifacts.installation.clone()])
        }

        async fn get(
            &self,
            installation_id: &InstallationId,
        ) -> anyhow::Result<Option<InstallationView>> {
            Ok(
                (self.artifacts.installation.record.installation_id == *installation_id)
                    .then(|| self.artifacts.installation.clone()),
            )
        }

        async fn create(
            &self,
            _request: crate::InstallationCreateRequest,
        ) -> anyhow::Result<crate::InstallationMutationResult> {
            anyhow::bail!("not used")
        }

        async fn update(
            &self,
            _request: crate::InstallationUpdateRequest,
        ) -> anyhow::Result<crate::InstallationMutationResult> {
            anyhow::bail!("not used")
        }

        async fn remove(
            &self,
            _request: crate::InstallationRemoveRequest,
        ) -> anyhow::Result<crate::InstallationMutationResult> {
            anyhow::bail!("not used")
        }

        async fn acquire_ready_for_run(
            &self,
            installation_id: &InstallationId,
            expected_revision: u64,
        ) -> anyhow::Result<RunInstallationGuard> {
            RunInstallationGuard::verified(
                installation_id,
                expected_revision,
                self.artifacts.clone(),
                Box::new(()),
            )
        }

        async fn inspect_current_ready_for_run(
            &self,
            installation_id: &InstallationId,
        ) -> anyhow::Result<RunInstallationArtifacts> {
            anyhow::ensure!(
                self.artifacts.installation.record.installation_id == *installation_id,
                "Installation not found"
            );
            Ok(self.artifacts.clone())
        }
    }

    fn manifest(id: &str, kind: PackageEntry) -> PackageManifest {
        PackageManifest {
            schema_version: 1,
            id: id.to_string(),
            version: "1.0.0".to_string(),
            display_name: None,
            description: None,
            author: None,
            license: None,
            entry: EntryDescriptor::v1(kind),
            provides: Vec::new(),
            consumes: Vec::new(),
            requires: Vec::new(),
            contributes: PackageContributions::default(),
            permissions: PermissionSet::default(),
            sandbox_policy: SandboxPolicy::default(),
        }
    }

    fn rust_record(id: &str) -> PackageRecord {
        PackageRecord::ready(manifest(
            id,
            PackageEntry::RustInproc {
                crate_ref: "test".to_string(),
                symbol: "register".to_string(),
                abi_version: 1,
            },
        ))
        .unwrap()
    }

    fn lock_for(record: &PackageRecord) -> NodeLock {
        let component = &record.components[0];
        NodeLock {
            node_id: NodeId::parse("node").unwrap(),
            artifact: component.artifact.clone(),
            behavior_digest: Some(component.behavior.digest.clone()),
            trust_class: Some(component.trust_class),
        }
    }

    fn artifact(artifact_type_uri: &str, marker: char) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: artifact_type_uri.to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", marker.to_string().repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    fn deferred_import(port: &str, phase: BindingPhase) -> PortDescriptor {
        PortDescriptor {
            port_id: PortId::parse(port).unwrap(),
            contract: PortContract {
                protocol_id: "plurora.capability".to_string(),
                interface_id: "tests/nested".to_string(),
                version: "1.0.0".to_string(),
                profiles: Vec::new(),
            },
            interaction: InteractionModelId(INTERACTION_CAPABILITY_UNARY.to_string()),
            role: PortRole::Import {
                multiplicity: PortMultiplicity {
                    min: 1,
                    max: Some(1),
                },
                latest_binding_phase: phase,
                availability: AvailabilityPolicy::Required,
                accepted_effects: vec![EffectClass::ExternalEffecting],
            },
            transport: TransportRequirements::default(),
            annotations: BTreeMap::new(),
        }
    }

    #[test]
    fn nested_deferred_imports_keep_root_ports_and_complete_node_paths() -> anyhow::Result<()> {
        let package = rust_record("tests/nested-package");
        let component = package.components[0].clone();
        let leaf_revision = artifact(ASSEMBLY_REVISION_TYPE_URI, 'l');
        let middle_revision = artifact(ASSEMBLY_REVISION_TYPE_URI, 'm');
        let root_revision = artifact(ASSEMBLY_REVISION_TYPE_URI, 'r');
        let leaf_lock_descriptor = artifact(ASSEMBLY_LOCK_TYPE_URI, 'x');
        let middle_lock_descriptor = artifact(ASSEMBLY_LOCK_TYPE_URI, 'y');
        let root_lock_descriptor = artifact(ASSEMBLY_LOCK_TYPE_URI, 'z');

        let leaf_node = NodeId::parse("leaf").unwrap();
        let same_node = NodeId::parse("same").unwrap();
        let leaf_port = PortId::parse("leaf-input").unwrap();
        let middle_port = PortId::parse("middle-input").unwrap();
        let leaf = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse("tests/nested-leaf").unwrap(),
            nodes: vec![AssemblyNode {
                node_id: leaf_node.clone(),
                source: AssemblyNodeSource::Component {
                    component: component.artifact.clone(),
                },
                ports: vec![deferred_import(leaf_port.as_str(), BindingPhase::Launch)],
                configuration: None,
                annotations: BTreeMap::new(),
            }],
            bindings: Vec::new(),
            exposed_ports: vec![AssemblyPortExposure {
                port_id: leaf_port.clone(),
                direction: PortDirection::Import,
                target: PortEndpoint {
                    node_id: leaf_node.clone(),
                    port_id: leaf_port.clone(),
                },
                annotations: BTreeMap::new(),
            }],
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let middle = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse("tests/nested-middle").unwrap(),
            nodes: vec![AssemblyNode {
                node_id: same_node.clone(),
                source: AssemblyNodeSource::Assembly {
                    assembly: leaf_revision.clone(),
                },
                ports: Vec::new(),
                configuration: None,
                annotations: BTreeMap::new(),
            }],
            bindings: Vec::new(),
            exposed_ports: vec![AssemblyPortExposure {
                port_id: middle_port.clone(),
                direction: PortDirection::Import,
                target: PortEndpoint {
                    node_id: same_node.clone(),
                    port_id: leaf_port.clone(),
                },
                annotations: BTreeMap::new(),
            }],
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let branches = ["branch-a", "branch-b"]
            .into_iter()
            .map(NodeId::parse)
            .collect::<Result<Vec<_>, _>>()?;
        let root_ports = ["root-a", "root-b"]
            .into_iter()
            .map(PortId::parse)
            .collect::<Result<Vec<_>, _>>()?;
        let root = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse("tests/nested-root").unwrap(),
            nodes: branches
                .iter()
                .cloned()
                .map(|node_id| AssemblyNode {
                    node_id,
                    source: AssemblyNodeSource::Assembly {
                        assembly: middle_revision.clone(),
                    },
                    ports: Vec::new(),
                    configuration: None,
                    annotations: BTreeMap::new(),
                })
                .collect(),
            bindings: Vec::new(),
            exposed_ports: branches
                .iter()
                .zip(&root_ports)
                .map(|(node_id, root_port)| AssemblyPortExposure {
                    port_id: root_port.clone(),
                    direction: PortDirection::Import,
                    target: PortEndpoint {
                        node_id: node_id.clone(),
                        port_id: middle_port.clone(),
                    },
                    annotations: BTreeMap::new(),
                })
                .collect(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let leaf_lock = AssemblyLock {
            schema: AssemblyLock::SCHEMA.to_string(),
            assembly: leaf_revision.clone(),
            nodes: vec![NodeLock {
                node_id: leaf_node.clone(),
                artifact: component.artifact.clone(),
                behavior_digest: Some(component.behavior.digest.clone()),
                trust_class: Some(component.trust_class),
            }],
            bindings: Vec::new(),
            protocol_profiles: Vec::new(),
            content_roots: Vec::new(),
        };
        let middle_lock = AssemblyLock {
            schema: AssemblyLock::SCHEMA.to_string(),
            assembly: middle_revision.clone(),
            nodes: vec![NodeLock {
                node_id: same_node.clone(),
                artifact: leaf_lock_descriptor.clone(),
                behavior_digest: None,
                trust_class: None,
            }],
            bindings: Vec::new(),
            protocol_profiles: Vec::new(),
            content_roots: Vec::new(),
        };
        let root_lock = AssemblyLock {
            schema: AssemblyLock::SCHEMA.to_string(),
            assembly: root_revision,
            nodes: branches
                .iter()
                .cloned()
                .map(|node_id| NodeLock {
                    node_id,
                    artifact: middle_lock_descriptor.clone(),
                    behavior_digest: None,
                    trust_class: None,
                })
                .collect(),
            bindings: Vec::new(),
            protocol_profiles: Vec::new(),
            content_roots: Vec::new(),
        };
        let mut runtime_leaf = leaf.clone();
        runtime_leaf.nodes[0].ports[0] = deferred_import(leaf_port.as_str(), BindingPhase::Runtime);
        let mut runtime_required = Vec::new();
        let mut runtime_gaps = Vec::new();
        validate_required_bindings(
            &runtime_leaf,
            &runtime_leaf.nodes[0],
            &[branches[0].clone(), same_node.clone(), leaf_node.clone()],
            &BTreeMap::from([(leaf_port.clone(), Some(root_ports[0].clone()))]),
            &leaf_lock,
            &mut runtime_required,
            &mut runtime_gaps,
        )?;
        assert!(runtime_required.is_empty());
        assert!(runtime_gaps.is_empty());
        let assemblies = BTreeMap::from([
            (leaf_revision.digest.clone(), leaf),
            (middle_revision.digest.clone(), middle),
        ]);
        let locks = BTreeMap::from([
            (leaf_lock_descriptor.digest.clone(), leaf_lock),
            (middle_lock_descriptor.digest.clone(), middle_lock),
            (root_lock_descriptor.digest, root_lock.clone()),
        ]);
        let available = root_ports
            .iter()
            .cloned()
            .map(|port| (port.clone(), Some(port)))
            .collect();
        let mut selected = Vec::new();
        let mut required = Vec::new();
        let mut gaps = Vec::new();
        collect_locked_nodes(
            &root,
            &root_lock,
            &assemblies,
            &locks,
            &[package],
            &[],
            &available,
            &mut selected,
            &mut required,
            &mut gaps,
        )?;
        assert!(gaps.is_empty(), "{gaps:?}");
        assert_eq!(required.len(), 2);
        required.sort_by(|left, right| left.root_port.cmp(&right.root_port));
        assert_eq!(required[0].root_port, root_ports[0]);
        assert_eq!(
            required[0].node_path,
            vec![branches[0].clone(), same_node.clone(), leaf_node.clone()]
        );
        assert_eq!(required[1].root_port, root_ports[1]);
        assert_eq!(
            required[1].node_path,
            vec![branches[1].clone(), same_node, leaf_node]
        );
        assert_ne!(required[0].node_path, required[1].node_path);
        Ok(())
    }

    fn preflight_artifacts(
        target: WorkEntrypointTarget,
        operational_intent: bool,
        missing_component: bool,
    ) -> RunInstallationArtifacts {
        let installation_id = InstallationId::new();
        let assembly_descriptor = artifact(ASSEMBLY_REVISION_TYPE_URI, 'a');
        let lock_descriptor = artifact(ASSEMBLY_LOCK_TYPE_URI, 'b');
        let component = artifact(COMPONENT_DESCRIPTOR_TYPE_URI, 'c');
        let node_id = NodeId::parse("node").expect("valid node id");
        let nodes = missing_component
            .then(|| AssemblyNode {
                node_id: node_id.clone(),
                source: AssemblyNodeSource::Component {
                    component: component.clone(),
                },
                ports: Vec::new(),
                configuration: None,
                annotations: BTreeMap::new(),
            })
            .into_iter()
            .collect();
        let node_locks = missing_component
            .then(|| NodeLock {
                node_id,
                artifact: component,
                behavior_digest: None,
                trust_class: None,
            })
            .into_iter()
            .collect();
        let assembly = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse("tests/run-preflight").expect("valid Assembly id"),
            nodes,
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let lock = AssemblyLock {
            schema: AssemblyLock::SCHEMA.to_string(),
            assembly: assembly_descriptor.clone(),
            nodes: node_locks,
            bindings: Vec::new(),
            protocol_profiles: Vec::new(),
            content_roots: Vec::new(),
        };
        let work = WorkRevision {
            schema: WorkRevision::SCHEMA.to_string(),
            work_id: WorkId::parse("tests/run-preflight").expect("valid Work id"),
            title: "Run preflight".to_string(),
            description: String::new(),
            assembly: assembly_descriptor.clone(),
            content_roots: Vec::new(),
            entrypoints: vec![WorkEntrypoint {
                id: "default".to_string(),
                intent_uri: "urn:plurora:test:play".to_string(),
                target,
                annotations: BTreeMap::new(),
            }],
            rights: None,
            transparency: None,
            operational_intent: operational_intent
                .then(|| artifact(OPERATIONAL_INTENT_TYPE_URI, 'd')),
            annotations: BTreeMap::new(),
        };
        let now = chrono::Utc::now();
        let installation = InstallationView {
            work_summary: InstallationWorkSummary::from_work_revision(&work),
            record: InstallationRecord {
                schema_version: InstallationRecord::SCHEMA_VERSION,
                installation_id,
                work_revision: artifact(WORK_REVISION_TYPE_URI, 'e'),
                assembly_lock: lock_descriptor.clone(),
                display_name: "Run preflight".to_string(),
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
            revision: 4,
            rollback: None,
        };
        RunInstallationArtifacts {
            installation,
            work,
            assemblies: BTreeMap::from([(assembly_descriptor.digest.clone(), assembly)]),
            locks: BTreeMap::from([(lock_descriptor.digest.clone(), lock)]),
        }
    }

    fn runnable_artifacts(record: &PackageRecord) -> RunInstallationArtifacts {
        let mut artifacts = preflight_artifacts(
            WorkEntrypointTarget::Surface {
                surface_id: "tests/surface".to_string(),
            },
            false,
            false,
        );
        let component = record.components[0].clone();
        let node_id = NodeId::parse("node").expect("valid node id");
        let assembly = artifacts
            .assemblies
            .values_mut()
            .next()
            .expect("root Assembly");
        assembly.nodes = vec![AssemblyNode {
            node_id: node_id.clone(),
            source: AssemblyNodeSource::Component {
                component: component.artifact.clone(),
            },
            ports: Vec::new(),
            configuration: None,
            annotations: BTreeMap::new(),
        }];
        let lock = artifacts.locks.values_mut().next().expect("root lock");
        lock.nodes = vec![NodeLock {
            node_id,
            artifact: component.artifact,
            behavior_digest: Some(component.behavior.digest),
            trust_class: Some(component.trust_class),
        }];
        artifacts
    }

    fn rights(execute: RightDisposition, dedicated_server: RightDisposition) -> RightsDeclaration {
        RightsDeclaration {
            license_expression: Some("LicenseRef-foreign-test".to_string()),
            terms_uri: None,
            install: RightDisposition::Allowed,
            execute,
            backup: RightDisposition::Allowed,
            export_state: RightDisposition::Denied,
            copy_across_hosts: RightDisposition::Denied,
            redistribute_artifacts: RightDisposition::Denied,
            modify: RightDisposition::Denied,
            derive: RightDisposition::Denied,
            modding: RightDisposition::Denied,
            dedicated_server,
            entitlement_requirements: Vec::new(),
            evidence_refs: Vec::new(),
        }
    }

    fn transparency() -> TransparencyDeclaration {
        TransparencyDeclaration {
            source_visibility: SourceVisibility::Closed,
            source_refs: Vec::new(),
            reproducible_build_claim: ClaimStatus::Unknown,
            sbom_refs: Vec::new(),
            provenance_refs: Vec::new(),
            signature_refs: Vec::new(),
            telemetry_disclosures: vec!["network access may be vendor-defined".to_string()],
            state_portability: StatePortability::OpaqueExportable,
            evidence_refs: Vec::new(),
        }
    }

    async fn put_model<T: ArtifactModel>(
        store: &Arc<InMemoryObjectStore>,
        model: &T,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let descriptor = model.artifact_descriptor()?;
        let bytes = model.canonical_bytes()?;
        store.put(bytes.into()).await?;
        Ok(descriptor)
    }

    async fn foreign_driver(
        execute: RightDisposition,
        dedicated_server: RightDisposition,
        dedicated: bool,
        entitlement: Option<crate::ForeignEntitlementAdapter>,
    ) -> anyhow::Result<(
        Arc<Runtime<InMemoryEventStore>>,
        AssemblyRuntimeDriver<InMemoryEventStore>,
        RunStartRequest,
    )> {
        let objects = Arc::new(InMemoryObjectStore::new());
        let rights = rights(execute, dedicated_server);
        let rights_descriptor = put_model(&objects, &rights).await?;
        let transparency = transparency();
        let transparency_descriptor = put_model(&objects, &transparency).await?;
        let requirement = ForeignLaunchRequirement {
            launch_id: "foreign".to_string(),
            kind: ForeignLaunchKind::RemoteService,
            required_protocols: Vec::new(),
            annotations: dedicated
                .then(|| {
                    BTreeMap::from([(
                        plurora_work::FOREIGN_DEDICATED_SERVER_ANNOTATION.to_string(),
                        serde_json::Value::Bool(true),
                    )])
                })
                .unwrap_or_default(),
        };
        let capsule = ForeignCapsuleDescriptor {
            capsule_id: "tests/run-preflight".to_string(),
            launch_requirements: vec![requirement],
            protocol_ports: Vec::new(),
            state_slots: Vec::new(),
            rights: rights_descriptor.clone(),
            transparency: transparency_descriptor.clone(),
            annotations: BTreeMap::new(),
        };
        let capsule_descriptor = put_model(&objects, &capsule).await?;
        let mut artifacts = preflight_artifacts(
            WorkEntrypointTarget::ForeignLaunch {
                launch_id: "foreign".to_string(),
            },
            false,
            false,
        );
        artifacts.work.content_roots = vec![capsule_descriptor];
        artifacts.work.rights = Some(rights_descriptor);
        artifacts.work.transparency = Some(transparency_descriptor);
        artifacts.work.entrypoints[0].intent_uri = if dedicated {
            FOREIGN_DEDICATED_SERVER_INTENT_URI.to_string()
        } else {
            plurora_work::FOREIGN_PLAY_INTENT_URI.to_string()
        };
        let binding = crate::ForeignLaunchBinding {
            schema: FOREIGN_LAUNCH_BINDING_SCHEMA.to_string(),
            launch_id: "foreign".to_string(),
            target: crate::ForeignLaunchTarget::RemoteService {
                endpoint: "https://service.example.invalid/session".to_string(),
            },
            entitlement,
        };
        let binding_json = serde_json::to_string(&binding)?;
        artifacts
            .installation
            .record
            .secret_policy
            .allowed_secret_refs = vec![crate::foreign_launch_secret_ref("foreign")];
        artifacts.installation.work_summary =
            InstallationWorkSummary::from_work_revision(&artifacts.work);
        let installation_id = artifacts.installation.record.installation_id.clone();
        let revision = artifacts.installation.revision;
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(
            store,
            RuntimeConfig {
                object_store: objects,
                installation_control: Arc::new(StaticInstallationControl { artifacts }),
                secret_resolver: SecretResolverConfig::with_resolver(Arc::new(
                    StaticSecretResolver(binding_json),
                )),
                ..RuntimeConfig::default()
            },
        ));
        let driver = AssemblyRuntimeDriver::new(Arc::downgrade(&runtime));
        Ok((
            runtime,
            driver,
            RunStartRequest {
                installation_id,
                expected_installation_revision: revision,
                entrypoint_id: "default".to_string(),
                idempotency_key: "foreign-start".to_string(),
                authority: None,
            },
        ))
    }

    async fn runnable_driver(
        package_id: &str,
    ) -> anyhow::Result<(
        Arc<Runtime<InMemoryEventStore>>,
        AssemblyRuntimeDriver<InMemoryEventStore>,
        RunStartRequest,
    )> {
        let record = rust_record(package_id);
        let artifacts = runnable_artifacts(&record);
        let installation_id = artifacts.installation.record.installation_id.clone();
        let revision = artifacts.installation.revision;
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(
            store,
            RuntimeConfig {
                installation_control: Arc::new(StaticInstallationControl { artifacts }),
                ..RuntimeConfig::default()
            },
        ));
        runtime.load_package(record.manifest).await?;
        let driver = AssemblyRuntimeDriver::new(Arc::downgrade(&runtime));
        let request = RunStartRequest {
            installation_id,
            expected_installation_revision: revision,
            entrypoint_id: "default".to_string(),
            idempotency_key: "test-start".to_string(),
            authority: None,
        };
        Ok((runtime, driver, request))
    }

    #[test]
    fn exact_match_rejects_absence_ambiguity_behavior_and_trust_drift() {
        let record = rust_record("tests/exact-component");
        let lock = lock_for(&record);
        let selected = exact_loaded_component(&lock, std::slice::from_ref(&record)).unwrap();
        assert_eq!(selected.disclosure.package_id, record.id);
        assert_eq!(
            selected.disclosure.component_artifact,
            selected.claim.component_artifact().clone()
        );
        assert_eq!(
            selected.disclosure.behavior.digest,
            selected.claim.behavior_digest()
        );
        assert_eq!(
            selected.disclosure.trust_class,
            selected.claim.trust_class()
        );

        let absent = exact_loaded_component(&lock, &[]).unwrap_err();
        assert_eq!(absent.reason_code, "artifact_missing");

        let duplicate =
            exact_loaded_component(&lock, &[record.clone(), record.clone()]).unwrap_err();
        assert_eq!(duplicate.reason_code, "binding_ambiguous");

        let mut behavior_drift = lock.clone();
        behavior_drift.behavior_digest = Some(format!("sha256:{}", "f".repeat(64)));
        assert_eq!(
            exact_loaded_component(&behavior_drift, std::slice::from_ref(&record))
                .unwrap_err()
                .reason_code,
            "artifact_missing"
        );

        let mut trust_drift = lock;
        trust_drift.trust_class = Some(ComponentTrustClass::IsolatedProcess);
        assert_eq!(
            exact_loaded_component(&trust_drift, &[record])
                .unwrap_err()
                .reason_code,
            "artifact_missing"
        );
    }

    #[test]
    fn exact_wasm_match_is_an_explicit_unsupported_backend_gap() {
        let record = PackageRecord::ready(manifest(
            "tests/wasm-component",
            PackageEntry::Wasm {
                module: "component.wasm".to_string(),
                abi_version: 1,
                memory_limit_mb: 64,
            },
        ))
        .unwrap();
        let gap = exact_loaded_component(&lock_for(&record), &[record]).unwrap_err();
        assert_eq!(gap.reason_code, "unsupported_backend");
    }

    #[tokio::test]
    async fn shared_preflight_is_structured_and_effect_free() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(store.clone(), RuntimeConfig::default());
        let cases = [
            (
                WorkEntrypointTarget::Surface {
                    surface_id: "tests/surface".to_string(),
                },
                false,
                false,
                None,
            ),
            (
                WorkEntrypointTarget::Surface {
                    surface_id: "tests/surface".to_string(),
                },
                false,
                true,
                Some("artifact_missing"),
            ),
            (
                WorkEntrypointTarget::Surface {
                    surface_id: "tests/surface".to_string(),
                },
                true,
                false,
                Some("target_unsatisfied"),
            ),
            (
                WorkEntrypointTarget::ForeignLaunch {
                    launch_id: "foreign".to_string(),
                },
                false,
                false,
                Some("artifact_missing"),
            ),
        ];

        for (target, managed, missing_component, expected_gap) in cases {
            let artifacts = preflight_artifacts(target, managed, missing_component);
            let preflight = inspect_entrypoint(&runtime, &artifacts, "default").await?;
            assert_eq!(
                preflight.gaps.first().map(|gap| gap.reason_code.as_str()),
                expected_gap
            );
        }

        assert!(store.list_all().await?.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn foreign_remote_service_preflight_and_lifecycle_use_exact_local_binding(
    ) -> anyhow::Result<()> {
        let (runtime, driver, request) = foreign_driver(
            RightDisposition::Allowed,
            RightDisposition::Denied,
            false,
            None,
        )
        .await?;
        let inspection = driver
            .inspect_status(&RunStatusRequest {
                installation_id: request.installation_id.clone(),
                entrypoint_id: Some(request.entrypoint_id.clone()),
            })
            .await?;
        assert!(inspection.preflight.unwrap().gaps.is_empty());
        let run_id = RunId::new();
        let preparation = driver.prepare_start(&request).await?;
        assert!(preparation.gaps.is_empty());
        let mut activation = driver.activate(&run_id, preparation).await?;
        let session_id = activation.context_id.clone().expect("foreign Run session");
        assert_eq!(
            runtime.get_session(&session_id).await.unwrap().status,
            SessionStatus::Open
        );
        driver.stop(&run_id, &mut activation).await?;
        assert_eq!(
            runtime.get_session(&session_id).await.unwrap().status,
            SessionStatus::Closed
        );
        Ok(())
    }

    #[tokio::test]
    async fn foreign_rights_and_entitlement_gaps_are_stable_and_effect_free() -> anyhow::Result<()>
    {
        let cases = [
            (
                RightDisposition::Denied,
                RightDisposition::Allowed,
                false,
                "rights_denied",
            ),
            (
                RightDisposition::Unspecified,
                RightDisposition::Allowed,
                false,
                "rights_unspecified",
            ),
            (
                RightDisposition::RequiresEntitlement,
                RightDisposition::Allowed,
                false,
                "entitlement_required",
            ),
            (
                RightDisposition::Allowed,
                RightDisposition::Denied,
                true,
                "rights_denied",
            ),
        ];
        for (execute, dedicated, dedicated_entrypoint, expected) in cases {
            let (runtime, driver, request) =
                foreign_driver(execute, dedicated, dedicated_entrypoint, None).await?;
            let preparation = driver.prepare_start(&request).await?;
            assert!(preparation
                .gaps
                .iter()
                .any(|gap| gap.reason_code == expected));
            assert!(runtime.sessions.read().await.is_empty());
        }
        Ok(())
    }

    #[tokio::test]
    async fn active_run_lease_blocks_package_unload_and_restart_until_stop() -> anyhow::Result<()> {
        let package_id = "tests/active-run-package".to_string();
        let (runtime, driver, request) = runnable_driver(&package_id).await?;
        let run_id = RunId::new();
        let preparation = driver.prepare_start(&request).await?;
        let mut activation = driver.activate(&run_id, preparation).await?;

        let unload = runtime
            .unload_package(&package_id)
            .await
            .expect_err("an active Run must exclude Package unload");
        assert!(unload.to_string().contains("active Run"));
        let restart = runtime
            .restart_package(&package_id)
            .await
            .expect_err("an active Run must exclude Package restart");
        assert!(restart.to_string().contains("active Run"));
        assert_eq!(
            runtime.packages().active_run_lease_count(&package_id).await,
            1
        );

        driver.stop(&run_id, &mut activation).await?;
        assert_eq!(
            runtime.packages().active_run_lease_count(&package_id).await,
            0
        );
        let unloaded = runtime.unload_package(&package_id).await?;
        assert_eq!(unloaded.state, PackageState::Unloaded);
        Ok(())
    }

    #[tokio::test]
    async fn stopping_one_of_two_runs_keeps_the_shared_package_leased() -> anyhow::Result<()> {
        let package_id = "tests/shared-run-package".to_string();
        let (runtime, driver, request) = runnable_driver(&package_id).await?;
        let run_a = RunId::new();
        let run_b = RunId::new();
        let mut activation_a = driver
            .activate(&run_a, driver.prepare_start(&request).await?)
            .await?;
        let mut activation_b = driver
            .activate(&run_b, driver.prepare_start(&request).await?)
            .await?;
        assert_eq!(
            runtime.packages().active_run_lease_count(&package_id).await,
            2
        );

        driver.stop(&run_a, &mut activation_a).await?;
        assert_eq!(
            runtime.packages().active_run_lease_count(&package_id).await,
            1
        );
        assert!(runtime.unload_package(&package_id).await.is_err());

        driver.stop(&run_b, &mut activation_b).await?;
        assert_eq!(
            runtime.packages().active_run_lease_count(&package_id).await,
            0
        );
        runtime.unload_package(&package_id).await?;
        Ok(())
    }

    #[tokio::test]
    async fn package_replacement_between_preflight_and_activate_cannot_become_running(
    ) -> anyhow::Result<()> {
        let package_id = "tests/run-package-race".to_string();
        let (runtime, driver, request) = runnable_driver(&package_id).await?;
        let preparation = driver.prepare_start(&request).await?;
        assert!(preparation.gaps.is_empty());

        runtime.unload_package(&package_id).await?;
        let mut replacement = rust_record(&package_id).manifest;
        replacement.version = "2.0.0".to_string();
        replacement.entry = EntryDescriptor::v1(PackageEntry::RustInproc {
            crate_ref: "replacement".to_string(),
            symbol: "register".to_string(),
            abi_version: 1,
        });
        runtime.load_package(replacement).await?;

        let error = driver
            .activate(&RunId::new(), preparation)
            .await
            .err()
            .expect("activation must revalidate the exact preflight Package");
        assert!(error.to_string().contains("changed after preflight"));
        assert_eq!(
            runtime.packages().active_run_lease_count(&package_id).await,
            0
        );
        assert!(runtime
            .sessions
            .read()
            .await
            .values()
            .all(|session| session.labels != vec!["host:run".to_string()]));
        Ok(())
    }
}
