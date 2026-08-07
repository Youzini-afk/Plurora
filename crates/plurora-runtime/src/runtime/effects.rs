use std::collections::BTreeMap;

use bytes::Bytes;
use chrono::{DateTime, Utc};
use plurora_core::{
    new_id, ArtifactDescriptor, EffectReceipt, EffectReplayMode, EffectScope, EffectTerminalStatus,
    PrincipalIdentity, APPROVAL_EVIDENCE_TYPE_URI, AUTHORITY_EVIDENCE_TYPE_URI,
    COMPONENT_EVIDENCE_TYPE_URI, EFFECT_RECEIPT_TYPE_URI, EFFECT_VALUE_TYPE_URI,
    POLICY_DECISION_TYPE_URI,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{ArtifactCommitRequest, Runtime};
use crate::{redaction, EventStore, ProtocolPrincipal};

pub const EFFECT_RECEIPT_MEDIA_TYPE: &str = "application/vnd.plurora.effect-receipt+json;version=1";
pub const EFFECT_VALUE_MEDIA_TYPE: &str = "application/json";

#[derive(Debug, Clone)]
pub(crate) struct EffectReceiptRequest {
    pub effect_kind: String,
    pub principal: PrincipalIdentity,
    pub component: Value,
    pub protocol_profiles: Vec<String>,
    pub inputs: Vec<Value>,
    pub outputs: Vec<Value>,
    pub external_effects: Vec<Value>,
    pub authority: Option<Value>,
    pub policy_decision: Option<Value>,
    pub approval: Option<Value>,
    pub status: EffectTerminalStatus,
    pub started_at: DateTime<Utc>,
    pub latency_ms: u64,
    pub usage: Value,
    pub cost: Value,
    pub trace_id: String,
    pub parent_receipts: Vec<String>,
    pub replay_mode: EffectReplayMode,
    pub scope: EffectScope,
    pub planned: Value,
    pub actual: Value,
    pub annotations: BTreeMap<String, Value>,
}

impl EffectReceiptRequest {
    pub fn live(
        effect_kind: impl Into<String>,
        principal: PrincipalIdentity,
        component: Value,
        status: EffectTerminalStatus,
        started_at: DateTime<Utc>,
        latency_ms: u64,
        trace_id: impl Into<String>,
    ) -> Self {
        Self {
            effect_kind: effect_kind.into(),
            principal,
            component,
            protocol_profiles: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            external_effects: Vec::new(),
            authority: None,
            policy_decision: None,
            approval: None,
            status,
            started_at,
            latency_ms,
            usage: Value::Null,
            cost: Value::Null,
            trace_id: trace_id.into(),
            parent_receipts: Vec::new(),
            replay_mode: EffectReplayMode::Live,
            scope: EffectScope::default(),
            planned: Value::Null,
            actual: Value::Null,
            annotations: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct EffectReplayResult {
    pub receipt_ref: ArtifactDescriptor,
    pub receipt: EffectReceipt,
    pub outputs: Vec<Value>,
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub(crate) async fn record_effect_receipt(
        &self,
        request: EffectReceiptRequest,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let component_ref = self
            .commit_effect_value(COMPONENT_EVIDENCE_TYPE_URI, "component", request.component)
            .await?;
        let input_refs = self
            .commit_effect_values(EFFECT_VALUE_TYPE_URI, "input", request.inputs)
            .await?;
        let output_refs = self
            .commit_effect_values(EFFECT_VALUE_TYPE_URI, "output", request.outputs)
            .await?;
        let external_effect_refs = self
            .commit_effect_values(
                EFFECT_VALUE_TYPE_URI,
                "external_effect",
                request.external_effects,
            )
            .await?;
        let authority_ref = self
            .commit_optional_effect_value(
                AUTHORITY_EVIDENCE_TYPE_URI,
                "authority",
                request.authority,
            )
            .await?;
        let policy_decision_ref = self
            .commit_optional_effect_value(
                POLICY_DECISION_TYPE_URI,
                "policy_decision",
                request.policy_decision,
            )
            .await?;
        let approval_ref = self
            .commit_optional_effect_value(APPROVAL_EVIDENCE_TYPE_URI, "approval", request.approval)
            .await?;

        let (usage, _) = redaction::redact_effect_value(&request.usage);
        let (cost, _) = redaction::redact_effect_value(&request.cost);
        let (planned, _) = redaction::redact_effect_value(&request.planned);
        let (actual, _) = redaction::redact_effect_value(&request.actual);
        let (annotations, _) =
            redaction::redact_effect_value(&serde_json::to_value(&request.annotations)?);
        let annotations = serde_json::from_value(annotations)?;
        let receipt = EffectReceipt {
            schema_version: 1,
            receipt_type_uri: EFFECT_RECEIPT_TYPE_URI.to_string(),
            receipt_id: new_id("rct"),
            effect_kind: request.effect_kind,
            principal: request.principal,
            component_ref,
            protocol_profiles: request.protocol_profiles,
            input_refs,
            output_refs,
            external_effect_refs,
            authority_ref,
            policy_decision_ref,
            approval_ref,
            status: request.status,
            started_at: request.started_at,
            completed_at: Utc::now(),
            latency_ms: request.latency_ms,
            usage,
            cost,
            trace_id: request.trace_id,
            parent_receipts: request.parent_receipts,
            replay_mode: request.replay_mode,
            scope: request.scope,
            planned,
            actual,
            annotations,
        };
        let receipt_value = serde_json::to_value(&receipt)?;
        let scan = redaction::scan_effect_value_for_raw_secrets(&receipt_value, "receipt");
        anyhow::ensure!(
            !scan.has_findings(),
            "effect receipt contains raw secret material"
        );
        let mut annotations = BTreeMap::new();
        annotations.insert("receipt_id".to_string(), json!(receipt.receipt_id));
        annotations.insert("effect_kind".to_string(), json!(receipt.effect_kind));
        annotations.insert("status".to_string(), serde_json::to_value(receipt.status)?);
        let references = receipt.referenced_digests();
        self.commit_artifact(ArtifactCommitRequest {
            artifact_type_uri: EFFECT_RECEIPT_TYPE_URI.to_string(),
            media_type: EFFECT_RECEIPT_MEDIA_TYPE.to_string(),
            bytes: Bytes::from(serde_json::to_vec(&receipt)?),
            references,
            annotations,
        })
        .await
        .map_err(Into::into)
    }

    pub async fn replay_effect_receipt(
        &self,
        receipt_digest: &str,
    ) -> anyhow::Result<EffectReplayResult> {
        let bytes = self
            .config
            .object_store
            .get(receipt_digest)
            .await
            .map_err(|error| anyhow::anyhow!("incomplete history: {error}"))?;
        let receipt: EffectReceipt = serde_json::from_slice(&bytes)
            .map_err(|error| anyhow::anyhow!("invalid effect receipt: {error}"))?;
        anyhow::ensure!(
            receipt.receipt_type_uri == EFFECT_RECEIPT_TYPE_URI,
            "unsupported effect receipt type '{}'",
            receipt.receipt_type_uri
        );
        let receipt_ref = ArtifactDescriptor {
            artifact_type_uri: EFFECT_RECEIPT_TYPE_URI.to_string(),
            media_type: EFFECT_RECEIPT_MEDIA_TYPE.to_string(),
            digest: receipt_digest.to_string(),
            size_bytes: bytes.len() as u64,
            references: receipt.referenced_digests(),
            annotations: BTreeMap::from([
                ("receipt_id".to_string(), json!(receipt.receipt_id)),
                ("effect_kind".to_string(), json!(receipt.effect_kind)),
                ("status".to_string(), serde_json::to_value(receipt.status)?),
            ]),
        };
        let mut outputs = Vec::with_capacity(receipt.output_refs.len());
        outputs.extend(
            self.read_effect_values(&receipt.output_refs, "output")
                .await?,
        );
        Ok(EffectReplayResult {
            receipt_ref,
            receipt,
            outputs,
        })
    }

    pub(crate) async fn read_effect_values(
        &self,
        descriptors: &[ArtifactDescriptor],
        role: &str,
    ) -> anyhow::Result<Vec<Value>> {
        let mut values = Vec::with_capacity(descriptors.len());
        for descriptor in descriptors {
            let bytes = self
                .read_artifact(descriptor)
                .await
                .map_err(|error| anyhow::anyhow!("incomplete history: {error}"))?;
            values.push(serde_json::from_slice(&bytes).map_err(|error| {
                anyhow::anyhow!(
                    "invalid recorded effect {role} '{}': {error}",
                    descriptor.digest
                )
            })?);
        }
        Ok(values)
    }

    async fn commit_effect_values(
        &self,
        artifact_type_uri: &str,
        role: &str,
        values: Vec<Value>,
    ) -> anyhow::Result<Vec<ArtifactDescriptor>> {
        let mut descriptors = Vec::with_capacity(values.len());
        for value in values {
            descriptors.push(
                self.commit_effect_value(artifact_type_uri, role, value)
                    .await?,
            );
        }
        Ok(descriptors)
    }

    async fn commit_optional_effect_value(
        &self,
        artifact_type_uri: &str,
        role: &str,
        value: Option<Value>,
    ) -> anyhow::Result<Option<ArtifactDescriptor>> {
        match value {
            Some(value) => Ok(Some(
                self.commit_effect_value(artifact_type_uri, role, value)
                    .await?,
            )),
            None => Ok(None),
        }
    }

    async fn commit_effect_value(
        &self,
        artifact_type_uri: &str,
        role: &str,
        value: Value,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let (value, scan) = redaction::redact_effect_value(&value);
        let mut annotations = BTreeMap::new();
        annotations.insert("effect_role".to_string(), json!(role));
        annotations.insert(
            "redaction_state".to_string(),
            json!(if scan.has_findings() {
                "redacted"
            } else {
                "captured_by_reference"
            }),
        );
        self.commit_artifact(ArtifactCommitRequest {
            artifact_type_uri: artifact_type_uri.to_string(),
            media_type: EFFECT_VALUE_MEDIA_TYPE.to_string(),
            bytes: Bytes::from(serde_json::to_vec(&value)?),
            references: Vec::new(),
            annotations,
        })
        .await
        .map_err(Into::into)
    }
}

pub(crate) fn principal_identity(principal: &ProtocolPrincipal) -> PrincipalIdentity {
    match principal {
        ProtocolPrincipal::HostAdmin => PrincipalIdentity::HostAdmin,
        ProtocolPrincipal::HostDev => PrincipalIdentity::HostDev,
        ProtocolPrincipal::Package { package_id } => PrincipalIdentity::Package {
            package_id: package_id.clone(),
        },
        ProtocolPrincipal::Human { user_id } => PrincipalIdentity::Human {
            user_id: user_id.clone(),
        },
        ProtocolPrincipal::Assistant {
            assistant_id,
            delegated_user_id,
        } => PrincipalIdentity::Assistant {
            assistant_id: assistant_id.clone(),
            delegated_user_id: delegated_user_id.clone(),
        },
        ProtocolPrincipal::Anonymous => PrincipalIdentity::Anonymous,
    }
}

pub(crate) fn request_principal(caller_package_id: Option<&str>) -> PrincipalIdentity {
    caller_package_id.map_or(PrincipalIdentity::Kernel, |package_id| {
        PrincipalIdentity::Package {
            package_id: package_id.to_string(),
        }
    })
}
