use plurora_core::EffectTerminalStatus;
use plurora_runtime::{OpenSessionRequest, ProtocolContext, ProtocolPrincipal};
use serde_json::json;

use super::fixtures::*;

pub(crate) async fn lifecycle_apply() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime
        .projection_register(plurora_runtime::runtime::ProjectionDefinition {
            id: "proposal/test-projection".to_string(),
            session_id: session.id.clone(),
            source_kind_prefix: Some("kernel/v1/session".to_string()),
            state: json!({}),
        })
        .await?;
    let created = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "kernel.v1.proposal.create",
            json!({
                "target_session_id": session.id,
                "required_permissions": ["assets.write", "projections.rebuild"],
                "expected_effects": {"summary": "write asset and rebuild projection"},
                "operations": [
                    {"op": "asset.put", "payload": {"mime": "application/json", "content": "{\"proposal\":true}"}},
                    {"op": "projection.rebuild", "target": "proposal/test-projection"}
                ]
            }),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let proposal_id = created["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("proposal missing id"))?
        .to_string();
    anyhow::ensure!(
        created["intent"]["intent_type_uri"] == json!(plurora_core::INTENT_TYPE_URI)
            && created["change_set"]["change_set_type_uri"]
                == json!(plurora_core::CHANGE_SET_TYPE_URI)
            && created["policy_decision"]["outcome"] == json!("requires_approval"),
        "legacy proposal was not adapted into change primitives"
    );
    let denied = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "kernel.v1.proposal.apply",
            json!({"proposal_id": proposal_id}),
        )
        .await;
    anyhow::ensure!(denied.is_err(), "unapproved proposal should not apply");
    runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "kernel.v1.proposal.approve",
            json!({"proposal_id": proposal_id, "reason": "conformance"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let applied = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "kernel.v1.proposal.apply",
            json!({"proposal_id": proposal_id}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        applied["status"] == json!("applied"),
        "proposal did not reach applied status"
    );
    anyhow::ensure!(
        applied["result"]["operations"]
            .as_array()
            .map(|items| items.len())
            .unwrap_or(0)
            == 2,
        "proposal apply results missing"
    );
    anyhow::ensure!(
        applied["commit"]["status"] == json!("committed")
            && applied["commit"]["operation_receipts"]
                .as_array()
                .map(|items| items.len())
                == Some(2),
        "proposal commit evidence missing"
    );
    let receipt_digest = applied["receipt"]["digest"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("proposal receipt missing"))?;
    let replay = runtime.replay_effect_receipt(receipt_digest).await?;
    anyhow::ensure!(
        replay.receipt.effect_kind == "change.commit"
            && replay.receipt.status == EffectTerminalStatus::Succeeded
            && replay.receipt.parent_receipts.len() == 2,
        "proposal commit receipt is incomplete"
    );
    anyhow::ensure!(
        runtime
            .fail_proposal(&proposal_id, "late external failure".to_string())
            .await
            .is_err(),
        "terminal applied proposal was overwritten by fail_proposal"
    );
    anyhow::ensure!(
        runtime.get_proposal(&proposal_id).await?.status
            == plurora_runtime::runtime::ProposalStatus::Applied,
        "applied proposal status changed after rejected failure transition"
    );
    Ok(())
}

pub(crate) async fn reject_and_apply_denied() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let created = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "kernel.v1.proposal.create",
            json!({"operations": [{"op": "asset.put", "payload": {"content": "{}"}}]}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let proposal_id = created["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("proposal missing id"))?
        .to_string();
    let rejected = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "kernel.v1.proposal.reject",
            json!({"proposal_id": proposal_id, "reason": "conformance"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        rejected["policy_decision"]["outcome"] == json!("denied"),
        "proposal rejection did not produce a denied policy decision"
    );
    let receipt_digest = rejected["receipt"]["digest"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("proposal rejection receipt missing"))?;
    let replay = runtime.replay_effect_receipt(receipt_digest).await?;
    anyhow::ensure!(
        replay.receipt.effect_kind == "change.policy"
            && replay.receipt.status == EffectTerminalStatus::Denied,
        "proposal rejection receipt is not denied"
    );
    let denied = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "kernel.v1.proposal.apply",
            json!({"proposal_id": proposal_id}),
        )
        .await;
    anyhow::ensure!(denied.is_err(), "rejected proposal should not apply");
    Ok(())
}

pub(crate) async fn authority_is_enforced() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let host = ProtocolContext::host_dev("conformance");
    let package_principal = ProtocolPrincipal::Package {
        package_id: "example/change-runner".to_string(),
    };
    let package = ProtocolContext::package("example/change-runner", "conformance");
    let created = runtime
        .call_protocol(
            &host,
            "kernel.v1.proposal.create",
            json!({
                "required_permissions": ["assets.write"],
                "operations": [{"op": "asset.put", "payload": {"content": "{}"}}]
            }),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let proposal_id = created["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("proposal missing id"))?
        .to_string();

    anyhow::ensure!(
        runtime
            .call_protocol(
                &package,
                "kernel.v1.proposal.approve",
                json!({"proposal_id": proposal_id}),
            )
            .await
            .is_err(),
        "package approved a proposal without review authority"
    );
    runtime
        .grant_permission(
            package_principal.clone(),
            "change.proposal.approve".to_string(),
            Some(proposal_id.clone()),
            Some("conformance".to_string()),
        )
        .await?;
    runtime
        .call_protocol(
            &package,
            "kernel.v1.proposal.approve",
            json!({"proposal_id": proposal_id}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;

    anyhow::ensure!(
        runtime
            .call_protocol(
                &package,
                "kernel.v1.proposal.apply",
                json!({"proposal_id": proposal_id}),
            )
            .await
            .is_err(),
        "package applied a proposal without apply authority"
    );
    runtime
        .grant_permission(
            package_principal.clone(),
            "change.proposal.apply".to_string(),
            Some(proposal_id.clone()),
            Some("conformance".to_string()),
        )
        .await?;
    runtime
        .grant_permission(
            package_principal,
            "assets.write".to_string(),
            None,
            Some("conformance".to_string()),
        )
        .await?;
    let applied = runtime
        .call_protocol(
            &package,
            "kernel.v1.proposal.apply",
            json!({"proposal_id": proposal_id}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        applied["status"] == json!("applied"),
        "authorized package did not apply proposal"
    );
    Ok(())
}

pub(crate) async fn preflight_failure_is_structured() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    let context = ProtocolContext::host_dev("conformance");
    let created = runtime
        .call_protocol(
            &context,
            "kernel.v1.proposal.create",
            json!({"operations": [{"op": "unsupported.operation", "payload": {}}]}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let proposal_id = created["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("proposal missing id"))?
        .to_string();
    runtime
        .call_protocol(
            &context,
            "kernel.v1.proposal.approve",
            json!({"proposal_id": proposal_id}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        runtime
            .call_protocol(
                &context,
                "kernel.v1.proposal.apply",
                json!({"proposal_id": proposal_id}),
            )
            .await
            .is_err(),
        "unsupported proposal operation unexpectedly applied"
    );
    let failed = runtime.get_proposal(&proposal_id).await?;
    anyhow::ensure!(
        failed.status == plurora_runtime::runtime::ProposalStatus::Failed,
        "preflight failure did not reach failed status"
    );
    let result = failed
        .result
        .ok_or_else(|| anyhow::anyhow!("preflight failure result missing"))?;
    anyhow::ensure!(
        result["failure"]["code"] == json!("preflight_failed")
            && result["failure"]["message_fingerprint"]
                .as_str()
                .is_some_and(|value| value.starts_with("sha256:")),
        "preflight failure evidence is not structured and content-free"
    );
    Ok(())
}
