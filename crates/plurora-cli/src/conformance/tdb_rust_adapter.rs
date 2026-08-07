use std::path::PathBuf;
use std::process::Command;

use plurora_runtime::CapabilityInvocationRequest;
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

const MANIFEST: &str = "examples/packages/tdb-rust-adapter/manifest.yaml";
const ADAPTER_MANIFEST: &str = "integrations/tdb/rust-adapter/Cargo.toml";
const REAL_ADAPTER_MANIFEST: &str = "integrations/tdb/rust-adapter-real-crate/Cargo.toml";

pub(crate) async fn subprocess_adapter_shell_invokes_disabled_smoke() -> anyhow::Result<()> {
    build_default_adapter()?;
    let (_store, runtime) = runtime();
    runtime
        .load_package(manifest::read_manifest(PathBuf::from(MANIFEST)).await?)
        .await?;

    let described = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/tdb-rust-adapter/describe_real_tdb_adapter".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/tdb-rust-adapter".to_string()),
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await?;
    anyhow::ensure!(described.output["kind"] == json!("real_tdb_rust_adapter"));
    anyhow::ensure!(described.output["real_tdb_available"] == json!(false));
    anyhow::ensure!(described.output["default_build"]["backend_opened"] == json!(false));

    let smoke = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/tdb-rust-adapter/run_real_tdb_smoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/tdb-rust-adapter".to_string()),
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await?;
    anyhow::ensure!(smoke.output["kind"] == json!("real_tdb_smoke_disabled"));
    anyhow::ensure!(smoke.output["smoke_executed"] == json!(false));
    anyhow::ensure!(smoke.output["backend_opened"] == json!(false));
    Ok(())
}

pub(crate) async fn subprocess_adapter_rejects_secret_and_raw_path() -> anyhow::Result<()> {
    build_default_adapter()?;
    let (_store, runtime) = runtime();
    runtime
        .load_package(manifest::read_manifest(PathBuf::from(MANIFEST)).await?)
        .await?;
    for input in [
        json!({"payload":"sk-test-value"}),
        json!({"store_path":"/tmp/private.tdb"}),
    ] {
        let denied = runtime
            .invoke_capability(CapabilityInvocationRequest {
                handle: None,
                capability_id: Some("plurora/tdb-rust-adapter/run_real_tdb_smoke".to_string()),
                caller_package_id: None,
                provider_package_id: Some("plurora/tdb-rust-adapter".to_string()),
                version: None,
                session_id: None,
                input,
            })
            .await;
        anyhow::ensure!(denied.is_err(), "unsafe TDB smoke input was accepted");
    }
    Ok(())
}

pub(crate) async fn real_crate_smoke_opt_in() -> anyhow::Result<()> {
    if std::env::var("PLURORA_TDB_REAL_TESTS").ok().as_deref() != Some("1") {
        return Ok(());
    }
    let status = Command::new("cargo")
        .args([
            "test",
            "--manifest-path",
            REAL_ADAPTER_MANIFEST,
            "--features",
            "real-tdb",
        ])
        .status()?;
    anyhow::ensure!(status.success(), "real crate TDB smoke test failed");
    Ok(())
}

fn build_default_adapter() -> anyhow::Result<()> {
    let status = Command::new("cargo")
        .args(["build", "--manifest-path", ADAPTER_MANIFEST])
        .status()?;
    anyhow::ensure!(status.success(), "default TDB adapter build failed");
    Ok(())
}
