use chrono::Utc;
use plurora_core::{
    new_id, ArtifactDescriptor, ChangeCommit, ChangeCommitStatus, ChangeOperation, ChangeSet,
    EffectReplayMode, EffectScope, EffectTerminalStatus, Intent, PolicyDecision,
    PolicyDecisionOutcome, SessionId, CHANGE_COMMIT_TYPE_URI, CHANGE_SET_TYPE_URI,
    EVENT_PROPOSAL_APPLIED, EVENT_PROPOSAL_APPROVED, EVENT_PROPOSAL_CREATED, EVENT_PROPOSAL_FAILED,
    EVENT_PROPOSAL_REJECTED, INTENT_TYPE_URI, KERNEL_PACKAGE_ID, POLICY_DECISION_TYPE_URI,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::effects::{principal_identity, EffectReceiptRequest};
use super::Runtime;
use crate::{
    redaction, sha256_digest, EventStore, ProtocolContext, ProtocolPrincipal,
    DEFAULT_CONTRACT_PROFILE,
};

const PROPOSAL_APPROVE_PERMISSION: &str = "change.proposal.approve";
const PROPOSAL_REJECT_PERMISSION: &str = "change.proposal.reject";
const PROPOSAL_APPLY_PERMISSION: &str = "change.proposal.apply";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Created,
    Approved,
    Applying,
    Rejected,
    Applied,
    Partial,
    Failed,
}

impl Default for ProposalStatus {
    fn default() -> Self {
        Self::Created
    }
}

