//! Conformance tests for `official/experience-runtime-lab` (Experience Beta 0).
//!
//! Covers:
//! - describe_contract shape (surfaces, capabilities, lifecycle states)
//! - checkpoint/recovery shape (create, inspect, draft recovery)
//! - No platform.experience.* / platform.world.* / platform.turn.* namespace
//! - Template generation (experience-runtime template produces 4 surfaces)
//! - Third-party shape parity / ordinary routing

use std::path::PathBuf;

use plurora_runtime::CapabilityInvocationRequest;
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

const PACKAGE_ID: &str = "official/experience-runtime-lab";

async fn load_experience_runtime_lab(
) -> anyhow::Result<plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/official/experience-runtime-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    Ok(runtime)
}

/// Case 1: describe_contract returns all surfaces, capabilities, lifecycle states,
/// checkpoint formats, and recovery strategies. No kernel experience namespace.
pub(crate) async fn experience_runtime_describe_contract() -> anyhow::Result<()> {
    let runtime = load_experience_runtime_lab().await?;

    let contract = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{PACKAGE_ID}/describe_contract")),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await?;

    anyhow::ensure!(
        contract.output["kind"] == json!("experience_runtime_contract"),
        "describe_contract must return experience_runtime_contract kind"
    );
    anyhow::ensure!(
        contract.output["package_kind"] == json!("ordinary"),
        "describe_contract must have package_kind=ordinary"
    );

    // 4 surfaces
    let surfaces = contract.output["surfaces"].as_object().unwrap();
    anyhow::ensure!(
        surfaces.contains_key("experience_entry"),
        "must have experience_entry surface"
    );
    anyhow::ensure!(
        surfaces.contains_key("play_renderer"),
        "must have play_renderer surface"
    );
    anyhow::ensure!(
        surfaces.contains_key("forge_panel"),
        "must have forge_panel surface"
    );
    anyhow::ensure!(
        surfaces.contains_key("assistant_action"),
        "must have assistant_action surface"
    );

    // 5 capabilities
    anyhow::ensure!(
        contract.output["capabilities"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
            == 5,
        "describe_contract must list 5 capabilities"
    );

    // Lifecycle states
    anyhow::ensure!(
        contract.output["lifecycle_states"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
            == 9,
        "describe_contract must have 9 lifecycle states"
    );

    // Checkpoint formats
    anyhow::ensure!(
        contract.output["checkpoint_formats"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
            == 3,
        "describe_contract must have 3 checkpoint formats"
    );

    // Recovery strategies
    anyhow::ensure!(
        contract.output["recovery_strategies"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
            == 5,
        "describe_contract must have 5 recovery strategies"
    );

    anyhow::ensure!(
        contract.output["inference_performed"] == json!(false),
        "describe_contract must have inference_performed=false"
    );
    anyhow::ensure!(
        contract.output["network_performed"] == json!(false),
        "describe_contract must have network_performed=false"
    );

    // No kernel experience namespace
    let output_str = serde_json::to_string(&contract.output).unwrap();
    anyhow::ensure!(
        !output_str.contains("platform.experience."),
        "must not contain platform.experience."
    );
    anyhow::ensure!(
        !output_str.contains("platform.world."),
        "must not contain platform.world."
    );
    anyhow::ensure!(
        !output_str.contains("platform.turn."),
        "must not contain platform.turn."
    );

    Ok(())
}

/// Case 2: create_checkpoint and inspect_checkpoint produce correct shapes.
/// Checkpoint is deterministic, no inference, no network.
pub(crate) async fn experience_runtime_checkpoint_shape() -> anyhow::Result<()> {
    let runtime = load_experience_runtime_lab().await?;

    let checkpoint = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{PACKAGE_ID}/create_checkpoint")),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input: json!({
                "session_id": "session_conf",
                "state_snapshot": {"health": 100, "step_index": 5},
                "asset_refs": ["asset:module:seed"],
                "branch_ref": "branch:target:main",
                "sequence": 3,
            }),
        })
        .await?;

    anyhow::ensure!(
        checkpoint.output["kind"] == json!("experience_checkpoint"),
        "create_checkpoint must return experience_checkpoint kind"
    );
    anyhow::ensure!(
        checkpoint.output["session_id"] == json!("session_conf"),
        "checkpoint session_id"
    );
    anyhow::ensure!(
        checkpoint.output["format"] == json!("snapshot"),
        "checkpoint default format is snapshot"
    );
    anyhow::ensure!(
        checkpoint.output["sequence"] == json!(3),
        "checkpoint sequence"
    );
    anyhow::ensure!(
        checkpoint.output["inference_performed"] == json!(false),
        "checkpoint must have inference_performed=false"
    );
    anyhow::ensure!(
        checkpoint.output["network_performed"] == json!(false),
        "checkpoint must have network_performed=false"
    );

    // Inspect checkpoint
    let inspection = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{PACKAGE_ID}/inspect_checkpoint")),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input: json!({
                "checkpoint_id": "cp:1",
                "session_id": "session_conf",
                "state_snapshot": {"health": 100},
                "format": "snapshot",
                "sequence": 1,
            }),
        })
        .await?;

    anyhow::ensure!(
        inspection.output["kind"] == json!("experience_checkpoint_inspection"),
        "inspect_checkpoint must return experience_checkpoint_inspection kind"
    );
    anyhow::ensure!(
        inspection.output["valid"] == json!(true),
        "valid checkpoint inspection must return valid=true"
    );

    Ok(())
}

