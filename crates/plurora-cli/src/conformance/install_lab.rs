use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use plurora_runtime::CapabilityInvocationRequest;
use serde_json::{json, Value};
use tempfile::TempDir;

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
    cap: &str,
    input: Value,
) -> anyhow::Result<plurora_runtime::CapabilityInvocationResult> {
    runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(cap.to_string()),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input,
        })
        .await
        .map_err(Into::into)
}

pub(crate) async fn resolve_plan_local_source() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let pkg = fixture_path("pkg-local");
    let out = invoke(
        &rt,
        "plurora/install-lab/resolve_plan",
        json!({ "root_url": pkg }),
    )
    .await?;
    let plan = &out.output["plan"];
    anyhow::ensure!(plan["root_id"] == json!("fixture/pkg-local"));
    anyhow::ensure!(plan["packages"].as_array().context("packages")?.len() == 1);
    anyhow::ensure!(plan["packages"][0]["source"] == json!("local"));
    anyhow::ensure!(plan["packages"][0]["manifest_hash"]
        .as_str()
        .unwrap_or("")
        .starts_with("sha256:"));
    anyhow::ensure!(plan["packages"][0]["tree_hash"]
        .as_str()
        .unwrap_or("")
        .starts_with("sha256:"));
    anyhow::ensure!(plan["packages"][0]["package_envelope_digest"]
        .as_str()
        .unwrap_or("")
        .starts_with("sha256:"));
    anyhow::ensure!(
        plan["packages"][0]["component_pins"]
            .as_array()
            .context("component_pins")?
            .len()
            == 1
    );
    Ok(())
}

pub(crate) async fn resolve_plan_runs_conformance() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let pkg = fixture_path("pkg-local");
    let out = invoke(
        &rt,
        "plurora/install-lab/resolve_plan",
        json!({ "root_url": pkg }),
    )
    .await?;
    let report = &out.output["plan"]["packages"][0]["conformance"];
    anyhow::ensure!(report["summary"]["passed"].as_u64().unwrap_or(0) >= 2);
    anyhow::ensure!(report["summary"]["skipped"].as_u64().unwrap_or(0) >= 5);
    anyhow::ensure!(report["checks"]
        .as_array()
        .context("checks")?
        .iter()
        .any(|check| {
            check["id"] == json!("manifest.schema_valid") && check["status"] == json!("Pass")
        }));
    Ok(())
}

pub(crate) async fn resolve_plan_blocks_when_strict() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let err = invoke(
        &rt,
        "plurora/install-lab/resolve_plan",
        json!({
            "root_url": fixture_path("pkg-broken-manifest"),
            "strict_conformance": true,
        }),
    )
    .await
    .expect_err("broken manifest should fail conformance in strict mode");
    anyhow::ensure!(
        err.to_string().contains("fails v1 conformance"),
        "unexpected error: {err}"
    );
    Ok(())
}

pub(crate) async fn strict_conformance_blocks() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let err = invoke(
        &rt,
        "plurora/install-lab/resolve_plan",
        json!({
            "root_url": fixture_path("pkg-broken-manifest"),
            "strict_conformance": true,
        }),
    )
    .await
    .expect_err("strict mode should block broken manifest");
    anyhow::ensure!(
        err.to_string().contains("fails v1 conformance"),
        "unexpected error: {err}"
    );

    let out = invoke(
        &rt,
        "plurora/install-lab/resolve_plan",
        json!({
            "root_url": fixture_path("pkg-broken-manifest"),
            "strict_conformance": false,
        }),
    )
    .await?;
    let report = &out.output["plan"]["packages"][0]["conformance"];
    anyhow::ensure!(report["summary"]["failed"].as_u64().unwrap_or(0) > 0);
    Ok(())
}

pub(crate) async fn lenient_conformance_warns_not_blocks() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let out = invoke(
        &rt,
        "plurora/install-lab/resolve_plan",
        json!({
            "root_url": fixture_path("pkg-broken-manifest"),
            "strict_conformance": false,
        }),
    )
    .await?;
    let report = &out.output["plan"]["packages"][0]["conformance"];
    anyhow::ensure!(report["summary"]["failed"].as_u64().unwrap_or(0) > 0);
    Ok(())
}

