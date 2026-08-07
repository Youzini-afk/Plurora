//! Conformance tests for `plurora/secret-store-lab`.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use plurora_runtime::{CapabilityInvocationRequest, HostSecretResolver, StoreSecretResolver};
use serde_json::json;
use tempfile::TempDir;
use tokio::sync::Mutex;

use super::fixtures::*;
use crate::cli::HostProfile;
use crate::commands::{host::runtime_config_from_profile, manifest};

const MANIFEST_PATH: &str = "packages/plurora/secret-store-lab/manifest.yaml";
const PACKAGE_ID: &str = "plurora/secret-store-lab";

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

struct DataDirGuard {
    previous: Option<String>,
    _tmp: TempDir,
}

impl DataDirGuard {
    fn new() -> anyhow::Result<Self> {
        let tmp = tempfile::tempdir()?;
        let previous = std::env::var("PLURORA_DATA_DIR").ok();
        std::env::set_var("PLURORA_DATA_DIR", tmp.path().display().to_string());
        Ok(Self {
            previous,
            _tmp: tmp,
        })
    }
}

impl Drop for DataDirGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("PLURORA_DATA_DIR", value),
            None => std::env::remove_var("PLURORA_DATA_DIR"),
        }
    }
}

async fn load_secret_store_lab(
) -> anyhow::Result<plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(manifest::read_manifest(PathBuf::from(MANIFEST_PATH)).await?)
        .await?;
    Ok(runtime)
}

async fn invoke(
    runtime: &plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>,
    cap: &str,
    input: serde_json::Value,
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

pub(crate) async fn put_then_has_succeeds() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = load_secret_store_lab().await?;

    let put = invoke(
        &rt,
        "plurora/secret-store-lab/put_secret",
        json!({ "name": "OPENAI_API_KEY", "value": "synthetic-test-value-12345" }),
    )
    .await?;
    anyhow::ensure!(put.output["stored"] == json!(true));
    anyhow::ensure!(put.output["created"] == json!(true));

    let has = invoke(
        &rt,
        "plurora/secret-store-lab/has_secret",
        json!({ "name": "OPENAI_API_KEY" }),
    )
    .await?;
    anyhow::ensure!(has.output["exists"] == json!(true));
    Ok(())
}

pub(crate) async fn list_returns_names_not_values() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = load_secret_store_lab().await?;

    invoke(
        &rt,
        "plurora/secret-store-lab/put_secret",
        json!({ "name": "KEY_ONE", "value": "synthetic-test-value-one" }),
    )
    .await?;
    invoke(
        &rt,
        "plurora/secret-store-lab/put_secret",
        json!({ "name": "KEY_TWO", "value": "synthetic-test-value-two" }),
    )
    .await?;
    let list = invoke(&rt, "plurora/secret-store-lab/list_secrets", json!({})).await?;
    let text = serde_json::to_string(&list.output)?;
    anyhow::ensure!(text.contains("KEY_ONE"));
    anyhow::ensure!(text.contains("KEY_TWO"));
    anyhow::ensure!(!text.contains("synthetic-test-value-one"));
    anyhow::ensure!(!text.contains("synthetic-test-value-two"));
    Ok(())
}

pub(crate) async fn delete_removes() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = load_secret_store_lab().await?;

    invoke(
        &rt,
        "plurora/secret-store-lab/put_secret",
        json!({ "name": "DELETE_ME", "value": "synthetic-test-value-delete" }),
    )
    .await?;
    let deleted = invoke(
        &rt,
        "plurora/secret-store-lab/delete_secret",
        json!({ "name": "DELETE_ME" }),
    )
    .await?;
    anyhow::ensure!(deleted.output["removed"] == json!(true));
    let has = invoke(
        &rt,
        "plurora/secret-store-lab/has_secret",
        json!({ "name": "DELETE_ME" }),
    )
    .await?;
    anyhow::ensure!(has.output["exists"] == json!(false));
    Ok(())
}

