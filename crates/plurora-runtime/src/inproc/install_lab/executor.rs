use std::path::Path;

use anyhow::Result;
use bytes::Bytes;
use plurora_work::InstallationSecretPolicy;
use serde_json::{json, Value};

use crate::inproc::invoke_capability_from_inproc;
use crate::{CapabilityInvocationRequest, FilesystemObjectStore, ObjectStore};

use super::candidate::{build_foreign_candidate, build_package_candidate, BuiltCandidate};
use super::layout::objects_dir;
use super::planner::{
    inspect_foreign_source, planned_fingerprint, resolve_package_graph, verify_consent,
};
use super::source::{parse_root_descriptor, value_str};
use super::types::{ExecutePlanInput, InstallationCandidate, SourceKind};
use super::PACKAGE_ID;

pub(super) async fn execute_plan(input: Value) -> Result<Value> {
    if input.as_object().is_some_and(|object| object.is_empty()) {
        anyhow::bail!("unsupported smoke input: missing plan");
    }
    let input: ExecutePlanInput = serde_json::from_value(input)?;
    verify_consent(&input.plan, &input.consent)?;
    anyhow::ensure!(
        input.plan.source_kind != SourceKind::Work,
        "work.yaml is an authoring source; run plurora work pack before installation"
    );

    let built = rebuild_candidate(&input).await?;
    verify_plan_fingerprint(&input.plan, &built)?;
    persist_candidate_objects(input.data_dir.as_deref(), &built).await?;

    let work_revision = built
        .work_candidate
        .work_revision
        .clone()
        .ok_or_else(|| anyhow::anyhow!("candidate is missing its WorkRevision descriptor"))?;
    let assembly_lock = built
        .work_candidate
        .assembly_lock
        .clone()
        .ok_or_else(|| anyhow::anyhow!("candidate is missing its AssemblyLock descriptor"))?;
    let installation_candidate = InstallationCandidate {
        work_revision,
        assembly_lock,
        display_name: built.work_candidate.display_name.clone(),
        source: built.acquisition,
        state_bindings: built.state_bindings,
        secret_policy: InstallationSecretPolicy {
            allowed_secret_refs: input.plan.permissions_summary.new_secret_refs.clone(),
            allow_platform_fallback: false,
        },
        closure: built.work_candidate.closure.clone(),
        diagnostics: built.diagnostics,
    };
    Ok(json!({
        "installation_candidate": installation_candidate,
        "persisted_objects": built.work_candidate.closure.iter().map(|descriptor| json!({
            "digest": descriptor.digest,
            "size_bytes": descriptor.size_bytes,
        })).collect::<Vec<_>>(),
        "next_step": "host.installation.create",
    }))
}

async fn rebuild_candidate(input: &ExecutePlanInput) -> Result<BuiltCandidate> {
    match input.plan.source_kind {
        SourceKind::Package => {
            let root_url = input.root_url.as_deref().ok_or_else(|| {
                anyhow::anyhow!("execute_plan requires root_url for a Package plan")
            })?;
            let source = parse_root_descriptor(root_url, &input.root_ref)?;
            let graph =
                resolve_package_graph(source, input.require_signed, input.strict_conformance)
                    .await?;
            let actual = graph
                .packages
                .iter()
                .map(|package| package.planned.clone())
                .collect::<Vec<_>>();
            anyhow::ensure!(
                planned_fingerprint(&actual)? == planned_fingerprint(&input.plan.packages)?,
                "package source changed after the installation plan was resolved"
            );
            build_package_candidate(&graph.packages)
        }
        SourceKind::Foreign => {
            if let Some(workspace_id) = input.workspace_id.as_deref() {
                let workspace = super::intake::read_workspace_for_execution(
                    input.data_dir.as_deref(),
                    workspace_id,
                )
                .await?;
                return build_foreign_candidate(&workspace.source_digest, workspace.display_name);
            }
            let root_url = input.root_url.as_deref().ok_or_else(|| {
                anyhow::anyhow!("execute_plan requires root_url or workspace_id for a Foreign plan")
            })?;
            let source = parse_root_descriptor(root_url, &input.root_ref)?;
            let (source_digest, display_name) = inspect_foreign_source(&source).await?;
            build_foreign_candidate(&source_digest, display_name)
        }
        SourceKind::Work => unreachable!("Work source rejected before candidate rebuild"),
    }
}

fn verify_plan_fingerprint(plan: &super::types::InstallPlan, built: &BuiltCandidate) -> Result<()> {
    anyhow::ensure!(
        plan.root_id == built.root_id,
        "candidate root identity changed after planning"
    );
    anyhow::ensure!(
        serde_json::to_value(&plan.work_candidate)? == serde_json::to_value(&built.work_candidate)?,
        "Work or AssemblyLock candidate changed after planning"
    );
    Ok(())
}

pub(super) async fn persist_candidate_objects(
    data_dir: Option<&str>,
    built: &BuiltCandidate,
) -> Result<()> {
    let store = FilesystemObjectStore::new(objects_dir(data_dir)?);
    for object in &built.artifacts {
        let info = store
            .put(Bytes::from(object.bytes.clone()))
            .await
            .map_err(|_| anyhow::anyhow!("ObjectStore rejected a candidate artifact"))?;
        anyhow::ensure!(
            info.digest == object.descriptor.digest
                && info.size_bytes == object.descriptor.size_bytes,
            "ObjectStore returned an inconsistent artifact descriptor"
        );
        let verified = store
            .verify(&object.descriptor.digest)
            .await
            .map_err(|_| anyhow::anyhow!("ObjectStore could not verify a candidate artifact"))?;
        anyhow::ensure!(
            verified.digest == object.descriptor.digest
                && verified.size_bytes == object.descriptor.size_bytes,
            "persisted candidate artifact failed descriptor verification"
        );
    }
    Ok(())
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
        "plurora/integrity-lab",
        "plurora/integrity-lab/compute_manifest_hash",
        json!({ "manifest_path": path.to_string_lossy() }),
    )
    .await?;
    Ok(value_str(&output, "sha256")?.to_string())
}

pub(super) async fn compute_tree_hash(path: &Path) -> Result<String> {
    let output = invoke_package_capability(
        "plurora/integrity-lab",
        "plurora/integrity-lab/compute_tree_hash",
        json!({ "dir": path.to_string_lossy() }),
    )
    .await?;
    Ok(value_str(&output, "sha256")?.to_string())
}

pub(super) async fn compute_external_tree_hash(path: &Path) -> Result<String> {
    compute_external_tree_hash_with_profile(path, "external_workspace_v1").await
}

pub(super) async fn compute_external_git_tree_hash(path: &Path) -> Result<String> {
    compute_external_tree_hash_with_profile(path, "external_git_workspace_v1").await
}

async fn compute_external_tree_hash_with_profile(path: &Path, profile: &str) -> Result<String> {
    let output = invoke_package_capability(
        "plurora/integrity-lab",
        "plurora/integrity-lab/compute_tree_hash",
        json!({
            "dir": path.to_string_lossy(),
            "profile": profile,
        }),
    )
    .await?;
    Ok(value_str(&output, "sha256")?.to_string())
}
