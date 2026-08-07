use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ArtifactDescriptor;

pub const EFFECT_RECEIPT_TYPE_URI: &str = "urn:plurora:effect-receipt:v1";
pub const EFFECT_VALUE_TYPE_URI: &str = "urn:plurora:effect-value:json:v1";
pub const COMPONENT_EVIDENCE_TYPE_URI: &str = "urn:plurora:component-evidence:v1";
pub const AUTHORITY_EVIDENCE_TYPE_URI: &str = "urn:plurora:authority-evidence:v1";
pub const POLICY_DECISION_TYPE_URI: &str = "urn:plurora:policy-decision:v1";
pub const APPROVAL_EVIDENCE_TYPE_URI: &str = "urn:plurora:approval-evidence:v1";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PrincipalIdentity {
    Kernel,
    HostAdmin,
    HostDev,
    Package {
        package_id: String,
    },
    Human {
        user_id: String,
    },
    Assistant {
        assistant_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delegated_user_id: Option<String>,
    },
    Anonymous,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EffectTerminalStatus {
    Succeeded,
    Denied,
    Failed,
    Cancelled,
    TimedOut,
    Partial,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EffectReplayMode {
    Live,
    Historical,
    Reexecute,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct EffectScope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct EffectReceipt {
    pub schema_version: u16,
    pub receipt_type_uri: String,
    pub receipt_id: String,
    pub effect_kind: String,
    pub principal: PrincipalIdentity,
    pub component_ref: ArtifactDescriptor,
    #[serde(default)]
    pub protocol_profiles: Vec<String>,
    #[serde(default)]
    pub input_refs: Vec<ArtifactDescriptor>,
    #[serde(default)]
    pub output_refs: Vec<ArtifactDescriptor>,
    #[serde(default)]
    pub external_effect_refs: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority_ref: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_decision_ref: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_ref: Option<ArtifactDescriptor>,
    pub status: EffectTerminalStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub latency_ms: u64,
    #[serde(default)]
    pub usage: Value,
    #[serde(default)]
    pub cost: Value,
    pub trace_id: String,
    #[serde(default)]
    pub parent_receipts: Vec<String>,
    pub replay_mode: EffectReplayMode,
    #[serde(default)]
    pub scope: EffectScope,
    #[serde(default)]
    pub planned: Value,
    #[serde(default)]
    pub actual: Value,
    #[serde(default)]
    pub annotations: BTreeMap<String, Value>,
}

impl EffectReceipt {
    pub fn referenced_digests(&self) -> Vec<String> {
        let mut digests = Vec::new();
        digests.push(self.component_ref.digest.clone());
        digests.extend(self.input_refs.iter().map(|item| item.digest.clone()));
        digests.extend(self.output_refs.iter().map(|item| item.digest.clone()));
        digests.extend(
            self.external_effect_refs
                .iter()
                .map(|item| item.digest.clone()),
        );
        digests.extend(
            [
                self.authority_ref.as_ref(),
                self.policy_decision_ref.as_ref(),
                self.approval_ref.as_ref(),
            ]
            .into_iter()
            .flatten()
            .map(|item| item.digest.clone()),
        );
        digests.extend(self.parent_receipts.iter().cloned());
        digests.sort();
        digests.dedup();
        digests
    }
}
