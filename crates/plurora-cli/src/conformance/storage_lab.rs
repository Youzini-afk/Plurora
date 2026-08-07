//! Conformance tests for `plurora/storage-lab` (Storage Backend Neutrality Alpha S2 + S3 + S4 + S5).
//!
//! Covers:
//! 1. Contract shape — no kernel database terms
//! 2. Backend classes — no secret-bearing backend config
//! 3. Package state plan — scoped namespace, no official priority
//! 4. Put document preview — no real write
//! 5. Get document preview — no real read
//! 6. Query prefix preview — no query execution
//! 7. Delete tombstone preview — no real delete
//! 8. Export snapshot preview — redacted
//! 9. Raw secret rejected
//! 10. Unsafe ID rejected
//! 11. Blob contract shape — content-addressed, red lines, no forbidden namespace
//! 12. Put blob preview — content address deterministic, no storage/content/event
//! 13. Get blob metadata preview — no content returned
//! 14. Export blob manifest preview — refs only, no content
//! 15. Blob raw secret and unsafe ID rejected
//! 16. Projection contract shape — backend candidates, red lines, no DB table leakage
//! 17. Projection materialization plan only — materialized=false, write_performed=false, backend_selected=false
//! 18. Projection query preview no execution — query_executed=false, rows_returned=false, no SQL/table/collection/vector terms
//! 19. Projection migration plan no rewrite — migration_applied=false, data_rewritten=false, requires_rebuild=true
//! 20. Projection rejects raw secret
//! 21. Projection no DB table leakage — no SQL/table/collection/vector/database in output
//! 22. Retrieval contract shape — backend candidates, red lines, no kernel vector namespace
//! 23. Multimodal index plan — no embedding, no storage, plan-only
//! 24. Multimodal index rejects invalid modality or too many refs
//! 25. Vector search plan — no execution, no embedding, plan-only
//! 26. Backend fit mentions TDB future only — no kernel vector namespace, no secret-bearing backend config
//! 27. Retrieval rejects raw secret
//! 28. Retrieval no kernel vector namespace or credentials

use std::path::PathBuf;

use plurora_runtime::CapabilityInvocationRequest;
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

const PACKAGE_ID: &str = "plurora/storage-lab";

fn forbidden_platform_namespace_tokens() -> Vec<String> {
    [
        "sqlite",
        "postgres",
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

async fn load_storage_lab(
) -> anyhow::Result<plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from("packages/plurora/storage-lab/manifest.yaml"))
                .await?,
        )
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

/// Case 1: Contract shape — 8 capabilities, 3 surfaces, ordinary package,
/// no reserved kernel database namespace terms.
pub(crate) async fn contract_shape_no_platform_database_terms() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let contract = invoke(&rt, "describe_storage_contract", json!({})).await?;

    anyhow::ensure!(
        contract.output["kind"] == json!("storage_lab_contract"),
        "describe_storage_contract must return storage_lab_contract kind"
    );
    anyhow::ensure!(
        contract.output["package_kind"] == json!("ordinary"),
        "must be ordinary package"
    );

    // 3 surfaces
    let surfaces = contract.output["surfaces"].as_object().unwrap();
    anyhow::ensure!(
        surfaces.contains_key("forge_panel"),
        "must have forge_panel"
    );
    anyhow::ensure!(
        surfaces.contains_key("assistant_action"),
        "must have assistant_action"
    );
    anyhow::ensure!(surfaces.contains_key("home_card"), "must have home_card");

    // 20 capabilities
    anyhow::ensure!(
        contract.output["capabilities"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
            == 20,
        "describe_storage_contract must list 20 capabilities"
    );

    // No kernel database terms
    let output_str = serde_json::to_string(&contract.output).unwrap();
    let forbidden = forbidden_platform_namespace_tokens();
    for token in &forbidden {
        anyhow::ensure!(
            !output_str.contains(token),
            "contract must not contain {}",
            token
        );
    }

    // No inference / no network
    anyhow::ensure!(contract.output["inference_performed"] == json!(false));
    anyhow::ensure!(contract.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 2: Backend classes — capability flags only, no secret-bearing backend config.
pub(crate) async fn backend_classes_no_secret_config() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let result = invoke(&rt, "describe_backend_classes", json!({})).await?;

    anyhow::ensure!(
        result.output["kind"] == json!("backend_classes"),
        "must return backend_classes kind"
    );

    let output_str = serde_json::to_string(&result.output).unwrap();
    let lower = output_str.to_lowercase();
    for token in &[
        &format!("d{}n", "s"),
        &format!("connection_{}", "string"),
        "password",
        &format!("cred{}", "ential"),
    ] {
        anyhow::ensure!(
            !lower.contains(token),
            "backend_classes must not contain {}",
            token
        );
    }
    // No standalone SQL/table/collection/vector product terminology
    // (sqlite_event_store as a backend class identifier is allowed)
    anyhow::ensure!(
        !lower.contains("\"sql\""),
        "backend_classes must not contain standalone sql term"
    );
    anyhow::ensure!(
        !lower.contains("table"),
        "backend_classes must not contain table terminology"
    );
    anyhow::ensure!(
        !lower.contains("collection"),
        "backend_classes must not contain collection terminology"
    );
    anyhow::ensure!(
        !lower.contains("vector"),
        "backend_classes must not contain vector terminology"
    );

    // No kernel database namespace tokens
    let forbidden = forbidden_platform_namespace_tokens();
    for token in &forbidden {
        anyhow::ensure!(
            !output_str.contains(token),
            "backend_classes must not contain {}",
            token
        );
    }

    // Backend classes have capability_flags
    let classes = result.output["backend_classes"].as_array().unwrap();
    for class in classes {
        anyhow::ensure!(
            class["capability_flags"].is_array(),
            "each backend class must have capability_flags"
        );
        anyhow::ensure!(
            class["status"].is_string(),
            "each backend class must have status"
        );
    }

    Ok(())
}

/// Case 3: Package state plan — namespace scoped to package, no official priority.
pub(crate) async fn package_state_plan_scoped() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let plan = invoke(
        &rt,
        "plan_package_state_store",
        json!({
            "package_id": "thirdparty/my-app",
            "store_id": "userdata",
            "schema_hint": "document",
        }),
    )
    .await?;

    anyhow::ensure!(
        plan.output["kind"] == json!("package_state_plan"),
        "must return package_state_plan kind"
    );

    // Namespace must belong to the package
    let namespace = plan.output["namespace"].as_str().unwrap();
    anyhow::ensure!(
        namespace.starts_with("thirdparty/my-app/state/"),
        "namespace must belong to the owning package, got: {}",
        namespace
    );

    // Plan-only, requires approval
    anyhow::ensure!(plan.output["plan_only"] == json!(true));
    anyhow::ensure!(plan.output["requires_user_approval"] == json!(true));

    // Has quota hints
    anyhow::ensure!(
        plan.output["quota_hints"].is_object(),
        "must have quota_hints"
    );

    // Has backend candidates
    anyhow::ensure!(
        plan.output["backend_candidates"].is_array(),
        "must have backend_candidates"
    );

    // No raw secrets or paths in output
    let output_str = serde_json::to_string(&plan.output).unwrap();
    let lower = output_str.to_lowercase();
    for token in &[
        &format!("d{}n", "s"),
        "password",
        &format!("cred{}", "ential"),
        &format!("connection_{}", "string"),
        "file://",
    ] {
        anyhow::ensure!(!lower.contains(token), "plan must not contain {}", token);
    }

    Ok(())
}

