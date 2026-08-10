use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use plurora_runtime::CapabilityInvocationRequest;
use serde_json::{json, Value};
use tempfile::TempDir;
use uuid::Uuid;

use super::fixtures::*;
use crate::commands::manifest;

const INSTALL_MANIFEST: &str = "packages/plurora/install-lab/manifest.yaml";
const GIT_MANIFEST: &str = "packages/plurora/git-tools-lab/manifest.yaml";
const INTEGRITY_MANIFEST: &str = "packages/plurora/integrity-lab/manifest.yaml";
const PACKAGE_ID: &str = "plurora/install-lab";

async fn load_install_lab(
) -> anyhow::Result<plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>> {
    let (_store, runtime) = runtime();
    for path in [GIT_MANIFEST, INTEGRITY_MANIFEST, INSTALL_MANIFEST] {
        runtime
            .load_package(manifest::read_manifest(PathBuf::from(path)).await?)
            .await?;
    }
    Ok(runtime)
}

async fn invoke(
    runtime: &plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>,
    capability_id: &str,
    input: Value,
) -> anyhow::Result<plurora_runtime::CapabilityInvocationResult> {
    runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(capability_id.to_string()),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input,
        })
        .await
        .map_err(Into::into)
}

pub(crate) async fn detect_source_classifies_work_package_and_foreign() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let temporary = TempDir::new()?;
    let work = temporary.path().join("work");
    let foreign = temporary.path().join("foreign");
    fs::create_dir_all(&work)?;
    fs::create_dir_all(&foreign)?;
    fs::write(work.join("work.yaml"), "schema_version: 1\n")?;
    fs::write(foreign.join("README.md"), "ordinary source")?;

    for (source, expected) in [
        (work.to_string_lossy().into_owned(), "work"),
        (fixture_path("pkg-local"), "package"),
        (foreign.to_string_lossy().into_owned(), "foreign"),
    ] {
        let output = invoke(
            &runtime,
            "plurora/install-lab/detect_source",
            json!({ "path": source }),
        )
        .await?;
        anyhow::ensure!(output.output["source_kind"] == json!(expected));
    }
    Ok(())
}

pub(crate) async fn resolve_plan_local_package() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let output = invoke(
        &runtime,
        "plurora/install-lab/resolve_plan",
        json!({ "root_url": fixture_path("pkg-local") }),
    )
    .await?;
    let plan = &output.output["plan"];
    anyhow::ensure!(plan["source_kind"] == json!("package"));
    anyhow::ensure!(plan["root_id"] == json!("fixture/pkg-local"));
    anyhow::ensure!(plan["packages"].as_array().context("packages")?.len() == 1);
    anyhow::ensure!(plan["packages"][0]["source_kind"] == json!("local"));
    for field in ["manifest_hash", "tree_hash", "package_envelope_digest"] {
        anyhow::ensure!(plan["packages"][0][field]
            .as_str()
            .with_context(|| format!("missing {field}"))?
            .starts_with("sha256:"));
    }
    anyhow::ensure!(
        plan["packages"][0]["component_pins"]
            .as_array()
            .context("component_pins")?
            .len()
            == 1
    );
    anyhow::ensure!(plan["work_candidate"]["status"] == json!("installable"));
    Ok(())
}

pub(crate) async fn resolve_plan_runs_conformance() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let output = invoke(
        &runtime,
        "plurora/install-lab/resolve_plan",
        json!({ "root_url": fixture_path("pkg-local") }),
    )
    .await?;
    let conformance = &output.output["plan"]["packages"][0]["conformance"];
    anyhow::ensure!(conformance["passed_blocking"] == json!(true));
    let failed_checks = conformance.get("failed_checks").and_then(Value::as_array);
    anyhow::ensure!(failed_checks.is_none() || failed_checks.is_some_and(Vec::is_empty));
    Ok(())
}