fn anonymous_principal() -> ProtocolPrincipal {
    ProtocolPrincipal::Anonymous
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProposalRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub status: ProposalStatus,
    #[serde(default = "anonymous_principal")]
    pub created_by: ProtocolPrincipal,
    #[serde(default = "Utc::now")]
    pub created_at: chrono::DateTime<Utc>,
    #[serde(default)]
    pub target_session_id: Option<SessionId>,
    #[serde(default)]
    pub target_branch_id: Option<String>,
    #[serde(default)]
    pub operations: Vec<ProposalOperation>,
    #[serde(default)]
    pub required_permissions: Vec<String>,
    #[serde(default)]
    pub expected_effects: Value,
    #[serde(default)]
    pub approval: Option<ProposalApproval>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<Intent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_set: Option<ChangeSet>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_decision: Option<PolicyDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<ChangeCommit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProposalOperation {
    pub op: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProposalApproval {
    pub principal: ProtocolPrincipal,
    pub decided_at: chrono::DateTime<Utc>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
struct ProposalFailureEvidence {
    code: String,
    message_fingerprint: String,
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub async fn create_proposal(
        &self,
        context: &ProtocolContext,
        mut proposal: ProposalRecord,
    ) -> anyhow::Result<ProposalRecord> {
        // Scan for raw secrets in proposal operations and expected_effects
        let proposal_value = serde_json::to_value(&proposal)?;
        let scan = redaction::scan_value_for_raw_secrets(&proposal_value, "");
        if scan.has_findings() {
            let findings: Vec<String> = scan
                .findings
                .iter()
                .map(|f| format!("{} ({:?})", f.path, f.detection))
                .collect();
            anyhow::bail!(
                "proposal contains raw secret(s) in field(s): {}; use secret_ref references instead",
                findings.join(", ")
            );
        }

        proposal.id = if proposal.id.trim().is_empty() {
            new_id("prp")
        } else {
            proposal.id
        };
        proposal.status = ProposalStatus::Created;
        proposal.created_by = context.principal.clone();
        proposal.created_at = Utc::now();
        let intent = Intent {
            id: new_id("int"),
            intent_type_uri: INTENT_TYPE_URI.to_string(),
            principal: principal_identity(&context.principal),
            goal: json!({
                "kind": "proposal",
                "proposal_id": proposal.id,
                "expected_effects": proposal.expected_effects,
            }),
            target_session_id: proposal.target_session_id.clone(),
            target_branch_id: proposal.target_branch_id.clone(),
            created_at: proposal.created_at,
            annotations: Default::default(),
        };
        let change_set = ChangeSet {
            id: new_id("chg"),
            change_set_type_uri: CHANGE_SET_TYPE_URI.to_string(),
            intent_id: intent.id.clone(),
            operations: proposal
                .operations
                .iter()
                .map(|operation| ChangeOperation {
                    op: operation.op.clone(),
                    target: operation.target.clone(),
                    input_refs: Vec::new(),
                    payload: operation.payload.clone(),
                })
                .collect(),
            preconditions: Vec::new(),
            required_authority: proposal.required_permissions.clone(),
            expected_effects: proposal.expected_effects.clone(),
            idempotency_key: Some(proposal.id.clone()),
            created_at: proposal.created_at,
        };
        proposal.intent = Some(intent);
        proposal.policy_decision = Some(PolicyDecision {
            id: new_id("pol"),
            decision_type_uri: POLICY_DECISION_TYPE_URI.to_string(),
            change_set_id: change_set.id.clone(),
            outcome: PolicyDecisionOutcome::RequiresApproval,
            principal: principal_identity(&context.principal),
            reason: None,
            evaluated_authority: proposal.required_permissions.clone(),
            decided_at: proposal.created_at,
            policy_ref: None,
        });
        proposal.change_set = Some(change_set);
        proposal.commit = None;
        proposal.receipt = None;
        self.proposals
            .write()
            .await
            .insert(proposal.id.clone(), proposal.clone());
        self.append_kernel_event(
            &format!("kernel_proposal_{}", proposal.id),
            EVENT_PROPOSAL_CREATED,
            serde_json::to_value(&proposal)?,
        )
        .await?;
        Ok(proposal)
    }

    pub async fn get_proposal(&self, proposal_id: &str) -> anyhow::Result<ProposalRecord> {
        self.proposals
            .read()
            .await
            .get(proposal_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("proposal '{proposal_id}' not found"))
    }

    pub async fn list_proposals(&self) -> Vec<ProposalRecord> {
        let mut proposals: Vec<_> = self.proposals.read().await.values().cloned().collect();
        proposals.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        proposals
    }

    pub async fn approve_proposal(
        &self,
        context: &ProtocolContext,
        proposal_id: &str,
        reason: Option<String>,
    ) -> anyhow::Result<ProposalRecord> {
        let proposal = self.get_proposal(proposal_id).await?;
        self.ensure_proposal_authority(context, &proposal, PROPOSAL_APPROVE_PERMISSION, false)
            .await?;
        let mut proposals = self.proposals.write().await;
        let proposal = proposals
            .get_mut(proposal_id)
            .ok_or_else(|| anyhow::anyhow!("proposal '{proposal_id}' not found"))?;
        if proposal.status != ProposalStatus::Created {
            anyhow::bail!("proposal '{proposal_id}' is not awaiting approval");
        }
        proposal.status = ProposalStatus::Approved;
        proposal.approval = Some(ProposalApproval {
            principal: context.principal.clone(),
            decided_at: Utc::now(),
            reason,
        });
        if let Some(decision) = proposal.policy_decision.as_mut() {
            decision.outcome = PolicyDecisionOutcome::Allowed;
            decision.principal = principal_identity(&context.principal);
            decision.reason = proposal
                .approval
                .as_ref()
                .and_then(|item| item.reason.clone());
            decision.decided_at = proposal
                .approval
                .as_ref()
                .map(|item| item.decided_at)
                .unwrap_or_else(Utc::now);
        }
        let proposal = proposal.clone();
        drop(proposals);
        self.append_kernel_event(
            &format!("kernel_proposal_{}", proposal.id),
            EVENT_PROPOSAL_APPROVED,
            serde_json::to_value(&proposal)?,
        )
        .await?;
        Ok(proposal)
    }

    pub async fn reject_proposal(
        &self,
        context: &ProtocolContext,
        proposal_id: &str,
        reason: Option<String>,
    ) -> anyhow::Result<ProposalRecord> {
        let proposal = self.get_proposal(proposal_id).await?;
        self.ensure_proposal_authority(context, &proposal, PROPOSAL_REJECT_PERMISSION, false)
            .await?;
        let mut proposals = self.proposals.write().await;
        let proposal = proposals
            .get_mut(proposal_id)
            .ok_or_else(|| anyhow::anyhow!("proposal '{proposal_id}' not found"))?;
        if proposal.status != ProposalStatus::Created {
            anyhow::bail!("proposal '{proposal_id}' is not awaiting review");
        }
        proposal.status = ProposalStatus::Rejected;
        proposal.approval = Some(ProposalApproval {
            principal: context.principal.clone(),
            decided_at: Utc::now(),
            reason,
        });
        if let Some(decision) = proposal.policy_decision.as_mut() {
            decision.outcome = PolicyDecisionOutcome::Denied;
            decision.principal = principal_identity(&context.principal);
            decision.reason = proposal
                .approval
                .as_ref()
                .and_then(|item| item.reason.clone());
            decision.decided_at = proposal
                .approval
                .as_ref()
                .map(|item| item.decided_at)
                .unwrap_or_else(Utc::now);
        }
        let mut proposal = proposal.clone();
        drop(proposals);
        let receipt = self
            .record_proposal_effect(
                &proposal,
                &context.principal,
                "change.policy",
                EffectTerminalStatus::Denied,
                proposal
                    .approval
                    .as_ref()
                    .map(|item| item.decided_at)
                    .unwrap_or_else(Utc::now),
                1,
                Vec::new(),
                None,
                false,
            )
            .await?;
        proposal.receipt = Some(receipt);
        self.proposals
            .write()
            .await
            .insert(proposal.id.clone(), proposal.clone());
        self.append_kernel_event(
            &format!("kernel_proposal_{}", proposal.id),
            EVENT_PROPOSAL_REJECTED,
            serde_json::to_value(&proposal)?,
        )
        .await?;
        Ok(proposal)
    }

    pub async fn apply_proposal(
        &self,
        context: &ProtocolContext,
        proposal_id: &str,
    ) -> anyhow::Result<ProposalRecord> {
        let started = std::time::Instant::now();
        let started_at = Utc::now();
        let proposal = self.get_proposal(proposal_id).await?;
        self.ensure_proposal_authority(context, &proposal, PROPOSAL_APPLY_PERMISSION, true)
            .await?;
        let mut proposal = {
            let mut proposals = self.proposals.write().await;
            let proposal = proposals
                .get_mut(proposal_id)
                .ok_or_else(|| anyhow::anyhow!("proposal '{proposal_id}' not found"))?;
            if proposal.status != ProposalStatus::Approved {
                anyhow::bail!("proposal '{proposal_id}' must be approved before apply");
            }
            proposal.status = ProposalStatus::Applying;
            proposal.clone()
        };

        if let Err(error) = self.preflight_proposal_operations(&proposal).await {
            self.finish_failed_proposal(
                proposal,
                &context.principal,
                started_at,
                proposal_elapsed_ms(started),
                Vec::new(),
                Vec::new(),
                false,
                proposal_failure_evidence("preflight_failed", &error.to_string()),
                ProposalStatus::Applying,
            )
            .await?;
            return Err(error);
        }

        let mut results = Vec::new();
        let mut operation_receipts = Vec::new();
        for (index, operation) in proposal.operations.iter().enumerate() {
            let operation_started = std::time::Instant::now();
            let operation_started_at = Utc::now();
            match self
                .execute_proposal_operation(&context.principal, &proposal, operation)
                .await
            {
                Ok(result) => {
                    operation_receipts.push(
                        self.record_proposal_operation_effect(
                            &proposal,
                            &context.principal,
                            operation,
                            index,
                            EffectTerminalStatus::Succeeded,
                            operation_started_at,
                            proposal_elapsed_ms(operation_started),
                            Some(result.clone()),
                        )
                        .await?,
                    );
                    results.push(result);
                }
                Err(error) => {
                    operation_receipts.push(
                        self.record_proposal_operation_effect(
                            &proposal,
                            &context.principal,
                            operation,
                            index,
                            EffectTerminalStatus::Failed,
                            operation_started_at,
                            proposal_elapsed_ms(operation_started),
                            None,
                        )
                        .await?,
                    );
                    self.finish_failed_proposal(
                        proposal,
                        &context.principal,
                        started_at,
                        proposal_elapsed_ms(started),
                        operation_receipts,
                        results,
                        index > 0,
                        proposal_failure_evidence("operation_failed", &error.to_string()),
                        ProposalStatus::Applying,
                    )
                    .await?;
                    return Err(error);
                }
            }
        }

        let result = json!({"operations": results});
        let receipt = self
            .record_proposal_effect(
                &proposal,
                &context.principal,
                "change.commit",
                EffectTerminalStatus::Succeeded,
                started_at,
                proposal_elapsed_ms(started),
                operation_receipts.clone(),
                Some(result.clone()),
                false,
            )
            .await?;
        let receipt_record = self.replay_effect_receipt(&receipt.digest).await?;
        proposal.status = ProposalStatus::Applied;
        proposal.result = Some(result);
        proposal.commit = Some(ChangeCommit {
            id: new_id("cmt"),
            commit_type_uri: CHANGE_COMMIT_TYPE_URI.to_string(),
            change_set_id: proposal
                .change_set
                .as_ref()
                .map(|item| item.id.clone())
                .unwrap_or_else(|| proposal.id.clone()),
            status: ChangeCommitStatus::Committed,
            started_at,
            completed_at: Utc::now(),
            operation_receipts,
            result_refs: receipt_record.receipt.output_refs,
            error: None,
            branch_id: proposal.target_branch_id.clone(),
            idempotency_key: proposal
                .change_set
                .as_ref()
                .and_then(|item| item.idempotency_key.clone()),
        });
        proposal.receipt = Some(receipt);
        self.replace_proposal_if_status(&proposal, ProposalStatus::Applying)
            .await?;
        self.append_kernel_event(
            &format!("kernel_proposal_{}", proposal.id),
            EVENT_PROPOSAL_APPLIED,
            serde_json::to_value(&proposal)?,
        )
        .await?;
        Ok(proposal)
    }

    pub async fn fail_proposal(
        &self,
        proposal_id: &str,
        error: String,
    ) -> anyhow::Result<ProposalRecord> {
        let (proposal, previous_status) = {
            let mut proposals = self.proposals.write().await;
            let proposal = proposals
                .get_mut(proposal_id)
                .ok_or_else(|| anyhow::anyhow!("proposal '{proposal_id}' not found"))?;
            anyhow::ensure!(
                matches!(
                    proposal.status,
                    ProposalStatus::Created | ProposalStatus::Approved
                ),
                "proposal '{proposal_id}' cannot be failed from status {:?}",
                proposal.status
            );
            let previous_status = proposal.status.clone();
            proposal.status = ProposalStatus::Applying;
            (proposal.clone(), previous_status)
        };
        let result = self
            .finish_failed_proposal(
                proposal,
                &ProtocolPrincipal::Package {
                    package_id: KERNEL_PACKAGE_ID.to_string(),
                },
                Utc::now(),
                1,
                Vec::new(),
                Vec::new(),
                false,
                proposal_failure_evidence("externally_failed", &error),
                ProposalStatus::Applying,
            )
            .await;
        if result.is_err() {
            let mut proposals = self.proposals.write().await;
            if let Some(proposal) = proposals.get_mut(proposal_id) {
                if proposal.status == ProposalStatus::Applying {
                    proposal.status = previous_status;
                }
            }
        }
        result
    }

    async fn ensure_proposal_authority(
        &self,
        context: &ProtocolContext,
        proposal: &ProposalRecord,
        operation_permission: &str,
        include_required_permissions: bool,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.principal_has_grant(
                &context.principal,
                operation_permission,
                Some(proposal.id.as_str()),
            )
            .await,
            "principal is not authorized for {operation_permission} on proposal '{}'",
            proposal.id
        );
        if include_required_permissions {
            let scope = proposal
                .target_session_id
                .as_deref()
                .unwrap_or(proposal.id.as_str());
            for permission in &proposal.required_permissions {
                anyhow::ensure!(
                    self.principal_has_grant(&context.principal, permission, Some(scope))
                        .await,
                    "principal lacks required permission '{permission}' for proposal '{}'",
                    proposal.id
                );
            }
        }
        Ok(())
    }

    async fn replace_proposal_if_status(
        &self,
        proposal: &ProposalRecord,
        expected_status: ProposalStatus,
    ) -> anyhow::Result<()> {
        let mut proposals = self.proposals.write().await;
        let current = proposals
            .get_mut(&proposal.id)
            .ok_or_else(|| anyhow::anyhow!("proposal '{}' not found", proposal.id))?;
        anyhow::ensure!(
            current.status == expected_status,
            "proposal '{}' changed while commit was in progress",
            proposal.id
        );
        *current = proposal.clone();
        Ok(())
    }

    async fn preflight_proposal_operations(&self, proposal: &ProposalRecord) -> anyhow::Result<()> {
        for operation in &proposal.operations {
            match operation.op.as_str() {
                "asset.put" => {
                    if let Some(content) = operation.payload.get("content") {
                        anyhow::ensure!(
                            content.is_string(),
                            "asset.put operation content must be a string"
                        );
                    }
                    if let Some(mime) = operation.payload.get("mime") {
                        anyhow::ensure!(
                            mime.as_str().is_some_and(|value| !value.trim().is_empty()),
                            "asset.put operation mime must be a non-empty string"
                        );
                    }
                }
                "projection.rebuild" => {
                    let projection_id = operation.target.as_deref().ok_or_else(|| {
                        anyhow::anyhow!("projection.rebuild operation requires target")
                    })?;
                    self.projection_get(projection_id).await?;
                }
                other => anyhow::bail!("unsupported proposal operation '{other}'"),
            }
        }
        Ok(())
    }

    async fn execute_proposal_operation(
        &self,
        principal: &ProtocolPrincipal,
        proposal: &ProposalRecord,
        operation: &ProposalOperation,
    ) -> anyhow::Result<Value> {
        match operation.op.as_str() {
            "asset.put" => {
                let content = operation
                    .payload
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or("{}")
                    .to_string();
                let mime = operation
                    .payload
                    .get("mime")
                    .and_then(Value::as_str)
                    .unwrap_or("application/json")
                    .to_string();
                let asset = self
                    .put_asset(super::AssetPutRequest {
                        origin_package_id: Some(proposal_origin_package_id(principal)),
                        mime,
                        content,
                        metadata: json!({"proposal_id": proposal.id}),
                    })
                    .await?;
                Ok(json!({"op": operation.op, "asset_id": asset.id}))
            }
            "projection.rebuild" => {
                let projection_id = operation.target.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("projection.rebuild operation requires target")
                })?;
                let projection = self.projection_rebuild(projection_id).await?;
                Ok(json!({"op": operation.op, "projection_id": projection.id}))
            }
            other => anyhow::bail!("unsupported proposal operation '{other}'"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_proposal_operation_effect(
        &self,
        proposal: &ProposalRecord,
        principal: &ProtocolPrincipal,
        operation: &ProposalOperation,
        operation_index: usize,
        status: EffectTerminalStatus,
        started_at: chrono::DateTime<Utc>,
        duration_ms: u64,
        output: Option<Value>,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let mut request = EffectReceiptRequest::live(
            "change.operation",
            principal_identity(principal),
            json!({
                "kind": "proposal_adapter",
                "version": 1,
                "proposal_id": proposal.id,
            }),
            status,
            started_at,
            duration_ms,
            new_id("trc"),
        );
        request.protocol_profiles = vec![DEFAULT_CONTRACT_PROFILE.to_string()];
        request.inputs = vec![serde_json::to_value(operation)?];
        request.outputs = output.clone().into_iter().collect();
        request.authority = Some(json!({
            "required_permissions": proposal.required_permissions,
            "principal": principal,
            "kernel_executor": KERNEL_PACKAGE_ID,
        }));
        request.policy_decision = proposal
            .policy_decision
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;
        request.approval = proposal
            .approval
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;
        request.scope = EffectScope {
            session_id: proposal.target_session_id.clone(),
            branch_id: proposal.target_branch_id.clone(),
        };
        request.planned = json!({
            "operation_index": operation_index,
            "operation": operation,
        });
        request.actual = json!({
            "operation_index": operation_index,
            "status": status,
            "output_present": output.is_some(),
        });
        self.record_effect_receipt(request).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_proposal_effect(
        &self,
        proposal: &ProposalRecord,
        principal: &ProtocolPrincipal,
        effect_kind: &str,
        status: EffectTerminalStatus,
        started_at: chrono::DateTime<Utc>,
        duration_ms: u64,
        operation_receipts: Vec<ArtifactDescriptor>,
        output: Option<Value>,
        error_present: bool,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let mut request = EffectReceiptRequest::live(
            effect_kind,
            principal_identity(principal),
            json!({
                "kind": "proposal_adapter",
                "version": 1,
                "proposal_id": proposal.id,
            }),
            status,
            started_at,
            duration_ms,
            new_id("trc"),
        );
        request.protocol_profiles = vec![DEFAULT_CONTRACT_PROFILE.to_string()];
        request.inputs = [
            proposal
                .intent
                .as_ref()
                .map(serde_json::to_value)
                .transpose()?,
            proposal
                .change_set
                .as_ref()
                .map(serde_json::to_value)
                .transpose()?,
        ]
        .into_iter()
        .flatten()
        .collect();
        request.outputs = output.clone().into_iter().collect();
        request.authority = Some(json!({
            "required_permissions": proposal.required_permissions,
            "created_by": proposal.created_by,
            "principal": principal,
            "kernel_executor": KERNEL_PACKAGE_ID,
        }));
        request.policy_decision = proposal
            .policy_decision
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;
        request.approval = proposal
            .approval
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;
        request.parent_receipts = operation_receipts
            .iter()
            .map(|descriptor| descriptor.digest.clone())
            .collect();
        request.replay_mode = EffectReplayMode::Live;
        request.scope = EffectScope {
            session_id: proposal.target_session_id.clone(),
            branch_id: proposal.target_branch_id.clone(),
        };
        request.planned = proposal
            .change_set
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?
            .unwrap_or_else(|| json!({"expected_effects": proposal.expected_effects}));
        request.actual = json!({
            "proposal_id": proposal.id,
            "status": status,
            "operation_receipt_count": operation_receipts.len(),
            "output_present": output.is_some(),
            "error_present": error_present,
        });
        self.record_effect_receipt(request).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn finish_failed_proposal(
        &self,
        mut proposal: ProposalRecord,
        principal: &ProtocolPrincipal,
        started_at: chrono::DateTime<Utc>,
        duration_ms: u64,
        operation_receipts: Vec<ArtifactDescriptor>,
        results: Vec<Value>,
        partial: bool,
        failure: ProposalFailureEvidence,
        expected_status: ProposalStatus,
    ) -> anyhow::Result<ProposalRecord> {
        let effect_status = if partial {
            EffectTerminalStatus::Partial
        } else {
            EffectTerminalStatus::Failed
        };
        let ProposalFailureEvidence {
            code,
            message_fingerprint,
        } = failure;
        let result = json!({
            "operations": results,
            "failure": {
                "code": code,
                "message_fingerprint": message_fingerprint,
            },
        });
        let failure_code = result["failure"]["code"]
            .as_str()
            .unwrap_or("proposal_failed")
            .to_string();
        let receipt = self
            .record_proposal_effect(
                &proposal,
                principal,
                "change.commit",
                effect_status,
                started_at,
                duration_ms,
                operation_receipts.clone(),
                Some(result.clone()),
                true,
            )
            .await?;
        let receipt_record = self.replay_effect_receipt(&receipt.digest).await?;
        proposal.status = if partial {
            ProposalStatus::Partial
        } else {
            ProposalStatus::Failed
        };
        proposal.result = Some(result);
        proposal.commit = Some(ChangeCommit {
            id: new_id("cmt"),
            commit_type_uri: CHANGE_COMMIT_TYPE_URI.to_string(),
            change_set_id: proposal
                .change_set
                .as_ref()
                .map(|item| item.id.clone())
                .unwrap_or_else(|| proposal.id.clone()),
            status: if partial {
                ChangeCommitStatus::Partial
            } else {
                ChangeCommitStatus::Failed
            },
            started_at,
            completed_at: Utc::now(),
            operation_receipts,
            result_refs: receipt_record.receipt.output_refs,
            error: Some(failure_code),
            branch_id: proposal.target_branch_id.clone(),
            idempotency_key: proposal
                .change_set
                .as_ref()
                .and_then(|item| item.idempotency_key.clone()),
        });
        proposal.receipt = Some(receipt);
        self.replace_proposal_if_status(&proposal, expected_status)
            .await?;
        self.append_kernel_event(
            &format!("kernel_proposal_{}", proposal.id),
            EVENT_PROPOSAL_FAILED,
            serde_json::to_value(&proposal)?,
        )
        .await?;
        Ok(proposal)
    }
}

fn proposal_elapsed_ms(started: std::time::Instant) -> u64 {
    (started.elapsed().as_millis() as u64).max(1)
}

fn proposal_failure_evidence(code: &str, error_message: &str) -> ProposalFailureEvidence {
    ProposalFailureEvidence {
        code: code.to_string(),
        message_fingerprint: sha256_digest(error_message.as_bytes()),
    }
}

fn proposal_origin_package_id(principal: &ProtocolPrincipal) -> String {
    match principal {
        ProtocolPrincipal::Package { package_id } => package_id.clone(),
        _ => KERNEL_PACKAGE_ID.to_string(),
    }
}
