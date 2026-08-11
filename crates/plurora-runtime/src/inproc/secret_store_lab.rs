//! Handler for `plurora/secret-store-lab` capabilities.

use anyhow::Result;
use plurora_work::InstallationId;
use serde::Deserialize;
use serde_json::Value;

use crate::installation_control::InstallationSecretStoreGuard;
use crate::secret_store::{
    current_key_source, load_store, resolve_master_key, save_store, validate_secret_name,
    validate_secret_value,
};

use super::InprocInvocation;

const PACKAGE_ID: &str = "plurora/secret-store-lab";
const INSTALLATION_STORE_UNAVAILABLE: &str = "installation secret store unavailable";
const INSTALLATION_REQUEST_INVALID: &str = "installation secret request invalid";

#[derive(Debug, Deserialize)]
struct NameInput {
    name: String,
}

#[derive(Debug, Deserialize)]
struct PutSecretInput {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct InstallationNameInput {
    installation_id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct PutInstallationSecretInput {
    installation_id: String,
    name: String,
    value: String,
}

pub async fn try_handle(request: &InprocInvocation) -> Option<Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }

    match request.capability_id.as_str() {
        "secret-store.put_secret" | "plurora/secret-store-lab/put_secret" => {
            Some(put_secret(request.input.clone()))
        }
        "secret-store.has_secret" | "plurora/secret-store-lab/has_secret" => {
            Some(has_secret(request.input.clone()))
        }
        "secret-store.list_secrets" | "plurora/secret-store-lab/list_secrets" => {
            Some(list_secrets())
        }
        "secret-store.delete_secret" | "plurora/secret-store-lab/delete_secret" => {
            Some(delete_secret(request.input.clone()))
        }
        "secret-store.put_installation_secret"
        | "plurora/secret-store-lab/put_installation_secret" => {
            Some(put_installation_secret(request.input.clone()).await)
        }
        "secret-store.has_installation_secret"
        | "plurora/secret-store-lab/has_installation_secret" => {
            Some(has_installation_secret(request.input.clone()).await)
        }
        "secret-store.list_installation_secrets"
        | "plurora/secret-store-lab/list_installation_secrets" => {
            Some(list_installation_secrets(request.input.clone()).await)
        }
        "secret-store.delete_installation_secret"
        | "plurora/secret-store-lab/delete_installation_secret" => {
            Some(delete_installation_secret(request.input.clone()).await)
        }
        "secret-store.health" | "plurora/secret-store-lab/health" => Some(health()),
        _ => None,
    }
}

fn store_path() -> Result<std::path::PathBuf> {
    plurora_core::paths::secret_store_path()
}

fn put_secret(input: Value) -> Result<Value> {
    let input: PutSecretInput = serde_json::from_value(input)?;
    validate_secret_name(&input.name)?;
    validate_secret_value(&input.value)?;

    let (identity, _) = resolve_master_key()?;
    let recipient = identity.to_public();
    let path = store_path()?;
    let mut store = load_store(&path, &identity)?;
    let created = !store.secrets.contains_key(&input.name);
    store.secrets.insert(input.name.clone(), input.value);
    save_store(&path, &store, &recipient)?;

    Ok(serde_json::json!({
        "name": input.name,
        "stored": true,
        "created": created,
    }))
}

fn has_secret(input: Value) -> Result<Value> {
    let input: NameInput = serde_json::from_value(input)?;
    validate_secret_name(&input.name)?;

    let (identity, _) = resolve_master_key()?;
    let path = store_path()?;
    let store = load_store(&path, &identity)?;
    Ok(serde_json::json!({
        "name": input.name,
        "exists": store.secrets.contains_key(&input.name),
    }))
}

fn list_secrets() -> Result<Value> {
    let (identity, _) = resolve_master_key()?;
    let path = store_path()?;
    let store = load_store(&path, &identity)?;
    let names: Vec<String> = store.secrets.keys().cloned().collect();
    Ok(serde_json::json!({ "names": names }))
}

fn delete_secret(input: Value) -> Result<Value> {
    let input: NameInput = serde_json::from_value(input)?;
    validate_secret_name(&input.name)?;

    let (identity, _) = resolve_master_key()?;
    let recipient = identity.to_public();
    let path = store_path()?;
    let mut store = load_store(&path, &identity)?;
    let removed = store.secrets.remove(&input.name).is_some();
    save_store(&path, &store, &recipient)?;

    Ok(serde_json::json!({
        "name": input.name,
        "removed": removed,
    }))
}

