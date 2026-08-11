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
use crate::lock::{AssemblyLock, ASSEMBLY_LOCK_TYPE_URI};
use crate::operational::{
    EndpointVisibility, OperationalIntent, StateDurability, TargetInventorySnapshot,
    OPERATIONAL_INTENT_TYPE_URI, TARGET_INVENTORY_TYPE_URI,
};
use crate::port::{BindingPhase, PortEndpoint, SelectedTransport};
use crate::work::WORK_REVISION_TYPE_URI;

pub const REALIZATION_PLAN_TYPE_URI: &str = "urn:plurora:realization-plan:v1";
pub const MAX_REALIZATION_PLAN_ACTIONS: usize = 4_096;
pub const EXECUTION_CLASS_CAPABILITY_PREFIX: &str = "plurora.execution-class/";

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
    pub revision: u64,
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
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activated_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped_at: Option<DateTime<Utc>>,
}

impl RealizationRevision {
    pub fn validate(&self) -> ModelResult<()> {
        if self.revision == 0 || self.updated_at < self.created_at {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "realization revision number or update timestamp is invalid",
            ));
        }
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
            || self
                .stopped_at
                .is_some_and(|stopped| stopped < self.created_at)
            || (self.status == RealizationStatus::Stopped && self.stopped_at.is_none())
            || (self.status != RealizationStatus::Stopped && self.stopped_at.is_some())
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

