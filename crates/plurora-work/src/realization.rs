use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use plurora_core::{ArtifactDescriptor, ChangePrecondition};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canonical::{
    fixed_string_schema, validate_artifact_descriptor, validate_descriptor_type,
    validate_portable_model, ArtifactModel,
};
use crate::diagnostic::{DiagnosticCode, ModelError, ModelResult};
use crate::host::HealthStatus;
use crate::ids::{validate_local_id, InstallationId, NodeId, RealizationId, StateSlotId};
use crate::lock::ASSEMBLY_LOCK_TYPE_URI;
use crate::operational::{
    EndpointVisibility, OPERATIONAL_INTENT_TYPE_URI, TARGET_INVENTORY_TYPE_URI,
};
use crate::port::{BindingPhase, PortEndpoint, SelectedTransport};
use crate::work::WORK_REVISION_TYPE_URI;

pub const REALIZATION_PLAN_TYPE_URI: &str = "urn:plurora:realization-plan:v1";
pub const MAX_REALIZATION_PLAN_ACTIONS: usize = 4_096;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct NodePlacement {
    pub placement_id: String,
    pub node_id: NodeId,
    pub target_id: String,
    pub execution_class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct TransportBindingPlan {
    pub binding_id: String,
    pub provider: PortEndpoint,
    pub consumer: PortEndpoint,
    pub provider_target_id: String,
    pub consumer_target_id: String,
    pub transport: SelectedTransport,
    pub phase: BindingPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct BuildAction {
    pub action_id: String,
    pub node_id: NodeId,
    pub builder_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_refs: Vec<ArtifactDescriptor>,
    pub parameter_ref: ArtifactDescriptor,
    pub output_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LaunchAction {
    pub action_id: String,
    pub node_id: NodeId,
    pub target_id: String,
    pub artifact: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_ref: Option<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StateActionKind {
    Provision,
    Attach,
    Snapshot,
    Restore,
    Migrate,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct StateAction {
    pub action_id: String,
    pub state_slot_id: StateSlotId,
    pub target_id: String,
    pub kind: StateActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_ref: Option<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct EndpointAction {
    pub action_id: String,
    pub endpoint_id: String,
    pub target_id: String,
    pub visibility: EndpointVisibility,
    pub transport: SelectedTransport,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RealizationPlan {
    #[schemars(schema_with = "realization_plan_schema")]
    pub schema: String,
    pub installation_id: InstallationId,
    pub work_revision: ArtifactDescriptor,
    pub assembly_lock: ArtifactDescriptor,
    pub operational_intent: ArtifactDescriptor,
    pub inventory_refs: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placements: Vec<NodePlacement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transports: Vec<TransportBindingPlan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub build_actions: Vec<BuildAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub launch_actions: Vec<LaunchAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_actions: Vec<StateAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub endpoint_actions: Vec<EndpointAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preconditions: Vec<ChangePrecondition>,
    pub required_authority: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risk_summary: Vec<String>,
}

fn realization_plan_schema(
    _generator: &mut schemars::gen::SchemaGenerator,
) -> schemars::schema::Schema {
    fixed_string_schema(RealizationPlan::SCHEMA)
}

impl RealizationPlan {
    pub const SCHEMA: &'static str = "plurora.realization-plan.v1";

    pub fn validate(&self) -> ModelResult<()> {
        if self.schema != Self::SCHEMA {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "realization plan uses an unsupported schema",
            ));
        }
        validate_descriptor_type(&self.work_revision, WORK_REVISION_TYPE_URI)?;
        validate_descriptor_type(&self.assembly_lock, ASSEMBLY_LOCK_TYPE_URI)?;
        validate_descriptor_type(&self.operational_intent, OPERATIONAL_INTENT_TYPE_URI)?;
        if self.inventory_refs.is_empty() {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "realization plan must reference at least one target inventory snapshot",
            ));
        }
        for inventory in &self.inventory_refs {
            validate_descriptor_type(inventory, TARGET_INVENTORY_TYPE_URI)?;
        }
        let action_count = self.build_actions.len()
            + self.launch_actions.len()
            + self.state_actions.len()
            + self.endpoint_actions.len();
        if action_count > MAX_REALIZATION_PLAN_ACTIONS {
            return Err(ModelError::new(
                DiagnosticCode::WorkTooComplex,
                "realization plan exceeds the 4,096 action implementation budget",
            ));
        }
        let mut placement_ids = BTreeSet::new();
        for placement in &self.placements {
            validate_local_id(&placement.placement_id, "placement id")?;
            validate_open_text(&placement.target_id, "placement target id")?;
            validate_open_text(&placement.execution_class, "placement execution class")?;
            if !placement_ids.insert(placement.placement_id.as_str()) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "realization plan contains a duplicate placement id",
                ));
            }
        }
        let mut binding_ids = BTreeSet::new();
        for binding in &self.transports {
            validate_local_id(&binding.binding_id, "transport binding id")?;
            binding.transport.validate()?;
            if !binding_ids.insert(binding.binding_id.as_str()) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "realization plan contains a duplicate transport binding id",
                ));
            }
        }
        let mut action_ids = BTreeSet::new();
        for action in &self.build_actions {
            validate_action_id(&action.action_id, &mut action_ids)?;
            validate_open_text(&action.builder_id, "builder id")?;
            validate_local_id(&action.output_id, "build output id")?;
            for input in &action.input_refs {
                validate_artifact_descriptor(input)?;
            }
            validate_artifact_descriptor(&action.parameter_ref)?;
        }
        for action in &self.launch_actions {
            validate_action_id(&action.action_id, &mut action_ids)?;
            validate_open_text(&action.target_id, "launch target id")?;
            validate_artifact_descriptor(&action.artifact)?;
            if let Some(parameter) = &action.parameter_ref {
                validate_artifact_descriptor(parameter)?;
            }
        }
        for action in &self.state_actions {
            validate_action_id(&action.action_id, &mut action_ids)?;
            validate_open_text(&action.target_id, "state target id")?;
            if let Some(input) = &action.input_ref {
                validate_artifact_descriptor(input)?;
            }
        }
        for action in &self.endpoint_actions {
            validate_action_id(&action.action_id, &mut action_ids)?;
            validate_local_id(&action.endpoint_id, "endpoint action endpoint id")?;
            validate_open_text(&action.target_id, "endpoint target id")?;
            action.transport.validate()?;
        }
        if self
            .required_authority
            .iter()
            .any(|authority| authority.trim().is_empty())
            || self
                .required_authority
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != self.required_authority.len()
        {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "realization plan must declare a unique non-empty authority set",
            ));
        }
        validate_portable_model(self)
    }
}