fn installation_store_error() -> anyhow::Error {
    anyhow::anyhow!(INSTALLATION_STORE_UNAVAILABLE)
}

fn parse_installation_id(value: String) -> Result<InstallationId> {
    InstallationId::parse(value).map_err(|_| anyhow::anyhow!(INSTALLATION_REQUEST_INVALID))
}

async fn ready_installation_store(
    installation_id: &InstallationId,
) -> Result<InstallationSecretStoreGuard> {
    let control =
        super::installation_control_from_inproc().map_err(|_| installation_store_error())?;
    let view = control
        .get(installation_id)
        .await
        .map_err(|_| installation_store_error())?
        .ok_or_else(installation_store_error)?;
    anyhow::ensure!(
        view.record.installation_id == *installation_id
            && view.record.status == plurora_work::InstallationStatus::Ready,
        INSTALLATION_STORE_UNAVAILABLE
    );
    let expected_revision = view.revision;
    let guard = control
        .acquire_ready_secret_store(installation_id, expected_revision)
        .await
        .map_err(|_| installation_store_error())?;
    anyhow::ensure!(
        guard.installation_id() == installation_id && guard.revision() == expected_revision,
        INSTALLATION_STORE_UNAVAILABLE
    );
    Ok(guard)
}

async fn put_installation_secret(input: Value) -> Result<Value> {
    let input: PutInstallationSecretInput =
        serde_json::from_value(input).map_err(|_| anyhow::anyhow!(INSTALLATION_REQUEST_INVALID))?;
    let installation_id = parse_installation_id(input.installation_id)?;
    let store_guard = ready_installation_store(&installation_id).await?;
    validate_secret_name(&input.name)?;
    validate_secret_value(&input.value)?;

    let (identity, _) = resolve_master_key().map_err(|_| installation_store_error())?;
    let recipient = identity.to_public();
    let mut store =
        load_store(store_guard.path(), &identity).map_err(|_| installation_store_error())?;
    let created = !store.secrets.contains_key(&input.name);
    store.secrets.insert(input.name.clone(), input.value);
    save_store(store_guard.path(), &store, &recipient).map_err(|_| installation_store_error())?;

    Ok(serde_json::json!({
        "installation_id": installation_id.as_str(),
        "name": input.name,
        "stored": true,
        "created": created,
    }))
}

async fn has_installation_secret(input: Value) -> Result<Value> {
    let input: InstallationNameInput =
        serde_json::from_value(input).map_err(|_| anyhow::anyhow!(INSTALLATION_REQUEST_INVALID))?;
    let installation_id = parse_installation_id(input.installation_id)?;
    let store_guard = ready_installation_store(&installation_id).await?;
    validate_secret_name(&input.name)?;

    let (identity, _) = resolve_master_key().map_err(|_| installation_store_error())?;
    let store =
        load_store(store_guard.path(), &identity).map_err(|_| installation_store_error())?;
    Ok(serde_json::json!({
        "installation_id": installation_id.as_str(),
        "name": input.name,
        "exists": store.secrets.contains_key(&input.name),
    }))
}

async fn list_installation_secrets(input: Value) -> Result<Value> {
    #[derive(Debug, Deserialize)]
    struct ListInstallationSecretsInput {
        installation_id: String,
    }

    let input: ListInstallationSecretsInput =
        serde_json::from_value(input).map_err(|_| anyhow::anyhow!(INSTALLATION_REQUEST_INVALID))?;
    let installation_id = parse_installation_id(input.installation_id)?;
    let store_guard = ready_installation_store(&installation_id).await?;
    let (identity, _) = resolve_master_key().map_err(|_| installation_store_error())?;
    let store =
        load_store(store_guard.path(), &identity).map_err(|_| installation_store_error())?;
    let names: Vec<String> = store.secrets.keys().cloned().collect();
    Ok(serde_json::json!({
        "installation_id": installation_id.as_str(),
        "names": names,
    }))
}

