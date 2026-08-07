use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use super::manifest::read_manifest;
use super::package::init_package;
use crate::cli::CompositionDescriptor;

pub(crate) async fn init_composition(path: PathBuf, id: String) -> Result<()> {
    fs::create_dir_all(&path)?;
    let package_path = path.join("package");
    init_package(
        package_path.clone(),
        id.clone(),
        "subprocess".to_string(),
        "python-experience".to_string(),
        None,
    )
    .await?;
    fs::write(
        path.join("composition.yaml"),
        format!(
            r#"id: {id}
version: 0.1.0
entry_surface_id: {id}/entry
# title: "{id} composition"
# description: "A composition descriptor for {id}"
packages:
  - package/manifest.yaml
required_surfaces:
  - experience_entry
  - play_renderer
  - forge_panel
# optional_packages:
  # - ../extras/optional-package/manifest.yaml
# required_capabilities:
  # - official/composition-lab/launch_plan
# default_activation:
  # auto_launch: true
# permission_expectations:
  # - capabilities.invoke
# replacement_candidates:
  # - official/experience-alt
# compatibility_notes:
  # - "Requires kernel v0.1.0 or later"
"#
        ),
    )?;
    println!(
        "initialized composition descriptor at {}",
        path.join("composition.yaml").display()
    );
    Ok(())
}

