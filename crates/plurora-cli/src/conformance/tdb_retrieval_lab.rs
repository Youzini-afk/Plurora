//! Conformance tests for `official/tdb-retrieval-lab`.
//!
//! TDB is treated as a future multimodal retrieval provider adapter, not a
//! kernel database, event-store replacement, or raw package backend.

use std::path::PathBuf;

use plurora_runtime::CapabilityInvocationRequest;
use serde_json::{json, Value};

use super::fixtures::*;
use crate::commands::manifest;

const PACKAGE_ID: &str = "official/tdb-retrieval-lab";

fn forbidden_platform_namespace_tokens() -> Vec<String> {
    [
        "tdb",
        "vector",
        "embedding",
        "collection",
        "sql",
        "database",
    ]
    .into_iter()
    .map(|segment| format!("platform.{segment}."))
    .collect()
}

async fn load_tdb_retrieval_lab(
) -> anyhow::Result<plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/official/tdb-retrieval-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
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
            capability_id: Some(format!("{PACKAGE_ID}/{cap}")),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input,
        })
        .await
        .map_err(Into::into)
}

pub(crate) async fn contract_shape() -> anyhow::Result<()> {
    let rt = load_tdb_retrieval_lab().await?;
    let out = invoke(&rt, "describe_tdb_retrieval_contract", json!({}))
        .await?
        .output;
    anyhow::ensure!(out["kind"] == json!("tdb_retrieval_contract"));
    anyhow::ensure!(out["package_kind"] == json!("ordinary_retrieval_provider_adapter"));
    anyhow::ensure!(out["red_lines"]["not_platform_database"] == json!(true));
    anyhow::ensure!(out["red_lines"]["no_real_tdb_crate_linkage_in_alpha"] == json!(true));
    anyhow::ensure!(out["capabilities"].as_array().map(|a| a.len()).unwrap_or(0) == 6);
    let serialized = serde_json::to_string(&out)?;
    for token in forbidden_platform_namespace_tokens() {
        anyhow::ensure!(
            !serialized.contains(&token),
            "forbidden namespace token leaked: {token}"
        );
    }
    Ok(())
}

pub(crate) async fn index_plan_no_execution() -> anyhow::Result<()> {
    let rt = load_tdb_retrieval_lab().await?;
    let out = invoke(
        &rt,
        "draft_tdb_index_plan",
        json!({"index_id": "demo/index", "asset_refs": ["asset/a", "asset/b"], "modality_hints": ["text", "image"]}),
    )
    .await?
    .output;
    anyhow::ensure!(out["kind"] == json!("tdb_index_plan"));
    anyhow::ensure!(out["plan_only"] == json!(true));
    anyhow::ensure!(out["tdb_opened"] == json!(false));
    anyhow::ensure!(out["index_created"] == json!(false));
    anyhow::ensure!(out["embedding_generated"] == json!(false));
    anyhow::ensure!(out["filesystem_performed"] == json!(false));
    Ok(())
}

pub(crate) async fn query_plan_no_execution() -> anyhow::Result<()> {
    let rt = load_tdb_retrieval_lab().await?;
    let out = invoke(
        &rt,
        "draft_tdb_query_plan",
        json!({"index_id": "demo/index", "query_modalities": ["text"], "limit": 128}),
    )
    .await?
    .output;
    anyhow::ensure!(out["kind"] == json!("tdb_query_plan"));
    anyhow::ensure!(out["limit"] == json!(32));
    anyhow::ensure!(out["search_executed"] == json!(false));
    anyhow::ensure!(out["tdb_opened"] == json!(false));
    anyhow::ensure!(out["vectors_loaded"] == json!(false));
    Ok(())
}

pub(crate) async fn backend_fit_boundary() -> anyhow::Result<()> {
    let rt = load_tdb_retrieval_lab().await?;
    let out = invoke(&rt, "explain_tdb_backend_fit", json!({}))
        .await?
        .output;
    anyhow::ensure!(out["kind"] == json!("tdb_backend_fit"));
    let not_fit = out["not_fit_for"].as_array().cloned().unwrap_or_default();
    anyhow::ensure!(not_fit.iter().any(|v| v == "event log authority"));
    anyhow::ensure!(out["real_backend_status"] == json!("future_opt_in_not_linked_in_alpha"));
    Ok(())
}

pub(crate) async fn invalid_input_rejected() -> anyhow::Result<()> {
    let rt = load_tdb_retrieval_lab().await?;
    let out = invoke(
        &rt,
        "draft_tdb_index_plan",
        json!({"index_id": "demo", "modality_hints": ["smell"]}),
    )
    .await?
    .output;
    anyhow::ensure!(out["kind"] == json!("tdb_retrieval_lab_rejected"));
    anyhow::ensure!(out["reason"] == json!("unsupported_modality"));

    let many: Vec<String> = (0..65).map(|i| format!("asset/{i}")).collect();
    let out = invoke(
        &rt,
        "draft_tdb_index_plan",
        json!({"index_id": "demo", "asset_refs": many}),
    )
    .await?
    .output;
    anyhow::ensure!(out["reason"] == json!("too_many_asset_refs"));
    Ok(())
}

pub(crate) async fn raw_secret_and_unsafe_id_rejected() -> anyhow::Result<()> {
    let rt = load_tdb_retrieval_lab().await?;
    let raw = ["s", "k-", "tdb-secret"].concat();
    let err = invoke(
        &rt,
        "draft_tdb_index_plan",
        json!({"index_id": "demo", "api_key": raw}),
    )
    .await;
    anyhow::ensure!(err.is_err(), "raw secret-like input must be rejected");

    let err = invoke(
        &rt,
        "draft_tdb_query_plan",
        json!({"index_id": "../private"}),
    )
    .await;
    anyhow::ensure!(err.is_err(), "unsafe id must be rejected");
    Ok(())
}

pub(crate) async fn real_tdb_opt_in_seam_crate_adapter_available() -> anyhow::Result<()> {
    let rt = load_tdb_retrieval_lab().await?;
    let out = invoke(&rt, "describe_real_tdb_opt_in_seam", json!({}))
        .await?
        .output;
    anyhow::ensure!(out["kind"] == json!("tdb_real_opt_in_seam"));
    anyhow::ensure!(out["status"] == json!("real_crate_adapter_available_opt_in"));
    anyhow::ensure!(out["current_alpha"]["path_dependency_committed"] == json!(false));
    anyhow::ensure!(out["current_alpha"]["tdb_crate_linked_by_default"] == json!(false));
    anyhow::ensure!(
        out["current_alpha"]["published_crate_adapter_manifest"]
            == json!("integrations/tdb/rust-adapter-real-crate/Cargo.toml")
    );
    anyhow::ensure!(out["current_alpha"]["backend_opened"] == json!(false));
    anyhow::ensure!(out["host_policy_requirements"]["store_path"] == json!("host_ref_only"));
    let modes = out["recommended_plurora_modes"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    anyhow::ensure!(modes
        .iter()
        .any(|mode| mode["mode"] == json!("subprocess_adapter_package")));
    anyhow::ensure!(modes
        .iter()
        .any(|mode| mode["mode"] == json!("feature_gated_inproc_adapter")));
    Ok(())
}
