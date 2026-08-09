use std::collections::BTreeSet;

use plurora_core::{
    validate_sha256, ArtifactDescriptor, ComponentTrustClass, ProtocolProfilePin,
    COMPONENT_DESCRIPTOR_TYPE_URI,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::assembly::ASSEMBLY_REVISION_TYPE_URI;
use crate::canonical::{
    fixed_string_schema, validate_artifact_descriptor, validate_descriptor_type,
    validate_portable_model, ArtifactModel,
};
use crate::diagnostic::{DiagnosticCode, ModelError, ModelResult};
use crate::ids::{validate_local_id, NodeId};
use crate::port::{BindingPhase, PortEndpoint, SelectedTransport};

pub const ASSEMBLY_LOCK_TYPE_URI: &str = "urn:plurora:assembly-lock:v1";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct NodeLock {
    pub node_id: NodeId,
    pub artifact: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behavior_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_class: Option<ComponentTrustClass>,
}

impl NodeLock {
    pub fn validate(&self) -> ModelResult<()> {
        validate_artifact_descriptor(&self.artifact)?;
        match self.artifact.artifact_type_uri.as_str() {
            COMPONENT_DESCRIPTOR_TYPE_URI => {
                let behavior = self.behavior_digest.as_deref().ok_or_else(|| {
                    ModelError::new(
                        DiagnosticCode::WorkInvalid,
                        "component node lock is missing its behavior digest",
                    )
                })?;
                validate_sha256(behavior).map_err(|_| {
                    ModelError::new(
                        DiagnosticCode::ArtifactDigestMismatch,
                        "component behavior digest is not a complete SHA-256 value",
                    )
                })?;
                if self.trust_class.is_none() {
                    return Err(ModelError::new(
                        DiagnosticCode::WorkInvalid,
                        "component node lock is missing its trust class",
                    ));
                }
            }
            ASSEMBLY_REVISION_TYPE_URI => {
                if self.behavior_digest.is_some() || self.trust_class.is_some() {
                    return Err(ModelError::new(
                        DiagnosticCode::WorkInvalid,
                        "nested assembly lock must not invent component behavior or trust",
                    ));
                }
            }
            _ => {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "node lock references neither a component nor an assembly artifact",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct BindingLock {
    pub binding_id: String,
    pub provider: PortEndpoint,
    pub consumer: PortEndpoint,
    pub provider_component: ArtifactDescriptor,
    pub transport: SelectedTransport,
    pub phase: BindingPhase,
}

impl BindingLock {
    pub fn validate(&self) -> ModelResult<()> {
        validate_local_id(&self.binding_id, "binding lock id")?;
        validate_descriptor_type(&self.provider_component, COMPONENT_DESCRIPTOR_TYPE_URI)?;
        self.transport.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AssemblyLock {
    #[schemars(schema_with = "assembly_lock_schema")]
    pub schema: String,
    pub assembly: ArtifactDescriptor,
    pub nodes: Vec<NodeLock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<BindingLock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocol_profiles: Vec<ProtocolProfilePin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content_roots: Vec<ArtifactDescriptor>,
}

fn assembly_lock_schema(
    _generator: &mut schemars::gen::SchemaGenerator,
) -> schemars::schema::Schema {
    fixed_string_schema(AssemblyLock::SCHEMA)
}

impl AssemblyLock {
    pub const SCHEMA: &'static str = "plurora.assembly-lock.v1";

    pub fn validate(&self) -> ModelResult<()> {
        if self.schema != Self::SCHEMA {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "assembly lock uses an unsupported schema",
            ));
        }
        validate_descriptor_type(&self.assembly, ASSEMBLY_REVISION_TYPE_URI)?;
        let mut node_ids = BTreeSet::new();
        for node in &self.nodes {
            node.validate()?;
            if !node_ids.insert(&node.node_id) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "assembly lock contains a duplicate node id",
                ));
            }
        }
        let mut binding_ids = BTreeSet::new();
        for binding in &self.bindings {
            binding.validate()?;
            if !binding_ids.insert(binding.binding_id.as_str()) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "assembly lock contains a duplicate binding id",
                ));
            }
            if !node_ids.contains(&binding.provider.node_id)
                || !node_ids.contains(&binding.consumer.node_id)
            {
                return Err(ModelError::new(
                    DiagnosticCode::PortUnresolved,
                    "assembly lock binding references an unknown node",
                ));
            }
        }
        let mut profiles = BTreeSet::new();
        for profile in &self.protocol_profiles {
            if profile.protocol_id.trim().is_empty()
                || profile.version.trim().is_empty()
                || profile.profile.trim().is_empty()
                || !profiles.insert((
                    profile.protocol_id.as_str(),
                    profile.version.as_str(),
                    profile.profile.as_str(),
                ))
            {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "assembly lock contains an invalid or duplicate protocol profile pin",
                ));
            }
        }
        let mut roots = BTreeSet::new();
        for root in &self.content_roots {
            validate_artifact_descriptor(root)?;
            if !roots.insert(root.digest.as_str()) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "assembly lock contains a duplicate content root",
                ));
            }
        }
        validate_portable_model(self)
    }

    pub fn replace_node(&mut self, node_id: &NodeId, replacement: NodeLock) -> ModelResult<()> {
        let index = self
            .nodes
            .iter()
            .position(|node| &node.node_id == node_id)
            .ok_or_else(|| {
                ModelError::new(
                    DiagnosticCode::ArtifactMissing,
                    "assembly lock node replacement target is absent",
                )
            })?;
        let mut candidate = self.clone();
        candidate.nodes[index] = replacement;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

impl ArtifactModel for AssemblyLock {
    const ARTIFACT_TYPE_URI: &'static str = ASSEMBLY_LOCK_TYPE_URI;

    fn validate(&self) -> ModelResult<()> {
        AssemblyLock::validate(self)
    }

    fn referenced_artifacts(&self) -> Vec<&ArtifactDescriptor> {
        std::iter::once(&self.assembly)
            .chain(self.nodes.iter().map(|node| &node.artifact))
            .chain(
                self.bindings
                    .iter()
                    .map(|binding| &binding.provider_component),
            )
            .chain(self.content_roots.iter())
            .collect()
    }
}
