use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use plurora_core::PackageEntry;
use plurora_runtime::{CapabilityInvocationRequest, InMemoryEventStore, Runtime, RuntimeConfig};
use serde_json::json;

use super::manifest::read_manifest;
use crate::cli::PackageTemplate;
use crate::templates;

pub(crate) async fn package_load(path: PathBuf) -> Result<()> {
    let manifest = read_manifest(path).await?;
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());
    let record = runtime.load_package(manifest).await?;
    println!(
        "loaded package: {}@{} ({:?})",
        record.id, record.version, record.state
    );
    Ok(())
}

pub(crate) async fn package_check(path: PathBuf) -> Result<()> {
    let manifest = read_manifest(path).await?;
    manifest.validate_basic()?;

    // Structured diagnostics
    let entry_kind = match &manifest.entry.kind {
        PackageEntry::RustInproc { .. } => "rust_inproc",
        PackageEntry::Subprocess { .. } => "subprocess",
        PackageEntry::Wasm { .. } => "wasm",
        PackageEntry::Remote { .. } => "remote",
        PackageEntry::SurfaceBundle { .. } => "surface_bundle",
    };
    let trust_level = match &manifest.entry.kind {
        PackageEntry::RustInproc { .. } => "trusted_inproc",
        PackageEntry::Subprocess { .. } => "process_isolated",
        PackageEntry::Wasm { .. } => "wasm_sandbox",
        PackageEntry::Remote { .. } => "remote_boundary",
        PackageEntry::SurfaceBundle { .. } => "static_surface",
    };

    let cap_count = manifest.provides.len();
    let mut surfaces_by_slot: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for surface in &manifest.contributes.surfaces {
        let slot_name = match surface.slot {
            plurora_core::SurfaceSlot::ExperienceEntry => "experience_entry",
            plurora_core::SurfaceSlot::HomeCard => "home_card",
            plurora_core::SurfaceSlot::QuickAction => "quick_action",
            plurora_core::SurfaceSlot::WorkshopCard => "workshop_card",
            plurora_core::SurfaceSlot::PlayRenderer => "play_renderer",
            plurora_core::SurfaceSlot::ForgePanel => "forge_panel",
            plurora_core::SurfaceSlot::AssetEditor => "asset_editor",
            plurora_core::SurfaceSlot::AssistantAction => "assistant_action",
        };
        surfaces_by_slot
            .entry(slot_name.to_string())
            .or_default()
            .push(surface.id.clone());
    }

    let perm = &manifest.permissions;
    let permissions_summary = json!({
        "events": {"read": perm.events.read, "append": perm.events.append},
        "capabilities_invoke": perm.capabilities.invoke.len(),
        "packages_call": perm.packages.call.len(),
        "assets": {"read": perm.assets.read, "write": perm.assets.write},
        "network_hosts": perm.network.hosts.len(),
        "network_declarations": perm.network.declarations.len(),
        "filesystem_read": perm.filesystem.read.len(),
        "filesystem_write": perm.filesystem.write.len(),
    });

    let sandbox = &manifest.sandbox_policy;
    let sandbox_summary = json!({
        "cpu_quota_ms_per_invoke": sandbox.cpu_quota_ms_per_invoke,
        "memory_mb": sandbox.memory_mb,
        "wall_clock_ms": sandbox.wall_clock_ms,
    });

    let mut warnings: Vec<String> = Vec::new();
    if cap_count == 0 {
        warnings.push("package provides no capabilities".to_string());
    }
    if manifest.contributes.surfaces.is_empty() {
        warnings.push("package contributes no surfaces".to_string());
    }
    if !perm.network.declarations.is_empty() || !perm.network.hosts.is_empty() {
        warnings.push("package requests network access — ensure allowlist is minimal".to_string());
    }

    // --- Creator-facing diagnostics (Beta 5) ---

    // Experience surface coverage: check for common surface slots
    let has_experience_entry = surfaces_by_slot.contains_key("experience_entry");
    let has_play_renderer = surfaces_by_slot.contains_key("play_renderer");
    let has_forge_panel = surfaces_by_slot.contains_key("forge_panel");
    let has_assistant_action = surfaces_by_slot.contains_key("assistant_action");

    if has_experience_entry && !has_play_renderer {
        warnings.push("experience_entry surface present but no play_renderer — playable experiences usually need both".to_string());
    }
    if has_experience_entry && !has_forge_panel {
        warnings.push("experience_entry surface present but no forge_panel — creators cannot inspect state through Forge".to_string());
    }
    if has_experience_entry && !has_assistant_action {
        warnings.push("experience_entry surface present but no assistant_action — no way to request changes via assistant".to_string());
    }

    // Checkpoint/recovery capability coverage for experience packages
    // Build a BTreeSet for exact capability suffix lookups
    let cap_ids: Vec<&str> = manifest.provides.iter().map(|c| c.id.as_str()).collect();
    let cap_suffix_set: BTreeSet<&str> = cap_ids
        .iter()
        .filter_map(|c| c.rfind('/').map(|i| &c[i..]))
        .collect();
    if has_experience_entry {
        let has_launch = cap_suffix_set.contains("/launch")
            || cap_suffix_set.contains("/describe_contract")
            || cap_suffix_set.contains("/describe-contract");
        if !has_launch {
            warnings.push("experience_entry surface has no launch/describe_contract capability — surface activation may fail".to_string());
        }
        let has_checkpoint = cap_suffix_set.contains("/create_checkpoint")
            || cap_suffix_set.contains("/create-checkpoint");
        let has_recovery = cap_suffix_set.contains("/draft_recovery")
            || cap_suffix_set.contains("/draft-recovery");
        if !has_checkpoint {
            warnings.push("missing create_checkpoint capability — experience cannot be saved/restored mid-session".to_string());
        }
        if !has_recovery {
            warnings.push(
                "missing draft_recovery capability — experience cannot recover from failures"
                    .to_string(),
            );
        }
        let has_request_change = cap_suffix_set.contains("/request_change")
            || cap_suffix_set.contains("/request-change")
            || cap_suffix_set.contains("/draft-recovery")
            || cap_suffix_set.contains("/draft_recovery");
        if has_assistant_action && !has_request_change {
            warnings.push("assistant_action surface present but no request_change/draft_recovery capability — assistant cannot propose changes".to_string());
        }
    }

    // Dangerous permissions check
    if !perm.network.declarations.is_empty() {
        let has_wildcard = perm
            .network
            .declarations
            .iter()
            .any(|d| d.methods.is_empty());
        if has_wildcard {
            warnings.push("network declaration with empty methods list allows any HTTP method — consider restricting".to_string());
        }
    }
    if perm.capabilities.invoke.iter().any(|p| p == "*") {
        warnings.push(
            "capabilities.invoke: [\"*\"] grants access to all capabilities — consider narrowing"
                .to_string(),
        );
    }

    // Deterministic path check: network or secret refs make package non-deterministic
    let has_network = !perm.network.declarations.is_empty() || !perm.network.hosts.is_empty();
    if has_network {
        warnings.push("package requests network access — not deterministic by default; consider providing a no-network path".to_string());
    }

    // Replacement metadata hint
    if has_experience_entry && manifest.contributes.extension_points.is_empty() {
        // This is informational, not a warning
    }

    println!("package check: {}@{} ok", manifest.id, manifest.version);
    println!("  entry_kind:   {}", entry_kind);
    println!("  trust_level:  {}", trust_level);
    println!("  capabilities: {}", cap_count);
    println!("  surfaces:");
    if surfaces_by_slot.is_empty() {
        println!("    (none)");
    } else {
        for (slot, ids) in &surfaces_by_slot {
            println!("    {}: {}", slot, ids.join(", "));
        }
    }
    println!(
        "  permissions:  {}",
        serde_json::to_string(&permissions_summary)?
    );
    if !perm.network.declarations.is_empty() {
        println!("  network declarations:");
        for decl in &perm.network.declarations {
            let methods = if decl.methods.is_empty() {
                "*".to_string()
            } else {
                decl.methods.join(", ")
            };
            let purpose = decl.purpose.as_deref().unwrap_or("(none)");
            println!(
                "    {}: methods=[{}] purpose={}",
                decl.host, methods, purpose
            );
        }
    } else if !perm.network.hosts.is_empty() {
        println!("  network hosts: {}", perm.network.hosts.join(", "));
    }
    println!(
        "  sandbox:      {}",
        serde_json::to_string(&sandbox_summary)?
    );
    if !warnings.is_empty() {
        println!("  warnings:");
        for w in &warnings {
            println!("    - {}", w);
        }
    }

    Ok(())
}

