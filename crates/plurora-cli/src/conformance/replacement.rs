//! Conformance cases for Phase H5: proving Plurora seed is replaceable
//! with a third-party package, no platform privilege or hardcoding required.
//!
//! Also covers Phase J6: proving third-party agent runtime is replaceable
//! with the first-party pi-agent-runtime-lab.

use std::path::PathBuf;

use plurora_runtime::{CapabilityInvocationRequest, ProtocolContext};
use serde_json::json;

use super::fixtures::*;
use crate::commands::{composition, manifest};

/// Proves that the third-party playable-seed replacement package loads and its
/// surfaces are discoverable through `shell.contribution.list`.
pub(crate) async fn thirdparty_seed_surfaces() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/thirdparty-playable-seed/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // List all surfaces — should include at least the 5 from thirdparty-playable-seed
    let all = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.list",
            json!({}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        all.as_array().map(|items| items.len()).unwrap_or(0) >= 5,
        "third-party surfaces not listed"
    );

    // Check experience_entry slot
    let entries = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.list",
            json!({"slot": "experience_entry"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let has_entry = entries
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|r| r["package_id"] == json!("thirdparty/playable-seed"))
        })
        .unwrap_or(false);
    anyhow::ensure!(has_entry, "third-party experience_entry surface missing");

    // Check play_renderer slot
    let renderers = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.list",
            json!({"slot": "play_renderer"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let has_renderer = renderers
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|r| r["package_id"] == json!("thirdparty/playable-seed"))
        })
        .unwrap_or(false);
    anyhow::ensure!(has_renderer, "third-party play_renderer surface missing");

    // Check forge_panel slot
    let forge_panels = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.list",
            json!({"slot": "forge_panel"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let has_forge = forge_panels
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|r| r["package_id"] == json!("thirdparty/playable-seed"))
        })
        .unwrap_or(false);
    anyhow::ensure!(has_forge, "third-party forge_panel surface missing");

    // Check assistant_action slot
    let assistants = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.list",
            json!({"slot": "assistant_action"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let has_assist = assistants
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|r| r["package_id"] == json!("thirdparty/playable-seed"))
        })
        .unwrap_or(false);
    anyhow::ensure!(has_assist, "third-party assistant_action surface missing");

    // Check asset_editor slot
    let editors = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.list",
            json!({"slot": "asset_editor"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let has_editor = editors
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|r| r["package_id"] == json!("thirdparty/playable-seed"))
        })
        .unwrap_or(false);
    anyhow::ensure!(has_editor, "third-party asset_editor surface missing");

    Ok(())
}

/// Proves that capability invocation works for the third-party playable-seed
/// package through normal routing.
pub(crate) async fn thirdparty_seed_invocation() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/thirdparty-playable-seed/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // Invoke the launch capability through normal routing (echo inproc echoes input)
    let launch = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("thirdparty/playable-seed/launch".to_string()),
            caller_package_id: None,
            provider_package_id: Some("thirdparty/playable-seed".to_string()),
            version: None,
            session_id: None,
            input: json!({"title": "Community Seed"}),
        })
        .await?;
    // Echo inproc returns input, so verify the input is echoed
    anyhow::ensure!(
        launch.output["title"] == json!("Community Seed"),
        "third-party launch capability did not echo input"
    );

    // Invoke render_payload capability
    let render = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("thirdparty/playable-seed/render_payload".to_string()),
            caller_package_id: None,
            provider_package_id: Some("thirdparty/playable-seed".to_string()),
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await?;
    anyhow::ensure!(
        render.provider_package_id == "thirdparty/playable-seed",
        "third-party render routed to wrong provider"
    );

    Ok(())
}

