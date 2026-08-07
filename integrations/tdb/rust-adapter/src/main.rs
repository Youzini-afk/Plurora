use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

const PACKAGE_ID: &str = "official/tdb-rust-adapter";
const PROTOCOL_VERSION: &str = "0.1.0";

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let response = match line {
            Ok(line) => handle_line(&line),
            Err(error) => jsonrpc_error(Value::Null, "read_error", &sanitize(&error.to_string())),
        };
        let _ = writeln!(stdout, "{}", response);
        let _ = stdout.flush();
    }
}

fn handle_line(line: &str) -> Value {
    let request: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => return jsonrpc_error(Value::Null, "parse_error", &sanitize(&error.to_string())),
    };
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    match request.get("method").and_then(Value::as_str) {
        Some("package.handshake") => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "ready": true,
                "package_protocol_version": PROTOCOL_VERSION,
                "package_id": PACKAGE_ID,
                "real_tdb_available": real_tdb_available(),
            }
        }),
        Some("capability.invoke") => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            let capability_id = params.get("capability_id").and_then(Value::as_str).unwrap_or_default();
            let input = params.get("input").cloned().unwrap_or_else(|| json!({}));
            match invoke(capability_id, input) {
                Ok(output) => json!({"jsonrpc": "2.0", "id": id, "result": {"output": output}}),
                Err(error) => jsonrpc_error(id, "capability_error", &sanitize(&error)),
            }
        }
        Some(other) => jsonrpc_error(id, "unknown_method", &sanitize(other)),
        None => jsonrpc_error(id, "missing_method", "request method is required"),
    }
}

fn invoke(capability_id: &str, input: Value) -> Result<Value, String> {
    let local = capability_id.strip_prefix("official/tdb-rust-adapter/").unwrap_or(capability_id);
    match local {
        "describe_real_tdb_adapter" => Ok(describe_adapter()),
        "run_real_tdb_smoke" => run_real_tdb_smoke(input),
        _ => Err(format!("unsupported capability '{local}'")),
    }
}

fn describe_adapter() -> Value {
    json!({
        "kind": "real_tdb_rust_adapter",
        "package_id": PACKAGE_ID,
        "adapter_role": "ordinary_subprocess_retrieval_provider",
        "real_tdb_available": real_tdb_available(),
        "default_build": {
            "triviumdb_dependency_linked": real_tdb_available(),
            "backend_opened": false,
            "filesystem_performed": false,
            "network_performed": false,
            "platform_storage": false,
            "event_store_backend": false
        },
        "real_feature": "real-tdb",
        "capabilities": [
            "official/tdb-rust-adapter/describe_real_tdb_adapter",
            "official/tdb-rust-adapter/run_real_tdb_smoke"
        ],
        "red_lines": [
            "not_platform_database",
            "not_event_store",
            "no_raw_backend_path_in_public_output",
            "bounded_vectors_only"
        ]
    })
}

fn run_real_tdb_smoke(_input: Value) -> Result<Value, String> {
    if contains_raw_secret(&_input) {
        return Err("raw secret-like input rejected before TDB access".to_string());
    }
    if contains_forbidden_backend_path(&_input) {
        return Err("raw backend path input rejected; use host-ref based configuration".to_string());
    }
    #[cfg(feature = "real-tdb")]
    {
        return real_tdb::run_smoke();
    }
    #[cfg(not(feature = "real-tdb"))]
    Ok(json!({
        "kind": "real_tdb_smoke_disabled",
        "real_tdb_available": real_tdb_available(),
        "smoke_executed": false,
        "triviumdb_dependency_linked": real_tdb_available(),
        "backend_opened": false,
        "filesystem_performed": false,
        "network_performed": false,
        "reason": "build without real-tdb feature; enable the opt-in local adapter manifest for a real TriviumDB API proof"
    }))
}

fn real_tdb_available() -> bool {
    cfg!(feature = "real-tdb")
}

fn jsonrpc_error(id: Value, code: &str, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn sanitize(message: &str) -> String {
    let mut redacted = message.replace('\n', " ").replace('\r', " ");
    for marker in ["sk-", "api_key", "password", "secret"] {
        redacted = redacted.replace(marker, "[redacted]");
    }
    redacted.chars().take(240).collect()
}

fn contains_raw_secret(value: &Value) -> bool {
    match value {
        Value::String(text) => {
            let lower = text.to_ascii_lowercase();
            lower.contains("sk-") || lower.contains("api_key") || lower.contains("password") || lower.contains("secret")
        }
        Value::Array(values) => values.iter().any(contains_raw_secret),
        Value::Object(map) => map.iter().any(|(key, value)| {
            let lower = key.to_ascii_lowercase();
            lower.contains("api_key") || lower.contains("password") || lower.contains("secret") || contains_raw_secret(value)
        }),
        _ => false,
    }
}

fn contains_forbidden_backend_path(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            matches!(key.as_str(), "path" | "store_path" | "backend_path" | "tdb_path") || contains_forbidden_backend_path(value)
        }),
        Value::Array(values) => values.iter().any(contains_forbidden_backend_path),
        _ => false,
    }
}

