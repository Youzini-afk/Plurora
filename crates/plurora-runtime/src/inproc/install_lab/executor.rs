use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use plurora_core::{
    package_envelope_for_manifest, protocol_profile_pins_for_envelope, ComponentLockPin,
    ExternalWorkspaceOwnership, LockEntry, LockRequirement, LockSource, Lockfile, PackageEntry,
    PackageManifest, ProjectId,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::inproc::invoke_capability_from_inproc;
use crate::CapabilityInvocationRequest;

use super::fs_copy::{
    copy_dir_atomic, installed_manifest_path, safe_relative_path, verify_installed_hashes,
};
use super::layout::{
    atomic_write, ensure_layout, lockfile_path, profile_path, profiles_dir, store_dir,
    store_path_for_hash,
};
use super::planner::verify_consent;
use super::project_kind::{read_project_descriptor, write_and_register_project};
use super::source::{manifest_path_in, parse_manifest_at, value_str};
use super::types::{
    Consent, ExecutePlanInput, InstallPlan, ProfileInput, RegisterProjectInput, UninstallInput,
};
use super::PACKAGE_ID;

const STORE_GC_EVENT_KIND: &str = "kernel/v1/install-lab.store.gc";

pub(super) async fn execute_plan(input: Value, _session_id: Option<&str>) -> Result<Value> {
    if input.as_object().is_some_and(|object| object.is_empty()) {
        anyhow::bail!("unsupported smoke input: missing plan");
    }
    let input: ExecutePlanInput = serde_json::from_value(input)?;
    verify_consent(&input.plan, &input.consent)?;
    ensure_layout(input.data_dir.as_deref())?;
    let store_dir = store_dir(input.data_dir.as_deref())?;
    let profiles_dir = profiles_dir(input.data_dir.as_deref())?;
    let staging_dir = store_dir.join(".staging");
    fs::create_dir_all(&staging_dir)?;
    fs::create_dir_all(&profiles_dir)?;

    let mut installed = Vec::new();
    for pkg in input.plan.packages.iter().rev() {
        let store_path = store_path_for_hash(&pkg.tree_hash, input.data_dir.as_deref())?;
        if !store_path.exists() {
            let staging = staging_dir.join(Uuid::new_v4().to_string());
            match pkg.source.as_str() {
                "local" => copy_dir_atomic(
                    Path::new(pkg.path.as_deref().context("local package missing path")?),
                    &staging,
                    &store_path,
                )?,
                "git" => {
                    let url = pkg.url.as_deref().context("git package missing url")?;
                    let commit = pkg
                        .commit_sha
                        .as_deref()
                        .context("git package missing commit_sha")?;
                    let ref_name = pkg.ref_name.as_deref().unwrap_or(commit);
                    invoke_package_capability(
                        "official/git-tools-lab",
                        "official/git-tools-lab/fetch_tree",
                        json!({ "remote_url": url, "commit_sha": commit, "ref_name": ref_name, "dest_dir": staging.to_string_lossy() }),
                    )
                    .await?;
                    fs::rename(&staging, &store_path).with_context(|| {
                        format!(
                            "failed to atomically move {} to {}",
                            staging.display(),
                            store_path.display()
                        )
                    })?;
                }
                other => anyhow::bail!("unsupported install source: {other}"),
            }
        }
        verify_installed_hashes(pkg, &store_path).await?;
        installed.push(json!({
            "id": pkg.id,
            "store_path": store_path.to_string_lossy(),
            "manifest_path": installed_manifest_path(pkg, &store_path)?.to_string_lossy(),
        }));
    }

    let profile_path = profile_path(&input.profile, input.data_dir.as_deref())?;
    let lockfile_path = lockfile_path(&input.profile, input.data_dir.as_deref())?;
    write_profile(&profile_path, &input.plan, input.data_dir.as_deref())?;
    let lock = build_lockfile(
        &input.profile,
        &input.plan,
        &input.consent,
        input.data_dir.as_deref(),
    )?;
    let lockfile = toml::to_string_pretty(&lock)?;
    atomic_write(&lockfile_path, lockfile.as_bytes())?;

    let project_descriptor = input
        .project_descriptor
        .or_else(|| input.plan.project_descriptor.clone());
    let registered_project = if let Some(descriptor) = project_descriptor {
        let registered = write_and_register_project(descriptor.clone(), input.data_dir.as_deref())?;
        copy_project_surface_dist(&descriptor, &input.plan, input.data_dir.as_deref())?;
        Some(registered)
    } else {
        let root = input.plan.packages.first();
        if let Some(root) = root {
            let store_path = store_path_for_hash(&root.tree_hash, input.data_dir.as_deref())?;
            let project_yaml = store_path.join("project.yaml");
            if project_yaml.is_file() {
                let descriptor = read_project_descriptor(&project_yaml)?;
                Some(write_and_register_project(
                    descriptor,
                    input.data_dir.as_deref(),
                )?)
            } else {
                None
            }
        } else {
            None
        }
    };

    Ok(json!({
        "installed": installed,
        "lockfile_path": lockfile_path.to_string_lossy(),
        "profile_path": profile_path.to_string_lossy(),
        "lockfile": lockfile,
        "project": registered_project,
    }))
}

pub(super) async fn register_project_capability(input: Value) -> Result<Value> {
    let input: RegisterProjectInput = serde_json::from_value(input)?;
    let info = write_and_register_project(input.descriptor, input.data_dir.as_deref())?;
    Ok(info)
}

pub(super) async fn uninstall(input: Value, session_id: Option<&str>) -> Result<Value> {
    if input.as_object().is_some_and(|object| object.is_empty()) {
        return Ok(json!({ "removed_from_profile": false, "store_path_orphaned": null }));
    }
    let input: UninstallInput = serde_json::from_value(input)?;
    let profile_path = profile_path(&input.profile, input.data_dir.as_deref())?;
    let lockfile_path = lockfile_path(&input.profile, input.data_dir.as_deref())?;
    let mut target_package_ids = input.package_ids.clone();
    if let Some(package_id) = &input.package_id {
        target_package_ids.push(package_id.clone());
    }
    let project_id = input
        .project_id
        .as_deref()
        .map(ProjectId::new)
        .transpose()?;
    let project_descriptor = project_id
        .as_ref()
        .map(|id| {
            read_project_descriptor(
                &project_dir(input.data_dir.as_deref(), id).join("project.yaml"),
            )
        })
        .transpose()?;
    let target_manifest_relatives = project_descriptor
        .as_ref()
        .map(|descriptor| descriptor.project.packages.clone())
        .unwrap_or_default();
    target_package_ids.sort();
    target_package_ids.dedup();
    anyhow::ensure!(
        !target_package_ids.is_empty() || project_id.is_some(),
        "uninstall requires package_id, package_ids, or project_id"
    );

    let mut orphaned = Vec::new();
    let mut manifest_paths_for_removed = Vec::new();
    let mut removed = false;

    if lockfile_path.exists() {
        let raw = fs::read_to_string(&lockfile_path)?;
        let mut lock: Lockfile = toml::from_str(&raw)?;
        for entry in lock.package.iter().filter(|entry| {
            should_uninstall_lock_entry(entry, &target_package_ids, &target_manifest_relatives)
        }) {
            orphaned.push(entry.installed_at_store.clone());
            manifest_paths_for_removed.extend(manifest_candidates_for_lock_entry(entry));
        }
        let before = lock.package.len();
        lock.package.retain(|entry| {
            !should_uninstall_lock_entry(entry, &target_package_ids, &target_manifest_relatives)
        });
        removed = before != lock.package.len();
        atomic_write(&lockfile_path, toml::to_string_pretty(&lock)?.as_bytes())?;
    }

    if profile_path.exists() {
        let raw = fs::read_to_string(&profile_path)?;
        let mut value: Value = serde_yaml::from_str(&raw)?;
        if let Some(autoload) = value.get_mut("autoload").and_then(Value::as_array_mut) {
            let before = autoload.len();
            let removed_manifest_paths = orphaned
                .iter()
                .flat_map(|store| {
                    [
                        format!("{store}/manifest.yaml"),
                        format!("{store}/manifest.json"),
                    ]
                })
                .chain(manifest_paths_for_removed.iter().cloned())
                .map(|path| portable_profile_path(&path))
                .collect::<std::collections::HashSet<_>>();
            autoload.retain(|entry| {
                !entry.as_str().is_some_and(|path| {
                    removed_manifest_paths.contains(&portable_profile_path(path))
                })
            });
            removed |= before != autoload.len();
        }
        atomic_write(&profile_path, serde_yaml::to_string(&value)?.as_bytes())?;
    }

    let project = if let Some(id) = &project_id {
        let (data_action, workspace_action) = uninstall_project_data(
            id,
            project_descriptor.as_ref(),
            input.data_dir.as_deref(),
            input.delete_project_data,
        )?;
        Some(json!({
            "project_id": id.as_str(),
            "data_action": data_action,
            "workspace_action": workspace_action,
        }))
    } else {
        None
    };
    let store_gc = if input.purge_orphaned_stores {
        let report = super::gc::prune_orphaned_stores(input.data_dir.as_deref())?;
        let store_gc = store_gc_json(report);
        emit_store_gc_event(session_id, &store_gc).await?;
        Some(store_gc)
    } else {
        None
    };
    Ok(json!({
        "removed_from_profile": removed,
        "store_path_orphaned": orphaned.first().cloned(),
        "store_paths_orphaned": orphaned,
        "store_gc": store_gc,
        "project": project,
    }))
}

fn store_gc_json(report: super::gc::StoreGcReport) -> Value {
    json!({
        "event_kind": STORE_GC_EVENT_KIND,
        "removed_paths": report
            .removed_paths
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        "orphaned_paths": report
            .orphaned_paths
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        "ignored_store_entries": report
            .ignored_store_entries
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>(),
    })
}

async fn emit_store_gc_event(session_id: Option<&str>, store_gc: &Value) -> Result<()> {
    if let Some(session_id) = session_id {
        crate::inproc::append_kernel_event_from_inproc(
            session_id,
            STORE_GC_EVENT_KIND,
            store_gc.clone(),
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn list_installed(input: Value) -> Result<Value> {
    let input: ProfileInput = serde_json::from_value(input)?;
    let lockfile_path = lockfile_path(&input.profile, input.data_dir.as_deref())?;
    if !lockfile_path.exists() {
        return Ok(json!({ "packages": [] }));
    }
    let lock: Lockfile = toml::from_str(&fs::read_to_string(lockfile_path)?)?;
    let packages = lock
        .package
        .into_iter()
        .map(|entry| {
            json!({
                "id": entry.id,
                "version": entry.version,
                "store_path": entry.installed_at_store,
                "granted_capabilities": entry.granted_capabilities,
                "granted_network": entry.granted_network,
                "granted_secrets": entry.granted_secrets,
                "tree_hash": entry.tree_hash,
                "manifest_hash": entry.manifest_hash,
                "package_envelope_digest": entry.package_envelope_digest,
                "component_pins": entry.component_pins,
                "protocol_profile_pins": entry.protocol_profile_pins,
                "content_roots": entry.content_roots,
                "manifest_relative_path": entry.manifest_relative_path,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "packages": packages }))
}

pub(super) async fn check_lockfile(input: Value) -> Result<Value> {
    let input: ProfileInput = serde_json::from_value(input)?;
    let lockfile_path = lockfile_path(&input.profile, input.data_dir.as_deref())?;
    if !lockfile_path.exists() {
        return Ok(
            json!({ "ok": false, "drift": [{ "id": "lockfile", "kind": "missing", "expected": lockfile_path.to_string_lossy(), "actual": null }] }),
        );
    }
    let lock: Lockfile = toml::from_str(&fs::read_to_string(lockfile_path)?)?;
    let mut drift = Vec::new();
    for entry in &lock.package {
        let store = PathBuf::from(&entry.installed_at_store);
        if !store.is_dir() {
            drift.push(json!({ "id": entry.id, "kind": "missing_store", "expected": entry.installed_at_store, "actual": null }));
            continue;
        }
        let manifest_path = if let Some(relative) = &entry.manifest_relative_path {
            store.join(safe_relative_path(relative)?)
        } else {
            match manifest_path_in(&store) {
                Ok(path) => path,
                Err(_) => {
                    drift.push(json!({ "id": entry.id, "kind": "missing_manifest", "expected": "manifest.yaml or manifest.json", "actual": null }));
                    continue;
                }
            }
        };
        if !manifest_path.is_file() {
            drift.push(json!({ "id": entry.id, "kind": "missing_manifest", "expected": manifest_path.to_string_lossy(), "actual": null }));
            continue;
        }
        let manifest_hash = compute_manifest_hash(&manifest_path).await?;
        if manifest_hash != entry.manifest_hash {
            drift.push(json!({ "id": entry.id, "kind": "manifest_hash", "expected": entry.manifest_hash, "actual": manifest_hash }));
        }
        if let Some(expected) = &entry.surface_bundle_hash {
            match surface_bundle_path_for_manifest(&manifest_path) {
                Ok(bundle_path) => {
                    let actual = compute_file_hash(&bundle_path)?;
                    if &actual != expected {
                        drift.push(json!({ "id": entry.id, "kind": "surface_bundle_hash", "expected": expected, "actual": actual }));
                    }
                }
                Err(error) => {
                    drift.push(json!({ "id": entry.id, "kind": "surface_bundle_hash", "expected": expected, "actual": error.to_string() }));
                }
            }
        }
        if entry.package_envelope_digest.is_some()
            || !entry.component_pins.is_empty()
            || !entry.protocol_profile_pins.is_empty()
            || !entry.content_roots.is_empty()
        {
            let manifest = parse_manifest_at(&manifest_path)?;
            let envelope = package_envelope_for_manifest(&manifest)?;
            if entry.package_envelope_digest.as_deref() != Some(envelope.artifact.digest.as_str()) {
                drift.push(json!({
                    "id": entry.id,
                    "kind": "package_envelope_digest",
                    "expected": entry.package_envelope_digest,
                    "actual": envelope.artifact.digest,
                }));
            }
            let component_pins = envelope
                .components
                .iter()
                .map(ComponentLockPin::from_descriptor)
                .collect::<Vec<_>>();
            if entry.component_pins != component_pins {
                drift.push(json!({
                    "id": entry.id,
                    "kind": "component_pins",
                    "expected": entry.component_pins,
                    "actual": component_pins,
                }));
            }
            let protocol_profile_pins = protocol_profile_pins_for_envelope(&envelope);
            if entry.protocol_profile_pins != protocol_profile_pins {
                drift.push(json!({
                    "id": entry.id,
                    "kind": "protocol_profile_pins",
                    "expected": entry.protocol_profile_pins,
                    "actual": protocol_profile_pins,
                }));
            }
            if entry.content_roots != envelope.content_roots {
                drift.push(json!({
                    "id": entry.id,
                    "kind": "content_roots",
                    "expected": entry.content_roots,
                    "actual": envelope.content_roots,
                }));
            }
        }
        let tree_hash = compute_tree_hash(&store).await?;
        if tree_hash != entry.tree_hash {
            drift.push(json!({ "id": entry.id, "kind": "tree_hash", "expected": entry.tree_hash, "actual": tree_hash }));
        }
    }
    Ok(json!({ "ok": drift.is_empty(), "drift": drift }))
}

fn copy_project_surface_dist(
    descriptor: &plurora_core::ProjectDescriptor,
    plan: &InstallPlan,
    data_dir_override: Option<&str>,
) -> Result<()> {
    let Some((pkg, manifest)) = find_surface_bundle_package(plan, data_dir_override)? else {
        return Ok(());
    };
    let PackageEntry::SurfaceBundle { bundle } = &manifest.entry.kind else {
        return Ok(());
    };
    let store_path = store_path_for_hash(&pkg.tree_hash, data_dir_override)?;
    let manifest_path = installed_manifest_path(pkg, &store_path)?;
    if let Some(expected) = &pkg.surface_bundle_hash {
        let actual =
            compute_file_hash(&surface_bundle_path_from_manifest(&manifest_path, bundle)?)?;
        if &actual != expected {
            anyhow::bail!("surface bundle hash mismatch for {}", manifest.id);
        }
    }
    let package_root = manifest_path.parent().unwrap_or(&store_path);
    let bundle_path = safe_relative_path(bundle.as_str())?;
    let source_dist = bundle_path
        .parent()
        .map(|parent| package_root.join(parent))
        .unwrap_or_else(|| package_root.to_path_buf());
    if !source_dist.is_dir() {
        anyhow::bail!(
            "surface bundle dist directory does not exist for {}: {}",
            manifest.id,
            source_dist.display()
        );
    }
    let project_dir = super::project_kind::ensure_project_initialized_for(
        &descriptor.project.id,
        data_dir_override,
    )?;
    let dest_dist = project_dir.join("dist");
    super::fs_copy::replace_dir_atomic(&source_dist, &dest_dist)
}

fn find_surface_bundle_package<'a>(
    plan: &'a InstallPlan,
    data_dir_override: Option<&str>,
) -> Result<Option<(&'a super::types::PlannedPackage, PackageManifest)>> {
    for pkg in &plan.packages {
        let store_path = store_path_for_hash(&pkg.tree_hash, data_dir_override)?;
        let manifest_path = installed_manifest_path(pkg, &store_path)?;
        let manifest = parse_manifest_at(&manifest_path)?;
        if matches!(manifest.entry.kind, PackageEntry::SurfaceBundle { .. }) {
            return Ok(Some((pkg, manifest)));
        }
    }
    Ok(None)
}

fn manifest_candidates_for_lock_entry(entry: &LockEntry) -> Vec<String> {
    if let Some(relative) = &entry.manifest_relative_path {
        vec![format!("{}/{}", entry.installed_at_store, relative)]
    } else {
        vec![
            format!("{}/manifest.yaml", entry.installed_at_store),
            format!("{}/manifest.json", entry.installed_at_store),
        ]
    }
}

fn should_uninstall_lock_entry(
    entry: &LockEntry,
    target_package_ids: &[String],
    target_manifest_relatives: &[String],
) -> bool {
    target_package_ids.iter().any(|id| id == &entry.id)
        || entry
            .manifest_relative_path
            .as_ref()
            .is_some_and(|relative| {
                target_manifest_relatives
                    .iter()
                    .any(|candidate| candidate == relative)
            })
}

fn project_dir(data_dir_override: Option<&str>, id: &ProjectId) -> PathBuf {
    if let Some(dir) = data_dir_override {
        PathBuf::from(dir).join("projects").join(id.as_str())
    } else {
        plurora_core::paths::project_dir(id)
            .unwrap_or_else(|_| PathBuf::from("projects").join(id.as_str()))
    }
}

fn uninstall_project_data(
    id: &ProjectId,
    descriptor: Option<&plurora_core::ProjectDescriptor>,
    data_dir_override: Option<&str>,
    delete_project_data: bool,
) -> Result<(&'static str, &'static str)> {
    let from = project_dir(data_dir_override, id);
    if !from.exists() {
        crate::inproc::project_registry_from_inproc()?.unregister(id);
        let workspace_action =
            uninstall_managed_workspace(id, descriptor, data_dir_override, delete_project_data)?;
        return Ok(("missing", workspace_action));
    }
    if delete_project_data {
        fs::remove_dir_all(&from)?;
        let workspace_action =
            uninstall_managed_workspace(id, descriptor, data_dir_override, true)?;
        crate::inproc::project_registry_from_inproc()?.unregister(id);
        return Ok(("deleted", workspace_action));
    }

    let archived = from
        .parent()
        .unwrap_or_else(|| Path::new("projects"))
        .join(".archived");
    fs::create_dir_all(&archived)?;
    let to = archived.join(id.as_str());
    if to.exists() {
        fs::remove_dir_all(&to)?;
    }
    if fs::rename(&from, &to).is_err() {
        super::fs_copy::copy_dir_recursive(&from, &to)?;
        fs::remove_dir_all(&from)?;
    }
    let workspace_action = uninstall_managed_workspace(id, descriptor, data_dir_override, false)?;
    crate::inproc::project_registry_from_inproc()?.unregister(id);
    Ok(("archived", workspace_action))
}

fn uninstall_managed_workspace(
    id: &ProjectId,
    descriptor: Option<&plurora_core::ProjectDescriptor>,
    data_dir_override: Option<&str>,
    delete: bool,
) -> Result<&'static str> {
    let Some(external) = descriptor.and_then(|descriptor| descriptor.project.external.as_ref())
    else {
        return Ok("not_applicable");
    };
    if external.workspace_ownership != Some(ExternalWorkspaceOwnership::Managed) {
        return Ok("preserved_linked_source");
    }
    let Some(data_dir) = data_dir_override
        .map(PathBuf::from)
        .or_else(|| plurora_core::paths::data_dir().ok())
    else {
        return Ok("missing");
    };
    let data_dir = fs::canonicalize(&data_dir).with_context(|| {
        format!(
            "failed to canonicalize data directory {}",
            data_dir.display()
        )
    })?;
    let workspaces_root = data_dir.join("workspaces");
    let external_root = workspaces_root.join("external");
    let managed_root = external_root.join(id.as_str());
    let Some(workspace_root) = external.workspace_root.as_deref().map(PathBuf::from) else {
        return Ok("missing");
    };
    if !managed_root.exists() {
        return Ok("missing");
    }
    for owned_root in [&workspaces_root, &external_root] {
        let metadata = fs::symlink_metadata(owned_root)?;
        anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "refusing to manage a symlinked workspace ancestor: {}",
            owned_root.display()
        );
    }
    let root_metadata = fs::symlink_metadata(&managed_root)?;
    anyhow::ensure!(
        root_metadata.is_dir() && !root_metadata.file_type().is_symlink(),
        "refusing to manage a symlinked external workspace root: {}",
        managed_root.display()
    );
    let canonical_workspaces_root = fs::canonicalize(&workspaces_root)?;
    let canonical_external_root = fs::canonicalize(&external_root)?;
    let canonical_managed_root = fs::canonicalize(&managed_root)?;
    let canonical_workspace_root = fs::canonicalize(&workspace_root).with_context(|| {
        format!(
            "failed to canonicalize managed external workspace {}",
            workspace_root.display()
        )
    })?;
    anyhow::ensure!(
        canonical_workspaces_root.parent() == Some(data_dir.as_path())
            && canonical_external_root.parent() == Some(canonical_workspaces_root.as_path())
            && canonical_managed_root.parent() == Some(canonical_external_root.as_path())
            && canonical_managed_root
                .file_name()
                .and_then(|name| name.to_str())
                == Some(id.as_str())
            && canonical_workspace_root.parent() == Some(canonical_managed_root.as_path()),
        "refusing to manage external workspace outside owned project root: {}",
        workspace_root.display()
    );
    if delete {
        fs::remove_dir_all(&managed_root)?;
        return Ok("deleted");
    }
    let archived_workspaces = ensure_uninstall_owned_directory(
        &canonical_workspaces_root,
        ".archived",
        "workspace archive root",
    )?;
    let archive_root = ensure_uninstall_owned_directory(
        &archived_workspaces,
        "external",
        "external workspace archive root",
    )?;
    let archived = archive_root.join(id.as_str());
    match fs::symlink_metadata(&archived) {
        Ok(metadata) => {
            anyhow::ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "refusing to replace a symlinked external workspace archive: {}",
                archived.display()
            );
            let canonical_archived = fs::canonicalize(&archived)?;
            anyhow::ensure!(
                canonical_archived.parent() == Some(archive_root.as_path()),
                "external workspace archive escaped its managed root: {}",
                archived.display()
            );
            fs::remove_dir_all(&canonical_archived)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    if fs::rename(&managed_root, &archived).is_err() {
        super::fs_copy::copy_dir_recursive(&managed_root, &archived)?;
        fs::remove_dir_all(&managed_root)?;
    }
    Ok("archived")
}

fn ensure_uninstall_owned_directory(parent: &Path, name: &str, label: &str) -> Result<PathBuf> {
    let path = parent.join(name);
    match fs::symlink_metadata(&path) {
        Ok(metadata) => anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "{label} must be a real directory, not a symlink: {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&path)
            .with_context(|| format!("failed to create {label} {}", path.display()))?,
        Err(error) => return Err(error.into()),
    }
    let canonical = fs::canonicalize(&path)?;
    anyhow::ensure!(
        canonical.parent() == Some(parent),
        "{label} escaped its managed parent: {}",
        canonical.display()
    );
    Ok(canonical)
}

pub(super) async fn invoke_package_capability(
    provider: &str,
    capability_id: &str,
    input: Value,
) -> Result<Value> {
    Ok(invoke_capability_from_inproc(CapabilityInvocationRequest {
        handle: None,
        capability_id: Some(capability_id.to_string()),
        caller_package_id: Some(PACKAGE_ID.to_string()),
        provider_package_id: Some(provider.to_string()),
        version: None,
        session_id: None,
        input,
    })
    .await?
    .output)
}

pub(super) async fn compute_manifest_hash(path: &Path) -> Result<String> {
    let output = invoke_package_capability(
        "official/integrity-lab",
        "official/integrity-lab/compute_manifest_hash",
        json!({ "manifest_path": path.to_string_lossy() }),
    )
    .await?;
    Ok(value_str(&output, "sha256")?.to_string())
}

pub(super) fn compute_file_hash(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(&bytes);
    let mut out = String::from("sha256:");
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    Ok(out)
}

fn surface_bundle_path_for_manifest(manifest_path: &Path) -> Result<PathBuf> {
    let manifest = parse_manifest_at(manifest_path)?;
    let PackageEntry::SurfaceBundle { bundle } = &manifest.entry.kind else {
        anyhow::bail!("manifest is not surface_bundle");
    };
    surface_bundle_path_from_manifest(manifest_path, bundle)
}

fn surface_bundle_path_from_manifest(manifest_path: &Path, bundle: &str) -> Result<PathBuf> {
    Ok(manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(safe_relative_path(bundle)?))
}

pub(super) async fn compute_tree_hash(path: &Path) -> Result<String> {
    let output = invoke_package_capability(
        "official/integrity-lab",
        "official/integrity-lab/compute_tree_hash",
        json!({ "dir": path.to_string_lossy() }),
    )
    .await?;
    Ok(value_str(&output, "sha256")?.to_string())
}

pub(super) async fn compute_external_tree_hash(path: &Path) -> Result<String> {
    let output = invoke_package_capability(
        "official/integrity-lab",
        "official/integrity-lab/compute_tree_hash",
        json!({
            "dir": path.to_string_lossy(),
            "profile": "external_workspace_v1",
        }),
    )
    .await?;
    Ok(value_str(&output, "sha256")?.to_string())
}

fn write_profile(
    profile_path: &Path,
    plan: &InstallPlan,
    data_dir_override: Option<&str>,
) -> Result<()> {
    let mut profile = if profile_path.exists() {
        let raw = fs::read_to_string(profile_path)?;
        serde_yaml::from_str::<Value>(&raw).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };
    let mut autoload = profile
        .get("autoload")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let existing = autoload
        .iter()
        .filter_map(Value::as_str)
        .map(portable_profile_path)
        .collect::<std::collections::HashSet<_>>();
    let mut existing = existing;
    for pkg in &plan.packages {
        let package_store = store_path_for_hash(&pkg.tree_hash, data_dir_override)?;
        let manifest = installed_manifest_path(pkg, &package_store)?;
        // Profiles are portable text artifacts. Persist slash-separated paths so
        // the same profile can be copied between Windows and Unix hosts.
        let entry = portable_profile_path(manifest.to_string_lossy().as_ref());
        if !existing.contains(&entry) {
            existing.insert(entry.clone());
            autoload.push(Value::String(entry));
        }
    }
    if !profile.is_object() {
        profile = json!({});
    }
    profile["autoload"] = Value::Array(autoload);
    atomic_write(profile_path, serde_yaml::to_string(&profile)?.as_bytes())
}

fn portable_profile_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn build_lockfile(
    profile: &str,
    plan: &InstallPlan,
    consent: &Consent,
    data_dir_override: Option<&str>,
) -> Result<Lockfile> {
    let mut lock = Lockfile::new(profile, "sha256:profile");
    for pkg in &plan.packages {
        let installed_at_store = store_path_for_hash(&pkg.tree_hash, data_dir_override)?;
        lock.package.push(LockEntry {
            id: pkg.id.clone(),
            version: pkg.version.clone(),
            source: match pkg.source.as_str() {
                "git" => LockSource::Git,
                "local" => LockSource::Local,
                _ => LockSource::Internal,
            },
            url: pkg.url.clone(),
            source_path: (pkg.source == "local").then(|| pkg.path.clone()).flatten(),
            r#ref: pkg.ref_name.clone(),
            commit: pkg.commit_sha.clone(),
            tree_hash: pkg.tree_hash.clone(),
            manifest_hash: pkg.manifest_hash.clone(),
            surface_bundle_hash: pkg.surface_bundle_hash.clone(),
            package_envelope_digest: pkg.package_envelope_digest.clone(),
            component_pins: pkg.component_pins.clone(),
            protocol_profile_pins: pkg.protocol_profile_pins.clone(),
            content_roots: pkg.content_roots.clone(),
            signed: pkg.signed,
            signed_by: pkg.signed_by.clone(),
            installed_at_store: installed_at_store.to_string_lossy().to_string(),
            manifest_relative_path: pkg.manifest_relative_path.clone(),
            granted_capabilities: consent.approved_capabilities.clone(),
            granted_network: consent.approved_network_hosts.clone(),
            granted_secrets: consent.approved_secret_refs.clone(),
            requires: pkg
                .requires
                .iter()
                .map(|req| LockRequirement {
                    id: req.id.clone(),
                    constraint: req.version.clone(),
                    resolved_to: req.id.clone(),
                })
                .collect(),
        });
    }
    Ok(lock)
}
