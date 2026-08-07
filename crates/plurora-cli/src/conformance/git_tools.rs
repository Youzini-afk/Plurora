use std::fs;
use std::path::PathBuf;
use std::process::Command;

use plurora_runtime::CapabilityInvocationRequest;
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

const MANIFEST_PATH: &str = "packages/plurora/git-tools-lab/manifest.yaml";

pub(crate) async fn url_validation_https_only() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(manifest::read_manifest(PathBuf::from(MANIFEST_PATH)).await?)
        .await?;
    for remote_url in [
        "ssh://github.com/example/repo.git",
        "git://github.com/example/repo.git",
        "file:///tmp/repo.git",
    ] {
        let result = invoke(
            &runtime,
            "plurora/git-tools-lab/fetch_refs",
            json!({ "remote_url": remote_url }),
        )
        .await;
        anyhow::ensure!(result.is_err(), "non-HTTPS URL was accepted: {remote_url}");
    }
    Ok(())
}

pub(crate) async fn url_validation_no_userinfo() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(manifest::read_manifest(PathBuf::from(MANIFEST_PATH)).await?)
        .await?;
    let result = invoke(
        &runtime,
        "plurora/git-tools-lab/fetch_refs",
        json!({ "remote_url": "https://user:pass@example.com/repo.git" }),
    )
    .await;
    anyhow::ensure!(result.is_err(), "URL with userinfo was accepted");
    Ok(())
}

pub(crate) async fn path_validation_absolute() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(manifest::read_manifest(PathBuf::from(MANIFEST_PATH)).await?)
        .await?;
    let result = invoke(
        &runtime,
        "plurora/git-tools-lab/fetch_tree",
        json!({
            "remote_url": "https://example.com/repo.git",
            "commit_sha": "0123456789abcdef0123456789abcdef01234567",
            "dest_dir": "relative/path"
        }),
    )
    .await;
    anyhow::ensure!(result.is_err(), "relative dest_dir was accepted");
    Ok(())
}

pub(crate) async fn path_validation_no_traversal() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(manifest::read_manifest(PathBuf::from(MANIFEST_PATH)).await?)
        .await?;
    let result = invoke(
        &runtime,
        "plurora/git-tools-lab/fetch_tree",
        json!({
            "remote_url": "https://example.com/repo.git",
            "commit_sha": "0123456789abcdef0123456789abcdef01234567",
            "dest_dir": "/tmp/plurora-git/../escape"
        }),
    )
    .await;
    anyhow::ensure!(result.is_err(), "dest_dir with .. was accepted");
    Ok(())
}

pub(crate) async fn read_signed_tag_unsigned() -> anyhow::Result<()> {
    if std::env::var("PLURORA_GIT_REAL_TESTS").ok().as_deref() != Some("1") {
        return Ok(());
    }

    let tmp = tempfile::tempdir()?;
    let repo = tmp.path().join("repo");
    fs::create_dir(&repo)?;
    run_git(&repo, &["init"])?;
    run_git(&repo, &["config", "user.name", "Plurora Conformance"])?;
    run_git(&repo, &["config", "user.email", "plurora@example.com"])?;
    fs::write(repo.join("README.md"), "# fixture\n")?;
    run_git(&repo, &["add", "README.md"])?;
    run_git(&repo, &["commit", "-m", "fixture"])?;
    run_git(&repo, &["tag", "lightweight-fixture"])?;

    let (_store, runtime) = runtime();
    runtime
        .load_package(manifest::read_manifest(PathBuf::from(MANIFEST_PATH)).await?)
        .await?;
    let output = invoke(
        &runtime,
        "plurora/git-tools-lab/read_signed_tag",
        json!({
            "remote_url": format!("file://{}", repo.display()),
            "tag": "lightweight-fixture"
        }),
    )
    .await?;
    anyhow::ensure!(
        output.output["pgp_signature"].is_null(),
        "lightweight unsigned tag should have pgp_signature: null"
    );
    Ok(())
}

async fn invoke(
    runtime: &plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>,
    capability_id: &str,
    input: serde_json::Value,
) -> anyhow::Result<plurora_runtime::CapabilityInvocationResult> {
    Ok(runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(capability_id.to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/git-tools-lab".to_string()),
            version: None,
            session_id: None,
            input,
        })
        .await?)
}

fn run_git(cwd: &std::path::Path, args: &[&str]) -> anyhow::Result<()> {
    let status = Command::new("git").args(args).current_dir(cwd).status()?;
    anyhow::ensure!(status.success(), "git {:?} failed", args);
    Ok(())
}
