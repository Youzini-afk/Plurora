use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use plurora_core::{
    package_envelope_for_manifest, protocol_profile_pins_for_envelope, ComponentLockPin,
    PackageManifest, PermissionSet,
};
use serde_json::{json, Value};
use uuid::Uuid;

use super::candidate::{build_foreign_candidate, build_package_candidate};
use super::detection::{detect_descriptor, detect_source_kind};
use super::executor::{
    compute_external_tree_hash, compute_manifest_hash, compute_tree_hash, invoke_package_capability,
};
use super::source::{
    block_on_current, dependency_source, manifest_path_in, parse_manifest_at,
    parse_root_descriptor, sorted_vec, value_str, SourceDescriptor,
};
use super::types::{
    Consent, InstallPlan, IntegritySummary, PermissionsSummary, PlannedConformance, PlannedPackage,
    PlannedPackageSourceKind, PlannedPermissions, PlannedRequirement, ResolvePlanInput,
    SignatureSummary, SourceKind, WorkCandidate, WorkCandidateStatus,
};

const MAX_DEPENDENCY_DEPTH: usize = 32;

#[derive(Debug, Clone)]
pub(super) struct ResolvedPackage {
    pub(super) planned: PlannedPackage,
    pub(super) manifest: PackageManifest,
}

#[derive(Debug)]
pub(super) struct ResolvedPackageGraph {
    pub(super) packages: Vec<ResolvedPackage>,
    temporary_roots: Vec<PathBuf>,
}

impl Drop for ResolvedPackageGraph {
    fn drop(&mut self) {
        for root in &self.temporary_roots {
            if root.is_dir() && !root.is_symlink() {
                let _ = fs::remove_dir_all(root);
            }
        }
    }
}

pub(super) async fn resolve_plan(input: Value) -> Result<Value> {
    if input.as_object().is_some_and(|object| object.is_empty()) {
        anyhow::bail!("unsupported smoke input: missing root_url");
    }
    let input: ResolvePlanInput = serde_json::from_value(input)?;
    let source = parse_root_descriptor(&input.root_url, &input.root_ref)?;
    let source_kind = detect_descriptor(&source).await?;

    let plan = match source_kind {
        SourceKind::Work => work_authoring_plan(&source)?,
        SourceKind::Package => {
            let graph =
                resolve_package_graph(source, input.require_signed, input.strict_conformance)
                    .await?;
            package_plan(&graph)?
        }
        SourceKind::Foreign => {
            let (source_digest, display_name) = inspect_foreign_source(&source).await?;
            foreign_plan(&source_digest, display_name)?
        }
    };
    Ok(json!({ "plan": plan }))
}

fn work_authoring_plan(source: &SourceDescriptor) -> Result<InstallPlan> {
    let display_name = source_display_name(source, "Work source");
    Ok(InstallPlan {
        source_kind: SourceKind::Work,
        root_id: String::new(),
        packages: Vec::new(),
        work_candidate: WorkCandidate {
            source_kind: SourceKind::Work,
            status: WorkCandidateStatus::AuthoringRequired,
            display_name,
            work_revision: None,
            assembly_lock: None,
            closure: Vec::new(),
            diagnostics: vec![json!({
                "code": "work_pack_required",
                "message": "work.yaml is an authoring source and must be packed before installation",
                "next_step": "plurora work pack",
            })],
        },
        permissions_summary: PermissionsSummary::default(),
        signature_summary: SignatureSummary {
            all_signed: true,
            unsigned_packages: Vec::new(),
        },
        integrity_summary: IntegritySummary {
            all_objects_content_addressed: false,
            drift_detected: Vec::new(),
        },
    })
}

fn package_plan(graph: &ResolvedPackageGraph) -> Result<InstallPlan> {
    let built = build_package_candidate(&graph.packages)?;
    let packages = graph
        .packages
        .iter()
        .map(|package| package.planned.clone())
        .collect::<Vec<_>>();
    let root_id = packages
        .first()
        .map(|package| package.id.clone())
        .ok_or_else(|| anyhow::anyhow!("package resolution produced no root package"))?;
    let unsigned_packages = packages
        .iter()
        .filter(|package| !package.signed)
        .map(|package| package.id.clone())
        .collect::<Vec<_>>();
    Ok(InstallPlan {
        source_kind: SourceKind::Package,
        root_id,
        permissions_summary: aggregate_permissions(&packages),
        signature_summary: SignatureSummary {
            all_signed: unsigned_packages.is_empty(),
            unsigned_packages,
        },
        integrity_summary: IntegritySummary {
            all_objects_content_addressed: true,
            drift_detected: Vec::new(),
        },
        packages,
        work_candidate: built.work_candidate,
    })
}

