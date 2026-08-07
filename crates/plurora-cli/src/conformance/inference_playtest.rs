//! Conformance tests for `official/inference-playtest-lab` (Phase C4).
//!
//! Proves inference is not "prompt -> text response" but participation
//! in the Plurora session/branch/proposal/inspection/fork creative runtime.

use std::path::PathBuf;

use plurora_runtime::{CapabilityInvocationRequest, OpenSessionRequest, ProtocolContext};
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

/// Load inference-local-lab + inference-playtest-lab and open a session.
async fn setup_both_labs() -> anyhow::Result<(
    plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>,
    plurora_core::SessionId,
)> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/official/inference-local-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/official/inference-playtest-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    Ok((runtime, session.id))
}

/// Produce a deterministic inference_result from inference-local-lab/invoke.
async fn invoke_inference_local(
    runtime: &plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>,
) -> anyhow::Result<serde_json::Value> {
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-local-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-local-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "operation_id": "op_c4_001",
                "operation_kind": "generate",
                "transport_kind": "in_memory",
            }),
        })
        .await?;
    Ok(result.output)
}

/// C4 conformance case 1: draft_proposal produces proposal_draft with
/// requires_user_approval, asset.put, no raw secret, not a chat message.
pub(crate) async fn inference_playtest_draft() -> anyhow::Result<()> {
    let (runtime, session_id) = setup_both_labs().await?;
    let inference_result = invoke_inference_local(&runtime).await?;

    let draft = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-playtest-lab/draft_proposal".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-playtest-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "session_id": session_id,
                "inference_result": inference_result,
                "intent": "create inference artifact",
            }),
        })
        .await?;

    // Must be a proposal_draft, not a chat/message/prompt
    anyhow::ensure!(
        draft.output["kind"] == json!("inference_playtest_proposal_draft"),
        "draft_proposal must return inference_playtest_proposal_draft kind"
    );
    // requires_user_approval must be true
    anyhow::ensure!(
        draft.output["requires_user_approval"] == json!(true),
        "proposal_draft must require user approval"
    );
    // Must have at least one asset.put operation
    let ops = draft.output["operations"]
        .as_array()
        .expect("operations must be array");
    anyhow::ensure!(
        ops.iter().any(|op| op["op"] == "asset.put"),
        "proposal_draft must contain asset.put operation"
    );
    // asset.put content must be JSON
    for op in ops {
        if op["op"] == "asset.put" {
            let content = op["payload"]["content"]
                .as_str()
                .expect("asset.put must have string content");
            serde_json::from_str::<serde_json::Value>(content)
                .map_err(|_| anyhow::anyhow!("asset.put content must be valid JSON"))?;
        }
    }
    // No raw secret in output
    let output_str = serde_json::to_string(&draft.output).unwrap();
    anyhow::ensure!(
        !output_str.contains("\"api_key\"") && !output_str.contains("\"secret\""),
        "proposal_draft must not contain raw secret fields"
    );
    // NOT a chat/message/prompt
    anyhow::ensure!(
        !output_str.contains("\"messages\""),
        "proposal_draft must not contain messages field"
    );
    anyhow::ensure!(
        !output_str.contains("\"prompt\""),
        "proposal_draft must not contain prompt field"
    );
    // source_inference provenance must exist
    anyhow::ensure!(
        draft.output["source_inference"]["package_id"].is_string(),
        "proposal_draft must have source_inference provenance"
    );
    anyhow::ensure!(
        draft.output["source_inference"]["network_performed"] == json!(false),
        "source_inference must record network_performed=false"
    );

    Ok(())
}

/// C4 conformance case 2: inspect_proposal returns risk/operations/permissions/provenance.
pub(crate) async fn inference_playtest_inspect() -> anyhow::Result<()> {
    let (runtime, session_id) = setup_both_labs().await?;
    let inference_result = invoke_inference_local(&runtime).await?;

    // Draft a proposal
    let draft = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-playtest-lab/draft_proposal".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-playtest-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "session_id": session_id,
                "inference_result": inference_result.clone(),
                "intent": "inspect test",
            }),
        })
        .await?;

    // Create the proposal via kernel
    let proposal = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "change.proposal.create",
            json!({
                "target_session_id": session_id,
                "operations": draft.output["operations"],
                "required_permissions": draft.output["required_permissions"],
                "expected_effects": draft.output["expected_effects"],
            }),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.message))?;

    // Inspect the proposal
    let inspection = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-playtest-lab/inspect_proposal".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-playtest-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "proposal": proposal,
            }),
        })
        .await?;

    anyhow::ensure!(
        inspection.output["kind"] == json!("inference_playtest_inspection"),
        "inspect_proposal must return inference_playtest_inspection kind"
    );
    anyhow::ensure!(
        inspection.output["risk"].is_string(),
        "inspection must have risk field"
    );
    anyhow::ensure!(
        inspection.output["operations_summary"].is_array(),
        "inspection must have operations_summary"
    );
    anyhow::ensure!(
        inspection.output["permissions"].is_array(),
        "inspection must have permissions"
    );
    anyhow::ensure!(
        inspection.output["provenance"]["source_inference"]["package_id"]
            == json!("official/inference-local-lab"),
        "inspection must preserve source_inference provenance from expected_effects"
    );

    Ok(())
}

