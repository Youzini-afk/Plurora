use serde_json::{json, Value};

use super::{safety, InprocInvocation};

const PACKAGE_ID: &str = "plurora/tdb-retrieval-lab";

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let local = request
        .capability_id
        .strip_prefix(&format!("{PACKAGE_ID}/"))?;
    Some(match local {
        "describe_tdb_retrieval_contract" => Ok(describe_contract()),
        "draft_tdb_index_plan" => draft_index_plan(&request.input),
        "draft_tdb_query_plan" => draft_query_plan(&request.input),
        "explain_tdb_backend_fit" => Ok(explain_backend_fit()),
        "inspect_tdb_adapter_surface" => Ok(inspect_adapter_surface()),
        "describe_real_tdb_opt_in_seam" => Ok(describe_real_tdb_opt_in_seam()),
        _ => return None,
    })
}

fn reject(reason: &str) -> Value {
    json!({
        "kind": "tdb_retrieval_lab_rejected",
        "accepted": false,
        "reason": reason,
        "tdb_opened": false,
        "index_created": false,
        "embedding_generated": false,
        "network_performed": false,
        "filesystem_performed": false,
    })
}

fn describe_contract() -> Value {
    json!({
        "kind": "tdb_retrieval_contract",
        "package_id": PACKAGE_ID,
        "package_kind": "ordinary_retrieval_provider_adapter",
        "backend_role": "future_multimodal_retrieval_provider_slot",
        "triviumdb_source_reviewed": {
            "database_config": "src/database/config.rs",
            "transaction_boundary": "src/database/transaction.rs",
            "pipeline": "src/database/pipeline.rs",
            "node_binding": "src/bindings/nodejs.rs",
            "python_binding": "src/bindings/python.rs"
        },
        "capabilities": [
            "describe_tdb_retrieval_contract",
            "draft_tdb_index_plan",
            "draft_tdb_query_plan",
            "explain_tdb_backend_fit",
            "inspect_tdb_adapter_surface",
            "describe_real_tdb_opt_in_seam"
        ],
        "modality_scope": ["text", "image", "audio", "video", "structured"],
        "red_lines": {
            "not_platform_database": true,
            "not_event_store_replacement": true,
            "not_package_raw_backend_access": true,
            "no_real_tdb_crate_linkage_in_alpha": true,
            "no_embedding_generation_in_alpha": true,
            "no_index_creation_in_alpha": true,
            "no_network_or_filesystem_in_alpha": true
        },
        "adapter_shape": {
            "input": ["asset_refs", "modality_hints", "schema_hint", "retrieval_objective"],
            "output": ["index_plan", "query_plan", "provenance", "audit_summary", "future_backend_requirements"]
        },
        "surfaces": {
            "forge_panel": "plurora/tdb-retrieval-lab/forge-panel",
            "assistant_action": "plurora/tdb-retrieval-lab/assistant-action",
            "home_card": "plurora/tdb-retrieval-lab/home-card"
        }
    })
}

fn draft_index_plan(input: &Value) -> anyhow::Result<Value> {
    validate_common_input(input)?;
    let index_id = required_safe_id(input, "index_id")?;
    let asset_refs = read_string_array(input, "asset_refs");
    if asset_refs.len() > 64 {
        return Ok(reject("too_many_asset_refs"));
    }
    let modality_hints = read_string_array(input, "modality_hints");
    if !modality_hints.iter().all(|m| is_allowed_modality(m)) {
        return Ok(reject("unsupported_modality"));
    }
    Ok(json!({
        "kind": "tdb_index_plan",
        "index_id": index_id,
        "plan_only": true,
        "provider_package_id": PACKAGE_ID,
        "asset_ref_count": asset_refs.len(),
        "asset_refs": asset_refs,
        "modality_hints": if modality_hints.is_empty() { json!(["text", "image", "structured"]) } else { json!(modality_hints) },
        "schema_hint": input.get("schema_hint").and_then(Value::as_str).unwrap_or("opaque_package_owned_schema"),
        "steps": [
            "validate package-owned retrieval schema",
            "select future TDB backend through host policy",
            "prepare multimodal metadata manifest",
            "draft approval-gated indexing proposal",
            "record provenance and redaction context"
        ],
        "future_backend_requirements": {
            "tdb_crate_opt_in": true,
            "host_backend_policy_required": true,
            "secret_backend_config_allowed": false,
            "private_backend_topology_exposed": false
        },
        "tdb_opened": false,
        "index_created": false,
        "embedding_generated": false,
        "vectors_stored": false,
        "network_performed": false,
        "filesystem_performed": false
    }))
}

