use std::collections::BTreeSet;
use std::sync::{Arc, Weak};

use async_trait::async_trait;
use plurora_core::{
    ComponentTrustClass, ContractMode, PackageEntry, PackageId, SessionStatus, SubprocessTransport,
    COMPONENT_DESCRIPTOR_TYPE_URI,
};
use plurora_work::{
    AssemblyLock, AssemblyNodeSource, AssemblyRevision, AvailabilityPolicy, BindingPhase,
    NodeInstanceRecord, NodeInstanceStatus, NodeLock, PortRole, RunId, WorkEntrypointTarget,
    ASSEMBLY_LOCK_TYPE_URI,
};

use crate::{
    package::{PackageRunClaim, PackageRunLease},
    EventStore, OpenSessionRequest, PackageRecord, PackageState, RunActivation, RunGap,
    RunInstallationArtifacts, RunInstallationGuard, RunLifecycleDriver, RunPreparation,
    RunStartRequest, RunStatusInspection, RunStatusRequest, Runtime,
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

struct PreparedAssemblyRun {
    installation: RunInstallationGuard,
    package_claims: Vec<PackageRunClaim>,
    package_ids: Vec<PackageId>,
    nodes: Vec<plurora_work::NodeId>,
}

struct AssemblyPreflight {
    entrypoint_id: String,
    package_claims: Vec<PackageRunClaim>,
    package_ids: Vec<PackageId>,
    nodes: Vec<plurora_work::NodeId>,
    gaps: Vec<RunGap>,
}

struct ActiveAssemblyRun {
    _installation: RunInstallationGuard,
    _packages: PackageRunLease,
}

#[derive(Debug, Clone)]
struct SelectedComponent {
    claim: PackageRunClaim,
    activates_node: bool,
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
        let node_instances = prepared
            .nodes
            .into_iter()
            .map(|node_id| NodeInstanceRecord {
                instance_id: plurora_core::new_id("rni"),
                node_id,
                status: NodeInstanceStatus::Running,
                realization_id: None,
            })
            .collect();
        Ok(RunActivation::new(
            Some(session.id.clone()),
            node_instances,
            Vec::new(),
            Box::new(ActiveAssemblyRun {
                _installation: prepared.installation,
                _packages: package_lease,
            }),
        ))
    }

    async fn stop(&self, _run_id: &RunId, activation: &mut RunActivation) -> anyhow::Result<()> {
        let runtime = self.runtime()?;
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
            gaps: vec![RunGap::new(
                "target_unsatisfied",
                "select an entrypoint declared by this fixed WorkRevision",
            )],
        });
    };

    let mut gaps = Vec::new();
    if artifacts.work.operational_intent.is_some() {
        gaps.push(RunGap::new(
            "target_unsatisfied",
            "plan and apply the required Realization before starting this Run",
        ));
    }
    match &entrypoint.target {
        WorkEntrypointTarget::ForeignLaunch { .. } => gaps.push(RunGap::new(
            "unsupported_backend",
            "use an Assembly or Surface entrypoint supported by the Host runtime",
        )),
        WorkEntrypointTarget::AssemblyPort { port_id } => {
            let root = artifacts
                .assemblies
                .get(&artifacts.work.assembly.digest)
                .ok_or_else(|| anyhow::anyhow!("verified root AssemblyRevision is unavailable"))?;
            let exposure = root
                .exposed_ports
                .iter()
                .find(|exposure| &exposure.port_id == port_id);
            if exposure.is_none_or(|exposure| {
                root.nodes
                    .iter()
                    .find(|node| node.node_id == exposure.target.node_id)
                    .is_none_or(|node| {
                        !node
                            .ports
                            .iter()
                            .any(|port| port.port_id == exposure.target.port_id)
                    })
            }) {
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
    collect_locked_nodes(
        root_assembly,
        root_lock,
        &artifacts.assemblies,
        &artifacts.locks,
        &packages,
        &mut selected,
        &mut gaps,
    )?;

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
    let package_claims = selected
        .into_iter()
        .map(|(_, component)| component.claim)
        .collect();
    Ok(AssemblyPreflight {
        entrypoint_id: entrypoint.id.clone(),
        package_claims,
        package_ids,
        nodes,
        gaps,
    })
}

fn collect_locked_nodes(
    assembly: &AssemblyRevision,
    lock: &AssemblyLock,
    assemblies: &std::collections::BTreeMap<String, AssemblyRevision>,
    locks: &std::collections::BTreeMap<String, AssemblyLock>,
    packages: &[PackageRecord],
    selected: &mut Vec<(plurora_work::NodeId, SelectedComponent)>,
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
        let node_lock = lock
            .nodes
            .iter()
            .find(|candidate| candidate.node_id == node.node_id)
            .ok_or_else(|| anyhow::anyhow!("verified AssemblyLock is missing a node"))?;
        validate_required_bindings(node, lock, gaps);
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
                collect_locked_nodes(
                    nested_assembly,
                    nested_lock,
                    assemblies,
                    locks,
                    packages,
                    selected,
                    gaps,
                )?;
            }
            (AssemblyNodeSource::Component { component }, COMPONENT_DESCRIPTOR_TYPE_URI) => {
                anyhow::ensure!(
                    component == &node_lock.artifact,
                    "AssemblyLock component pin differs from the AssemblyRevision"
                );
                match exact_loaded_component(node_lock, packages) {
                    Ok(component) => selected.push((node.node_id.clone(), component)),
                    Err(gap) => gaps.push(gap.for_node(&node.node_id)),
                }
            }
            _ => anyhow::bail!("verified Assembly and AssemblyLock node kinds disagree"),
        }
    }
    Ok(())
}

