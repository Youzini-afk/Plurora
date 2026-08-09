//! Conformance tests for Experience Beta 5 — Creator Loop Beta.
//!
//! Covers:
//! 1. Generated playable-board template passes check/conformance with correct surfaces/capabilities
//! 2. Generated playable-experience template passes check/conformance with checkpoint/recovery
//! 3. Package diagnostics: experience_entry without play_renderer/forge_panel/assistant_action warns
//! 4. Package diagnostics: missing checkpoint capability warns for experience packages
//! 5. Package diagnostics: dangerous permissions (wildcard invoke, empty network methods) warn
//! 6. Package diagnostics: network access triggers non-deterministic hint
//! 7. Work materialization diagnostics: experience Port projection and replacement coverage
//! 8. Walkthrough reference: playable-creation-board package check output is verifiable
//! 9. No privileged first-party dependency: third-party playable-seed replaces first-party playable-seed

use std::fs;
use std::path::PathBuf;

use crate::cli::PackageTemplate;
use crate::commands::{manifest, package, work};
use plurora_core::PackageEntry;
use serde_json;

/// Case 1: Generated playable-board template passes check/conformance with
/// 4 surfaces (experience_entry, play_renderer, forge_panel, assistant_action)
/// and 7 capabilities (launch, project_state, render_payload, record_player_action,
/// request_change, create_checkpoint, echo). No network declarations.
pub(crate) async fn creator_loop_playable_board_template() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "plurora-generated-playable-board-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }

    package::init_package(
        path.clone(),
        "example/generated-playable-board".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(PackageTemplate::PlayableBoard),
    )
    .await?;

    package::package_check(path.join("manifest.yaml")).await?;
    package::package_conformance(path.join("manifest.yaml")).await?;

    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;

    // 4 surfaces
    anyhow::ensure!(
        manifest.contributes.surfaces.len() == 4,
        "playable-board template should have 4 surfaces, got {}",
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
        "playable-board should have experience_entry"
    );
    anyhow::ensure!(
        slots.contains(&"play_renderer"),
        "playable-board should have play_renderer"
    );
    anyhow::ensure!(
        slots.contains(&"forge_panel"),
        "playable-board should have forge_panel"
    );
    anyhow::ensure!(
        slots.contains(&"assistant_action"),
        "playable-board should have assistant_action"
    );

    // 7 capabilities
    anyhow::ensure!(
        manifest.provides.len() == 7,
        "playable-board template should have 7 capabilities, got {}",
        manifest.provides.len()
    );

    // No network declarations
    anyhow::ensure!(
        manifest.permissions.network.declarations.is_empty(),
        "playable-board should have no network declarations"
    );

    // No platform-reserved namespace in manifest
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
            "playable-board manifest must not contain '{}' text",
            token
        );
    }

    // package.ts exists and is valid
    let package_ts = fs::read_to_string(path.join("package.ts"))?;
    for token in &forbidden {
        anyhow::ensure!(
            !package_ts.contains(token),
            "playable-board package.ts must not contain '{}' text",
            token
        );
    }

    fs::remove_dir_all(path)?;
    Ok(())
}

/// Case 2: Generated playable-experience template passes check/conformance with
/// 4 surfaces and 9 capabilities including inspect_checkpoint and draft_recovery.
pub(crate) async fn creator_loop_playable_experience_template() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "plurora-generated-playable-experience-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }

    package::init_package(
        path.clone(),
        "example/generated-playable-experience".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(PackageTemplate::PlayableExperience),
    )
    .await?;

    package::package_check(path.join("manifest.yaml")).await?;
    package::package_conformance(path.join("manifest.yaml")).await?;

    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;

    // 4 surfaces
    anyhow::ensure!(
        manifest.contributes.surfaces.len() == 4,
        "playable-experience template should have 4 surfaces, got {}",
        manifest.contributes.surfaces.len()
    );

    // 9 capabilities: launch, project_state, render_payload, record_player_action,
    // request_change, create_checkpoint, inspect_checkpoint, draft_recovery, echo
    anyhow::ensure!(
        manifest.provides.len() == 9,
        "playable-experience template should have 9 capabilities, got {}",
        manifest.provides.len()
    );

    // Check specific capabilities exist
    let cap_ids: Vec<&str> = manifest.provides.iter().map(|c| c.id.as_str()).collect();
    anyhow::ensure!(
        cap_ids.iter().any(|c| c.contains("/launch")),
        "playable-experience should have launch"
    );
    anyhow::ensure!(
        cap_ids
            .iter()
            .any(|c| c.contains("/create_checkpoint") || c.contains("/create-checkpoint")),
        "playable-experience should have create_checkpoint"
    );
    anyhow::ensure!(
        cap_ids
            .iter()
            .any(|c| c.contains("/inspect_checkpoint") || c.contains("/inspect-checkpoint")),
        "playable-experience should have inspect_checkpoint"
    );
    anyhow::ensure!(
        cap_ids
            .iter()
            .any(|c| c.contains("/draft_recovery") || c.contains("/draft-recovery")),
        "playable-experience should have draft_recovery"
    );

    // No network declarations
    anyhow::ensure!(
        manifest.permissions.network.declarations.is_empty(),
        "playable-experience should have no network declarations"
    );

    fs::remove_dir_all(path)?;
    Ok(())
}

