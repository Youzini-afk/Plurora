use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use plurora_core::{LockEntry, LockRequirement, LockSource, Lockfile, ProjectId, ProjectType};
use serde_json::{json, Value};
use uuid::Uuid;

use super::fs_copy::{copy_dir_recursive, replace_dir_atomic, safe_relative_path};
use super::layout::{
    atomic_write, ensure_layout, lockfile_path, profile_path, store_dir, store_path_for_hash,
};
use super::planner::resolve_plan;
use super::project_kind::read_project_descriptor;
use super::types::{
    Consent, InstallPlan, IntegritySummary, PermissionsSummary, PlannedPackage, PlannedPermissions,
    PlannedRequirement, SignatureSummary, UpdateProjectInput,
};

pub(super) async fn update_project(input: Value, session_id: Option<&str>) -> Result<Value> {
    let input: UpdateProjectInput = serde_json::from_value(input)?;
    let profile = input.profile;
    let data_dir = input.data_dir;
    let project_id = input.project_id;
    let package_id = input.package_id;
    let force = input.force;
    ensure_layout(data_dir.as_deref())?;

    if let Some(project_id) = project_id.as_deref() {
        if let Some(record) = external_project_record_if_exists(project_id, data_dir.as_deref())? {
            return Ok(json!({
                "status": "not_applicable",
                "updated": false,
                "updated_packages": [],
                "reason": record["reason"].clone(),
                "check": { "results": [record] },
            }));
        }
    }

    let lock_path = lockfile_path(&profile, data_dir.as_deref())?;
    if !lock_path.exists() {
        return Ok(json!({
            "status": "current",
            "updated": false,
            "updated_packages": [],
            "check": { "results": [] },
        }));
    }
    let raw_lockfile = fs::read_to_string(&lock_path)
        .with_context(|| format!("failed to read lockfile {}", lock_path.display()))?;
    let lock: Lockfile = toml::from_str(&raw_lockfile)?;
    let candidates = candidate_entries(
        &lock,
        project_id.as_deref(),
        package_id.as_deref(),
        data_dir.as_deref(),
    )?;
    if candidates.is_empty() {
        return Ok(json!({
            "status": "current",
            "updated": false,
            "updated_packages": [],
            "check": { "results": [] },
        }));
    }

    let groups = update_groups(&candidates)?;
    if groups.is_empty() {
        return Ok(json!({
            "status": "current",
            "updated": false,
            "updated_packages": [],
            "check": { "results": check_records(&candidates, &[], force) },
        }));
    }

    let old_ids = lock
        .package
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<BTreeSet<_>>();
    let mut resolved_by_group = Vec::new();
    for group in &groups {
        let resolved = resolve_plan(json!({
            "root_url": group.root_url,
            "root_ref": group.root_ref,
            "lockfile": raw_lockfile,
            "require_signed": false,
            "strict_conformance": false,
        }))
        .await?;
        let plan: InstallPlan = serde_json::from_value(resolved["plan"].clone())?;
        if let (Some(expected_id), Some(descriptor)) =
            (project_id.as_deref(), &plan.project_descriptor)
        {
            if descriptor.project.id.as_str() != expected_id {
                anyhow::bail!(
                    "updated project descriptor id drifted from {expected_id} to {}",
                    descriptor.project.id.as_str()
                );
            }
        }
        if let Some(new_pkg) = plan.packages.iter().find(|pkg| !old_ids.contains(&pkg.id)) {
            anyhow::bail!(
                "update introduces new package {}; reinstall or re-consent is required",
                new_pkg.id
            );
        }
        resolved_by_group.push((group.clone(), plan));
    }

    let changed_ids = changed_package_ids(&candidates, &resolved_by_group, force);
    let check = json!({ "results": check_records(&candidates, &resolved_by_group, force) });
    if changed_ids.is_empty() {
        return Ok(json!({
            "status": "current",
            "updated": false,
            "updated_packages": [],
            "check": check,
        }));
    }

    let remove_ids = groups
        .iter()
        .flat_map(|group| group.package_ids.intersection(&changed_ids).cloned())
        .collect::<BTreeSet<_>>();
    let old_manifest_paths = lock
        .package
        .iter()
        .filter(|entry| remove_ids.contains(&entry.id))
        .flat_map(manifest_candidates_for_lock_entry)
        .collect::<BTreeSet<_>>();
    let mut merged = lock
        .package
        .iter()
        .filter(|entry| !remove_ids.contains(&entry.id))
        .map(planned_from_lock_entry)
        .collect::<Result<Vec<_>>>()?;
    let mut updated_ids = BTreeSet::new();
    let mut project_descriptor = None;
    for (_group, plan) in resolved_by_group {
        if project_descriptor.is_none() {
            project_descriptor = plan.project_descriptor.clone();
        }
        for pkg in plan.packages {
            if !changed_ids.contains(&pkg.id) {
                continue;
            }
            updated_ids.insert(pkg.id.clone());
            merged.retain(|existing| existing.id != pkg.id);
            merged.push(pkg);
        }
    }

    let full_plan = merged_plan(&lock, merged, project_descriptor);
    let old_grants = grant_map_from_lockfile(&lock);
    enforce_permission_drift(&full_plan, &old_grants, &remove_ids)?;
    let consent = consent_for_plan(&full_plan, &old_grants, &remove_ids)?;
    let new_manifest_paths = manifest_paths_for_plan(&full_plan, data_dir.as_deref())?;
    let new_store_paths = new_store_paths_for_plan(&full_plan, data_dir.as_deref())?;
    let snapshot = MutationSnapshot::capture(&profile, data_dir.as_deref(), &full_plan)?;

    let executed = super::executor::execute_plan(
        json!({
            "plan": full_plan,
            "consent": consent,
            "profile": profile,
            "data_dir": data_dir,
        }),
        session_id,
    )
    .await;

    let executed = match executed {
        Ok(value) => value,
        Err(error) => {
            cleanup_failed_update_paths(data_dir.as_deref(), &new_store_paths);
            return Err(restore_after_failure(error, &snapshot));
        }
    };

    if let Err(error) = remove_stale_profile_entries(
        &profile,
        data_dir.as_deref(),
        &old_manifest_paths,
        &new_manifest_paths,
    ) {
        cleanup_failed_update_paths(data_dir.as_deref(), &new_store_paths);
        return Err(restore_after_failure(error, &snapshot));
    }
    if let Err(error) =
        rewrite_lockfile_per_package_grants(&profile, data_dir.as_deref(), &old_grants, &remove_ids)
    {
        cleanup_failed_update_paths(data_dir.as_deref(), &new_store_paths);
        return Err(restore_after_failure(error, &snapshot));
    }

    snapshot.cleanup();

    let store_gc = match super::gc::prune_orphaned_stores(data_dir.as_deref()) {
        Ok(report) => json!({ "ok": true, "report": store_gc_json(report) }),
        Err(error) => json!({ "ok": false, "warning": error.to_string() }),
    };

    Ok(json!({
        "status": "updated",
        "updated": true,
        "updated_packages": updated_ids.into_iter().collect::<Vec<_>>(),
        "check": check,
        "execute": executed,
        "store_gc": store_gc,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct UpdateGroup {
    root_url: String,
    root_ref: String,
    package_ids: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct CandidateEntry {
    entry: LockEntry,
    project_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct PackageGrants {
    capabilities: Vec<String>,
    network: Vec<String>,
    secrets: Vec<String>,
}

fn candidate_entries(
    lock: &Lockfile,
    project_id: Option<&str>,
    package_id: Option<&str>,
    data_dir_override: Option<&str>,
) -> Result<Vec<CandidateEntry>> {
    let mut manifest_relatives = Vec::new();
    let project_id = if let Some(project_id) = project_id {
        let project_id = ProjectId::new(project_id)?;
        let descriptor_path = project_dir(data_dir_override, &project_id).join("project.yaml");
        let descriptor = read_project_descriptor(&descriptor_path)?;
        if matches!(
            descriptor.project.project_type,
            ProjectType::ExternalWrapped | ProjectType::ExternalWorkspace
        ) {
            return Ok(Vec::new());
        }
        manifest_relatives = descriptor.project.packages.clone();
        manifest_relatives.sort();
        manifest_relatives.dedup();
        Some(project_id.as_str().to_string())
    } else {
        None
    };

    Ok(lock
        .package
        .iter()
        .filter(|entry| package_id.is_none_or(|package_id| package_id == entry.id))
        .filter(|entry| {
            manifest_relatives.is_empty()
                || entry
                    .manifest_relative_path
                    .as_ref()
                    .is_some_and(|relative| {
                        manifest_relatives
                            .iter()
                            .any(|candidate| candidate == relative)
                    })
        })
        .cloned()
        .map(|entry| CandidateEntry {
            entry,
            project_id: project_id.clone(),
        })
        .collect())
}

fn update_groups(candidates: &[CandidateEntry]) -> Result<Vec<UpdateGroup>> {
    let mut groups = BTreeMap::<(String, String), BTreeSet<String>>::new();
    for candidate in candidates {
        let entry = &candidate.entry;
        let (root_url, root_ref) = match entry.source {
            LockSource::Local => {
                let path = entry
                    .source_path
                    .clone()
                    .context("local update requires source_path in lockfile")?;
                if !Path::new(&path).is_absolute() {
                    continue;
                }
                (path, String::new())
            }
            LockSource::Git => (
                entry
                    .url
                    .clone()
                    .context("git update requires url in lockfile")?,
                entry.r#ref.clone().unwrap_or_else(|| "HEAD".to_string()),
            ),
            LockSource::Internal => continue,
        };
        groups
            .entry((root_url, root_ref))
            .or_default()
            .insert(entry.id.clone());
    }
    Ok(groups
        .into_iter()
        .map(|((root_url, root_ref), package_ids)| UpdateGroup {
            root_url,
            root_ref,
            package_ids,
        })
        .collect())
}

fn changed_package_ids(
    candidates: &[CandidateEntry],
    resolved_by_group: &[(UpdateGroup, InstallPlan)],
    force: bool,
) -> BTreeSet<String> {
    let mut changed = BTreeSet::new();
    for candidate in candidates {
        let entry = &candidate.entry;
        if let Some(planned) = resolved_package_for(entry, resolved_by_group) {
            let dangling = !Path::new(&entry.installed_at_store).is_dir();
            if force
                || dangling
                || entry.tree_hash != planned.tree_hash
                || entry.manifest_hash != planned.manifest_hash
                || entry.commit != planned.commit_sha
                || entry.surface_bundle_hash != planned.surface_bundle_hash
                || entry.package_envelope_digest != planned.package_envelope_digest
                || entry.component_pins != planned.component_pins
                || entry.protocol_profile_pins != planned.protocol_profile_pins
                || entry.content_roots != planned.content_roots
            {
                changed.insert(entry.id.clone());
            }
        }
    }
    changed
}

fn check_records(
    candidates: &[CandidateEntry],
    resolved_by_group: &[(UpdateGroup, InstallPlan)],
    force: bool,
) -> Vec<Value> {
    let changed = changed_package_ids(candidates, resolved_by_group, force);
    candidates
        .iter()
        .map(|candidate| {
            let entry = &candidate.entry;
            let source_kind = source_kind(&entry.source);
            let applicable = matches!(entry.source, LockSource::Git | LockSource::Local);
            let dangling = !Path::new(&entry.installed_at_store).is_dir();
            let planned = resolved_package_for(entry, resolved_by_group);
            let available = changed.contains(&entry.id);
            let status = if !applicable {
                "not_applicable"
            } else if dangling && available {
                "repair_required"
            } else if available {
                "update_available"
            } else {
                "current"
            };
            let reason = if !applicable {
                "internal source is managed by the host".to_string()
            } else if dangling {
                "installed store path is missing".to_string()
            } else if available {
                "resolved package differs from lockfile".to_string()
            } else {
                "resolved package matches lockfile".to_string()
            };
            json!({
                "id": entry.id,
                "package_id": entry.id,
                "project_id": candidate.project_id,
                "source_kind": source_kind,
                "applicable": applicable,
                "status": status,
                "reason": reason,
                "available": available,
                "dangling": dangling,
                "current_commit": entry.commit,
                "upstream_commit": planned.and_then(|pkg| pkg.commit_sha.clone()),
                "current_tree_hash": entry.tree_hash,
                "available_tree_hash": planned.map(|pkg| pkg.tree_hash.clone()),
                "installed_at_store": entry.installed_at_store,
            })
        })
        .collect()
}

fn resolved_package_for<'a>(
    entry: &LockEntry,
    resolved_by_group: &'a [(UpdateGroup, InstallPlan)],
) -> Option<&'a PlannedPackage> {
    resolved_by_group.iter().find_map(|(group, plan)| {
        group
            .package_ids
            .contains(&entry.id)
            .then(|| plan.packages.iter().find(|pkg| pkg.id == entry.id))
            .flatten()
    })
}

fn planned_from_lock_entry(entry: &LockEntry) -> Result<PlannedPackage> {
    Ok(PlannedPackage {
        id: entry.id.clone(),
        version: entry.version.clone(),
        source: source_kind(&entry.source).to_string(),
        url: entry.url.clone(),
        ref_name: entry.r#ref.clone(),
        path: entry.source_path.clone(),
        commit_sha: entry.commit.clone(),
        manifest_hash: entry.manifest_hash.clone(),
        tree_hash: entry.tree_hash.clone(),
        surface_bundle_hash: entry.surface_bundle_hash.clone(),
        package_envelope_digest: entry.package_envelope_digest.clone(),
        component_pins: entry.component_pins.clone(),
        protocol_profile_pins: entry.protocol_profile_pins.clone(),
        content_roots: entry.content_roots.clone(),
        manifest_relative_path: entry.manifest_relative_path.clone(),
        signed: entry.signed,
        signed_by: entry.signed_by.clone(),
        permissions: PlannedPermissions {
            capabilities_invoke: entry.granted_capabilities.clone(),
            network_hosts: entry.granted_network.clone(),
            secret_refs: entry.granted_secrets.clone(),
        },
        requires: entry
            .requires
            .iter()
            .map(planned_requirement_from_lock)
            .collect(),
        conformance: None,
    })
}

fn planned_requirement_from_lock(req: &LockRequirement) -> PlannedRequirement {
    PlannedRequirement {
        id: req.id.clone(),
        source: Value::Null,
        version: req.constraint.clone(),
    }
}

fn merged_plan(
    lock: &Lockfile,
    packages: Vec<PlannedPackage>,
    project_descriptor: Option<plurora_core::ProjectDescriptor>,
) -> InstallPlan {
    let permissions_summary = permissions_summary(&packages);
    let unsigned_packages = packages
        .iter()
        .filter(|pkg| !pkg.signed)
        .map(|pkg| pkg.id.clone())
        .collect::<Vec<_>>();
    InstallPlan {
        root_id: packages
            .first()
            .map(|pkg| pkg.id.clone())
            .or_else(|| lock.package.first().map(|entry| entry.id.clone()))
            .unwrap_or_default(),
        packages,
        project_descriptor,
        permissions_summary,
        signature_summary: SignatureSummary {
            all_signed: unsigned_packages.is_empty(),
            unsigned_packages,
        },
        integrity_summary: IntegritySummary {
            manifest_hashes_match_lockfile: true,
            drift_detected: Vec::new(),
        },
    }
}

fn permissions_summary(packages: &[PlannedPackage]) -> PermissionsSummary {
    PermissionsSummary {
        new_capabilities: sorted_unique(
            packages
                .iter()
                .flat_map(|pkg| pkg.permissions.capabilities_invoke.iter().cloned()),
        ),
        new_network_hosts: sorted_unique(
            packages
                .iter()
                .flat_map(|pkg| pkg.permissions.network_hosts.iter().cloned()),
        ),
        new_secret_refs: sorted_unique(
            packages
                .iter()
                .flat_map(|pkg| pkg.permissions.secret_refs.iter().cloned()),
        ),
    }
}

fn grant_map_from_lockfile(lock: &Lockfile) -> HashMap<String, PackageGrants> {
    lock.package
        .iter()
        .map(|entry| {
            (
                entry.id.clone(),
                PackageGrants {
                    capabilities: entry.granted_capabilities.clone(),
                    network: entry.granted_network.clone(),
                    secrets: entry.granted_secrets.clone(),
                },
            )
        })
        .collect()
}

fn enforce_permission_drift(
    plan: &InstallPlan,
    old_grants: &HashMap<String, PackageGrants>,
    updated_existing: &BTreeSet<String>,
) -> Result<()> {
    for pkg in plan
        .packages
        .iter()
        .filter(|pkg| updated_existing.contains(&pkg.id))
    {
        let old = old_grants
            .get(&pkg.id)
            .with_context(|| format!("updated package {} is missing previous grants", pkg.id))?;
        ensure_subset(
            &pkg.permissions.capabilities_invoke,
            &old.capabilities,
            &pkg.id,
            "capability",
        )?;
        ensure_subset(
            &pkg.permissions.network_hosts,
            &old.network,
            &pkg.id,
            "network host",
        )?;
        ensure_subset(
            &pkg.permissions.secret_refs,
            &old.secrets,
            &pkg.id,
            "secret ref",
        )?;
    }
    Ok(())
}

fn ensure_subset(
    required: &[String],
    granted: &[String],
    package_id: &str,
    kind: &str,
) -> Result<()> {
    for item in required {
        if !granted.iter().any(|granted| granted == item) {
            anyhow::bail!(
                "update for {package_id} requests new {kind} not previously granted: {item}"
            );
        }
    }
    Ok(())
}

fn consent_for_plan(
    plan: &InstallPlan,
    old_grants: &HashMap<String, PackageGrants>,
    updated_existing: &BTreeSet<String>,
) -> Result<Consent> {
    let mut capabilities = BTreeSet::new();
    let mut network = BTreeSet::new();
    let mut secrets = BTreeSet::new();
    for pkg in &plan.packages {
        if let Some(old) = old_grants.get(&pkg.id) {
            capabilities.extend(old.capabilities.iter().cloned());
            network.extend(old.network.iter().cloned());
            secrets.extend(old.secrets.iter().cloned());
        } else if updated_existing.contains(&pkg.id) {
            anyhow::bail!("updated package {} has no previous grants", pkg.id);
        } else {
            capabilities.extend(pkg.permissions.capabilities_invoke.iter().cloned());
            network.extend(pkg.permissions.network_hosts.iter().cloned());
            secrets.extend(pkg.permissions.secret_refs.iter().cloned());
        }
    }
    Ok(Consent {
        approved_capabilities: capabilities.into_iter().collect(),
        approved_network_hosts: network.into_iter().collect(),
        approved_secret_refs: secrets.into_iter().collect(),
    })
}

fn rewrite_lockfile_per_package_grants(
    profile: &str,
    data_dir_override: Option<&str>,
    old_grants: &HashMap<String, PackageGrants>,
    updated_existing: &BTreeSet<String>,
) -> Result<()> {
    let path = lockfile_path(profile, data_dir_override)?;
    let mut lock: Lockfile = toml::from_str(&fs::read_to_string(&path)?)?;
    for entry in &mut lock.package {
        if let Some(old) = old_grants.get(&entry.id) {
            entry.granted_capabilities = old.capabilities.clone();
            entry.granted_network = old.network.clone();
            entry.granted_secrets = old.secrets.clone();
        } else if !updated_existing.contains(&entry.id) {
            entry.granted_capabilities = entry.granted_capabilities.clone();
            entry.granted_network = entry.granted_network.clone();
            entry.granted_secrets = entry.granted_secrets.clone();
        }
    }
    atomic_write(&path, toml::to_string_pretty(&lock)?.as_bytes())
}

fn sorted_unique(values: impl Iterator<Item = String>) -> Vec<String> {
    values.collect::<BTreeSet<_>>().into_iter().collect()
}

fn manifest_paths_for_plan(
    plan: &InstallPlan,
    data_dir_override: Option<&str>,
) -> Result<BTreeSet<String>> {
    let mut paths = BTreeSet::new();
    for pkg in &plan.packages {
        let store = store_path_for_hash(&pkg.tree_hash, data_dir_override)?;
        let manifest = if let Some(relative) = &pkg.manifest_relative_path {
            store.join(safe_relative_path(relative)?)
        } else {
            store.join("manifest.yaml")
        };
        paths.insert(manifest.to_string_lossy().to_string());
    }
    Ok(paths)
}

fn new_store_paths_for_plan(
    plan: &InstallPlan,
    data_dir_override: Option<&str>,
) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut seen = BTreeSet::new();
    for pkg in &plan.packages {
        let path = store_path_for_hash(&pkg.tree_hash, data_dir_override)?;
        if seen.insert(path.clone()) && !path.exists() {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn cleanup_failed_update_paths(data_dir_override: Option<&str>, paths: &[PathBuf]) {
    for path in paths {
        if path.exists() {
            fs::remove_dir_all(path).ok();
        }
    }
    if let Ok(staging) = store_dir(data_dir_override).map(|store| store.join(".staging")) {
        if staging.is_dir() && !staging.is_symlink() {
            fs::remove_dir_all(&staging).ok();
        }
    }
}

fn remove_stale_profile_entries(
    profile: &str,
    data_dir_override: Option<&str>,
    old_paths: &BTreeSet<String>,
    new_paths: &BTreeSet<String>,
) -> Result<()> {
    let stale = old_paths.difference(new_paths).collect::<BTreeSet<_>>();
    if stale.is_empty() {
        return Ok(());
    }
    let path = profile_path(profile, data_dir_override)?;
    if !path.exists() {
        return Ok(());
    }
    let raw = fs::read_to_string(&path)?;
    let mut value: Value = serde_yaml::from_str(&raw).unwrap_or_else(|_| json!({}));
    if let Some(autoload) = value.get_mut("autoload").and_then(Value::as_array_mut) {
        autoload.retain(|entry| {
            !entry
                .as_str()
                .is_some_and(|path| stale.iter().any(|stale| stale.as_str() == path))
        });
    }
    atomic_write(&path, serde_yaml::to_string(&value)?.as_bytes())
}

#[derive(Debug)]
struct MutationSnapshot {
    files: Vec<FileSnapshot>,
    dirs: Vec<DirSnapshot>,
    projects: Vec<ProjectRegistrySnapshot>,
}

#[derive(Debug)]
struct FileSnapshot {
    path: PathBuf,
    bytes: Option<Vec<u8>>,
}

#[derive(Debug)]
struct DirSnapshot {
    path: PathBuf,
    backup: Option<PathBuf>,
}

#[derive(Debug)]
struct ProjectRegistrySnapshot {
    id: ProjectId,
    entry: Option<crate::ProjectEntry>,
}

impl MutationSnapshot {
    fn capture(profile: &str, data_dir_override: Option<&str>, plan: &InstallPlan) -> Result<Self> {
        let mut files = vec![
            snapshot_file(lockfile_path(profile, data_dir_override)?)?,
            snapshot_file(profile_path(profile, data_dir_override)?)?,
        ];
        let mut dirs = Vec::new();
        let mut projects = Vec::new();
        if let Some(descriptor) = &plan.project_descriptor {
            let dir = project_dir(data_dir_override, &descriptor.project.id);
            let project_yaml = dir.join("project.yaml");
            let registry_entry = crate::inproc::project_registry_from_inproc()
                .ok()
                .and_then(|registry| registry.get(&descriptor.project.id));
            files.push(snapshot_file(project_yaml)?);
            dirs.push(snapshot_dir(dir.join("dist"))?);
            projects.push(ProjectRegistrySnapshot {
                id: descriptor.project.id.clone(),
                entry: registry_entry,
            });
        }
        Ok(Self {
            files,
            dirs,
            projects,
        })
    }

    fn restore(&self) -> Result<()> {
        let mut errors = Vec::new();
        for dir in &self.dirs {
            if let Err(error) = restore_dir(dir) {
                errors.push(format!(
                    "restore dir {} failed: {error}",
                    dir.path.display()
                ));
            }
        }
        for file in &self.files {
            if let Err(error) = restore_file(file) {
                errors.push(format!(
                    "restore file {} failed: {error}",
                    file.path.display()
                ));
            }
        }
        match crate::inproc::project_registry_from_inproc() {
            Ok(registry) => {
                for project in &self.projects {
                    if let Some(entry) = &project.entry {
                        if let Err(error) = registry.register(entry.descriptor.clone()) {
                            errors.push(format!(
                                "restore registry {} failed: {error}",
                                project.id.as_str()
                            ));
                            continue;
                        }
                        if let Err(error) = registry.set_state(&project.id, entry.state.clone()) {
                            errors.push(format!(
                                "restore registry state {} failed: {error}",
                                project.id.as_str()
                            ));
                        }
                    } else {
                        registry.unregister(&project.id);
                    }
                }
            }
            Err(error) => errors.push(format!("restore registry unavailable: {error}")),
        }
        if errors.is_empty() {
            Ok(())
        } else {
            anyhow::bail!(errors.join("; "))
        }
    }

    fn cleanup(&self) {
        for dir in &self.dirs {
            if let Some(backup) = &dir.backup {
                fs::remove_dir_all(backup).ok();
            }
        }
    }
}

fn restore_after_failure(error: anyhow::Error, snapshot: &MutationSnapshot) -> anyhow::Error {
    match snapshot.restore() {
        Ok(()) => error,
        Err(rollback_error) => {
            anyhow::anyhow!("{error}; rollback failed: {rollback_error}")
        }
    }
}

fn snapshot_file(path: PathBuf) -> Result<FileSnapshot> {
    let bytes = match fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    Ok(FileSnapshot { path, bytes })
}

fn restore_file(snapshot: &FileSnapshot) -> Result<()> {
    if let Some(bytes) = &snapshot.bytes {
        atomic_write(&snapshot.path, bytes)?;
    } else if snapshot.path.exists() {
        fs::remove_file(&snapshot.path)?;
    }
    Ok(())
}

fn snapshot_dir(path: PathBuf) -> Result<DirSnapshot> {
    if !path.exists() {
        return Ok(DirSnapshot { path, backup: None });
    }
    let backup = std::env::temp_dir().join(format!("plurora-update-snapshot-{}", Uuid::new_v4()));
    copy_dir_recursive(&path, &backup)?;
    Ok(DirSnapshot {
        path,
        backup: Some(backup),
    })
}

fn restore_dir(snapshot: &DirSnapshot) -> Result<()> {
    match &snapshot.backup {
        Some(backup) => replace_dir_atomic(backup, &snapshot.path),
        None => {
            if snapshot.path.exists() {
                fs::remove_dir_all(&snapshot.path)?;
            }
            Ok(())
        }
    }
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

fn external_project_record_if_exists(
    project_id: &str,
    data_dir_override: Option<&str>,
) -> Result<Option<Value>> {
    let project_id = ProjectId::new(project_id)?;
    let descriptor_path = project_dir(data_dir_override, &project_id).join("project.yaml");
    if !descriptor_path.is_file() {
        return Ok(None);
    }
    let descriptor = read_project_descriptor(&descriptor_path)?;
    Ok(matches!(
        descriptor.project.project_type,
        ProjectType::ExternalWrapped | ProjectType::ExternalWorkspace
    )
    .then(|| {
        json!({
            "id": project_id.as_str(),
            "package_id": project_id.as_str(),
            "project_id": project_id.as_str(),
            "source_kind": project_type_source_kind(&descriptor.project.project_type),
            "applicable": false,
            "status": "not_applicable",
            "reason": "external project updates are not handled by install-lab update_project; adapter-package updates are deferred",
            "available": false,
            "dangling": false,
        })
    }))
}

fn project_type_source_kind(project_type: &ProjectType) -> &'static str {
    match project_type {
        ProjectType::PluroraNative => "project",
        ProjectType::ExternalWrapped => "external_wrapped",
        ProjectType::ExternalWorkspace => "external_workspace",
    }
}

fn project_dir(data_dir_override: Option<&str>, id: &ProjectId) -> PathBuf {
    if let Some(dir) = data_dir_override {
        PathBuf::from(dir).join("projects").join(id.as_str())
    } else {
        plurora_core::paths::project_dir(id)
            .unwrap_or_else(|_| PathBuf::from("projects").join(id.as_str()))
    }
}

fn source_kind(source: &LockSource) -> &'static str {
    match source {
        LockSource::Internal => "internal",
        LockSource::Git => "git",
        LockSource::Local => "local",
    }
}

fn store_gc_json(report: super::gc::StoreGcReport) -> Value {
    json!({
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
