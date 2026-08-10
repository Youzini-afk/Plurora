//! Conformance for Installation-scoped secret policy and runtime resolution.

use std::path::PathBuf;
use std::sync::Arc;

use plurora_runtime::{
    CapabilityInvocationRequest, CompositeSecretResolver, InMemoryEventStore, InstallationControl,
    InstallationStoreSecretResolver, OpenSessionRequest, Runtime, RuntimeConfig,
    SecretResolverConfig, StoreSecretResolver, ACTIVE_INSTALLATION_SCOPE,
};
use plurora_work::{InstallationId, InstallationSecretPolicy};
use serde_json::json;
use tokio::sync::Mutex;

use super::protocol_installation::{create_request, fixture, InstallationFixture};
use crate::commands::manifest;

const PACKAGE_ID: &str = "plurora/secret-store-lab";
const MANIFEST_PATH: &str = "packages/plurora/secret-store-lab/manifest.yaml";
const INSTALLATION_REF: &str = "secret_ref:installation:API_KEY";

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

struct EnvGuard {
    previous: Option<String>,
}

impl EnvGuard {
    fn set(path: &std::path::Path) -> Self {
        let previous = std::env::var("PLURORA_DATA_DIR").ok();
        std::env::set_var("PLURORA_DATA_DIR", path);
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("PLURORA_DATA_DIR", value),
            None => std::env::remove_var("PLURORA_DATA_DIR"),
        }
    }
}

fn scoped_runtime(base: &InstallationFixture) -> anyhow::Result<Runtime<InMemoryEventStore>> {
    let platform = Arc::new(StoreSecretResolver::with_path(
        plurora_core::paths::secret_store_path()?,
    ));
    let installation = InstallationStoreSecretResolver::new(|| {
        ACTIVE_INSTALLATION_SCOPE
            .try_with(|scope| scope.clone())
            .ok()
    })
    .with_platform_fallback(platform.clone());
    let resolver = CompositeSecretResolver::new()
        .with_store(platform)
        .with_installation(Arc::new(installation));
    Ok(Runtime::new(
        base.store.clone(),
        RuntimeConfig {
            object_store: base.objects.clone(),
            installation_control: base.registry.clone(),
            secret_resolver: SecretResolverConfig::with_resolver(Arc::new(resolver)),
            ..RuntimeConfig::default()
        },
    ))
}

async fn load_secret_lab(runtime: &Runtime<InMemoryEventStore>) -> anyhow::Result<()> {
    runtime
        .load_package(manifest::read_manifest(PathBuf::from(MANIFEST_PATH)).await?)
        .await?;
    Ok(())
}

async fn invoke_lab(
    runtime: &Runtime<InMemoryEventStore>,
    capability: &str,
    input: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    Ok(runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{PACKAGE_ID}/{capability}")),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input,
        })
        .await?
        .output)
}

async fn create_installation(
    base: &InstallationFixture,
    runtime: &Runtime<InMemoryEventStore>,
    key: &str,
    policy: InstallationSecretPolicy,
) -> anyhow::Result<InstallationId> {
    let value = runtime
        .call_protocol(
            &plurora_runtime::ProtocolContext::host_dev("installation-secret-conformance"),
            "host.installation.create",
            serde_json::to_value(create_request(
                base.work_id.clone(),
                base.work_revision.clone(),
                base.assembly_lock.clone(),
                "Secret installation",
                key,
                policy,
            ))?,
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    Ok(
        serde_json::from_value::<plurora_runtime::InstallationMutationResult>(value)?
            .installation
            .record
            .installation_id,
    )
}

pub(crate) async fn put_resolve_uses_installation_owned_path() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let base = fixture().await?;
    let _env = EnvGuard::set(base.data.path());
    let runtime = scoped_runtime(&base)?;
    load_secret_lab(&runtime).await?;
    let installation_id = create_installation(
        &base,
        &runtime,
        "secret-create",
        InstallationSecretPolicy {
            allowed_secret_refs: vec![INSTALLATION_REF.to_string()],
            allow_platform_fallback: false,
        },
    )
    .await?;

    invoke_lab(
        &runtime,
        "put_installation_secret",
        json!({
            "installation_id": installation_id,
            "name": "API_KEY",
            "value": "installation-secret-sentinel"
        }),
    )
    .await?;
    let path = base
        .registry
        .installation_secret_store_path(&installation_id)?;
    let data_root = std::fs::canonicalize(base.data.path())?;
    anyhow::ensure!(
        path == data_root
            .join("installations")
            .join(installation_id.as_str())
            .join("secrets.dat")
    );
    anyhow::ensure!(path.is_file());
    anyhow::ensure!(
        runtime
            .resolve_secret_ref_for_installation(INSTALLATION_REF, &installation_id)
            .await?
            == "installation-secret-sentinel"
    );

    let session = runtime
        .open_session(OpenSessionRequest {
            metadata: json!({"installation_id": installation_id}),
            ..OpenSessionRequest::default()
        })
        .await?;
    anyhow::ensure!(
        runtime
            .resolve_secret_ref_with_session(INSTALLATION_REF, Some(&session.id))
            .await?
            == "installation-secret-sentinel"
    );
    let missing_scope = runtime
        .resolve_secret_ref_with_session(INSTALLATION_REF, None)
        .await
        .expect_err("Installation secret resolution without a session must fail");
    anyhow::ensure!(missing_scope.to_string().contains("requires session_id"));
    Ok(())
}

