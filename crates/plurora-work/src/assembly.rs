use std::collections::{BTreeMap, BTreeSet};

use plurora_core::{ArtifactDescriptor, COMPONENT_DESCRIPTOR_TYPE_URI};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canonical::{
    fixed_string_schema, validate_artifact_descriptor, validate_descriptor_type,
    validate_portable_model, ArtifactModel,
};
use crate::diagnostic::{DiagnosticCode, ModelError, ModelResult};
use crate::ids::{validate_local_id, AssemblyId, NodeId, PortId};
use crate::port::{
    check_port_compatibility, check_transport_policy_compatibility, AvailabilityPolicy,
    BindingPhase, PortDescriptor, PortDirection, PortEndpoint, PortRole, TransportPolicy,
};
use crate::state::StateSlotDescriptor;

pub const ASSEMBLY_REVISION_TYPE_URI: &str = "urn:plurora:assembly-revision:v1";
pub const MAX_ASSEMBLY_NODES: usize = 4_096;
pub const MAX_ASSEMBLY_BINDINGS: usize = 16_384;
pub const MAX_ASSEMBLY_DEPTH: usize = 32;
pub const MAX_PORTS_PER_NODE: usize = 1_024;
pub const MAX_EXPOSED_PORTS: usize = 4_096;
pub const MAX_STATE_SLOTS: usize = 4_096;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssemblyNodeSource {
    Component { component: ArtifactDescriptor },
    Assembly { assembly: ArtifactDescriptor },
}

impl AssemblyNodeSource {
    pub fn artifact(&self) -> &ArtifactDescriptor {
        match self {
            Self::Component { component } => component,
            Self::Assembly { assembly } => assembly,
        }
    }