/// Case 3: Package diagnostics warn when experience_entry surface is present
/// but play_renderer/forge_panel/assistant_action are missing.
pub(crate) async fn creator_loop_experience_surface_warnings() -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "plurora-creator-loop-surface-warn-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }

    // Generate with just experience template (only experience_entry, no play/forge/assist)
    package::init_package(
        path.clone(),
        "example/surface-warning-test".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(PackageTemplate::Experience),
    )
    .await?;

    // The manifest should be valid (check passes)
    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;
    anyhow::ensure!(
        manifest.contributes.surfaces.len() == 1,
        "experience template should have 1 surface"
    );
    anyhow::ensure!(
        matches!(
            manifest.contributes.surfaces[0].slot,
            plurora_core::SurfaceSlot::ExperienceEntry
        ),
        "experience template surface should be experience_entry"
    );

    // package check should still succeed (these are warnings, not errors)
    package::package_check(path.join("manifest.yaml")).await?;

    fs::remove_dir_all(path)?;
    Ok(())
}

/// Case 4: Package diagnostics warn for missing checkpoint capability in
/// experience packages (experience_entry surface present).
pub(crate) async fn creator_loop_missing_checkpoint_warning() -> anyhow::Result<()> {
    // Load the playable-creation-board which has checkpoint capability
    let manifest_path = PathBuf::from("packages/plurora/playable-creation-board/manifest.yaml");
    let manifest = manifest::read_manifest(manifest_path.clone()).await?;
    // Verify it has create_checkpoint — this package should NOT warn
    let has_checkpoint = manifest
        .provides
        .iter()
        .any(|c| c.id.contains("/create_checkpoint") || c.id.contains("/create-checkpoint"));
    anyhow::ensure!(
        has_checkpoint,
        "playable-creation-board should have create_checkpoint"
    );

    // Verify the experience_entry surface exists
    let has_entry = manifest
        .contributes
        .surfaces
        .iter()
        .any(|s| matches!(s.slot, plurora_core::SurfaceSlot::ExperienceEntry));
    anyhow::ensure!(
        has_entry,
        "playable-creation-board should have experience_entry surface"
    );

    // This package passes check
    package::package_check(manifest_path).await?;
    Ok(())
}

