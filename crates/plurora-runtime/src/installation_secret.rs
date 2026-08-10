//! Installation-scoped secret resolution with optional platform-store fallback.

use std::sync::Arc;

use async_trait::async_trait;
use plurora_work::{InstallationId, InstallationSecretPolicy};

use crate::installation_control::InstallationControl;
use crate::secret::{HostSecretResolver, StoreSecretResolver};
use crate::secret_store::{load_store, resolve_master_key};

pub struct InstallationStoreSecretResolver {
    active_scope: Arc<dyn Fn() -> Option<InstallationScopeContext> + Send + Sync>,
    platform_fallback: Option<Arc<StoreSecretResolver>>,
    reader: Arc<dyn InstallationSecretReader>,
}

#[derive(Clone)]
pub struct InstallationScopeContext {
    pub installation_id: InstallationId,
    pub revision: u64,
    pub secret_policy: InstallationSecretPolicy,
    pub installation_control: Arc<dyn InstallationControl>,
}

impl std::fmt::Debug for InstallationScopeContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstallationScopeContext")
            .field("installation_id", &"redacted")
            .field("revision", &self.revision)
            .field(
                "allowed_secret_ref_count",
                &self.secret_policy.allowed_secret_refs.len(),
            )
            .field(
                "allow_platform_fallback",
                &self.secret_policy.allow_platform_fallback,
            )
            .field("installation_control", &"configured")
            .finish()
    }
}

#[async_trait]
trait InstallationSecretReader: Send + Sync + 'static {
    async fn read(
        &self,
        store_path: &std::path::Path,
        name: &str,
    ) -> anyhow::Result<Option<String>>;
}

struct FileInstallationSecretReader;

#[async_trait]
impl InstallationSecretReader for FileInstallationSecretReader {
    async fn read(
        &self,
        store_path: &std::path::Path,
        name: &str,
    ) -> anyhow::Result<Option<String>> {
        read_installation_secret(store_path, name).await
    }
}

impl InstallationStoreSecretResolver {
    pub fn new<F>(active_scope: F) -> Self
    where
        F: Fn() -> Option<InstallationScopeContext> + Send + Sync + 'static,
    {
        Self {
            active_scope: Arc::new(active_scope),
            platform_fallback: None,
            reader: Arc::new(FileInstallationSecretReader),
        }
    }

    pub fn with_platform_fallback(mut self, platform: Arc<StoreSecretResolver>) -> Self {
        self.platform_fallback = Some(platform);
        self
    }

    #[cfg(test)]
    fn with_reader(mut self, reader: Arc<dyn InstallationSecretReader>) -> Self {
        self.reader = reader;
        self
    }
}

impl std::fmt::Debug for InstallationStoreSecretResolver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstallationStoreSecretResolver")
            .field("has_platform_fallback", &self.platform_fallback.is_some())
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl HostSecretResolver for InstallationStoreSecretResolver {
    async fn resolve(&self, ref_id: &str) -> anyhow::Result<String> {
        let name =
            plurora_core::secret_ref::extract_installation_name(ref_id).ok_or_else(|| {
                anyhow::anyhow!("secret resolution denied: reference is not installation-scoped")
            })?;
        let scope = (self.active_scope)().ok_or_else(|| {
            anyhow::anyhow!("secret resolution failed: no active installation scope")
        })?;

        let allowed = scope
            .secret_policy
            .allowed_secret_refs
            .iter()
            .any(|reference| reference == ref_id);
        anyhow::ensure!(
            allowed,
            "secret resolution denied: reference is not allowed by the installation policy"
        );

        let guard = scope
            .installation_control
            .acquire_ready_secret_store(&scope.installation_id, scope.revision)
            .await
            .map_err(|_| anyhow::anyhow!("installation secret store unavailable"))?;
        anyhow::ensure!(
            guard.installation_id() == &scope.installation_id && guard.revision() == scope.revision,
            "installation secret store unavailable"
        );

        let installation_value = self
            .reader
            .read(guard.path(), name)
            .await
            .map_err(|_| anyhow::anyhow!("installation secret store unavailable"))?;
        let resolved = match installation_value {
            Some(value) => Ok(value),
            None if !scope.secret_policy.allow_platform_fallback => anyhow::bail!(
                "secret resolution failed: installation entry is absent and platform fallback is disabled"
            ),
            None => {
                let platform = self.platform_fallback.as_ref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "secret resolution failed: platform fallback resolver is unavailable"
                    )
                })?;
                platform
                    .resolve(&format!("secret_ref:store:{name}"))
                    .await
                    .map_err(|_| anyhow::anyhow!("platform secret resolution failed"))
            }
        };
        // Keep the lifecycle lease through the entire one-shot resolution,
        // including an explicitly allowed platform fallback. A long-lived scope
        // never owns this guard; the next read must reacquire and revalidate it.
        drop(guard);
        resolved
    }
}

