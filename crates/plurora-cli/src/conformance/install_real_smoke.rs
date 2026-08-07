//! Opt-in real GitHub smoke test for package-install git connectivity.
//!
//! Gated behind `PLURORA_GIT_INSTALL_REAL_TESTS=1` (default skipped). This case
//! exercises the real `plurora/git-tools-lab` remote paths against a small
//! public GitHub repository without adding a default network dependency to the
//! conformance suite.

use std::path::PathBuf;

use anyhow::Context;
use plurora_runtime::CapabilityInvocationRequest;
use serde_json::{json, Value};

use super::fixtures::runtime;
use crate::commands::manifest;

const GIT_MANIFEST: &str = "packages/plurora/git-tools-lab/manifest.yaml";
const PACKAGE_ID: &str = "plurora/git-tools-lab";
const REMOTE_URL: &str = "https://github.com/octocat/Hello-World";
const REF_NAME: &str = "master";

pub(crate) async fn real_github_smoke() -> anyhow::Result<()> {
    if std::env::var("PLURORA_GIT_INSTALL_REAL_TESTS").as_deref() != Ok("1") {
        println!("install.real_github_smoke SKIP  PLURORA_GIT_INSTALL_REAL_TESTS=1 not set");
        return Ok(());
    }

    let (_store, runtime) = runtime();
    runtime
        .load_package(manifest::read_manifest(PathBuf::from(GIT_MANIFEST)).await?)
        .await?;

    let resolved = invoke(
        &runtime,
        "plurora/git-tools-lab/resolve_ref",
        json!({ "remote_url": REMOTE_URL, "ref": REF_NAME }),
    )
    .await?;
    let commit_sha = resolved.output["commit_sha"]
        .as_str()
        .context("resolve_ref missing commit_sha")?;
    anyhow::ensure!(is_full_sha(commit_sha), "invalid commit_sha: {commit_sha}");
    anyhow::ensure!(
        resolved.output["ref_name"].as_str().unwrap_or_default() == "refs/heads/master",
        "resolve_ref returned unexpected ref_name: {}",
        resolved.output["ref_name"]
    );

    let refs = invoke(
        &runtime,
        "plurora/git-tools-lab/fetch_refs",
        json!({ "remote_url": REMOTE_URL }),
    )
    .await?;
    let refs = refs.output["refs"]
        .as_array()
        .context("fetch_refs missing refs")?;
    anyhow::ensure!(!refs.is_empty(), "fetch_refs returned no refs");
    anyhow::ensure!(
        refs.iter()
            .any(|reference| reference["name"] == json!("refs/heads/master")),
        "fetch_refs did not include refs/heads/master"
    );

    Ok(())
}

async fn invoke(
    runtime: &plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>,
    capability_id: &str,
    input: Value,
) -> anyhow::Result<plurora_runtime::CapabilityInvocationResult> {
    Ok(runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(capability_id.to_string()),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input,
        })
        .await?)
}

fn is_full_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