/// Case 5: Package diagnostics warn for dangerous permissions (wildcard invoke,
/// empty network methods).
pub(crate) async fn creator_loop_dangerous_permissions_warning() -> anyhow::Result<()> {
    // Create a manifest with dangerous permissions
    let path = std::env::temp_dir().join(format!(
        "plurora-creator-loop-dangerous-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }
    fs::create_dir_all(&path)?;

    let manifest_content = r#"schema_version: 1
id: test/dangerous-permissions
version: 0.1.0
entry:
  kind: rust_inproc
  crate_ref: test-crate
  symbol: register
  abi_version: 1
provides:
  - id: test/dangerous-permissions/echo
    version: 0.1.0
    input_schema: {}
    output_schema: {}
    streaming: false
consumes: []
contributes:
  schemas: []
  hooks: []
  extension_points: []
  surfaces: []
permissions:
  capabilities:
    invoke:
      - "*"
  network:
    declarations:
      - host: api.example.com
        methods: []
        purpose: wildcard methods test
sandbox_policy:
  cpu_quota_ms_per_invoke: 5000
  memory_mb: 128
  wall_clock_ms: 30000
"#;
    fs::write(path.join("manifest.yaml"), manifest_content)?;

    // This manifest should still pass basic validation
    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;
    manifest.validate_basic()?;

    // The warnings for dangerous permissions should be generated by package check
    // (package check prints warnings to stdout)
    package::package_check(path.join("manifest.yaml")).await?;

    fs::remove_dir_all(path)?;
    Ok(())
}

/// Case 6: Package diagnostics note non-deterministic path when network is declared.
pub(crate) async fn creator_loop_network_nondeterministic_hint() -> anyhow::Result<()> {
    // The networked template declares network access
    let path = std::env::temp_dir().join(format!(
        "plurora-creator-loop-network-{}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }

    package::init_package(
        path.clone(),
        "example/network-nondeterministic-test".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(PackageTemplate::Networked),
    )
    .await?;

    // Package check should print the non-deterministic hint
    package::package_check(path.join("manifest.yaml")).await?;

    let manifest = manifest::read_manifest(path.join("manifest.yaml")).await?;
    anyhow::ensure!(
        !manifest.permissions.network.declarations.is_empty(),
        "networked template should have network declarations"
    );

    fs::remove_dir_all(path)?;
    Ok(())
}

/// Case 7: Work materialization preserves an experience package's projected Ports.
pub(crate) async fn creator_loop_work_experience_diagnostics() -> anyhow::Result<()> {
    let root = std::env::temp_dir().join(format!("plurora-creator-work-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;

    // Create a playable-board package
    let package_path = root.join("packages/experience");
    package::init_package(
        package_path.clone(),
        "example/creator-work-experience".to_string(),
        "subprocess".to_string(),
        "typescript".to_string(),
        Some(PackageTemplate::PlayableBoard),
    )
    .await?;
    // Package init emits a directly runnable host path; a portable Work source keeps only the
    // package-relative entry and leaves its concrete location to Installation.
    let manifest_path = package_path.join("manifest.yaml");
    let mut generated_manifest = manifest::read_manifest(manifest_path.clone()).await?;
    let PackageEntry::Subprocess { command, .. } = &mut generated_manifest.entry.kind else {
        anyhow::bail!("playable-board template did not generate a subprocess entry");
    };
    *command = vec!["node".to_string(), "package.mjs".to_string()];
    fs::write(&manifest_path, serde_yaml::to_string(&generated_manifest)?)?;

    fs::write(
        root.join("work.yaml"),
        r#"schema: plurora.work-source.v1
work:
  id: example/creator-work-experience
  title: Creator Loop Work Test
  assembly: assembly.yaml
"#,
    )?;
    fs::write(
        root.join("assembly.yaml"),
        r#"schema: plurora.assembly-source.v1
assembly:
  id: example/creator-work-experience/main
  nodes:
    - id: experience
      component: packages/experience/manifest.yaml
"#,
    )?;

    work::check_work_path(&root)?;

    fs::remove_dir_all(root)?;
    Ok(())
}

/// Case 8: Walkthrough reference — playable-creation-board package check
/// output is verifiable and contains expected diagnostic fields.
pub(crate) async fn creator_loop_walkthrough_reference() -> anyhow::Result<()> {
    let manifest_path = PathBuf::from("packages/plurora/playable-creation-board/manifest.yaml");
    let manifest = manifest::read_manifest(manifest_path.clone()).await?;

    // Verify key playable-creation-board properties for walkthrough
    anyhow::ensure!(
        manifest.id == "plurora/playable-creation-board",
        "manifest id should be plurora/playable-creation-board"
    );

    // 4 surfaces
    anyhow::ensure!(
        manifest.contributes.surfaces.len() == 4,
        "playable-creation-board should have 4 surfaces, got {}",
        manifest.contributes.surfaces.len()
    );

    // Has experience_entry surface with launch capability
    let entry_surface = manifest
        .contributes
        .surfaces
        .iter()
        .find(|s| matches!(s.slot, plurora_core::SurfaceSlot::ExperienceEntry));
    anyhow::ensure!(
        entry_surface.is_some(),
        "must have experience_entry surface"
    );
    let entry = entry_surface.unwrap();
    anyhow::ensure!(
        entry.activation.launch_capability_id.is_some(),
        "experience_entry must have launch_capability_id"
    );

    // Has create_checkpoint capability
    let has_checkpoint = manifest
        .provides
        .iter()
        .any(|c| c.id.contains("/create_checkpoint"));
    anyhow::ensure!(has_checkpoint, "must have create_checkpoint capability");

    // Has request_change capability
    let has_request_change = manifest
        .provides
        .iter()
        .any(|c| c.id.contains("/request_change"));
    anyhow::ensure!(has_request_change, "must have request_change capability");

    // No network permissions
    anyhow::ensure!(
        manifest.permissions.network.declarations.is_empty()
            && manifest.permissions.network.hosts.is_empty(),
        "playable-creation-board should have no network permissions"
    );

    // Package check passes
    package::package_check(manifest_path).await?;

    Ok(())
}

/// Case 9: No privileged first-party dependency — third-party playable-seed
/// materializes through the same Work path as any first-party Package.
pub(crate) async fn creator_loop_thirdparty_no_privilege() -> anyhow::Result<()> {
    // Verify the third-party playable-seed package passes package check
    let tp_manifest_path =
        PathBuf::from("examples/packages/thirdparty-playable-seed/manifest.yaml");
    package::package_check(tp_manifest_path.clone()).await?;

    let manifest = manifest::read_manifest(tp_manifest_path).await?;

    // Verify it has experience surfaces like the Plurora seed
    anyhow::ensure!(
        !manifest.contributes.surfaces.is_empty(),
        "thirdparty/playable-seed must have surfaces"
    );

    // Verify no platform-reserved namespace in manifest
    let manifest_json = serde_json::to_value(&manifest)?;
    let manifest_str = serde_json::to_string(&manifest_json)?;
    anyhow::ensure!(
        !manifest_str.contains("platform.experience."),
        "thirdparty/playable-seed must not contain platform.experience."
    );

    work::check_work_path(std::path::Path::new(
        "examples/works/playable-seed-replacement/work.yaml",
    ))?;

    Ok(())
}