/// Proves that when both a first-party and a third-party Package provide the same
/// capability ID, the kernel does NOT prefer the first-party Package. The ambiguous
/// route is rejected, requiring explicit provider selection — same as any other
/// duplicate-provider scenario.
pub(crate) async fn ambiguous_no_publisher_priority() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();

    // Load an first-party Package that provides a shared capability ID
    runtime
        .load_package(echo_package(
            "plurora/replacement-fixture",
            "shared/playable-seed/launch",
        ))
        .await?;
    // Load a third-party package that provides the SAME capability ID
    runtime
        .load_package(echo_package(
            "thirdparty/replacement-fixture",
            "shared/playable-seed/launch",
        ))
        .await?;

    // Without explicit provider, the route should be ambiguous — NOT preferred to the first-party implementation
    let denied = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("shared/playable-seed/launch".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await;
    anyhow::ensure!(
        denied.is_err(),
        "ambiguous route should be rejected, first-party Package should NOT win"
    );

    // With explicit third-party provider, it should work — proving no publisher priority
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("shared/playable-seed/launch".to_string()),
            caller_package_id: None,
            provider_package_id: Some("thirdparty/replacement-fixture".to_string()),
            version: None,
            session_id: None,
            input: json!({"source": "thirdparty"}),
        })
        .await?;
    anyhow::ensure!(
        result.provider_package_id == "thirdparty/replacement-fixture",
        "explicit third-party provider was not used"
    );

    Ok(())
}

/// Proves that composition check passes with the third-party replacement
/// composition descriptor.
pub(crate) async fn composition_thirdparty() -> anyhow::Result<()> {
    composition::composition_check(PathBuf::from(
        "examples/compositions/playable-seed-replacement/composition.yaml",
    ))
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Phase J6 — Third-party agent runtime replacement proof
// ---------------------------------------------------------------------------

/// Proves that the third-party agent-runtime replacement package loads and its
/// surfaces (assistant_action, forge_panel, home_card) are discoverable through
/// `shell.contribution.list`.
pub(crate) async fn thirdparty_agent_runtime_surfaces() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/thirdparty-agent-runtime/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // Check assistant_action slot
    let assistants = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.list",
            json!({"slot": "assistant_action"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let has_assistant = assistants
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|r| r["package_id"] == json!("thirdparty/agent-runtime"))
        })
        .unwrap_or(false);
    anyhow::ensure!(
        has_assistant,
        "third-party agent-runtime assistant_action surface missing"
    );

    // Check forge_panel slot
    let forge_panels = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.list",
            json!({"slot": "forge_panel"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let has_forge = forge_panels
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|r| r["package_id"] == json!("thirdparty/agent-runtime"))
        })
        .unwrap_or(false);
    anyhow::ensure!(
        has_forge,
        "third-party agent-runtime forge_panel surface missing"
    );

    // Check home_card slot
    let home_cards = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.list",
            json!({"slot": "home_card"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let has_home = home_cards
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|r| r["package_id"] == json!("thirdparty/agent-runtime"))
        })
        .unwrap_or(false);
    anyhow::ensure!(
        has_home,
        "third-party agent-runtime home_card surface missing"
    );

    Ok(())
}

