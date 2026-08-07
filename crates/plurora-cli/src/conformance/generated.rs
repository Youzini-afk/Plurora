use std::fs;

use crate::cli::PackageTemplate;
use crate::commands::{composition, manifest, package};

pub(crate) async fn generated_subprocess_package() -> anyhow::Result<()> {
    let path =
        std::env::temp_dir().join(format!("plurora-generated-package-{}", std::process::id()));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }
    package::init_package(
        path.clone(),
        "example/generated-subprocess".to_string(),
        "subprocess".to_string(),
        "python".to_string(),
        None,
    )
    .await?;
    package::package_check(path.join("manifest.yaml")).await?;
    package::package_conformance(path.join("manifest.yaml")).await?;
    fs::remove_dir_all(path)?;
    Ok(())
}

pub(crate) async fn generated_typescript_subprocess_package() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "plurora-generated-ts-package-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }
    package::init_package(
        path.clone(),
        "example/generated-typescript-subprocess".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        None,
    )
    .await?;
    package::package_check(path.join("manifest.yaml")).await?;
    package::package_conformance(path.join("manifest.yaml")).await?;
    fs::remove_dir_all(path)?;
    Ok(())
}

pub(crate) async fn generated_experience_template() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "plurora-generated-experience-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }
    package::init_package(
        path.clone(),
        "example/generated-experience".to_string(),
        "subprocess".to_string(),
        "typescript-experience".to_string(),
        None,
    )
    .await?;
    package::package_check(path.join("manifest.yaml")).await?;
    package::package_conformance(path.join("manifest.yaml")).await?;
    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;
    // Legacy experience template (auto-detected from --language typescript-experience)
    // preserves the original 4 surfaces for backward compatibility.
    anyhow::ensure!(
        manifest.contributes.surfaces.len() >= 4,
        "legacy experience template should have >= 4 surfaces, got {}",
        manifest.contributes.surfaces.len()
    );
    fs::remove_dir_all(path)?;
    Ok(())
}

/// Test that the basic template generates no surfaces.
pub(crate) async fn generated_basic_template() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!("plurora-generated-basic-{}", std::process::id()));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }
    package::init_package(
        path.clone(),
        "example/generated-basic".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(PackageTemplate::Basic),
    )
    .await?;
    package::package_check(path.join("manifest.yaml")).await?;
    package::package_conformance(path.join("manifest.yaml")).await?;
    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;
    anyhow::ensure!(
        manifest.contributes.surfaces.is_empty(),
        "basic template should have 0 surfaces, got {}",
        manifest.contributes.surfaces.len()
    );
    fs::remove_dir_all(path)?;
    Ok(())
}

/// Test that the explicit --template experience generates only experience_entry.
pub(crate) async fn generated_explicit_experience_template() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "plurora-generated-explicit-experience-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }
    package::init_package(
        path.clone(),
        "example/generated-explicit-experience".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(PackageTemplate::Experience),
    )
    .await?;
    package::package_check(path.join("manifest.yaml")).await?;
    package::package_conformance(path.join("manifest.yaml")).await?;
    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;
    anyhow::ensure!(
        manifest.contributes.surfaces.len() == 1,
        "explicit experience template should have 1 surface, got {}",
        manifest.contributes.surfaces.len()
    );
    anyhow::ensure!(
        matches!(
            manifest.contributes.surfaces[0].slot,
            plurora_core::SurfaceSlot::ExperienceEntry
        ),
        "explicit experience template surface slot should be experience_entry"
    );
    fs::remove_dir_all(path)?;
    Ok(())
}

/// Test that the assistant-action template generates one surface with fork_then_approve.
pub(crate) async fn generated_assistant_action_template() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "plurora-generated-assistant-action-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }
    package::init_package(
        path.clone(),
        "example/generated-assistant-action".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(PackageTemplate::AssistantAction),
    )
    .await?;
    package::package_check(path.join("manifest.yaml")).await?;
    package::package_conformance(path.join("manifest.yaml")).await?;
    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;
    anyhow::ensure!(
        manifest.contributes.surfaces.len() == 1,
        "assistant-action template should have 1 surface, got {}",
        manifest.contributes.surfaces.len()
    );
    anyhow::ensure!(
        matches!(
            manifest.contributes.surfaces[0].slot,
            plurora_core::SurfaceSlot::AssistantAction
        ),
        "assistant-action template surface slot should be assistant_action"
    );
    anyhow::ensure!(
        manifest.contributes.surfaces[0].approval_policy
            == Some(plurora_core::SurfaceApprovalPolicy::ForkThenApprove),
        "assistant-action template should have fork_then_approve policy"
    );
    fs::remove_dir_all(path)?;
    Ok(())
}

