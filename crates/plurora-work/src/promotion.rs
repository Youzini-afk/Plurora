use std::collections::{BTreeMap, BTreeSet};

use plurora_core::ArtifactDescriptor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::assembly::{
    validate_assembly_closure, AssemblyNodeSource, AssemblyPortExposure, AssemblyRevision,
    ASSEMBLY_REVISION_TYPE_URI,
};
use crate::canonical::{
    fixed_string_schema, validate_descriptor_type, validate_portable_model, ArtifactModel,
};
use crate::diagnostic::{DiagnosticCode, ModelError, ModelResult};
use crate::ids::{validate_local_id, AssemblyId, NodeId, PortId, StateSlotId};
use crate::package::CanonicalArtifactObject;
use crate::port::{BindingPhase, PortDescriptor, PortDirection, PortEndpoint};

pub const ASSEMBLY_PROMOTION_CANDIDATE_TYPE_URI: &str =
    "urn:plurora:assembly-promotion-candidate:v1";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PromotedBoundaryPort {
    pub port_id: PortId,
    pub direction: PortDirection,
    pub target: PortEndpoint,
    pub descriptor: PortDescriptor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_binding_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_endpoint: Option<PortEndpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<BindingPhase>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromotionDiagnosticKind {
    ExistingBoundary,
    CutImport,
    CutExport,
    IncludedState,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AssemblyPromotionDiagnostic {
    pub kind: PromotionDiagnosticKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<NodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_id: Option<PortId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_slot_id: Option<StateSlotId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AssemblyPromotionCandidate {
    #[schemars(schema_with = "promotion_candidate_schema")]
    pub schema: String,
    pub source_assembly: ArtifactDescriptor,
    pub selected_nodes: Vec<NodeId>,
    pub nested_assembly: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boundary_ports: Vec<PromotedBoundaryPort>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_slot_ids: Vec<StateSlotId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<AssemblyPromotionDiagnostic>,
}

fn promotion_candidate_schema(
    _generator: &mut schemars::gen::SchemaGenerator,
) -> schemars::schema::Schema {
    fixed_string_schema(AssemblyPromotionCandidate::SCHEMA)
}

impl AssemblyPromotionCandidate {
    pub const SCHEMA: &'static str = "plurora.assembly-promotion-candidate.v1";

    pub fn validate(&self) -> ModelResult<()> {
        if self.schema != Self::SCHEMA || self.selected_nodes.is_empty() {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "Assembly promotion candidate schema or selection is invalid",
            ));
        }
        validate_descriptor_type(&self.source_assembly, ASSEMBLY_REVISION_TYPE_URI)?;
        validate_descriptor_type(&self.nested_assembly, ASSEMBLY_REVISION_TYPE_URI)?;
        if !is_sorted_unique(&self.selected_nodes)
            || !is_sorted_unique(&self.state_slot_ids)
            || !is_sorted_unique_by(&self.boundary_ports, |port| &port.port_id)
        {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "Assembly promotion candidate collections are not canonical",
            ));
        }
        for boundary in &self.boundary_ports {
            boundary.descriptor.validate()?;
            if boundary.port_id != boundary.descriptor.port_id {
                return Err(ModelError::new(
                    DiagnosticCode::PortIncompatible,
                    "promoted boundary id differs from its Port descriptor",
                ));
            }
            if let Some(binding_id) = &boundary.source_binding_id {
                validate_local_id(binding_id, "promotion source binding id")?;
            }
        }
        validate_portable_model(self)
    }
}

impl ArtifactModel for AssemblyPromotionCandidate {
    const ARTIFACT_TYPE_URI: &'static str = ASSEMBLY_PROMOTION_CANDIDATE_TYPE_URI;

    fn validate(&self) -> ModelResult<()> {
        AssemblyPromotionCandidate::validate(self)
    }