fn validate_required_bindings(
    node: &plurora_work::AssemblyNode,
    lock: &AssemblyLock,
    gaps: &mut Vec<RunGap>,
) {
    for port in &node.ports {
        let PortRole::Import {
            latest_binding_phase,
            availability: AvailabilityPolicy::Required,
            ..
        } = &port.role
        else {
            continue;
        };
        if !matches!(
            latest_binding_phase,
            BindingPhase::Launch | BindingPhase::Runtime
        ) {
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
            gaps.push(
                RunGap::new(
                    "binding_unavailable",
                    "create an explicit Exposure/Binding in Phase 5 before starting the Run",
                )
                .for_node(&node.node_id)
                .for_port(&port.port_id),
            );
        }
    }
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
        PackageEntry::RustInproc { .. } => Ok(SelectedComponent {
            claim: PackageRunClaim::exact(package, component).map_err(|_| {
                RunGap::new(
                    "artifact_digest_mismatch",
                    "reload the exact Package selected by the AssemblyLock",
                )
            })?,
            activates_node: true,
        }),
        PackageEntry::Subprocess {
            transport: SubprocessTransport::JsonRpcStdio,
            ..
        } => Ok(SelectedComponent {
            claim: PackageRunClaim::exact(package, component).map_err(|_| {
                RunGap::new(
                    "artifact_digest_mismatch",
                    "reload the exact Package selected by the AssemblyLock",
                )
            })?,
            activates_node: true,
        }),
        PackageEntry::SurfaceBundle { .. } => Ok(SelectedComponent {
            claim: PackageRunClaim::exact(package, component).map_err(|_| {
                RunGap::new(
                    "artifact_digest_mismatch",
                    "reload the exact Package selected by the AssemblyLock",
                )
            })?,
            activates_node: false,
        }),
        PackageEntry::Subprocess { .. }
        | PackageEntry::Wasm { .. }
        | PackageEntry::Remote { .. } => Err(RunGap::new(
            "unsupported_backend",
            "load the locked component with rust_inproc or json_rpc_stdio support",
        )),
    }
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
        AcquisitionKind, AcquisitionRecord, AssemblyId, AssemblyNode, InstallationId,
        InstallationRecord, InstallationSecretPolicy, InstallationStatus, NodeId, WorkEntrypoint,
        WorkId, WorkRevision, ASSEMBLY_REVISION_TYPE_URI, OPERATIONAL_INTENT_TYPE_URI,
        WORK_REVISION_TYPE_URI,
    };

    use super::*;
    use crate::{
        InMemoryEventStore, InstallationControl, InstallationView, InstallationWorkSummary,
        RuntimeConfig,
    };

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
        exact_loaded_component(&lock, std::slice::from_ref(&record)).unwrap();

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
                Some("unsupported_backend"),
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
