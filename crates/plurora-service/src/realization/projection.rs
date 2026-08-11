use std::collections::{BTreeMap, BTreeSet, HashMap};

use plurora_core::{EventEnvelope, EventSequence};
use plurora_runtime::{RealizationBackendSelection, RealizationEffectReceipt};
use plurora_work::{RealizationId, RealizationRevision};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RealizationRecord {
    pub revision: RealizationRevision,
    pub target_id: String,
    pub backends: Vec<RealizationBackendSelection>,
    pub plan_idempotency_key: String,
    pub plan_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RealizationJournalEvent {
    pub record: RealizationRecord,
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effect_receipts: Vec<RealizationEffectReceipt>,
}

#[derive(Debug, Default)]
pub(super) struct RealizationProjection {
    pub next_sequence: EventSequence,
    pub records: BTreeMap<RealizationId, RealizationRecord>,
    pub idempotency: HashMap<(String, String), (String, RealizationId)>,
    pub apply_checkpoints: BTreeSet<RealizationId>,
    pub stop_checkpoints: BTreeSet<RealizationId>,
    pub pending_apply_receipts: BTreeMap<RealizationId, Vec<RealizationEffectReceipt>>,
    pub pending_stop_receipts: BTreeMap<RealizationId, Vec<RealizationEffectReceipt>>,
}

pub(super) fn apply_event(
    projection: &mut RealizationProjection,
    event: &EventEnvelope,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        event.sequence == projection.next_sequence,
        "Realization journal sequence is not contiguous"
    );
    let payload: RealizationJournalEvent = serde_json::from_value(event.payload.clone())?;
    payload
        .record
        .revision
        .validate()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    if let (Some(idempotency_key), Some(fingerprint)) = (
        payload.idempotency_key.as_ref(),
        payload.request_fingerprint.as_ref(),
    ) {
        let key = (payload.operation.clone(), idempotency_key.clone());
        if let Some((existing_fingerprint, existing_id)) = projection.idempotency.get(&key) {
            anyhow::ensure!(
                existing_fingerprint == fingerprint
                    && existing_id == &payload.record.revision.realization_id,
                "Realization idempotency projection conflicts with durable history"
            );
        } else {
            projection.idempotency.insert(
                key,
                (
                    fingerprint.clone(),
                    payload.record.revision.realization_id.clone(),
                ),
            );
        }
    }
    let realization_id = payload.record.revision.realization_id.clone();
    let status = payload.record.revision.status;
    projection
        .records
        .insert(realization_id.clone(), payload.record);
    if event.kind == super::PRIVATE_EVENT_REALIZATION_EFFECT_APPLIED {
        projection.apply_checkpoints.insert(realization_id.clone());
        projection
            .pending_apply_receipts
            .insert(realization_id.clone(), payload.effect_receipts.clone());
    } else if status != plurora_work::RealizationStatus::Applying {
        projection.apply_checkpoints.remove(&realization_id);
        projection.pending_apply_receipts.remove(&realization_id);
    }
    if event.kind == super::PRIVATE_EVENT_REALIZATION_EFFECT_STOPPED {
        projection.stop_checkpoints.insert(realization_id.clone());
        projection
            .pending_stop_receipts
            .insert(realization_id.clone(), payload.effect_receipts);
    } else if status != plurora_work::RealizationStatus::Stopping {
        projection.stop_checkpoints.remove(&realization_id);
        projection.pending_stop_receipts.remove(&realization_id);
    }
    projection.next_sequence = projection.next_sequence.saturating_add(1);
    Ok(())
}
