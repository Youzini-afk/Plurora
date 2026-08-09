use std::collections::{BTreeMap, BTreeSet};

use plurora_core::{
    ArtifactDescriptor, ComponentDescriptor, ComponentTrustClass, ProtocolProfilePin,
    COMPONENT_DESCRIPTOR_TYPE_URI,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::assembly::{
    AssemblyNodeSource, AssemblyRevision, MAX_ASSEMBLY_DEPTH, MAX_PORTS_PER_NODE,
};
use crate::canonical::{validate_artifact_descriptor, validate_descriptor_type, ArtifactModel};
use crate::diagnostic::{
    DiagnosticCode, DiagnosticReport, DiagnosticSeverity, ModelError, ModelResult, WorkDiagnostic,
};
use crate::ids::{NodeId, PortId, StateSlotId};
use crate::lock::{AssemblyLock, BindingLock, NodeLock};
use crate::package::CanonicalArtifactObject;
use crate::port::{
    check_port_compatibility, select_transport, AvailabilityPolicy, BindingPhase, PortDescriptor,
    PortDirection, PortEndpoint, PortRole, SelectedTransport, TransportPolicy,
};
use crate::state::StateSlotDescriptor;

pub const MAX_PROVIDER_CANDIDATES: usize = 256;
pub const MAX_RESOLVER_DIAGNOSTICS: usize = 1_000;

#[derive(
    Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(transparent)]
pub struct NodePath(pub Vec<NodeId>);

impl NodePath {
    pub fn child(&self, node_id: &NodeId) -> Self {
        let mut path = self.0.clone();
        path.push(node_id.clone());
        Self(path)
    }