    fn referenced_artifacts(&self) -> Vec<&ArtifactDescriptor> {
        vec![&self.source_assembly, &self.nested_assembly]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyPromotionOutput {
    pub candidate: AssemblyPromotionCandidate,
    pub candidate_artifact: CanonicalArtifactObject,
    pub nested_assembly: AssemblyRevision,
    pub nested_assembly_artifact: CanonicalArtifactObject,
}

pub fn promote_assembly_subgraph(
    source_assembly: &ArtifactDescriptor,
    assemblies: &BTreeMap<String, AssemblyRevision>,
    selected_nodes: &[NodeId],
    promoted_assembly_id: AssemblyId,
) -> ModelResult<AssemblyPromotionOutput> {
    validate_descriptor_type(source_assembly, ASSEMBLY_REVISION_TYPE_URI)?;
    validate_assembly_closure(source_assembly, assemblies)?;
    let source = assemblies.get(&source_assembly.digest).ok_or_else(|| {
        ModelError::new(
            DiagnosticCode::ArtifactMissing,
            "promotion source Assembly is absent from the verified closure",
        )
    })?;
    if source.artifact_descriptor()? != *source_assembly {
        return Err(ModelError::new(
            DiagnosticCode::ArtifactDigestMismatch,
            "promotion source descriptor differs from canonical Assembly bytes",
        ));
    }
    if selected_nodes.is_empty() {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            "promotion requires at least one selected Assembly node",
        ));
    }