pub(crate) async fn transitive_conformance_propagates() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let out = invoke(
        &rt,
        "plurora/install-lab/resolve_plan",
        json!({
            "root_url": fixture_path("pkg-a-broken-dep"),
            "strict_conformance": false,
        }),
    )
    .await?;
    let packages = out.output["plan"]["packages"]
        .as_array()
        .context("packages")?;
    let dep = packages
        .iter()
        .find(|pkg| pkg["id"] == json!("fixture/pkg-broken-manifest"))
        .context("broken dependency package")?;
    anyhow::ensure!(
        dep["conformance"]["summary"]["failed"]
            .as_u64()
            .unwrap_or(0)
            > 0
    );
    Ok(())
}

pub(crate) async fn resolve_plan_with_transitive() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let out = invoke(
        &rt,
        "plurora/install-lab/resolve_plan",
        json!({ "root_url": fixture_path("pkg-a") }),
    )
    .await?;
    let packages = out.output["plan"]["packages"]
        .as_array()
        .context("packages")?;
    let ids: Vec<_> = packages
        .iter()
        .filter_map(|pkg| pkg["id"].as_str())
        .collect();
    anyhow::ensure!(ids.contains(&"fixture/pkg-a"));
    anyhow::ensure!(ids.contains(&"fixture/pkg-b"));
    Ok(())
}

pub(crate) async fn resolve_plan_cycle_detection() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let err = invoke(
        &rt,
        "plurora/install-lab/resolve_plan",
        json!({ "root_url": fixture_path("pkg-cycle-a") }),
    )
    .await
    .expect_err("cycle should fail");
    anyhow::ensure!(err.to_string().contains("cycle"), "unexpected error: {err}");
    Ok(())
}

pub(crate) async fn project_root_install_registers_surface_dist() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let tmp = TempDir::new()?;
    let out = invoke(
        &rt,
        "plurora/install-lab/resolve_plan",
        json!({ "root_url": fixture_path("project-root") }),
    )
    .await?;
    let plan = take_plan(out)?;
    let packages = plan["packages"].as_array().context("packages")?;
    anyhow::ensure!(packages.len() == 2);
    anyhow::ensure!(packages.iter().any(|pkg| {
        pkg["id"] == json!("fixture/project-engine")
            && pkg["manifest_relative_path"] == json!("packages/engine/manifest.yaml")
    }));
    anyhow::ensure!(packages.iter().any(|pkg| {
        pkg["id"] == json!("fixture/project-surface")
            && pkg["manifest_relative_path"] == json!("packages/surface/manifest.yaml")
    }));
    anyhow::ensure!(
        plan["project_descriptor"]["project"]["id"] == json!("fixture-project__abc12345")
    );

    fs::create_dir_all(tmp.path().join("profiles"))?;
    fs::write(
        tmp.path().join("profiles/default.yaml"),
        "title: Existing profile\nevent_store:\n  kind: sqlite\n  path: /tmp/events.sqlite\nsecret_resolver:\n  store_enabled: true\nautoload:\n  - /existing/package/manifest.yaml\n",
    )?;
    let executed = execute_with_full_consent(&rt, plan, tmp.path()).await?;
    anyhow::ensure!(executed.output["project"]["project_id"] == json!("fixture-project__abc12345"));
    anyhow::ensure!(tmp
        .path()
        .join("projects/fixture-project__abc12345/project.yaml")
        .is_file());
    anyhow::ensure!(tmp
        .path()
        .join("projects/fixture-project__abc12345/dist/bundle.mjs")
        .is_file());
    let profile = fs::read_to_string(tmp.path().join("profiles/default.yaml"))?;
    anyhow::ensure!(profile.contains("event_store:"));
    anyhow::ensure!(profile.contains("secret_resolver:"));
    anyhow::ensure!(profile.contains("/existing/package/manifest.yaml"));
    anyhow::ensure!(profile.contains("packages/engine/manifest.yaml"));
    anyhow::ensure!(profile.contains("packages/surface/manifest.yaml"));
    let checked = invoke(
        &rt,
        "plurora/install-lab/check_lockfile",
        json!({ "data_dir": tmp.path() }),
    )
    .await?;
    anyhow::ensure!(checked.output["ok"] == json!(true));

    let lockfile_path = tmp.path().join("profiles/default.lock.toml");
    let lockfile_text = fs::read_to_string(&lockfile_path)?;
    let lockfile: plurora_core::Lockfile = toml::from_str(&lockfile_text)?;
    anyhow::ensure!(lockfile.package.iter().all(|entry| {
        entry.package_envelope_digest.is_some() && entry.component_pins.len() == 1
    }));
    let mut pin_drift = lockfile.clone();
    pin_drift.package[0].component_pins[0].digest = format!("sha256:{}", "f".repeat(64));
    fs::write(&lockfile_path, toml::to_string_pretty(&pin_drift)?)?;
    let checked = invoke(
        &rt,
        "plurora/install-lab/check_lockfile",
        json!({ "data_dir": tmp.path() }),
    )
    .await?;
    anyhow::ensure!(checked.output["ok"] == json!(false));
    anyhow::ensure!(checked.output["drift"]
        .as_array()
        .context("component drift array")?
        .iter()
        .any(|entry| entry["kind"] == json!("component_pins")));
    fs::write(&lockfile_path, &lockfile_text)?;
    let surface_store = lockfile
        .package
        .iter()
        .find(|entry| entry.id == "fixture/project-surface")
        .map(|entry| entry.installed_at_store.as_str())
        .context("surface installed_at_store in lockfile")?;
    fs::write(
        Path::new(&surface_store).join("packages/surface/dist/bundle.mjs"),
        "tampered",
    )?;
    let checked = invoke(
        &rt,
        "plurora/install-lab/check_lockfile",
        json!({ "data_dir": tmp.path() }),
    )
    .await?;
    anyhow::ensure!(checked.output["ok"] == json!(false));
    let drift = checked.output["drift"].as_array().context("drift array")?;
    anyhow::ensure!(drift
        .iter()
        .any(|entry| entry["kind"] == json!("surface_bundle_hash")));
    Ok(())
}

