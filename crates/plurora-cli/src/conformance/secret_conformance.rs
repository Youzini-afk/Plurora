use std::collections::HashSet;
use std::sync::Arc;

use plurora_runtime::{
    CapabilityInvocationRequest, EnvSecretResolver, InMemoryEventStore, OpenSessionRequest,
    ProtocolContext, Runtime, RuntimeConfig, SecretResolverConfig,
};
use serde_json::json;

use super::fixtures::*;

/// Permission grants survive SQLite-backed runtime rehydrate.
pub(crate) async fn permission_grant_rehydrate() -> anyhow::Result<()> {
    use plurora_runtime::{Runtime, RuntimeConfig, SqliteEventStore};
    use std::fs;
    use std::sync::Arc;

    let path =
        std::env::temp_dir().join(format!("plurora-grant-rehydrate-{}.db", std::process::id()));
    if path.exists() {
        fs::remove_file(&path)?;
    }
    let store = Arc::new(SqliteEventStore::open(&path)?);
    let runtime = Runtime::new(store.clone(), RuntimeConfig::default());
    let _session = runtime.open_session(OpenSessionRequest::default()).await?;

    // Grant a permission to a human principal
    let human = json!({"kind": "human", "user_id": "user/rehydrate-test"});
    let grant = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "authority.grant.create",
            json!({"principal": human, "permission": "events.read", "scope": "test-scope", "reason": "rehydrate conformance"}),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.message))?;
    let grant_id = grant["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("grant missing id"))?
        .to_string();

    // Verify the grant is effective before drop
    let grants_before = runtime.list_permission_grants(None).await;
    anyhow::ensure!(
        grants_before
            .iter()
            .any(|g| g.id == grant_id && g.revoked_at.is_none()),
        "grant not found before drop"
    );

    drop(runtime);
    drop(store);

    // Rehydrate from the same SQLite store
    let reopened = Arc::new(SqliteEventStore::open(&path)?);
    let hydrated = Runtime::new(reopened, RuntimeConfig::default());
    hydrated.hydrate_substrate_from_events().await?;

    // Verify the grant survived rehydrate
    let grants_after = hydrated.list_permission_grants(None).await;
    anyhow::ensure!(
        grants_after
            .iter()
            .any(|g| g.id == grant_id && g.revoked_at.is_none()),
        "grant did not survive rehydrate"
    );

    // Revoke and rehydrate again
    hydrated
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "authority.grant.revoke",
            json!({"grant_id": grant_id}),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.message))?;
    drop(hydrated);

    let reopened2 = Arc::new(SqliteEventStore::open(&path)?);
    let hydrated2 = Runtime::new(reopened2, RuntimeConfig::default());
    hydrated2.hydrate_substrate_from_events().await?;
    let grants_final = hydrated2.list_permission_grants(None).await;
    let revived_grant = grants_final.iter().find(|g| g.id == grant_id);
    anyhow::ensure!(
        revived_grant.is_some(),
        "revoked grant not found after rehydrate"
    );
    anyhow::ensure!(
        revived_grant.unwrap().revoked_at.is_some(),
        "revoked grant should have revoked_at after rehydrate"
    );

    let _ = fs::remove_file(path);
    Ok(())
}

/// Secret ref type validation works.
pub(crate) async fn secret_ref_validation() -> anyhow::Result<()> {
    use plurora_core::SecretRef;

    // Valid references
    assert!(SecretRef::is_valid_ref("secret_ref:env:MY_KEY"));
    assert!(SecretRef::is_valid_ref("secretRef:vault:prod/key"));
    assert!(SecretRef::is_valid_ref("secret-ref:file:path"));
    assert!(SecretRef::is_valid_ref("host:my_secret"));

    // Invalid references
    assert!(!SecretRef::is_valid_ref("not_a_ref"));
    assert!(!SecretRef::is_valid_ref("secret_ref:"));
    assert!(!SecretRef::is_valid_ref(""));

    Ok(())
}

/// Raw secret in proposal payload is rejected.
pub(crate) async fn raw_secret_blocked_in_proposal() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();

    // Proposal with raw secret in operation payload should be rejected
    let denied = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "change.proposal.create",
            json!({
                "operations": [
                    {"op": "asset.put", "payload": {"api_key": "sk-abc123def456ghi789jkl012mno345"}}
                ]
            }),
        )
        .await;
    anyhow::ensure!(
        denied.is_err(),
        "proposal with raw secret should be rejected"
    );
    anyhow::ensure!(
        denied.unwrap_err().message.contains("raw secret"),
        "error should mention raw secret"
    );

    // Proposal with secret_ref should be accepted
    let accepted = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "change.proposal.create",
            json!({
                "operations": [
                    {"op": "asset.put", "payload": {"secret": "secret_ref:env:MY_KEY"}}
                ]
            }),
        )
        .await;
    anyhow::ensure!(
        accepted.is_ok(),
        "proposal with secret_ref should be accepted"
    );

    Ok(())
}