    let mut selected = selected_nodes.to_vec();
    selected.sort();
    if selected.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            "promotion selection contains a duplicate node",
        ));
    }
    let source_node_ids = source
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<BTreeSet<_>>();
    if selected.iter().any(|node| !source_node_ids.contains(node)) {
        return Err(ModelError::new(
            DiagnosticCode::PortUnresolved,
            "promotion selection references an unknown Assembly node",
        ));
    }
    let selected_set = selected.iter().cloned().collect::<BTreeSet<_>>();

    let mut catalog_cache = BTreeMap::new();
    let source_catalog = local_port_catalog(source_assembly, assemblies, &mut catalog_cache)?;
    let selected_catalog = source_catalog
        .iter()
        .filter(|(endpoint, _)| selected_set.contains(&endpoint.node_id))
        .map(|(endpoint, descriptor)| (endpoint.clone(), descriptor.clone()))
        .collect::<BTreeMap<_, _>>();

    let nodes = source
        .nodes
        .iter()
        .filter(|node| selected_set.contains(&node.node_id))
        .cloned()
        .collect::<Vec<_>>();
    let bindings = source
        .bindings
        .iter()
        .filter(|binding| {
            selected_set.contains(&binding.provider.node_id)
                && selected_set.contains(&binding.consumer.node_id)
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut exposed_ports = Vec::new();
    let mut boundary_ports = Vec::new();
    let mut diagnostics = Vec::new();
    let mut boundary_ids = BTreeSet::new();
    for exposure in source
        .exposed_ports
        .iter()
        .filter(|exposure| selected_set.contains(&exposure.target.node_id))
    {
        let descriptor = source_catalog.get(&exposure.target).ok_or_else(|| {
            ModelError::new(
                DiagnosticCode::PortUnresolved,
                "promotion source exposure has no verified Port descriptor",
            )
        })?;
        if !boundary_ids.insert(exposure.port_id.clone()) {
            return Err(boundary_collision());
        }
        exposed_ports.push(exposure.clone());
        boundary_ports.push(PromotedBoundaryPort {
            port_id: exposure.port_id.clone(),
            direction: exposure.direction,
            target: exposure.target.clone(),
            descriptor: descriptor_with_port_id(descriptor, &exposure.port_id),
            source_binding_id: None,
            external_endpoint: None,
            phase: None,
        });
        diagnostics.push(boundary_diagnostic(
            PromotionDiagnosticKind::ExistingBoundary,
            "existing Assembly boundary is preserved by the promotion candidate",
            &exposure.target,
            &exposure.port_id,
        ));
    }

    for binding in &source.bindings {
        let provider_selected = selected_set.contains(&binding.provider.node_id);
        let consumer_selected = selected_set.contains(&binding.consumer.node_id);
        let (direction, target, external, kind, message) =
            match (provider_selected, consumer_selected) {
                (true, false) => (
                    PortDirection::Export,
                    &binding.provider,
                    &binding.consumer,
                    PromotionDiagnosticKind::CutExport,
                    "outgoing binding is promoted to an explicit export boundary",
                ),
                (false, true) => (
                    PortDirection::Import,
                    &binding.consumer,
                    &binding.provider,
                    PromotionDiagnosticKind::CutImport,
                    "incoming binding is promoted to an explicit import boundary",
                ),
                _ => continue,
            };
        let port_id = PortId::parse(binding.binding_id.clone())?;
        if !boundary_ids.insert(port_id.clone()) {
            return Err(boundary_collision());
        }
        let descriptor = source_catalog.get(target).ok_or_else(|| {
            ModelError::new(
                DiagnosticCode::PortUnresolved,
                "cut binding endpoint has no verified Port descriptor",
            )
        })?;
        exposed_ports.push(AssemblyPortExposure {
            port_id: port_id.clone(),
            direction,
            target: target.clone(),
            annotations: BTreeMap::from([(
                "plurora.promotion/source_binding".to_string(),
                Value::String(binding.binding_id.clone()),
            )]),
        });
        boundary_ports.push(PromotedBoundaryPort {
            port_id: port_id.clone(),
            direction,
            target: target.clone(),
            descriptor: descriptor_with_port_id(descriptor, &port_id),
            source_binding_id: Some(binding.binding_id.clone()),
            external_endpoint: Some(external.clone()),
            phase: Some(binding.phase),
        });
        diagnostics.push(boundary_diagnostic(kind, message, target, &port_id));
    }

    exposed_ports.sort_by(|left, right| left.port_id.cmp(&right.port_id));
    boundary_ports.sort_by(|left, right| left.port_id.cmp(&right.port_id));

    let mut state_slots = Vec::new();
    let mut state_slot_ids = Vec::new();
    for slot in source
        .state_slots
        .iter()
        .filter(|slot| selected_set.contains(&slot.owner_node_id))
    {
        if slot
            .migration_port
            .as_ref()
            .is_some_and(|endpoint| !selected_set.contains(&endpoint.node_id))
        {
            return Err(ModelError::new(
                DiagnosticCode::StateMigrationRequired,
                "selected durable state has a migration Port outside the promoted subgraph",
            ));
        }
        state_slots.push(slot.clone());
        state_slot_ids.push(slot.state_slot_id.clone());
        diagnostics.push(AssemblyPromotionDiagnostic {
            kind: PromotionDiagnosticKind::IncludedState,
            message: "state owned by the selected subgraph is included in the candidate"
                .to_string(),
            node_id: Some(slot.owner_node_id.clone()),
            port_id: slot
                .migration_port
                .as_ref()
                .map(|endpoint| endpoint.port_id.clone()),
            state_slot_id: Some(slot.state_slot_id.clone()),
        });
    }
    state_slot_ids.sort();

    let nested_assembly = AssemblyRevision {
        schema: AssemblyRevision::SCHEMA.to_string(),
        assembly_id: promoted_assembly_id,
        nodes,
        bindings,
        exposed_ports,
        state_slots,
        annotations: BTreeMap::from([(
            "plurora.promotion/source_assembly".to_string(),
            Value::String(source_assembly.digest.clone()),
        )]),
    };
    nested_assembly.validate_ports(&selected_catalog)?;
    let nested_assembly_artifact = CanonicalArtifactObject::from_model(&nested_assembly)?;
    let candidate = AssemblyPromotionCandidate {
        schema: AssemblyPromotionCandidate::SCHEMA.to_string(),
        source_assembly: source_assembly.clone(),
        selected_nodes: selected,
        nested_assembly: nested_assembly_artifact.descriptor.clone(),
        boundary_ports,
        state_slot_ids,
        diagnostics,
    };
    candidate.validate()?;
    let candidate_artifact = CanonicalArtifactObject::from_model(&candidate)?;
    Ok(AssemblyPromotionOutput {
        candidate,
        candidate_artifact,
        nested_assembly,
        nested_assembly_artifact,
    })
}

fn local_port_catalog(
    assembly: &ArtifactDescriptor,
    assemblies: &BTreeMap<String, AssemblyRevision>,
    cache: &mut BTreeMap<String, BTreeMap<PortEndpoint, PortDescriptor>>,
) -> ModelResult<BTreeMap<PortEndpoint, PortDescriptor>> {
    if let Some(cached) = cache.get(&assembly.digest) {
        return Ok(cached.clone());
    }
    let value = assemblies.get(&assembly.digest).ok_or_else(|| {
        ModelError::new(
            DiagnosticCode::ArtifactMissing,
            "promotion Assembly closure is incomplete",
        )
    })?;
    let mut catalog = BTreeMap::new();
    for node in &value.nodes {
        match &node.source {
            AssemblyNodeSource::Component { .. } => {
                for port in &node.ports {
                    let endpoint = PortEndpoint {
                        node_id: node.node_id.clone(),
                        port_id: port.port_id.clone(),
                    };
                    if catalog.insert(endpoint, port.clone()).is_some() {
                        return Err(ModelError::new(
                            DiagnosticCode::WorkInvalid,
                            "promotion Port catalog contains a duplicate endpoint",
                        ));
                    }
                }
            }
            AssemblyNodeSource::Assembly { assembly } => {
                let child_catalog = local_port_catalog(assembly, assemblies, cache)?;
                let child = assemblies.get(&assembly.digest).ok_or_else(|| {
                    ModelError::new(
                        DiagnosticCode::ArtifactMissing,
                        "nested promotion Assembly is absent from the closure",
                    )
                })?;
                for exposure in &child.exposed_ports {
                    let descriptor = child_catalog.get(&exposure.target).ok_or_else(|| {
                        ModelError::new(
                            DiagnosticCode::PortUnresolved,
                            "nested promotion exposure has no Port descriptor",
                        )
                    })?;
                    let endpoint = PortEndpoint {
                        node_id: node.node_id.clone(),
                        port_id: exposure.port_id.clone(),
                    };
                    if catalog
                        .insert(
                            endpoint,
                            descriptor_with_port_id(descriptor, &exposure.port_id),
                        )
                        .is_some()
                    {
                        return Err(ModelError::new(
                            DiagnosticCode::WorkInvalid,
                            "nested promotion Port catalog contains a duplicate endpoint",
                        ));
                    }
                }
            }
        }
    }
    value.validate_ports(&catalog)?;
    cache.insert(assembly.digest.clone(), catalog.clone());
    Ok(catalog)
}

fn descriptor_with_port_id(descriptor: &PortDescriptor, port_id: &PortId) -> PortDescriptor {
    let mut value = descriptor.clone();
    value.port_id = port_id.clone();
    value
}

fn boundary_diagnostic(
    kind: PromotionDiagnosticKind,
    message: &str,
    target: &PortEndpoint,
    port_id: &PortId,
) -> AssemblyPromotionDiagnostic {
    AssemblyPromotionDiagnostic {
        kind,
        message: message.to_string(),
        node_id: Some(target.node_id.clone()),
        port_id: Some(port_id.clone()),
        state_slot_id: None,
    }
}

fn boundary_collision() -> ModelError {
    ModelError::new(
        DiagnosticCode::BindingAmbiguous,
        "promoted boundary Port ids collide; rename the source binding or exposure explicitly",
    )
}

fn is_sorted_unique<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn is_sorted_unique_by<T, K: Ord>(values: &[T], key: impl Fn(&T) -> &K) -> bool {
    values.windows(2).all(|pair| key(&pair[0]) < key(&pair[1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AvailabilityPolicy, BackupPolicy, EffectClass, InteractionModelId, PortContract,
        PortMultiplicity, PortRole, StatePortability, StateScope, StateSlotDescriptor,
        TransportPolicy, TransportRequirements, INTERACTION_CAPABILITY_UNARY,
    };
    use plurora_core::COMPONENT_DESCRIPTOR_TYPE_URI;

    fn component(byte: char) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: COMPONENT_DESCRIPTOR_TYPE_URI.to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", byte.to_string().repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    fn port(id: &str, role: PortRole, version: &str) -> PortDescriptor {
        PortDescriptor {
            port_id: PortId::parse(id).unwrap(),
            contract: PortContract {
                protocol_id: "example.simulation".to_string(),
                interface_id: "state".to_string(),
                version: version.to_string(),
                profiles: Vec::new(),
            },
            interaction: InteractionModelId(INTERACTION_CAPABILITY_UNARY.to_string()),
            role,
            transport: TransportRequirements::default(),
            annotations: BTreeMap::new(),
        }
    }

    fn export(id: &str) -> PortDescriptor {
        port(
            id,
            PortRole::Export {
                multiplicity: PortMultiplicity { min: 0, max: None },
                effect_class: EffectClass::DeterministicStateful,
            },
            "1.0.0",
        )
    }

    fn import(id: &str) -> PortDescriptor {
        port(
            id,
            PortRole::Import {
                multiplicity: PortMultiplicity {
                    min: 0,
                    max: Some(1),
                },
                latest_binding_phase: BindingPhase::Runtime,
                availability: AvailabilityPolicy::Optional,
                accepted_effects: vec![EffectClass::DeterministicStateful],
            },
            "^1.0",
        )
    }

    fn node(id: &str, byte: char, ports: Vec<PortDescriptor>) -> crate::AssemblyNode {
        crate::AssemblyNode {
            node_id: NodeId::parse(id).unwrap(),
            source: AssemblyNodeSource::Component {
                component: component(byte),
            },
            ports,
            configuration: None,
            annotations: BTreeMap::new(),
        }
    }

    fn insert(
        assemblies: &mut BTreeMap<String, AssemblyRevision>,
        value: AssemblyRevision,
    ) -> ArtifactDescriptor {
        let descriptor = value.artifact_descriptor().unwrap();
        assemblies.insert(descriptor.digest.clone(), value);
        descriptor
    }

    #[test]
    fn selected_subgraph_computes_import_export_existing_boundary_and_state() {
        let save = NodeId::parse("save-provider").unwrap();
        let simulation = NodeId::parse("simulation").unwrap();
        let renderer = NodeId::parse("renderer").unwrap();
        let schema = ArtifactDescriptor {
            artifact_type_uri: "urn:example:state-schema:v1".to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", "f".repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let root = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse("example/root").unwrap(),
            nodes: vec![
                node("save-provider", 'a', vec![export("save")]),
                node(
                    "simulation",
                    'b',
                    vec![import("save"), export("render"), export("migrate")],
                ),
                node("renderer", 'c', vec![import("render")]),
            ],
            bindings: vec![
                crate::AssemblyBinding {
                    binding_id: "save-input".to_string(),
                    provider: PortEndpoint {
                        node_id: save,
                        port_id: PortId::parse("save").unwrap(),
                    },
                    consumer: PortEndpoint {
                        node_id: simulation.clone(),
                        port_id: PortId::parse("save").unwrap(),
                    },
                    phase: BindingPhase::Runtime,
                    transport_policy: TransportPolicy::default(),
                    annotations: BTreeMap::new(),
                },
                crate::AssemblyBinding {
                    binding_id: "render-output".to_string(),
                    provider: PortEndpoint {
                        node_id: simulation.clone(),
                        port_id: PortId::parse("render").unwrap(),
                    },
                    consumer: PortEndpoint {
                        node_id: renderer,
                        port_id: PortId::parse("render").unwrap(),
                    },
                    phase: BindingPhase::Runtime,
                    transport_policy: TransportPolicy::default(),
                    annotations: BTreeMap::new(),
                },
            ],
            exposed_ports: vec![AssemblyPortExposure {
                port_id: PortId::parse("migrate").unwrap(),
                direction: PortDirection::Export,
                target: PortEndpoint {
                    node_id: simulation.clone(),
                    port_id: PortId::parse("migrate").unwrap(),
                },
                annotations: BTreeMap::new(),
            }],
            state_slots: vec![StateSlotDescriptor {
                state_slot_id: StateSlotId::parse("simulation-save").unwrap(),
                owner_node_id: simulation.clone(),
                schema_ref: Some(schema),
                scope: StateScope::Installation,
                portability: StatePortability::Portable,
                migration_port: Some(PortEndpoint {
                    node_id: simulation.clone(),
                    port_id: PortId::parse("migrate").unwrap(),
                }),
                backup_policy: BackupPolicy::Required,
                annotations: BTreeMap::new(),
            }],
            annotations: BTreeMap::new(),
        };
        let mut assemblies = BTreeMap::new();
        let source = insert(&mut assemblies, root);
        let first = promote_assembly_subgraph(
            &source,
            &assemblies,
            std::slice::from_ref(&simulation),
            AssemblyId::parse("example/promoted").unwrap(),
        )
        .unwrap();
        let second = promote_assembly_subgraph(
            &source,
            &assemblies,
            std::slice::from_ref(&simulation),
            AssemblyId::parse("example/promoted").unwrap(),
        )
        .unwrap();
        assert_eq!(first.candidate_artifact, second.candidate_artifact);
        assert_eq!(first.nested_assembly.nodes.len(), 1);
        assert_eq!(first.nested_assembly.bindings.len(), 0);
        assert_eq!(first.nested_assembly.exposed_ports.len(), 3);
        assert_eq!(first.nested_assembly.state_slots.len(), 1);
        assert!(first.candidate.boundary_ports.iter().any(|port| {
            port.port_id.as_str() == "save-input" && port.direction == PortDirection::Import
        }));
        assert!(first.candidate.boundary_ports.iter().any(|port| {
            port.port_id.as_str() == "render-output" && port.direction == PortDirection::Export
        }));
    }

    #[test]
    fn internal_bindings_remain_inside_the_candidate() {
        let first = NodeId::parse("first").unwrap();
        let second = NodeId::parse("second").unwrap();
        let root = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse("example/root").unwrap(),
            nodes: vec![
                node("first", 'a', vec![export("state")]),
                node("second", 'b', vec![import("state")]),
            ],
            bindings: vec![crate::AssemblyBinding {
                binding_id: "internal".to_string(),
                provider: PortEndpoint {
                    node_id: first.clone(),
                    port_id: PortId::parse("state").unwrap(),
                },
                consumer: PortEndpoint {
                    node_id: second.clone(),
                    port_id: PortId::parse("state").unwrap(),
                },
                phase: BindingPhase::Runtime,
                transport_policy: TransportPolicy::default(),
                annotations: BTreeMap::new(),
            }],
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let mut assemblies = BTreeMap::new();
        let source = insert(&mut assemblies, root);
        let output = promote_assembly_subgraph(
            &source,
            &assemblies,
            &[second, first],
            AssemblyId::parse("example/promoted").unwrap(),
        )
        .unwrap();
        assert_eq!(output.nested_assembly.bindings.len(), 1);
        assert!(output.candidate.boundary_ports.is_empty());
        assert_eq!(
            output
                .candidate
                .selected_nodes
                .iter()
                .map(NodeId::as_str)
                .collect::<Vec<_>>(),
            vec!["first", "second"]
        );
    }

    #[test]
    fn nested_assembly_node_uses_its_exposed_port_catalog() {
        let child = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse("example/child").unwrap(),
            nodes: vec![node("leaf", 'a', vec![export("out")])],
            bindings: Vec::new(),
            exposed_ports: vec![AssemblyPortExposure {
                port_id: PortId::parse("public").unwrap(),
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
        let mut assemblies = BTreeMap::new();
        let child_ref = insert(&mut assemblies, child);
        let root = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse("example/root").unwrap(),
            nodes: vec![crate::AssemblyNode {
                node_id: NodeId::parse("nested").unwrap(),
                source: AssemblyNodeSource::Assembly {
                    assembly: child_ref,
                },
                ports: Vec::new(),
                configuration: None,
                annotations: BTreeMap::new(),
            }],
            bindings: Vec::new(),
            exposed_ports: vec![AssemblyPortExposure {
                port_id: PortId::parse("root-public").unwrap(),
                direction: PortDirection::Export,
                target: PortEndpoint {
                    node_id: NodeId::parse("nested").unwrap(),
                    port_id: PortId::parse("public").unwrap(),
                },
                annotations: BTreeMap::new(),
            }],
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let source = insert(&mut assemblies, root);
        let output = promote_assembly_subgraph(
            &source,
            &assemblies,
            &[NodeId::parse("nested").unwrap()],
            AssemblyId::parse("example/promoted").unwrap(),
        )
        .unwrap();
        assert_eq!(output.candidate.boundary_ports.len(), 1);
        assert_eq!(
            output.candidate.boundary_ports[0]
                .descriptor
                .port_id
                .as_str(),
            "root-public"
        );
    }

    #[test]
    fn external_migration_port_and_boundary_collision_fail_closed() {
        let owner = NodeId::parse("owner").unwrap();
        let migration = NodeId::parse("migration").unwrap();
        let root = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse("example/root").unwrap(),
            nodes: vec![
                node("owner", 'a', vec![import("state")]),
                node("migration", 'b', vec![export("state")]),
            ],
            bindings: vec![crate::AssemblyBinding {
                binding_id: "state".to_string(),
                provider: PortEndpoint {
                    node_id: migration.clone(),
                    port_id: PortId::parse("state").unwrap(),
                },
                consumer: PortEndpoint {
                    node_id: owner.clone(),
                    port_id: PortId::parse("state").unwrap(),
                },
                phase: BindingPhase::Runtime,
                transport_policy: TransportPolicy::default(),
                annotations: BTreeMap::new(),
            }],
            exposed_ports: vec![AssemblyPortExposure {
                port_id: PortId::parse("state").unwrap(),
                direction: PortDirection::Import,
                target: PortEndpoint {
                    node_id: owner.clone(),
                    port_id: PortId::parse("state").unwrap(),
                },
                annotations: BTreeMap::new(),
            }],
            state_slots: vec![StateSlotDescriptor {
                state_slot_id: StateSlotId::parse("save").unwrap(),
                owner_node_id: owner.clone(),
                schema_ref: None,
                scope: StateScope::Installation,
                portability: StatePortability::HostBound,
                migration_port: Some(PortEndpoint {
                    node_id: migration,
                    port_id: PortId::parse("state").unwrap(),
                }),
                backup_policy: BackupPolicy::Allowed,
                annotations: BTreeMap::new(),
            }],
            annotations: BTreeMap::new(),
        };
        let mut assemblies = BTreeMap::new();
        let source = insert(&mut assemblies, root.clone());
        assert_eq!(
            promote_assembly_subgraph(
                &source,
                &assemblies,
                std::slice::from_ref(&owner),
                AssemblyId::parse("example/promoted").unwrap(),
            )
            .unwrap_err()
            .code,
            DiagnosticCode::BindingAmbiguous
        );

        let mut without_collision = root;
        without_collision.exposed_ports.clear();
        let mut assemblies = BTreeMap::new();
        let source = insert(&mut assemblies, without_collision);
        assert_eq!(
            promote_assembly_subgraph(
                &source,
                &assemblies,
                &[owner],
                AssemblyId::parse("example/promoted").unwrap(),
            )
            .unwrap_err()
            .code,
            DiagnosticCode::StateMigrationRequired
        );
    }
}