async fn delete_installation_secret(input: Value) -> Result<Value> {
    let input: InstallationNameInput =
        serde_json::from_value(input).map_err(|_| anyhow::anyhow!(INSTALLATION_REQUEST_INVALID))?;
    let installation_id = parse_installation_id(input.installation_id)?;
    let store_guard = ready_installation_store(&installation_id).await?;
    validate_secret_name(&input.name)?;

    let (identity, _) = resolve_master_key().map_err(|_| installation_store_error())?;
    let recipient = identity.to_public();
    let mut store =
        load_store(store_guard.path(), &identity).map_err(|_| installation_store_error())?;
    let removed = store.secrets.remove(&input.name).is_some();
    save_store(store_guard.path(), &store, &recipient).map_err(|_| installation_store_error())?;

    Ok(serde_json::json!({
        "installation_id": installation_id.as_str(),
        "name": input.name,
        "removed": removed,
    }))
}

fn health() -> Result<Value> {
    let path = store_path()?;
    let exists = path.exists();
    let key_source = current_key_source()?;
    let secret_count = if exists {
        let (identity, _) = resolve_master_key()?;
        load_store(&path, &identity)?.secrets.len()
    } else {
        0
    };

    Ok(serde_json::json!({
        "store_path": path.display().to_string(),
        "exists": exists,
        "secret_count": secret_count,
        "key_source": key_source.as_str(),
    }))
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use async_trait::async_trait;
    use plurora_core::ArtifactDescriptor;
    use plurora_work::{
        AcquisitionKind, AcquisitionRecord, InstallationRecord, InstallationSecretPolicy,
        InstallationStatus, WorkId,
    };
    use tokio::sync::Mutex;

    use super::*;
    use crate::{
        InMemoryEventStore, InstallationControl, InstallationCreateRequest,
        InstallationListRequest, InstallationMutationResult, InstallationRemoveRequest,
        InstallationSecretStoreGuard, InstallationUpdateRequest, InstallationView,
        InstallationWorkSummary, Runtime, RuntimeConfig,
    };

    static ENV_LOCK: Mutex<()> = Mutex::const_new(());

    struct DataDirGuard {
        previous: Option<OsString>,
        _lock: tokio::sync::MutexGuard<'static, ()>,
    }

    impl DataDirGuard {
        async fn set(path: &std::path::Path) -> Self {
            let lock = ENV_LOCK.lock().await;
            let previous = std::env::var_os("PLURORA_DATA_DIR");
            std::env::set_var("PLURORA_DATA_DIR", path);
            Self {
                previous,
                _lock: lock,
            }
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

    struct TestInstallationControl {
        installation_id: InstallationId,
        state: Arc<Mutex<Option<InstallationView>>>,
        path: PathBuf,
        transition_after_get: AtomicBool,
    }

    impl TestInstallationControl {
        fn new(
            status: Option<InstallationStatus>,
            path: PathBuf,
            transition_after_get: bool,
        ) -> Arc<Self> {
            let installation_id = InstallationId::new();
            let view = status.map(|status| installation_view(installation_id.clone(), status));
            Arc::new(Self {
                installation_id,
                state: Arc::new(Mutex::new(view)),
                path,
                transition_after_get: AtomicBool::new(transition_after_get),
            })
        }
    }

    #[async_trait]
    impl InstallationControl for TestInstallationControl {
        async fn list(
            &self,
            _request: InstallationListRequest,
        ) -> anyhow::Result<Vec<InstallationView>> {
            Ok(self.state.lock().await.clone().into_iter().collect())
        }

        async fn get(
            &self,
            installation_id: &InstallationId,
        ) -> anyhow::Result<Option<InstallationView>> {
            if installation_id != &self.installation_id {
                return Ok(None);
            }
            let mut state = self.state.lock().await;
            let result = state.clone();
            if self.transition_after_get.swap(false, Ordering::SeqCst) {
                if let Some(view) = state.as_mut() {
                    view.revision += 1;
                    view.record.status = InstallationStatus::Removed;
                }
            }
            Ok(result)
        }

        async fn create(
            &self,
            _request: InstallationCreateRequest,
        ) -> anyhow::Result<InstallationMutationResult> {
            anyhow::bail!("not used")
        }

        async fn update(
            &self,
            _request: InstallationUpdateRequest,
        ) -> anyhow::Result<InstallationMutationResult> {
            anyhow::bail!("not used")
        }

        async fn remove(
            &self,
            _request: InstallationRemoveRequest,
        ) -> anyhow::Result<InstallationMutationResult> {
            anyhow::bail!("not used")
        }

        fn installation_secret_store_path(
            &self,
            _installation_id: &InstallationId,
        ) -> anyhow::Result<PathBuf> {
            Ok(self.path.clone())
        }

        async fn acquire_ready_secret_store(
            &self,
            installation_id: &InstallationId,
            expected_revision: u64,
        ) -> anyhow::Result<InstallationSecretStoreGuard> {
            let lease = self.state.clone().lock_owned().await;
            let view = lease
                .as_ref()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("installation unavailable"))?;
            InstallationSecretStoreGuard::verified(
                installation_id,
                expected_revision,
                &view,
                self.path.clone(),
                Box::new(lease),
            )
        }
    }

    fn installation_view(
        installation_id: InstallationId,
        status: InstallationStatus,
    ) -> InstallationView {
        let artifact = ArtifactDescriptor {
            artifact_type_uri: "urn:plurora:test:artifact:v1".to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let now = chrono::Utc::now();
        InstallationView {
            work_summary: InstallationWorkSummary {
                work_id: WorkId::parse("tests/secret-store-lab").expect("valid Work id"),
                title: "Secret gate fixture".to_string(),
                description: String::new(),
                content_roots: Vec::new(),
                entrypoints: Vec::new(),
                rights: None,
                transparency: None,
                operational_intent: None,
                annotations: BTreeMap::new(),
            },
            record: InstallationRecord {
                schema_version: InstallationRecord::SCHEMA_VERSION,
                installation_id,
                work_revision: artifact.clone(),
                assembly_lock: artifact,
                display_name: "Secret gate fixture".to_string(),
                source: AcquisitionRecord {
                    kind: AcquisitionKind::LocalImport,
                    source_ref: None,
                    provenance_refs: Vec::new(),
                    update_channel: None,
                },
                state_bindings: Vec::new(),
                secret_policy: InstallationSecretPolicy::default(),
                created_at: now,
                updated_at: now,
                status,
            },
            revision: 1,
            rollback: None,
        }
    }

    async fn invoke_installation_capability(
        control: Arc<TestInstallationControl>,
        capability_id: &str,
        input: Value,
    ) -> Result<Value> {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig {
                installation_control: control,
                ..RuntimeConfig::default()
            },
        );
        let request = InprocInvocation {
            capability_id: capability_id.to_string(),
            provider_package_id: PACKAGE_ID.to_string(),
            session_id: None,
            input,
        };
        crate::inproc::with_runtime_invoker(
            runtime,
            None,
            PACKAGE_ID.to_string(),
            HashMap::new(),
            async {
                try_handle(&request)
                    .await
                    .expect("secret-store capability must be handled")
            },
        )
        .await
    }

    fn request_input(capability_id: &str, installation_id: &InstallationId) -> Value {
        match capability_id {
            "plurora/secret-store-lab/put_installation_secret" => serde_json::json!({
                "installation_id": installation_id.as_str(),
                "name": "PRIVATE_TOKEN",
                "value": "sensitive-fixture-value"
            }),
            "plurora/secret-store-lab/list_installation_secrets" => serde_json::json!({
                "installation_id": installation_id.as_str()
            }),
            _ => serde_json::json!({
                "installation_id": installation_id.as_str(),
                "name": "PRIVATE_TOKEN"
            }),
        }
    }

    fn assert_redacted_error(error: anyhow::Error, control: &TestInstallationControl) {
        let message = error.to_string();
        assert_eq!(message, INSTALLATION_STORE_UNAVAILABLE);
        assert!(!message.contains(control.installation_id.as_str()));
        assert!(!message.contains("PRIVATE_TOKEN"));
        assert!(!message.contains("sensitive-fixture-value"));
        assert!(!message.contains(control.path.to_string_lossy().as_ref()));
    }

    #[tokio::test]
    async fn installation_secret_operations_require_a_current_ready_revision() -> Result<()> {
        let data = tempfile::tempdir()?;
        let _data_dir = DataDirGuard::set(data.path()).await;
        let capabilities = [
            "plurora/secret-store-lab/put_installation_secret",
            "plurora/secret-store-lab/has_installation_secret",
            "plurora/secret-store-lab/list_installation_secrets",
            "plurora/secret-store-lab/delete_installation_secret",
        ];

        for status in [
            Some(InstallationStatus::Resolving),
            Some(InstallationStatus::Updating),
            Some(InstallationStatus::Blocked),
            Some(InstallationStatus::Failed),
            Some(InstallationStatus::Removing),
            Some(InstallationStatus::Removed),
            None,
        ] {
            for capability_id in capabilities {
                let path = data
                    .path()
                    .join("installations")
                    .join(InstallationId::new().as_str())
                    .join("secrets.dat");
                let control = TestInstallationControl::new(status, path.clone(), false);
                let error = invoke_installation_capability(
                    control.clone(),
                    capability_id,
                    request_input(capability_id, &control.installation_id),
                )
                .await
                .expect_err("non-Ready or unknown Installation must fail closed");
                assert_redacted_error(error, &control);
                assert!(!path.exists());
            }
        }
        assert!(!data.path().join("secret-store.key").exists());

        let absent_path = data
            .path()
            .join("installations")
            .join("race-absent")
            .join("secrets.dat");
        std::fs::create_dir_all(absent_path.parent().expect("race path parent"))?;
        let control = TestInstallationControl::new(
            Some(InstallationStatus::Ready),
            absent_path.clone(),
            true,
        );
        let error = invoke_installation_capability(
            control.clone(),
            capabilities[0],
            request_input(capabilities[0], &control.installation_id),
        )
        .await
        .expect_err("transition after initial check must reject before write");
        assert_redacted_error(error, &control);
        assert!(!absent_path.exists());

        let existing_path = data
            .path()
            .join("installations")
            .join("race-existing")
            .join("secrets.dat");
        std::fs::create_dir_all(existing_path.parent().expect("existing path parent"))?;
        let (identity, _) = resolve_master_key()?;
        let recipient = identity.to_public();
        let mut existing = crate::secret_store::StoreFile::empty();
        existing
            .secrets
            .insert("PRIVATE_TOKEN".to_string(), "original-value".to_string());
        save_store(&existing_path, &existing, &recipient)?;
        let before = std::fs::read(&existing_path)?;
        let control = TestInstallationControl::new(
            Some(InstallationStatus::Ready),
            existing_path.clone(),
            true,
        );
        let error = invoke_installation_capability(
            control.clone(),
            capabilities[0],
            request_input(capabilities[0], &control.installation_id),
        )
        .await
        .expect_err("stale revision must not overwrite an existing store");
        assert_redacted_error(error, &control);
        assert_eq!(std::fs::read(&existing_path)?, before);

        let ready_path = data
            .path()
            .join("installations")
            .join("ready")
            .join("secrets.dat");
        std::fs::create_dir_all(ready_path.parent().expect("ready path parent"))?;
        let ready = TestInstallationControl::new(
            Some(InstallationStatus::Ready),
            ready_path.clone(),
            false,
        );
        let put = invoke_installation_capability(
            ready.clone(),
            capabilities[0],
            request_input(capabilities[0], &ready.installation_id),
        )
        .await?;
        assert_eq!(put.get("stored").and_then(Value::as_bool), Some(true));
        assert!(ready_path.exists());

        let has = invoke_installation_capability(
            ready.clone(),
            capabilities[1],
            request_input(capabilities[1], &ready.installation_id),
        )
        .await?;
        assert_eq!(has.get("exists").and_then(Value::as_bool), Some(true));

        let list = invoke_installation_capability(
            ready.clone(),
            capabilities[2],
            request_input(capabilities[2], &ready.installation_id),
        )
        .await?;
        assert_eq!(list["names"], serde_json::json!(["PRIVATE_TOKEN"]));

        let delete = invoke_installation_capability(
            ready.clone(),
            capabilities[3],
            request_input(capabilities[3], &ready.installation_id),
        )
        .await?;
        assert_eq!(delete.get("removed").and_then(Value::as_bool), Some(true));
        let has = invoke_installation_capability(
            ready.clone(),
            capabilities[1],
            request_input(capabilities[1], &ready.installation_id),
        )
        .await?;
        assert_eq!(has.get("exists").and_then(Value::as_bool), Some(false));
        Ok(())
    }
}