fn draft_query_plan(input: &Value) -> anyhow::Result<Value> {
    validate_common_input(input)?;
    let index_id = required_safe_id(input, "index_id")?;
    let query_modalities = read_string_array(input, "query_modalities");
    if !query_modalities.iter().all(|m| is_allowed_modality(m)) {
        return Ok(reject("unsupported_modality"));
    }
    let limit = input
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(8)
        .min(32);
    Ok(json!({
        "kind": "tdb_query_plan",
        "index_id": index_id,
        "plan_only": true,
        "query_objective": input.get("query_objective").and_then(Value::as_str).unwrap_or("retrieve relevant multimodal context"),
        "query_modalities": if query_modalities.is_empty() { json!(["text"]) } else { json!(query_modalities) },
        "limit": limit,
        "ranking_policy": input.get("ranking_policy").and_then(Value::as_str).unwrap_or("provider_defined_with_package_audit"),
        "steps": [
            "validate caller permission and provider selection",
            "construct package-owned retrieval request",
            "execute future TDB query through provider adapter",
            "return redacted result refs and provenance"
        ],
        "search_executed": false,
        "tdb_opened": false,
        "embedding_generated": false,
        "vectors_loaded": false,
        "network_performed": false,
        "filesystem_performed": false
    }))
}

fn explain_backend_fit() -> Value {
    json!({
        "kind": "tdb_backend_fit",
        "tdb_role": "multimodal_retrieval_provider",
        "good_fit_for": [
            "semantic asset recall",
            "multimodal creator references",
            "agent workspace retrieval",
            "knowledge/memory package indexes",
            "external source semantic indexing"
        ],
        "not_fit_for": [
            "event log authority",
            "permission audit source of truth",
            "proposal lifecycle storage",
            "branch lineage authority",
            "raw package database access"
        ],
        "integration_boundary": {
            "platform_owns": ["events", "sessions", "packages", "capabilities", "permissions", "assets", "proposals"],
            "tdb_adapter_owns": ["retrieval index plans", "provider-specific ranking", "result references", "retrieval provenance"],
            "host_owns": ["backend enablement", "secret-bearing backend config", "resource limits", "audit policy"]
        },
        "real_backend_status": "future_opt_in_not_linked_in_alpha"
    })
}

fn inspect_adapter_surface() -> Value {
    json!({
        "kind": "tdb_adapter_surface",
        "surface_ids": [
            "plurora/tdb-retrieval-lab/forge-panel",
            "plurora/tdb-retrieval-lab/assistant-action",
            "plurora/tdb-retrieval-lab/home-card"
        ],
        "safe_actions": [
            "describe contract",
            "draft index plan",
            "draft query plan",
            "explain backend fit"
        ],
        "dangerous_actions_deferred": [
            "open real TDB backend",
            "create persistent index",
            "generate embeddings",
            "read local media files",
            "serve retrieval over network"
        ],
        "requires_user_approval_for_future_execution": true,
        "current_alpha_execution": false
    })
}