impl ArtifactModel for RealizationPlan {
    const ARTIFACT_TYPE_URI: &'static str = REALIZATION_PLAN_TYPE_URI;

    fn validate(&self) -> ModelResult<()> {
        RealizationPlan::validate(self)
    }

    fn referenced_artifacts(&self) -> Vec<&ArtifactDescriptor> {
        let mut references = vec![
            &self.work_revision,
            &self.assembly_lock,
            &self.operational_intent,
        ];
        references.extend(&self.inventory_refs);
        references.extend(
            self.build_actions
                .iter()
                .flat_map(|action| action.input_refs.iter().chain([&action.parameter_ref])),
        );
        for action in &self.launch_actions {
            references.push(&action.artifact);
            references.extend(action.parameter_ref.iter());
        }
        references.extend(
            self.state_actions
                .iter()
                .filter_map(|action| action.input_ref.as_ref()),
        );
        references
    }
}

fn validate_action_id<'a>(
    action_id: &'a str,
    action_ids: &mut BTreeSet<&'a str>,
) -> ModelResult<()> {
    validate_local_id(action_id, "realization action id")?;
    if !action_ids.insert(action_id) {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            "realization plan contains a duplicate action id",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RealizationStatus {
    Planned,
    Applying,
    Active,
    Degraded,
    Stopping,
    Stopped,
    Failed,
    OutcomeUnknown,
    RecoveryRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RealizedResource {
    pub resource_id: String,
    pub resource_type: String,
    pub target_id: String,
    pub backend_id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_ref: Option<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RealizationHealth {
    pub status: HealthStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RealizationRevision {
    pub realization_id: RealizationId,
    pub installation_id: InstallationId,
    pub plan_ref: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_realization_id: Option<RealizationId>,
    pub status: RealizationStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actual_resources: Vec<RealizedResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipts: Vec<ArtifactDescriptor>,
    pub health: RealizationHealth,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activated_at: Option<DateTime<Utc>>,
}

impl RealizationRevision {
    pub fn validate(&self) -> ModelResult<()> {
        validate_descriptor_type(&self.plan_ref, REALIZATION_PLAN_TYPE_URI)?;
        let requires_activation = matches!(
            self.status,
            RealizationStatus::Active
                | RealizationStatus::Degraded
                | RealizationStatus::Stopping
                | RealizationStatus::Stopped
        );
        let is_pre_activation = matches!(
            self.status,
            RealizationStatus::Planned | RealizationStatus::Applying
        );
        if (requires_activation && self.activated_at.is_none())
            || (is_pre_activation && self.activated_at.is_some())
            || self
                .activated_at
                .is_some_and(|activated| activated < self.created_at)
        {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "realization status and lifecycle timestamps are inconsistent",
            ));
        }
        let mut resource_ids = BTreeSet::new();
        for resource in &self.actual_resources {
            if resource.resource_id.trim().is_empty()
                || resource.resource_type.trim().is_empty()
                || resource.target_id.trim().is_empty()
                || resource.backend_id.trim().is_empty()
                || !resource_ids.insert(resource.resource_id.as_str())
            {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "realization revision contains an invalid or duplicate actual resource",
                ));
            }
            if let Some(receipt) = &resource.receipt_ref {
                validate_artifact_descriptor(receipt)?;
            }
        }
        for receipt in self.receipts.iter().chain(&self.health.evidence_refs) {
            validate_artifact_descriptor(receipt)?;
        }
        validate_portable_model(self)
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