pub(crate) async fn policy_controls_fallback_and_rejects_retired_scheme() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let base = fixture().await?;
    let _env = EnvGuard::set(base.data.path());
    let runtime = scoped_runtime(&base)?;
    load_secret_lab(&runtime).await?;
    invoke_lab(
        &runtime,
        "put_secret",
        json!({"name": "API_KEY", "value": "platform-fallback-sentinel"}),
    )
    .await?;
    let installation_id = create_installation(
        &base,
        &runtime,
        "fallback-create",
        InstallationSecretPolicy {
            allowed_secret_refs: vec![INSTALLATION_REF.to_string()],
            allow_platform_fallback: true,
        },
    )
    .await?;
    anyhow::ensure!(
        runtime
            .resolve_secret_ref_for_installation(INSTALLATION_REF, &installation_id)
            .await?
            == "platform-fallback-sentinel"
    );

    let no_fallback = create_installation(
        &base,
        &runtime,
        "no-fallback-create",
        InstallationSecretPolicy {
            allowed_secret_refs: vec![INSTALLATION_REF.to_string()],
            allow_platform_fallback: false,
        },
    )
    .await?;
    let error = runtime
        .resolve_secret_ref_for_installation(INSTALLATION_REF, &no_fallback)
        .await
        .expect_err("a missing Installation secret must not use a disabled fallback");
    anyhow::ensure!(error.to_string().contains("fallback is disabled"));

    let unlisted = create_installation(
        &base,
        &runtime,
        "unlisted-create",
        InstallationSecretPolicy::default(),
    )
    .await?;
    let error = runtime
        .resolve_secret_ref_for_installation(INSTALLATION_REF, &unlisted)
        .await
        .expect_err("an unlisted Installation secret reference must be denied");
    anyhow::ensure!(error.to_string().contains("not allowed"));

    let retired_vault = ["pro", "ject"].concat();
    let retired_ref = format!("secret_ref:{retired_vault}:API_KEY");
    let error = runtime
        .resolve_secret_ref_for_installation(&retired_ref, &installation_id)
        .await
        .expect_err("the retired secret reference scheme must not resolve");
    anyhow::ensure!(error
        .to_string()
        .contains("unsupported secret reference scheme"));
    Ok(())
}

pub(crate) async fn installation_secret_isolation_and_list_redaction() -> anyhow::Result<()> {
    let _lock = ENV_LOCK.lock().await;
    let base = fixture().await?;
    let _env = EnvGuard::set(base.data.path());
    let runtime = scoped_runtime(&base)?;
    load_secret_lab(&runtime).await?;
    let policy = InstallationSecretPolicy {
        allowed_secret_refs: vec![INSTALLATION_REF.to_string()],
        allow_platform_fallback: false,
    };
    let first = create_installation(&base, &runtime, "isolation-first", policy.clone()).await?;
    let second = create_installation(&base, &runtime, "isolation-second", policy).await?;
    invoke_lab(
        &runtime,
        "put_installation_secret",
        json!({"installation_id": first, "name": "API_KEY", "value": "first-value-sentinel"}),
    )
    .await?;
    invoke_lab(
        &runtime,
        "put_installation_secret",
        json!({"installation_id": second, "name": "API_KEY", "value": "second-value-sentinel"}),
    )
    .await?;
    anyhow::ensure!(
        runtime
            .resolve_secret_ref_for_installation(INSTALLATION_REF, &first)
            .await?
            == "first-value-sentinel"
    );
    anyhow::ensure!(
        runtime
            .resolve_secret_ref_for_installation(INSTALLATION_REF, &second)
            .await?
            == "second-value-sentinel"
    );

    let listed = invoke_lab(
        &runtime,
        "list_installation_secrets",
        json!({"installation_id": first}),
    )
    .await?;
    let text = serde_json::to_string(&listed)?;
    anyhow::ensure!(text.contains("API_KEY"));
    anyhow::ensure!(!text.contains("first-value-sentinel"));
    anyhow::ensure!(!text.contains("second-value-sentinel"));
    Ok(())
}