/// Case 4: Put document preview — write_performed=false.
pub(crate) async fn put_document_preview_no_write() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let result = invoke(
        &rt,
        "put_document_preview",
        json!({
            "document_id": "my-pkg/doc/hello",
            "store_id": "default",
            "content": {"title": "Hello", "body": "World"},
        }),
    )
    .await?;

    anyhow::ensure!(
        result.output["kind"] == json!("document_put_preview"),
        "must return document_put_preview kind"
    );
    anyhow::ensure!(
        result.output["write_performed"] == json!(false),
        "write_performed must be false"
    );
    anyhow::ensure!(
        result.output["content_address"].is_string(),
        "must have content_address"
    );
    anyhow::ensure!(
        result.output["redacted_content"].is_object(),
        "must have redacted_content"
    );

    Ok(())
}

/// Case 5: Get document preview — read_performed=false.
pub(crate) async fn get_document_preview_no_read() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let result = invoke(
        &rt,
        "get_document_preview",
        json!({
            "document_id": "my-pkg/doc/hello",
            "store_id": "default",
        }),
    )
    .await?;

    anyhow::ensure!(
        result.output["kind"] == json!("document_get_preview"),
        "must return document_get_preview kind"
    );
    anyhow::ensure!(
        result.output["read_performed"] == json!(false),
        "read_performed must be false"
    );

    Ok(())
}

/// Case 6: Query prefix preview — query_performed=false.
pub(crate) async fn query_prefix_preview_no_query_execution() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let result = invoke(
        &rt,
        "query_document_prefix_preview",
        json!({
            "prefix": "my-pkg/doc",
            "store_id": "default",
        }),
    )
    .await?;

    anyhow::ensure!(
        result.output["kind"] == json!("document_query_preview"),
        "must return document_query_preview kind"
    );
    anyhow::ensure!(
        result.output["query_performed"] == json!(false),
        "query_performed must be false"
    );

    Ok(())
}

/// Case 7: Delete tombstone preview — delete_performed=false.
pub(crate) async fn delete_tombstone_preview_no_delete() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let result = invoke(
        &rt,
        "delete_document_tombstone_preview",
        json!({
            "document_id": "my-pkg/doc/hello",
            "store_id": "default",
        }),
    )
    .await?;

    anyhow::ensure!(
        result.output["kind"] == json!("document_tombstone_preview"),
        "must return document_tombstone_preview kind"
    );
    anyhow::ensure!(
        result.output["delete_performed"] == json!(false),
        "delete_performed must be false"
    );
    anyhow::ensure!(
        result.output["tombstone_status"] == json!("preview_only"),
        "tombstone_status must be preview_only"
    );

    Ok(())
}

/// Case 8: Export snapshot preview — redacted, snapshot_exported=false.
pub(crate) async fn export_snapshot_preview_redacted() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let result = invoke(
        &rt,
        "export_store_snapshot_preview",
        json!({
            "store_id": "default",
        }),
    )
    .await?;

    anyhow::ensure!(
        result.output["kind"] == json!("store_snapshot_preview"),
        "must return store_snapshot_preview kind"
    );
    anyhow::ensure!(
        result.output["snapshot_exported"] == json!(false),
        "snapshot_exported must be false"
    );
    anyhow::ensure!(
        result.output["redacted_entries"].is_object(),
        "must have redacted_entries"
    );

    Ok(())
}