pub(crate) async fn put_invalid_name_rejected() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = load_secret_store_lab().await?;

    for name in ["", "HAS SPACE", &"A".repeat(129)] {
        let result = invoke(
            &rt,
            "plurora/secret-store-lab/put_secret",
            json!({ "name": name, "value": "synthetic-test-value-invalid" }),
        )
        .await;
        anyhow::ensure!(result.is_err(), "invalid name should be rejected: {name:?}");
    }
    Ok(())
}

pub(crate) async fn put_oversized_value_rejected() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = load_secret_store_lab().await?;

    let result = invoke(
        &rt,
        "plurora/secret-store-lab/put_secret",
        json!({ "name": "TOO_LARGE", "value": "x".repeat(16 * 1024 + 1) }),
    )
    .await;
    anyhow::ensure!(result.is_err());
    Ok(())
}

pub(crate) async fn health_reports_layout() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = load_secret_store_lab().await?;

    let health = invoke(&rt, "plurora/secret-store-lab/health", json!({})).await?;
    anyhow::ensure!(health.output["store_path"]
        .as_str()
        .unwrap_or("")
        .ends_with("secrets.dat"));
    anyhow::ensure!(health.output["key_source"].as_str().is_some());
    Ok(())
}

pub(crate) async fn resolver_resolves_existing() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = load_secret_store_lab().await?;
    invoke(
        &rt,
        "plurora/secret-store-lab/put_secret",
        json!({ "name": "RESOLVE_ME", "value": "synthetic-test-value-resolve" }),
    )
    .await?;

    let resolver = StoreSecretResolver::new()?;
    let resolved = resolver.resolve("secret_ref:store:RESOLVE_ME").await?;
    anyhow::ensure!(resolved == "synthetic-test-value-resolve");
    Ok(())
}

pub(crate) async fn resolver_missing_name_fails_closed() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let resolver = StoreSecretResolver::new()?;
    let err = resolver
        .resolve("secret_ref:store:MISSING_NAME")
        .await
        .expect_err("missing secret should fail");
    anyhow::ensure!(err.to_string().contains("not in store"));
    Ok(())
}

pub(crate) async fn resolver_non_store_ref_rejected() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let resolver = StoreSecretResolver::new()?;
    let err = resolver
        .resolve("secret_ref:env:OPENAI_API_KEY")
        .await
        .expect_err("env ref should be rejected");
    anyhow::ensure!(err.to_string().contains("not a store-backed reference"));
    Ok(())
}

pub(crate) async fn resolver_error_does_not_leak_value() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let rt = load_secret_store_lab().await?;
    invoke(
        &rt,
        "plurora/secret-store-lab/put_secret",
        json!({ "name": "NO_LEAK", "value": "synthetic-test-value-no-leak" }),
    )
    .await?;
    let resolver = StoreSecretResolver::new()?;
    let err = resolver
        .resolve("secret_ref:store:NOT_PRESENT")
        .await
        .context("unexpected resolution success")
        .expect_err("missing name should fail");
    anyhow::ensure!(!err.to_string().contains("synthetic-test-value-no-leak"));
    Ok(())
}

pub(crate) async fn host_profile_installs_composite_resolver() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let _guard = DataDirGuard::new()?;
    let profile: HostProfile = serde_yaml::from_str("title: conformance\n")?;
    let config = runtime_config_from_profile(&profile)?;
    let runtime = plurora_runtime::Runtime::new(
        Arc::new(plurora_runtime::InMemoryEventStore::default()),
        config,
    );

    let err = runtime
        .resolve_secret_ref("secret_ref:store:NONEXISTENT")
        .await
        .expect_err("missing store secret should fail");
    let msg = err.to_string();
    anyhow::ensure!(
        msg.contains("not in store"),
        "host profile should install store resolver, got: {msg}"
    );
    anyhow::ensure!(
        !msg.contains("no secret resolver configured"),
        "host profile must not leave DenyAll resolver installed: {msg}"
    );
    Ok(())
}
