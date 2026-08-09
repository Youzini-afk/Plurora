use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use plurora_core::ArtifactDescriptor;

use crate::ids::{NodeId, PortId};
use crate::port::BindingPhase;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    WorkInvalid,
    WorkTooComplex,
    ArtifactBudgetExceeded,
    ArtifactMissing,
    ArtifactDigestMismatch,
    InvalidId,
    AssemblyCycle,
    PortUnresolved,
    PortIncompatible,
    BindingAmbiguous,
    BindingUnavailable,
    BindingExpired,
    UnsupportedInteraction,
    StateMigrationRequired,
    StateResetRequired,
    RightsBlocked,
    EntitlementRequired,
    AuthorityDenied,
    TargetUnsatisfied,
    PlanStale,
    PlanDigestMismatch,
    ApprovalRequired,
    OutcomeUnknown,
    RecoveryRequired,
    UnsupportedBackend,
    RawSecret,
    RawPath,
}

impl DiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkInvalid => "work_invalid",
            Self::WorkTooComplex => "work_too_complex",
            Self::ArtifactBudgetExceeded => "artifact_budget_exceeded",
            Self::ArtifactMissing => "artifact_missing",
            Self::ArtifactDigestMismatch => "artifact_digest_mismatch",
            Self::InvalidId => "invalid_id",
            Self::AssemblyCycle => "assembly_cycle",
            Self::PortUnresolved => "port_unresolved",
            Self::PortIncompatible => "port_incompatible",
            Self::BindingAmbiguous => "binding_ambiguous",
            Self::BindingUnavailable => "binding_unavailable",
            Self::BindingExpired => "binding_expired",
            Self::UnsupportedInteraction => "unsupported_interaction",
            Self::StateMigrationRequired => "state_migration_required",
            Self::StateResetRequired => "state_reset_required",
            Self::RightsBlocked => "rights_blocked",
            Self::EntitlementRequired => "entitlement_required",
            Self::AuthorityDenied => "authority_denied",
            Self::TargetUnsatisfied => "target_unsatisfied",
            Self::PlanStale => "plan_stale",
            Self::PlanDigestMismatch => "plan_digest_mismatch",
            Self::ApprovalRequired => "approval_required",
            Self::OutcomeUnknown => "outcome_unknown",
            Self::RecoveryRequired => "recovery_required",
            Self::UnsupportedBackend => "unsupported_backend",
            Self::RawSecret => "raw_secret",
            Self::RawPath => "raw_path",
        }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

impl DiagnosticSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

impl fmt::Display for DiagnosticSeverity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorkDiagnostic {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<BindingPhase>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_path: Vec<NodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_id: Option<PortId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_refs: Vec<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DiagnosticReport {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<WorkDiagnostic>,
    #[serde(default)]
    pub omitted_count: u64,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("{code}: {message}")]
pub struct ModelError {
    pub code: DiagnosticCode,
    pub message: String,
}

impl ModelError {
    pub fn new(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn diagnostic(&self) -> WorkDiagnostic {
        WorkDiagnostic {
            code: self.code,
            severity: DiagnosticSeverity::Error,
            message: self.message.clone(),
            field: None,
            phase: None,
            node_path: Vec::new(),
            port_id: None,
            candidate_refs: Vec::new(),
        }
    }
}

pub type ModelResult<T> = Result<T, ModelError>;