/// Case 9: Raw secret rejected across all mutating capabilities.
pub(crate) async fn raw_secret_rejected() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    // put_document_preview blocks raw secret
    let put = invoke(
        &rt,
        "put_document_preview",
        json!({
            "document_id": "doc1",
            "token_field": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(put.output["kind"] == json!("storage_lab_rejected"));
    anyhow::ensure!(put.output["redaction_state"] == json!("unsafe_blocked"));

    // plan_package_state_store blocks raw secret
    let plan = invoke(
        &rt,
        "plan_package_state_store",
        json!({
            "package_id": "my-pkg",
            "token": "Bearer abc123",
        }),
    )
    .await?;
    anyhow::ensure!(plan.output["kind"] == json!("storage_lab_rejected"));

    // get_document_preview blocks raw secret
    let get = invoke(
        &rt,
        "get_document_preview",
        json!({
            "document_id": "doc1",
            "secret": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(get.output["kind"] == json!("storage_lab_rejected"));

    Ok(())
}

/// Case 10: Unsafe ID rejected (path traversal, special characters).
pub(crate) async fn unsafe_id_rejected() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    // Path traversal in document_id
    let get = invoke(
        &rt,
        "get_document_preview",
        json!({
            "document_id": "../../../etc/passwd",
            "store_id": "default",
        }),
    )
    .await?;
    anyhow::ensure!(get.output["kind"] == json!("storage_lab_rejected"));

    // Path traversal in store_id
    let put = invoke(
        &rt,
        "put_document_preview",
        json!({
            "document_id": "doc1",
            "store_id": "../escape",
        }),
    )
    .await?;
    anyhow::ensure!(put.output["kind"] == json!("storage_lab_rejected"));

    // Empty package_id in plan
    let plan = invoke(
        &rt,
        "plan_package_state_store",
        json!({
            "package_id": "",
            "store_id": "test",
        }),
    )
    .await?;
    anyhow::ensure!(plan.output["kind"] == json!("storage_lab_rejected"));

    Ok(())
}

/// Case 11: Blob contract shape — content-addressed, backend candidates, red lines,
/// no kernel database/vector/blob namespace.
pub(crate) async fn blob_contract_shape() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let contract = invoke(&rt, "describe_blob_store_contract", json!({})).await?;

    anyhow::ensure!(
        contract.output["kind"] == json!("blob_store_contract"),
        "must return blob_store_contract kind"
    );
    anyhow::ensure!(
        contract.output["contract_type"] == json!("content_addressed_blob_store"),
        "contract_type must be content_addressed_blob_store"
    );

    // Backend candidates
    let candidates = contract.output["backend_candidates"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("backend_candidates must be array"))?;
    anyhow::ensure!(
        candidates.len() >= 3,
        "must have at least 3 backend candidates"
    );

    // Red lines
    let red_lines = contract.output["red_lines"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("red_lines must be array"))?;
    anyhow::ensure!(
        red_lines.contains(&json!("no_blob_content_in_events")),
        "must have no_blob_content_in_events red line"
    );
    anyhow::ensure!(
        red_lines.contains(&json!("no_raw_secrets")),
        "must have no_raw_secrets red line"
    );
    anyhow::ensure!(
        red_lines.contains(&json!("no_private_backend_topology")),
        "must have no_private_backend_topology red line"
    );
    anyhow::ensure!(
        red_lines.contains(&json!("content_address_required")),
        "must have content_address_required red line"
    );

    // No forbidden namespace tokens
    let output_str = serde_json::to_string(&contract.output).unwrap();
    let forbidden = forbidden_platform_namespace_tokens();
    for token in &forbidden {
        anyhow::ensure!(
            !output_str.contains(token),
            "blob contract must not contain {}",
            token
        );
    }

    // No secret-bearing backend config in backend candidates
    let lower = output_str.to_lowercase();
    for token in &[
        &format!("d{}n", "s"),
        &format!("connection_{}", "string"),
        "password",
        &format!("cred{}", "ential"),
        "bucket",
        "s3://",
        "gcs://",
    ] {
        anyhow::ensure!(
            !lower.contains(token),
            "blob contract must not contain {}",
            token
        );
    }

    // No inference / no network
    anyhow::ensure!(contract.output["inference_performed"] == json!(false));
    anyhow::ensure!(contract.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 12: Put blob preview — content address deterministic, no real storage,
/// no blob content in events.
pub(crate) async fn put_blob_preview_content_address_deterministic() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    // With content_hash provided → normalized sha256: prefix
    let result = invoke(
        &rt,
        "put_blob_preview",
        json!({
            "package_id": "thirdparty/my-app",
            "blob_id": "asset/avatar",
            "mime": "image/png",
            "size_bytes": 2048,
            "content_hash": "deadbeef1234",
        }),
    )
    .await?;

    anyhow::ensure!(
        result.output["kind"] == json!("blob_put_preview"),
        "must return blob_put_preview kind"
    );
    anyhow::ensure!(
        result.output["content_address"] == json!("sha256:deadbeef1234"),
        "content_hash must be normalized with sha256: prefix"
    );
    anyhow::ensure!(
        result.output["mime"] == json!("image/png"),
        "mime must match input"
    );
    anyhow::ensure!(
        result.output["size_bytes"] == json!(2048),
        "size_bytes must match input"
    );

    // Deterministic: same input → same content_address
    let result2 = invoke(
        &rt,
        "put_blob_preview",
        json!({
            "package_id": "thirdparty/my-app",
            "blob_id": "asset/avatar",
            "mime": "image/png",
            "size_bytes": 2048,
            "content_hash": "deadbeef1234",
        }),
    )
    .await?;
    anyhow::ensure!(
        result.output["content_address"] == result2.output["content_address"],
        "same input must produce same content_address"
    );

    Ok(())
}

/// Case 13: Put blob preview — no real storage, no blob content in event payload.
pub(crate) async fn put_blob_preview_no_storage_no_content_event() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let result = invoke(
        &rt,
        "put_blob_preview",
        json!({
            "package_id": "my-pkg",
            "blob_id": "doc/readme",
            "content_sample": "Hello world",
        }),
    )
    .await?;

    anyhow::ensure!(
        result.output["kind"] == json!("blob_put_preview"),
        "must return blob_put_preview kind"
    );
    anyhow::ensure!(
        result.output["blob_stored"] == json!(false),
        "blob_stored must be false"
    );
    anyhow::ensure!(
        result.output["filesystem_performed"] == json!(false),
        "filesystem_performed must be false"
    );
    anyhow::ensure!(
        result.output["network_performed"] == json!(false),
        "network_performed must be false"
    );
    anyhow::ensure!(
        result.output["event_payload_contains_blob"] == json!(false),
        "event_payload_contains_blob must be false"
    );
    anyhow::ensure!(
        result.output["content_address"].is_string(),
        "must have content_address"
    );

    Ok(())
}

/// Case 14: Get blob metadata preview — no blob content returned.
pub(crate) async fn get_blob_metadata_preview_no_content() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let result = invoke(
        &rt,
        "get_blob_metadata_preview",
        json!({
            "blob_id": "my-pkg/asset/avatar",
        }),
    )
    .await?;

    anyhow::ensure!(
        result.output["kind"] == json!("blob_metadata_preview"),
        "must return blob_metadata_preview kind"
    );
    anyhow::ensure!(
        result.output["blob_read"] == json!(false),
        "blob_read must be false"
    );
    anyhow::ensure!(
        result.output["content_returned"] == json!(false),
        "content_returned must be false"
    );
    anyhow::ensure!(
        result.output["content_address"].is_string(),
        "must have content_address"
    );

    Ok(())
}

/// Case 15: Export blob manifest preview — refs only, no content.
pub(crate) async fn export_blob_manifest_refs_only() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let result = invoke(
        &rt,
        "export_blob_manifest_preview",
        json!({
            "package_id": "my-pkg",
        }),
    )
    .await?;

    anyhow::ensure!(
        result.output["kind"] == json!("blob_manifest_preview"),
        "must return blob_manifest_preview kind"
    );
    anyhow::ensure!(
        result.output["content_included"] == json!(false),
        "content_included must be false"
    );
    anyhow::ensure!(
        result.output["manifest_items"].is_array(),
        "must have manifest_items array"
    );

    Ok(())
}

/// Case 16: Blob raw secret and unsafe ID rejected.
pub(crate) async fn blob_raw_secret_and_unsafe_id_rejected() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    // Raw secret in put_blob_preview
    let put = invoke(
        &rt,
        "put_blob_preview",
        json!({
            "package_id": "my-pkg",
            "blob_id": "doc/1",
            "token_field": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(put.output["kind"] == json!("storage_lab_rejected"));
    anyhow::ensure!(put.output["redaction_state"] == json!("unsafe_blocked"));

    // Path traversal in blob_id
    let put2 = invoke(
        &rt,
        "put_blob_preview",
        json!({
            "package_id": "my-pkg",
            "blob_id": "../../etc/passwd",
        }),
    )
    .await?;
    anyhow::ensure!(put2.output["kind"] == json!("storage_lab_rejected"));

    // Path traversal in package_id
    let put3 = invoke(
        &rt,
        "put_blob_preview",
        json!({
            "package_id": "../escape",
            "blob_id": "doc/1",
        }),
    )
    .await?;
    anyhow::ensure!(put3.output["kind"] == json!("storage_lab_rejected"));

    // Raw secret in get_blob_metadata_preview
    let get = invoke(
        &rt,
        "get_blob_metadata_preview",
        json!({
            "blob_id": "doc/1",
            "secret": "Bearer abc123xyz456",
        }),
    )
    .await?;
    anyhow::ensure!(get.output["kind"] == json!("storage_lab_rejected"));

    // Raw secret in export_blob_manifest_preview
    let export = invoke(
        &rt,
        "export_blob_manifest_preview",
        json!({
            "package_id": "my-pkg",
            "token": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(export.output["kind"] == json!("storage_lab_rejected"));

    // Oversized content_sample rejected
    let big_sample = "x".repeat(5000);
    let put4 = invoke(
        &rt,
        "put_blob_preview",
        json!({
            "package_id": "my-pkg",
            "blob_id": "doc/big",
            "content_sample": big_sample,
        }),
    )
    .await?;
    anyhow::ensure!(put4.output["kind"] == json!("storage_lab_rejected"));

    Ok(())
}

/// Case 16: Projection contract shape — backend candidates, red lines,
/// no DB table/collection/vector/database namespace leakage.
pub(crate) async fn projection_contract_shape() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let contract = invoke(&rt, "describe_projection_store_contract", json!({})).await?;

    anyhow::ensure!(
        contract.output["kind"] == json!("projection_store_contract"),
        "must return projection_store_contract kind"
    );

    // Contract kinds
    let contract_kinds = contract.output["contract_kinds"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("contract_kinds must be array"))?;
    anyhow::ensure!(
        contract_kinds.len() >= 4,
        "must have at least 4 contract kinds"
    );

    // Backend candidates
    let candidates = contract.output["backend_candidates"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("backend_candidates must be array"))?;
    anyhow::ensure!(
        candidates.len() >= 4,
        "must have at least 4 backend candidates"
    );

    // Red lines
    let red_lines = contract.output["red_lines"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("red_lines must be array"))?;
    anyhow::ensure!(
        red_lines.contains(&json!("no_table_exposure")),
        "must have no_table_exposure red line"
    );
    anyhow::ensure!(
        red_lines.contains(&json!("no_sql_exposure")),
        "must have no_sql_exposure red line"
    );
    anyhow::ensure!(
        red_lines.contains(&json!("no_secret_backend_config")),
        "must have no_secret_backend_config red line"
    );
    anyhow::ensure!(
        red_lines.contains(&json!("projection_derives_from_events_assets_only")),
        "must have projection_derives_from_events_assets_only red line"
    );

    // No DB table/collection/vector/database namespace leakage
    // "no_table_exposure" / "no_sql_exposure" in red_lines are negation terms
    // (blocking the leakage), not leakage themselves — filter them out.
    let output_str = serde_json::to_string(&contract.output).unwrap();
    let lower_raw = output_str.to_lowercase();
    let lower = lower_raw
        .replace("no_table_exposure", "")
        .replace("no_sql_exposure", "");
    anyhow::ensure!(
        !lower.contains("\"sql\""),
        "must not contain standalone sql term"
    );
    anyhow::ensure!(
        !lower.contains("table"),
        "must not contain table terminology"
    );
    anyhow::ensure!(
        !lower.contains("collection"),
        "must not contain collection terminology"
    );
    anyhow::ensure!(
        !lower.contains("vector"),
        "must not contain vector terminology"
    );
    anyhow::ensure!(
        !lower.contains("\"database\""),
        "must not contain database terminology"
    );

    // No platform-reserved namespace tokens
    let forbidden = forbidden_platform_namespace_tokens();
    for token in &forbidden {
        anyhow::ensure!(
            !output_str.contains(token),
            "projection contract must not contain {}",
            token
        );
    }

    // No inference / no network
    anyhow::ensure!(contract.output["inference_performed"] == json!(false));
    anyhow::ensure!(contract.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 17: Projection materialization plan only — materialized=false,
/// write_performed=false, backend_selected=false.
pub(crate) async fn projection_materialization_plan_only() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let plan = invoke(
        &rt,
        "plan_projection_materialization",
        json!({
            "package_id": "thirdparty/my-app",
            "projection_id": "thirdparty/my-app/projection/scores",
            "source_kinds": ["thirdparty/my-app/event/score_update"],
            "index_keys": ["player_id", "score"],
        }),
    )
    .await?;

    anyhow::ensure!(
        plan.output["kind"] == json!("projection_materialization_plan"),
        "must return projection_materialization_plan kind"
    );
    anyhow::ensure!(
        plan.output["materialized"] == json!(false),
        "materialized must be false"
    );
    anyhow::ensure!(
        plan.output["write_performed"] == json!(false),
        "write_performed must be false"
    );
    anyhow::ensure!(
        plan.output["backend_selected"] == json!(false),
        "backend_selected must be false"
    );
    anyhow::ensure!(
        plan.output["plan_only"] == json!(true),
        "plan_only must be true"
    );

    // Backend candidates present
    let candidates = plan.output["backend_candidates"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("backend_candidates must be array"))?;
    anyhow::ensure!(
        candidates.len() >= 4,
        "must have at least 4 backend candidates"
    );

    // No credentials in output
    let output_str = serde_json::to_string(&plan.output).unwrap();
    let lower = output_str.to_lowercase();
    for token in &[
        &format!("d{}n", "s"),
        "password",
        &format!("cred{}", "ential"),
        &format!("connection_{}", "string"),
    ] {
        anyhow::ensure!(!lower.contains(token), "plan must not contain {}", token);
    }

    Ok(())
}

/// Case 18: Projection query preview — no execution, no SQL/table/collection/vector terms.
pub(crate) async fn projection_query_preview_no_execution() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let result = invoke(
        &rt,
        "query_projection_preview",
        json!({
            "projection_ref": "thirdparty/my-app/projection/scores",
            "filter_preview": {"player_id": "abc"},
            "limit": 5,
        }),
    )
    .await?;

    anyhow::ensure!(
        result.output["kind"] == json!("projection_query_preview"),
        "must return projection_query_preview kind"
    );
    anyhow::ensure!(
        result.output["query_executed"] == json!(false),
        "query_executed must be false"
    );
    anyhow::ensure!(
        result.output["rows_returned"] == json!(false),
        "rows_returned must be false"
    );
    anyhow::ensure!(
        result.output["preview_shape"].is_object(),
        "must have preview_shape"
    );

    // No SQL/table/collection/vector terms in output
    let output_str = serde_json::to_string(&result.output).unwrap();
    let lower = output_str.to_lowercase();
    anyhow::ensure!(
        !lower.contains("\"sql\""),
        "must not contain standalone sql term"
    );
    anyhow::ensure!(
        !lower.contains("table"),
        "must not contain table terminology"
    );
    anyhow::ensure!(
        !lower.contains("collection"),
        "must not contain collection terminology"
    );
    anyhow::ensure!(
        !lower.contains("vector"),
        "must not contain vector terminology"
    );

    Ok(())
}

/// Case 19: Projection migration plan — no rewrite, requires_rebuild=true.
pub(crate) async fn projection_migration_plan_no_rewrite() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let result = invoke(
        &rt,
        "migrate_projection_plan_preview",
        json!({
            "projection_id": "thirdparty/my-app/projection/scores",
            "from_version": "1",
            "to_version": "2",
            "change_summary": "added player_id index",
        }),
    )
    .await?;

    anyhow::ensure!(
        result.output["kind"] == json!("projection_migration_plan_preview"),
        "must return projection_migration_plan_preview kind"
    );
    anyhow::ensure!(
        result.output["migration_applied"] == json!(false),
        "migration_applied must be false"
    );
    anyhow::ensure!(
        result.output["data_rewritten"] == json!(false),
        "data_rewritten must be false"
    );
    anyhow::ensure!(
        result.output["requires_rebuild"] == json!(true),
        "requires_rebuild must be true"
    );

    Ok(())
}

/// Case 20: Projection rejects raw secret.
pub(crate) async fn projection_rejects_raw_secret() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    // Raw secret in plan_projection_materialization
    let plan = invoke(
        &rt,
        "plan_projection_materialization",
        json!({
            "package_id": "my-pkg",
            "projection_id": "my-pkg/proj/1",
            "token_field": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(plan.output["kind"] == json!("storage_lab_rejected"));
    anyhow::ensure!(plan.output["redaction_state"] == json!("unsafe_blocked"));

    // Raw secret in query_projection_preview
    let query = invoke(
        &rt,
        "query_projection_preview",
        json!({
            "projection_ref": "my-pkg/proj/1",
            "secret": "Bearer RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(query.output["kind"] == json!("storage_lab_rejected"));

    // Raw secret in migrate_projection_plan_preview
    let migrate = invoke(
        &rt,
        "migrate_projection_plan_preview",
        json!({
            "projection_id": "my-pkg/proj/1",
            "token": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(migrate.output["kind"] == json!("storage_lab_rejected"));

    Ok(())
}

/// Case 21: Projection no DB table leakage — no SQL/table/collection/vector/database
/// in projection-related outputs across all 4 capabilities.
pub(crate) async fn projection_no_db_table_leakage() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let capabilities = vec![
        ("describe_projection_store_contract", json!({})),
        (
            "plan_projection_materialization",
            json!({
                "package_id": "my-pkg",
                "projection_id": "my-pkg/proj/1",
            }),
        ),
        (
            "query_projection_preview",
            json!({
                "projection_ref": "my-pkg/proj/1",
            }),
        ),
        (
            "migrate_projection_plan_preview",
            json!({
                "projection_id": "my-pkg/proj/1",
            }),
        ),
    ];

    let forbidden_terms = ["table", "collection", "vector", "\"database\""];
    let platform_prefixes = forbidden_platform_namespace_tokens();

    for (cap, input) in capabilities {
        let result = invoke(&rt, cap, input).await?;
        let output_str = serde_json::to_string(&result.output).unwrap();
        // "no_table_exposure" / "no_sql_exposure" in red_lines are negation terms
        // (blocking the leakage), not leakage themselves — filter them out.
        let lower_raw = output_str.to_lowercase();
        let lower = lower_raw
            .replace("no_table_exposure", "")
            .replace("no_sql_exposure", "");

        for term in &forbidden_terms {
            anyhow::ensure!(
                !lower.contains(term),
                "projection capability {} must not contain {}",
                cap,
                term
            );
        }
        for prefix in &platform_prefixes {
            anyhow::ensure!(
                !output_str.contains(prefix),
                "projection capability {} must not contain {}",
                cap,
                prefix
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// S5 — Retrieval / Vector / Multimodal Provider Contract conformance
// ---------------------------------------------------------------------------

/// Case 22: Retrieval contract shape — backend candidates, red lines,
/// no kernel vector/embedding namespace.
pub(crate) async fn retrieval_contract_shape() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let contract = invoke(&rt, "describe_retrieval_provider_contract", json!({})).await?;

    anyhow::ensure!(
        contract.output["kind"] == json!("retrieval_provider_contract"),
        "must return retrieval_provider_contract kind"
    );

    let contract_kinds = contract.output["contract_kinds"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("contract_kinds must be array"))?;
    anyhow::ensure!(
        contract_kinds.len() >= 5,
        "must have at least 5 contract kinds"
    );

    let candidates = contract.output["backend_candidates"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("backend_candidates must be array"))?;
    anyhow::ensure!(
        candidates.len() >= 6,
        "must have at least 6 backend candidates"
    );

    let red_lines = contract.output["red_lines"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("red_lines must be array"))?;
    anyhow::ensure!(red_lines.contains(&json!("no_embedding_generation")));
    anyhow::ensure!(red_lines.contains(&json!("no_vector_storage")));
    anyhow::ensure!(red_lines.contains(&json!("no_network")));
    anyhow::ensure!(red_lines.contains(&json!("no_secret_backend_config")));
    anyhow::ensure!(red_lines.contains(&json!("no_platform_vector_namespace")));

    // No platform-reserved namespace tokens
    let output_str = serde_json::to_string(&contract.output).unwrap();
    let forbidden = forbidden_platform_namespace_tokens();
    for token in &forbidden {
        anyhow::ensure!(
            !output_str.contains(token),
            "retrieval contract must not contain {}",
            token
        );
    }

    anyhow::ensure!(contract.output["inference_performed"] == json!(false));
    anyhow::ensure!(contract.output["network_performed"] == json!(false));

    Ok(())
}

/// Case 23: Multimodal index plan — no embedding, no storage, plan-only.
pub(crate) async fn multimodal_index_plan_no_embedding_no_storage() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let plan = invoke(
        &rt,
        "draft_multimodal_index_plan",
        json!({
            "package_id": "thirdparty/my-app",
            "index_id": "thirdparty/my-app/index/multimodal-docs",
            "modalities": ["text", "image", "structured"],
            "asset_refs": ["thirdparty/my-app/asset/doc1", "thirdparty/my-app/asset/img1"],
            "schema_hint": "multimodal_document",
        }),
    )
    .await?;

    anyhow::ensure!(
        plan.output["kind"] == json!("multimodal_index_plan"),
        "must return multimodal_index_plan kind"
    );
    anyhow::ensure!(plan.output["embedding_generated"] == json!(false));
    anyhow::ensure!(plan.output["index_created"] == json!(false));
    anyhow::ensure!(plan.output["vectors_stored"] == json!(false));
    anyhow::ensure!(plan.output["network_performed"] == json!(false));
    anyhow::ensure!(plan.output["plan_only"] == json!(true));
    anyhow::ensure!(plan.output["inference_performed"] == json!(false));

    let candidates = plan.output["backend_candidates"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("backend_candidates must be array"))?;
    anyhow::ensure!(
        candidates.len() >= 6,
        "must have at least 6 backend candidates"
    );
    for candidate in candidates {
        anyhow::ensure!(
            candidate["status"] == json!("future"),
            "all retrieval backend candidates must be future"
        );
    }

    Ok(())
}

/// Case 24: Multimodal index rejects invalid modality or too many asset_refs.
pub(crate) async fn multimodal_index_rejects_invalid_modality_or_too_many_refs(
) -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    // Invalid modality "embedding"
    let plan1 = invoke(
        &rt,
        "draft_multimodal_index_plan",
        json!({
            "package_id": "my-pkg",
            "index_id": "my-pkg/index/1",
            "modalities": ["text", "embedding"],
            "asset_refs": ["my-pkg/asset/1"],
        }),
    )
    .await?;
    anyhow::ensure!(
        plan1.output["kind"] == json!("storage_lab_rejected"),
        "invalid modality must be rejected"
    );

    // Too many asset_refs (> 64)
    let many_refs: Vec<String> = (0..70).map(|i| format!("my-pkg/asset/{}", i)).collect();
    let plan2 = invoke(
        &rt,
        "draft_multimodal_index_plan",
        json!({
            "package_id": "my-pkg",
            "index_id": "my-pkg/index/1",
            "modalities": ["text"],
            "asset_refs": many_refs,
        }),
    )
    .await?;
    anyhow::ensure!(
        plan2.output["kind"] == json!("storage_lab_rejected"),
        "too many asset_refs must be rejected"
    );

    // Empty modalities
    let plan3 = invoke(
        &rt,
        "draft_multimodal_index_plan",
        json!({
            "package_id": "my-pkg",
            "index_id": "my-pkg/index/1",
            "modalities": [],
        }),
    )
    .await?;
    anyhow::ensure!(
        plan3.output["kind"] == json!("storage_lab_rejected"),
        "empty modalities must be rejected"
    );

    Ok(())
}

/// Case 25: Vector search plan — no execution, no embedding, plan-only.
pub(crate) async fn vector_search_plan_no_execution() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let plan = invoke(
        &rt,
        "draft_vector_search_plan",
        json!({
            "index_ref": "thirdparty/my-app/index/multimodal-docs",
            "query_kind": "similarity",
            "top_k": 10,
            "filter_preview": {"modality": "text"},
        }),
    )
    .await?;

    anyhow::ensure!(
        plan.output["kind"] == json!("vector_search_plan"),
        "must return vector_search_plan kind"
    );
    anyhow::ensure!(plan.output["search_executed"] == json!(false));
    anyhow::ensure!(plan.output["embedding_generated"] == json!(false));
    anyhow::ensure!(plan.output["vectors_loaded"] == json!(false));
    anyhow::ensure!(plan.output["plan_only"] == json!(true));
    anyhow::ensure!(plan.output["inference_performed"] == json!(false));
    anyhow::ensure!(plan.output["network_performed"] == json!(false));

    let candidates = plan.output["backend_candidates"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("backend_candidates must be array"))?;
    anyhow::ensure!(
        candidates.len() >= 6,
        "must have at least 6 backend candidates"
    );

    Ok(())
}

/// Case 26: Backend fit mentions TDB future only — no kernel vector namespace, no secret-bearing backend config.
pub(crate) async fn backend_fit_mentions_tdb_future_only() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    let fit = invoke(
        &rt,
        "explain_retrieval_backend_fit",
        json!({
            "workload_hint": "multimodal",
        }),
    )
    .await?;

    anyhow::ensure!(
        fit.output["kind"] == json!("retrieval_backend_fit"),
        "must return retrieval_backend_fit kind"
    );

    let fit_matrix = fit.output["fit_matrix"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("fit_matrix must be array"))?;
    anyhow::ensure!(fit_matrix.len() >= 6, "must have at least 6 fit entries");

    // TDB entry exists and is "future" only
    let tdb_entry = fit_matrix.iter().find(|e| e["class_id"] == "tdb_future");
    anyhow::ensure!(tdb_entry.is_some(), "must contain tdb_future entry");
    let tdb = tdb_entry.unwrap();
    anyhow::ensure!(
        tdb["status"] == "future",
        "tdb_future must have status future"
    );

    // No platform-reserved namespace tokens
    let output_str = serde_json::to_string(&fit.output).unwrap();
    let forbidden = forbidden_platform_namespace_tokens();
    for token in &forbidden {
        anyhow::ensure!(
            !output_str.contains(token),
            "backend fit must not contain {}",
            token
        );
    }

    // No secret-bearing backend config
    let lower_raw = output_str.to_lowercase();
    // "no_secret_backend_config" is a negation term — filter it out.
    let lower = lower_raw.replace("no_secret_backend_config", "");
    for token in &[
        &format!("d{}n", "s"),
        &format!("connection_{}", "string"),
        "password",
        &format!("cred{}", "ential"),
        &format!("postgres{}://", "ql"),
        &format!("redis{}", "://"),
    ] {
        anyhow::ensure!(
            !lower.contains(token),
            "backend fit must not contain {}",
            token
        );
    }

    Ok(())
}

/// Case 27: Retrieval rejects raw secret.
pub(crate) async fn retrieval_rejects_raw_secret() -> anyhow::Result<()> {
    let rt = load_storage_lab().await?;

    // Raw secret in draft_multimodal_index_plan
    let plan1 = invoke(
        &rt,
        "draft_multimodal_index_plan",
        json!({
            "package_id": "my-pkg",
            "index_id": "my-pkg/index/1",
            "modalities": ["text"],
            "token_field": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(plan1.output["kind"] == json!("storage_lab_rejected"));
    anyhow::ensure!(plan1.output["redaction_state"] == json!("unsafe_blocked"));

    // Raw secret in draft_vector_search_plan
    let plan2 = invoke(
        &rt,
        "draft_vector_search_plan",
        json!({
            "index_ref": "my-pkg/index/1",
            "token": "Bearer RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(plan2.output["kind"] == json!("storage_lab_rejected"));

    // Raw secret in explain_retrieval_backend_fit
    let fit = invoke(
        &rt,
        "explain_retrieval_backend_fit",
        json!({
            "workload_hint": "general",
            "secret": "RawSecretExample1234567890abcdefABCDEF123456",
        }),
    )
    .await?;
    anyhow::ensure!(fit.output["kind"] == json!("storage_lab_rejected"));

    Ok(())
}

/// Case 28: Retrieval no kernel vector namespace or credentials.
pub(crate) async fn retrieval_no_platform_vector_namespace_or_secret_config() -> anyhow::Result<()>
{
    let rt = load_storage_lab().await?;

    let capabilities = vec![
        ("describe_retrieval_provider_contract", json!({})),
        (
            "draft_multimodal_index_plan",
            json!({
                "package_id": "my-pkg",
                "index_id": "my-pkg/index/1",
                "modalities": ["text"],
                "asset_refs": ["my-pkg/asset/1"],
            }),
        ),
        (
            "draft_vector_search_plan",
            json!({
                "index_ref": "my-pkg/index/1",
                "query_kind": "similarity",
                "top_k": 5,
            }),
        ),
        (
            "explain_retrieval_backend_fit",
            json!({
                "workload_hint": "general",
            }),
        ),
    ];

    let platform_prefixes = forbidden_platform_namespace_tokens();

    for (cap, input) in capabilities {
        let result = invoke(&rt, cap, input).await?;
        let output_str = serde_json::to_string(&result.output).unwrap();

        for prefix in &platform_prefixes {
            anyhow::ensure!(
                !output_str.contains(prefix),
                "retrieval capability {} must not contain {}",
                cap,
                prefix
            );
        }
    }

    // Check contract output specifically for credentials
    let contract = invoke(&rt, "describe_retrieval_provider_contract", json!({})).await?;
    let output_str = serde_json::to_string(&contract.output).unwrap();
    // "no_secret_backend_config" is a negation term, not leakage — filter it out.
    let lower_raw = output_str.to_lowercase();
    let lower = lower_raw.replace("no_secret_backend_config", "");
    for token in &[
        &format!("d{}n", "s"),
        &format!("connection_{}", "string"),
        "password",
        &format!("cred{}", "ential"),
        "bucket",
    ] {
        anyhow::ensure!(
            !lower.contains(token),
            "retrieval contract must not contain {}",
            token
        );
    }

    Ok(())
}