pub(crate) async fn execute_plan_local() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let tmp = TempDir::new()?;
    let plan = plan_for(&rt, "pkg-local").await?;
    let out = execute_with_full_consent(&rt, plan, tmp.path()).await?;
    anyhow::ensure!(Path::new(
        out.output["installed"][0]["store_path"]
            .as_str()
            .context("store_path")?
    )
    .is_dir());
    anyhow::ensure!(Path::new(
        out.output["profile_path"]
            .as_str()
            .context("profile_path")?
    )
    .is_file());
    anyhow::ensure!(Path::new(
        out.output["lockfile_path"]
            .as_str()
            .context("lockfile_path")?
    )
    .is_file());
    anyhow::ensure!(out.output["lockfile"]
        .as_str()
        .unwrap_or("")
        .contains("fixture/pkg-local"));
    let lockfile_text = out.output["lockfile"]
        .as_str()
        .context("serialized lockfile")?;
    let mut lockfile: plurora_core::Lockfile = toml::from_str(lockfile_text)?;
    lockfile.package[0].component_pins[0].digest = format!("sha256:{}", "e".repeat(64));
    let replanned = invoke(
        &rt,
        "plurora/install-lab/resolve_plan",
        json!({
            "root_url": fixture_path("pkg-local"),
            "lockfile": toml::to_string_pretty(&lockfile)?,
        }),
    )
    .await?;
    anyhow::ensure!(
        replanned.output["plan"]["integrity_summary"]["drift_detected"]
            .as_array()
            .context("resolve-plan drift")?
            .iter()
            .any(|entry| entry["kind"] == json!("component_pins"))
    );
    Ok(())
}

pub(crate) async fn execute_plan_consent_mismatch() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let tmp = TempDir::new()?;
    let plan = plan_for(&rt, "pkg-local").await?;
    let mut input = serde_json::Map::with_capacity(3);
    input.insert("plan".to_string(), plan);
    input.insert(
        "consent".to_string(),
        json!({ "approved_capabilities": [], "approved_network_hosts": [], "approved_secret_refs": [] }),
    );
    input.insert(
        "data_dir".to_string(),
        Value::String(tmp.path().to_string_lossy().into_owned()),
    );
    let err = invoke(
        &rt,
        "plurora/install-lab/execute_plan",
        Value::Object(input),
    )
    .await
    .expect_err("consent mismatch should fail");
    anyhow::ensure!(err.to_string().contains("consent missing"));
    Ok(())
}

