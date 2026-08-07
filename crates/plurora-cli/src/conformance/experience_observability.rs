//! Conformance tests for `plurora/experience-observability-lab` (Experience Beta 3).
//!
//! Covers:
//! 1. Observability contract shape (surfaces, capabilities, output shapes)
//! 2. Session health summary shape and status derivation
//! 3. Package health summary shape
//! 4. Agent run health summary shape
//! 5. Proposal causal chain with content_address per step
//! 6. Cost/latency summary shape (no raw secrets, refs only)
//! 7. Failure breadcrumbs shape
//! 8. Guardrail/audit summary shape
//! 9. No forbidden namespace (platform.observability.* / platform.experience.* etc.)
//! 10. No raw secrets in any capability output/input processing

use std::path::PathBuf;

use plurora_runtime::CapabilityInvocationRequest;
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

const PACKAGE_ID: &str = "plurora/experience-observability-lab";

async fn load_experience_observability_lab(
) -> anyhow::Result<plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/experience-observability-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    Ok(runtime)
}

async fn invoke(
    runtime: &plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>,
    cap: &str,
    input: serde_json::Value,
) -> anyhow::Result<plurora_runtime::CapabilityInvocationResult> {
    runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{PACKAGE_ID}/{cap}")),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input,
        })
        .await
        .map_err(Into::into)
}