/// Test that the asset-editor template generates one surface with the asset_editor slot.
pub(crate) async fn generated_asset_editor_template() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "plurora-generated-asset-editor-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }
    package::init_package(
        path.clone(),
        "example/generated-asset-editor".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(PackageTemplate::AssetEditor),
    )
    .await?;
    package::package_check(path.join("manifest.yaml")).await?;
    package::package_conformance(path.join("manifest.yaml")).await?;
    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;
    anyhow::ensure!(
        manifest.contributes.surfaces.len() == 1,
        "asset-editor template should have 1 surface, got {}",
        manifest.contributes.surfaces.len()
    );
    anyhow::ensure!(
        matches!(
            manifest.contributes.surfaces[0].slot,
            plurora_core::SurfaceSlot::AssetEditor
        ),
        "asset-editor template surface slot should be asset_editor"
    );
    fs::remove_dir_all(path)?;
    Ok(())
}

/// Test that the full-surface template generates all 5 surfaces.
pub(crate) async fn generated_full_surface_template() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "plurora-generated-full-surface-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }
    package::init_package(
        path.clone(),
        "example/generated-full-surface".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(PackageTemplate::FullSurface),
    )
    .await?;
    package::package_check(path.join("manifest.yaml")).await?;
    package::package_conformance(path.join("manifest.yaml")).await?;
    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;
    anyhow::ensure!(
        manifest.contributes.surfaces.len() == 5,
        "full-surface template should have 5 surfaces, got {}",
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
        "full-surface should include experience_entry"
    );
    anyhow::ensure!(
        slots.contains(&"play_renderer"),
        "full-surface should include play_renderer"
    );
    anyhow::ensure!(
        slots.contains(&"forge_panel"),
        "full-surface should include forge_panel"
    );
    anyhow::ensure!(
        slots.contains(&"assistant_action"),
        "full-surface should include assistant_action"
    );
    anyhow::ensure!(
        slots.contains(&"asset_editor"),
        "full-surface should include asset_editor"
    );
    // Verify assistant_action has fork_then_approve
    let assist = manifest
        .contributes
        .surfaces
        .iter()
        .find(|s| matches!(s.slot, plurora_core::SurfaceSlot::AssistantAction))
        .unwrap();
    anyhow::ensure!(
        assist.approval_policy == Some(plurora_core::SurfaceApprovalPolicy::ForkThenApprove),
        "full-surface assistant_action should have fork_then_approve"
    );
    fs::remove_dir_all(path)?;
    Ok(())
}

/// Test that the networked template generates network declarations and passes check/conformance.
pub(crate) async fn generated_networked_template() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "plurora-generated-networked-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }
    package::init_package(
        path.clone(),
        "example/generated-networked".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(PackageTemplate::Networked),
    )
    .await?;
    package::package_check(path.join("manifest.yaml")).await?;
    package::package_conformance(path.join("manifest.yaml")).await?;
    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;
    // Networked template should have 2 capabilities: fetch and echo
    anyhow::ensure!(
        manifest.provides.len() == 2,
        "networked template should have 2 capabilities, got {}",
        manifest.provides.len()
    );
    // Verify at least one capability has network side effect
    let has_network_side_effect = manifest
        .provides
        .iter()
        .any(|c| c.side_effects.contains(&"network".to_string()));
    anyhow::ensure!(
        has_network_side_effect,
        "networked template should have a capability with network side_effect"
    );
    // Verify network declarations exist
    anyhow::ensure!(
        !manifest.permissions.network.declarations.is_empty(),
        "networked template should have network declarations"
    );
    // Verify declared network metadata is present and structured
    let decl = &manifest.permissions.network.declarations[0];
    anyhow::ensure!(
        !decl.host.is_empty(),
        "network declaration should have a host"
    );
    anyhow::ensure!(
        !decl.methods.is_empty(),
        "network declaration should specify methods"
    );
    anyhow::ensure!(
        decl.purpose.is_some(),
        "network declaration should have a purpose"
    );
    // Verify no surfaces (networked template is capability-focused)
    anyhow::ensure!(
        manifest.contributes.surfaces.is_empty(),
        "networked template should have 0 surfaces, got {}",
        manifest.contributes.surfaces.len()
    );
    fs::remove_dir_all(path)?;
    Ok(())
}