pub(crate) async fn uninstall_removes_from_profile() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let tmp = TempDir::new()?;
    let plan = plan_for(&rt, "pkg-local").await?;
    execute_with_full_consent(&rt, plan, tmp.path()).await?;
    let out = invoke(
        &rt,
        "plurora/install-lab/uninstall",
        json!({ "package_id": "fixture/pkg-local", "data_dir": tmp.path() }),
    )
    .await?;
    anyhow::ensure!(out.output["removed_from_profile"] == json!(true));
    let profile = fs::read_to_string(tmp.path().join("profiles/default.yaml"))?;
    anyhow::ensure!(!profile.contains("pkg-local"));

    let plan = plan_for(&rt, "project-root").await?;
    execute_with_full_consent(&rt, plan, tmp.path()).await?;
    let out = invoke(
        &rt,
        "plurora/install-lab/uninstall",
        json!({ "project_id": "fixture-project__abc12345", "data_dir": tmp.path() }),
    )
    .await?;
    anyhow::ensure!(out.output["removed_from_profile"] == json!(true));
    anyhow::ensure!(out.output["project"]["data_action"] == json!("archived"));
    let profile = fs::read_to_string(tmp.path().join("profiles/default.yaml"))?;
    anyhow::ensure!(!profile.contains("packages/engine/manifest.yaml"));
    anyhow::ensure!(!profile.contains("packages/surface/manifest.yaml"));
    anyhow::ensure!(!tmp
        .path()
        .join("projects/fixture-project__abc12345")
        .exists());
    anyhow::ensure!(tmp
        .path()
        .join("projects/.archived/fixture-project__abc12345/project.yaml")
        .is_file());
    Ok(())
}

pub(crate) async fn list_installed_reflects_lockfile() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let tmp = TempDir::new()?;
    let plan = plan_for(&rt, "pkg-local").await?;
    execute_with_full_consent(&rt, plan, tmp.path()).await?;
    let out = invoke(
        &rt,
        "plurora/install-lab/list_installed",
        json!({ "data_dir": tmp.path() }),
    )
    .await?;
    let packages = out.output["packages"].as_array().context("packages")?;
    anyhow::ensure!(packages
        .iter()
        .any(|pkg| pkg["id"] == json!("fixture/pkg-local")));
    Ok(())
}

pub(crate) async fn check_lockfile_drift_detection() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let tmp = TempDir::new()?;
    let plan = plan_for(&rt, "pkg-local").await?;
    let out = execute_with_full_consent(&rt, plan, tmp.path()).await?;
    let store = PathBuf::from(
        out.output["installed"][0]["store_path"]
            .as_str()
            .context("store_path")?,
    );
    fs::write(store.join("tamper.txt"), "changed")?;
    let checked = invoke(
        &rt,
        "plurora/install-lab/check_lockfile",
        json!({ "data_dir": tmp.path() }),
    )
    .await?;
    anyhow::ensure!(checked.output["ok"] == json!(false));
    anyhow::ensure!(!checked.output["drift"]
        .as_array()
        .context("drift")?
        .is_empty());
    Ok(())
}