pub(crate) async fn package_run_fixture(path: PathBuf) -> Result<()> {
    let manifest = read_manifest(path.clone()).await?;
    manifest.validate_basic()?;

    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());

    let load_result = runtime.load_package(manifest.clone()).await;
    let mut capability_results = Vec::new();
    let mut passed = 0u32;
    let mut failed = 0u32;

    match load_result {
        Ok(_record) => {
            for cap in &manifest.provides {
                if cap.streaming {
                    continue;
                }
                let input = json!({"fixture": true});
                let result = runtime
                    .invoke_capability(CapabilityInvocationRequest {
                        handle: None,
                        capability_id: Some(cap.id.clone()),
                        caller_package_id: None,
                        provider_package_id: None,
                        version: None,
                        session_id: None,
                        input: input.clone(),
                    })
                    .await;
                match result {
                    Ok(invocation_result) => {
                        passed += 1;
                        capability_results.push(json!({
                            "capability_id": cap.id,
                            "status": "ok",
                            "input": input,
                            "output": invocation_result.output,
                        }));
                    }
                    Err(err) => {
                        failed += 1;
                        capability_results.push(json!({
                            "capability_id": cap.id,
                            "status": "error",
                            "input": input,
                            "error": err.to_string(),
                        }));
                    }
                }
            }
        }
        Err(err) => {
            failed += 1;
            capability_results.push(json!({
                "capability_id": "load",
                "status": "error",
                "error": err.to_string(),
            }));
        }
    }

    let summary = json!({
        "package_id": manifest.id,
        "version": manifest.version,
        "capabilities_tested": capability_results,
        "total": passed + failed,
        "passed": passed,
        "failed": failed,
    });
    println!("{}", serde_json::to_string_pretty(&summary)?);

    // Creator-facing fixture diagnostics (Beta 5)
    if failed > 0 {
        println!("\n  creator diagnostics:");
        for result in &capability_results {
            if result["status"] == "error" {
                let cap_id = result["capability_id"].as_str().unwrap_or("unknown");
                let error_msg = result["error"].as_str().unwrap_or("unknown error");
                println!("    - {}: {}", cap_id, error_msg);
                // Provide common fix hints
                if error_msg.contains("not found") || error_msg.contains("no provider") {
                    println!("      hint: check that the capability id in the surface's capability_id field matches a provided capability");
                }
                if error_msg.contains("timeout") {
                    println!("      hint: the capability may be waiting for external I/O; consider a deterministic/no-network path for fixture testing");
                }
            }
        }
    }
    if passed == 0 && failed == 0 {
        println!("\n  creator diagnostics: no capabilities were tested (streaming capabilities are skipped in fixtures)");
        println!("    hint: add at least one non-streaming capability to run fixture checks");
    }

    Ok(())
}

