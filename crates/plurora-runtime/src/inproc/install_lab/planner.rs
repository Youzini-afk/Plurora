use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use plurora_core::{
    DependencySource, LockEntry, Lockfile, PackageDependency, PackageManifest, PermissionSet,
};
use serde_json::{json, Value};
use uuid::Uuid;

use super::executor::{
    compute_file_hash, compute_manifest_hash, compute_tree_hash, invoke_package_capability,
};
use super::source::{
    block_on_current, manifest_path_in, parse_manifest_at, parse_project_descriptor_at,
    parse_root_descriptor, resolve_dep, sorted_vec, value_str,
};
use super::types::{
    Consent, InstallPlan, IntegritySummary, PackageDescriptor, PermissionsSummary, PlannedPackage,
    PlannedPermissions, PlannedRequirement, ResolvePlanInput, ResolvedPackages, SignatureSummary,
    SourceDescriptor,
};

const MAX_DEPTH: usize = 32;

pub(super) async fn resolve_plan(input: Value) -> Result<Value> {
    if input.as_object().is_some_and(|object| object.is_empty()) {
        anyhow::bail!("unsupported smoke input: missing root_url");
    }
    let input: ResolvePlanInput = serde_json::from_value(input)?;
    let root = parse_root_descriptor(&input.root_url, &input.root_ref)?;
    let mut packages = Vec::new();
    let mut project_descriptor = None;
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    let mut resolving = HashSet::new();
    match &root.source {
        SourceDescriptor::Local { path } => {
            let canonical = fs::canonicalize(path).with_context(|| {
                format!("failed to canonicalize local package {}", path.display())
            })?;
            let project_yaml = canonical.join("project.yaml");
            if project_yaml.is_file() {
                let descriptor = parse_project_descriptor_at(&project_yaml)?;
                project_descriptor = Some(descriptor.clone());
                resolve_project_packages(
                    &canonical,
                    &descriptor,
                    ProjectPackageSource::Local {
                        path: project_root_path(&canonical),
                    },
                    input.strict_conformance,
                    &mut visited,
                    &mut resolving,
                    &mut stack,
                    &mut packages,
                )?;
            } else {
                resolve_transitive(
                    root,
                    input.require_signed,
                    input.strict_conformance,
                    &mut project_descriptor,
                    &mut visited,
                    &mut resolving,
                    &mut stack,
                    &mut packages,
                    MAX_DEPTH,
                )?;
            }
        }
        _ => resolve_transitive(
            root,
            input.require_signed,
            input.strict_conformance,
            &mut project_descriptor,
            &mut visited,
            &mut resolving,
            &mut stack,
            &mut packages,
            MAX_DEPTH,
        )?,
    }

    let mut lock_map = HashMap::<String, LockEntry>::new();
    if let Some(lockfile) = input.lockfile.as_deref() {
        let lock: Lockfile = toml::from_str(lockfile).context("failed to parse lockfile TOML")?;
        lock.validate().context("invalid lockfile")?;
        for entry in lock.package {
            lock_map.insert(entry.id.clone(), entry);
        }
    }
    let mut drift = Vec::new();
    for pkg in &packages {
        if let Some(expected) = lock_map.get(&pkg.id) {
            if expected.manifest_hash != pkg.manifest_hash {
                drift.push(json!({
                    "id": pkg.id,
                    "kind": "manifest_hash",
                    "expected": expected.manifest_hash,
                    "actual": pkg.manifest_hash,
                }));
            }
            if expected.package_envelope_digest.is_some()
                && expected.package_envelope_digest != pkg.package_envelope_digest
            {
                drift.push(json!({
                    "id": pkg.id,
                    "kind": "package_envelope_digest",
                    "expected": expected.package_envelope_digest,
                    "actual": pkg.package_envelope_digest,
                }));
            }
            if !expected.component_pins.is_empty() && expected.component_pins != pkg.component_pins
            {
                drift.push(json!({
                    "id": pkg.id,
                    "kind": "component_pins",
                    "expected": expected.component_pins,
                    "actual": pkg.component_pins,
                }));
            }
            if !expected.protocol_profile_pins.is_empty()
                && expected.protocol_profile_pins != pkg.protocol_profile_pins
            {
                drift.push(json!({
                    "id": pkg.id,
                    "kind": "protocol_profile_pins",
                    "expected": expected.protocol_profile_pins,
                    "actual": pkg.protocol_profile_pins,
                }));
            }
            if !expected.content_roots.is_empty() && expected.content_roots != pkg.content_roots {
                drift.push(json!({
                    "id": pkg.id,
                    "kind": "content_roots",
                    "expected": expected.content_roots,
                    "actual": pkg.content_roots,
                }));
            }
        }
    }

    let permissions_summary = aggregate_permissions(&packages);
    let unsigned_packages = packages
        .iter()
        .filter(|pkg| !pkg.signed)
        .map(|pkg| pkg.id.clone())
        .collect::<Vec<_>>();
    let root_id = packages
        .first()
        .map(|pkg| pkg.id.clone())
        .unwrap_or_default();
    let plan = InstallPlan {
        root_id,
        packages,
        project_descriptor,
        permissions_summary,
        signature_summary: SignatureSummary {
            all_signed: unsigned_packages.is_empty(),
            unsigned_packages,
        },
        integrity_summary: IntegritySummary {
            manifest_hashes_match_lockfile: drift.is_empty(),
            drift_detected: drift,
        },
    };
    Ok(json!({ "plan": plan }))
}

