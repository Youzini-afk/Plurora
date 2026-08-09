use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use plurora_core::ArtifactDescriptor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canonical::{
    fixed_string_schema, validate_artifact_descriptor, validate_portable_model, ArtifactModel,
};
use crate::diagnostic::{DiagnosticCode, ModelError, ModelResult};
use crate::ids::{validate_local_id, NodeId, StateSlotId};
use crate::port::PortEndpoint;

pub const OPERATIONAL_INTENT_TYPE_URI: &str = "urn:plurora:operational-intent:v1";
pub const TARGET_INVENTORY_TYPE_URI: &str = "urn:plurora:target-inventory:v1";

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ResourceRequirements {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_concurrency: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
    AppendOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct NetworkImport {
    pub import_id: String,
    pub protocol: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_destinations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct FilesystemImport {
    pub import_id: String,
    pub resource_id: String,
    pub access: AccessMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SecretImport {
    pub secret_id: String,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorkloadImports {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub network: Vec<NetworkImport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filesystem: Vec<FilesystemImport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secret_imports: Vec<SecretImport>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RestartPolicy {
    Never,
    OnFailure,
    Always,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ReplicaPolicy {
    pub min: u16,
    pub max: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorkloadIntent {
    pub workload_id: String,
    pub node_id: NodeId,
    pub execution_classes: Vec<String>,
    #[serde(default)]
    pub resources: ResourceRequirements,
    #[serde(default)]
    pub imports: WorkloadImports,
    pub replicas: ReplicaPolicy,
    pub restart_policy: RestartPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_port: Option<PortEndpoint>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EndpointVisibility {
    Private,
    Shared,
    Public,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct EndpointIntent {
    pub endpoint_id: String,
    pub port: PortEndpoint,
    pub visibility: EndpointVisibility,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_hint: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StateDurability {
    Ephemeral,
    Durable,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct StatePlacementIntent {
    pub state_slot_id: StateSlotId,
    pub durability: StateDurability,
    #[serde(default)]
    pub replication: u16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlacementConstraintKind {
    CoLocate,
    Separate,
    RequireLabel,
    PreferLabel,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PlacementConstraint {
    pub constraint_id: String,
    pub kind: PlacementConstraintKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<NodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub hard: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStrategy {
    Replace,
    Rolling,
    Recreate,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct UpdatePolicy {
    pub strategy: UpdateStrategy,
    #[serde(default)]
    pub max_unavailable: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_timeout_seconds: Option<u64>,
}

impl Default for UpdatePolicy {
    fn default() -> Self {
        Self {
            strategy: UpdateStrategy::Replace,
            max_unavailable: 1,
            health_timeout_seconds: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct OperationalIntent {
    #[schemars(schema_with = "operational_intent_schema")]
    pub schema: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workloads: Vec<WorkloadIntent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub endpoints: Vec<EndpointIntent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state: Vec<StatePlacementIntent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placement: Vec<PlacementConstraint>,
    #[serde(default)]
    pub update_policy: UpdatePolicy,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

fn operational_intent_schema(
    _generator: &mut schemars::gen::SchemaGenerator,
) -> schemars::schema::Schema {
    fixed_string_schema(OperationalIntent::SCHEMA)
}

impl OperationalIntent {
    pub const SCHEMA: &'static str = "plurora.operational-intent.v1";

    pub fn validate(&self) -> ModelResult<()> {
        if self.schema != Self::SCHEMA {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "operational intent uses an unsupported schema",
            ));
        }
        let mut workloads = BTreeSet::new();
        for workload in &self.workloads {
            validate_local_id(&workload.workload_id, "workload id")?;
            if !workloads.insert(workload.workload_id.as_str())
                || workload.execution_classes.is_empty()
                || workload.replicas.min == 0
                || workload.replicas.min > workload.replicas.max
            {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "workload identity, execution class, or replica range is invalid",
                ));
            }
            for class in &workload.execution_classes {
                validate_open_text(class, "execution class")?;
            }
            validate_imports(&workload.imports)?;
        }
        let mut endpoints = BTreeSet::new();
        for endpoint in &self.endpoints {
            validate_local_id(&endpoint.endpoint_id, "endpoint id")?;
            if !endpoints.insert(endpoint.endpoint_id.as_str()) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "operational intent contains a duplicate endpoint id",
                ));
            }
            if let Some(protocol) = &endpoint.protocol_hint {
                validate_open_text(protocol, "endpoint protocol")?;
            }
        }
        let mut state = BTreeSet::new();
        for placement in &self.state {
            if !state.insert(&placement.state_slot_id) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "operational intent repeats a state slot",
                ));
            }
            if placement.durability != StateDurability::Ephemeral && placement.replication == 0 {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "durable or external state requires a non-zero replication declaration",
                ));
            }
        }
        let mut constraints = BTreeSet::new();
        for constraint in &self.placement {
            validate_local_id(&constraint.constraint_id, "placement constraint id")?;
            if !constraints.insert(constraint.constraint_id.as_str()) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "operational intent contains a duplicate placement constraint",
                ));
            }
        }
        validate_portable_model(self)
    }
}

impl ArtifactModel for OperationalIntent {
    const ARTIFACT_TYPE_URI: &'static str = OPERATIONAL_INTENT_TYPE_URI;

    fn validate(&self) -> ModelResult<()> {
        OperationalIntent::validate(self)
    }
}

fn validate_imports(imports: &WorkloadImports) -> ModelResult<()> {
    let mut ids = BTreeSet::new();
    for import in &imports.network {
        validate_local_id(&import.import_id, "network import id")?;
        validate_open_text(&import.protocol, "network import protocol")?;
        if !ids.insert(import.import_id.as_str()) {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "workload contains a duplicate import id",
            ));
        }
    }
    for import in &imports.filesystem {
        validate_local_id(&import.import_id, "filesystem import id")?;
        validate_open_text(&import.resource_id, "filesystem resource id")?;
        if !ids.insert(import.import_id.as_str()) {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "workload contains a duplicate import id",
            ));
        }
    }
    for import in &imports.secret_imports {
        validate_local_id(&import.secret_id, "secret import id")?;
        if !ids.insert(import.secret_id.as_str()) {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "workload contains a duplicate import id",
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct TargetCapabilityRecord {
    pub capability_id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, Value>,
    pub available: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ResourceCapacity {
    #[serde(default)]
    pub cpu_millis: u64,
    #[serde(default)]
    pub memory_bytes: u64,
    #[serde(default)]
    pub storage_bytes: u64,
    #[serde(default)]
    pub gpu_count: u32,
    #[serde(default)]
    pub max_workloads: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct TopologyRelation {
    pub other_target_id: String,
    pub relation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_class: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct TargetInventorySnapshot {
    pub target_id: String,
    pub observed_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<TargetCapabilityRecord>,
    #[serde(default)]
    pub capacity: ResourceCapacity,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    pub trust_zone: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub topology: Vec<TopologyRelation>,
}

impl TargetInventorySnapshot {
    pub fn validate(&self) -> ModelResult<()> {
        validate_open_text(&self.target_id, "target id")?;
        validate_open_text(&self.trust_zone, "target trust zone")?;
        let mut capabilities = BTreeSet::new();
        for capability in &self.capabilities {
            validate_open_text(&capability.capability_id, "target capability id")?;
            validate_open_text(&capability.version, "target capability version")?;
            if !capabilities.insert((&capability.capability_id, &capability.version)) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "target inventory repeats a capability and version",
                ));
            }
            for evidence in &capability.evidence_refs {
                validate_artifact_descriptor(evidence)?;
            }
        }
        for relation in &self.topology {
            validate_open_text(&relation.other_target_id, "topology target id")?;
            validate_open_text(&relation.relation_id, "topology relation id")?;
        }
        validate_portable_model(self)
    }
}

impl ArtifactModel for TargetInventorySnapshot {
    const ARTIFACT_TYPE_URI: &'static str = TARGET_INVENTORY_TYPE_URI;

    fn validate(&self) -> ModelResult<()> {
        TargetInventorySnapshot::validate(self)
    }

    fn referenced_artifacts(&self) -> Vec<&ArtifactDescriptor> {
        self.capabilities
            .iter()
            .flat_map(|capability| capability.evidence_refs.iter())
            .collect()
    }
}

fn validate_open_text(value: &str, label: &str) -> ModelResult<()> {
    if value.trim().is_empty()
        || value.chars().any(char::is_whitespace)
        || value.chars().any(char::is_control)
    {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            format!("{label} is empty or contains whitespace/control characters"),
        ));
    }
    Ok(())
}