/// Case 1: Observability contract — 8 capabilities, 3 surfaces, ordinary package,
/// no forbidden namespace, output shapes defined.
pub(crate) async fn experience_observability_contract() -> anyhow::Result<()> {
    let runtime = load_experience_observability_lab().await?;

    let contract = invoke(&runtime, "describe_observability", json!({})).await?;

    anyhow::ensure!(
        contract.output["kind"] == json!("experience_observability_contract"),
        "describe_observability must return experience_observability_contract kind"
    );
    anyhow::ensure!(
        contract.output["package_kind"] == json!("ordinary"),
        "must be ordinary package"
    );

    // 3 surfaces
    let surfaces = contract.output["surfaces"].as_object().unwrap();
    anyhow::ensure!(
        surfaces.contains_key("forge_panel"),
        "must have forge_panel surface"
    );
    anyhow::ensure!(
        surfaces.contains_key("assistant_action"),
        "must have assistant_action surface"
    );
    anyhow::ensure!(
        surfaces.contains_key("home_card"),
        "must have home_card surface"
    );

    // 8 capabilities
    anyhow::ensure!(
        contract.output["capabilities"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
            == 8,
        "describe_observability must list 8 capabilities"
    );

    // Output shapes defined
    anyhow::ensure!(
        contract.output["output_shapes"].is_object(),
        "must have output_shapes"
    );
    anyhow::ensure!(
        contract.output["output_shapes"]["session_health"].is_array(),
        "output_shapes must have session_health"
    );
    anyhow::ensure!(
        contract.output["output_shapes"]["proposal_causal_chain"].is_array(),
        "output_shapes must have proposal_causal_chain"
    );

    // No inference / no network
    anyhow::ensure!(contract.output["inference_performed"] == json!(false));
    anyhow::ensure!(contract.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 2: Session health — status derived from inputs, no SQLite reads.
pub(crate) async fn experience_observability_session_health() -> anyhow::Result<()> {
    let runtime = load_experience_observability_lab().await?;

    // Healthy session
    let healthy = invoke(
        &runtime,
        "summarize_session_health",
        json!({
            "session_id": "session:healthy",
            "event_count": 50,
            "package_count": 3,
            "proposal_count": 2,
            "asset_count": 10,
            "failure_count": 0,
        }),
    )
    .await?;
    anyhow::ensure!(healthy.output["kind"] == json!("session_health"));
    anyhow::ensure!(healthy.output["status"] == json!("healthy"));
    anyhow::ensure!(healthy.output["event_count"] == json!(50));
    anyhow::ensure!(healthy.output["failure_count"] == json!(0));
    anyhow::ensure!(healthy.output["inference_performed"] == json!(false));

    // Degraded session (with failures)
    let degraded = invoke(
        &runtime,
        "summarize_session_health",
        json!({
            "session_id": "session:degraded",
            "event_count": 30,
            "failure_count": 2,
        }),
    )
    .await?;
    anyhow::ensure!(degraded.output["status"] == json!("degraded"));

    Ok(())
}

/// Case 3: Package health — status derived from inputs.
pub(crate) async fn experience_observability_package_health() -> anyhow::Result<()> {
    let runtime = load_experience_observability_lab().await?;

    let pkg_health = invoke(
        &runtime,
        "summarize_package_health",
        json!({
            "package_id": "plurora/playable-creation-board",
            "capability_count": 14,
            "error_count": 0,
        }),
    )
    .await?;

    anyhow::ensure!(pkg_health.output["kind"] == json!("package_health"));
    anyhow::ensure!(pkg_health.output["status"] == json!("loaded"));
    anyhow::ensure!(pkg_health.output["capability_count"] == json!(14));
    anyhow::ensure!(pkg_health.output["inference_performed"] == json!(false));
    anyhow::ensure!(pkg_health.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 4: Agent run health — status from inputs, no runtime internals.
pub(crate) async fn experience_observability_agent_run_health() -> anyhow::Result<()> {
    let runtime = load_experience_observability_lab().await?;

    let run_health = invoke(
        &runtime,
        "summarize_agent_run_health",
        json!({
            "run_id": "run:forge:board1",
            "status": "completed",
            "plan_node_count": 5,
            "candidate_count": 2,
            "inference_count": 1,
            "duration_hint_ms": 2500,
        }),
    )
    .await?;

    anyhow::ensure!(run_health.output["kind"] == json!("agent_run_health"));
    anyhow::ensure!(run_health.output["status"] == json!("completed"));
    anyhow::ensure!(run_health.output["plan_node_count"] == json!(5));
    anyhow::ensure!(run_health.output["candidate_count"] == json!(2));
    anyhow::ensure!(run_health.output["inference_performed"] == json!(false));

    Ok(())
}

/// Case 5: Proposal causal chain — each step has content_address.
pub(crate) async fn experience_observability_proposal_causality() -> anyhow::Result<()> {
    let runtime = load_experience_observability_lab().await?;

    let causal = invoke(
        &runtime,
        "trace_proposal_causality",
        json!({
            "proposal_id": "proposal:board1:1",
            "player_action_ref": "action:board1:1",
            "run_ref": "run:forge:board1",
        }),
    )
    .await?;

    anyhow::ensure!(causal.output["kind"] == json!("proposal_causal_chain"));
    anyhow::ensure!(causal.output["proposal_id"] == json!("proposal:board1:1"));

    // Every chain step must have content_address
    let chain = causal.output["chain"].as_array().unwrap();
    for (i, step) in chain.iter().enumerate() {
        anyhow::ensure!(
            step["content_address"].is_string(),
            "chain step {} must have content_address",
            i
        );
        anyhow::ensure!(
            step["step"].is_string(),
            "chain step {} must have step kind",
            i
        );
        anyhow::ensure!(step["ref"].is_string(), "chain step {} must have ref", i);
    }

    anyhow::ensure!(causal.output["inference_performed"] == json!(false));
    Ok(())
}

/// Case 6: Cost/latency summary — only refs, no raw secrets or cost data.
pub(crate) async fn experience_observability_cost_latency() -> anyhow::Result<()> {
    let runtime = load_experience_observability_lab().await?;

    let cost = invoke(
        &runtime,
        "summarize_cost_latency",
        json!({
            "session_id": "session:board1",
            "total_invocations": 12,
            "outbound_request_count": 3,
            "total_duration_hint_ms": 5000,
            "cost_refs": ["audit:session:board1:1", "audit:session:board1:2"],
        }),
    )
    .await?;

    anyhow::ensure!(cost.output["kind"] == json!("cost_latency_summary"));
    anyhow::ensure!(cost.output["total_invocations"] == json!(12));
    anyhow::ensure!(cost.output["outbound_request_count"] == json!(3));
    anyhow::ensure!(cost.output["total_duration_hint_ms"] == json!(5000));
    anyhow::ensure!(cost.output["cost_refs"].is_array(), "must have cost_refs");
    anyhow::ensure!(cost.output["inference_performed"] == json!(false));

    // No raw secret data
    let output_str = serde_json::to_string(&cost.output).unwrap();
    anyhow::ensure!(
        !output_str.contains("api_key"),
        "cost summary must not contain api_key"
    );
    anyhow::ensure!(
        !output_str.contains("Bearer"),
        "cost summary must not contain Bearer"
    );

    Ok(())
}

/// Case 7: Failure breadcrumbs — from protocol-visible event refs, not SQLite.
pub(crate) async fn experience_observability_failure_breadcrumbs() -> anyhow::Result<()> {
    let runtime = load_experience_observability_lab().await?;

    let breadcrumbs = invoke(
        &runtime,
        "list_failure_breadcrumbs",
        json!({
            "session_id": "session:board1",
            "failure_kind": "proposal_rejected",
            "sequence": 5,
        }),
    )
    .await?;

    anyhow::ensure!(breadcrumbs.output["kind"] == json!("failure_breadcrumbs"));
    anyhow::ensure!(
        breadcrumbs.output["breadcrumbs"].is_array(),
        "must have breadcrumbs array"
    );
    anyhow::ensure!(
        breadcrumbs.output["breadcrumbs"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false),
        "must have at least one breadcrumb"
    );
    anyhow::ensure!(breadcrumbs.output["inference_performed"] == json!(false));

    // Breadcrumbs must have kind, ref, sequence
    let crumbs = breadcrumbs.output["breadcrumbs"].as_array().unwrap();
    for (i, crumb) in crumbs.iter().enumerate() {
        anyhow::ensure!(crumb["kind"].is_string(), "breadcrumb {} must have kind", i);
        anyhow::ensure!(crumb["ref"].is_string(), "breadcrumb {} must have ref", i);
    }

    Ok(())
}

/// Case 8: Guardrail/audit summary — from protocol-visible audit refs.
pub(crate) async fn experience_observability_guardrail_summary() -> anyhow::Result<()> {
    let runtime = load_experience_observability_lab().await?;

    let guardrails = invoke(
        &runtime,
        "summarize_guardrails",
        json!({
            "session_id": "session:board1",
            "guardrail_kind": "raw_secret_blocked",
            "sequence": 3,
        }),
    )
    .await?;

    anyhow::ensure!(guardrails.output["kind"] == json!("guardrail_audit_summary"));
    anyhow::ensure!(
        guardrails.output["total_guardrails"]
            .as_u64()
            .map(|c| c > 0)
            .unwrap_or(false),
        "must have total_guardrails > 0"
    );
    anyhow::ensure!(
        guardrails.output["guardrails"].is_array(),
        "must have guardrails array"
    );
    anyhow::ensure!(guardrails.output["inference_performed"] == json!(false));
    anyhow::ensure!(guardrails.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 9: No forbidden namespace in any output.
/// platform.observability.* is forbidden — observability is package-owned.
pub(crate) async fn experience_observability_no_forbidden_namespace() -> anyhow::Result<()> {
    let runtime = load_experience_observability_lab().await?;

    let caps = [
        "describe_observability",
        "summarize_session_health",
        "summarize_package_health",
        "summarize_agent_run_health",
        "trace_proposal_causality",
        "summarize_cost_latency",
        "list_failure_breadcrumbs",
        "summarize_guardrails",
    ];

    let forbidden = [
        "platform.observability.",
        "platform.experience.",
        "platform.world.",
        "platform.scene.",
        "platform.turn.",
        "platform.chat.",
        "platform.memory.",
        "platform.agent.",
        "platform.model.",
        "platform.prompt.",
        "platform.director.",
    ];

    for cap in &caps {
        let result = invoke(&runtime, cap, json!({"session_id": "session:ns"})).await?;
        let output_str = serde_json::to_string(&result.output).unwrap();
        for token in &forbidden {
            anyhow::ensure!(
                !output_str.contains(token),
                "{cap} must not contain {token}"
            );
        }
    }

    Ok(())
}

/// Case 10: No raw secrets in any capability output or input processing.
pub(crate) async fn experience_observability_no_raw_secrets() -> anyhow::Result<()> {
    let runtime = load_experience_observability_lab().await?;

    // summarize_session_health blocks raw secret
    let health = invoke(
        &runtime,
        "summarize_session_health",
        json!({
            "session_id": "session:secret",
            "api_key": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(health.output["kind"] == json!("experience_observability_rejected"));
    anyhow::ensure!(health.output["redaction_state"] == json!("unsafe_blocked"));

    // trace_proposal_causality blocks raw secret
    let causal = invoke(
        &runtime,
        "trace_proposal_causality",
        json!({
            "proposal_id": "p1",
            "token": "Bearer abc123",
        }),
    )
    .await?;
    anyhow::ensure!(causal.output["kind"] == json!("experience_observability_rejected"));

    // summarize_guardrails blocks raw secret
    let guardrails = invoke(
        &runtime,
        "summarize_guardrails",
        json!({
            "session_id": "session:secret",
            "secret": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(guardrails.output["kind"] == json!("experience_observability_rejected"));

    Ok(())
}