#[cfg(feature = "real-tdb")]
mod real_tdb {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    use triviumdb::database::{Config, SearchConfig, StorageMode};
    use triviumdb::storage::wal::SyncMode;
    use triviumdb::Database;

    pub(super) fn run_smoke() -> Result<Value, String> {
        let dim = 3usize;
        let mut root = std::env::temp_dir();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| sanitize(&error.to_string()))?
            .as_nanos();
        root.push(format!("plurora-tdb-smoke-{nonce}"));
        std::fs::create_dir_all(&root).map_err(|error| sanitize(&error.to_string()))?;
        let store_path = root.join("adapter-proof.tdb");
        let store_path_str = store_path.to_string_lossy().to_string();

        let cfg = Config {
            dim,
            sync_mode: SyncMode::Off,
            storage_mode: StorageMode::Rom,
        };
        let mut db: Database<f32> = Database::open_with_config(&store_path_str, cfg).map_err(|error| sanitize(&error.to_string()))?;

        let first = db
            .insert(&[0.12, -0.45, 0.78], json!({"ref":"asset/redacted-a","text":"Alice likes apples"}))
            .map_err(|error| sanitize(&error.to_string()))?;
        let second = db
            .insert(&[0.08, -0.52, 0.81], json!({"ref":"asset/redacted-b","text":"Bob gave Alice apples"}))
            .map_err(|error| sanitize(&error.to_string()))?;
        db.link(first, second, "related_to", 0.95)
            .map_err(|error| sanitize(&error.to_string()))?;

        let vector_hits = db
            .search(&[0.10, -0.48, 0.80], 5, 2, 0.1)
            .map_err(|error| sanitize(&error.to_string()))?;
        let search_cfg = SearchConfig {
            top_k: 5,
            expand_depth: 2,
            min_score: 0.1,
            ..Default::default()
        };
        let hybrid_hits = db
            .search_hybrid(Some("Alice apples"), Some(&[0.10, -0.48, 0.80]), &search_cfg)
            .map_err(|error| sanitize(&error.to_string()))?;

        let first_payload_ref = db
            .get(first)
            .and_then(|node| node.payload.get("ref").and_then(|value| value.as_str()).map(str::to_string))
            .unwrap_or_else(|| "redacted".to_string());
        drop(db);
        let _ = std::fs::remove_dir_all(&root);

        Ok(json!({
            "kind": "real_tdb_smoke_executed",
            "real_tdb_available": true,
            "smoke_executed": true,
            "triviumdb_dependency_linked": true,
            "backend_opened": true,
            "filesystem_performed": true,
            "network_performed": false,
            "raw_path_exposed": false,
            "store_ref": "temp:tdb-smoke-redacted",
            "dim": dim,
            "inserted_nodes": 2,
            "linked_edges": 1,
            "vector_hit_count": vector_hits.len(),
            "hybrid_hit_count": hybrid_hits.len(),
            "first_ref": first_payload_ref,
            "hit_preview": vector_hits.iter().take(3).map(|hit| json!({
                "id_present": hit.id > 0,
                "score": hit.score,
                "payload_redacted": true,
                "ref": hit.payload.get("ref").cloned().unwrap_or_else(|| json!("redacted"))
            })).collect::<Vec<_>>()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_describe_is_safe_stub() {
        let out = describe_adapter();
        assert_eq!(out["kind"], json!("real_tdb_rust_adapter"));
        assert_eq!(out["default_build"]["backend_opened"], json!(false));
    }

    #[test]
    fn default_smoke_does_not_execute() {
        let out = run_real_tdb_smoke(json!({})).unwrap();
        #[cfg(not(feature = "real-tdb"))]
        {
            assert_eq!(out["smoke_executed"], json!(false));
            assert_eq!(out["backend_opened"], json!(false));
        }
        #[cfg(feature = "real-tdb")]
        {
            assert_eq!(out["kind"], json!("real_tdb_smoke_executed"));
            assert_eq!(out["smoke_executed"], json!(true));
            assert_eq!(out["backend_opened"], json!(true));
            assert_eq!(out["inserted_nodes"], json!(2));
            assert!(out["vector_hit_count"].as_u64().unwrap_or(0) >= 1);
            assert!(out["hybrid_hit_count"].as_u64().unwrap_or(0) >= 1);
            assert_eq!(out["raw_path_exposed"], json!(false));
        }
    }

    #[test]
    fn rejects_secret_and_raw_path_inputs() {
        assert!(run_real_tdb_smoke(json!({"payload":"sk-test"})).is_err());
        assert!(run_real_tdb_smoke(json!({"store_path":"/tmp/private.tdb"})).is_err());
    }
}