fn resolve_project_packages(
    project_root: &Path,
    descriptor: &plurora_core::ProjectDescriptor,
    source: ProjectPackageSource,
    strict_conformance: bool,
    visited: &mut HashSet<String>,
    resolving: &mut HashSet<String>,
    stack: &mut Vec<String>,
    plan: &mut Vec<PlannedPackage>,
) -> Result<()> {
    let tree_hash = block_on_current(compute_tree_hash(project_root))?;
    let mut ignored_project_descriptor = None;
    for manifest_ref in &descriptor.project.packages {
        let relative = normalize_relative_manifest_path(manifest_ref)?;
        let manifest_path = project_root.join(&relative);
        if !manifest_path.is_file() {
            anyhow::bail!(
                "project package manifest does not exist: {}",
                manifest_path.display()
            );
        }
        let manifest = parse_manifest_at(&manifest_path)?;
        let manifest_hash = block_on_current(compute_manifest_hash(&manifest_path))?;
        let package_root = manifest_path.parent().unwrap_or(project_root);
        let mut planned = planned_from_manifest(
            &manifest,
            package_root,
            source.source_kind().to_string(),
            source.url().map(str::to_string),
            source.ref_name().map(str::to_string),
            source.path().map(str::to_string),
            source.commit_sha().map(str::to_string),
            manifest_hash,
            tree_hash.clone(),
            Some(relative),
            source.signed(),
            None,
        )?;
        attach_conformance(&mut planned, package_root, strict_conformance)?;
        push_resolved_package(
            planned,
            false,
            strict_conformance,
            &mut ignored_project_descriptor,
            visited,
            resolving,
            stack,
            plan,
            MAX_DEPTH,
        )?;
    }
    Ok(())
}

#[derive(Clone)]
enum ProjectPackageSource {
    Local {
        path: String,
    },
    Git {
        url: String,
        ref_name: String,
        commit_sha: String,
        signed: bool,
    },
}

impl ProjectPackageSource {
    fn source_kind(&self) -> &'static str {
        match self {
            Self::Local { .. } => "local",
            Self::Git { .. } => "git",
        }
    }

    fn url(&self) -> Option<&str> {
        match self {
            Self::Local { .. } => None,
            Self::Git { url, .. } => Some(url),
        }
    }

    fn ref_name(&self) -> Option<&str> {
        match self {
            Self::Local { .. } => None,
            Self::Git { ref_name, .. } => Some(ref_name),
        }
    }

    fn path(&self) -> Option<&str> {
        match self {
            Self::Local { path } => Some(path),
            Self::Git { .. } => None,
        }
    }

    fn commit_sha(&self) -> Option<&str> {
        match self {
            Self::Local { .. } => None,
            Self::Git { commit_sha, .. } => Some(commit_sha),
        }
    }

    fn signed(&self) -> bool {
        match self {
            Self::Local { .. } => false,
            Self::Git { signed, .. } => *signed,
        }
    }
}