/// Case 3: draft_recovery produces correct recovery plan with strategy,
/// steps, and checkpoint_available flag. No platform-reserved namespace.
pub(crate) async fn experience_runtime_recovery_shape() -> anyhow::Result<()> {
    let runtime = load_experience_runtime_lab().await?;

    // With checkpoint available
    let recovery_with_cp = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{PACKAGE_ID}/draft_recovery")),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input: json!({
                "session_id": "session_conf",
                "failure_kind": "state_corruption",
                "last_checkpoint_ref": "checkpoint:123",
            }),
        })
        .await?;

    anyhow::ensure!(
        recovery_with_cp.output["kind"] == json!("experience_recovery_plan"),
        "draft_recovery must return experience_recovery_plan kind"
    );
    anyhow::ensure!(
        recovery_with_cp.output["recommended_strategy"] == json!("restore_last_checkpoint"),
        "with checkpoint, recommended_strategy should be restore_last_checkpoint"
    );
    anyhow::ensure!(
        recovery_with_cp.output["plan"]["checkpoint_available"] == json!(true),
        "plan must reflect checkpoint_available=true"
    );
    anyhow::ensure!(
        recovery_with_cp.output["plan"]["steps"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
            > 0,
        "recovery plan must have steps"
    );
    anyhow::ensure!(
        recovery_with_cp.output["inference_performed"] == json!(false),
        "recovery must have inference_performed=false"
    );

    // Without checkpoint
    let recovery_no_cp = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{PACKAGE_ID}/draft_recovery")),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input: json!({
                "session_id": "session_conf",
                "failure_kind": "checkpoint_missing",
            }),
        })
        .await?;

    anyhow::ensure!(
        recovery_no_cp.output["recommended_strategy"] == json!("restart_session"),
        "without checkpoint, recommended_strategy should be restart_session"
    );
    anyhow::ensure!(
        recovery_no_cp.output["plan"]["checkpoint_available"] == json!(false),
        "plan must reflect checkpoint_available=false"
    );

    // No platform-reserved namespace
    let output_str = serde_json::to_string(&recovery_with_cp.output).unwrap();
    anyhow::ensure!(
        !output_str.contains("platform.experience."),
        "recovery must not contain platform.experience."
    );
    anyhow::ensure!(
        !output_str.contains("platform.turn."),
        "recovery must not contain platform.turn."
    );

    Ok(())
}

/// Case 4: no platform.experience.* / platform.world.* / platform.turn.* namespace
/// in any output from the package.
pub(crate) async fn experience_runtime_no_platform_namespace() -> anyhow::Result<()> {
    let runtime = load_experience_runtime_lab().await?;

    let caps = [
        format!("{PACKAGE_ID}/describe_contract"),
        format!("{PACKAGE_ID}/create_checkpoint"),
        format!("{PACKAGE_ID}/inspect_checkpoint"),
        format!("{PACKAGE_ID}/draft_recovery"),
        format!("{PACKAGE_ID}/bind_agent_run"),
    ];

    for cap in &caps {
        let result = runtime
            .invoke_capability(CapabilityInvocationRequest {
                handle: None,
                capability_id: Some(cap.clone()),
                caller_package_id: None,
                provider_package_id: Some(PACKAGE_ID.to_string()),
                version: None,
                session_id: None,
                input: json!({"session_id": "session_ns_test"}),
            })
            .await?;

        let output_str = serde_json::to_string(&result.output).unwrap();
        anyhow::ensure!(
            !output_str.contains("platform.experience."),
            "{cap} must not contain platform.experience."
        );
        anyhow::ensure!(
            !output_str.contains("platform.world."),
            "{cap} must not contain platform.world."
        );
        anyhow::ensure!(
            !output_str.contains("platform.turn."),
            "{cap} must not contain platform.turn."
        );
        anyhow::ensure!(
            !output_str.contains("platform.chat."),
            "{cap} must not contain platform.chat."
        );
        anyhow::ensure!(
            !output_str.contains("platform.memory."),
            "{cap} must not contain platform.memory."
        );
    }

    Ok(())
}