pub(crate) async fn effect_receipt_redacts_raw_secret_fields() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(echo_package(
            "example/receipt-secret",
            "example/receipt-secret/echo",
        ))
        .await?;
    let raw_secret = "sk-abc123def456ghi789jkl012mno345";
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("example/receipt-secret/echo".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"api_key": raw_secret, "content": raw_secret}),
        })
        .await?;
    anyhow::ensure!(
        result.output["api_key"] == json!(raw_secret)
            && result.output["content"] == json!(raw_secret),
        "capability output fixture changed unexpectedly"
    );
    let receipt_digest = result
        .receipt
        .as_ref()
        .map(|item| item.digest.as_str())
        .ok_or_else(|| anyhow::anyhow!("effect receipt missing"))?;
    let replay = runtime.replay_effect_receipt(receipt_digest).await?;
    let receipt_value = serde_json::to_value(&replay.receipt)?;
    anyhow::ensure!(
        !plurora_runtime::scan_value_for_raw_secrets(&receipt_value, "receipt").has_findings(),
        "effect receipt contains raw secret material"
    );
    anyhow::ensure!(
        replay.outputs
            == vec![json!({
                "api_key": "<secret:redacted>",
                "content": "<secret:redacted>"
            })],
        "recorded effect output was not redacted"
    );
    Ok(())
}

/// Raw secret in asset metadata is rejected.
pub(crate) async fn raw_secret_blocked_in_asset_metadata() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();

    // Asset with raw secret in metadata should be rejected
    let denied = runtime
        .put_asset(plurora_runtime::runtime::AssetPutRequest {
            origin_package_id: None,
            mime: "application/json".to_string(),
            content: "normal content".to_string(),
            metadata: json!({"api_key": "sk-abc123def456ghi789jkl012mno345"}),
            artifact: None,
        })
        .await;
    anyhow::ensure!(
        denied.is_err(),
        "asset with raw secret in metadata should be rejected"
    );
    anyhow::ensure!(
        denied.unwrap_err().to_string().contains("raw secret"),
        "error should mention raw secret"
    );

    // Asset with secret_ref in metadata should be accepted
    let accepted = runtime
        .put_asset(plurora_runtime::runtime::AssetPutRequest {
            origin_package_id: None,
            mime: "application/json".to_string(),
            content: "normal content".to_string(),
            metadata: json!({"secret": "secret_ref:env:MY_KEY"}),
            artifact: None,
        })
        .await;
    anyhow::ensure!(
        accepted.is_ok(),
        "asset with secret_ref in metadata should be accepted"
    );

    // Asset with arbitrary content (no secret field names) should be accepted
    let content_ok = runtime
        .put_asset(plurora_runtime::runtime::AssetPutRequest {
            origin_package_id: None,
            mime: "text/plain".to_string(),
            content: "sk-abc123def456ghi789jkl012mno345 this looks like a key but it's in content"
                .to_string(),
            metadata: json!({"purpose": "conformance"}),
            artifact: None,
        })
        .await;
    anyhow::ensure!(
        content_ok.is_ok(),
        "asset with non-secret metadata and arbitrary content should be accepted"
    );

    Ok(())
}

/// First-party Packages have no secret bypass.
pub(crate) async fn no_secret_bypass() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();

    // Even an first-party-looking package cannot bypass secret scanning
    let denied = runtime
        .put_asset(plurora_runtime::runtime::AssetPutRequest {
            origin_package_id: Some("plurora/some-lab".to_string()),
            mime: "application/json".to_string(),
            content: "data".to_string(),
            metadata: json!({"api_key": "sk-abc123def456ghi789jkl012mno345"}),
            artifact: None,
        })
        .await;
    anyhow::ensure!(
        denied.is_err(),
        "first-party Package must not bypass secret scanning"
    );

    Ok(())
}

/// Unique env var name per test + process to avoid pollution.
fn unique_env_name(suffix: &str) -> String {
    format!("PLURORA_CONF_ENV_{}_{}", std::process::id(), suffix)
}

struct EnvVarGuard(String);

impl EnvVarGuard {
    fn set(name: &str, value: &str) -> Self {
        std::env::set_var(name, value);
        Self(name.to_string())
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        std::env::remove_var(&self.0);
    }
}