fn project_root_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn resolve_transitive(
    root: PackageDescriptor,
    require_signed: bool,
    strict_conformance: bool,
    project_descriptor: &mut Option<plurora_core::ProjectDescriptor>,
    visited: &mut HashSet<String>,
    resolving: &mut HashSet<String>,
    stack: &mut Vec<String>,
    plan: &mut Vec<PlannedPackage>,
    max_depth: usize,
) -> Result<()> {
    if max_depth == 0 {
        anyhow::bail!("dependency depth exceeded {MAX_DEPTH} (possible cycle)");
    }

    match &root.source {
        SourceDescriptor::Internal => return Ok(()),
        SourceDescriptor::Local { path } => {
            if !path.exists() {
                anyhow::bail!("local dependency path does not exist: {}", path.display());
            }
        }
        SourceDescriptor::Git { .. } => {}
    }

    let resolved = resolve_one(root, require_signed, strict_conformance)?;
    if project_descriptor.is_none() {
        *project_descriptor = resolved.project_descriptor;
    }
    for resolved in resolved.packages {
        push_resolved_package(
            resolved,
            require_signed,
            strict_conformance,
            project_descriptor,
            visited,
            resolving,
            stack,
            plan,
            max_depth,
        )?;
    }
    Ok(())
}

fn push_resolved_package(
    resolved: PlannedPackage,
    require_signed: bool,
    strict_conformance: bool,
    project_descriptor: &mut Option<plurora_core::ProjectDescriptor>,
    visited: &mut HashSet<String>,
    resolving: &mut HashSet<String>,
    stack: &mut Vec<String>,
    plan: &mut Vec<PlannedPackage>,
    max_depth: usize,
) -> Result<()> {
    if let Some(pos) = stack.iter().position(|id| id == &resolved.id) {
        let mut cycle = stack[pos..].to_vec();
        cycle.push(resolved.id.clone());
        anyhow::bail!("dependency cycle detected: {}", cycle.join(" -> "));
    }
    if visited.contains(&resolved.id) {
        return Ok(());
    }
    if !resolving.insert(resolved.id.clone()) {
        anyhow::bail!("dependency cycle detected at {}", resolved.id);
    }
    visited.insert(resolved.id.clone());
    stack.push(resolved.id.clone());

    let requires = resolved.requires.clone();
    plan.push(resolved);
    for req in requires {
        let next = resolve_dep(&req)?;
        resolve_transitive(
            next,
            require_signed,
            strict_conformance,
            project_descriptor,
            visited,
            resolving,
            stack,
            plan,
            max_depth - 1,
        )?;
    }
    let done = stack.pop();
    if let Some(done) = done {
        resolving.remove(&done);
    }
    Ok(())
}

fn resolve_one(
    desc: PackageDescriptor,
    require_signed: bool,
    strict_conformance: bool,
) -> Result<ResolvedPackages> {
    match desc.source {
        SourceDescriptor::Local { path } => Ok(ResolvedPackages {
            packages: vec![resolve_local_package(path, strict_conformance)?],
            project_descriptor: None,
        }),
        SourceDescriptor::Git { url, ref_name } => block_on_current(resolve_git_package(
            url,
            ref_name,
            require_signed,
            strict_conformance,
        )),
        SourceDescriptor::Internal => {
            anyhow::bail!("internal packages do not require installation")
        }
    }
}

fn resolve_local_package(path: PathBuf, strict_conformance: bool) -> Result<PlannedPackage> {
    let path = fs::canonicalize(&path)
        .with_context(|| format!("failed to canonicalize local package {}", path.display()))?;
    let manifest_path = manifest_path_in(&path)?;
    let manifest = parse_manifest_at(&manifest_path)?;
    let manifest_hash = block_on_current(compute_manifest_hash(&manifest_path))?;
    let tree_hash = block_on_current(compute_tree_hash(&path))?;
    let mut planned = planned_from_manifest(
        &manifest,
        manifest_path.parent().unwrap_or(&path),
        "local".to_string(),
        None,
        None,
        Some(path.to_string_lossy().to_string()),
        None,
        manifest_hash,
        tree_hash,
        None,
        false,
        None,
    )?;
    attach_conformance(&mut planned, &path, strict_conformance)?;
    Ok(planned)
}