async fn read_installation_secret(
    store_path: &std::path::Path,
    name: &str,
) -> anyhow::Result<Option<String>> {
    if !store_path.exists() {
        return Ok(None);
    }
    let path_owned = store_path.to_path_buf();
    let name_owned = name.to_string();
    tokio::task::spawn_blocking(move || {
        let (key, _) = resolve_master_key()
            .map_err(|_| anyhow::anyhow!("installation secret store unavailable"))?;
        read_installation_secret_with_key(&path_owned, &name_owned, &key)
    })
    .await
    .map_err(|_| anyhow::anyhow!("installation secret store unavailable"))?
}

fn read_installation_secret_with_key(
    store_path: &std::path::Path,
    name: &str,
    key: &age::x25519::Identity,
) -> anyhow::Result<Option<String>> {
    let store = load_store(store_path, key)
        .map_err(|_| anyhow::anyhow!("installation secret store unavailable"))?;
    Ok(store.secrets.get(name).cloned())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use plurora_core::ArtifactDescriptor;
    use plurora_work::{
        AcquisitionKind, AcquisitionRecord, InstallationRecord, InstallationStatus, WorkId,
    };
    use tokio::sync::{Mutex, Semaphore};

    use super::*;
    use crate::installation_control::{
        InstallationCreateRequest, InstallationListRequest, InstallationMutationResult,
        InstallationRemoveRequest, InstallationSecretStoreGuard, InstallationStateAction,
        InstallationUpdateRequest, InstallationView, InstallationWorkSummary, StateDisposition,
    };

    const SECRET_REF: &str = "secret_ref:installation:API_KEY";
    const SECRET_VALUE: &str = "sensitive-resolver-fixture-value";

    struct TestInstallationControl {
        installation_id: InstallationId,
        view: Mutex<Option<InstallationView>>,
        lifecycle: Arc<Mutex<()>>,
        path: PathBuf,
        acquire_calls: AtomicUsize,
        transition_started: Semaphore,
    }

    impl TestInstallationControl {
        fn new(status: Option<InstallationStatus>) -> Arc<Self> {
            let installation_id = InstallationId::new();
            Arc::new(Self {
                view: Mutex::new(
                    status.map(|status| installation_view(installation_id.clone(), status)),
                ),
                installation_id,
                lifecycle: Arc::new(Mutex::new(())),
                path: PathBuf::from("sensitive-store-path"),
                acquire_calls: AtomicUsize::new(0),
                transition_started: Semaphore::new(0),
            })
        }

        async fn set_status_and_revision(&self, status: InstallationStatus, revision: u64) {
            let mut view = self.view.lock().await;
            let current = view.as_mut().expect("fixture has an Installation");
            current.record.status = status;
            current.revision = revision;
        }
    }

    #[async_trait]
    impl InstallationControl for TestInstallationControl {
        async fn list(
            &self,
            _request: InstallationListRequest,
        ) -> anyhow::Result<Vec<InstallationView>> {
            Ok(self.view.lock().await.clone().into_iter().collect())
        }

        async fn get(
            &self,
            installation_id: &InstallationId,
        ) -> anyhow::Result<Option<InstallationView>> {
            if installation_id != &self.installation_id {
                return Ok(None);
            }
            Ok(self.view.lock().await.clone())
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
            self.transition_started.add_permits(1);
            let _lifecycle = self.lifecycle.clone().lock_owned().await;
            let mut view = self.view.lock().await;
            let current = view
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("fixture Installation unavailable"))?;
            current.revision += 1;
            current.record.status = InstallationStatus::Ready;
            Ok(InstallationMutationResult {
                installation: current.clone(),
                diff: None,
                receipts: Vec::new(),
                idempotent: false,
            })
        }

        async fn remove(
            &self,
            _request: InstallationRemoveRequest,
        ) -> anyhow::Result<InstallationMutationResult> {
            self.transition_started.add_permits(1);
            let _lifecycle = self.lifecycle.clone().lock_owned().await;
            let mut view = self.view.lock().await;
            let current = view
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("fixture Installation unavailable"))?;
            current.revision += 1;
            current.record.status = InstallationStatus::Removed;
            Ok(InstallationMutationResult {
                installation: current.clone(),
                diff: None,
                receipts: Vec::new(),
                idempotent: false,
            })
        }

        async fn acquire_ready_secret_store(
            &self,
            installation_id: &InstallationId,
            expected_revision: u64,
        ) -> anyhow::Result<InstallationSecretStoreGuard> {
            self.acquire_calls.fetch_add(1, Ordering::SeqCst);
            let lifecycle = self.lifecycle.clone().lock_owned().await;
            let view = self
                .view
                .lock()
                .await
                .clone()
                .ok_or_else(|| anyhow::anyhow!("fixture Installation unavailable"))?;
            InstallationSecretStoreGuard::verified(
                installation_id,
                expected_revision,
                &view,
                self.path.clone(),
                Box::new(lifecycle),
            )
        }
    }

    struct CountingReader {
        calls: AtomicUsize,
        value: Option<String>,
    }

    impl CountingReader {
        fn new(value: Option<&str>) -> Arc<Self> {
            Arc::new(Self {
                calls: AtomicUsize::new(0),
                value: value.map(ToOwned::to_owned),
            })
        }
    }

    #[async_trait]
    impl InstallationSecretReader for CountingReader {
        async fn read(
            &self,
            _store_path: &std::path::Path,
            _name: &str,
        ) -> anyhow::Result<Option<String>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.value.clone())
        }
    }

    struct BlockingReader {
        calls: AtomicUsize,
        started: Semaphore,
        release: Semaphore,
    }

    impl BlockingReader {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                calls: AtomicUsize::new(0),
                started: Semaphore::new(0),
                release: Semaphore::new(0),
            })
        }
    }

    #[async_trait]
    impl InstallationSecretReader for BlockingReader {
        async fn read(
            &self,
            _store_path: &std::path::Path,
            _name: &str,
        ) -> anyhow::Result<Option<String>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.started.add_permits(1);
            self.release
                .acquire()
                .await
                .expect("blocking reader release semaphore is open")
                .forget();
            Ok(Some(SECRET_VALUE.to_string()))
        }
    }

    fn artifact() -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: "urn:plurora:test:artifact:v1".to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    fn installation_view(
        installation_id: InstallationId,
        status: InstallationStatus,
    ) -> InstallationView {
        let now = chrono::Utc::now();
        InstallationView {
            work_summary: InstallationWorkSummary {
                work_id: WorkId::parse("tests/secret-resolver").expect("valid Work id"),
                title: "Resolver guard fixture".to_string(),
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
                work_revision: artifact(),
                assembly_lock: artifact(),
                display_name: "Resolver guard fixture".to_string(),
                source: AcquisitionRecord {
                    kind: AcquisitionKind::LocalImport,
                    source_ref: None,
                    provenance_refs: Vec::new(),
                    update_channel: None,
                },
                state_bindings: Vec::new(),
                secret_policy: InstallationSecretPolicy {
                    allowed_secret_refs: vec![SECRET_REF.to_string()],
                    allow_platform_fallback: false,
                },
                created_at: now,
                updated_at: now,
                status,
            },
            revision: 1,
            rollback: None,
        }
    }

    fn scope(control: Arc<TestInstallationControl>) -> InstallationScopeContext {
        InstallationScopeContext {
            installation_id: control.installation_id.clone(),
            revision: 1,
            secret_policy: InstallationSecretPolicy {
                allowed_secret_refs: vec![SECRET_REF.to_string()],
                allow_platform_fallback: false,
            },
            installation_control: control,
        }
    }

    fn resolver(
        scope: InstallationScopeContext,
        reader: Arc<dyn InstallationSecretReader>,
    ) -> InstallationStoreSecretResolver {
        InstallationStoreSecretResolver::new(move || Some(scope.clone())).with_reader(reader)
    }

    fn assert_redacted(error: anyhow::Error, control: &TestInstallationControl) {
        let message = error.to_string();
        assert!(!message.contains(control.installation_id.as_str()));
        assert!(!message.contains(control.path.to_string_lossy().as_ref()));
        assert!(!message.contains("API_KEY"));
        assert!(!message.contains(SECRET_VALUE));
    }

    #[tokio::test]
    async fn resolver_rejects_unlisted_installation_reference() {
        let control = TestInstallationControl::new(Some(InstallationStatus::Ready));
        let installation_id = control.installation_id.clone();
        let control_for_scope = control.clone();
        let resolver = InstallationStoreSecretResolver::new(move || {
            Some(InstallationScopeContext {
                installation_id: installation_id.clone(),
                revision: 1,
                secret_policy: InstallationSecretPolicy::default(),
                installation_control: control_for_scope.clone(),
            })
        });
        let error = resolver
            .resolve("secret_ref:installation:API_KEY")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("not allowed"));
        assert!(!error.contains("API_KEY"));
        assert_eq!(control.acquire_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn resolver_requires_an_exact_allowed_reference() {
        let control = TestInstallationControl::new(Some(InstallationStatus::Ready));
        let installation_id = control.installation_id.clone();
        let control_for_scope = control.clone();
        let resolver = InstallationStoreSecretResolver::new(move || {
            Some(InstallationScopeContext {
                installation_id: installation_id.clone(),
                revision: 1,
                secret_policy: InstallationSecretPolicy {
                    allowed_secret_refs: vec![SECRET_REF.to_string()],
                    allow_platform_fallback: false,
                },
                installation_control: control_for_scope.clone(),
            })
        });

        let error = resolver
            .resolve("secretRef:installation:API_KEY")
            .await
            .expect_err("an alternate spelling must not widen an exact allow entry")
            .to_string();
        assert!(error.contains("not allowed"));
        assert!(!error.contains("API_KEY"));
        assert_eq!(control.acquire_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn resolver_revalidates_every_status_and_unknown_installations_before_read() {
        for status in [
            Some(InstallationStatus::Resolving),
            Some(InstallationStatus::Updating),
            Some(InstallationStatus::Blocked),
            Some(InstallationStatus::Failed),
            Some(InstallationStatus::Removing),
            Some(InstallationStatus::Removed),
            None,
        ] {
            let control = TestInstallationControl::new(status);
            let reader = CountingReader::new(Some(SECRET_VALUE));
            let resolver = resolver(scope(control.clone()), reader.clone());
            let error = resolver
                .resolve(SECRET_REF)
                .await
                .expect_err("non-Ready and unknown Installations must fail closed");
            assert_redacted(error, &control);
            assert_eq!(reader.calls.load(Ordering::SeqCst), 0);
            assert_eq!(control.acquire_calls.load(Ordering::SeqCst), 1);
        }
    }

    #[tokio::test]
    async fn ready_revision_resolves_and_scope_debug_redacts_authority() {
        let control = TestInstallationControl::new(Some(InstallationStatus::Ready));
        let captured_scope = scope(control.clone());
        let debug = format!("{captured_scope:?}");
        assert!(!debug.contains(control.installation_id.as_str()));
        assert!(!debug.contains("API_KEY"));
        assert!(!debug.contains(control.path.to_string_lossy().as_ref()));

        let reader = CountingReader::new(Some(SECRET_VALUE));
        let resolver = resolver(captured_scope, reader.clone());
        assert_eq!(resolver.resolve(SECRET_REF).await.unwrap(), SECRET_VALUE);
        assert_eq!(reader.calls.load(Ordering::SeqCst), 1);
        assert_eq!(control.acquire_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn stale_scope_revision_or_status_rejects_without_store_read() {
        for status in [InstallationStatus::Updating, InstallationStatus::Removed] {
            let control = TestInstallationControl::new(Some(InstallationStatus::Ready));
            let captured_scope = scope(control.clone());
            control.set_status_and_revision(status, 2).await;
            let reader = CountingReader::new(Some(SECRET_VALUE));
            let resolver = resolver(captured_scope, reader.clone());

            let error = resolver
                .resolve(SECRET_REF)
                .await
                .expect_err("a lifecycle transition after scope creation must fail closed");
            assert_redacted(error, &control);
            assert_eq!(reader.calls.load(Ordering::SeqCst), 0);
            assert_eq!(control.acquire_calls.load(Ordering::SeqCst), 1);
        }
    }

    #[derive(Clone, Copy)]
    enum Transition {
        Update,
        Remove,
    }

    #[tokio::test]
    async fn resolve_guard_blocks_update_and_remove_only_until_the_read_finishes() {
        for transition_kind in [Transition::Update, Transition::Remove] {
            let control = TestInstallationControl::new(Some(InstallationStatus::Ready));
            let reader = BlockingReader::new();
            let resolver = resolver(scope(control.clone()), reader.clone());
            let resolution = tokio::spawn(async move { resolver.resolve(SECRET_REF).await });

            reader
                .started
                .acquire()
                .await
                .expect("reader started semaphore is open")
                .forget();

            let current = control
                .get(&control.installation_id)
                .await
                .unwrap()
                .expect("fixture Installation exists");
            let control_for_transition = control.clone();
            let transition = match transition_kind {
                Transition::Update => tokio::spawn(async move {
                    control_for_transition
                        .update(InstallationUpdateRequest {
                            installation_id: current.record.installation_id.clone(),
                            expected_revision: current.revision,
                            work_revision: current.record.work_revision.clone(),
                            assembly_lock: current.record.assembly_lock.clone(),
                            display_name: None,
                            source: None,
                            state_bindings: None,
                            secret_policy: None,
                            state_action: InstallationStateAction::Preserve,
                            idempotency_key: "resolver-guard-update".to_string(),
                            authority: None,
                        })
                        .await
                }),
                Transition::Remove => tokio::spawn(async move {
                    control_for_transition
                        .remove(InstallationRemoveRequest {
                            installation_id: current.record.installation_id.clone(),
                            expected_revision: current.revision,
                            state_disposition: StateDisposition::Keep,
                            idempotency_key: "resolver-guard-remove".to_string(),
                            authority: None,
                        })
                        .await
                }),
            };

            control
                .transition_started
                .acquire()
                .await
                .expect("transition started semaphore is open")
                .forget();
            tokio::task::yield_now().await;
            assert!(
                !transition.is_finished(),
                "lifecycle transition must wait while resolve reads under its guard"
            );

            reader.release.add_permits(1);
            assert_eq!(resolution.await.unwrap().unwrap(), SECRET_VALUE);
            let transitioned = transition.await.unwrap().unwrap().installation;
            assert_eq!(transitioned.revision, 2);
            match transition_kind {
                Transition::Update => {
                    assert_eq!(transitioned.record.status, InstallationStatus::Ready)
                }
                Transition::Remove => {
                    assert_eq!(transitioned.record.status, InstallationStatus::Removed)
                }
            }
        }
    }

    #[test]
    fn reading_an_existing_installation_store_does_not_rewrite_it() -> anyhow::Result<()> {
        let data = tempfile::tempdir()?;
        let path = data.path().join("secrets.dat");
        let identity = age::x25519::Identity::generate();
        let recipient = identity.to_public();
        let mut store = crate::secret_store::StoreFile::empty();
        store
            .secrets
            .insert("API_KEY".to_string(), SECRET_VALUE.to_string());
        crate::secret_store::save_store(&path, &store, &recipient)?;
        let before = std::fs::read(&path)?;

        assert_eq!(
            read_installation_secret_with_key(&path, "API_KEY", &identity)?,
            Some(SECRET_VALUE.to_string())
        );
        assert_eq!(std::fs::read(path)?, before);
        Ok(())
    }
}