/// Pure, Host-supplied binding between one portable workload and a selected
/// Target/backend artifact set. The selection is input to planning and never
/// grants execution authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedWorkloadInput {
    pub workload_id: String,
    pub target_id: String,
    pub execution_class: String,
    pub launch_artifact: ArtifactDescriptor,
    pub parameter_ref: Option<ArtifactDescriptor>,
    pub build_action: Option<BuildAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RealizationPlanningGap {
    pub reason_code: String,
    pub next_step: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizationPlannerInput {
    pub installation_id: InstallationId,
    pub work_revision: ArtifactDescriptor,
    pub assembly_lock_ref: ArtifactDescriptor,
    pub operational_intent_ref: ArtifactDescriptor,
    pub inventory_refs: Vec<ArtifactDescriptor>,
    pub intent: OperationalIntent,
    pub assembly_lock: AssemblyLock,
    pub inventories: Vec<TargetInventorySnapshot>,
    pub workloads: Vec<PlannedWorkloadInput>,
    pub preconditions: Vec<ChangePrecondition>,
    pub required_authority: Vec<String>,
    pub risk_summary: Vec<String>,
}

/// Compile one exact Installation/Intent/Inventory snapshot into a stable
/// plan. This function has no filesystem, network, clock, ObjectStore, or Host
/// registry access; callers persist its result only after it succeeds.
pub fn compile_realization_plan(
    mut input: RealizationPlannerInput,
) -> Result<RealizationPlan, Vec<RealizationPlanningGap>> {
    let mut gaps = Vec::new();
    if input.intent.validate().is_err() || input.assembly_lock.validate().is_err() {
        gaps.push(RealizationPlanningGap {
            reason_code: "work_invalid".to_string(),
            next_step: "repair the OperationalIntent or AssemblyLock before planning".to_string(),
            workload_id: None,
            target_id: None,
        });
        return Err(gaps);
    }
    if input.inventory_refs.len() != input.inventories.len()
        || input
            .inventories
            .iter()
            .any(|inventory| inventory.validate().is_err())
    {
        gaps.push(RealizationPlanningGap {
            reason_code: "target_unsatisfied".to_string(),
            next_step: "refresh the selected Target inventory snapshot".to_string(),
            workload_id: None,
            target_id: None,
        });
        return Err(gaps);
    }

    input.workloads.sort_by(|left, right| {
        left.workload_id
            .cmp(&right.workload_id)
            .then_with(|| left.target_id.cmp(&right.target_id))
    });
    input
        .inventory_refs
        .sort_by(|left, right| left.digest.cmp(&right.digest));
    input
        .inventories
        .sort_by(|left, right| left.target_id.cmp(&right.target_id));
    input.required_authority.sort();
    input.required_authority.dedup();
    input.risk_summary.sort();
    input.risk_summary.dedup();
    input.preconditions.sort_by(|left, right| {
        serde_json::to_string(left)
            .unwrap_or_default()
            .cmp(&serde_json::to_string(right).unwrap_or_default())
    });

    let workload_intents = input
        .intent
        .workloads
        .iter()
        .map(|workload| (workload.workload_id.as_str(), workload))
        .collect::<BTreeMap<_, _>>();
    let lock_nodes = input
        .assembly_lock
        .nodes
        .iter()
        .map(|node| (&node.node_id, node))
        .collect::<BTreeMap<_, _>>();
    let inventories = input
        .inventories
        .iter()
        .map(|inventory| (inventory.target_id.as_str(), inventory))
        .collect::<BTreeMap<_, _>>();

    let mut placements = Vec::new();
    let mut build_actions = Vec::new();
    let mut launch_actions = Vec::new();
    let mut workload_targets = BTreeMap::new();
    let mut seen_workloads = BTreeSet::new();
    let mut used_capacity: BTreeMap<&str, ResourceRequirementsUsed> = BTreeMap::new();
    for selection in &input.workloads {
        let Some(workload) = workload_intents.get(selection.workload_id.as_str()) else {
            gaps.push(planning_gap(
                "work_invalid",
                "select a workload declared by the OperationalIntent",
                selection,
            ));
            continue;
        };
        if !seen_workloads.insert(selection.workload_id.as_str())
            || !lock_nodes.contains_key(&workload.node_id)
            || !workload
                .execution_classes
                .contains(&selection.execution_class)
        {
            gaps.push(planning_gap(
                "work_invalid",
                "repair the workload selection and AssemblyLock node binding",
                selection,
            ));
            continue;
        }
        let Some(inventory) = inventories.get(selection.target_id.as_str()) else {
            gaps.push(planning_gap(
                "target_unsatisfied",
                "select a Target with a current inventory snapshot",
                selection,
            ));
            continue;
        };
        let capability_id = format!(
            "{EXECUTION_CLASS_CAPABILITY_PREFIX}{}",
            selection.execution_class
        );
        if !inventory
            .capabilities
            .iter()
            .any(|capability| capability.available && capability.capability_id == capability_id)
        {
            gaps.push(planning_gap(
                "unsupported_backend",
                "select a Target that advertises the workload execution class",
                selection,
            ));
            continue;
        }
        let capacity = used_capacity.entry(&selection.target_id).or_default();
        capacity.add(workload);
        if !capacity.fits(inventory) {
            gaps.push(planning_gap(
                "target_unsatisfied",
                "select a Target with sufficient observed resource capacity",
                selection,
            ));
            continue;
        }
        if validate_artifact_descriptor(&selection.launch_artifact).is_err()
            || selection
                .parameter_ref
                .as_ref()
                .is_some_and(|parameter| validate_artifact_descriptor(parameter).is_err())
        {
            gaps.push(planning_gap(
                "artifact_missing",
                "supply immutable backend launch artifacts",
                selection,
            ));
            continue;
        }
        workload_targets.insert(workload.node_id.clone(), selection.target_id.clone());
        placements.push(NodePlacement {
            placement_id: format!("placement-{}", selection.workload_id),
            node_id: workload.node_id.clone(),
            target_id: selection.target_id.clone(),
            execution_class: selection.execution_class.clone(),
        });
        if let Some(build) = &selection.build_action {
            build_actions.push(build.clone());
        }
        launch_actions.push(LaunchAction {
            action_id: format!("launch-{}", selection.workload_id),
            node_id: workload.node_id.clone(),
            target_id: selection.target_id.clone(),
            artifact: selection.launch_artifact.clone(),
            parameter_ref: selection.parameter_ref.clone(),
        });
    }
    for workload in &input.intent.workloads {
        if !seen_workloads.contains(workload.workload_id.as_str()) {
            gaps.push(RealizationPlanningGap {
                reason_code: "target_unsatisfied".to_string(),
                next_step: "select a Target and backend for every workload".to_string(),
                workload_id: Some(workload.workload_id.clone()),
                target_id: None,
            });
        }
    }
    if !gaps.is_empty() {
        gaps.sort_by(|left, right| {
            left.workload_id
                .cmp(&right.workload_id)
                .then_with(|| left.target_id.cmp(&right.target_id))
                .then_with(|| left.reason_code.cmp(&right.reason_code))
        });
        return Err(gaps);
    }

    let default_target = placements
        .first()
        .map(|placement| placement.target_id.clone())
        .unwrap_or_default();
    let mut state_actions = input
        .intent
        .state
        .iter()
        .filter(|state| state.durability != StateDurability::Ephemeral)
        .map(|state| StateAction {
            action_id: format!("state-{}", state.state_slot_id),
            state_slot_id: state.state_slot_id.clone(),
            target_id: default_target.clone(),
            kind: StateActionKind::Provision,
            input_ref: None,
        })
        .collect::<Vec<_>>();
    state_actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let mut endpoint_actions = input
        .intent
        .endpoints
        .iter()
        .map(|endpoint| EndpointAction {
            action_id: format!("endpoint-{}", endpoint.endpoint_id),
            endpoint_id: endpoint.endpoint_id.clone(),
            target_id: workload_targets
                .get(&endpoint.port.node_id)
                .cloned()
                .unwrap_or_else(|| default_target.clone()),
            visibility: endpoint.visibility,
            transport: SelectedTransport {
                class_id: "host.proxy/http-v1".to_string(),
                properties: BTreeMap::new(),
            },
        })
        .collect::<Vec<_>>();
    endpoint_actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));

    let plan = RealizationPlan {
        schema: RealizationPlan::SCHEMA.to_string(),
        installation_id: input.installation_id,
        work_revision: input.work_revision,
        assembly_lock: input.assembly_lock_ref,
        operational_intent: input.operational_intent_ref,
        inventory_refs: input.inventory_refs,
        placements,
        transports: Vec::new(),
        build_actions,
        launch_actions,
        state_actions,
        endpoint_actions,
        preconditions: input.preconditions,
        required_authority: input.required_authority,
        risk_summary: input.risk_summary,
    };
    plan.validate().map_err(|_| {
        vec![RealizationPlanningGap {
            reason_code: "work_invalid".to_string(),
            next_step: "repair the generated plan inputs".to_string(),
            workload_id: None,
            target_id: None,
        }]
    })?;
    Ok(plan)
}