async fn resolve_git_package(
    url: String,
    ref_name: String,
    require_signed: bool,
    strict_conformance: bool,
) -> Result<ResolvedPackages> {
    let resolved = invoke_package_capability(
        "official/git-tools-lab",
        "official/git-tools-lab/resolve_ref",
        json!({ "remote_url": url, "ref": ref_name }),
    )
    .await?;
    let commit_sha = value_str(&resolved, "commit_sha")?.to_string();
    let resolved_ref_name = value_str(&resolved, "ref_name")?.to_string();
    let tmp = std::env::temp_dir().join(format!("plurora-git-install-{}", Uuid::new_v4()));
    let fetch = invoke_package_capability(
        "official/git-tools-lab",
        "official/git-tools-lab/fetch_tree",
        json!({ "remote_url": url, "commit_sha": commit_sha, "ref_name": resolved_ref_name, "dest_dir": tmp.to_string_lossy() }),
    )
    .await?;
    let _git_tree_hash = value_str(&fetch, "tree_hash")?.to_string();
    let result = async {
        let tree_hash = compute_tree_hash(&tmp).await?;
        let mut signed = false;
        if require_signed {
            let tag = invoke_package_capability(
                "official/git-tools-lab",
                "official/git-tools-lab/read_signed_tag",
                json!({ "remote_url": url, "tag": ref_name }),
            )
            .await?;
            if tag.get("pgp_signature").and_then(Value::as_str).is_none() {
                anyhow::bail!("git source {}@{} is unsigned", url, ref_name);
            }
            signed = true;
        }
        let project_yaml = tmp.join("project.yaml");
        if project_yaml.is_file() {
            let descriptor = parse_project_descriptor_at(&project_yaml)?;
            let mut packages = Vec::new();
            let mut visited = HashSet::new();
            let mut resolving = HashSet::new();
            let mut stack = Vec::new();
            resolve_project_packages(
                &tmp,
                &descriptor,
                ProjectPackageSource::Git {
                    url: url.clone(),
                    ref_name: resolved_ref_name.clone(),
                    commit_sha: commit_sha.clone(),
                    signed,
                },
                strict_conformance,
                &mut visited,
                &mut resolving,
                &mut stack,
                &mut packages,
            )?;
            return Ok(ResolvedPackages {
                packages,
                project_descriptor: Some(descriptor),
            });
        }

        let manifest_path = manifest_path_in(&tmp)?;
        let manifest = parse_manifest_at(&manifest_path)?;
        let manifest_hash = compute_manifest_hash(&manifest_path).await?;
        let mut planned = planned_from_manifest(
            &manifest,
            manifest_path.parent().unwrap_or(&tmp),
            "git".to_string(),
            Some(url.clone()),
            Some(resolved_ref_name.clone()),
            None,
            Some(commit_sha.clone()),
            manifest_hash,
            tree_hash,
            None,
            signed,
            None,
        )?;
        attach_conformance(&mut planned, &tmp, strict_conformance)?;
        Ok(ResolvedPackages {
            packages: vec![planned],
            project_descriptor: None,
        })
    }
    .await;
    fs::remove_dir_all(&tmp).ok();
    result
}

fn planned_from_manifest(
    manifest: &PackageManifest,
    base_dir: &Path,
    source: String,
    url: Option<String>,
    ref_name: Option<String>,
    path: Option<String>,
    commit_sha: Option<String>,
    manifest_hash: String,
    tree_hash: String,
    manifest_relative_path: Option<String>,
    signed: bool,
    signed_by: Option<String>,
) -> Result<PlannedPackage> {
    if manifest.entry.component.is_some() {
        manifest
            .validate_basic()
            .context("invalid explicit component declaration")?;
    }
    let surface_bundle_hash = compute_surface_bundle_hash(manifest, base_dir);
    let package_envelope = plurora_core::package_envelope_for_manifest(manifest)?;
    let package_envelope_digest = Some(package_envelope.artifact.digest.clone());
    let component_pins = package_envelope
        .components
        .iter()
        .map(plurora_core::ComponentLockPin::from_descriptor)
        .collect();
    let protocol_profile_pins = plurora_core::protocol_profile_pins_for_envelope(&package_envelope);
    let content_roots = package_envelope.content_roots.clone();
    Ok(PlannedPackage {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        source,
        url,
        ref_name,
        path,
        commit_sha,
        manifest_hash,
        tree_hash,
        surface_bundle_hash,
        package_envelope_digest,
        component_pins,
        protocol_profile_pins,
        content_roots,
        manifest_relative_path,
        signed,
        signed_by,
        permissions: permissions_from_manifest(&manifest.permissions),
        requires: manifest
            .requires
            .iter()
            .map(|req| planned_requirement(req, base_dir))
            .collect(),
        conformance: None,
    })
}