/// Case 5: experience-runtime template generates a package with 4 surfaces
/// (experience_entry, play_renderer, forge_panel, assistant_action),
/// contract/checkpoint/recovery capabilities, and passes check/conformance.
pub(crate) async fn experience_runtime_template_generation() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "plurora-generated-experience-runtime-{}",
        std::process::id()
    ));
    if path.exists() {
        std::fs::remove_dir_all(&path)?;
    }

    crate::commands::package::init_package(
        path.clone(),
        "example/generated-experience-runtime".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(crate::cli::PackageTemplate::ExperienceRuntime),
    )
    .await?;

    crate::commands::package::package_check(path.join("manifest.yaml")).await?;
    crate::commands::package::package_conformance(path.join("manifest.yaml")).await?;

    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;

    // 4 surfaces
    anyhow::ensure!(
        manifest.contributes.surfaces.len() == 4,
        "experience-runtime template should have 4 surfaces, got {}",
        manifest.contributes.surfaces.len()
    );

    let slots: Vec<&str> = manifest
        .contributes
        .surfaces
        .iter()
        .map(|s| match s.slot {
            plurora_core::SurfaceSlot::ExperienceEntry => "experience_entry",
            plurora_core::SurfaceSlot::PlayRenderer => "play_renderer",
            plurora_core::SurfaceSlot::ForgePanel => "forge_panel",
            plurora_core::SurfaceSlot::AssistantAction => "assistant_action",
            plurora_core::SurfaceSlot::AssetEditor => "asset_editor",
            plurora_core::SurfaceSlot::HomeCard => "home_card",
            plurora_core::SurfaceSlot::QuickAction => "quick_action",
            plurora_core::SurfaceSlot::WorkshopCard => "workshop_card",
        })
        .collect();
    anyhow::ensure!(
        slots.contains(&"experience_entry"),
        "experience-runtime should have experience_entry"
    );
    anyhow::ensure!(
        slots.contains(&"play_renderer"),
        "experience-runtime should have play_renderer"
    );
    anyhow::ensure!(
        slots.contains(&"forge_panel"),
        "experience-runtime should have forge_panel"
    );
    anyhow::ensure!(
        slots.contains(&"assistant_action"),
        "experience-runtime should have assistant_action"
    );

    // Contract/checkpoint/recovery capabilities (5 + echo = 6)
    anyhow::ensure!(
        manifest.provides.len() == 6,
        "experience-runtime template should have 6 capabilities, got {}",
        manifest.provides.len()
    );

    // No network declarations
    anyhow::ensure!(
        manifest.permissions.network.declarations.is_empty(),
        "experience-runtime should have no network declarations"
    );

    // No platform-reserved namespace in manifest or package.ts
    let manifest_json = serde_json::to_value(&manifest)?;
    let manifest_str = serde_json::to_string(&manifest_json)?;
    for token in &[
        "platform.experience.",
        "platform.world.",
        "platform.turn.",
        "platform.chat.",
        "platform.memory.",
    ] {
        anyhow::ensure!(
            !manifest_str.contains(token),
            "experience-runtime manifest must not contain '{}' text",
            token
        );
    }

    let package_ts = std::fs::read_to_string(path.join("package.ts"))?;
    for token in &[
        "platform.experience.",
        "platform.world.",
        "platform.turn.",
        "platform.chat.",
        "platform.memory.",
    ] {
        anyhow::ensure!(
            !package_ts.contains(token),
            "experience-runtime package.ts must not contain '{}' text",
            token
        );
    }

    std::fs::remove_dir_all(path)?;
    Ok(())
}

/// Case 6: bind_agent_run produces correct shape linking experience session
/// to an agentic forge run with scoped branch, forge binding, and assist binding.
pub(crate) async fn experience_runtime_bind_agent_run() -> anyhow::Result<()> {
    let runtime = load_experience_runtime_lab().await?;

    let binding = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{PACKAGE_ID}/bind_agent_run")),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input: json!({
                "session_id": "session_bind",
                "agent_package_id": "official/agentic-forge-lab",
                "target_branch_ref": "branch:target:main",
                "scratch_branch_ref": "branch:scratch:s1",
            }),
        })
        .await?;

    anyhow::ensure!(
        binding.output["kind"] == json!("experience_agent_run_binding"),
        "bind_agent_run must return experience_agent_run_binding kind"
    );
    anyhow::ensure!(
        binding.output["scoped_to_branch"] == json!(true),
        "bind_agent_run must be scoped_to_branch=true"
    );
    anyhow::ensure!(
        binding.output["forge_panel_binding"]["branch_aware"] == json!(true),
        "forge_panel_binding must be branch_aware=true"
    );
    anyhow::ensure!(
        binding.output["assist_binding"]["approval_policy"] == json!("fork_then_approve"),
        "assist_binding must have fork_then_approve"
    );
    anyhow::ensure!(
        binding.output["inference_performed"] == json!(false),
        "bind_agent_run must have inference_performed=false"
    );
    anyhow::ensure!(
        binding.output["network_performed"] == json!(false),
        "bind_agent_run must have network_performed=false"
    );

    // No platform-reserved namespace
    let output_str = serde_json::to_string(&binding.output).unwrap();
    anyhow::ensure!(
        !output_str.contains("platform.experience."),
        "binding must not contain platform.experience."
    );

    Ok(())
}