fn describe_real_tdb_opt_in_seam() -> Value {
    json!({
        "kind": "tdb_real_opt_in_seam",
        "status": "real_crate_adapter_available_opt_in",
        "reason_default_is_fake": [
            "default Forge profiles should not open local retrieval backends without host policy",
            "the real Rust proof is available through the published triviumdb crate in the explicit adapter manifest",
            "real backend access needs host policy, resource limits, redaction, and lifecycle ownership"
        ],
        "reviewed_triviumdb_api": {
            "crate": "triviumdb",
            "version_observed_in_source": "0.7.1",
            "published_adapter_version": "0.7.0",
            "crate_types": ["rlib", "cdylib"],
            "default_features": [],
            "optional_bindings": ["python", "nodejs", "cli"],
            "open_api": [
                "Database<T>::open(path, dim)",
                "Database<T>::open_with_config(path, Config)",
                "Config { dim, sync_mode, storage_mode }",
                "StorageMode::{Mmap, Rom}"
            ],
            "write_api": [
                "insert(vector, payload)",
                "insert_with_id(id, vector, payload)",
                "link(src, dst, label, weight)",
                "begin_tx() with WAL-first commit"
            ],
            "query_api": [
                "search(query_vector, top_k, expand_depth, min_score)",
                "search_advanced(query_vector, SearchConfig)",
                "search_hybrid(query_text, query_vector, SearchConfig)",
                "search_hybrid_with_context(...)"
            ],
            "storage_notes": [
                "single-process exclusive lock per database file",
                "WAL recovery and compaction are internal to TriviumDB",
                "mmap mode may create sidecar vector storage"
            ]
        },
        "recommended_plurora_modes": [
            {
                "mode": "subprocess_adapter_package",
                "default_preference": true,
                "why": "keeps TDB faults, file locks, and native dependencies outside the host platform runtime process"
            },
            {
                "mode": "feature_gated_inproc_adapter",
                "default_preference": false,
                "why": "only after TriviumDB is vendored or published in a way ordinary clones can resolve"
            }
        ],
        "host_policy_requirements": {
            "backend_enablement": "explicit_opt_in",
            "store_path": "host_ref_only",
            "dimension": "declared_and_bounded",
            "modality_schema": "package_owned",
            "resource_limits": ["max_nodes", "max_inline_payload_bytes", "max_query_top_k", "max_expand_depth"],
            "dangerous_actions_need_approval": ["open_backend", "create_index", "write_nodes", "compact", "repair"]
        },
        "current_alpha": {
            "path_dependency_committed": false,
            "tdb_crate_linked_by_default": false,
            "published_crate_adapter_manifest": "integrations/tdb/rust-adapter-real-crate/Cargo.toml",
            "backend_opened": false,
            "filesystem_performed": false,
            "network_performed": false,
            "embedding_generated": false,
            "index_created": false
        }
    })
}

fn validate_common_input(input: &Value) -> anyhow::Result<()> {
    if safety::contains_raw_secret(input) {
        anyhow::bail!("tdb retrieval lab input contains raw-secret-like content");
    }
    Ok(())
}

fn required_safe_id(input: &Value, field: &str) -> anyhow::Result<String> {
    let value = input
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or("default_index");
    if !is_safe_id(value) {
        anyhow::bail!("tdb retrieval lab id field is unsafe");
    }
    Ok(value.to_string())
}

fn is_safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && !value.contains("..")
        && !value.starts_with('/')
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '/' | '.'))
}

fn read_string_array(input: &Value, field: &str) -> Vec<String> {
    input
        .get(field)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn is_allowed_modality(value: &str) -> bool {
    matches!(value, "text" | "image" | "audio" | "video" | "structured")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invoke(capability: &str, input: Value) -> Value {
        let request = InprocInvocation {
            capability_id: format!("{PACKAGE_ID}/{capability}"),
            provider_package_id: PACKAGE_ID.to_string(),
            session_id: None,
            input,
        };
        try_handle(&request).unwrap().unwrap()
    }

    #[test]
    fn index_plan_is_plan_only() {
        let output = invoke(
            "draft_tdb_index_plan",
            json!({"index_id": "demo", "asset_refs": ["asset/a"]}),
        );
        assert_eq!(output["kind"], json!("tdb_index_plan"));
        assert_eq!(output["index_created"], json!(false));
        assert_eq!(output["tdb_opened"], json!(false));
    }

    #[test]
    fn rejects_unsafe_index_id() {
        let request = InprocInvocation {
            capability_id: format!("{PACKAGE_ID}/draft_tdb_index_plan"),
            provider_package_id: PACKAGE_ID.to_string(),
            session_id: None,
            input: json!({"index_id": "../secret"}),
        };
        assert!(try_handle(&request).unwrap().is_err());
    }
}