/// Test that the streaming template generates a streaming capability and passes check/conformance.
pub(crate) async fn generated_streaming_template() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "plurora-generated-streaming-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }
    package::init_package(
        path.clone(),
        "example/generated-streaming".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(PackageTemplate::Streaming),
    )
    .await?;
    package::package_check(path.join("manifest.yaml")).await?;
    package::package_conformance(path.join("manifest.yaml")).await?;
    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;
    // Streaming template should have 2 capabilities: stream-plan and echo
    anyhow::ensure!(
        manifest.provides.len() == 2,
        "streaming template should have 2 capabilities, got {}",
        manifest.provides.len()
    );
    // Verify at least one capability has streaming=true
    let has_streaming = manifest.provides.iter().any(|c| c.streaming);
    anyhow::ensure!(
        has_streaming,
        "streaming template should have a streaming capability"
    );
    // Verify no surfaces (streaming template is capability-focused)
    anyhow::ensure!(
        manifest.contributes.surfaces.is_empty(),
        "streaming template should have 0 surfaces, got {}",
        manifest.contributes.surfaces.len()
    );
    fs::remove_dir_all(path)?;
    Ok(())
}

/// Test that the agent-runtime template generates a 4-capability agent package
/// with streaming run, proposal, trace, echo capabilities and passes check/conformance.
/// Verifies: 4 capabilities, run is streaming, assistant_action + forge_panel surfaces,
/// permissions.network.declarations empty, no raw secrets, no platform.agent/model/prompt/memory/turn text.
pub(crate) async fn generated_agent_runtime_template() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "plurora-generated-agent-runtime-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }
    package::init_package(
        path.clone(),
        "example/generated-agent-runtime".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(PackageTemplate::AgentRuntime),
    )
    .await?;
    package::package_check(path.join("manifest.yaml")).await?;
    package::package_conformance(path.join("manifest.yaml")).await?;
    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;

    // 4 capabilities: run (streaming), explain-run, draft-proposal, echo
    anyhow::ensure!(
        manifest.provides.len() == 4,
        "agent-runtime template should have 4 capabilities, got {}",
        manifest.provides.len()
    );

    // run capability must be streaming
    let run_cap = manifest
        .provides
        .iter()
        .find(|c| c.id == "example/generated-agent-runtime/run");
    anyhow::ensure!(
        run_cap.is_some(),
        "agent-runtime should have run capability"
    );
    anyhow::ensure!(
        run_cap.unwrap().streaming,
        "run capability should be streaming"
    );

    // 2 surfaces: assistant_action + forge_panel (no experience_entry)
    anyhow::ensure!(
        manifest.contributes.surfaces.len() == 2,
        "agent-runtime template should have 2 surfaces, got {}",
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
        slots.contains(&"assistant_action"),
        "agent-runtime should have assistant_action surface"
    );
    anyhow::ensure!(
        slots.contains(&"forge_panel"),
        "agent-runtime should have forge_panel surface"
    );
    anyhow::ensure!(
        !slots.contains(&"experience_entry"),
        "agent-runtime should NOT have experience_entry surface"
    );

    // No network declarations (no-network)
    anyhow::ensure!(
        manifest.permissions.network.declarations.is_empty(),
        "agent-runtime should have no network declarations"
    );

    // No raw secrets in manifest
    let manifest_json = serde_json::to_value(&manifest)?;
    let manifest_str = serde_json::to_string(&manifest_json)?;
    anyhow::ensure!(
        !plurora_core::looks_like_raw_secret(&manifest_str),
        "agent-runtime manifest must not contain raw secrets"
    );

    // No platform.agent/model/prompt/memory/turn text in manifest or package.ts
    let forbidden = [
        "platform.agent",
        "platform.model",
        "platform.prompt",
        "platform.memory",
        "platform.turn",
    ];
    for token in &forbidden {
        anyhow::ensure!(
            !manifest_str.contains(token),
            "agent-runtime manifest must not contain '{}' text",
            token
        );
    }
    let package_ts = fs::read_to_string(path.join("package.ts"))?;
    for token in &forbidden {
        anyhow::ensure!(
            !package_ts.contains(token),
            "agent-runtime package.ts must not contain '{}' text",
            token
        );
    }

    fs::remove_dir_all(path)?;
    Ok(())
}