/// EnvSecretResolver allows resolution when the env name is in the allowlist.
pub(crate) async fn env_resolver_allowed() -> anyhow::Result<()> {
    let name = unique_env_name("ALLOWED");
    let _guard = EnvVarGuard::set(&name, "test-value-not-a-provider-key");

    let resolver = Arc::new(EnvSecretResolver::new(HashSet::from([name.clone()])));
    let config = RuntimeConfig {
        secret_resolver: SecretResolverConfig::with_resolver(resolver),
        ..RuntimeConfig::default()
    };
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, config);

    // All supported prefix forms should resolve
    let result = runtime
        .resolve_secret_ref(&format!("secret_ref:env:{}", name))
        .await;
    anyhow::ensure!(
        result.is_ok(),
        "secret_ref:env should resolve: {:?}",
        result
    );
    anyhow::ensure!(
        result.unwrap() == "test-value-not-a-provider-key",
        "resolved value mismatch"
    );

    let result = runtime
        .resolve_secret_ref(&format!("secretRef:env:{}", name))
        .await;
    anyhow::ensure!(result.is_ok(), "secretRef:env should resolve: {:?}", result);

    let result = runtime
        .resolve_secret_ref(&format!("secret-ref:env:{}", name))
        .await;
    anyhow::ensure!(
        result.is_ok(),
        "secret-ref:env should resolve: {:?}",
        result
    );

    let result = runtime
        .resolve_secret_ref(&format!("host:env:{}", name))
        .await;
    anyhow::ensure!(result.is_ok(), "host:env should resolve: {:?}", result);

    Ok(())
}

/// EnvSecretResolver denies resolution when the env name is not in the allowlist.
pub(crate) async fn env_resolver_denied() -> anyhow::Result<()> {
    let name = unique_env_name("DENIED");
    let _guard = EnvVarGuard::set(&name, "should-not-be-returned");

    // Empty allowlist — env name is not allowed
    let resolver = Arc::new(EnvSecretResolver::new(HashSet::new()));
    let config = RuntimeConfig {
        secret_resolver: SecretResolverConfig::with_resolver(resolver),
        ..RuntimeConfig::default()
    };
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, config);

    let result = runtime
        .resolve_secret_ref(&format!("secret_ref:env:{}", name))
        .await;
    anyhow::ensure!(result.is_err(), "denied env should fail");
    let err_msg = result.unwrap_err().to_string();
    anyhow::ensure!(
        err_msg.contains("not in allowlist"),
        "error should mention allowlist: {err_msg}"
    );
    anyhow::ensure!(
        !err_msg.contains("should-not-be-returned"),
        "error must not leak raw value: {err_msg}"
    );

    // Non-env vault should also be rejected
    let result = runtime
        .resolve_secret_ref("secret_ref:vault:prod/openai")
        .await;
    anyhow::ensure!(result.is_err(), "non-env vault should be rejected");

    // host:<non-env-key> should not be treated as env
    let result = runtime.resolve_secret_ref("host:my_secret").await;
    anyhow::ensure!(result.is_err(), "host:my_secret should not resolve as env");

    Ok(())
}

/// EnvSecretResolver returns typed error for missing env var without leaking raw value.
pub(crate) async fn env_resolver_missing_no_leak() -> anyhow::Result<()> {
    let name = unique_env_name("MISSING");
    // Ensure the env var is NOT set
    std::env::remove_var(&name);

    let resolver = Arc::new(EnvSecretResolver::new(HashSet::from([name.clone()])));
    let config = RuntimeConfig {
        secret_resolver: SecretResolverConfig::with_resolver(resolver),
        ..RuntimeConfig::default()
    };
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, config);

    let result = runtime
        .resolve_secret_ref(&format!("secret_ref:env:{}", name))
        .await;
    anyhow::ensure!(result.is_err(), "missing env should fail");
    let err_msg = result.unwrap_err().to_string();
    anyhow::ensure!(
        err_msg.contains("not set"),
        "error should mention 'not set': {err_msg}"
    );
    anyhow::ensure!(
        err_msg.contains(&name),
        "error should contain env name for debugging: {err_msg}"
    );

    // Even if the env var exists, denied env name must not leak the value in errors
    let existing_name = unique_env_name("NOLEAK");
    let _guard = EnvVarGuard::set(&existing_name, "super-secret-value-xyz");
    let resolver2 = Arc::new(EnvSecretResolver::new(HashSet::new())); // empty allowlist
    let config2 = RuntimeConfig {
        secret_resolver: SecretResolverConfig::with_resolver(resolver2),
        ..RuntimeConfig::default()
    };
    let store2 = Arc::new(InMemoryEventStore::default());
    let runtime2 = Runtime::new(store2, config2);

    let result2 = runtime2
        .resolve_secret_ref(&format!("secret_ref:env:{}", existing_name))
        .await;
    anyhow::ensure!(result2.is_err(), "denied env should fail");
    let err_msg2 = result2.unwrap_err().to_string();
    anyhow::ensure!(
        !err_msg2.contains("super-secret-value-xyz"),
        "error must not leak raw env value: {err_msg2}"
    );

    Ok(())
}