pub(super) fn foreign_plan(source_digest: &str, display_name: String) -> Result<InstallPlan> {
    let built = build_foreign_candidate(source_digest, display_name)?;
    Ok(InstallPlan {
        source_kind: SourceKind::Foreign,
        root_id: built.root_id,
        packages: Vec::new(),
        work_candidate: built.work_candidate,
        permissions_summary: PermissionsSummary::default(),
        signature_summary: SignatureSummary {
            all_signed: false,
            unsigned_packages: Vec::new(),
        },
        integrity_summary: IntegritySummary {
            all_objects_content_addressed: true,
            drift_detected: Vec::new(),
        },
    })
}

pub(super) async fn resolve_package_graph(
    source: SourceDescriptor,
    require_signed: bool,
    strict_conformance: bool,
) -> Result<ResolvedPackageGraph> {
    let mut graph = ResolvedPackageGraph {
        packages: Vec::new(),
        temporary_roots: Vec::new(),
    };
    let mut visited = HashMap::<String, String>::new();
    let mut stack = Vec::new();
    resolve_package_recursive(
        source,
        require_signed,
        strict_conformance,
        MAX_DEPENDENCY_DEPTH,
        &mut visited,
        &mut stack,
        &mut graph,
    )?;
    Ok(graph)
}

#[allow(clippy::too_many_arguments)]
fn resolve_package_recursive(
    source: SourceDescriptor,
    require_signed: bool,
    strict_conformance: bool,
    remaining_depth: usize,
    visited: &mut HashMap<String, String>,
    stack: &mut Vec<String>,
    graph: &mut ResolvedPackageGraph,
) -> Result<()> {
    anyhow::ensure!(remaining_depth > 0, "package dependency depth exceeds 32");
    let materialized = materialize_package_source(source, require_signed, graph)?;
    let manifest_path = manifest_path_in(&materialized.root)?;
    let manifest = parse_manifest_at(&manifest_path)?;
    manifest
        .validate_basic()
        .context("package manifest is invalid")?;
    let package_id = manifest.id.clone();
    if let Some(position) = stack.iter().position(|id| id == &package_id) {
        let mut cycle = stack[position..].to_vec();
        cycle.push(package_id);
        anyhow::bail!("package dependency cycle detected: {}", cycle.join(" -> "));
    }

    let manifest_hash = block_on_current(compute_manifest_hash(&manifest_path))?;
    let tree_hash = block_on_current(compute_tree_hash(&materialized.root))?;
    if let Some(previous) = visited.get(&manifest.id) {
        anyhow::ensure!(
            previous == &tree_hash,
            "package dependency identity resolved to more than one content digest"
        );
        return Ok(());
    }
    visited.insert(manifest.id.clone(), tree_hash.clone());
    stack.push(manifest.id.clone());

    let planned = planned_from_manifest(
        &manifest,
        &materialized.root,
        materialized.source_kind,
        materialized.commit_sha,
        materialized.signed,
        manifest_hash,
        tree_hash,
        strict_conformance,
    )?;
    graph.packages.push(ResolvedPackage {
        planned,
        manifest: manifest.clone(),
    });

    for dependency in &manifest.requires {
        let dependency_source = dependency_source(dependency, &materialized.root)?;
        if matches!(dependency_source, SourceDescriptor::Internal) {
            continue;
        }
        resolve_package_recursive(
            dependency_source,
            require_signed,
            strict_conformance,
            remaining_depth - 1,
            visited,
            stack,
            graph,
        )?;
    }
    stack.pop();
    Ok(())
}

struct MaterializedPackageSource {
    root: PathBuf,
    source_kind: PlannedPackageSourceKind,
    commit_sha: Option<String>,
    signed: bool,
}