/// Test that the experience-runtime template generates a 6-capability package
/// with contract/checkpoint/recovery capabilities and 4 experience surfaces,
/// and passes check/conformance. Verifies: 4 surfaces (experience_entry,
/// play_renderer, forge_panel, assistant_action), 6 capabilities, no network
/// declarations, no platform.experience/world/turn/chat/memory text.
pub(crate) async fn generated_experience_runtime_template() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "plurora-generated-experience-runtime-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }
    package::init_package(
        path.clone(),
        "example/generated-experience-runtime".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(PackageTemplate::ExperienceRuntime),
    )
    .await?;
    package::package_check(path.join("manifest.yaml")).await?;
    package::package_conformance(path.join("manifest.yaml")).await?;
    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;

    // 4 surfaces: experience_entry, play_renderer, forge_panel, assistant_action
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

    // 6 capabilities: describe-contract, create-checkpoint, inspect-checkpoint, draft-recovery, bind-agent-run, echo
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

    // No kernel experience namespace
    let manifest_json = serde_json::to_value(&manifest)?;
    let manifest_str = serde_json::to_string(&manifest_json)?;
    let forbidden = [
        "platform.experience",
        "platform.world",
        "platform.turn",
        "platform.chat",
        "platform.memory",
    ];
    for token in &forbidden {
        anyhow::ensure!(
            !manifest_str.contains(token),
            "experience-runtime manifest must not contain '{}' text",
            token
        );
    }
    let package_ts = fs::read_to_string(path.join("package.ts"))?;
    for token in &forbidden {
        anyhow::ensure!(
            !package_ts.contains(token),
            "experience-runtime package.ts must not contain '{}' text",
            token
        );
    }

    fs::remove_dir_all(path)?;
    Ok(())
}

/// Test that the faux-model-readiness example package passes check/conformance.
/// Proves the no-network readiness substrate shape without real model inference.
pub(crate) async fn faux_model_readiness_package() -> anyhow::Result<()> {
    let manifest_path =
        std::path::PathBuf::from("examples/packages/faux-model-readiness/manifest.yaml");
    anyhow::ensure!(
        manifest_path.exists(),
        "faux-model-readiness manifest not found"
    );
    package::package_check(manifest_path.clone()).await?;
    let manifest = manifest::read_manifest(manifest_path.clone()).await?;
    // Verify it has network declarations
    anyhow::ensure!(
        !manifest.permissions.network.declarations.is_empty(),
        "faux-model-readiness should declare network permissions"
    );
    // Verify at least one capability has network side effect
    let has_network_side_effect = manifest
        .provides
        .iter()
        .any(|c| c.side_effects.contains(&"network".to_string()));
    anyhow::ensure!(
        has_network_side_effect,
        "faux-model-readiness should have a capability with network side_effect"
    );
    // Verify at least one streaming capability
    let has_streaming = manifest.provides.iter().any(|c| c.streaming);
    anyhow::ensure!(
        has_streaming,
        "faux-model-readiness should have a streaming capability"
    );
    // Verify no raw secrets in manifest metadata
    let manifest_json = serde_json::to_value(&manifest)?;
    let manifest_str = serde_json::to_string(&manifest_json)?;
    anyhow::ensure!(
        !plurora_core::looks_like_raw_secret(&manifest_str),
        "faux-model-readiness manifest must not contain raw secrets"
    );
    // Verify declared network metadata has proper structure
    let decl = &manifest.permissions.network.declarations[0];
    anyhow::ensure!(
        !decl.host.is_empty(),
        "network declaration should have a host"
    );
    anyhow::ensure!(
        !decl.methods.is_empty(),
        "network declaration should specify methods"
    );
    anyhow::ensure!(
        decl.purpose.is_some(),
        "network declaration should have a purpose"
    );
    Ok(())
}

/// Test that the faux-agent-readiness example package passes check/conformance.
/// Proves the proposal/trace substrate shape without real agent loop or model inference.
pub(crate) async fn faux_agent_readiness_package() -> anyhow::Result<()> {
    let manifest_path =
        std::path::PathBuf::from("examples/packages/faux-agent-readiness/manifest.yaml");
    anyhow::ensure!(
        manifest_path.exists(),
        "faux-agent-readiness manifest not found"
    );
    package::package_check(manifest_path.clone()).await?;
    let manifest = manifest::read_manifest(manifest_path.clone()).await?;
    // Verify it has proposal/trace capabilities
    anyhow::ensure!(
        manifest.provides.len() >= 2,
        "faux-agent-readiness should have at least 2 capabilities, got {}",
        manifest.provides.len()
    );
    // Verify at least one streaming capability
    let has_streaming = manifest.provides.iter().any(|c| c.streaming);
    anyhow::ensure!(
        has_streaming,
        "faux-agent-readiness should have a streaming capability"
    );
    // Verify no network permissions (agent readiness does not need network)
    anyhow::ensure!(
        manifest.permissions.network.declarations.is_empty()
            && manifest.permissions.network.hosts.is_empty(),
        "faux-agent-readiness should not declare network permissions (no-network proof)"
    );
    // Verify no raw secrets in manifest
    let manifest_json = serde_json::to_value(&manifest)?;
    let manifest_str = serde_json::to_string(&manifest_json)?;
    anyhow::ensure!(
        !plurora_core::looks_like_raw_secret(&manifest_str),
        "faux-agent-readiness manifest must not contain raw secrets"
    );
    Ok(())
}