    pub fn validate(&self) -> ModelResult<()> {
        match self {
            Self::Component { component } => {
                validate_descriptor_type(component, COMPONENT_DESCRIPTOR_TYPE_URI)
            }
            Self::Assembly { assembly } => {
                validate_descriptor_type(assembly, ASSEMBLY_REVISION_TYPE_URI)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AssemblyNode {
    pub node_id: NodeId,
    pub source: AssemblyNodeSource,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<PortDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AssemblyBinding {
    pub binding_id: String,
    pub provider: PortEndpoint,
    pub consumer: PortEndpoint,
    pub phase: BindingPhase,
    #[serde(default)]
    pub transport_policy: TransportPolicy,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AssemblyPortExposure {
    pub port_id: PortId,
    pub direction: PortDirection,
    pub target: PortEndpoint,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AssemblyRevision {
    #[schemars(schema_with = "assembly_revision_schema")]
    pub schema: String,
    pub assembly_id: AssemblyId,
    pub nodes: Vec<AssemblyNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<AssemblyBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exposed_ports: Vec<AssemblyPortExposure>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_slots: Vec<StateSlotDescriptor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

fn assembly_revision_schema(
    _generator: &mut schemars::gen::SchemaGenerator,
) -> schemars::schema::Schema {
    fixed_string_schema(AssemblyRevision::SCHEMA)
}

impl AssemblyRevision {
    pub const SCHEMA: &'static str = "plurora.assembly-revision.v1";

    pub fn validate(&self) -> ModelResult<()> {
        if self.schema != Self::SCHEMA {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "assembly revision uses an unsupported schema",
            ));
        }
        if self.nodes.len() > MAX_ASSEMBLY_NODES
            || self.bindings.len() > MAX_ASSEMBLY_BINDINGS
            || self.exposed_ports.len() > MAX_EXPOSED_PORTS
            || self.state_slots.len() > MAX_STATE_SLOTS
        {
            return Err(ModelError::new(
                DiagnosticCode::WorkTooComplex,
                "assembly exceeds an implementation complexity budget",
            ));
        }

        let mut node_ids = BTreeSet::new();
        for node in &self.nodes {
            if !node_ids.insert(&node.node_id) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "assembly contains a duplicate node id",
                ));
            }
            node.source.validate()?;
            if node.ports.len() > MAX_PORTS_PER_NODE {
                return Err(ModelError::new(
                    DiagnosticCode::WorkTooComplex,
                    "assembly node exceeds the port implementation budget",
                ));
            }
            if matches!(node.source, AssemblyNodeSource::Assembly { .. }) && !node.ports.is_empty()
            {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "nested Assembly nodes expose Ports through the nested revision, not an inline catalog",
                ));
            }
            let mut port_ids = BTreeSet::new();
            for port in &node.ports {
                port.validate()?;
                if !port_ids.insert(&port.port_id) {
                    return Err(ModelError::new(
                        DiagnosticCode::WorkInvalid,
                        "assembly node contains a duplicate Port id",
                    ));
                }
            }
            if let Some(configuration) = &node.configuration {
                validate_artifact_descriptor(configuration)?;
            }
        }

        let mut binding_ids = BTreeSet::new();
        for binding in &self.bindings {
            validate_local_id(&binding.binding_id, "assembly binding id")?;
            if !binding_ids.insert(binding.binding_id.as_str()) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "assembly contains a duplicate binding id",
                ));
            }
            if !node_ids.contains(&binding.provider.node_id)
                || !node_ids.contains(&binding.consumer.node_id)
            {
                return Err(ModelError::new(
                    DiagnosticCode::PortUnresolved,
                    "assembly binding references an unknown node",
                ));
            }
            binding.transport_policy.validate()?;
        }

        let mut exposed_port_ids = BTreeSet::new();
        for exposure in &self.exposed_ports {
            if !exposed_port_ids.insert(&exposure.port_id) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "assembly contains a duplicate exposed port id",
                ));
            }
            if !node_ids.contains(&exposure.target.node_id) {
                return Err(ModelError::new(
                    DiagnosticCode::PortUnresolved,
                    "assembly exposure references an unknown node",
                ));
            }
        }

        let mut state_slot_ids = BTreeSet::new();
        for slot in &self.state_slots {
            if !state_slot_ids.insert(&slot.state_slot_id) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "assembly contains a duplicate state slot id",
                ));
            }
            if !node_ids.contains(&slot.owner_node_id)
                || slot
                    .migration_port
                    .as_ref()
                    .is_some_and(|port| !node_ids.contains(&port.node_id))
            {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "state slot references an unknown owner or migration node",
                ));
            }
            slot.validate()?;
        }
        validate_portable_model(self)
    }

    pub fn validate_ports(
        &self,
        ports: &BTreeMap<PortEndpoint, PortDescriptor>,
    ) -> ModelResult<()> {
        self.validate()?;
        let node_ids = self
            .nodes
            .iter()
            .map(|node| &node.node_id)
            .collect::<BTreeSet<_>>();
        let component_node_ids = self
            .nodes
            .iter()
            .filter_map(|node| {
                matches!(node.source, AssemblyNodeSource::Component { .. }).then_some(&node.node_id)
            })
            .collect::<BTreeSet<_>>();
        let declared_component_ports = self
            .nodes
            .iter()
            .filter(|node| matches!(node.source, AssemblyNodeSource::Component { .. }))
            .flat_map(|node| {
                node.ports.iter().map(|port| {
                    (
                        PortEndpoint {
                            node_id: node.node_id.clone(),
                            port_id: port.port_id.clone(),
                        },
                        port,
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        for (endpoint, descriptor) in &declared_component_ports {
            if ports.get(endpoint) != Some(*descriptor) {
                return Err(ModelError::new(
                    DiagnosticCode::ArtifactDigestMismatch,
                    "port catalog differs from the canonical Assembly node contract",
                ));
            }
        }
        let mut ports_per_node: BTreeMap<&NodeId, usize> = BTreeMap::new();
        for (endpoint, descriptor) in ports {
            if !node_ids.contains(&endpoint.node_id) || endpoint.port_id != descriptor.port_id {
                return Err(ModelError::new(
                    DiagnosticCode::PortUnresolved,
                    "port catalog does not match the assembly graph",
                ));
            }
            if component_node_ids.contains(&endpoint.node_id)
                && !declared_component_ports.contains_key(endpoint)
            {
                return Err(ModelError::new(
                    DiagnosticCode::ArtifactDigestMismatch,
                    "port catalog contains a Component Port absent from the canonical Assembly node",
                ));
            }
            descriptor.validate()?;
            *ports_per_node.entry(&endpoint.node_id).or_default() += 1;
        }
        if ports_per_node
            .values()
            .any(|count| *count > MAX_PORTS_PER_NODE)
        {
            return Err(ModelError::new(
                DiagnosticCode::WorkTooComplex,
                "assembly node exceeds the port implementation budget",
            ));
        }

        let mut provider_counts: BTreeMap<&PortEndpoint, usize> = BTreeMap::new();
        let mut consumer_counts: BTreeMap<&PortEndpoint, usize> = BTreeMap::new();
        for binding in &self.bindings {
            let provider = ports.get(&binding.provider).ok_or_else(|| {
                ModelError::new(
                    DiagnosticCode::PortUnresolved,
                    "binding provider port is absent from the port catalog",
                )
            })?;
            let consumer = ports.get(&binding.consumer).ok_or_else(|| {
                ModelError::new(
                    DiagnosticCode::PortUnresolved,
                    "binding consumer port is absent from the port catalog",
                )
            })?;
            check_port_compatibility(provider, consumer)?;
            check_transport_policy_compatibility(
                &provider.transport,
                &consumer.transport,
                &binding.transport_policy,
            )?;
            if let PortRole::Import {
                latest_binding_phase,
                ..
            } = consumer.role
            {
                if binding.phase > latest_binding_phase {
                    return Err(ModelError::new(
                        DiagnosticCode::PortIncompatible,
                        "binding occurs after the consumer's latest binding phase",
                    ));
                }
            }
            *provider_counts.entry(&binding.provider).or_default() += 1;
            *consumer_counts.entry(&binding.consumer).or_default() += 1;
        }

        for (endpoint, descriptor) in ports {
            let count = match descriptor.role {
                PortRole::Import { .. } => consumer_counts.get(endpoint).copied().unwrap_or(0),
                PortRole::Export { .. } => provider_counts.get(endpoint).copied().unwrap_or(0),
            };
            let multiplicity = descriptor.role.multiplicity();
            if multiplicity.max.is_some_and(|max| count > usize::from(max)) {
                return Err(ModelError::new(
                    DiagnosticCode::PortIncompatible,
                    "port binding count exceeds its maximum cardinality",
                ));
            }
            if let PortRole::Import {
                latest_binding_phase: BindingPhase::Authoring,
                availability: AvailabilityPolicy::Required,
                ..
            } = descriptor.role
            {
                if count < usize::from(multiplicity.min) {
                    return Err(ModelError::new(
                        DiagnosticCode::PortUnresolved,
                        "authoring-time required import is unresolved",
                    ));
                }
            }
        }

        for exposure in &self.exposed_ports {
            let target = ports.get(&exposure.target).ok_or_else(|| {
                ModelError::new(
                    DiagnosticCode::PortUnresolved,
                    "exposed port target is absent from the port catalog",
                )
            })?;
            let direction_matches = matches!(
                (exposure.direction, &target.role),
                (PortDirection::Import, PortRole::Import { .. })
                    | (PortDirection::Export, PortRole::Export { .. })
            );
            if !direction_matches {
                return Err(ModelError::new(
                    DiagnosticCode::PortIncompatible,
                    "exposed port direction does not match its target",
                ));
            }
        }
        for slot in &self.state_slots {
            let Some(endpoint) = &slot.migration_port else {
                continue;
            };
            let descriptor = ports.get(endpoint).ok_or_else(|| {
                ModelError::new(
                    DiagnosticCode::PortUnresolved,
                    "state migration port is absent from the port catalog",
                )
            })?;
            if !descriptor.interaction.is_directly_supported() {
                return Err(ModelError::new(
                    DiagnosticCode::UnsupportedInteraction,
                    "state migration port uses an interaction without an implementation or adapter",
                ));
            }
        }
        Ok(())
    }
}

impl ArtifactModel for AssemblyRevision {
    const ARTIFACT_TYPE_URI: &'static str = ASSEMBLY_REVISION_TYPE_URI;

    fn validate(&self) -> ModelResult<()> {
        AssemblyRevision::validate(self)
    }

    fn referenced_artifacts(&self) -> Vec<&ArtifactDescriptor> {
        let mut references = Vec::new();
        for node in &self.nodes {
            references.push(node.source.artifact());
            if let Some(configuration) = &node.configuration {
                references.push(configuration);
            }
        }
        for slot in &self.state_slots {
            if let Some(schema) = &slot.schema_ref {
                references.push(schema);
            }
        }
        references
    }
}

pub fn validate_assembly_closure(
    root: &ArtifactDescriptor,
    assemblies: &BTreeMap<String, AssemblyRevision>,
) -> ModelResult<()> {
    validate_descriptor_type(root, ASSEMBLY_REVISION_TYPE_URI)?;
    let mut visiting = BTreeSet::new();
    let mut heights = BTreeMap::new();
    validate_assembly_subtree(&root.digest, 1, assemblies, &mut visiting, &mut heights)?;
    for (digest, assembly) in assemblies {
        if !heights.contains_key(digest) {
            continue;
        }
        assembly.validate()?;
        let descriptor = assembly.artifact_descriptor()?;
        if descriptor.digest != *digest {
            return Err(ModelError::new(
                DiagnosticCode::ArtifactDigestMismatch,
                "assembly closure key does not match canonical assembly bytes",
            ));
        }
    }
    Ok(())
}

fn validate_assembly_subtree(
    digest: &str,
    depth: usize,
    assemblies: &BTreeMap<String, AssemblyRevision>,
    visiting: &mut BTreeSet<String>,
    heights: &mut BTreeMap<String, usize>,
) -> ModelResult<usize> {
    ensure_assembly_depth(depth, 1)?;
    if let Some(height) = heights.get(digest).copied() {
        ensure_assembly_depth(depth, height)?;
        return Ok(height);
    }
    if !visiting.insert(digest.to_string()) {
        return Err(ModelError::new(
            DiagnosticCode::AssemblyCycle,
            "nested assembly containment graph contains a cycle",
        ));
    }
    let result = (|| {
        let assembly = assemblies.get(digest).ok_or_else(|| {
            ModelError::new(
                DiagnosticCode::ArtifactMissing,
                "nested assembly closure is incomplete",
            )
        })?;
        let mut maximum_child_height = 0;
        for child in assembly.nodes.iter().filter_map(|node| match &node.source {
            AssemblyNodeSource::Assembly { assembly } => Some(assembly.digest.as_str()),
            AssemblyNodeSource::Component { .. } => None,
        }) {
            let child_height =
                validate_assembly_subtree(child, depth + 1, assemblies, visiting, heights)?;
            maximum_child_height = maximum_child_height.max(child_height);
        }
        let height = maximum_child_height.checked_add(1).ok_or_else(|| {
            ModelError::new(
                DiagnosticCode::WorkTooComplex,
                "nested assembly containment depth overflowed",
            )
        })?;
        ensure_assembly_depth(depth, height)?;
        Ok(height)
    })();
    visiting.remove(digest);
    let height = result?;
    heights.insert(digest.to_string(), height);
    Ok(height)
}

fn ensure_assembly_depth(depth: usize, height: usize) -> ModelResult<()> {
    if depth
        .checked_add(height.saturating_sub(1))
        .is_none_or(|deepest| deepest > MAX_ASSEMBLY_DEPTH)
    {
        return Err(ModelError::new(
            DiagnosticCode::WorkTooComplex,
            "nested assembly containment exceeds the maximum depth of 32",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn assembly_ref(value: char) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: ASSEMBLY_REVISION_TYPE_URI.to_string(),
            media_type: "application/json".to_string(),
            digest: digest(value),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    fn component_ref(value: char) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: COMPONENT_DESCRIPTOR_TYPE_URI.to_string(),
            media_type: "application/json".to_string(),
            digest: digest(value),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    fn assembly(id: &str, child: Option<ArtifactDescriptor>) -> AssemblyRevision {
        AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse(id).unwrap(),
            nodes: child
                .into_iter()
                .map(|assembly| AssemblyNode {
                    node_id: NodeId::parse("nested").unwrap(),
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

    fn insert_assembly(
        closure: &mut BTreeMap<String, AssemblyRevision>,
        value: AssemblyRevision,
    ) -> ArtifactDescriptor {
        let descriptor = value.artifact_descriptor().unwrap();
        closure.insert(descriptor.digest.clone(), value);
        descriptor
    }

    fn shared_dag_closure(
        wrapper_count: usize,
    ) -> (ArtifactDescriptor, BTreeMap<String, AssemblyRevision>) {
        let mut closure = BTreeMap::new();
        let leaf = insert_assembly(&mut closure, assembly("example/shared-leaf", None));
        let shared = insert_assembly(&mut closure, assembly("example/shared-target", Some(leaf)));
        let mut deep = shared.clone();
        for level in 0..wrapper_count {
            deep = insert_assembly(
                &mut closure,
                assembly(&format!("example/shared-wrapper-{level}"), Some(deep)),
            );
        }
        let root = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse("example/shared-root").unwrap(),
            nodes: vec![
                AssemblyNode {
                    node_id: NodeId::parse("a-direct").unwrap(),
                    source: AssemblyNodeSource::Assembly { assembly: shared },
                    ports: Vec::new(),
                    configuration: None,
                    annotations: BTreeMap::new(),
                },
                AssemblyNode {
                    node_id: NodeId::parse("z-deep").unwrap(),
                    source: AssemblyNodeSource::Assembly { assembly: deep },
                    ports: Vec::new(),
                    configuration: None,
                    annotations: BTreeMap::new(),
                },
            ],
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let root = insert_assembly(&mut closure, root);
        (root, closure)
    }

    #[test]
    fn nested_assembly_cycle_is_rejected_before_resolution() {
        let a = assembly_ref('a');
        let b = assembly_ref('b');
        let closure = BTreeMap::from([
            (a.digest.clone(), assembly("example/a", Some(b.clone()))),
            (b.digest.clone(), assembly("example/b", Some(a.clone()))),
        ]);
        assert_eq!(
            validate_assembly_closure(&a, &closure).unwrap_err().code,
            DiagnosticCode::AssemblyCycle
        );
    }

    #[test]
    fn shared_dag_depth_is_checked_for_every_occurrence() {
        let (valid_root, valid_closure) = shared_dag_closure(29);
        validate_assembly_closure(&valid_root, &valid_closure).unwrap();

        let (too_deep_root, too_deep_closure) = shared_dag_closure(30);
        assert_eq!(
            validate_assembly_closure(&too_deep_root, &too_deep_closure)
                .unwrap_err()
                .code,
            DiagnosticCode::WorkTooComplex
        );
    }

    #[test]
    fn unknown_annotations_survive_round_trip() {
        let mut value = assembly("example/main", None);
        value.annotations.insert(
            "thirdparty.example/custom".to_string(),
            serde_json::json!({"enabled": true}),
        );
        let decoded: AssemblyRevision =
            serde_json::from_value(serde_json::to_value(&value).unwrap()).unwrap();
        assert_eq!(decoded.annotations, value.annotations);
        assert_eq!(decoded.digest().unwrap(), value.digest().unwrap());
    }

    #[test]
    fn port_contract_semantics_are_content_addressed_by_the_assembly() {
        let mut value = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse("example/ports").unwrap(),
            nodes: vec![AssemblyNode {
                node_id: NodeId::parse("provider").unwrap(),
                source: AssemblyNodeSource::Component {
                    component: component_ref('c'),
                },
                ports: vec![PortDescriptor {
                    port_id: PortId::parse("out").unwrap(),
                    contract: crate::PortContract {
                        protocol_id: "example.protocol".to_string(),
                        interface_id: "example/interface".to_string(),
                        version: "1.0.0".to_string(),
                        profiles: Vec::new(),
                    },
                    interaction: crate::InteractionModelId(
                        crate::INTERACTION_CAPABILITY_UNARY.to_string(),
                    ),
                    role: PortRole::Export {
                        multiplicity: crate::PortMultiplicity { min: 0, max: None },
                        effect_class: crate::EffectClass::Pure,
                    },
                    transport: crate::TransportRequirements::default(),
                    annotations: BTreeMap::new(),
                }],
                configuration: None,
                annotations: BTreeMap::new(),
            }],
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let original = value.digest().unwrap();
        value.nodes[0].ports[0].contract.version = "2.0.0".to_string();
        assert_ne!(value.digest().unwrap(), original);
    }

    #[test]
    fn state_migration_endpoint_must_exist_in_the_port_catalog() {
        let node_id = NodeId::parse("stateful").unwrap();
        let value = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse("example/stateful").unwrap(),
            nodes: vec![AssemblyNode {
                node_id: node_id.clone(),
                source: AssemblyNodeSource::Component {
                    component: component_ref('c'),
                },
                ports: Vec::new(),
                configuration: None,
                annotations: BTreeMap::new(),
            }],
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: vec![StateSlotDescriptor {
                state_slot_id: crate::StateSlotId::parse("save").unwrap(),
                owner_node_id: node_id.clone(),
                schema_ref: None,
                scope: crate::StateScope::Installation,
                portability: crate::StatePortability::HostBound,
                migration_port: Some(PortEndpoint {
                    node_id,
                    port_id: PortId::parse("migrate").unwrap(),
                }),
                backup_policy: crate::BackupPolicy::Allowed,
                annotations: BTreeMap::new(),
            }],
            annotations: BTreeMap::new(),
        };
        assert_eq!(
            value.validate_ports(&BTreeMap::new()).unwrap_err().code,
            DiagnosticCode::PortUnresolved
        );
    }

    #[test]
    fn state_migration_endpoint_requires_a_supported_interaction() {
        let node_id = NodeId::parse("stateful").unwrap();
        let endpoint = PortEndpoint {
            node_id: node_id.clone(),
            port_id: PortId::parse("migrate").unwrap(),
        };
        let mut value = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: AssemblyId::parse("example/stateful").unwrap(),
            nodes: vec![AssemblyNode {
                node_id,
                source: AssemblyNodeSource::Component {
                    component: component_ref('c'),
                },
                ports: Vec::new(),
                configuration: None,
                annotations: BTreeMap::new(),
            }],
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: vec![StateSlotDescriptor {
                state_slot_id: crate::StateSlotId::parse("save").unwrap(),
                owner_node_id: endpoint.node_id.clone(),
                schema_ref: None,
                scope: crate::StateScope::Installation,
                portability: crate::StatePortability::HostBound,
                migration_port: Some(endpoint.clone()),
                backup_policy: crate::BackupPolicy::Allowed,
                annotations: BTreeMap::new(),
            }],
            annotations: BTreeMap::new(),
        };
        let port = PortDescriptor {
            port_id: endpoint.port_id.clone(),
            contract: crate::PortContract {
                protocol_id: "example.state".to_string(),
                interface_id: "migration".to_string(),
                version: "1.0.0".to_string(),
                profiles: Vec::new(),
            },
            interaction: crate::InteractionModelId("example.interaction.migration/v1".to_string()),
            role: PortRole::Export {
                multiplicity: crate::PortMultiplicity {
                    min: 0,
                    max: Some(1),
                },
                effect_class: crate::EffectClass::DeterministicStateful,
            },
            transport: crate::TransportRequirements::default(),
            annotations: BTreeMap::new(),
        };
        value.nodes[0].ports = vec![port.clone()];
        assert_eq!(
            value
                .validate_ports(&BTreeMap::from([(endpoint, port)]))
                .unwrap_err()
                .code,
            DiagnosticCode::UnsupportedInteraction
        );
    }
}
