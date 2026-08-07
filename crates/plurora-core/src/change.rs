use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ArtifactDescriptor, PrincipalIdentity};

pub const INTENT_TYPE_URI: &str = "urn:plurora:intent:v1";
pub const CHANGE_SET_TYPE_URI: &str = "urn:plurora:change-set:v1";
pub const CHANGE_COMMIT_TYPE_URI: &str = "urn:plurora:change-commit:v1";

fn default_intent_type_uri() -> String {
    INTENT_TYPE_URI.to_string()
}

fn default_change_set_type_uri() -> String {
    CHANGE_SET_TYPE_URI.to_string()
}

fn default_policy_decision_type_uri() -> String {
    crate::POLICY_DECISION_TYPE_URI.to_string()
}

fn default_change_commit_type_uri() -> String {
    CHANGE_COMMIT_TYPE_URI.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Intent {
    pub id: String,
    #[serde(default = "default_intent_type_uri")]
    pub intent_type_uri: String,
    pub principal: PrincipalIdentity,
    #[serde(default)]
    pub goal: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_branch_id: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub annotations: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ChangeOperation {
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default)]
    pub input_refs: Vec<ArtifactDescriptor>,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ChangePrecondition {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default)]
    pub expected: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ChangeSet {
    pub id: String,
    #[serde(default = "default_change_set_type_uri")]
    pub change_set_type_uri: String,
    pub intent_id: String,
    #[serde(default)]
    pub operations: Vec<ChangeOperation>,
    #[serde(default)]
    pub preconditions: Vec<ChangePrecondition>,
    #[serde(default)]
    pub required_authority: Vec<String>,
    #[serde(default)]
    pub expected_effects: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecisionOutcome {
    Allowed,
    Denied,
    RequiresApproval,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PolicyDecision {
    pub id: String,
    #[serde(default = "default_policy_decision_type_uri")]
    pub decision_type_uri: String,
    pub change_set_id: String,
    pub outcome: PolicyDecisionOutcome,
    pub principal: PrincipalIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default)]
    pub evaluated_authority: Vec<String>,
    pub decided_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_ref: Option<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeCommitStatus {
    Committed,
    Failed,
    Partial,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ChangeCommit {
    pub id: String,
    #[serde(default = "default_change_commit_type_uri")]
    pub commit_type_uri: String,
    pub change_set_id: String,
    pub status: ChangeCommitStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    #[serde(default)]
    pub operation_receipts: Vec<ArtifactDescriptor>,
    #[serde(default)]
    pub result_refs: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}