pub(crate) async fn composition_descriptor() -> anyhow::Result<()> {
    let root = std::env::temp_dir().join(format!("plurora-composition-{}", std::process::id()));
    let package_path = root.join("package");
    let composition_path = root.join("composition");
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    package::init_package(
        package_path,
        "example/composed-experience".to_string(),
        "subprocess".to_string(),
        "typescript-experience".to_string(),
        None,
    )
    .await?;
    composition::init_composition(
        composition_path.clone(),
        "example/composed-experience".to_string(),
    )
    .await?;
    composition::composition_check(composition_path.join("composition.yaml")).await?;
    fs::remove_dir_all(root)?;
    Ok(())
}

/// Test composition descriptor v2 fields: required capabilities pass,
/// optional missing only warning, required missing fails.
pub(crate) async fn composition_descriptor_v2() -> anyhow::Result<()> {
    let root = std::env::temp_dir().join(format!("plurora-composition-v2-{}", std::process::id()));
    let package_path = root.join("package");
    let composition_path = root.join("composition");
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;

    // Create a package with experience surfaces
    package::init_package(
        package_path.clone(),
        "example/composed-v2".to_string(),
        "subprocess".to_string(),
        "typescript-experience".to_string(),
        None,
    )
    .await?;

    // Create v2 composition with all new fields
    fs::create_dir_all(&composition_path)?;
    let manifest_yaml = package_path.join("manifest.yaml");
    fs::write(
        composition_path.join("composition.yaml"),
        format!(
            r#"id: example/composed-v2
version: 0.1.0
entry_surface_id: example/composed-v2/entry
title: "V2 Test Composition"
description: "A composition descriptor with v2 fields"
packages:
  - {}
required_surfaces:
  - experience_entry
optional_packages:
  - /nonexistent/optional-package/manifest.yaml
required_capabilities:
  - example/composed-v2/echo
permission_expectations:
  - capabilities.invoke
replacement_candidates:
  - example/experience-alt
compatibility_notes:
  - "Requires kernel v0.1.0 or later"
"#,
            manifest_yaml.display()
        ),
    )?;

    // This should succeed: required capabilities are provided, optional missing only warns
    composition::composition_check(composition_path.join("composition.yaml")).await?;

    // Now test that missing required capability fails
    let fail_path = composition_path.join("composition-fail.yaml");
    fs::write(
        &fail_path,
        format!(
            r#"id: example/composed-v2-fail
version: 0.1.0
entry_surface_id: example/composed-v2/entry
packages:
  - {}
required_surfaces:
  - experience_entry
required_capabilities:
  - nonexistent/missing-capability
"#,
            manifest_yaml.display()
        ),
    )?;
    let result = composition::composition_check(fail_path).await;
    anyhow::ensure!(
        result.is_err(),
        "composition check should fail when required capability is missing"
    );

    fs::remove_dir_all(root)?;
    Ok(())
}

pub(crate) async fn component_identity_independent_of_package_envelope() -> anyhow::Result<()> {
    let declaration = phase7_component_declaration();
    let first = plurora_core::package_envelope_for_manifest(&phase7_manifest(
        "vendor/one",
        plurora_core::ContractMode::V1,
        Some(declaration.clone()),
    ))?;
    let second = plurora_core::package_envelope_for_manifest(&phase7_manifest(
        "vendor/two",
        plurora_core::ContractMode::V1,
        Some(declaration),
    ))?;
    anyhow::ensure!(
        first.artifact.digest != second.artifact.digest,
        "different packages must have different envelope digests"
    );
    anyhow::ensure!(
        first.components[0].component_id == second.components[0].component_id,
        "component identity changed across package envelopes"
    );
    anyhow::ensure!(
        first.components[0].behavior.digest == second.components[0].behavior.digest,
        "behavior claim changed across package envelopes"
    );
    Ok(())
}