#[derive(Debug, Default)]
struct ResourceRequirementsUsed {
    cpu_millis: u64,
    memory_bytes: u64,
    storage_bytes: u64,
    gpu_count: u32,
    workloads: u32,
}

impl ResourceRequirementsUsed {
    fn add(&mut self, workload: &crate::operational::WorkloadIntent) {
        let replicas = u64::from(workload.replicas.min);
        self.cpu_millis = self.cpu_millis.saturating_add(
            workload
                .resources
                .cpu_millis
                .unwrap_or_default()
                .saturating_mul(replicas),
        );
        self.memory_bytes = self.memory_bytes.saturating_add(
            workload
                .resources
                .memory_bytes
                .unwrap_or_default()
                .saturating_mul(replicas),
        );
        self.storage_bytes = self.storage_bytes.saturating_add(
            workload
                .resources
                .storage_bytes
                .unwrap_or_default()
                .saturating_mul(replicas),
        );
        self.gpu_count = self.gpu_count.saturating_add(
            workload
                .resources
                .gpu_count
                .unwrap_or_default()
                .saturating_mul(u32::from(workload.replicas.min)),
        );
        self.workloads = self
            .workloads
            .saturating_add(u32::from(workload.replicas.min));
    }

    fn fits(&self, inventory: &TargetInventorySnapshot) -> bool {
        (inventory.capacity.cpu_millis == 0 || self.cpu_millis <= inventory.capacity.cpu_millis)
            && (inventory.capacity.memory_bytes == 0
                || self.memory_bytes <= inventory.capacity.memory_bytes)
            && (inventory.capacity.storage_bytes == 0
                || self.storage_bytes <= inventory.capacity.storage_bytes)
            && (inventory.capacity.gpu_count == 0 || self.gpu_count <= inventory.capacity.gpu_count)
            && (inventory.capacity.max_workloads == 0
                || self.workloads <= inventory.capacity.max_workloads)
    }
}