pub(crate) async fn package_invoke_local(
    path: PathBuf,
    capability_id: String,
    input: String,
) -> Result<()> {
    let manifest = read_manifest(path).await?;
    let payload: serde_json::Value = serde_json::from_str(&input)?;
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());
    runtime.load_package(manifest).await?;
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(capability_id),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: payload,
        })
        .await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub(crate) async fn package_conformance(path: PathBuf) -> Result<()> {
    let manifest = read_manifest(path).await?;
    manifest.validate_basic()?;
    let capability = manifest
        .provides
        .first()
        .ok_or_else(|| {
            anyhow::anyhow!("package conformance requires at least one provided capability")
        })?
        .id
        .clone();
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());
    runtime.load_package(manifest).await?;
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(capability),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"package_conformance": true}),
        })
        .await?;
    anyhow::ensure!(
        result.output == json!({"package_conformance": true}),
        "package did not echo conformance payload"
    );
    println!("package conformance: ok");
    Ok(())
}

/// Local dev reload/restart smoke: loads a package into an in-memory runtime,
/// attempts restart (subprocess only), and prints before/after status and log count.
pub(crate) async fn package_reload(path: PathBuf) -> Result<()> {
    let manifest = read_manifest(path.clone()).await?;
    manifest.validate_basic()?;

    let entry_kind = match &manifest.entry.kind {
        PackageEntry::RustInproc { .. } => "rust_inproc",
        PackageEntry::Subprocess { .. } => "subprocess",
        PackageEntry::Wasm { .. } => "wasm",
        PackageEntry::Remote { .. } => "remote",
        PackageEntry::SurfaceBundle { .. } => "surface_bundle",
    };
    let can_restart = matches!(manifest.entry.kind, PackageEntry::Subprocess { .. });
    let package_id = manifest.id.clone();

    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store.clone(), RuntimeConfig::default());

    // Load
    let load_record = runtime.load_package(manifest.clone()).await?;
    let before = runtime.package_status(&package_id).await;
    println!(
        "package load: {}@{} ({:?})",
        load_record.id, load_record.version, load_record.state
    );

    // Logs before restart
    let logs_before = runtime.package_logs(&package_id).await;
    println!("logs before restart: {}", logs_before.len());

    if !can_restart {
        println!(
            "restart: skipped (entry kind '{}' does not support restart)",
            entry_kind
        );
        println!("  hint: only subprocess packages support restart; rust_inproc/wasm/remote require re-load");
        runtime.unload_package(&package_id).await?;
        return Ok(());
    }

    // Restart
    let restart_record = runtime.restart_package(&package_id).await?;
    let after = runtime.package_status(&package_id).await;

    // Logs after restart
    let logs_after = runtime.package_logs(&package_id).await;

    println!(
        "restart: {}@{} ({:?})",
        restart_record.id, restart_record.version, restart_record.state
    );
    println!("status before: {:?}", before.as_ref().map(|r| &r.state));
    println!("status after:  {:?}", after.as_ref().map(|r| &r.state));
    println!("logs after restart: {}", logs_after.len());

    // Creator-facing reload diagnostics (Beta 5)
    if after.is_none() {
        println!("  WARNING: package status unavailable after restart — package may be degraded");
    }
    if let Some(record) = after.as_ref() {
        if let plurora_runtime::PackageState::Degraded = &record.state {
            println!("  WARNING: package is in degraded state after restart — check subprocess logs for errors");
        }
    }

    // Unload
    runtime.unload_package(&package_id).await?;
    println!("unloaded: {}", package_id);

    Ok(())
}