pub(crate) async fn check_for_updates_local_dangling_unsupported() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let tmp = TempDir::new()?;
    let source = tmp.path().join("local-source");
    fs::create_dir_all(&source)?;
    fs::write(
        source.join("manifest.yaml"),
        "schema_version: 1\nid: fixture/update-local\nversion: 0.1.0\nentry:\n  kind: rust_inproc\n  crate_ref: fixture\n  contract: v1\n  symbol: register\n  abi_version: 1\nprovides: []\npermissions: {}\n",
    )?;
    fs::write(source.join("content.txt"), "one")?;
    let plan = take_plan(
        invoke(
            &rt,
            "plurora/install-lab/resolve_plan",
            json!({ "root_url": source }),
        )
        .await?,
    )?;
    let executed = execute_with_full_consent(&rt, plan, tmp.path()).await?;
    let store = PathBuf::from(
        executed.output["installed"][0]["store_path"]
            .as_str()
            .context("store path")?,
    );

    let current = invoke(
        &rt,
        "plurora/install-lab/check_for_updates",
        json!({ "data_dir": tmp.path(), "package_id": "fixture/update-local" }),
    )
    .await?;
    anyhow::ensure!(current.output["results"][0]["status"] == json!("current"));

    fs::write(source.join("content.txt"), "two")?;
    let changed = invoke(
        &rt,
        "plurora/install-lab/check_for_updates",
        json!({ "data_dir": tmp.path(), "package_id": "fixture/update-local" }),
    )
    .await?;
    anyhow::ensure!(changed.output["results"][0]["status"] == json!("update_available"));
    anyhow::ensure!(changed.output["results"][0]["available"] == json!(true));

    let lock_path = tmp.path().join("profiles/default.lock.toml");
    let mut lock: plurora_core::Lockfile = toml::from_str(&fs::read_to_string(&lock_path)?)?;
    let saved_source_path = lock.package[0].source_path.take();
    fs::write(&lock_path, toml::to_string_pretty(&lock)?)?;
    let missing_source_path = invoke(
        &rt,
        "plurora/install-lab/check_for_updates",
        json!({ "data_dir": tmp.path(), "package_id": "fixture/update-local" }),
    )
    .await?;
    anyhow::ensure!(missing_source_path.output["results"][0]["status"] == json!("not_applicable"));
    anyhow::ensure!(missing_source_path.output["results"][0]["available"] == json!(false));
    anyhow::ensure!(missing_source_path.output["results"][0]["source_kind"] == json!("local"));
    anyhow::ensure!(missing_source_path.output["results"][0]["reason"]
        .as_str()
        .unwrap_or_default()
        .contains("source_path"));

    lock.package[0].source_path = saved_source_path;
    fs::write(&lock_path, toml::to_string_pretty(&lock)?)?;

    fs::remove_dir_all(&store)?;
    let dangling = invoke(
        &rt,
        "plurora/install-lab/check_for_updates",
        json!({ "data_dir": tmp.path(), "package_id": "fixture/update-local" }),
    )
    .await?;
    anyhow::ensure!(dangling.output["results"][0]["status"] == json!("repair_required"));
    anyhow::ensure!(dangling.output["results"][0]["dangling"] == json!(true));

    let mut lock: plurora_core::Lockfile = toml::from_str(&fs::read_to_string(&lock_path)?)?;
    lock.package[0].id = "plurora/internal-fixture".to_string();
    lock.package[0].source = plurora_core::LockSource::Internal;
    lock.package[0].manifest_relative_path = None;
    fs::write(&lock_path, toml::to_string_pretty(&lock)?)?;
    let unsupported = invoke(
        &rt,
        "plurora/install-lab/check_for_updates",
        json!({ "data_dir": tmp.path(), "package_id": "plurora/internal-fixture" }),
    )
    .await?;
    anyhow::ensure!(unsupported.output["results"][0]["status"] == json!("not_applicable"));
    anyhow::ensure!(unsupported.output["results"][0]["applicable"] == json!(false));
    Ok(())
}

pub(crate) async fn check_for_updates_external_project_not_applicable() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let tmp = TempDir::new()?;
    let source = tmp.path().join("external-source");
    fs::create_dir_all(&source)?;
    let project_id = "external_updates__abc123";
    let project_dir = tmp.path().join("projects").join(project_id);
    fs::create_dir_all(&project_dir)?;
    fs::write(
        project_dir.join("project.yaml"),
        format!(
            "schema_version: 1\nproject:\n  id: {project_id}\n  title: External Updates\n  type: external_workspace\n  packages: []\n  external:\n    source: {}\n    workspace_root: {}\n",
            source.display(),
            source.display()
        ),
    )?;

    let checked = invoke(
        &rt,
        "plurora/install-lab/check_for_updates",
        json!({ "data_dir": tmp.path(), "project_id": project_id }),
    )
    .await?;
    let results = checked.output["results"].as_array().context("results")?;
    anyhow::ensure!(results.len() == 1);
    anyhow::ensure!(results[0]["status"] == json!("not_applicable"));
    anyhow::ensure!(results[0]["available"] == json!(false));
    anyhow::ensure!(results[0]["project_id"] == json!(project_id));
    anyhow::ensure!(results[0]["source_kind"] == json!("external_workspace"));
    Ok(())
}