    pub fn as_slice(&self) -> &[NodeId] {
        &self.0
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct NodeEvidenceKey {
    pub assembly_digest: String,
    pub node_id: NodeId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct NodeEvidence {
    pub component: ComponentDescriptor,
    pub ports: Vec<PortDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance_refs: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_refs: Vec<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ResolverInput {
    pub root_assembly: ArtifactDescriptor,
    pub assemblies: BTreeMap<String, AssemblyRevision>,
    pub node_evidence: BTreeMap<NodeEvidenceKey, NodeEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content_roots: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocol_profiles: Vec<ProtocolProfilePin>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExposureHop {
    pub assembly_digest: String,
    pub assembly_id: crate::AssemblyId,
    pub exposed_port_id: PortId,
    pub target: PortEndpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct FlattenedNode {
    pub node_path: NodePath,
    pub origin_assembly_digest: String,
    pub origin_assembly_id: crate::AssemblyId,
    pub origin_lock: ArtifactDescriptor,
    pub component: ArtifactDescriptor,
    pub behavior: ArtifactDescriptor,
    pub trust_class: ComponentTrustClass,
    pub ports: Vec<PortDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exposure_chain: Vec<ExposureHop>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance_refs: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_refs: Vec<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ResolvedExposedPort {
    pub port_id: PortId,
    pub direction: PortDirection,
    pub leaf_node_path: NodePath,
    pub leaf_port_id: PortId,
    pub descriptor: PortDescriptor,
    pub exposure_chain: Vec<ExposureHop>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ResolvedBinding {
    pub origin_assembly_digest: String,
    pub origin_assembly_id: crate::AssemblyId,
    pub binding_id: String,
    pub provider_node_path: NodePath,
    pub provider_port_id: PortId,
    pub consumer_node_path: NodePath,
    pub consumer_port_id: PortId,
    pub provider_component: ArtifactDescriptor,
    pub transport: SelectedTransport,
    pub phase: BindingPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ResolvedStateSlot {
    pub origin_assembly_digest: String,
    pub origin_assembly_id: crate::AssemblyId,
    pub state_slot_id: StateSlotId,
    pub owner_node_path: NodePath,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_node_path: Option<NodePath>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_port_id: Option<PortId>,
    pub descriptor: StateSlotDescriptor,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ResolverOutput {
    pub root_lock: AssemblyLock,
    pub root_lock_artifact: CanonicalArtifactObject,
    pub lock_artifacts: Vec<CanonicalArtifactObject>,
    pub flattened_nodes: Vec<FlattenedNode>,
    pub exposed_ports: Vec<ResolvedExposedPort>,
    pub bindings: Vec<ResolvedBinding>,
    pub state_slots: Vec<ResolvedStateSlot>,
    pub diagnostics: DiagnosticReport,
    pub complete: bool,
    pub portable: bool,
}

#[derive(Clone)]
struct ResolvedPortRef {
    leaf_node_path: NodePath,
    leaf_port_id: PortId,
    descriptor: PortDescriptor,
    component: ArtifactDescriptor,
    exposure_chain: Vec<ExposureHop>,
}

type ResolvedPortKey = (NodePath, PortId);

fn resolved_port_key(port: &ResolvedPortRef) -> ResolvedPortKey {
    (port.leaf_node_path.clone(), port.leaf_port_id.clone())
}

#[derive(Clone)]
struct OccurrenceResult {
    external_ports: BTreeMap<PortId, ResolvedPortRef>,
    external_directions: BTreeMap<PortId, PortDirection>,
    lock: AssemblyLock,
    lock_artifact: CanonicalArtifactObject,
    lock_artifacts: Vec<CanonicalArtifactObject>,
    flattened_nodes: Vec<FlattenedNode>,
    bindings: Vec<ResolvedBinding>,
    state_slots: Vec<ResolvedStateSlot>,
}

#[derive(Clone)]
struct BindingPath {
    provider: ResolvedPortRef,
    transport: SelectedTransport,
}

struct DiagnosticCollector {
    diagnostics: Vec<WorkDiagnostic>,
    omitted_count: u64,
    complete: bool,
    portable: bool,
}

impl Default for DiagnosticCollector {
    fn default() -> Self {
        Self {
            diagnostics: Vec::new(),
            omitted_count: 0,
            complete: true,
            portable: true,
        }
    }
}

impl DiagnosticCollector {
    fn push(&mut self, diagnostic: WorkDiagnostic) {
        if self.diagnostics.len() < MAX_RESOLVER_DIAGNOSTICS {
            self.diagnostics.push(diagnostic);
        } else {
            self.omitted_count += 1;
        }
    }

    fn unresolved(
        &mut self,
        code: DiagnosticCode,
        port: &ResolvedPortRef,
        availability: AvailabilityPolicy,
        phase: BindingPhase,
        mut candidate_refs: Vec<ArtifactDescriptor>,
    ) {
        sort_artifacts(&mut candidate_refs);
        let severity = match availability {
            AvailabilityPolicy::Required if phase == BindingPhase::Authoring => {
                self.complete = false;
                self.portable = false;
                DiagnosticSeverity::Error
            }
            AvailabilityPolicy::Required => {
                self.complete = false;
                DiagnosticSeverity::Warning
            }
            AvailabilityPolicy::DegradedWithout => DiagnosticSeverity::Warning,
            AvailabilityPolicy::Optional => DiagnosticSeverity::Info,
        };
        self.push(WorkDiagnostic {
            code,
            severity,
            message: match code {
                DiagnosticCode::BindingAmbiguous => {
                    "import has more than one compatible provider path".to_string()
                }
                DiagnosticCode::PortIncompatible => {
                    "declared binding has incompatible Port contracts".to_string()
                }
                _ => "import has no compatible provider path".to_string(),
            },
            field: Some("assembly.import".to_string()),
            phase: Some(phase),
            node_path: port.leaf_node_path.0.clone(),
            port_id: Some(port.leaf_port_id.clone()),
            candidate_refs,
        });
    }

    fn finish(mut self) -> DiagnosticReport {
        self.diagnostics.sort_by(|left, right| {
            (
                left.code.as_str(),
                left.severity,
                left.phase,
                &left.node_path,
                &left.port_id,
                &left.field,
                left.candidate_refs
                    .iter()
                    .map(|reference| reference.digest.as_str())
                    .collect::<Vec<_>>(),
                &left.message,
            )
                .cmp(&(
                    right.code.as_str(),
                    right.severity,
                    right.phase,
                    &right.node_path,
                    &right.port_id,
                    &right.field,
                    right
                        .candidate_refs
                        .iter()
                        .map(|reference| reference.digest.as_str())
                        .collect::<Vec<_>>(),
                    &right.message,
                ))
        });
        DiagnosticReport {
            diagnostics: self.diagnostics,
            omitted_count: self.omitted_count,
        }
    }
}

pub fn resolve_assembly(input: &ResolverInput) -> ModelResult<ResolverOutput> {
    resolve_assembly_at_phase(input, BindingPhase::Authoring)
}

pub fn resolve_assembly_at_phase(
    input: &ResolverInput,
    resolution_phase: BindingPhase,
) -> ModelResult<ResolverOutput> {
    validate_resolver_containment(&input.root_assembly, &input.assemblies)?;
    for root in &input.content_roots {
        validate_artifact_descriptor(root)?;
    }
    let mut context = ResolveContext {
        input,
        resolution_phase,
        validated_evidence: BTreeSet::new(),
        memo: BTreeMap::new(),
        diagnostics: DiagnosticCollector::default(),
    };
    let root =
        context.resolve_occurrence(&input.root_assembly.digest, &NodePath::default(), 1, true)?;
    for port in root.external_ports.values() {
        if let PortRole::Import {
            ref multiplicity,
            latest_binding_phase,
            availability,
            ..
        } = port.descriptor.role
        {
            if multiplicity.min > 0 {
                context.diagnostics.unresolved(
                    DiagnosticCode::BindingUnavailable,
                    port,
                    availability,
                    latest_binding_phase,
                    Vec::new(),
                );
            }
        }
    }
    let complete = context.diagnostics.complete;
    let portable = context.diagnostics.portable;
    let diagnostics = std::mem::take(&mut context.diagnostics).finish();
    let mut exposed_ports = root
        .external_ports
        .iter()
        .map(|(port_id, resolved)| ResolvedExposedPort {
            port_id: port_id.clone(),
            direction: root.external_directions[port_id],
            leaf_node_path: resolved.leaf_node_path.clone(),
            leaf_port_id: resolved.leaf_port_id.clone(),
            descriptor: resolved.descriptor.clone(),
            exposure_chain: resolved.exposure_chain.clone(),
        })
        .collect::<Vec<_>>();
    exposed_ports.sort_by(|left, right| left.port_id.cmp(&right.port_id));
    let mut flattened_nodes = root.flattened_nodes;
    for node in &mut flattened_nodes {
        let mut chain = exposed_ports
            .iter()
            .filter(|port| port.leaf_node_path == node.node_path)
            .flat_map(|port| port.exposure_chain.iter().cloned())
            .collect::<Vec<_>>();
        chain.sort_by(|left, right| {
            (&left.assembly_digest, &left.exposed_port_id, &left.target).cmp(&(
                &right.assembly_digest,
                &right.exposed_port_id,
                &right.target,
            ))
        });
        chain.dedup();
        node.exposure_chain = chain;
    }
    flattened_nodes.sort_by(|left, right| left.node_path.cmp(&right.node_path));
    let mut bindings = root.bindings;
    bindings.sort_by(|left, right| {
        (
            &left.origin_assembly_digest,
            &left.binding_id,
            &left.consumer_node_path,
            &left.consumer_port_id,
        )
            .cmp(&(
                &right.origin_assembly_digest,
                &right.binding_id,
                &right.consumer_node_path,
                &right.consumer_port_id,
            ))
    });
    validate_export_minimums(&flattened_nodes, &bindings, &exposed_ports)?;
    let mut state_slots = root.state_slots;
    state_slots.sort_by(|left, right| {
        (
            &left.origin_assembly_digest,
            &left.state_slot_id,
            &left.owner_node_path,
        )
            .cmp(&(
                &right.origin_assembly_digest,
                &right.state_slot_id,
                &right.owner_node_path,
            ))
    });
    let mut lock_artifacts = root.lock_artifacts;
    lock_artifacts.sort_by(|left, right| left.descriptor.digest.cmp(&right.descriptor.digest));
    lock_artifacts.dedup_by(|left, right| left.descriptor.digest == right.descriptor.digest);
    Ok(ResolverOutput {
        root_lock: root.lock,
        root_lock_artifact: root.lock_artifact,
        lock_artifacts,
        flattened_nodes,
        exposed_ports,
        bindings,
        state_slots,
        diagnostics,
        complete,
        portable,
    })
}

fn validate_export_minimums(
    nodes: &[FlattenedNode],
    bindings: &[ResolvedBinding],
    root_exposures: &[ResolvedExposedPort],
) -> ModelResult<()> {
    let mut counts = BTreeMap::<ResolvedPortKey, usize>::new();
    for binding in bindings {
        *counts
            .entry((
                binding.provider_node_path.clone(),
                binding.provider_port_id.clone(),
            ))
            .or_default() += 1;
    }
    let deferred = root_exposures
        .iter()
        .filter(|exposure| exposure.direction == PortDirection::Export)
        .map(|exposure| {
            (
                exposure.leaf_node_path.clone(),
                exposure.leaf_port_id.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    for node in nodes {
        for port in &node.ports {
            let PortRole::Export { multiplicity, .. } = &port.role else {
                continue;
            };
            let key = (node.node_path.clone(), port.port_id.clone());
            if !deferred.contains(&key)
                && counts.get(&key).copied().unwrap_or_default() < usize::from(multiplicity.min)
            {
                return Err(ModelError::new(
                    DiagnosticCode::PortIncompatible,
                    "export Port binding count is below its declared minimum",
                ));
            }
        }
    }
    Ok(())
}

struct ResolveContext<'a> {
    input: &'a ResolverInput,
    resolution_phase: BindingPhase,
    validated_evidence: BTreeSet<NodeEvidenceKey>,
    memo: BTreeMap<String, (NodePath, OccurrenceResult)>,
    diagnostics: DiagnosticCollector,
}

impl ResolveContext<'_> {
    fn resolve_occurrence(
        &mut self,
        assembly_digest: &str,
        prefix: &NodePath,
        depth: usize,
        root: bool,
    ) -> ModelResult<OccurrenceResult> {
        if depth > MAX_ASSEMBLY_DEPTH {
            return Err(complexity_error("nested Assembly depth exceeds 32"));
        }
        if !root {
            if let Some((cached_prefix, cached)) = self.memo.get(assembly_digest).cloned() {
                return Ok(rebase_occurrence(cached, &cached_prefix, prefix));
            }
        }
        let diagnostic_count = self.diagnostics.diagnostics.len();
        let omitted_count = self.diagnostics.omitted_count;
        let assembly = self.input.assemblies.get(assembly_digest).ok_or_else(|| {
            ModelError::new(
                DiagnosticCode::ArtifactMissing,
                "Assembly closure is missing a referenced revision",
            )
        })?;
        let mut local_ports = BTreeMap::<PortEndpoint, ResolvedPortRef>::new();
        let mut node_locks = Vec::new();
        let mut child_results = Vec::new();
        let mut local_components = Vec::<(NodeId, NodeEvidence)>::new();
        let mut sorted_nodes = assembly.nodes.iter().collect::<Vec<_>>();
        sorted_nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        for node in sorted_nodes {
            let node_path = prefix.child(&node.node_id);
            match &node.source {
                AssemblyNodeSource::Component { component } => {
                    let key = NodeEvidenceKey {
                        assembly_digest: assembly_digest.to_string(),
                        node_id: node.node_id.clone(),
                    };
                    let evidence = self.input.node_evidence.get(&key).ok_or_else(|| {
                        ModelError::new(
                            DiagnosticCode::ArtifactMissing,
                            "component node is missing resolver evidence",
                        )
                    })?;
                    self.validate_evidence(&key, evidence, component, &node.ports)?;
                    if evidence.ports.len() > MAX_PORTS_PER_NODE {
                        return Err(complexity_error("component node exceeds 1,024 Ports"));
                    }
                    for port in &evidence.ports {
                        local_ports.insert(
                            PortEndpoint {
                                node_id: node.node_id.clone(),
                                port_id: port.port_id.clone(),
                            },
                            ResolvedPortRef {
                                leaf_node_path: node_path.clone(),
                                leaf_port_id: port.port_id.clone(),
                                descriptor: port.clone(),
                                component: evidence.component.artifact.clone(),
                                exposure_chain: Vec::new(),
                            },
                        );
                    }
                    node_locks.push(NodeLock {
                        node_id: node.node_id.clone(),
                        artifact: evidence.component.artifact.clone(),
                        behavior_digest: Some(evidence.component.behavior.digest.clone()),
                        trust_class: Some(evidence.component.trust_class),
                    });
                    local_components.push((node.node_id.clone(), evidence.clone()));
                }
                AssemblyNodeSource::Assembly { assembly: child } => {
                    let child_result =
                        self.resolve_occurrence(&child.digest, &node_path, depth + 1, false)?;
                    for (port_id, port) in &child_result.external_ports {
                        local_ports.insert(
                            PortEndpoint {
                                node_id: node.node_id.clone(),
                                port_id: port_id.clone(),
                            },
                            port.clone(),
                        );
                    }
                    node_locks.push(NodeLock {
                        node_id: node.node_id.clone(),
                        artifact: child_result.lock_artifact.descriptor.clone(),
                        behavior_digest: None,
                        trust_class: None,
                    });
                    child_results.push(child_result);
                }
            }
        }

        let outward_imports = assembly
            .exposed_ports
            .iter()
            .filter(|exposure| exposure.direction == PortDirection::Import)
            .map(|exposure| &exposure.target)
            .collect::<BTreeSet<_>>();
        let mut consumer_counts = BTreeMap::<ResolvedPortKey, usize>::new();
        let mut provider_counts = BTreeMap::<ResolvedPortKey, usize>::new();
        for child in &child_results {
            for binding in &child.bindings {
                *consumer_counts
                    .entry((
                        binding.consumer_node_path.clone(),
                        binding.consumer_port_id.clone(),
                    ))
                    .or_default() += 1;
                *provider_counts
                    .entry((
                        binding.provider_node_path.clone(),
                        binding.provider_port_id.clone(),
                    ))
                    .or_default() += 1;
            }
        }
        let mut binding_locks = Vec::new();
        let mut local_resolved_bindings = Vec::new();
        let mut used_binding_ids = assembly
            .bindings
            .iter()
            .map(|binding| binding.binding_id.clone())
            .collect::<BTreeSet<_>>();

        let mut declared_bindings = assembly.bindings.iter().collect::<Vec<_>>();
        declared_bindings.sort_by(|left, right| left.binding_id.cmp(&right.binding_id));
        let declared_consumers = declared_bindings
            .iter()
            .map(|binding| &binding.consumer)
            .collect::<BTreeSet<_>>();
        for binding in declared_bindings {
            let provider = local_ports.get(&binding.provider);
            let consumer = local_ports.get(&binding.consumer);
            let Some(consumer) = consumer else {
                return Err(ModelError::new(
                    DiagnosticCode::PortUnresolved,
                    "declared binding consumer is absent from resolver evidence",
                ));
            };
            let PortRole::Import {
                latest_binding_phase,
                ..
            } = consumer.descriptor.role
            else {
                return Err(ModelError::new(
                    DiagnosticCode::PortIncompatible,
                    "declared binding consumer is not an import Port",
                ));
            };
            let Some(provider) = provider else {
                self.emit_binding_failure(
                    DiagnosticCode::BindingUnavailable,
                    consumer,
                    binding.phase,
                    Vec::new(),
                );
                continue;
            };
            if binding.phase > latest_binding_phase {
                self.emit_binding_failure(
                    DiagnosticCode::PortIncompatible,
                    consumer,
                    binding.phase,
                    vec![provider.component.clone()],
                );
                continue;
            }
            if binding.phase > self.resolution_phase {
                self.emit_binding_failure(
                    DiagnosticCode::BindingUnavailable,
                    consumer,
                    binding.phase,
                    vec![provider.component.clone()],
                );
                continue;
            }
            let candidates = self.binding_paths(provider, consumer, &binding.transport_policy)?;
            if candidates.len() == 1 {
                let candidate = candidates.into_iter().next().expect("single candidate");
                let provider_key = resolved_port_key(&candidate.provider);
                let consumer_key = resolved_port_key(consumer);
                self.record_binding(
                    assembly_digest,
                    assembly,
                    &binding.binding_id,
                    &binding.provider,
                    &binding.consumer,
                    consumer,
                    binding.phase,
                    candidate,
                    &mut binding_locks,
                    &mut local_resolved_bindings,
                )?;
                *consumer_counts.entry(consumer_key).or_default() += 1;
                *provider_counts.entry(provider_key).or_default() += 1;
            } else {
                self.emit_binding_failure(
                    if candidates.is_empty() {
                        DiagnosticCode::PortIncompatible
                    } else {
                        DiagnosticCode::BindingAmbiguous
                    },
                    consumer,
                    binding.phase,
                    candidate_artifacts(&candidates),
                );
            }
        }

        let mut imports = local_ports
            .iter()
            .filter(|(endpoint, port)| {
                matches!(port.descriptor.role, PortRole::Import { .. })
                    && !outward_imports.contains(endpoint)
                    && !declared_consumers.contains(endpoint)
            })
            .map(|(endpoint, port)| (endpoint.clone(), port.clone()))
            .collect::<Vec<_>>();
        imports.sort_by(|left, right| left.0.cmp(&right.0));
        for (consumer_endpoint, consumer) in imports {
            let PortRole::Import {
                ref multiplicity,
                latest_binding_phase,
                availability,
                ..
            } = consumer.descriptor.role
            else {
                unreachable!()
            };
            let current = consumer_counts
                .get(&resolved_port_key(&consumer))
                .copied()
                .unwrap_or_default();
            if current >= usize::from(multiplicity.min) {
                continue;
            }
            if latest_binding_phase > self.resolution_phase {
                self.diagnostics.unresolved(
                    DiagnosticCode::BindingUnavailable,
                    &consumer,
                    availability,
                    latest_binding_phase,
                    Vec::new(),
                );
                continue;
            }
            let raw_providers = local_ports
                .iter()
                .filter(|(_, port)| matches!(port.descriptor.role, PortRole::Export { .. }))
                .collect::<Vec<_>>();
            if raw_providers.len() > MAX_PROVIDER_CANDIDATES {
                return Err(complexity_error(
                    "raw provider candidate count exceeds 256 for one import",
                ));
            }
            let mut paths = Vec::<(PortEndpoint, BindingPath)>::new();
            for (provider_endpoint, provider) in raw_providers {
                for path in self.binding_paths(provider, &consumer, &TransportPolicy::default())? {
                    paths.push((provider_endpoint.clone(), path));
                }
            }
            paths.sort_by(|left, right| {
                (&left.0, &left.1.provider.component.digest)
                    .cmp(&(&right.0, &right.1.provider.component.digest))
            });
            if paths.len() == 1 {
                let (provider_endpoint, candidate) = paths.pop().expect("single candidate");
                let binding_id = automatic_binding_id(&consumer_endpoint, &used_binding_ids);
                used_binding_ids.insert(binding_id.clone());
                let provider_key = resolved_port_key(&candidate.provider);
                let consumer_key = resolved_port_key(&consumer);
                self.record_binding(
                    assembly_digest,
                    assembly,
                    &binding_id,
                    &provider_endpoint,
                    &consumer_endpoint,
                    &consumer,
                    latest_binding_phase,
                    candidate,
                    &mut binding_locks,
                    &mut local_resolved_bindings,
                )?;
                *consumer_counts.entry(consumer_key).or_default() += 1;
                *provider_counts.entry(provider_key).or_default() += 1;
                if current + 1 < usize::from(multiplicity.min) {
                    self.diagnostics.unresolved(
                        DiagnosticCode::BindingUnavailable,
                        &consumer,
                        availability,
                        latest_binding_phase,
                        Vec::new(),
                    );
                }
            } else {
                self.diagnostics.unresolved(
                    if paths.is_empty() {
                        DiagnosticCode::BindingUnavailable
                    } else {
                        DiagnosticCode::BindingAmbiguous
                    },
                    &consumer,
                    availability,
                    latest_binding_phase,
                    paths
                        .iter()
                        .map(|(_, path)| path.provider.component.clone())
                        .collect(),
                );
            }
        }

        for port in local_ports.values() {
            let key = resolved_port_key(port);
            let count = match port.descriptor.role {
                PortRole::Import { .. } => consumer_counts.get(&key).copied().unwrap_or(0),
                PortRole::Export { .. } => provider_counts.get(&key).copied().unwrap_or(0),
            };
            if port
                .descriptor
                .role
                .multiplicity()
                .max
                .is_some_and(|max| count > usize::from(max))
            {
                return Err(ModelError::new(
                    DiagnosticCode::PortIncompatible,
                    "resolved binding count exceeds Port cardinality",
                ));
            }
        }

        let mut external_ports = BTreeMap::new();
        let mut external_directions = BTreeMap::new();
        let mut exposures = assembly.exposed_ports.iter().collect::<Vec<_>>();
        exposures.sort_by(|left, right| left.port_id.cmp(&right.port_id));
        for exposure in exposures {
            let target = local_ports.get(&exposure.target).ok_or_else(|| {
                ModelError::new(
                    DiagnosticCode::PortUnresolved,
                    "Assembly exposure target is missing from resolver evidence",
                )
            })?;
            let matches = matches!(
                (exposure.direction, &target.descriptor.role),
                (PortDirection::Import, PortRole::Import { .. })
                    | (PortDirection::Export, PortRole::Export { .. })
            );
            if !matches {
                return Err(ModelError::new(
                    DiagnosticCode::PortIncompatible,
                    "Assembly exposure direction does not match its target Port",
                ));
            }
            let mut resolved = target.clone();
            resolved.descriptor.port_id = exposure.port_id.clone();
            let hop = ExposureHop {
                assembly_digest: assembly_digest.to_string(),
                assembly_id: assembly.assembly_id.clone(),
                exposed_port_id: exposure.port_id.clone(),
                target: exposure.target.clone(),
            };
            let mut chain = vec![hop];
            chain.extend(resolved.exposure_chain);
            resolved.exposure_chain = chain;
            external_directions.insert(exposure.port_id.clone(), exposure.direction);
            external_ports.insert(exposure.port_id.clone(), resolved);
        }

        let mut content_roots = Vec::new();
        let mut protocol_profiles = Vec::new();
        for (_, evidence) in &local_components {
            content_roots.extend(evidence.component.content_roots.clone());
            protocol_profiles.extend(component_protocol_profiles(&evidence.component));
        }
        for child in &child_results {
            content_roots.extend(child.lock.content_roots.clone());
            protocol_profiles.extend(child.lock.protocol_profiles.clone());
        }
        if root {
            content_roots.extend(self.input.content_roots.clone());
            protocol_profiles.extend(self.input.protocol_profiles.clone());
        }
        sort_artifacts(&mut content_roots);
        sort_profiles(&mut protocol_profiles);
        node_locks.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        binding_locks.sort_by(|left, right| left.binding_id.cmp(&right.binding_id));
        let assembly_descriptor = assembly.artifact_descriptor()?;
        let lock = AssemblyLock {
            schema: AssemblyLock::SCHEMA.to_string(),
            assembly: assembly_descriptor,
            nodes: node_locks,
            bindings: binding_locks,
            protocol_profiles,
            content_roots,
        };
        lock.validate()?;
        let lock_artifact = CanonicalArtifactObject::from_model(&lock)?;

        let mut flattened_nodes = Vec::new();
        let mut resolved_bindings = Vec::new();
        let mut resolved_state_slots = Vec::new();
        let mut lock_artifacts = Vec::new();
        for child in child_results {
            flattened_nodes.extend(child.flattened_nodes);
            resolved_bindings.extend(child.bindings);
            resolved_state_slots.extend(child.state_slots);
            lock_artifacts.extend(child.lock_artifacts);
        }
        for (node_id, mut evidence) in local_components {
            evidence
                .ports
                .sort_by(|left, right| left.port_id.cmp(&right.port_id));
            flattened_nodes.push(FlattenedNode {
                node_path: prefix.child(&node_id),
                origin_assembly_digest: assembly_digest.to_string(),
                origin_assembly_id: assembly.assembly_id.clone(),
                origin_lock: lock_artifact.descriptor.clone(),
                component: evidence.component.artifact,
                behavior: evidence.component.behavior,
                trust_class: evidence.component.trust_class,
                ports: evidence.ports,
                exposure_chain: Vec::new(),
                provenance_refs: evidence.provenance_refs,
                artifact_refs: evidence.artifact_refs,
            });
        }
        resolved_bindings.extend(local_resolved_bindings);
        for slot in &assembly.state_slots {
            let owner_path = prefix.child(&slot.owner_node_id);
            let (migration_node_path, migration_port_id) = slot
                .migration_port
                .as_ref()
                .map(|endpoint| {
                    local_ports
                        .get(endpoint)
                        .and_then(|port| {
                            port.descriptor
                                .interaction
                                .is_directly_supported()
                                .then(|| {
                                    (port.leaf_node_path.clone(), port.leaf_port_id.clone())
                                })
                        })
                        .ok_or_else(|| {
                            ModelError::new(
                                DiagnosticCode::StateMigrationRequired,
                                "state migration Port is absent or has no executable interaction implementation",
                            )
                        })
                })
                .transpose()?
                .map_or((None, None), |(path, port)| (Some(path), Some(port)));
            resolved_state_slots.push(ResolvedStateSlot {
                origin_assembly_digest: assembly_digest.to_string(),
                origin_assembly_id: assembly.assembly_id.clone(),
                state_slot_id: slot.state_slot_id.clone(),
                owner_node_path: owner_path,
                migration_node_path,
                migration_port_id,
                descriptor: slot.clone(),
            });
        }
        lock_artifacts.push(lock_artifact.clone());
        let result = OccurrenceResult {
            external_ports,
            external_directions,
            lock,
            lock_artifact,
            lock_artifacts,
            flattened_nodes,
            bindings: resolved_bindings,
            state_slots: resolved_state_slots,
        };
        if !root
            && self.diagnostics.diagnostics.len() == diagnostic_count
            && self.diagnostics.omitted_count == omitted_count
        {
            self.memo.insert(
                assembly_digest.to_string(),
                (prefix.clone(), result.clone()),
            );
        }
        Ok(result)
    }

    fn validate_evidence(
        &mut self,
        key: &NodeEvidenceKey,
        evidence: &NodeEvidence,
        expected_component: &ArtifactDescriptor,
        expected_ports: &[PortDescriptor],
    ) -> ModelResult<()> {
        if self.validated_evidence.contains(key) {
            return Ok(());
        }
        validate_descriptor_type(expected_component, COMPONENT_DESCRIPTOR_TYPE_URI)?;
        validate_descriptor_type(&evidence.component.artifact, COMPONENT_DESCRIPTOR_TYPE_URI)?;
        validate_artifact_descriptor(&evidence.component.behavior)?;
        if &evidence.component.artifact != expected_component {
            return Err(ModelError::new(
                DiagnosticCode::ArtifactDigestMismatch,
                "node evidence component does not match the Assembly reference",
            ));
        }
        if evidence.ports != expected_ports {
            return Err(ModelError::new(
                DiagnosticCode::ArtifactDigestMismatch,
                "node evidence Ports do not match the canonical Assembly revision",
            ));
        }
        let mut ports = BTreeSet::new();
        for port in &evidence.ports {
            port.validate()?;
            if !ports.insert(&port.port_id) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "node evidence contains a duplicate Port id",
                ));
            }
        }
        for reference in evidence
            .provenance_refs
            .iter()
            .chain(&evidence.artifact_refs)
        {
            validate_artifact_descriptor(reference)?;
        }
        self.validated_evidence.insert(key.clone());
        Ok(())
    }

    fn binding_paths(
        &self,
        provider: &ResolvedPortRef,
        consumer: &ResolvedPortRef,
        policy: &TransportPolicy,
    ) -> ModelResult<Vec<BindingPath>> {
        if check_port_compatibility(&provider.descriptor, &consumer.descriptor).is_ok() {
            return Ok(vec![BindingPath {
                provider: provider.clone(),
                transport: select_transport(
                    &provider.descriptor.transport,
                    &consumer.descriptor.transport,
                    policy,
                )?,
            }]);
        }
        Ok(Vec::new())
    }

    #[allow(clippy::too_many_arguments)]
    fn record_binding(
        &self,
        assembly_digest: &str,
        assembly: &AssemblyRevision,
        binding_id: &str,
        provider_endpoint: &PortEndpoint,
        consumer_endpoint: &PortEndpoint,
        consumer: &ResolvedPortRef,
        phase: BindingPhase,
        candidate: BindingPath,
        binding_locks: &mut Vec<BindingLock>,
        resolved_bindings: &mut Vec<ResolvedBinding>,
    ) -> ModelResult<()> {
        let lock = BindingLock {
            binding_id: binding_id.to_string(),
            provider: provider_endpoint.clone(),
            consumer: consumer_endpoint.clone(),
            provider_component: candidate.provider.component.clone(),
            transport: candidate.transport.clone(),
            phase,
        };
        lock.validate()?;
        binding_locks.push(lock);
        resolved_bindings.push(ResolvedBinding {
            origin_assembly_digest: assembly_digest.to_string(),
            origin_assembly_id: assembly.assembly_id.clone(),
            binding_id: binding_id.to_string(),
            provider_node_path: candidate.provider.leaf_node_path.clone(),
            provider_port_id: candidate.provider.leaf_port_id.clone(),
            consumer_node_path: consumer.leaf_node_path.clone(),
            consumer_port_id: consumer.leaf_port_id.clone(),
            provider_component: candidate.provider.component,
            transport: candidate.transport,
            phase,
        });
        Ok(())
    }

    fn emit_binding_failure(
        &mut self,
        code: DiagnosticCode,
        consumer: &ResolvedPortRef,
        phase: BindingPhase,
        candidates: Vec<ArtifactDescriptor>,
    ) {
        let PortRole::Import { availability, .. } = consumer.descriptor.role else {
            return;
        };
        self.diagnostics
            .unresolved(code, consumer, availability, phase, candidates);
    }
}

fn rebase_occurrence(
    mut occurrence: OccurrenceResult,
    from: &NodePath,
    to: &NodePath,
) -> OccurrenceResult {
    for port in occurrence.external_ports.values_mut() {
        port.leaf_node_path = rebase_node_path(&port.leaf_node_path, from, to);
    }
    for node in &mut occurrence.flattened_nodes {
        node.node_path = rebase_node_path(&node.node_path, from, to);
    }
    for binding in &mut occurrence.bindings {
        binding.provider_node_path = rebase_node_path(&binding.provider_node_path, from, to);
        binding.consumer_node_path = rebase_node_path(&binding.consumer_node_path, from, to);
    }
    for slot in &mut occurrence.state_slots {
        slot.owner_node_path = rebase_node_path(&slot.owner_node_path, from, to);
        slot.migration_node_path = slot
            .migration_node_path
            .as_ref()
            .map(|path| rebase_node_path(path, from, to));
    }
    occurrence
}

fn rebase_node_path(path: &NodePath, from: &NodePath, to: &NodePath) -> NodePath {
    let suffix = path
        .0
        .strip_prefix(from.as_slice())
        .expect("memoized occurrence path must retain its cached prefix");
    let mut rebased = to.0.clone();
    rebased.extend_from_slice(suffix);
    NodePath(rebased)
}

fn candidate_artifacts(paths: &[BindingPath]) -> Vec<ArtifactDescriptor> {
    let mut artifacts = paths
        .iter()
        .map(|path| path.provider.component.clone())
        .collect::<Vec<_>>();
    sort_artifacts(&mut artifacts);
    artifacts
}

fn automatic_binding_id(endpoint: &PortEndpoint, used: &BTreeSet<String>) -> String {
    let base = format!("auto-{}-{}", endpoint.node_id, endpoint.port_id);
    if !used.contains(&base) {
        return base;
    }
    let mut suffix = 2usize;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !used.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn component_protocol_profiles(component: &ComponentDescriptor) -> Vec<ProtocolProfilePin> {
    component
        .protocol_implementations
        .iter()
        .flat_map(|protocol| {
            protocol
                .implementation
                .profiles
                .iter()
                .map(|profile| ProtocolProfilePin {
                    protocol_id: protocol.implementation.protocol_id.clone(),
                    version: protocol.implementation.version.clone(),
                    profile: profile.clone(),
                })
        })
        .collect()
}

fn sort_artifacts(artifacts: &mut Vec<ArtifactDescriptor>) {
    artifacts.sort_by(|left, right| {
        (&left.digest, &left.artifact_type_uri, &left.media_type).cmp(&(
            &right.digest,
            &right.artifact_type_uri,
            &right.media_type,
        ))
    });
    artifacts.dedup_by(|left, right| left.digest == right.digest);
}

fn sort_profiles(profiles: &mut Vec<ProtocolProfilePin>) {
    profiles.sort_by(|left, right| {
        (&left.protocol_id, &left.version, &left.profile).cmp(&(
            &right.protocol_id,
            &right.version,
            &right.profile,
        ))
    });
    profiles.dedup_by(|left, right| left == right);
}

fn complexity_error(message: &'static str) -> ModelError {
    ModelError::new(DiagnosticCode::WorkTooComplex, message)
}

pub fn validate_resolver_containment(
    root: &ArtifactDescriptor,
    assemblies: &BTreeMap<String, AssemblyRevision>,
) -> ModelResult<()> {
    validate_descriptor_type(root, crate::ASSEMBLY_REVISION_TYPE_URI)?;
    let mut stack = vec![(root.clone(), 1usize, Vec::<String>::new())];
    let mut expected_descriptors = BTreeMap::<String, ArtifactDescriptor>::new();
    while let Some((expected, depth, ancestors)) = stack.pop() {
        let digest = expected.digest.clone();
        if depth > MAX_ASSEMBLY_DEPTH {
            return Err(complexity_error("nested Assembly depth exceeds 32"));
        }
        if ancestors.contains(&digest) {
            return Err(ModelError::new(
                DiagnosticCode::AssemblyCycle,
                "nested Assembly containment graph contains a cycle",
            ));
        }
        let assembly = assemblies.get(&digest).ok_or_else(|| {
            ModelError::new(
                DiagnosticCode::ArtifactMissing,
                "Assembly closure is missing a referenced revision",
            )
        })?;
        if expected_descriptors
            .insert(digest.clone(), expected.clone())
            .is_some_and(|previous| previous != expected)
        {
            return Err(ModelError::new(
                DiagnosticCode::ArtifactDigestMismatch,
                "Assembly closure uses inconsistent descriptors for one digest",
            ));
        }
        let mut next_ancestors = ancestors;
        next_ancestors.push(digest);
        let mut children = assembly
            .nodes
            .iter()
            .filter_map(|node| match &node.source {
                AssemblyNodeSource::Assembly { assembly } => Some(assembly.clone()),
                AssemblyNodeSource::Component { .. } => None,
            })
            .collect::<Vec<_>>();
        children.sort_by(|left, right| left.digest.cmp(&right.digest));
        for child in children.into_iter().rev() {
            // Digest memoization intentionally does not skip this occurrence:
            // depth and cycle checks are path-sensitive for shared children.
            stack.push((child, depth + 1, next_ancestors.clone()));
        }
    }
    for (digest, expected) in expected_descriptors {
        let assembly = &assemblies[&digest];
        assembly.validate()?;
        if assembly.artifact_descriptor()? != expected {
            return Err(ModelError::new(
                DiagnosticCode::ArtifactDigestMismatch,
                "Assembly closure descriptor does not match canonical bytes and references",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::{AssemblyBinding, AssemblyNode, AssemblyPortExposure};
    use crate::port::{
        EffectClass, InteractionModelId, PortContract, PortMultiplicity, TransportRequirements,
        INTERACTION_CAPABILITY_STREAM, INTERACTION_CAPABILITY_UNARY,
    };
    use plurora_core::{
        ComponentBoundaryClaims, ComponentClaimStatus, COMPONENT_BEHAVIOR_TYPE_URI,
    };

    fn descriptor(kind: &str, byte: char) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: kind.to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", byte.to_string().repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    fn component(byte: char, id: &str) -> ComponentDescriptor {
        ComponentDescriptor {
            component_id: id.to_string(),
            version: "1.0.0".to_string(),
            artifact: descriptor(COMPONENT_DESCRIPTOR_TYPE_URI, byte),
            behavior: descriptor(COMPONENT_BEHAVIOR_TYPE_URI, byte),
            entry_kind: "subprocess".to_string(),
            trust_class: ComponentTrustClass::IsolatedProcess,
            claim_status: ComponentClaimStatus::Declared,
            enforced_boundaries: ComponentBoundaryClaims::default(),
            capability_ids: Vec::new(),
            protocol_implementations: Vec::new(),
            content_roots: Vec::new(),
            surfaces: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    fn export(id: &str, interaction: &str) -> PortDescriptor {
        PortDescriptor {
            port_id: PortId::parse(id).unwrap(),
            contract: PortContract {
                protocol_id: "example.protocol".to_string(),
                interface_id: "example/interface".to_string(),
                version: "1.0.0".to_string(),
                profiles: vec!["example/default/v1".to_string()],
            },
            interaction: InteractionModelId(interaction.to_string()),
            role: PortRole::Export {
                multiplicity: PortMultiplicity {
                    min: 0,
                    max: Some(8),
                },
                effect_class: EffectClass::ExternalEffecting,
            },
            transport: TransportRequirements::default(),
            annotations: BTreeMap::new(),
        }
    }

    fn import(id: &str, phase: BindingPhase) -> PortDescriptor {
        PortDescriptor {
            port_id: PortId::parse(id).unwrap(),
            contract: PortContract {
                protocol_id: "example.protocol".to_string(),
                interface_id: "example/interface".to_string(),
                version: "^1.0".to_string(),
                profiles: vec!["example/default/v1".to_string()],
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

    fn single_assembly(
        nodes: Vec<(NodeId, ComponentDescriptor, Vec<PortDescriptor>)>,
    ) -> ResolverInput {
        let assembly = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: crate::AssemblyId::parse("example/root").unwrap(),
            nodes: nodes
                .iter()
                .map(|(node_id, component, ports)| AssemblyNode {
                    node_id: node_id.clone(),
                    source: AssemblyNodeSource::Component {
                        component: component.artifact.clone(),
                    },
                    ports: ports.clone(),
                    configuration: None,
                    annotations: BTreeMap::new(),
                })
                .collect(),
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let root = assembly.artifact_descriptor().unwrap();
        let evidence = nodes
            .into_iter()
            .map(|(node_id, component, ports)| {
                (
                    NodeEvidenceKey {
                        assembly_digest: root.digest.clone(),
                        node_id,
                    },
                    NodeEvidence {
                        component,
                        ports,
                        provenance_refs: Vec::new(),
                        artifact_refs: Vec::new(),
                    },
                )
            })
            .collect();
        ResolverInput {
            root_assembly: root.clone(),
            assemblies: BTreeMap::from([(root.digest.clone(), assembly)]),
            node_evidence: evidence,
            content_roots: Vec::new(),
            protocol_profiles: Vec::new(),
        }
    }

    #[test]
    fn zero_one_and_multiple_providers_are_unavailable_resolved_and_ambiguous() {
        let consumer = (
            NodeId::parse("consumer").unwrap(),
            component('a', "same/consumer"),
            vec![import("in", BindingPhase::Installation)],
        );
        let zero = resolve_assembly_at_phase(
            &single_assembly(vec![consumer.clone()]),
            BindingPhase::Installation,
        )
        .unwrap();
        assert_eq!(
            zero.diagnostics.diagnostics[0].code,
            DiagnosticCode::BindingUnavailable
        );
        let provider = (
            NodeId::parse("provider").unwrap(),
            component('b', "same/provider"),
            vec![export("out", INTERACTION_CAPABILITY_UNARY)],
        );
        let one = resolve_assembly_at_phase(
            &single_assembly(vec![consumer.clone(), provider.clone()]),
            BindingPhase::Installation,
        )
        .unwrap();
        assert_eq!(one.bindings.len(), 1);
        let provider_two = (
            NodeId::parse("provider2").unwrap(),
            component('c', "same/provider2"),
            vec![export("out2", INTERACTION_CAPABILITY_UNARY)],
        );
        let many = resolve_assembly_at_phase(
            &single_assembly(vec![consumer, provider, provider_two]),
            BindingPhase::Installation,
        )
        .unwrap();
        assert_eq!(
            many.diagnostics.diagnostics[0].code,
            DiagnosticCode::BindingAmbiguous
        );
    }

    #[test]
    fn binding_phase_controls_portability_without_hiding_later_gaps() {
        for (phase, portable) in [
            (BindingPhase::Authoring, false),
            (BindingPhase::Installation, true),
            (BindingPhase::Launch, true),
            (BindingPhase::Runtime, true),
        ] {
            let input = single_assembly(vec![(
                NodeId::parse("consumer").unwrap(),
                component('a', "example/consumer"),
                vec![import("in", phase)],
            )]);
            let output = resolve_assembly(&input).unwrap();
            assert_eq!(output.portable, portable);
            assert_eq!(output.diagnostics.diagnostics[0].phase, Some(phase));
        }
    }

    #[test]
    fn later_phase_provider_is_not_pinned_during_authoring() {
        let input = single_assembly(vec![
            (
                NodeId::parse("consumer").unwrap(),
                component('a', "example/consumer"),
                vec![import("in", BindingPhase::Installation)],
            ),
            (
                NodeId::parse("provider").unwrap(),
                component('b', "example/provider"),
                vec![export("out", INTERACTION_CAPABILITY_UNARY)],
            ),
        ]);
        let authoring = resolve_assembly(&input).unwrap();
        assert!(authoring.bindings.is_empty());
        assert_eq!(
            authoring.diagnostics.diagnostics[0].code,
            DiagnosticCode::BindingUnavailable
        );
        let installation = resolve_assembly_at_phase(&input, BindingPhase::Installation).unwrap();
        assert_eq!(installation.bindings.len(), 1);
        assert!(installation.diagnostics.diagnostics.is_empty());
    }

    #[test]
    fn optional_and_degraded_imports_remain_visible_without_becoming_required() {
        for (availability, severity) in [
            (AvailabilityPolicy::Optional, DiagnosticSeverity::Info),
            (
                AvailabilityPolicy::DegradedWithout,
                DiagnosticSeverity::Warning,
            ),
        ] {
            let mut port = import("in", BindingPhase::Runtime);
            if let PortRole::Import {
                availability: ref mut value,
                ..
            } = port.role
            {
                *value = availability;
            }
            let output = resolve_assembly(&single_assembly(vec![(
                NodeId::parse("consumer").unwrap(),
                component('a', "example/consumer"),
                vec![port],
            )]))
            .unwrap();
            assert!(output.complete && output.portable);
            assert_eq!(output.diagnostics.diagnostics[0].severity, severity);
        }
    }

    #[test]
    fn adapter_is_an_ordinary_component_with_two_ordinary_bindings() {
        let mut consumer_port = import("in", BindingPhase::Installation);
        consumer_port.interaction = InteractionModelId(INTERACTION_CAPABILITY_STREAM.to_string());
        let mut adapter_input = import("adapter-in", BindingPhase::Installation);
        adapter_input.interaction = InteractionModelId(INTERACTION_CAPABILITY_UNARY.to_string());
        let adapter_output = export("adapter-out", INTERACTION_CAPABILITY_STREAM);
        let input = single_assembly(vec![
            (
                NodeId::parse("consumer").unwrap(),
                component('a', "example/consumer"),
                vec![consumer_port],
            ),
            (
                NodeId::parse("provider").unwrap(),
                component('b', "example/provider"),
                vec![export("out", INTERACTION_CAPABILITY_UNARY)],
            ),
            (
                NodeId::parse("adapter").unwrap(),
                component('c', "example/adapter"),
                vec![adapter_input, adapter_output],
            ),
        ]);
        let resolved = resolve_assembly_at_phase(&input, BindingPhase::Installation).unwrap();
        assert_eq!(resolved.bindings.len(), 2);
        assert_eq!(resolved.root_lock.nodes.len(), 3);
        assert!(resolved
            .root_lock
            .nodes
            .iter()
            .any(|node| node.node_id.as_str() == "adapter"));
        assert!(resolved
            .bindings
            .iter()
            .any(|binding| { binding.consumer_node_path.0.last().unwrap().as_str() == "adapter" }));
        assert!(resolved
            .bindings
            .iter()
            .any(|binding| { binding.provider_node_path.0.last().unwrap().as_str() == "adapter" }));
    }

    #[test]
    fn two_ordinary_adapter_components_remain_ambiguous() {
        let mut consumer_port = import("in", BindingPhase::Installation);
        consumer_port.interaction = InteractionModelId(INTERACTION_CAPABILITY_STREAM.to_string());
        let adapter_ports = || {
            let mut input = import("adapter-in", BindingPhase::Installation);
            input.interaction = InteractionModelId(INTERACTION_CAPABILITY_UNARY.to_string());
            vec![input, export("adapter-out", INTERACTION_CAPABILITY_STREAM)]
        };
        let input = single_assembly(vec![
            (
                NodeId::parse("consumer").unwrap(),
                component('a', "example/consumer"),
                vec![consumer_port],
            ),
            (
                NodeId::parse("provider").unwrap(),
                component('b', "example/provider"),
                vec![export("out", INTERACTION_CAPABILITY_UNARY)],
            ),
            (
                NodeId::parse("adapter-one").unwrap(),
                component('c', "example/adapter-one"),
                adapter_ports(),
            ),
            (
                NodeId::parse("adapter-two").unwrap(),
                component('d', "example/adapter-two"),
                adapter_ports(),
            ),
        ]);
        let ambiguous = resolve_assembly_at_phase(&input, BindingPhase::Installation).unwrap();
        assert!(ambiguous
            .diagnostics
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::BindingAmbiguous));
    }

    #[test]
    fn ordinary_adapter_output_participates_in_cardinality() {
        let mut adapter_input = import("adapter-in", BindingPhase::Installation);
        adapter_input.interaction = InteractionModelId(INTERACTION_CAPABILITY_UNARY.to_string());
        let mut adapter_output = export("adapter-out", INTERACTION_CAPABILITY_STREAM);
        if let PortRole::Export {
            ref mut multiplicity,
            ..
        } = adapter_output.role
        {
            multiplicity.max = Some(1);
        }
        let consumers = ["consumer-one", "consumer-two"].map(|id| {
            let mut port = import("in", BindingPhase::Installation);
            port.interaction = InteractionModelId(INTERACTION_CAPABILITY_STREAM.to_string());
            (NodeId::parse(id).unwrap(), port)
        });
        let input = single_assembly(vec![
            (
                consumers[0].0.clone(),
                component('a', "example/consumer-one"),
                vec![consumers[0].1.clone()],
            ),
            (
                consumers[1].0.clone(),
                component('b', "example/consumer-two"),
                vec![consumers[1].1.clone()],
            ),
            (
                NodeId::parse("provider").unwrap(),
                component('c', "example/provider"),
                vec![export("out", INTERACTION_CAPABILITY_UNARY)],
            ),
            (
                NodeId::parse("adapter").unwrap(),
                component('d', "example/adapter"),
                vec![adapter_input, adapter_output],
            ),
        ]);
        assert_eq!(
            resolve_assembly_at_phase(&input, BindingPhase::Installation)
                .unwrap_err()
                .code,
            DiagnosticCode::PortIncompatible
        );
    }

    #[test]
    fn incompatible_provider_without_an_adapter_is_unavailable() {
        let mut consumer = import("in", BindingPhase::Installation);
        consumer.interaction = InteractionModelId(INTERACTION_CAPABILITY_STREAM.to_string());
        let output = resolve_assembly_at_phase(
            &single_assembly(vec![
                (
                    NodeId::parse("consumer").unwrap(),
                    component('a', "example/consumer"),
                    vec![consumer],
                ),
                (
                    NodeId::parse("provider").unwrap(),
                    component('b', "example/provider"),
                    vec![export("out", INTERACTION_CAPABILITY_UNARY)],
                ),
            ]),
            BindingPhase::Installation,
        )
        .unwrap();
        assert_eq!(
            output.diagnostics.diagnostics[0].code,
            DiagnosticCode::BindingUnavailable
        );
    }

    #[test]
    fn raw_provider_candidate_budget_is_enforced_before_filtering() {
        let mut ports = (0..257)
            .map(|index| export(&format!("out{index}"), INTERACTION_CAPABILITY_UNARY))
            .collect::<Vec<_>>();
        for port in &mut ports {
            port.contract.interface_id = "other/interface".to_string();
        }
        let input = single_assembly(vec![
            (
                NodeId::parse("consumer").unwrap(),
                component('a', "example/consumer"),
                vec![import("in", BindingPhase::Installation)],
            ),
            (
                NodeId::parse("provider").unwrap(),
                component('b', "example/provider"),
                ports,
            ),
        ]);
        assert_eq!(
            resolve_assembly_at_phase(&input, BindingPhase::Installation)
                .unwrap_err()
                .code,
            DiagnosticCode::WorkTooComplex
        );
    }

    #[test]
    fn explicit_binding_is_not_replaced_automatically_and_cardinality_is_strict() {
        let mut wrong = export("wrong", INTERACTION_CAPABILITY_UNARY);
        wrong.contract.interface_id = "other/interface".to_string();
        let mut input = single_assembly(vec![
            (
                NodeId::parse("consumer").unwrap(),
                component('a', "example/consumer"),
                vec![import("in", BindingPhase::Authoring)],
            ),
            (
                NodeId::parse("wrong-provider").unwrap(),
                component('b', "example/wrong"),
                vec![wrong],
            ),
            (
                NodeId::parse("good-provider").unwrap(),
                component('c', "example/good"),
                vec![export("good", INTERACTION_CAPABILITY_UNARY)],
            ),
        ]);
        rewrite_assembly(&mut input, |assembly| {
            assembly.bindings = vec![AssemblyBinding {
                binding_id: "fixed".to_string(),
                provider: PortEndpoint {
                    node_id: NodeId::parse("wrong-provider").unwrap(),
                    port_id: PortId::parse("wrong").unwrap(),
                },
                consumer: PortEndpoint {
                    node_id: NodeId::parse("consumer").unwrap(),
                    port_id: PortId::parse("in").unwrap(),
                },
                phase: BindingPhase::Authoring,
                transport_policy: TransportPolicy::default(),
                annotations: BTreeMap::new(),
            }];
        });
        let output = resolve_assembly(&input).unwrap();
        assert!(output.bindings.is_empty());
        assert_eq!(
            output.diagnostics.diagnostics[0].code,
            DiagnosticCode::PortIncompatible
        );

        let mut cardinality = single_assembly(vec![
            (
                NodeId::parse("consumer").unwrap(),
                component('d', "example/consumer2"),
                vec![import("in", BindingPhase::Authoring)],
            ),
            (
                NodeId::parse("provider1").unwrap(),
                component('e', "example/provider1"),
                vec![export("out", INTERACTION_CAPABILITY_UNARY)],
            ),
            (
                NodeId::parse("provider2").unwrap(),
                component('f', "example/provider2"),
                vec![export("out", INTERACTION_CAPABILITY_UNARY)],
            ),
        ]);
        rewrite_assembly(&mut cardinality, |assembly| {
            assembly.bindings = ["provider1", "provider2"]
                .into_iter()
                .enumerate()
                .map(|(index, provider)| AssemblyBinding {
                    binding_id: format!("fixed{index}"),
                    provider: PortEndpoint {
                        node_id: NodeId::parse(provider).unwrap(),
                        port_id: PortId::parse("out").unwrap(),
                    },
                    consumer: PortEndpoint {
                        node_id: NodeId::parse("consumer").unwrap(),
                        port_id: PortId::parse("in").unwrap(),
                    },
                    phase: BindingPhase::Authoring,
                    transport_policy: TransportPolicy::default(),
                    annotations: BTreeMap::new(),
                })
                .collect();
        });
        assert_eq!(
            resolve_assembly(&cardinality).unwrap_err().code,
            DiagnosticCode::PortIncompatible
        );
    }

    #[test]
    fn nested_exposure_aliases_cannot_bypass_leaf_cardinality() {
        let provider_component = component('a', "example/provider");
        let mut provider_port = export("out", INTERACTION_CAPABILITY_UNARY);
        if let PortRole::Export {
            ref mut multiplicity,
            ..
        } = provider_port.role
        {
            multiplicity.max = Some(1);
        }
        let child = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: crate::AssemblyId::parse("example/child-cardinality").unwrap(),
            nodes: vec![AssemblyNode {
                node_id: NodeId::parse("provider").unwrap(),
                source: AssemblyNodeSource::Component {
                    component: provider_component.artifact.clone(),
                },
                ports: vec![provider_port.clone()],
                configuration: None,
                annotations: BTreeMap::new(),
            }],
            bindings: Vec::new(),
            exposed_ports: ["out-a", "out-b"]
                .into_iter()
                .map(|id| AssemblyPortExposure {
                    port_id: PortId::parse(id).unwrap(),
                    direction: PortDirection::Export,
                    target: PortEndpoint {
                        node_id: NodeId::parse("provider").unwrap(),
                        port_id: PortId::parse("out").unwrap(),
                    },
                    annotations: BTreeMap::new(),
                })
                .collect(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let child_ref = child.artifact_descriptor().unwrap();
        let consumer_one = component('b', "example/consumer-one");
        let consumer_two = component('c', "example/consumer-two");
        let root = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: crate::AssemblyId::parse("example/root-cardinality").unwrap(),
            nodes: vec![
                AssemblyNode {
                    node_id: NodeId::parse("nested").unwrap(),
                    source: AssemblyNodeSource::Assembly {
                        assembly: child_ref.clone(),
                    },
                    ports: Vec::new(),
                    configuration: None,
                    annotations: BTreeMap::new(),
                },
                AssemblyNode {
                    node_id: NodeId::parse("consumer-one").unwrap(),
                    source: AssemblyNodeSource::Component {
                        component: consumer_one.artifact.clone(),
                    },
                    ports: vec![import("in", BindingPhase::Authoring)],
                    configuration: None,
                    annotations: BTreeMap::new(),
                },
                AssemblyNode {
                    node_id: NodeId::parse("consumer-two").unwrap(),
                    source: AssemblyNodeSource::Component {
                        component: consumer_two.artifact.clone(),
                    },
                    ports: vec![import("in", BindingPhase::Authoring)],
                    configuration: None,
                    annotations: BTreeMap::new(),
                },
            ],
            bindings: [
                ("bind-one", "out-a", "consumer-one"),
                ("bind-two", "out-b", "consumer-two"),
            ]
            .into_iter()
            .map(
                |(binding_id, provider_port, consumer_node)| AssemblyBinding {
                    binding_id: binding_id.to_string(),
                    provider: PortEndpoint {
                        node_id: NodeId::parse("nested").unwrap(),
                        port_id: PortId::parse(provider_port).unwrap(),
                    },
                    consumer: PortEndpoint {
                        node_id: NodeId::parse(consumer_node).unwrap(),
                        port_id: PortId::parse("in").unwrap(),
                    },
                    phase: BindingPhase::Authoring,
                    transport_policy: TransportPolicy::default(),
                    annotations: BTreeMap::new(),
                },
            )
            .collect(),
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let root_ref = root.artifact_descriptor().unwrap();
        let input = ResolverInput {
            root_assembly: root_ref.clone(),
            assemblies: BTreeMap::from([
                (root_ref.digest.clone(), root),
                (child_ref.digest.clone(), child),
            ]),
            node_evidence: BTreeMap::from([
                (
                    NodeEvidenceKey {
                        assembly_digest: child_ref.digest,
                        node_id: NodeId::parse("provider").unwrap(),
                    },
                    NodeEvidence {
                        component: provider_component,
                        ports: vec![provider_port],
                        provenance_refs: Vec::new(),
                        artifact_refs: Vec::new(),
                    },
                ),
                (
                    NodeEvidenceKey {
                        assembly_digest: root_ref.digest.clone(),
                        node_id: NodeId::parse("consumer-one").unwrap(),
                    },
                    NodeEvidence {
                        component: consumer_one,
                        ports: vec![import("in", BindingPhase::Authoring)],
                        provenance_refs: Vec::new(),
                        artifact_refs: Vec::new(),
                    },
                ),
                (
                    NodeEvidenceKey {
                        assembly_digest: root_ref.digest,
                        node_id: NodeId::parse("consumer-two").unwrap(),
                    },
                    NodeEvidence {
                        component: consumer_two,
                        ports: vec![import("in", BindingPhase::Authoring)],
                        provenance_refs: Vec::new(),
                        artifact_refs: Vec::new(),
                    },
                ),
            ]),
            content_roots: Vec::new(),
            protocol_profiles: Vec::new(),
        };
        assert_eq!(
            resolve_assembly(&input).unwrap_err().code,
            DiagnosticCode::PortIncompatible
        );
    }

    #[test]
    fn export_minimum_is_enforced_unless_the_port_is_exposed_outward() {
        let mut provider = export("out", INTERACTION_CAPABILITY_UNARY);
        let PortRole::Export {
            ref mut multiplicity,
            ..
        } = provider.role
        else {
            unreachable!()
        };
        multiplicity.min = 1;
        let mut input = single_assembly(vec![(
            NodeId::parse("provider").unwrap(),
            component('a', "example/provider"),
            vec![provider],
        )]);
        assert_eq!(
            resolve_assembly(&input).unwrap_err().code,
            DiagnosticCode::PortIncompatible
        );

        rewrite_assembly(&mut input, |assembly| {
            assembly.exposed_ports = vec![AssemblyPortExposure {
                port_id: PortId::parse("public-out").unwrap(),
                direction: PortDirection::Export,
                target: PortEndpoint {
                    node_id: NodeId::parse("provider").unwrap(),
                    port_id: PortId::parse("out").unwrap(),
                },
                annotations: BTreeMap::new(),
            }];
        });
        let output = resolve_assembly(&input).unwrap();
        assert!(output.bindings.is_empty());
        assert_eq!(output.exposed_ports.len(), 1);
    }

    #[test]
    fn diagnostics_are_bounded_and_report_omissions() {
        let ports = (0..1024)
            .map(|index| {
                let mut port = import(&format!("in{index}"), BindingPhase::Runtime);
                if let PortRole::Import {
                    ref mut availability,
                    ..
                } = port.role
                {
                    *availability = AvailabilityPolicy::Optional;
                }
                port
            })
            .collect();
        let output = resolve_assembly(&single_assembly(vec![(
            NodeId::parse("consumer").unwrap(),
            component('a', "example/consumer"),
            ports,
        )]))
        .unwrap();
        assert_eq!(
            output.diagnostics.diagnostics.len(),
            MAX_RESOLVER_DIAGNOSTICS
        );
        assert_eq!(output.diagnostics.omitted_count, 24);
    }

    #[test]
    fn nested_reexport_preserves_exposure_chain_and_lock_dag() {
        let leaf_component = component('a', "example/leaf");
        let child = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: crate::AssemblyId::parse("example/child").unwrap(),
            nodes: vec![AssemblyNode {
                node_id: NodeId::parse("leaf").unwrap(),
                source: AssemblyNodeSource::Component {
                    component: leaf_component.artifact.clone(),
                },
                ports: vec![export("out", INTERACTION_CAPABILITY_UNARY)],
                configuration: None,
                annotations: BTreeMap::new(),
            }],
            bindings: Vec::new(),
            exposed_ports: vec![AssemblyPortExposure {
                port_id: PortId::parse("child-out").unwrap(),
                direction: PortDirection::Export,
                target: PortEndpoint {
                    node_id: NodeId::parse("leaf").unwrap(),
                    port_id: PortId::parse("out").unwrap(),
                },
                annotations: BTreeMap::new(),
            }],
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let child_ref = child.artifact_descriptor().unwrap();
        let root = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: crate::AssemblyId::parse("example/root").unwrap(),
            nodes: vec![AssemblyNode {
                node_id: NodeId::parse("nested").unwrap(),
                source: AssemblyNodeSource::Assembly {
                    assembly: child_ref.clone(),
                },
                ports: Vec::new(),
                configuration: None,
                annotations: BTreeMap::new(),
            }],
            bindings: Vec::new(),
            exposed_ports: vec![AssemblyPortExposure {
                port_id: PortId::parse("root-out").unwrap(),
                direction: PortDirection::Export,
                target: PortEndpoint {
                    node_id: NodeId::parse("nested").unwrap(),
                    port_id: PortId::parse("child-out").unwrap(),
                },
                annotations: BTreeMap::new(),
            }],
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let root_ref = root.artifact_descriptor().unwrap();
        let input = ResolverInput {
            root_assembly: root_ref.clone(),
            assemblies: BTreeMap::from([
                (root_ref.digest.clone(), root),
                (child_ref.digest.clone(), child),
            ]),
            node_evidence: BTreeMap::from([(
                NodeEvidenceKey {
                    assembly_digest: child_ref.digest.clone(),
                    node_id: NodeId::parse("leaf").unwrap(),
                },
                NodeEvidence {
                    component: leaf_component,
                    ports: vec![export("out", INTERACTION_CAPABILITY_UNARY)],
                    provenance_refs: Vec::new(),
                    artifact_refs: Vec::new(),
                },
            )]),
            content_roots: vec![
                descriptor("urn:example:content:v1", 'd'),
                descriptor("urn:example:content:v1", 'c'),
                descriptor("urn:example:content:v1", 'c'),
            ],
            protocol_profiles: vec![
                ProtocolProfilePin {
                    protocol_id: "z.protocol".to_string(),
                    version: "1.0.0".to_string(),
                    profile: "z/default/v1".to_string(),
                },
                ProtocolProfilePin {
                    protocol_id: "a.protocol".to_string(),
                    version: "1.0.0".to_string(),
                    profile: "a/default/v1".to_string(),
                },
                ProtocolProfilePin {
                    protocol_id: "a.protocol".to_string(),
                    version: "1.0.0".to_string(),
                    profile: "a/default/v1".to_string(),
                },
            ],
        };
        let output = resolve_assembly(&input).unwrap();
        assert_eq!(output.exposed_ports[0].exposure_chain.len(), 2);
        assert_eq!(output.flattened_nodes[0].exposure_chain.len(), 2);
        assert_eq!(output.root_lock.content_roots.len(), 2);
        assert!(
            output.root_lock.content_roots[0].digest < output.root_lock.content_roots[1].digest
        );
        assert_eq!(output.root_lock.protocol_profiles.len(), 2);
        assert_eq!(
            output.root_lock.protocol_profiles[0].protocol_id,
            "a.protocol"
        );
        assert_eq!(
            output.root_lock.nodes[0].artifact.artifact_type_uri,
            crate::ASSEMBLY_LOCK_TYPE_URI
        );
        assert!(output
            .root_lock_artifact
            .descriptor
            .references
            .contains(&output.root_lock.nodes[0].artifact.digest));
        assert_eq!(
            resolve_assembly(&input)
                .unwrap()
                .root_lock_artifact
                .descriptor
                .digest,
            output.root_lock_artifact.descriptor.digest
        );
    }

    #[test]
    fn shared_child_digest_is_rebased_per_occurrence_and_pinned_once() {
        let leaf_component = component('a', "example/shared-leaf");
        let child = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: crate::AssemblyId::parse("example/shared-child").unwrap(),
            nodes: vec![AssemblyNode {
                node_id: NodeId::parse("leaf").unwrap(),
                source: AssemblyNodeSource::Component {
                    component: leaf_component.artifact.clone(),
                },
                ports: Vec::new(),
                configuration: None,
                annotations: BTreeMap::new(),
            }],
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let child_ref = child.artifact_descriptor().unwrap();
        let root = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: crate::AssemblyId::parse("example/shared-root").unwrap(),
            nodes: ["left", "right"]
                .into_iter()
                .map(|node_id| AssemblyNode {
                    node_id: NodeId::parse(node_id).unwrap(),
                    source: AssemblyNodeSource::Assembly {
                        assembly: child_ref.clone(),
                    },
                    ports: Vec::new(),
                    configuration: None,
                    annotations: BTreeMap::new(),
                })
                .collect(),
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let root_ref = root.artifact_descriptor().unwrap();
        let output = resolve_assembly(&ResolverInput {
            root_assembly: root_ref.clone(),
            assemblies: BTreeMap::from([
                (root_ref.digest.clone(), root),
                (child_ref.digest.clone(), child),
            ]),
            node_evidence: BTreeMap::from([(
                NodeEvidenceKey {
                    assembly_digest: child_ref.digest,
                    node_id: NodeId::parse("leaf").unwrap(),
                },
                NodeEvidence {
                    component: leaf_component,
                    ports: Vec::new(),
                    provenance_refs: Vec::new(),
                    artifact_refs: Vec::new(),
                },
            )]),
            content_roots: Vec::new(),
            protocol_profiles: Vec::new(),
        })
        .unwrap();

        assert_eq!(output.flattened_nodes.len(), 2);
        assert_eq!(
            output.flattened_nodes[0]
                .node_path
                .as_slice()
                .iter()
                .map(NodeId::as_str)
                .collect::<Vec<_>>(),
            vec!["left", "leaf"]
        );
        assert_eq!(
            output.flattened_nodes[1]
                .node_path
                .as_slice()
                .iter()
                .map(NodeId::as_str)
                .collect::<Vec<_>>(),
            vec!["right", "leaf"]
        );
        assert_eq!(output.lock_artifacts.len(), 2);
        assert_eq!(
            output.root_lock.nodes[0].artifact,
            output.root_lock.nodes[1].artifact
        );
    }

    #[test]
    fn cycle_depth_and_shared_child_paths_are_checked_per_occurrence() {
        let fake_a = descriptor(crate::ASSEMBLY_REVISION_TYPE_URI, 'a');
        let fake_b = descriptor(crate::ASSEMBLY_REVISION_TYPE_URI, 'b');
        let cycle = BTreeMap::from([
            (
                fake_a.digest.clone(),
                fake_nested("example/a", vec![fake_b.clone()]),
            ),
            (
                fake_b.digest.clone(),
                fake_nested("example/b", vec![fake_a.clone()]),
            ),
        ]);
        assert_eq!(
            validate_resolver_containment(&fake_a, &cycle)
                .unwrap_err()
                .code,
            DiagnosticCode::AssemblyCycle
        );

        let mut closure = BTreeMap::new();
        let refs = (0..33)
            .map(|index| {
                descriptor(
                    crate::ASSEMBLY_REVISION_TYPE_URI,
                    char::from(b'a' + (index % 26) as u8),
                )
            })
            .collect::<Vec<_>>();
        // Make digests unique beyond the alphabet rollover.
        let refs = refs
            .into_iter()
            .enumerate()
            .map(|(index, mut descriptor)| {
                descriptor.digest = format!("sha256:{index:064x}");
                descriptor
            })
            .collect::<Vec<_>>();
        for index in 0..refs.len() {
            closure.insert(
                refs[index].digest.clone(),
                fake_nested(
                    &format!("example/a{index}"),
                    refs.get(index + 1).cloned().into_iter().collect(),
                ),
            );
        }
        assert_eq!(
            validate_resolver_containment(&refs[0], &closure)
                .unwrap_err()
                .code,
            DiagnosticCode::WorkTooComplex
        );

        let shared = refs[32].clone();
        let shallow = fake_nested("example/shallow", vec![shared.clone()]);
        let shallow_ref = descriptor(crate::ASSEMBLY_REVISION_TYPE_URI, 'x');
        let mut graph = BTreeMap::from([(shallow_ref.digest.clone(), shallow)]);
        graph.insert(
            shared.digest.clone(),
            fake_nested("example/shared", Vec::new()),
        );
        // A shared digest is traversed for every path; the simple shallow occurrence remains valid until canonical verification.
        assert_ne!(
            validate_resolver_containment(&shallow_ref, &graph)
                .unwrap_err()
                .code,
            DiagnosticCode::AssemblyCycle
        );

        let shallow_ref = ArtifactDescriptor {
            digest: format!("sha256:{:064x}", 1),
            ..descriptor(crate::ASSEMBLY_REVISION_TYPE_URI, 'a')
        };
        let deep_ref = ArtifactDescriptor {
            digest: format!("sha256:{:064x}", 2),
            ..descriptor(crate::ASSEMBLY_REVISION_TYPE_URI, 'b')
        };
        let shared_ref = ArtifactDescriptor {
            digest: format!("sha256:{:064x}", 3),
            ..descriptor(crate::ASSEMBLY_REVISION_TYPE_URI, 'c')
        };
        let root_ref = ArtifactDescriptor {
            digest: format!("sha256:{:064x}", 4),
            ..descriptor(crate::ASSEMBLY_REVISION_TYPE_URI, 'd')
        };
        let mut shared_graph = BTreeMap::from([
            (
                root_ref.digest.clone(),
                fake_nested(
                    "example/root-shared",
                    vec![shallow_ref.clone(), deep_ref.clone()],
                ),
            ),
            (
                shallow_ref.digest.clone(),
                fake_nested("example/shallow-shared", vec![shared_ref.clone()]),
            ),
            (
                shared_ref.digest.clone(),
                fake_nested("example/shared-leaf", Vec::new()),
            ),
        ]);
        let mut previous = deep_ref;
        for index in 0..31 {
            let next = if index == 30 {
                shared_ref.clone()
            } else {
                ArtifactDescriptor {
                    digest: format!("sha256:{:064x}", 10 + index),
                    ..descriptor(crate::ASSEMBLY_REVISION_TYPE_URI, 'e')
                }
            };
            shared_graph.insert(
                previous.digest.clone(),
                fake_nested(&format!("example/deep{index}"), vec![next.clone()]),
            );
            previous = next;
        }
        assert_eq!(
            validate_resolver_containment(&root_ref, &shared_graph)
                .unwrap_err()
                .code,
            DiagnosticCode::WorkTooComplex
        );
    }

    #[test]
    fn durable_state_replacement_requires_migration_or_explicit_reset() {
        let current = StateSlotDescriptor {
            state_slot_id: StateSlotId::parse("save").unwrap(),
            owner_node_id: NodeId::parse("owner").unwrap(),
            schema_ref: Some(descriptor("urn:example:schema:v1", 'a')),
            scope: crate::StateScope::User,
            portability: crate::StatePortability::Portable,
            migration_port: None,
            backup_policy: crate::BackupPolicy::Required,
            annotations: BTreeMap::new(),
        };
        let mut candidate = current.clone();
        candidate.schema_ref = Some(descriptor("urn:example:schema:v1", 'b'));
        assert_eq!(
            crate::validate_state_replacement(&current, &candidate, false)
                .unwrap_err()
                .code,
            DiagnosticCode::StateMigrationRequired
        );
        candidate.migration_port = Some(PortEndpoint {
            node_id: NodeId::parse("owner").unwrap(),
            port_id: PortId::parse("migrate").unwrap(),
        });
        crate::validate_state_replacement(&current, &candidate, false).unwrap();
    }

    #[test]
    fn migration_endpoint_requires_an_executable_interaction() {
        let mut migration = export("migrate", "example.interaction/unknown/v1");
        migration.contract.interface_id = "example/state-migration".to_string();
        let mut input = single_assembly(vec![(
            NodeId::parse("owner").unwrap(),
            component('a', "example/owner"),
            vec![migration],
        )]);
        rewrite_assembly(&mut input, |assembly| {
            assembly.state_slots = vec![StateSlotDescriptor {
                state_slot_id: StateSlotId::parse("save").unwrap(),
                owner_node_id: NodeId::parse("owner").unwrap(),
                schema_ref: Some(descriptor("urn:example:schema:v1", 'b')),
                scope: crate::StateScope::User,
                portability: crate::StatePortability::Portable,
                migration_port: Some(PortEndpoint {
                    node_id: NodeId::parse("owner").unwrap(),
                    port_id: PortId::parse("migrate").unwrap(),
                }),
                backup_policy: crate::BackupPolicy::Required,
                annotations: BTreeMap::new(),
            }];
        });
        assert_eq!(
            resolve_assembly(&input).unwrap_err().code,
            DiagnosticCode::StateMigrationRequired
        );
        let interaction = InteractionModelId(INTERACTION_CAPABILITY_UNARY.to_string());
        input.node_evidence.values_mut().next().unwrap().ports[0].interaction = interaction.clone();
        rewrite_assembly(&mut input, |assembly| {
            assembly.nodes[0].ports[0].interaction = interaction;
        });
        resolve_assembly(&input).unwrap();
    }

    #[test]
    fn containment_requires_the_complete_canonical_descriptor() {
        let mut input = single_assembly(Vec::new());
        input.root_assembly.size_bytes += 1;
        assert_eq!(
            validate_resolver_containment(&input.root_assembly, &input.assemblies)
                .unwrap_err()
                .code,
            DiagnosticCode::ArtifactDigestMismatch
        );
    }

    #[test]
    fn canonical_port_changes_also_change_the_resolved_lock_digest() {
        let mut input = single_assembly(vec![(
            NodeId::parse("provider").unwrap(),
            component('a', "example/provider"),
            vec![export("out", INTERACTION_CAPABILITY_UNARY)],
        )]);
        let first_assembly = input.root_assembly.digest.clone();
        let first_lock = resolve_assembly(&input)
            .unwrap()
            .root_lock_artifact
            .descriptor
            .digest;
        input.node_evidence.values_mut().next().unwrap().ports[0]
            .contract
            .version = "2.0.0".to_string();
        rewrite_assembly(&mut input, |assembly| {
            assembly.nodes[0].ports[0].contract.version = "2.0.0".to_string();
        });
        let second_lock = resolve_assembly(&input)
            .unwrap()
            .root_lock_artifact
            .descriptor
            .digest;
        assert_ne!(input.root_assembly.digest, first_assembly);
        assert_ne!(second_lock, first_lock);
    }

    #[test]
    fn per_assembly_budgets_do_not_become_an_undocumented_global_occurrence_cap() {
        let child = fake_nested("example/shared-budget-child", Vec::new());
        let child_ref = child.artifact_descriptor().unwrap();
        let root = fake_nested("example/shared-budget-root", vec![child_ref.clone(); 2_049]);
        let root_ref = root.artifact_descriptor().unwrap();
        let closure = BTreeMap::from([
            (root_ref.digest.clone(), root),
            (child_ref.digest.clone(), child),
        ]);
        validate_resolver_containment(&root_ref, &closure).unwrap();
    }

    fn rewrite_assembly(input: &mut ResolverInput, update: impl FnOnce(&mut AssemblyRevision)) {
        let old_digest = input.root_assembly.digest.clone();
        let mut assembly = input.assemblies.remove(&old_digest).unwrap();
        update(&mut assembly);
        let new_ref = assembly.artifact_descriptor().unwrap();
        let evidence = std::mem::take(&mut input.node_evidence)
            .into_iter()
            .map(|(mut key, value)| {
                key.assembly_digest = new_ref.digest.clone();
                (key, value)
            })
            .collect();
        input.root_assembly = new_ref.clone();
        input.assemblies = BTreeMap::from([(new_ref.digest.clone(), assembly)]);
        input.node_evidence = evidence;
    }

    fn fake_nested(id: &str, children: Vec<ArtifactDescriptor>) -> AssemblyRevision {
        AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: crate::AssemblyId::parse(id).unwrap(),
            nodes: children
                .into_iter()
                .enumerate()
                .map(|(index, assembly)| AssemblyNode {
                    node_id: NodeId::parse(format!("n{index}")).unwrap(),
                    source: AssemblyNodeSource::Assembly { assembly },
                    ports: Vec::new(),
                    configuration: None,
                    annotations: BTreeMap::new(),
                })
                .collect(),
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }
}