/// Internal representation of the effective surface generation strategy.
/// `LegacyExperience` is the backward-compatible mode triggered when
/// `--language *-experience` is used without an explicit `--template`,
/// reproducing the original 4-surface experience generation.
enum EffectiveTemplate {
    Basic,
    Experience,
    PlayRenderer,
    ForgePanel,
    AssistantAction,
    AssetEditor,
    FullSurface,
    LegacyExperience,
    Networked,
    Streaming,
    AgentRuntime,
    ExperienceRuntime,
    PlayableBoard,
    PlayableExperience,
}

impl std::fmt::Debug for EffectiveTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectiveTemplate::Basic => write!(f, "Basic"),
            EffectiveTemplate::Experience => write!(f, "Experience"),
            EffectiveTemplate::PlayRenderer => write!(f, "PlayRenderer"),
            EffectiveTemplate::ForgePanel => write!(f, "ForgePanel"),
            EffectiveTemplate::AssistantAction => write!(f, "AssistantAction"),
            EffectiveTemplate::AssetEditor => write!(f, "AssetEditor"),
            EffectiveTemplate::FullSurface => write!(f, "FullSurface"),
            EffectiveTemplate::LegacyExperience => write!(f, "Experience (legacy)"),
            EffectiveTemplate::Networked => write!(f, "Networked"),
            EffectiveTemplate::Streaming => write!(f, "Streaming"),
            EffectiveTemplate::AgentRuntime => write!(f, "AgentRuntime"),
            EffectiveTemplate::ExperienceRuntime => write!(f, "ExperienceRuntime"),
            EffectiveTemplate::PlayableBoard => write!(f, "PlayableBoard"),
            EffectiveTemplate::PlayableExperience => write!(f, "PlayableExperience"),
        }
    }
}

/// Resolve the effective template: explicit --template wins, otherwise
/// fall back to auto-detection from the language string (legacy experience
/// if language contains "experience", otherwise basic).
fn resolve_template(template: &Option<PackageTemplate>, language: &str) -> EffectiveTemplate {
    match template {
        Some(PackageTemplate::Basic) => EffectiveTemplate::Basic,
        Some(PackageTemplate::Experience) => EffectiveTemplate::Experience,
        Some(PackageTemplate::PlayRenderer) => EffectiveTemplate::PlayRenderer,
        Some(PackageTemplate::ForgePanel) => EffectiveTemplate::ForgePanel,
        Some(PackageTemplate::AssistantAction) => EffectiveTemplate::AssistantAction,
        Some(PackageTemplate::AssetEditor) => EffectiveTemplate::AssetEditor,
        Some(PackageTemplate::FullSurface) => EffectiveTemplate::FullSurface,
        Some(PackageTemplate::Networked) => EffectiveTemplate::Networked,
        Some(PackageTemplate::Streaming) => EffectiveTemplate::Streaming,
        Some(PackageTemplate::AgentRuntime) => EffectiveTemplate::AgentRuntime,
        Some(PackageTemplate::ExperienceRuntime) => EffectiveTemplate::ExperienceRuntime,
        Some(PackageTemplate::PlayableBoard) => EffectiveTemplate::PlayableBoard,
        Some(PackageTemplate::PlayableExperience) => EffectiveTemplate::PlayableExperience,
        None => {
            if language.contains("experience") {
                EffectiveTemplate::LegacyExperience
            } else {
                EffectiveTemplate::Basic
            }
        }
    }
}