pub(crate) async fn update_project_local_replaces_dist_and_lockfile() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let tmp = TempDir::new()?;
    let source = tmp.path().join("project-source");
    copy_dir_all(Path::new(&fixture_path("project-root")), &source)?;

    let plan = take_plan(
        invoke(
            &rt,
            "plurora/install-lab/resolve_plan",
            json!({ "root_url": source }),
        )
        .await?,
    )?;
    execute_with_full_consent(&rt, plan, tmp.path()).await?;
    let lock_path = tmp.path().join("profiles/default.lock.toml");
    let before_lock = fs::read_to_string(&lock_path)?;
    let before: plurora_core::Lockfile = toml::from_str(&before_lock)?;
    let before_surface = before
        .package
        .iter()
        .find(|entry| entry.id == "fixture/project-surface")
        .context("surface entry before update")?;
    let before_tree_hash = before_surface.tree_hash.clone();
    let before_store = before_surface.installed_at_store.clone();

    fs::write(
        source.join("packages/surface/dist/bundle.mjs"),
        "export const updated = true;\n",
    )?;
    let updated = invoke(
        &rt,
        "plurora/install-lab/update_project",
        json!({ "data_dir": tmp.path(), "project_id": "fixture-project__abc12345" }),
    )
    .await?;
    anyhow::ensure!(updated.output["status"] == json!("updated"));
    anyhow::ensure!(updated.output["updated_packages"]
        .as_array()
        .context("updated packages")?
        .iter()
        .any(|id| id == "fixture/project-surface"));

    let after: plurora_core::Lockfile = toml::from_str(&fs::read_to_string(&lock_path)?)?;
    let after_surface = after
        .package
        .iter()
        .find(|entry| entry.id == "fixture/project-surface")
        .context("surface entry after update")?;
    anyhow::ensure!(after_surface.tree_hash != before_tree_hash);
    anyhow::ensure!(after_surface.installed_at_store != before_store);
    anyhow::ensure!(tmp
        .path()
        .join("projects/fixture-project__abc12345/dist/bundle.mjs")
        .is_file());
    let dist = fs::read_to_string(
        tmp.path()
            .join("projects/fixture-project__abc12345/dist/bundle.mjs"),
    )?;
    anyhow::ensure!(dist.contains("updated = true"));
    anyhow::ensure!(!Path::new(&before_store).exists());
    Ok(())
}

pub(crate) async fn update_project_local_current_noop() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let tmp = TempDir::new()?;
    let source = tmp.path().join("project-source");
    copy_dir_all(Path::new(&fixture_path("project-root")), &source)?;
    let plan = take_plan(
        invoke(
            &rt,
            "plurora/install-lab/resolve_plan",
            json!({ "root_url": source }),
        )
        .await?,
    )?;
    execute_with_full_consent(&rt, plan, tmp.path()).await?;
    let before = fs::read_to_string(tmp.path().join("profiles/default.lock.toml"))?;
    let out = invoke(
        &rt,
        "plurora/install-lab/update_project",
        json!({ "data_dir": tmp.path(), "project_id": "fixture-project__abc12345" }),
    )
    .await?;
    anyhow::ensure!(out.output["status"] == json!("current"));
    let after = fs::read_to_string(tmp.path().join("profiles/default.lock.toml"))?;
    anyhow::ensure!(after == before);
    Ok(())
}

pub(crate) async fn update_project_local_force_reinstalls_current() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let tmp = TempDir::new()?;
    let source = tmp.path().join("project-source");
    copy_dir_all(Path::new(&fixture_path("project-root")), &source)?;
    let plan = take_plan(
        invoke(
            &rt,
            "plurora/install-lab/resolve_plan",
            json!({ "root_url": source }),
        )
        .await?,
    )?;
    execute_with_full_consent(&rt, plan, tmp.path()).await?;
    let out = invoke(
        &rt,
        "plurora/install-lab/update_project",
        json!({ "data_dir": tmp.path(), "project_id": "fixture-project__abc12345", "force": true }),
    )
    .await?;
    anyhow::ensure!(out.output["status"] == json!("updated"));
    anyhow::ensure!(out.output["updated"] == json!(true));
    anyhow::ensure!(!out.output["updated_packages"]
        .as_array()
        .context("updated packages")?
        .is_empty());
    Ok(())
}