pub(crate) async fn invalid_manifest_is_rejected_in_strict_mode() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let error = invoke(
        &runtime,
        "plurora/install-lab/resolve_plan",
        json!({
            "root_url": fixture_path("pkg-broken-manifest"),
            "strict_conformance": true,
        }),
    )
    .await
    .expect_err("strict mode must reject an invalid package manifest before planning");
    anyhow::ensure!(
        error.to_string().contains("package manifest is invalid"),
        "unexpected error: {error}"
    );
    Ok(())
}

pub(crate) async fn invalid_manifest_is_rejected_in_lenient_mode() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let error = invoke(
        &runtime,
        "plurora/install-lab/resolve_plan",
        json!({
            "root_url": fixture_path("pkg-broken-manifest"),
            "strict_conformance": false,
        }),
    )
    .await
    .expect_err("lenient conformance must not bypass basic manifest validation");
    anyhow::ensure!(
        error.to_string().contains("package manifest is invalid"),
        "unexpected error: {error}"
    );
    Ok(())
}

pub(crate) async fn invalid_transitive_manifest_is_rejected() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let error = invoke(
        &runtime,
        "plurora/install-lab/resolve_plan",
        json!({
            "root_url": fixture_path("pkg-a-broken-dep"),
            "strict_conformance": false,
        }),
    )
    .await
    .expect_err("an invalid transitive dependency manifest must stop planning");
    anyhow::ensure!(
        error.to_string().contains("package manifest is invalid"),
        "unexpected error: {error}"
    );
    Ok(())
}

pub(crate) async fn resolve_plan_with_transitive() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let output = invoke(
        &runtime,
        "plurora/install-lab/resolve_plan",
        json!({ "root_url": fixture_path("pkg-a") }),
    )
    .await?;
    let ids = output.output["plan"]["packages"]
        .as_array()
        .context("packages")?
        .iter()
        .filter_map(|package| package["id"].as_str())
        .collect::<Vec<_>>();
    anyhow::ensure!(ids.contains(&"fixture/pkg-a"));
    anyhow::ensure!(ids.contains(&"fixture/pkg-b"));
    Ok(())
}

pub(crate) async fn resolve_plan_cycle_detection() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let error = invoke(
        &runtime,
        "plurora/install-lab/resolve_plan",
        json!({ "root_url": fixture_path("pkg-cycle-a") }),
    )
    .await
    .expect_err("dependency cycle must fail");
    anyhow::ensure!(
        error.to_string().contains("cycle"),
        "unexpected error: {error}"
    );
    Ok(())
}

pub(crate) async fn execute_plan_emits_installation_candidate() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let temporary = TempDir::new()?;
    let source = fixture_path("pkg-local");
    let plan = resolve_package_plan(&runtime, &source).await?;
    let output =
        execute_with_full_consent(&runtime, plan, Some(&source), None, temporary.path()).await?;

    anyhow::ensure!(output.output["next_step"] == json!("host.installation.create"));
    let candidate = &output.output["installation_candidate"];
    for field in ["work_revision", "assembly_lock"] {
        anyhow::ensure!(candidate[field]["digest"]
            .as_str()
            .with_context(|| format!("missing {field} digest"))?
            .starts_with("sha256:"));
    }
    anyhow::ensure!(candidate["display_name"] == json!("fixture/pkg-local"));
    anyhow::ensure!(candidate.get("installation_id").is_none());
    anyhow::ensure!(!temporary.path().join("installations").exists());
    Ok(())
}