/// Build the YAML surfaces block for a given template and package id.
fn build_surfaces_yaml(template: &EffectiveTemplate, id: &str) -> String {
    match template {
        EffectiveTemplate::Basic => "  surfaces: []\n".to_string(),
        EffectiveTemplate::Experience => format!(
            r#"  surfaces:
    - id: {id}/entry
      version: 0.1.0
      slot: experience_entry
      title: Generated Experience
      description: Launchable package entry generated by plurora init-package.
      capability_id: {id}/echo
      activation:
        launch_capability_id: {id}/echo
        session_template:
          labels: [generated, experience]
      approval_policy: user_approval
"#
        ),
        EffectiveTemplate::PlayRenderer => format!(
            r#"  surfaces:
    - id: {id}/play
      version: 0.1.0
      slot: play_renderer
      title: Generated Play Renderer
      description: Play renderer surface generated by plurora init-package.
      capability_id: {id}/echo
"#
        ),
        EffectiveTemplate::ForgePanel => format!(
            r#"  surfaces:
    - id: {id}/forge
      version: 0.1.0
      slot: forge_panel
      title: Generated Forge Panel
      description: Forge panel surface generated by plurora init-package.
      capability_id: {id}/echo
"#
        ),
        EffectiveTemplate::AssistantAction => format!(
            r#"  surfaces:
    - id: {id}/assist
      version: 0.1.0
      slot: assistant_action
      title: Generated Assistant Action
      description: Assistant action surface generated by plurora init-package.
      capability_id: {id}/echo
      approval_policy: fork_then_approve
"#
        ),
        EffectiveTemplate::AssetEditor => format!(
            r#"  surfaces:
    - id: {id}/asset-edit
      version: 0.1.0
      slot: asset_editor
      title: Generated Asset Editor
      description: Asset editor surface generated by plurora init-package.
      capability_id: {id}/echo
"#
        ),
        EffectiveTemplate::FullSurface => format!(
            r#"  surfaces:
    - id: {id}/entry
      version: 0.1.0
      slot: experience_entry
      title: Generated Experience
      description: Launchable package entry generated by plurora init-package.
      capability_id: {id}/echo
      activation:
        launch_capability_id: {id}/echo
        session_template:
          labels: [generated, experience]
      approval_policy: user_approval
    - id: {id}/play
      version: 0.1.0
      slot: play_renderer
      title: Generated Play Renderer
      description: Play renderer surface generated by plurora init-package.
      capability_id: {id}/echo
    - id: {id}/forge
      version: 0.1.0
      slot: forge_panel
      title: Generated Forge Panel
      description: Forge panel surface generated by plurora init-package.
      capability_id: {id}/echo
    - id: {id}/assist
      version: 0.1.0
      slot: assistant_action
      title: Generated Assistant Action
      description: Assistant action surface generated by plurora init-package.
      capability_id: {id}/echo
      approval_policy: fork_then_approve
    - id: {id}/asset-edit
      version: 0.1.0
      slot: asset_editor
      title: Generated Asset Editor
      description: Asset editor surface generated by plurora init-package.
      capability_id: {id}/echo
"#
        ),
        EffectiveTemplate::LegacyExperience => format!(
            r#"  surfaces:
    - id: {id}/entry
      version: 0.1.0
      slot: experience_entry
      title: Generated Experience
      description: Launchable package entry generated by plurora init-package.
      capability_id: {id}/echo
      activation:
        launch_capability_id: {id}/echo
        session_template:
          labels: [generated, experience]
      approval_policy: user_approval
    - id: {id}/play
      version: 0.1.0
      slot: play_renderer
      title: Generated Play Renderer
      description: Play renderer surface generated by plurora init-package.
      capability_id: {id}/echo
    - id: {id}/forge
      version: 0.1.0
      slot: forge_panel
      title: Generated Forge Panel
      description: Forge panel surface generated by plurora init-package.
      capability_id: {id}/echo
    - id: {id}/assist
      version: 0.1.0
      slot: assistant_action
      title: Generated Assistant Action
      description: Assistant action surface generated by plurora init-package.
      capability_id: {id}/echo
      approval_policy: fork_then_approve
"#
        ),
        EffectiveTemplate::Networked => "  surfaces: []\n".to_string(),
        EffectiveTemplate::Streaming => "  surfaces: []\n".to_string(),
        EffectiveTemplate::AgentRuntime => format!(
            r#"  surfaces:
    - id: {id}/assist
      version: 0.1.0
      slot: assistant_action
      title: Agent Runtime Assistant
      description: Agent runtime assistant action surface for proposal-gated operations.
      capability_id: {id}/run
      approval_policy: fork_then_approve
    - id: {id}/forge
      version: 0.1.0
      slot: forge_panel
      title: Agent Runtime Forge Panel
      description: Agent runtime forge panel for trace and proposal observability.
      capability_id: {id}/explain-run
"#
        ),
        EffectiveTemplate::ExperienceRuntime => format!(
            r#"  surfaces:
    - id: {id}/entry
      version: 0.1.0
      slot: experience_entry
      title: Experience Runtime Entry
      description: Launchable experience entry surface for the experience runtime contract.
      capability_id: {id}/describe-contract
      activation:
        launch_capability_id: {id}/describe-contract
        session_template:
          labels: [generated, experience, runtime]
      approval_policy: user_approval
    - id: {id}/play
      version: 0.1.0
      slot: play_renderer
      title: Experience Runtime Play Renderer
      description: Play renderer surface for experience state visualization.
      capability_id: {id}/describe-contract
    - id: {id}/forge
      version: 0.1.0
      slot: forge_panel
      title: Experience Runtime Forge Panel
      description: Forge panel for inspecting and modifying experience state, checkpoints, and recovery plans.
      capability_id: {id}/describe-contract
    - id: {id}/assist
      version: 0.1.0
      slot: assistant_action
      title: Experience Runtime Assistant Action
      description: Assistant action for proposal-gated experience modifications via Agentic Forge.
      capability_id: {id}/draft-recovery
      approval_policy: fork_then_approve
"#
        ),
        EffectiveTemplate::PlayableBoard => format!(
            r#"  surfaces:
    - id: {id}/entry
      version: 0.1.0
      slot: experience_entry
      title: Playable Board Entry
      description: Launchable playable board entry surface — board/module/constraint/marker state.
      capability_id: {id}/launch
      activation:
        launch_capability_id: {id}/launch
        session_template:
          labels: [generated, playable, board]
      approval_policy: none
    - id: {id}/play-renderer
      version: 0.1.0
      slot: play_renderer
      title: Playable Board Renderer
      description: Protocol-visible render payload for the playable board.
      capability_id: {id}/render_payload
    - id: {id}/forge-panel
      version: 0.1.0
      slot: forge_panel
      title: Playable Board Inspector
      description: Inspect board state, modules, constraints, markers, and checkpoints.
      capability_id: {id}/project_state
    - id: {id}/assistant-action
      version: 0.1.0
      slot: assistant_action
      title: Request Board Change
      description: Draft a user-approved change proposal for the playable board.
      capability_id: {id}/request_change
      approval_policy: fork_then_approve
"#
        ),
        EffectiveTemplate::PlayableExperience => format!(
            r#"  surfaces:
    - id: {id}/entry
      version: 0.1.0
      slot: experience_entry
      title: Playable Experience Entry
      description: Launchable playable experience entry surface — full checkpoint/recovery lifecycle.
      capability_id: {id}/launch
      activation:
        launch_capability_id: {id}/launch
        session_template:
          labels: [generated, playable, experience]
      approval_policy: none
    - id: {id}/play-renderer
      version: 0.1.0
      slot: play_renderer
      title: Playable Experience Renderer
      description: Protocol-visible render payload for the playable experience.
      capability_id: {id}/render_payload
    - id: {id}/forge-panel
      version: 0.1.0
      slot: forge_panel
      title: Playable Experience Inspector
      description: Inspect experience state, checkpoints, and recovery plans.
      capability_id: {id}/project_state
    - id: {id}/assistant-action
      version: 0.1.0
      slot: assistant_action
      title: Request Experience Change
      description: Draft a user-approved change proposal for the playable experience.
      capability_id: {id}/request_change
      approval_policy: fork_then_approve
"#
        ),
    }
}