pub(crate) async fn composition_check(path: PathBuf) -> Result<()> {
    let raw = fs::read_to_string(&path)?;
    let composition: CompositionDescriptor = match path.extension().and_then(|ext| ext.to_str()) {
        Some("yaml") | Some("yml") => serde_yaml::from_str(&raw)?,
        _ => serde_json::from_str(&raw)?,
    };
    anyhow::ensure!(
        !composition.id.trim().is_empty(),
        "composition id is required"
    );
    anyhow::ensure!(
        !composition.version.trim().is_empty(),
        "composition version is required"
    );
    anyhow::ensure!(
        !composition.entry_surface_id.trim().is_empty(),
        "composition entry_surface_id is required"
    );

    let base = path.parent().unwrap_or_else(|| std::path::Path::new("."));

    // Collect surface and capability info from required packages
    let mut surface_ids = Vec::new();
    let mut slots = Vec::new();
    let mut capability_ids = Vec::new();
    let mut warnings = Vec::new();
    let mut loaded_packages = Vec::new();

    for package_path in &composition.packages {
        let resolved = if package_path.is_absolute() {
            package_path.clone()
        } else {
            base.join(package_path)
        };
        match read_manifest(resolved).await {
            Ok(manifest) => {
                manifest.validate_basic()?;
                for surface in manifest.contributes.surfaces {
                    let slot = serde_json::to_value(&surface.slot)?
                        .as_str()
                        .unwrap_or_default()
                        .to_string();
                    surface_ids.push(surface.id);
                    slots.push(slot);
                }
                for cap in &manifest.provides {
                    capability_ids.push(cap.id.clone());
                }
                loaded_packages.push(package_path.display().to_string());
            }
            Err(e) => {
                anyhow::bail!(
                    "required package '{}' failed to load: {e}",
                    package_path.display()
                );
            }
        }
    }

    // Collect info from optional packages (warn only on missing)
    let mut optional_loaded = Vec::new();
    for package_path in &composition.optional_packages {
        let resolved = if package_path.is_absolute() {
            package_path.clone()
        } else {
            base.join(package_path)
        };
        match read_manifest(resolved).await {
            Ok(manifest) => {
                manifest.validate_basic()?;
                for surface in manifest.contributes.surfaces {
                    let slot = serde_json::to_value(&surface.slot)?
                        .as_str()
                        .unwrap_or_default()
                        .to_string();
                    surface_ids.push(surface.id);
                    slots.push(slot);
                }
                for cap in &manifest.provides {
                    capability_ids.push(cap.id.clone());
                }
                optional_loaded.push(package_path.display().to_string());
            }
            Err(e) => {
                warnings.push(format!(
                    "optional package '{}' not loaded: {e}",
                    package_path.display()
                ));
            }
        }
    }

    // Build indexed lookup sets for membership checks
    let surface_id_set: BTreeSet<&str> = surface_ids.iter().map(|s| s.as_str()).collect();
    let slot_set: BTreeSet<&str> = slots.iter().map(|s| s.as_str()).collect();
    let capability_id_set: BTreeSet<&str> = capability_ids.iter().map(|s| s.as_str()).collect();

    // Build slot counts from the indexed BTreeMap (already existed for diagnostics)
    // and surface_id → slot index for entry surface lookup
    let surface_slot_map: BTreeMap<&str, &str> = surface_ids
        .iter()
        .zip(slots.iter())
        .map(|(id, slot)| (id.as_str(), slot.as_str()))
        .collect();

    // Check entry surface
    if !surface_id_set.contains(composition.entry_surface_id.as_str()) {
        anyhow::bail!(
            "entry surface '{}' not provided by composition packages",
            composition.entry_surface_id
        );
    }

    // Check required surfaces
    let mut missing_surfaces = Vec::new();
    for required in &composition.required_surfaces {
        if !slot_set.contains(required.as_str()) {
            missing_surfaces.push(required.clone());
        }
    }
    if !missing_surfaces.is_empty() {
        anyhow::bail!(
            "required surface slots missing: {}",
            missing_surfaces.join(", ")
        );
    }

    // Check required capabilities
    let mut missing_capabilities = Vec::new();
    for required_cap in &composition.required_capabilities {
        if !capability_id_set.contains(required_cap.as_str()) {
            missing_capabilities.push(required_cap.clone());
        }
    }
    if !missing_capabilities.is_empty() {
        anyhow::bail!(
            "required capabilities missing: {}",
            missing_capabilities.join(", ")
        );
    }

    // Print structured diagnostics
    println!("composition: {}@{}", composition.id, composition.version);
    if let Some(ref title) = composition.title {
        println!("  title: {title}");
    }
    if let Some(ref desc) = composition.description {
        println!("  description: {desc}");
    }
    println!("  entry_surface_id: {}", composition.entry_surface_id);

    println!("  required packages ({}):", loaded_packages.len());
    for pkg in &loaded_packages {
        println!("    - {pkg}");
    }

    println!(
        "  optional packages ({} loaded, {} total):",
        optional_loaded.len(),
        composition.optional_packages.len()
    );
    for pkg in &optional_loaded {
        println!("    - {pkg}");
    }

    println!("  surfaces by slot ({}):", slots.len());
    let mut slot_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for slot in &slots {
        *slot_counts.entry(slot.as_str()).or_insert(0) += 1;
    }
    for (slot, count) in &slot_counts {
        println!("    {slot}: {count}");
    }

    println!("  capabilities ({}):", capability_ids.len());
    for cap in &capability_ids {
        println!("    - {cap}");
    }

    if !composition.required_capabilities.is_empty() {
        println!(
            "  required capabilities: {} (all present)",
            composition.required_capabilities.len()
        );
    }

    println!(
        "  entry activation: {}",
        if composition.default_activation.is_some() {
            "present"
        } else {
            "missing"
        }
    );

    if !composition.permission_expectations.is_empty() {
        println!("  permission expectations:");
        for perm in &composition.permission_expectations {
            println!("    - {perm}");
        }
    }

    if !composition.replacement_candidates.is_empty() {
        println!("  replacement candidates:");
        for cand in &composition.replacement_candidates {
            println!("    - {cand}");
        }
    }

    if !composition.compatibility_notes.is_empty() {
        println!("  compatibility notes:");
        for note in &composition.compatibility_notes {
            println!("    - {note}");
        }
    }

    for warning in &warnings {
        println!("  WARNING: {warning}");
    }

    // --- Experience package set diagnostics (Beta 5) ---

    // Entry surface check
    let entry_surface_slot = surface_slot_map
        .get(composition.entry_surface_id.as_str())
        .copied();
    if let Some(slot) = entry_surface_slot {
        println!("  entry surface slot: {slot}");
    } else if !composition.entry_surface_id.is_empty() {
        println!("  entry surface slot: (not found among loaded packages)");
    }

    // Surface slot coverage summary
    let required_experience_slots = [
        "experience_entry",
        "play_renderer",
        "forge_panel",
        "assistant_action",
    ];
    println!("  experience surface coverage:");
    for slot in &required_experience_slots {
        let count = slot_counts.get(slot).copied().unwrap_or(0);
        let status = if count > 0 { "covered" } else { "missing" };
        println!("    {slot}: {status} ({count} package(s))");
    }

    // Replacement candidates diagnostics
    if !composition.replacement_candidates.is_empty() {
        println!("  replacement candidates:");
        for cand in &composition.replacement_candidates {
            // Check if any loaded package could serve as replacement (prefix-based, keep helper)
            let is_loaded = capability_ids
                .iter()
                .any(|cap| cap.starts_with(cand.as_str()));
            let status = if is_loaded { "available" } else { "not loaded" };
            println!("    - {cand} ({status})");
        }
    } else {
        // No explicit replacement candidates declared
        // Check if multiple packages provide the same slot
        let mut slot_providers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (slot, surface_id) in slots.iter().zip(surface_ids.iter()) {
            // Extract package id from surface id (prefix before last /)
            let pkg_id = surface_id
                .rfind('/')
                .map(|i| &surface_id[..i])
                .unwrap_or(surface_id);
            slot_providers.entry(slot).or_default().push(pkg_id);
        }
        let multi_slots: Vec<(&str, usize)> = slot_providers
            .iter()
            .filter(|(_, pkgs)| pkgs.len() > 1)
            .map(|(slot, pkgs)| (*slot, pkgs.len()))
            .collect();
        if !multi_slots.is_empty() {
            println!("  replacement hint: multiple packages provide the same surface slot(s):");
            for (slot, count) in &multi_slots {
                println!("    - {slot}: {count} provider(s) — consider declaring replacement_candidates for explicit selection");
            }
        }
    }

    // Permission expectations diagnostics
    if !composition.permission_expectations.is_empty() {
        println!("  permission expectations:");
        for perm in &composition.permission_expectations {
            println!("    - {perm}");
        }
    }

    // Checkpoint/state capability coverage (uses contains-based suffix check on indexed set)
    let has_experience_entry_slot = slot_set.contains("experience_entry");
    if has_experience_entry_slot {
        let checkpoint_count = capability_ids
            .iter()
            .filter(|cap| cap.contains("/create_checkpoint") || cap.contains("/create-checkpoint"))
            .count();
        let recovery_count = capability_ids
            .iter()
            .filter(|cap| cap.contains("/draft_recovery") || cap.contains("/draft-recovery"))
            .count();
        println!("  state capability coverage:");
        let cp_status = if checkpoint_count == 0 {
            "missing".to_string()
        } else {
            format!("{} provider(s)", checkpoint_count)
        };
        let rec_status = if recovery_count == 0 {
            "missing".to_string()
        } else {
            format!("{} provider(s)", recovery_count)
        };
        println!("    create_checkpoint: {}", cp_status);
        println!("    draft_recovery: {}", rec_status);
        if checkpoint_count == 0 {
            println!("    hint: add a checkpoint capability for session save/restore support");
        }
        if recovery_count == 0 {
            println!("    hint: add a draft_recovery capability for failure recovery support");
        }
    }

    // Memory/observability optional packages hint
    if has_experience_entry_slot {
        let has_memory = capability_ids.iter().any(|cap| {
            cap.contains("/record_memory")
                || cap.contains("/record-memory")
                || cap.contains("/retrieve_memory")
                || cap.contains("/retrieve-memory")
        });
        let has_observability = capability_ids.iter().any(|cap| {
            cap.contains("/summarize_session_health")
                || cap.contains("/summarize-session-health")
                || cap.contains("/summarize_experience_health")
                || cap.contains("/summarize-experience-health")
        });
        println!("  optional package coverage:");
        println!(
            "    memory: {}",
            if has_memory {
                "present"
            } else {
                "not present — consider adding memory-lab for long-term memory"
            }
        );
        println!(
            "    observability: {}",
            if has_observability {
                "present"
            } else {
                "not present — consider adding experience-observability-lab for health/diagnostics"
            }
        );
    }

    println!(
        "composition check: {}@{} ok",
        composition.id, composition.version
    );
    Ok(())
}