pub(crate) async fn component_replacement_preserves_content_roots() -> anyhow::Result<()> {
    let root = plurora_core::ArtifactDescriptor {
        artifact_type_uri: "urn:plurora:world-content:v1".to_string(),
        media_type: "application/octet-stream".to_string(),
        digest: format!("sha256:{}", "a".repeat(64)),
        size_bytes: 1,
        references: Vec::new(),
        annotations: Default::default(),
    };
    let mut lock = plurora_core::CompositionLock::new(
        vec![plurora_core::ComponentLockPin {
            component_id: "org.example/component".to_string(),
            digest: format!("sha256:{}", "b".repeat(64)),
            behavior_digest: format!("sha256:{}", "c".repeat(64)),
            trust_class: plurora_core::ComponentTrustClass::IsolatedProcess,
        }],
        Vec::new(),
        vec![root.clone()],
    )?;
    lock.replace_component(
        "org.example/component",
        plurora_core::ComponentLockPin {
            component_id: "org.example/replacement".to_string(),
            digest: format!("sha256:{}", "d".repeat(64)),
            behavior_digest: format!("sha256:{}", "e".repeat(64)),
            trust_class: plurora_core::ComponentTrustClass::SandboxedComponent,
        },
    )?;
    anyhow::ensure!(
        lock.content_roots == vec![root],
        "component replacement mutated content roots"
    );
    Ok(())
}

pub(crate) async fn contract_none_is_foreign_capsule() -> anyhow::Result<()> {
    let manifest = phase7_manifest("vendor/foreign", plurora_core::ContractMode::None, None);
    let runtime = plurora_runtime::Runtime::new(
        std::sync::Arc::new(plurora_runtime::InMemoryEventStore::default()),
        plurora_runtime::RuntimeConfig::default(),
    );
    let record = runtime.load_package(manifest.clone()).await?;
    anyhow::ensure!(
        record.state == plurora_runtime::PackageState::Ready,
        "Foreign Capsule did not reach ready state"
    );
    anyhow::ensure!(
        record.trust_class == plurora_core::ComponentTrustClass::ForeignCapsule,
        "contract:none did not map to the Foreign Capsule trust class"
    );
    let check = plurora_core::conformance::check_component_identity_and_trust(&manifest);
    anyhow::ensure!(
        check.status == plurora_core::CheckStatus::Warning,
        "Foreign Capsule conformance must be an explicit warning"
    );
    let details = check.details.unwrap_or_default();
    anyhow::ensure!(
        details.contains("composability and portability are not guaranteed"),
        "Foreign Capsule report omitted its composability/portability limitation"
    );
    runtime.unload_package(&record.id).await?;
    Ok(())
}

fn phase7_component_declaration() -> plurora_core::ComponentDeclaration {
    plurora_core::ComponentDeclaration {
        id: "org.example/reference-component".to_string(),
        version: "1.0.0".to_string(),
        capability_ids: Vec::new(),
        protocol_implementations: vec![plurora_core::ProtocolImplementationDeclaration {
            protocol_id: "plurora.change".to_string(),
            version: "1.0.0".to_string(),
            profiles: vec!["plurora.change/default/v1".to_string()],
            conformance_vectors: vec!["proposal.lifecycle_apply".to_string()],
        }],
        content_roots: Vec::new(),
        surface_ids: Vec::new(),
        annotations: Default::default(),
    }
}

fn phase7_manifest(
    package_id: &str,
    contract: plurora_core::ContractMode,
    component: Option<plurora_core::ComponentDeclaration>,
) -> plurora_core::PackageManifest {
    plurora_core::PackageManifest {
        schema_version: 1,
        id: package_id.to_string(),
        version: "1.0.0".to_string(),
        display_name: None,
        description: None,
        author: None,
        license: None,
        entry: plurora_core::EntryDescriptor {
            kind: plurora_core::PackageEntry::RustInproc {
                crate_ref: "phase7_reference".to_string(),
                symbol: "register".to_string(),
                abi_version: 1,
            },
            contract,
            component,
        },
        provides: Vec::new(),
        consumes: Vec::new(),
        requires: Vec::new(),
        contributes: plurora_core::PackageContributions::default(),
        permissions: plurora_core::PermissionSet::default(),
        sandbox_policy: plurora_core::SandboxPolicy::default(),
    }
}