fn planning_gap(
    reason_code: &str,
    next_step: &str,
    selection: &PlannedWorkloadInput,
) -> RealizationPlanningGap {
    RealizationPlanningGap {
        reason_code: reason_code.to_string(),
        next_step: next_step.to_string(),
        workload_id: Some(selection.workload_id.clone()),
        target_id: Some(selection.target_id.clone()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        OperationalIntent, ReplicaPolicy, ResourceCapacity, ResourceRequirements, RestartPolicy,
        TargetCapabilityRecord, UpdatePolicy, WorkloadImports, WorkloadIntent,
        ASSEMBLY_REVISION_TYPE_URI,
    };

    fn descriptor(kind: &str, marker: char) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: kind.to_string(),
            media_type: crate::CANONICAL_JSON_MEDIA_TYPE.to_string(),
            digest: format!("sha256:{}", marker.to_string().repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    fn planner_input() -> RealizationPlannerInput {
        let node_id = NodeId::parse("server").unwrap();
        let target_id = "local".to_string();
        RealizationPlannerInput {
            installation_id: InstallationId::parse("11111111-1111-4111-8111-111111111111").unwrap(),
            work_revision: descriptor(WORK_REVISION_TYPE_URI, 'a'),
            assembly_lock_ref: descriptor(ASSEMBLY_LOCK_TYPE_URI, 'b'),
            operational_intent_ref: descriptor(OPERATIONAL_INTENT_TYPE_URI, 'c'),
            inventory_refs: vec![descriptor(TARGET_INVENTORY_TYPE_URI, 'd')],
            intent: OperationalIntent {
                schema: OperationalIntent::SCHEMA.to_string(),
                workloads: vec![WorkloadIntent {
                    workload_id: "api".to_string(),
                    node_id: node_id.clone(),
                    execution_classes: vec!["oci-container.v1".to_string()],
                    resources: ResourceRequirements::default(),
                    imports: WorkloadImports::default(),
                    replicas: ReplicaPolicy { min: 1, max: 1 },
                    restart_policy: RestartPolicy::OnFailure,
                    health_port: None,
                    annotations: BTreeMap::new(),
                }],
                endpoints: Vec::new(),
                state: Vec::new(),
                placement: Vec::new(),
                update_policy: UpdatePolicy::default(),
                annotations: BTreeMap::new(),
            },
            assembly_lock: AssemblyLock {
                schema: AssemblyLock::SCHEMA.to_string(),
                assembly: descriptor(ASSEMBLY_REVISION_TYPE_URI, 'e'),
                nodes: vec![crate::NodeLock {
                    node_id: node_id.clone(),
                    artifact: descriptor(ASSEMBLY_LOCK_TYPE_URI, 'f'),
                    behavior_digest: None,
                    trust_class: None,
                }],
                bindings: Vec::new(),
                protocol_profiles: Vec::new(),
                content_roots: Vec::new(),
            },
            inventories: vec![TargetInventorySnapshot {
                target_id: target_id.clone(),
                observed_at: Utc::now(),
                capabilities: vec![TargetCapabilityRecord {
                    capability_id: format!("{EXECUTION_CLASS_CAPABILITY_PREFIX}oci-container.v1"),
                    version: "1".to_string(),
                    properties: BTreeMap::new(),
                    available: true,
                    evidence_refs: Vec::new(),
                }],
                capacity: ResourceCapacity::default(),
                labels: BTreeMap::new(),
                trust_zone: "host-local".to_string(),
                topology: Vec::new(),
            }],
            workloads: vec![PlannedWorkloadInput {
                workload_id: "api".to_string(),
                target_id,
                execution_class: "oci-container.v1".to_string(),
                launch_artifact: descriptor(
                    "urn:plurora:realization-backend:oci-image-reference:v1",
                    '9',
                ),
                parameter_ref: None,
                build_action: None,
            }],
            preconditions: Vec::new(),
            required_authority: vec!["realization.apply".to_string()],
            risk_summary: vec!["managed_target_effects".to_string()],
        }
    }

    #[test]
    fn pure_planner_is_stable_and_has_no_host_effect_inputs() {
        let input = planner_input();
        let first = compile_realization_plan(input.clone()).expect("plan");
        let second = compile_realization_plan(input).expect("same plan");
        assert_eq!(first, second);
        assert_eq!(first.placements.len(), 1);
        assert_eq!(first.launch_actions.len(), 1);
        assert_eq!(first.required_authority, vec!["realization.apply"]);
    }

    #[test]
    fn planner_fails_closed_for_missing_target_or_lock_node() {
        let mut no_target = planner_input();
        no_target.inventories.clear();
        assert_eq!(
            compile_realization_plan(no_target)
                .expect_err("missing inventory")
                .first()
                .map(|gap| gap.reason_code.as_str()),
            Some("target_unsatisfied")
        );
        let mut no_node = planner_input();
        no_node.assembly_lock.nodes.clear();
        assert_eq!(
            compile_realization_plan(no_node)
                .expect_err("missing lock node")
                .first()
                .map(|gap| gap.reason_code.as_str()),
            Some("work_invalid")
        );
    }
}