fn compute_surface_bundle_hash(manifest: &PackageManifest, base_dir: &Path) -> Option<String> {
    let plurora_core::PackageEntry::SurfaceBundle { bundle } = &manifest.entry.kind else {
        return None;
    };
    let path = normalize_relative_manifest_path(bundle)
        .ok()
        .map(|relative| base_dir.join(relative))?;
    compute_file_hash(&path).ok()
}

fn normalize_relative_manifest_path(path: &str) -> Result<String> {
    let path = Path::new(path);
    if path.is_absolute() {
        anyhow::bail!(
            "project package manifest path must be relative: {}",
            path.display()
        );
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::CurDir => {}
            _ => anyhow::bail!("project package manifest path must stay inside project root"),
        }
    }
    if normalized.as_os_str().is_empty() {
        anyhow::bail!("project package manifest path must not be empty");
    }
    Ok(normalized
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn attach_conformance(
    planned: &mut PlannedPackage,
    package_path: &Path,
    strict_conformance: bool,
) -> Result<()> {
    let report = block_on_current(plurora_core::conformance::run_checks(
        package_path,
        "v1",
        true,
    ))?;
    if !report.summary.passed_all_blocking() && strict_conformance {
        let errors = report.failed_checks().join("; ");
        anyhow::bail!("package {} fails v1 conformance: {errors}", planned.id);
    }
    planned.conformance = Some(report);
    Ok(())
}

fn planned_requirement(req: &PackageDependency, base_dir: &Path) -> PlannedRequirement {
    let source = match &req.source {
        DependencySource::Local { path } => {
            let dep_path = PathBuf::from(path);
            let absolute = if dep_path.is_absolute() {
                dep_path
            } else {
                base_dir.join(dep_path)
            };
            serde_json::to_value(DependencySource::Local {
                path: absolute.to_string_lossy().to_string(),
            })
            .unwrap_or(Value::Null)
        }
        other => serde_json::to_value(other).unwrap_or(Value::Null),
    };
    PlannedRequirement {
        id: req.id.clone(),
        source,
        version: req.version.clone(),
    }
}

fn permissions_from_manifest(permissions: &PermissionSet) -> PlannedPermissions {
    let mut network_hosts = permissions.network.hosts.clone();
    network_hosts.extend(
        permissions
            .network
            .declarations
            .iter()
            .map(|d| d.host.clone()),
    );
    PlannedPermissions {
        capabilities_invoke: sorted_vec(permissions.capabilities.invoke.iter().cloned()),
        network_hosts: sorted_vec(network_hosts),
        secret_refs: sorted_vec(permissions.secret_refs.iter().cloned()),
    }
}

fn aggregate_permissions(packages: &[PlannedPackage]) -> PermissionsSummary {
    PermissionsSummary {
        new_capabilities: sorted_vec(
            packages
                .iter()
                .flat_map(|pkg| pkg.permissions.capabilities_invoke.clone()),
        ),
        new_network_hosts: sorted_vec(
            packages
                .iter()
                .flat_map(|pkg| pkg.permissions.network_hosts.clone()),
        ),
        new_secret_refs: sorted_vec(
            packages
                .iter()
                .flat_map(|pkg| pkg.permissions.secret_refs.clone()),
        ),
    }
}

pub(super) fn verify_consent(plan: &InstallPlan, consent: &Consent) -> Result<()> {
    ensure_subset(
        &plan.permissions_summary.new_capabilities,
        &consent.approved_capabilities,
        "capability",
    )?;
    ensure_subset(
        &plan.permissions_summary.new_network_hosts,
        &consent.approved_network_hosts,
        "network host",
    )?;
    ensure_subset(
        &plan.permissions_summary.new_secret_refs,
        &consent.approved_secret_refs,
        "secret ref",
    )?;
    Ok(())
}

fn ensure_subset(required: &[String], approved: &[String], kind: &str) -> Result<()> {
    for item in required {
        if !approved.iter().any(|approved| approved == item) {
            anyhow::bail!("consent missing required {kind}: {item}");
        }
    }
    Ok(())
}
