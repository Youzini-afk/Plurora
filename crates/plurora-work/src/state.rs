use std::collections::BTreeMap;

use plurora_core::ArtifactDescriptor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canonical::{validate_artifact_descriptor, validate_portable_model};
use crate::diagnostic::{DiagnosticCode, ModelError, ModelResult};
use crate::ids::{NodeId, StateSlotId};
use crate::port::PortEndpoint;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StateScope {
    Run,
    Installation,
    User,
    Shared,
    External,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StatePortability {
    Portable,
    OpaqueExportable,
    HostBound,
    ExternalAuthority,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackupPolicy {
    Required,
    Allowed,
    Forbidden,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct StateSlotDescriptor {
    pub state_slot_id: StateSlotId,
    pub owner_node_id: NodeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_ref: Option<ArtifactDescriptor>,
    pub scope: StateScope,
    pub portability: StatePortability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_port: Option<PortEndpoint>,
    pub backup_policy: BackupPolicy,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl StateSlotDescriptor {
    pub fn validate(&self) -> ModelResult<()> {
        if self.portability == StatePortability::Portable && self.schema_ref.is_none() {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "portable state slot must reference a schema artifact",
            ));
        }
        if let Some(schema) = &self.schema_ref {
            validate_artifact_descriptor(schema)?;
        }
        validate_portable_model(&self.annotations)
    }

    pub fn is_durable(&self) -> bool {
        self.scope != StateScope::Run
    }
}

pub fn validate_state_replacement(
    current: &StateSlotDescriptor,
    candidate: &StateSlotDescriptor,
    reset_approved: bool,
) -> ModelResult<()> {
    current.validate()?;
    candidate.validate()?;
    if !current.is_durable() {
        return Ok(());
    }
    let current_schema = current
        .schema_ref
        .as_ref()
        .map(|value| value.digest.as_str());
    let candidate_schema = candidate
        .schema_ref
        .as_ref()
        .map(|value| value.digest.as_str());
    let compatible = current.owner_node_id == candidate.owner_node_id
        && current_schema == candidate_schema
        && current.portability == candidate.portability;
    if !compatible && candidate.migration_port.is_none() && !reset_approved {
        return Err(ModelError::new(
            DiagnosticCode::StateMigrationRequired,
            "durable state replacement requires a migration port or an explicit reset decision",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_state_requires_a_schema() {
        let slot = StateSlotDescriptor {
            state_slot_id: StateSlotId::parse("save").unwrap(),
            owner_node_id: NodeId::parse("simulation").unwrap(),
            schema_ref: None,
            scope: StateScope::User,
            portability: StatePortability::Portable,
            migration_port: None,
            backup_policy: BackupPolicy::Required,
            annotations: BTreeMap::new(),
        };
        assert_eq!(
            slot.validate().unwrap_err().code,
            DiagnosticCode::WorkInvalid
        );
    }
}