pub(crate) async fn init_package(
    path: PathBuf,
    id: String,
    entry: String,
    language: String,
    template: Option<PackageTemplate>,
) -> Result<()> {
    let effective_template = resolve_template(&template, &language);

    fs::create_dir_all(&path)?;
    let package_py = path.join("package.py").display().to_string();
    let package_mjs = path.join("package.mjs").display().to_string();
    let is_typescript = language.starts_with("typescript");
    let effective_entry = if is_typescript && entry == "rust_inproc" {
        "subprocess"
    } else {
        entry.as_str()
    };
    let subprocess_command = if is_typescript {
        format!("    - node\n    - {package_mjs}")
    } else {
        format!("    - python3\n    - {package_py}")
    };
    let surfaces = build_surfaces_yaml(&effective_template, &id);
    let manifest = match effective_entry {
        "wasm" => format!(
            r#"schema_version: 1
id: {id}
version: 0.1.0
entry:
  kind: wasm
  module: package.wasm
  abi_version: 1
  memory_limit_mb: 64
provides: []
consumes: []
contributes:
  schemas: []
  hooks: []
  extension_points: []
  surfaces: []
permissions: {{}}
sandbox_policy:
  cpu_quota_ms_per_invoke: 5000
  memory_mb: 64
  wall_clock_ms: 30000
"#
        ),
        "remote" => format!(
            r#"schema_version: 1
id: {id}
version: 0.1.0
entry:
  kind: remote
  endpoint: https://example.invalid/plurora/package
  auth:
    scheme: none
    config: null
provides: []
consumes: []
contributes:
  schemas: []
  hooks: []
  extension_points: []
  surfaces: []
permissions: {{}}
sandbox_policy:
  cpu_quota_ms_per_invoke: 5000
  memory_mb: 128
  wall_clock_ms: 30000
"#
        ),
        "subprocess" if matches!(effective_template, EffectiveTemplate::Networked) => format!(
            r#"schema_version: 1
id: {id}
version: 0.1.0
entry:
  kind: subprocess
  command:
{subprocess_command}
  transport: json_rpc_stdio
provides:
  - id: {id}/fetch
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects:
      - network
  - id: {id}/echo
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
consumes: []
contributes:
  schemas: []
  hooks: []
  extension_points: []
{surfaces}permissions:
  network:
    declarations:
      - host: api.example.com
        methods:
          - GET
          - POST
        purpose: model inference
    hosts: []
sandbox_policy:
  cpu_quota_ms_per_invoke: 5000
  memory_mb: 128
  wall_clock_ms: 30000
"#
        ),
        "subprocess" if matches!(effective_template, EffectiveTemplate::Streaming) => format!(
            r#"schema_version: 1
id: {id}
version: 0.1.0
entry:
  kind: subprocess
  command:
{subprocess_command}
  transport: json_rpc_stdio
provides:
  - id: {id}/stream-plan
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: true
  - id: {id}/echo
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
consumes: []
contributes:
  schemas: []
  hooks: []
  extension_points: []
{surfaces}permissions: {{}}
sandbox_policy:
  cpu_quota_ms_per_invoke: 5000
  memory_mb: 128
  wall_clock_ms: 30000
"#
        ),
        "subprocess" if matches!(effective_template, EffectiveTemplate::AgentRuntime) => format!(
            r#"schema_version: 1
id: {id}
version: 0.1.0
entry:
  kind: subprocess
  command:
{subprocess_command}
  transport: json_rpc_stdio
provides:
  - id: {id}/run
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: true
  - id: {id}/explain-run
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
  - id: {id}/draft-proposal
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
  - id: {id}/echo
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
consumes: []
contributes:
  schemas: []
  hooks: []
  extension_points: []
{surfaces}permissions: {{}}
sandbox_policy:
  cpu_quota_ms_per_invoke: 5000
  memory_mb: 128
  wall_clock_ms: 30000
"#
        ),
        "subprocess" if matches!(effective_template, EffectiveTemplate::ExperienceRuntime) => {
            format!(
                r#"schema_version: 1
id: {id}
version: 0.1.0
entry:
  kind: subprocess
  command:
{subprocess_command}
  transport: json_rpc_stdio
provides:
  - id: {id}/describe-contract
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
  - id: {id}/create-checkpoint
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
  - id: {id}/inspect-checkpoint
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
  - id: {id}/draft-recovery
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
  - id: {id}/bind-agent-run
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
  - id: {id}/echo
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
consumes: []
contributes:
  schemas: []
  hooks: []
  extension_points: []
{surfaces}permissions: {{}}
sandbox_policy:
  cpu_quota_ms_per_invoke: 5000
  memory_mb: 128
  wall_clock_ms: 30000
"#
            )
        }
        "subprocess" if matches!(effective_template, EffectiveTemplate::PlayableBoard) => format!(
            r#"schema_version: 1
id: {id}
version: 0.1.0
display_name: Generated Playable Board
description: "Deterministic/no-network playable board package skeleton. Board/module/constraint/marker state with play, Forge inspection, assistant proposals. No real model inference, no network, no kernel privilege."
entry:
  kind: subprocess
  command:
{subprocess_command}
  transport: json_rpc_stdio
provides:
  - id: {id}/launch
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects: []
  - id: {id}/project_state
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects: []
  - id: {id}/render_payload
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects: []
  - id: {id}/record_player_action
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects: []
  - id: {id}/request_change
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects: []
  - id: {id}/create_checkpoint
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects: []
  - id: {id}/echo
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
consumes: []
contributes:
  schemas: []
  hooks: []
  extension_points: []
{surfaces}permissions: {{}}
sandbox_policy:
  cpu_quota_ms_per_invoke: 5000
  memory_mb: 128
  wall_clock_ms: 30000
"#
        ),
        "subprocess" if matches!(effective_template, EffectiveTemplate::PlayableExperience) => {
            format!(
                r#"schema_version: 1
id: {id}
version: 0.1.0
display_name: Generated Playable Experience
description: "Deterministic/no-network playable experience package skeleton. Full checkpoint/recovery lifecycle with play, Forge inspection, assistant proposals. No real model inference, no network, no kernel privilege."
entry:
  kind: subprocess
  command:
{subprocess_command}
  transport: json_rpc_stdio
provides:
  - id: {id}/launch
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects: []
  - id: {id}/project_state
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects: []
  - id: {id}/render_payload
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects: []
  - id: {id}/record_player_action
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects: []
  - id: {id}/request_change
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects: []
  - id: {id}/create_checkpoint
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects: []
  - id: {id}/inspect_checkpoint
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects: []
  - id: {id}/draft_recovery
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
    side_effects: []
  - id: {id}/echo
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
consumes: []
contributes:
  schemas: []
  hooks: []
  extension_points: []
{surfaces}permissions: {{}}
sandbox_policy:
  cpu_quota_ms_per_invoke: 5000
  memory_mb: 128
  wall_clock_ms: 30000
"#
            )
        }
        "subprocess" => format!(
            r#"schema_version: 1
id: {id}
version: 0.1.0
entry:
  kind: subprocess
  command:
{subprocess_command}
  transport: json_rpc_stdio
provides:
  - id: {id}/echo
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
    streaming: false
consumes: []
contributes:
  schemas: []
  hooks: []
  extension_points: []
{surfaces}permissions: {{}}
sandbox_policy:
  cpu_quota_ms_per_invoke: 5000
  memory_mb: 128
  wall_clock_ms: 30000
"#
        ),
        _ => format!(
            r#"schema_version: 1
id: {id}
version: 0.1.0
entry:
  kind: rust_inproc
  crate_ref: package-crate
  symbol: register
  abi_version: 1
provides: []
consumes: []
contributes:
  schemas: []
  hooks: []
  extension_points: []
  surfaces: []
permissions: {{}}
sandbox_policy:
  cpu_quota_ms_per_invoke: 5000
  memory_mb: 128
  wall_clock_ms: 30000
"#
        ),
    };
    fs::write(path.join("manifest.yaml"), manifest)?;
    if effective_entry == "subprocess" && language.starts_with("python") {
        fs::write(
            path.join("package.py"),
            templates::PYTHON_SUBPROCESS_TEMPLATE,
        )?;
    } else if effective_entry == "subprocess" && is_typescript {
        if matches!(effective_template, EffectiveTemplate::Networked) {
            fs::write(
                path.join("package.ts"),
                templates::typescript_networked_template(&id),
            )?;
        } else if matches!(effective_template, EffectiveTemplate::Streaming) {
            fs::write(
                path.join("package.ts"),
                templates::typescript_streaming_template(&id),
            )?;
        } else if matches!(effective_template, EffectiveTemplate::AgentRuntime) {
            fs::write(
                path.join("package.ts"),
                templates::typescript_agent_runtime_template(&id),
            )?;
        } else if matches!(effective_template, EffectiveTemplate::ExperienceRuntime) {
            fs::write(
                path.join("package.ts"),
                templates::typescript_experience_runtime_template(&id),
            )?;
        } else if matches!(effective_template, EffectiveTemplate::PlayableBoard) {
            fs::write(
                path.join("package.ts"),
                templates::typescript_playable_board_template(&id),
            )?;
        } else if matches!(effective_template, EffectiveTemplate::PlayableExperience) {
            fs::write(
                path.join("package.ts"),
                templates::typescript_playable_experience_template(&id),
            )?;
        } else {
            fs::write(
                path.join("package.ts"),
                templates::typescript_subprocess_template(&id),
            )?;
        }
        fs::write(
            path.join("package.mjs"),
            templates::TYPESCRIPT_SUBPROCESS_RUNTIME_TEMPLATE,
        )?;
        fs::write(path.join("tsconfig.json"), templates::TYPESCRIPT_TSCONFIG)?;
        fs::write(
            path.join("package.json"),
            templates::typescript_package_json(&id),
        )?;
    }
    fs::write(
        path.join("README.md"),
        format!("# {id}\n\nPlurora capability package skeleton (template: {:?}).\n\nRun `plurora package conformance manifest.yaml` from this directory.\n", effective_template),
    )?;
    println!(
        "initialized package skeleton at {} (template: {:?})",
        path.display(),
        effective_template
    );
    Ok(())
}