pub(crate) async fn execute_plan_persists_candidate_objects() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let temporary = TempDir::new()?;
    let source = fixture_path("pkg-local");
    let plan = resolve_package_plan(&runtime, &source).await?;
    let output =
        execute_with_full_consent(&runtime, plan, Some(&source), None, temporary.path()).await?;

    let persisted = output.output["persisted_objects"]
        .as_array()
        .context("persisted_objects")?;
    anyhow::ensure!(!persisted.is_empty());
    anyhow::ensure!(persisted.iter().all(|object| object["digest"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:"))));
    anyhow::ensure!(tree_contains_file(&temporary.path().join("objects"))?);
    anyhow::ensure!(!temporary.path().join("profiles").exists());
    Ok(())
}

pub(crate) async fn execute_plan_consent_mismatch() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let temporary = TempDir::new()?;
    let source = fixture_path("pkg-local");
    let plan = resolve_package_plan(&runtime, &source).await?;
    let error = invoke(
        &runtime,
        "plurora/install-lab/execute_plan",
        json!({
            "plan": plan,
            "consent": {
                "approved_capabilities": [],
                "approved_network_hosts": [],
                "approved_secret_refs": [],
            },
            "root_url": source,
            "data_dir": temporary.path(),
        }),
    )
    .await
    .expect_err("missing consent must fail");
    anyhow::ensure!(
        error.to_string().contains("consent is missing"),
        "unexpected error: {error}"
    );
    Ok(())
}

pub(crate) async fn execute_plan_rejects_source_drift() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let temporary = TempDir::new()?;
    let source = temporary.path().join("package-source");
    copy_dir_all(Path::new(&fixture_path("pkg-local")), &source)?;
    let source_text = source.to_string_lossy().into_owned();
    let plan = resolve_package_plan(&runtime, &source_text).await?;
    fs::write(source.join("changed-after-plan.txt"), "drift")?;

    let error =
        execute_with_full_consent(&runtime, plan, Some(&source_text), None, temporary.path())
            .await
            .expect_err("source drift must invalidate a resolved plan");
    anyhow::ensure!(
        error.to_string().contains("source changed"),
        "unexpected error: {error}"
    );
    Ok(())
}

pub(crate) async fn work_source_requires_pack() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let temporary = TempDir::new()?;
    let source = temporary.path().join("authored-work");
    fs::create_dir_all(&source)?;
    fs::write(source.join("work.yaml"), "schema_version: 1\n")?;
    let source_text = source.to_string_lossy().into_owned();
    let plan = take_plan(
        invoke(
            &runtime,
            "plurora/install-lab/resolve_plan",
            json!({ "root_url": &source_text }),
        )
        .await?,
    )?;
    anyhow::ensure!(plan["source_kind"] == json!("work"));
    anyhow::ensure!(plan["work_candidate"]["status"] == json!("authoring_required"));
    anyhow::ensure!(
        plan["work_candidate"]["diagnostics"][0]["next_step"] == json!("plurora work pack")
    );

    let error =
        execute_with_full_consent(&runtime, plan, Some(&source_text), None, temporary.path())
            .await
            .expect_err("an authoring source must be packed before installation");
    anyhow::ensure!(
        error.to_string().contains("work pack"),
        "unexpected error: {error}"
    );
    Ok(())
}

pub(crate) async fn external_intake_creates_managed_workspace() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let temporary = TempDir::new()?;
    let source = temporary.path().join("foreign-source");
    fs::create_dir_all(&source)?;
    fs::write(source.join("README.md"), "foreign input")?;

    let output = invoke(
        &runtime,
        "plurora/install-lab/prepare_external_intake",
        json!({
            "source": source,
            "data_dir": temporary.path(),
        }),
    )
    .await?;
    let workspace_id = output.output["workspace"]["workspace_id"]
        .as_str()
        .context("workspace_id")?;
    Uuid::parse_str(workspace_id)?;
    anyhow::ensure!(output.output["workspace"]["ownership"] == json!("managed"));
    anyhow::ensure!(output.output["work_candidate"]["status"] == json!("foreign_binding_required"));
    let workspace = temporary.path().join("workspaces").join(workspace_id);
    anyhow::ensure!(workspace.join("workspace.json").is_file());
    anyhow::ensure!(workspace.join("source/README.md").is_file());
    anyhow::ensure!(!output.output.to_string().contains("source_locator"));
    Ok(())
}