pub(crate) async fn update_project_external_not_applicable() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let tmp = TempDir::new()?;
    let source = tmp.path().join("external-source");
    fs::create_dir_all(&source)?;
    let project_id = "external_update_project__abc123";
    let project_dir = tmp.path().join("projects").join(project_id);
    fs::create_dir_all(&project_dir)?;
    fs::write(
        project_dir.join("project.yaml"),
        format!(
            "schema_version: 1\nproject:\n  id: {project_id}\n  title: External Updates\n  type: external_workspace\n  packages: []\n  external:\n    source: {}\n    workspace_root: {}\n",
            source.display(),
            source.display()
        ),
    )?;
    let out = invoke(
        &rt,
        "plurora/install-lab/update_project",
        json!({ "data_dir": tmp.path(), "project_id": project_id }),
    )
    .await?;
    anyhow::ensure!(out.output["status"] == json!("not_applicable"));
    anyhow::ensure!(out.output["updated"] == json!(false));
    Ok(())
}

pub(crate) async fn update_project_permission_drift_blocks_before_mutation() -> anyhow::Result<()> {
    let rt = load_install_lab().await?;
    let tmp = TempDir::new()?;
    let source = tmp.path().join("perm-source");
    fs::create_dir_all(&source)?;
    fs::write(
        source.join("manifest.yaml"),
        "schema_version: 1\nid: fixture/perm-update\nversion: 0.1.0\nentry:\n  kind: rust_inproc\n  crate_ref: fixture\n  contract: v1\n  symbol: register\n  abi_version: 1\nprovides: []\npermissions: {}\n",
    )?;
    fs::write(source.join("content.txt"), "one")?;
    let plan = take_plan(
        invoke(
            &rt,
            "plurora/install-lab/resolve_plan",
            json!({ "root_url": source }),
        )
        .await?,
    )?;
    execute_with_full_consent(&rt, plan, tmp.path()).await?;
    let lock_path = tmp.path().join("profiles/default.lock.toml");
    let before_lock = fs::read_to_string(&lock_path)?;

    fs::write(
        source.join("manifest.yaml"),
        "schema_version: 1\nid: fixture/perm-update\nversion: 0.1.0\nentry:\n  kind: rust_inproc\n  crate_ref: fixture\n  contract: v1\n  symbol: register\n  abi_version: 1\nprovides: []\npermissions:\n  capabilities:\n    invoke:\n      - plurora/other/*\n",
    )?;
    fs::write(source.join("content.txt"), "two")?;
    let err = invoke(
        &rt,
        "plurora/install-lab/update_project",
        json!({ "data_dir": tmp.path(), "package_id": "fixture/perm-update" }),
    )
    .await
    .expect_err("permission drift should block update");
    anyhow::ensure!(
        err.to_string().contains("new capability"),
        "unexpected error: {err}"
    );
    let after_lock = fs::read_to_string(&lock_path)?;
    anyhow::ensure!(after_lock == before_lock);
    Ok(())
}

async fn plan_for(
    runtime: &plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>,
    fixture: &str,
) -> anyhow::Result<Value> {
    take_plan(
        invoke(
            runtime,
            "plurora/install-lab/resolve_plan",
            json!({ "root_url": fixture_path(fixture) }),
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
    data_dir: &Path,
) -> anyhow::Result<plurora_runtime::CapabilityInvocationResult> {
    let summary = &plan["permissions_summary"];
    let consent = json!({
        "approved_capabilities": summary["new_capabilities"].clone(),
        "approved_network_hosts": summary["new_network_hosts"].clone(),
        "approved_secret_refs": summary["new_secret_refs"].clone(),
    });
    let mut input = serde_json::Map::with_capacity(3);
    input.insert("plan".to_string(), plan);
    input.insert("consent".to_string(), consent);
    input.insert(
        "data_dir".to_string(),
        Value::String(data_dir.to_string_lossy().into_owned()),
    );
    invoke(
        runtime,
        "plurora/install-lab/execute_plan",
        Value::Object(input),
    )
    .await
}

fn fixture_path(name: &str) -> String {
    format!(
        "{}",
        PathBuf::from("crates/plurora-cli/src/conformance/fixtures/install")
            .join(name)
            .display()
    )
}

fn copy_dir_all(src: &Path, dest: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