fn materialize_package_source(
    source: SourceDescriptor,
    require_signed: bool,
    graph: &mut ResolvedPackageGraph,
) -> Result<MaterializedPackageSource> {
    match source {
        SourceDescriptor::Local { path } => {
            let root = fs::canonicalize(path)
                .map_err(|_| anyhow::anyhow!("local package source could not be opened"))?;
            anyhow::ensure!(root.is_dir(), "local package source must be a directory");
            anyhow::ensure!(
                detect_source_kind(&root) == SourceKind::Package,
                "package dependency source does not contain a Package manifest"
            );
            Ok(MaterializedPackageSource {
                root,
                source_kind: PlannedPackageSourceKind::Local,
                commit_sha: None,
                signed: false,
            })
        }
        SourceDescriptor::Git { url, ref_name } => {
            let resolved = block_on_current(invoke_package_capability(
                "plurora/git-tools-lab",
                "plurora/git-tools-lab/resolve_ref",
                json!({ "remote_url": url, "ref": ref_name }),
            ))?;
            let commit_sha = value_str(&resolved, "commit_sha")?.to_string();
            let resolved_ref = resolved
                .get("ref_name")
                .and_then(Value::as_str)
                .unwrap_or(&ref_name)
                .to_string();
            let temporary =
                std::env::temp_dir().join(format!("plurora-package-source-{}", Uuid::new_v4()));
            block_on_current(invoke_package_capability(
                "plurora/git-tools-lab",
                "plurora/git-tools-lab/fetch_tree",
                json!({
                    "remote_url": url,
                    "commit_sha": commit_sha,
                    "ref_name": resolved_ref,
                    "dest_dir": temporary.to_string_lossy(),
                }),
            ))?;
            let signed = if require_signed {
                let tag = block_on_current(invoke_package_capability(
                    "plurora/git-tools-lab",
                    "plurora/git-tools-lab/read_signed_tag",
                    json!({ "remote_url": url, "tag": ref_name }),
                ))?;
                anyhow::ensure!(
                    tag.get("pgp_signature").and_then(Value::as_str).is_some(),
                    "git package source is unsigned"
                );
                true
            } else {
                false
            };
            graph.temporary_roots.push(temporary.clone());
            Ok(MaterializedPackageSource {
                root: temporary,
                source_kind: PlannedPackageSourceKind::Git,
                commit_sha: Some(commit_sha),
                signed,
            })
        }
        SourceDescriptor::Internal => {
            anyhow::bail!("internal package does not require acquisition")
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn planned_from_manifest(
    manifest: &PackageManifest,
    package_root: &Path,
    source_kind: PlannedPackageSourceKind,
    commit_sha: Option<String>,
    signed: bool,
    manifest_hash: String,
    tree_hash: String,
    strict_conformance: bool,
) -> Result<PlannedPackage> {
    let envelope = package_envelope_for_manifest(manifest)?;
    let report = block_on_current(plurora_core::conformance::run_checks(
        package_root,
        "v1",
        true,
    ))?;
    let passed_blocking = report.summary.passed_all_blocking();
    let failed_checks = report.failed_checks();
    if strict_conformance && !passed_blocking {
        anyhow::bail!(
            "package {} fails v1 conformance: {}",
            manifest.id,
            failed_checks.join("; ")
        );
    }
    Ok(PlannedPackage {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        source_kind,
        manifest_hash,
        tree_hash,
        commit_sha,
        package_envelope_digest: Some(envelope.artifact.digest.clone()),
        component_pins: envelope
            .components
            .iter()
            .map(ComponentLockPin::from_descriptor)
            .collect(),
        protocol_profile_pins: protocol_profile_pins_for_envelope(&envelope),
        content_roots: envelope.content_roots.clone(),
        signed,
        signed_by: None,
        permissions: permissions_from_manifest(&manifest.permissions),
        requires: manifest
            .requires
            .iter()
            .map(|dependency| PlannedRequirement {
                id: dependency.id.clone(),
                source_kind: match &dependency.source {
                    plurora_core::DependencySource::Internal => "internal",
                    plurora_core::DependencySource::Git { .. } => "git",
                    plurora_core::DependencySource::Local { .. } => "local",
                }
                .to_string(),
                version: dependency.version.clone(),
            })
            .collect(),
        conformance: Some(PlannedConformance {
            passed_blocking,
            failed_checks,
        }),
    })
}

fn permissions_from_manifest(permissions: &PermissionSet) -> PlannedPermissions {
    let mut network_hosts = permissions.network.hosts.clone();
    network_hosts.extend(
        permissions
            .network
            .declarations
            .iter()
            .map(|declaration| declaration.host.clone()),
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
                .flat_map(|package| package.permissions.capabilities_invoke.clone()),
        ),
        new_network_hosts: sorted_vec(
            packages
                .iter()
                .flat_map(|package| package.permissions.network_hosts.clone()),
        ),
        new_secret_refs: sorted_vec(
            packages
                .iter()
                .flat_map(|package| package.permissions.secret_refs.clone()),
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
        "secret reference",
    )
}

fn ensure_subset(required: &[String], approved: &[String], kind: &str) -> Result<()> {
    let approved = approved.iter().collect::<HashSet<_>>();
    for item in required {
        anyhow::ensure!(
            approved.contains(item),
            "consent is missing a required {kind}"
        );
    }
    Ok(())
}

pub(super) async fn inspect_foreign_source(source: &SourceDescriptor) -> Result<(String, String)> {
    match source {
        SourceDescriptor::Local { path } => {
            let root = fs::canonicalize(path)
                .map_err(|_| anyhow::anyhow!("foreign source could not be opened"))?;
            anyhow::ensure!(root.is_dir(), "foreign source must be a directory");
            let digest = compute_external_tree_hash(&root).await?;
            Ok((digest, source_display_name(source, "Foreign source")))
        }
        SourceDescriptor::Git { url, ref_name } => {
            let resolved = invoke_package_capability(
                "plurora/git-tools-lab",
                "plurora/git-tools-lab/resolve_ref",
                json!({ "remote_url": url, "ref": ref_name }),
            )
            .await?;
            let commit_sha = value_str(&resolved, "commit_sha")?.to_string();
            let temporary =
                std::env::temp_dir().join(format!("plurora-foreign-source-{}", Uuid::new_v4()));
            let result = async {
                invoke_package_capability(
                    "plurora/git-tools-lab",
                    "plurora/git-tools-lab/fetch_tree",
                    json!({
                        "remote_url": url,
                        "commit_sha": commit_sha,
                        "ref_name": ref_name,
                        "dest_dir": temporary.to_string_lossy(),
                        "max_files": super::intake::EXTERNAL_WORKSPACE_MAX_FILES,
                        "max_directories": super::intake::EXTERNAL_WORKSPACE_MAX_DIRECTORIES,
                        "max_total_bytes": super::intake::EXTERNAL_WORKSPACE_MAX_BYTES,
                    }),
                )
                .await?;
                let digest = compute_external_tree_hash(&temporary).await?;
                Ok((digest, source_display_name(source, "Foreign source")))
            }
            .await;
            if temporary.is_dir() && !temporary.is_symlink() {
                let _ = fs::remove_dir_all(&temporary);
            }
            result
        }
        SourceDescriptor::Internal => anyhow::bail!("internal source cannot become a Foreign Work"),
    }
}

pub(super) fn source_display_name(source: &SourceDescriptor, fallback: &str) -> String {
    let candidate = match source {
        SourceDescriptor::Local { path } => path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string),
        SourceDescriptor::Git { url, .. } => url::Url::parse(url).ok().and_then(|parsed| {
            parsed
                .path_segments()
                .and_then(|mut segments| segments.next_back().map(str::to_string))
        }),
        SourceDescriptor::Internal => None,
    };
    candidate
        .map(|name| name.trim_end_matches(".git").to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn planned_fingerprint(packages: &[PlannedPackage]) -> Result<Value> {
    Ok(serde_json::to_value(packages)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_display_name_never_returns_a_full_local_path() {
        let source = SourceDescriptor::Local {
            path: PathBuf::from("C:/users/example/source-tree"),
        };
        assert_eq!(source_display_name(&source, "fallback"), "source-tree");
    }

    #[test]
    fn work_authoring_plan_contains_a_pack_next_step() {
        let plan = work_authoring_plan(&SourceDescriptor::Local {
            path: PathBuf::from("work"),
        })
        .unwrap();
        assert_eq!(plan.source_kind, SourceKind::Work);
        assert_eq!(
            plan.work_candidate.status,
            WorkCandidateStatus::AuthoringRequired
        );
        assert_eq!(
            plan.work_candidate.diagnostics[0]["next_step"],
            json!("plurora work pack")
        );
    }
}
