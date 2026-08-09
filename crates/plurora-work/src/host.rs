use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use plurora_core::{ArtifactDescriptor, SecretRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::canonical::{
    fixed_u16_schema, validate_artifact_descriptor, validate_descriptor_type,
    validate_portable_model,
};
use crate::diagnostic::{DiagnosticCode, ModelError, ModelResult};
use crate::ids::{
    BindingId, ExposureId, InstallationId, NodeId, PortId, RealizationId, RunId, StateSlotId,
};
use crate::lock::ASSEMBLY_LOCK_TYPE_URI;
use crate::port::SelectedTransport;
use crate::work::WORK_REVISION_TYPE_URI;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionKind {
    WorkBundle,
    Package,
    GitSnapshot,
    LocalImport,
    RemoteCatalog,
    ForeignBinding,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AcquisitionRecord {
    pub kind: AcquisitionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance_refs: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_channel: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StateBindingKind {
    HostManaged,
    ExternalProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct StateBindingRecord {
    pub state_slot_id: StateSlotId,
    pub kind: StateBindingKind,
    pub binding_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_ref: Option<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct InstallationSecretPolicy {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_secret_refs: Vec<String>,
    #[serde(default)]
    pub allow_platform_fallback: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallationStatus {
    Resolving,
    Ready,
    Updating,
    Blocked,
    Failed,
    Removing,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct InstallationRecord {
    #[schemars(schema_with = "installation_schema_version")]
    pub schema_version: u16,
    pub installation_id: InstallationId,
    pub work_revision: ArtifactDescriptor,
    pub assembly_lock: ArtifactDescriptor,
    pub display_name: String,
    pub source: AcquisitionRecord,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_bindings: Vec<StateBindingRecord>,
    #[serde(default)]
    pub secret_policy: InstallationSecretPolicy,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: InstallationStatus,
}

fn installation_schema_version(
    _generator: &mut schemars::gen::SchemaGenerator,
) -> schemars::schema::Schema {
    fixed_u16_schema(InstallationRecord::SCHEMA_VERSION)
}

impl InstallationRecord {
    pub const SCHEMA_VERSION: u16 = 1;

    pub fn validate(&self) -> ModelResult<()> {
        if self.schema_version != Self::SCHEMA_VERSION || self.display_name.trim().is_empty() {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "installation record schema or display name is invalid",
            ));
        }
        validate_descriptor_type(&self.work_revision, WORK_REVISION_TYPE_URI)?;
        validate_descriptor_type(&self.assembly_lock, ASSEMBLY_LOCK_TYPE_URI)?;
        if self.updated_at < self.created_at {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "installation update time precedes its creation time",
            ));
        }
        if let Some(source) = &self.source.source_ref {
            validate_artifact_descriptor(source)?;
        }
        for provenance in &self.source.provenance_refs {
            validate_artifact_descriptor(provenance)?;
        }
        let mut slots = BTreeSet::new();
        for binding in &self.state_bindings {
            if binding.binding_id.trim().is_empty() || !slots.insert(&binding.state_slot_id) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "installation contains an invalid or duplicate state binding",
                ));
            }
            if let Some(provider) = &binding.provider_ref {
                validate_artifact_descriptor(provider)?;
            }
        }
        let mut secret_refs = BTreeSet::new();
        for reference in &self.secret_policy.allowed_secret_refs {
            if !SecretRef::is_valid_ref(reference) {
                return Err(ModelError::new(
                    DiagnosticCode::RawSecret,
                    "installation secret policy contains a value instead of a secret reference",
                ));
            }
            if !secret_refs.insert(reference.as_str()) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "installation secret policy contains a duplicate secret reference",
                ));
            }
        }
        validate_portable_model(self)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeInstanceStatus {
    Starting,
    Running,
    Degraded,
    Stopping,
    Stopped,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct NodeInstanceRecord {
    pub instance_id: String,
    pub node_id: NodeId,
    pub status: NodeInstanceStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub realization_id: Option<RealizationId>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Starting,
    Running,
    Degraded,
    Stopping,
    Stopped,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Unknown,
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RunHealth {
    pub status: HealthStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostic_refs: Vec<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ActiveBindingRecord {
    pub binding_id: BindingId,
    pub consumer_installation_id: InstallationId,
    pub consumer_port: PortId,
    pub exposure_id: ExposureId,
    pub authority_handle_id: String,
    pub transport: SelectedTransport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

impl ActiveBindingRecord {
    pub fn validate(&self) -> ModelResult<()> {
        if self.authority_handle_id.trim().is_empty() {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "active binding is missing its authority handle id",
            ));
        }
        self.transport.validate()?;
        validate_portable_model(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RunRecord {
    pub run_id: RunId,
    pub installation_id: InstallationId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
    pub status: RunStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_instances: Vec<NodeInstanceRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<ActiveBindingRecord>,
    pub started_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped_at: Option<DateTime<Utc>>,
    pub health: RunHealth,
}

impl RunRecord {
    pub fn validate(&self) -> ModelResult<()> {
        let active = matches!(
            self.status,
            RunStatus::Starting | RunStatus::Running | RunStatus::Degraded | RunStatus::Stopping
        );
        let terminal = matches!(
            self.status,
            RunStatus::Stopped | RunStatus::Failed | RunStatus::Interrupted
        );
        if (active && self.stopped_at.is_some())
            || (terminal && self.stopped_at.is_none())
            || self
                .stopped_at
                .is_some_and(|stopped| stopped < self.started_at)
        {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "run status and lifecycle timestamps are inconsistent",
            ));
        }
        let mut instance_ids = BTreeSet::new();
        for instance in &self.node_instances {
            if instance.instance_id.trim().is_empty()
                || !instance_ids.insert(instance.instance_id.as_str())
            {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "run contains an invalid or duplicate node instance id",
                ));
            }
        }
        let mut binding_ids = BTreeSet::new();
        for binding in &self.bindings {
            binding.validate()?;
            if !binding_ids.insert(&binding.binding_id) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "run contains a duplicate active binding id",
                ));
            }
        }
        for diagnostic in &self.health.diagnostic_refs {
            validate_artifact_descriptor(diagnostic)?;
        }
        validate_portable_model(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ResourceSelector {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExposureStatus {
    Active,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExposureRecord {
    pub exposure_id: ExposureId,
    pub installation_id: InstallationId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    pub export_port: PortId,
    pub audience: Vec<ResourceSelector>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    pub status: ExposureStatus,
}

impl ExposureRecord {
    pub fn validate(&self) -> ModelResult<()> {
        if self.audience.is_empty()
            || self
                .audience
                .iter()
                .any(|selector| selector.kind.trim().is_empty() || selector.id.trim().is_empty())
        {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "exposure must declare a non-empty exact audience",
            ));
        }
        validate_portable_model(self)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

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

    fn installation() -> InstallationRecord {
        let created_at = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        InstallationRecord {
            schema_version: InstallationRecord::SCHEMA_VERSION,
            installation_id: InstallationId::new(),
            work_revision: descriptor(WORK_REVISION_TYPE_URI, 'a'),
            assembly_lock: descriptor(ASSEMBLY_LOCK_TYPE_URI, 'b'),
            display_name: "Example".to_string(),
            source: AcquisitionRecord {
                kind: AcquisitionKind::WorkBundle,
                source_ref: None,
                provenance_refs: Vec::new(),
                update_channel: None,
            },
            state_bindings: Vec::new(),
            secret_policy: InstallationSecretPolicy::default(),
            created_at,
            updated_at: created_at,
            status: InstallationStatus::Ready,
        }
    }

    #[test]
    fn installation_rejects_plaintext_in_secret_reference_policy() {
        let mut value = installation();
        value
            .secret_policy
            .allowed_secret_refs
            .push("hunter2".to_string());
        assert_eq!(
            value.validate().unwrap_err().code,
            DiagnosticCode::RawSecret
        );

        value.secret_policy.allowed_secret_refs = vec!["secret_ref:store:EXAMPLE_KEY".to_string()];
        value.validate().unwrap();
    }

    #[test]
    fn running_record_cannot_claim_a_stop_time() {
        let started_at = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let run = RunRecord {
            run_id: RunId::new(),
            installation_id: InstallationId::new(),
            context_id: None,
            status: RunStatus::Running,
            node_instances: Vec::new(),
            bindings: Vec::new(),
            started_at,
            stopped_at: Some(started_at),
            health: RunHealth {
                status: HealthStatus::Healthy,
                reason_code: None,
                diagnostic_refs: Vec::new(),
            },
        };
        assert_eq!(
            run.validate().unwrap_err().code,
            DiagnosticCode::WorkInvalid
        );
    }

    #[test]
    fn terminal_run_record_requires_a_stop_time() {
        let started_at = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        for status in [
            RunStatus::Stopped,
            RunStatus::Failed,
            RunStatus::Interrupted,
        ] {
            let run = RunRecord {
                run_id: RunId::new(),
                installation_id: InstallationId::new(),
                context_id: None,
                status,
                node_instances: Vec::new(),
                bindings: Vec::new(),
                started_at,
                stopped_at: None,
                health: RunHealth {
                    status: HealthStatus::Unhealthy,
                    reason_code: None,
                    diagnostic_refs: Vec::new(),
                },
            };
            assert_eq!(
                run.validate().unwrap_err().code,
                DiagnosticCode::WorkInvalid
            );
        }
    }
}