/// Proves that the third-party agent-runtime capabilities produce
/// deterministic, no-network, no-inference, approval-gated output
/// — the same constraints as the first-party pi-agent-runtime-lab.
pub(crate) async fn thirdparty_agent_runtime_invocation() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/thirdparty-agent-runtime/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // run: deterministic no-inference no-network plan
    let run_result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("thirdparty/agent-runtime/run".to_string()),
            caller_package_id: None,
            provider_package_id: Some("thirdparty/agent-runtime".to_string()),
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await?;
    anyhow::ensure!(
        run_result.output["kind"] == json!("thirdparty_agent_run_plan"),
        "thirdparty agent-runtime run returned wrong kind"
    );
    anyhow::ensure!(
        run_result.output["inference_performed"] == json!(false),
        "thirdparty agent-runtime run must not perform inference"
    );
    anyhow::ensure!(
        run_result.output["network_performed"] == json!(false),
        "thirdparty agent-runtime run must not perform network"
    );
    anyhow::ensure!(
        run_result.output["trace_events"].is_array(),
        "thirdparty agent-runtime run missing trace_events"
    );
    anyhow::ensure!(
        run_result.output["stream_frames"].is_array(),
        "thirdparty agent-runtime run missing stream_frames"
    );
    anyhow::ensure!(
        run_result.output["proposal_draft"].is_object(),
        "thirdparty agent-runtime run missing proposal_draft"
    );
    anyhow::ensure!(
        run_result.output["provenance"]["package_id"] == json!("thirdparty/agent-runtime"),
        "thirdparty agent-runtime run provenance mismatch"
    );

    // draft_proposal: approval-gated
    let proposal = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("thirdparty/agent-runtime/draft_proposal".to_string()),
            caller_package_id: None,
            provider_package_id: Some("thirdparty/agent-runtime".to_string()),
            version: None,
            session_id: None,
            input: json!({"change": "community-driven modification"}),
        })
        .await?;
    anyhow::ensure!(
        proposal.output["kind"] == json!("thirdparty_agent_proposal"),
        "thirdparty agent-runtime draft_proposal wrong kind"
    );
    anyhow::ensure!(
        proposal.output["requires_user_approval"] == json!(true),
        "thirdparty agent-runtime proposal must require approval"
    );
    anyhow::ensure!(
        proposal.output["provenance"]["package_id"] == json!("thirdparty/agent-runtime"),
        "thirdparty agent-runtime proposal provenance mismatch"
    );

    // explain_run: no-inference explanation
    let explain = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("thirdparty/agent-runtime/explain_run".to_string()),
            caller_package_id: None,
            provider_package_id: Some("thirdparty/agent-runtime".to_string()),
            version: None,
            session_id: None,
            input: json!({"trace_events": [{"step": 1}, {"step": 2}]}),
        })
        .await?;
    anyhow::ensure!(
        explain.output["kind"] == json!("thirdparty_agent_run_explanation"),
        "thirdparty agent-runtime explain_run wrong kind"
    );
    anyhow::ensure!(
        explain.output["inference_performed"] == json!(false),
        "thirdparty agent-runtime explain_run must not claim inference"
    );
    anyhow::ensure!(
        explain.output["network_performed"] == json!(false),
        "thirdparty agent-runtime explain_run must not claim network"
    );
    anyhow::ensure!(
        explain.output["trace_event_count"] == json!(2),
        "thirdparty agent-runtime explain_run wrong event count"
    );

    // summarize_trace: no-inference summary
    let trace = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("thirdparty/agent-runtime/summarize_trace".to_string()),
            caller_package_id: None,
            provider_package_id: Some("thirdparty/agent-runtime".to_string()),
            version: None,
            session_id: None,
            input: json!({"trace_events": [{"e": 1}, {"e": 2}, {"e": 3}]}),
        })
        .await?;
    anyhow::ensure!(
        trace.output["kind"] == json!("thirdparty_agent_trace_summary"),
        "thirdparty agent-runtime summarize_trace wrong kind"
    );
    anyhow::ensure!(
        trace.output["event_count"] == json!(3),
        "thirdparty agent-runtime summarize_trace wrong event count"
    );
    anyhow::ensure!(
        trace.output["inference_performed"] == json!(false),
        "thirdparty agent-runtime summarize_trace must not claim inference"
    );
    anyhow::ensure!(
        trace.output["network_performed"] == json!(false),
        "thirdparty agent-runtime summarize_trace must not claim network"
    );

    // echo: passthrough
    let echo = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("thirdparty/agent-runtime/echo".to_string()),
            caller_package_id: None,
            provider_package_id: Some("thirdparty/agent-runtime".to_string()),
            version: None,
            session_id: None,
            input: json!({"hello": "community"}),
        })
        .await?;
    anyhow::ensure!(
        echo.output["kind"] == json!("thirdparty_agent_echo"),
        "thirdparty agent-runtime echo wrong kind"
    );
    anyhow::ensure!(
        echo.output["input"]["hello"] == json!("community"),
        "thirdparty agent-runtime echo did not pass through input"
    );

    Ok(())
}

/// Proves that when both the first-party pi-agent-runtime-lab and the third-party
/// agent-runtime are loaded, the composition check with the third-party as the
/// required package and the first-party Package as replacement_candidate succeeds. This
/// verifies no publisher priority: the third-party is the selected provider and
/// the first-party Package is only a candidate.
pub(crate) async fn composition_agent_runtime_replacement() -> anyhow::Result<()> {
    composition::composition_check(PathBuf::from(
        "examples/compositions/agent-runtime-replacement/composition.yaml",
    ))
    .await?;
    Ok(())
}