/// C4 conformance case 3: rejected proposal cannot be applied.
pub(crate) async fn inference_playtest_reject_apply_denied() -> anyhow::Result<()> {
    let (runtime, session_id) = setup_both_labs().await?;
    let inference_result = invoke_inference_local(&runtime).await?;

    let draft = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-playtest-lab/draft_proposal".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-playtest-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "session_id": session_id,
                "inference_result": inference_result,
                "intent": "reject test",
            }),
        })
        .await?;

    // Create proposal
    let created = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "change.proposal.create",
            json!({
                "target_session_id": session_id,
                "operations": draft.output["operations"],
                "required_permissions": draft.output["required_permissions"],
                "expected_effects": draft.output["expected_effects"],
            }),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.message))?;

    let proposal_id = created["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("proposal missing id"))?
        .to_string();

    // Reject it
    runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "change.proposal.reject",
            json!({"proposal_id": proposal_id, "reason": "conformance reject test"}),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.message))?;

    // Apply must fail
    let denied = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "change.proposal.apply",
            json!({"proposal_id": proposal_id}),
        )
        .await;
    anyhow::ensure!(denied.is_err(), "rejected proposal must not apply");

    Ok(())
}

/// C4 conformance case 4: approved proposal applies and asset is written;
/// then branch_plan + fork create branch with proposal/inference provenance.
pub(crate) async fn inference_playtest_apply_and_branch() -> anyhow::Result<()> {
    let (runtime, session_id) = setup_both_labs().await?;
    let inference_result = invoke_inference_local(&runtime).await?;

    let draft = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-playtest-lab/draft_proposal".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-playtest-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "session_id": session_id,
                "inference_result": inference_result.clone(),
                "intent": "apply and branch test",
            }),
        })
        .await?;

    // Create proposal
    let created = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "change.proposal.create",
            json!({
                "target_session_id": session_id,
                "operations": draft.output["operations"],
                "required_permissions": draft.output["required_permissions"],
                "expected_effects": draft.output["expected_effects"],
            }),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.message))?;

    let proposal_id = created["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("proposal missing id"))?
        .to_string();

    // Approve it
    runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "change.proposal.approve",
            json!({"proposal_id": proposal_id, "reason": "conformance approve test"}),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.message))?;

    // Apply it — should succeed
    let applied = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "change.proposal.apply",
            json!({"proposal_id": proposal_id}),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.message))?;

    anyhow::ensure!(
        applied["status"] == json!("applied"),
        "proposal must reach applied status"
    );
    anyhow::ensure!(
        applied["result"]["operations"].is_array(),
        "apply result must have operations"
    );

    // Verify asset was written
    let assets = runtime.list_assets().await;
    anyhow::ensure!(
        !assets.is_empty(),
        "asset must be written after proposal apply"
    );

    // Get branch_plan from inference-playtest-lab
    let branch_plan = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-playtest-lab/branch_plan".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-playtest-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "session_id": session_id,
                "proposal_id": proposal_id,
                "source_inference": draft.output["source_inference"],
            }),
        })
        .await?;

    anyhow::ensure!(
        branch_plan.output["kind"] == json!("inference_playtest_branch_plan"),
        "branch_plan must return inference_playtest_branch_plan kind"
    );
    anyhow::ensure!(
        branch_plan.output["fork_metadata"]["proposal_id"] == json!(proposal_id),
        "branch_plan must reference the proposal"
    );

    // Actually fork the session using context.fork with the metadata from branch_plan
    let branch = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "context.fork",
            json!({
                "parent_session_id": session_id,
                "forked_from_sequence": 0,
                "metadata": branch_plan.output["fork_metadata"],
            }),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.message))?;

    // Verify branch has proposal and inference provenance in metadata
    anyhow::ensure!(
        branch["metadata"]["proposal_id"] == json!(proposal_id),
        "branch metadata must contain proposal_id"
    );
    anyhow::ensure!(
        branch["metadata"]["source_inference"]["package_id"]
            == json!("official/inference-local-lab"),
        "branch metadata must contain source_inference provenance"
    );

    Ok(())
}

/// C4 conformance case 5: output contains no chat/message/kernel model terms.
pub(crate) async fn inference_playtest_no_chat_platform_terms() -> anyhow::Result<()> {
    let (runtime, session_id) = setup_both_labs().await?;
    let inference_result = invoke_inference_local(&runtime).await?;

    // Check draft_proposal
    let draft = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-playtest-lab/draft_proposal".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-playtest-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "session_id": session_id,
                "inference_result": inference_result,
                "intent": "term check",
            }),
        })
        .await?;

    let draft_str = serde_json::to_string(&draft.output).unwrap();
    anyhow::ensure!(
        !draft_str.contains("\"messages\""),
        "draft_proposal must not contain 'messages' field"
    );
    anyhow::ensure!(
        !draft_str.contains("\"prompt\""),
        "draft_proposal must not contain 'prompt' field"
    );
    anyhow::ensure!(
        !draft_str.contains("\"chat\""),
        "draft_proposal must not contain 'chat' field"
    );
    anyhow::ensure!(
        !draft_str.contains("\"system\""),
        "draft_proposal must not contain 'system' field (chat-shaped)"
    );
    anyhow::ensure!(
        !draft_str.contains("\"user\""),
        "draft_proposal must not contain 'user' field (chat-shaped)"
    );
    anyhow::ensure!(
        !draft_str.contains("\"assistant\""),
        "draft_proposal must not contain 'assistant' field (chat-shaped)"
    );

    // Check explain_flow
    let flow = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("official/inference-playtest-lab/explain_flow".to_string()),
            caller_package_id: None,
            provider_package_id: Some("official/inference-playtest-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await?;

    let flow_str = serde_json::to_string(&flow.output).unwrap();
    anyhow::ensure!(
        !flow_str.contains("platform.model"),
        "explain_flow must not reference platform.model"
    );
    anyhow::ensure!(
        !flow_str.contains("platform.prompt"),
        "explain_flow must not reference platform.prompt"
    );
    anyhow::ensure!(
        !flow_str.contains("platform.chat"),
        "explain_flow must not reference platform.chat"
    );

    Ok(())
}