pub(crate) async fn linked_local_intake_preserves_user_source() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let temporary = TempDir::new()?;
    let source = temporary.path().join("linked-source");
    fs::create_dir_all(&source)?;
    fs::write(source.join("owned-by-user.txt"), "keep")?;

    let output = invoke(
        &runtime,
        "plurora/install-lab/prepare_external_intake",
        json!({
            "source": source,
            "data_dir": temporary.path(),
            "linked_local": true,
        }),
    )
    .await?;
    let workspace_id = output.output["workspace"]["workspace_id"]
        .as_str()
        .context("workspace_id")?;
    anyhow::ensure!(output.output["workspace"]["ownership"] == json!("linked_local"));
    let workspace = temporary.path().join("workspaces").join(workspace_id);
    anyhow::ensure!(workspace.join("workspace.json").is_file());
    anyhow::ensure!(!workspace.join("source").exists());
    anyhow::ensure!(source.join("owned-by-user.txt").is_file());
    Ok(())
}

pub(crate) async fn external_workspace_emits_installation_candidate() -> anyhow::Result<()> {
    let runtime = load_install_lab().await?;
    let temporary = TempDir::new()?;
    let source = temporary.path().join("foreign-source");
    fs::create_dir_all(&source)?;
    fs::write(source.join("README.md"), "foreign input")?;
    let intake = invoke(
        &runtime,
        "plurora/install-lab/prepare_external_intake",
        json!({
            "source": source,
            "data_dir": temporary.path(),
        }),
    )
    .await?;
    let workspace_id = intake.output["workspace"]["workspace_id"]
        .as_str()
        .context("workspace_id")?
        .to_string();
    let plan = intake.output["plan"].clone();
    let output =
        execute_with_full_consent(&runtime, plan, None, Some(&workspace_id), temporary.path())
            .await?;

    anyhow::ensure!(output.output["next_step"] == json!("host.installation.create"));
    anyhow::ensure!(
        output.output["installation_candidate"]["work_revision"]["digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:"))
    );
    anyhow::ensure!(
        output.output["installation_candidate"]["source"]["kind"] == json!("local_import")
    );
    anyhow::ensure!(output.output["installation_candidate"]
        .get("installation_id")
        .is_none());
    Ok(())
}

async fn resolve_package_plan(
    runtime: &plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>,
    source: &str,
) -> anyhow::Result<Value> {
    take_plan(
        invoke(
            runtime,
            "plurora/install-lab/resolve_plan",
            json!({ "root_url": source }),
        )
        .await?,
    )
}

fn take_plan(mut result: plurora_runtime::CapabilityInvocationResult) -> anyhow::Result<Value> {
    result
        .output
        .get_mut("plan")
        .map(Value::take)
        .context("install-lab resolve_plan response missing plan")
}

async fn execute_with_full_consent(
    runtime: &plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>,
    plan: Value,
    root_url: Option<&str>,
    workspace_id: Option<&str>,
    data_dir: &Path,
) -> anyhow::Result<plurora_runtime::CapabilityInvocationResult> {
    let approved_capabilities = plan["permissions_summary"]["new_capabilities"].clone();
    let approved_network_hosts = plan["permissions_summary"]["new_network_hosts"].clone();
    let approved_secret_refs = plan["permissions_summary"]["new_secret_refs"].clone();
    let mut input = json!({
        "plan": plan,
        "consent": {
            "approved_capabilities": approved_capabilities,
            "approved_network_hosts": approved_network_hosts,
            "approved_secret_refs": approved_secret_refs,
        },
        "data_dir": data_dir,
    });
    if let Some(root_url) = root_url {
        input["root_url"] = json!(root_url);
    }
    if let Some(workspace_id) = workspace_id {
        input["workspace_id"] = json!(workspace_id);
    }
    invoke(runtime, "plurora/install-lab/execute_plan", input).await
}

fn fixture_path(name: &str) -> String {
    PathBuf::from("crates/plurora-cli/src/conformance/fixtures/install")
        .join(name)
        .display()
        .to_string()
}

fn copy_dir_all(source: &Path, destination: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn tree_contains_file(root: &Path) -> anyhow::Result<bool> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_file() || tree_contains_file(&entry.path())? {
            return Ok(true);
        }
    }
    Ok(false)
}
