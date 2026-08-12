use std::fs;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use plurora_runtime::{
    DenyAllWebSocketExecutor, EventStore, FakeOutboundExecutor, FakeWebSocketExecutor,
    FilesystemObjectStore, InMemoryEventStore, InMemoryObjectStore, LiveHttpOutboundExecutor,
    LiveLocalExecExecutor, LiveLocalExecExecutorConfig, LiveWebSocketExecutor,
    LiveWebSocketProfile, LocalExecExecutorConfig, OutboundExecutePolicyConfig,
    OutboundExecutorConfig, ProtocolContext, Runtime, RuntimeConfig, SqliteEventStore,
    WebSocketExecutor,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::manifest::read_manifest;
use crate::cli::{
    HostEventStoreProfile, HostExecuteOutboundExecutorKind, HostExecuteOutboundProfile,
    HostLocalExecExecutorKind, HostLocalExecProfile, HostProfile, HostSecretResolverProfile,
    HostWebSocketOutboundExecutorKind, HostWebSocketOutboundProfile,
};

const RUNTIME_PUBLIC_JOURNAL_FILE: &str = "events.sqlite3";

impl LiveWebSocketProfile for HostWebSocketOutboundProfile {
    fn allowed_hosts(&self) -> &[String] {
        &self.allowed_hosts
    }

    fn wss_only(&self) -> bool {
        self.wss_only
    }

    fn max_idle_ms(&self) -> u64 {
        self.max_idle_ms
    }

    fn max_duration_ms(&self) -> u64 {
        self.max_duration_ms
    }

    fn max_frame_bytes(&self) -> usize {
        self.max_frame_bytes
    }

    fn max_total_bytes_inbound(&self) -> usize {
        self.max_total_bytes_inbound
    }

    fn max_total_bytes_outbound(&self) -> usize {
        self.max_total_bytes_outbound
    }

    fn max_concurrent_connections(&self) -> usize {
        self.max_concurrent_connections
    }

    fn allow_insecure_ws_for_tests(&self) -> bool {
        self.allow_insecure_ws_for_tests
    }
}

pub(crate) async fn host_serve(
    http: SocketAddr,
    profile: Option<PathBuf>,
    static_dir: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    access_token: Option<String>,
    app_base_domain: Option<String>,
) -> Result<()> {
    if let Some(data_dir) = data_dir.as_ref() {
        println!("host data dir: {}", data_dir.display());
        std::env::set_var("PLURORA_DATA_DIR", data_dir);
        plurora_core::paths::ensure_initialized().with_context(|| {
            format!("failed to initialize data directory {}", data_dir.display())
        })?;
    } else {
        plurora_core::paths::ensure_initialized()
            .context("failed to initialize the default Host data directory")?;
    }
    let default_data_dir;
    let schema_data_dir = if let Some(data_dir) = data_dir.as_ref() {
        data_dir.as_path()
    } else {
        default_data_dir = plurora_core::paths::data_dir()?;
        default_data_dir.as_path()
    };
    let object_root = prepare_host_owned_directory(schema_data_dir, "objects")?;
    super::work::prepare_object_store_for_installation(&object_root)
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let runtime_root = prepare_host_owned_directory(schema_data_dir, "runtime")?;
    let installation_journal = super::installation::prepare_installation_journal(&runtime_root)?;
    let installation_store = Arc::new(
        SqliteEventStore::open(&installation_journal)
            .context("failed to open the durable Installation authority journal")?,
    );
    let development = plurora_service::development_registry();
    let owner_lease = plurora_service::acquire_development_host_lease(
        installation_store.clone(),
        development.clone(),
    )
    .await
    .context("failed to acquire the durable Host owner lease")?;
    let owner_heartbeat = plurora_service::spawn_development_host_lease_heartbeat(
        installation_store.clone(),
        owner_lease.clone(),
    );

    let serve_result: Result<()> = async {
        if let Some(profile_path) = profile {
        println!("host profile: {}", profile_path.display());
        let raw = fs::read_to_string(&profile_path)
            .with_context(|| format!("failed to read host profile {}", profile_path.display()))?;
        let profile: HostProfile = serde_yaml::from_str(&raw)
            .with_context(|| format!("failed to parse host profile {}", profile_path.display()))?;
        let mut runtime_config = runtime_config_from_profile(&profile)?;
        runtime_config.object_store = Arc::new(FilesystemObjectStore::new(object_root.clone()));
        register_profile_package_roots(&mut runtime_config, &profile, Some(&profile_path)).await?;
        match &profile.event_store {
            HostEventStoreProfile::Memory => {
                let (runtime, installations, powerbox, realizations, runs) = runtime_with_installations(
                    Arc::new(InMemoryEventStore::default()),
                    runtime_config,
                    schema_data_dir,
                    installation_store.clone(),
                    &owner_lease,
                )
                .await?;
                load_profile_packages(runtime.clone(), profile, profile_path.clone()).await?;
                serve_runtime(
                    http,
                    runtime,
                    installations,
                    powerbox,
                    realizations,
                    runs,
                    "memory",
                    static_dir,
                    access_token,
                    app_base_domain,
                    development.clone(),
                    owner_lease.clone(),
                )
                .await
            }
            HostEventStoreProfile::Sqlite { path } => {
                let resolved = resolve_profile_path(&profile_path, path.clone());
                if let Some(parent) = resolved.parent() {
                    fs::create_dir_all(parent).with_context(|| {
                        format!(
                            "failed to create event store directory {}",
                            parent.display()
                        )
                    })?;
                }
                ensure_distinct_journal_paths(&resolved, &installation_journal)?;
                let store = Arc::new(SqliteEventStore::open(&resolved).with_context(|| {
                    format!("failed to open sqlite event store {}", resolved.display())
                })?);
                let (runtime, installations, powerbox, realizations, runs) = runtime_with_installations(
                    store,
                    runtime_config,
                    schema_data_dir,
                    installation_store.clone(),
                    &owner_lease,
                )
                .await?;
                runtime
                    .hydrate_substrate_from_events()
                    .await
                    .context("failed to rehydrate substrate from sqlite event log")?;
                load_profile_packages(runtime.clone(), profile, profile_path.clone()).await?;
                serve_runtime(
                    http,
                    runtime,
                    installations,
                    powerbox,
                    realizations,
                    runs,
                    "sqlite",
                    static_dir,
                    access_token,
                    app_base_domain,
                    development.clone(),
                    owner_lease.clone(),
                )
                .await
            }
            HostEventStoreProfile::Postgres { env } => {
                #[cfg(feature = "postgres")]
                {
                    let url = std::env::var(env).map_err(|_| {
                        anyhow::anyhow!(
                            "postgres event store env ref unavailable (details redacted)"
                        )
                    })?;
                    let store = plurora_runtime::PostgresEventStore::connect(&url).await?;
                    let (runtime, installations, powerbox, realizations, runs) = runtime_with_installations(
                        Arc::new(store),
                        runtime_config,
                        schema_data_dir,
                        installation_store.clone(),
                        &owner_lease,
                    )
                    .await?;
                    runtime
                        .hydrate_substrate_from_events()
                        .await
                        .context("failed to rehydrate substrate from postgres event log")?;
                    load_profile_packages(runtime.clone(), profile, profile_path).await?;
                    serve_runtime(
                        http,
                        runtime,
                        installations,
                        powerbox,
                        realizations,
                        runs,
                        "postgres",
                        static_dir,
                        access_token,
                        app_base_domain,
                        development.clone(),
                        owner_lease.clone(),
                    )
                    .await
                }
                #[cfg(not(feature = "postgres"))]
                {
                    let _ = env;
                    anyhow::bail!("postgres event store requested but this binary was built without postgres support")
                }
            }
        }
    } else {
        let mut runtime_config = RuntimeConfig::default();
        runtime_config.object_store = Arc::new(FilesystemObjectStore::new(object_root));
        runtime_config.workload_reconcile_source =
            Arc::new(plurora_runtime::DockerWorkloadReconcileSource);
        let runtime_journal = prepare_host_event_journal(
            &runtime_root,
            RUNTIME_PUBLIC_JOURNAL_FILE,
        )?;
        ensure_distinct_journal_paths(&runtime_journal, &installation_journal)?;
        let store = Arc::new(
            SqliteEventStore::open(&runtime_journal)
                .context("failed to open the durable Runtime public journal")?,
        );
        let (runtime, installations, powerbox, realizations, runs) = runtime_with_installations(
            store,
            runtime_config,
            schema_data_dir,
            installation_store.clone(),
            &owner_lease,
        )
        .await?;
        runtime
            .hydrate_substrate_from_events()
            .await
            .context("failed to rehydrate substrate from the default Host journal")?;
        serve_runtime(
            http,
            runtime,
            installations,
            powerbox,
            realizations,
            runs,
            "sqlite",
            static_dir,
            access_token,
            app_base_domain,
            development.clone(),
            owner_lease.clone(),
        )
        .await
        }
    }
    .await;
    owner_heartbeat.abort();
    if let Err(error) =
        plurora_service::release_development_host_lease(installation_store, &owner_lease).await
    {
        eprintln!("warning: failed to release durable Host owner lease: {error}");
    }
    serve_result
}

fn prepare_host_owned_directory(data_dir: &Path, name: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(data_dir).context("failed to inspect Host data root")?;
    anyhow::ensure!(
        metadata.file_type().is_dir()
            && !metadata.file_type().is_symlink()
            && !is_reparse_point(&metadata),
        "Host data root must be a real directory"
    );
    let root = fs::canonicalize(data_dir).context("failed to resolve Host data root")?;
    let path = root.join(name);
    if !path.exists() {
        fs::create_dir(&path).context("failed to create Host-owned directory")?;
    }
    let metadata = fs::symlink_metadata(&path).context("failed to inspect Host-owned directory")?;
    anyhow::ensure!(
        metadata.file_type().is_dir()
            && !metadata.file_type().is_symlink()
            && !is_reparse_point(&metadata),
        "Host-owned directory must be a real directory"
    );
    let canonical = fs::canonicalize(&path).context("failed to resolve Host-owned directory")?;
    anyhow::ensure!(
        canonical.starts_with(&root),
        "Host-owned directory escapes the data root"
    );
    Ok(canonical)
}

fn prepare_host_event_journal(runtime_root: &Path, name: &str) -> Result<PathBuf> {
    let path = runtime_root.join(name);
    for candidate in std::iter::once(path.clone()).chain(
        ["-journal", "-wal", "-shm"]
            .into_iter()
            .map(|suffix| runtime_root.join(format!("{name}{suffix}"))),
    ) {
        if !candidate.exists() {
            continue;
        }
        let metadata = fs::symlink_metadata(&candidate)
            .with_context(|| format!("failed to inspect Host journal {}", candidate.display()))?;
        anyhow::ensure!(
            metadata.file_type().is_file()
                && !metadata.file_type().is_symlink()
                && !is_reparse_point(&metadata),
            "Host journal must be a regular non-link file"
        );
    }
    Ok(path)
}

fn ensure_distinct_journal_paths(left: &Path, right: &Path) -> Result<()> {
    fn resolved_identity(path: &Path) -> Result<PathBuf> {
        if path.exists() {
            return fs::canonicalize(path)
                .with_context(|| format!("failed to resolve Host journal {}", path.display()));
        }
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Host journal has no parent directory"))?;
        let parent = fs::canonicalize(parent).with_context(|| {
            format!("failed to resolve Host journal parent {}", parent.display())
        })?;
        let file = path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Host journal has no file name"))?;
        Ok(parent.join(file))
    }

    anyhow::ensure!(
        resolved_identity(left)? != resolved_identity(right)?,
        "Runtime public journal must be physically distinct from the Host authority journal"
    );
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

async fn runtime_with_installations<S>(
    store: Arc<S>,
    mut config: RuntimeConfig,
    data_dir: &Path,
    installation_store: Arc<dyn EventStore>,
    owner_lease: &plurora_service::DevelopmentHostLease,
) -> Result<(
    Arc<Runtime<S>>,
    Arc<plurora_service::InstallationRegistry>,
    Arc<plurora_service::PowerboxRegistry>,
    Arc<plurora_service::RealizationRegistry>,
    Arc<plurora_service::RunRegistry>,
)>
where
    S: EventStore,
{
    let runtime_public_store: Arc<dyn EventStore> = store.clone();
    let installations = plurora_service::InstallationRegistry::persistent(
        installation_store.clone(),
        config.object_store.clone(),
        data_dir,
    )?;
    installations.install_owner_lease(owner_lease.clone())?;
    let runs = plurora_service::RunRegistry::new(installation_store.clone());
    runs.install_owner_lease(owner_lease.clone())?;
    let powerbox = plurora_service::PowerboxRegistry::new(
        installation_store.clone(),
        runtime_public_store.clone(),
        installations.clone(),
        runs.clone(),
    )?;
    powerbox.install_owner_lease(owner_lease.clone())?;
    let realizations = plurora_service::RealizationRegistry::new(
        installation_store,
        runtime_public_store,
        config.object_store.clone(),
        installations.clone(),
        config.target_registry.clone(),
    )?;
    realizations.install_owner_lease(owner_lease.clone())?;
    config.installation_control = installations.clone();
    config.run_control = runs.clone();
    config.powerbox_control = powerbox.clone();
    config.realization_control = realizations.clone();
    let runtime = Arc::new(Runtime::new(store, config));
    runs.install_driver(Arc::new(plurora_runtime::AssemblyRuntimeDriver::new(
        Arc::downgrade(&runtime),
    )))?;
    powerbox.install_inspector(Arc::new(plurora_service::RuntimePowerboxInspector::new(
        Arc::downgrade(&runtime),
    )))?;
    Ok((runtime, installations, powerbox, realizations, runs))
}

pub fn runtime_config_from_profile(profile: &HostProfile) -> Result<RuntimeConfig> {
    validate_execute_outbound_profile(&profile.outbound.execute)?;
    validate_websocket_outbound_profile(&profile.outbound.websocket)?;
    validate_local_exec_profile(&profile.local_exec)?;

    let mut config = RuntimeConfig::default();
    config.workload_reconcile_source = Arc::new(plurora_runtime::DockerWorkloadReconcileSource);
    // Y1: Wire outbound.execute profile into RuntimeConfig
    let exec = &profile.outbound.execute;
    config.outbound_execute_policy = OutboundExecutePolicyConfig {
        enabled: exec.enabled,
        allowed_hosts: exec.allowed_hosts.clone(),
        https_only: exec.https_only,
        timeout_ms: exec.timeout_ms,
        allow_redirects: exec.allow_redirects,
        allow_insecure_loopback_for_tests: exec.allow_insecure_loopback_for_tests,
    };
    config.outbound_executor = build_outbound_execute_executor(exec)?;

    config.outbound_websocket_executor =
        build_outbound_websocket_executor(&profile.outbound.websocket)?;

    config.local_exec_executor = build_local_exec_executor(&profile.local_exec)?;

    config.secret_resolver = build_secret_resolver(&profile.secret_resolver)?;
    config.surface_dev_paths = profile.surface_dev_paths.clone();

    Ok(config)
}

async fn register_profile_package_roots(
    config: &mut RuntimeConfig,
    profile: &HostProfile,
    profile_path: Option<&Path>,
) -> Result<()> {
    let base = profile_path
        .and_then(Path::parent)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    for manifest_path in &profile.autoload {
        let resolved = if manifest_path.is_absolute() {
            manifest_path.clone()
        } else {
            base.join(manifest_path)
        };
        if should_skip_dangling_store_autoload(&resolved) {
            continue;
        }
        let manifest = read_manifest(resolved.clone()).await.with_context(|| {
            format!(
                "failed to register package root from manifest {}",
                resolved.display()
            )
        })?;
        if let Some(parent) = resolved.parent() {
            let root = std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
            config.package_roots.insert(manifest.id.clone(), root);
        }
    }
    Ok(())
}

fn should_skip_dangling_store_autoload(resolved_manifest: &Path) -> bool {
    if resolved_manifest.exists() {
        return false;
    }
    let Ok(store_dir) = plurora_core::paths::store_dir() else {
        return false;
    };
    should_skip_dangling_store_autoload_in_store(resolved_manifest, &store_dir)
}

fn should_skip_dangling_store_autoload_in_store(
    resolved_manifest: &Path,
    store_dir: &Path,
) -> bool {
    if resolved_manifest.exists() {
        return false;
    }
    if is_under_store_dir(resolved_manifest, store_dir) {
        eprintln!(
            "host/autoload.skipped: missing migrated store manifest {}",
            resolved_manifest.display()
        );
        return true;
    }
    false
}

fn is_under_store_dir(path: &Path, data_dir: &Path) -> bool {
    normalize_path(path).starts_with(normalize_path(data_dir))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                out.push(component.as_os_str());
            }
        }
    }
    out
}

pub(crate) fn build_secret_resolver(
    profile: &HostSecretResolverProfile,
) -> Result<plurora_runtime::SecretResolverConfig> {
    use plurora_runtime::{
        CompositeSecretResolver, EnvSecretResolver, SecretResolverConfig, StoreSecretResolver,
    };
    use std::collections::HashSet;

    let mut composite = CompositeSecretResolver::new();

    if !profile.env_allowlist.is_empty() {
        let allowlist: HashSet<String> = profile.env_allowlist.iter().cloned().collect();
        composite = composite.with_env(Arc::new(EnvSecretResolver::new(allowlist)));
    }

    let platform_store = if profile.store_enabled {
        // StoreSecretResolver::new() resolves the default store path via
        // plurora_core::paths::secret_store_path(). It can fail if data_dir
        // resolution fails on this system — propagate that error.
        let r = Arc::new(StoreSecretResolver::new()?);
        composite = composite.with_store(r.clone());
        Some(r)
    } else {
        None
    };

    // Installation secrets remain scoped to the active Installation. Platform
    // fallback is whichever StoreSecretResolver we just built (or none).
    let installation_resolver = plurora_runtime::InstallationStoreSecretResolver::new(|| {
        plurora_runtime::ACTIVE_INSTALLATION_SCOPE
            .try_with(|scope| scope.clone())
            .ok()
    });
    let installation_resolver = if let Some(platform) = platform_store {
        installation_resolver.with_platform_fallback(platform)
    } else {
        installation_resolver
    };
    composite = composite.with_installation(Arc::new(installation_resolver));

    Ok(SecretResolverConfig::with_resolver(Arc::new(composite)))
}

/// Build the `OutboundExecutorConfig` from the execute profile section (Y1).
///
/// - If `enabled` is false, always returns `DenyAll` (fail-closed).
/// - If `enabled` is true, selects based on `executor` field:
///   - `deny_all` → DenyAll
///   - `fake` → Custom(FakeOutboundExecutor)
///   - `live` → LiveHttp(config built from profile fields)
pub(crate) fn build_outbound_execute_executor(
    config: &HostExecuteOutboundProfile,
) -> Result<OutboundExecutorConfig> {
    if !config.enabled {
        return Ok(OutboundExecutorConfig::DenyAll);
    }
    match config.executor {
        HostExecuteOutboundExecutorKind::DenyAll => Ok(OutboundExecutorConfig::DenyAll),
        HostExecuteOutboundExecutorKind::Fake => Ok(OutboundExecutorConfig::Custom(Arc::new(
            FakeOutboundExecutor::new(),
        ))),
        HostExecuteOutboundExecutorKind::Live => {
            let executor = LiveHttpOutboundExecutor::new_from_profile(
                config.https_only,
                config.timeout_ms,
                config.allow_redirects,
                config.allow_insecure_loopback_for_tests,
            )?;
            Ok(OutboundExecutorConfig::Custom(Arc::new(executor)))
        }
    }
}

pub(crate) fn build_outbound_websocket_executor(
    profile: &HostWebSocketOutboundProfile,
) -> Result<Arc<dyn WebSocketExecutor>> {
    if !profile.enabled {
        return Ok(Arc::new(DenyAllWebSocketExecutor));
    }
    match profile.executor {
        HostWebSocketOutboundExecutorKind::DenyAll => Ok(Arc::new(DenyAllWebSocketExecutor)),
        HostWebSocketOutboundExecutorKind::Fake => Ok(Arc::new(FakeWebSocketExecutor::new())),
        HostWebSocketOutboundExecutorKind::Live => {
            let executor = LiveWebSocketExecutor::new_from_profile(profile)?;
            Ok(Arc::new(executor))
        }
    }
}

pub(crate) fn build_local_exec_executor(
    profile: &HostLocalExecProfile,
) -> Result<LocalExecExecutorConfig> {
    if !profile.enabled {
        return Ok(LocalExecExecutorConfig::DenyAll);
    }
    match profile.executor {
        HostLocalExecExecutorKind::DenyAll => Ok(LocalExecExecutorConfig::DenyAll),
        HostLocalExecExecutorKind::Fake => Ok(LocalExecExecutorConfig::Fake),
        HostLocalExecExecutorKind::Live => {
            let config = LiveLocalExecExecutorConfig::new(
                profile.allowed_programs.clone(),
                profile.allowed_working_dirs.clone(),
                profile.allowed_env_vars.clone(),
                profile.max_duration_ms,
                profile.max_log_bytes,
            )?;
            Ok(LocalExecExecutorConfig::Custom(Arc::new(
                LiveLocalExecExecutor::new(config)?,
            )))
        }
    }
}

/// Validate the execute outbound profile section (Y1).
///
/// Enforces fail-closed constraints:
/// - `timeout_ms` must be > 0 when enabled
/// - `allowed_hosts` must not be empty when enabled with a non-deny_all executor
/// - `allowed_hosts` must not contain empty or wildcard hosts
/// - `https_only=false` is not supported (HTTPS-only is the only safe default)
/// - `allow_redirects=true` is not supported (redirects fail closed)
pub(crate) fn validate_execute_outbound_profile(exec: &HostExecuteOutboundProfile) -> Result<()> {
    if !exec.https_only {
        anyhow::bail!(
            "outbound.execute.https_only=false is not supported; live outbound is HTTPS-only"
        )
    }
    if exec.allow_redirects {
        anyhow::bail!(
            "outbound.execute.allow_redirects=true is not supported; redirects fail closed"
        )
    }
    if exec.timeout_ms == 0 {
        anyhow::bail!("outbound.execute.timeout_ms must be greater than zero")
    }
    if !exec.enabled {
        return Ok(());
    }
    if !matches!(exec.executor, HostExecuteOutboundExecutorKind::DenyAll)
        && exec.allowed_hosts.is_empty()
    {
        anyhow::bail!(
            "outbound.execute.allowed_hosts is required when execute outbound is enabled with a non-deny_all executor"
        )
    }
    if exec
        .allowed_hosts
        .iter()
        .any(|host| host.trim().is_empty() || host == "*")
    {
        anyhow::bail!(
            "outbound.execute.allowed_hosts must not contain empty hosts or wildcard hosts"
        )
    }
    Ok(())
}

pub(crate) fn validate_websocket_outbound_profile(
    profile: &HostWebSocketOutboundProfile,
) -> Result<()> {
    if !profile.wss_only && !profile.allow_insecure_ws_for_tests {
        anyhow::bail!(
            "outbound.websocket.wss_only=false is only supported with allow_insecure_ws_for_tests=true"
        )
    }
    if profile.max_idle_ms == 0 {
        anyhow::bail!("outbound.websocket.max_idle_ms must be greater than zero")
    }
    if profile.max_duration_ms == 0 {
        anyhow::bail!("outbound.websocket.max_duration_ms must be greater than zero")
    }
    if profile.max_frame_bytes == 0 {
        anyhow::bail!("outbound.websocket.max_frame_bytes must be greater than zero")
    }
    if profile.max_total_bytes_inbound == 0 {
        anyhow::bail!("outbound.websocket.max_total_bytes_inbound must be greater than zero")
    }
    if profile.max_total_bytes_outbound == 0 {
        anyhow::bail!("outbound.websocket.max_total_bytes_outbound must be greater than zero")
    }
    if profile.max_concurrent_connections == 0 {
        anyhow::bail!("outbound.websocket.max_concurrent_connections must be greater than zero")
    }
    if !profile.enabled {
        return Ok(());
    }
    if matches!(profile.executor, HostWebSocketOutboundExecutorKind::Live)
        && profile.allowed_hosts.is_empty()
    {
        anyhow::bail!(
            "outbound.websocket.allowed_hosts is required when websocket outbound is enabled with live executor"
        )
    }
    if profile
        .allowed_hosts
        .iter()
        .any(|host| host.trim().is_empty() || host == "*")
    {
        anyhow::bail!(
            "outbound.websocket.allowed_hosts must not contain empty hosts or wildcard hosts"
        )
    }
    Ok(())
}

pub(crate) fn validate_local_exec_profile(profile: &HostLocalExecProfile) -> Result<()> {
    if profile.max_duration_ms == 0 {
        anyhow::bail!("local_exec.max_duration_ms must be greater than zero");
    }
    if profile.max_log_bytes == 0 {
        anyhow::bail!("local_exec.max_log_bytes must be greater than zero");
    }
    validate_local_exec_string_allowlist(
        "local_exec.allowed_programs",
        &profile.allowed_programs,
        true,
    )?;
    validate_local_exec_string_allowlist(
        "local_exec.allowed_env_vars",
        &profile.allowed_env_vars,
        false,
    )?;
    for dir in &profile.allowed_working_dirs {
        let raw = dir.to_string_lossy();
        if raw.trim().is_empty() || raw.contains('*') {
            anyhow::bail!(
                "local_exec.allowed_working_dirs must not contain empty or wildcard entries"
            );
        }
        if dir
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            anyhow::bail!(
                "local_exec.allowed_working_dirs must not contain parent-directory components"
            );
        }
    }
    if !profile.enabled {
        return Ok(());
    }
    if matches!(profile.executor, HostLocalExecExecutorKind::Live) {
        if profile.allowed_programs.is_empty() {
            anyhow::bail!(
                "local_exec.allowed_programs is required when local_exec is enabled with live executor"
            );
        }
        if profile.allowed_working_dirs.is_empty() {
            anyhow::bail!(
                "local_exec.allowed_working_dirs is required when local_exec is enabled with live executor"
            );
        }
    }
    Ok(())
}

fn validate_local_exec_string_allowlist(
    name: &str,
    values: &[String],
    reject_path_parent: bool,
) -> Result<()> {
    for value in values {
        if value.trim().is_empty() || value.contains('*') {
            anyhow::bail!("{name} must not contain empty or wildcard entries");
        }
        if reject_path_parent
            && Path::new(value)
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            anyhow::bail!("{name} must not contain parent-directory components");
        }
        if !reject_path_parent && value.contains('=') {
            anyhow::bail!("{name} must not contain '='");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    use crate::cli::{
        Cli, Command, HostWebSocketOutboundExecutorKind, HostWebSocketOutboundProfile,
    };

    #[test]
    fn validate_websocket_outbound_profile_fails_closed_on_live_with_empty_allowed_hosts() {
        let profile = HostWebSocketOutboundProfile {
            enabled: true,
            executor: HostWebSocketOutboundExecutorKind::Live,
            allowed_hosts: Vec::new(),
            ..HostWebSocketOutboundProfile::default()
        };
        let err = validate_websocket_outbound_profile(&profile)
            .expect_err("live websocket without hosts should fail closed");
        assert!(err.to_string().contains("allowed_hosts"));
    }

    #[test]
    fn validate_websocket_outbound_profile_rejects_non_wss_when_no_test_flag() {
        let profile = HostWebSocketOutboundProfile {
            enabled: true,
            executor: HostWebSocketOutboundExecutorKind::Fake,
            wss_only: false,
            allow_insecure_ws_for_tests: false,
            ..HostWebSocketOutboundProfile::default()
        };
        let err = validate_websocket_outbound_profile(&profile)
            .expect_err("non-wss websocket without test flag should fail");
        assert!(err.to_string().contains("wss_only=false"));
    }

    #[test]
    fn parses_host_serve_paas_args() {
        let cli = Cli::try_parse_from([
            "plurora",
            "host",
            "serve",
            "--http",
            "0.0.0.0:8080",
            "--profile",
            "/data/profiles/zeabur.yaml",
            "--static-dir",
            "/app/public",
            "--data-dir",
            "/data",
            "--access-token",
            "token",
            "--app-base-domain",
            "apps.example.com",
        ])
        .unwrap();

        match cli.command {
            Command::Host {
                command:
                    crate::cli::HostCommand::Serve {
                        http,
                        profile,
                        static_dir,
                        data_dir,
                        access_token,
                        app_base_domain,
                    },
            } => {
                assert_eq!(http, "0.0.0.0:8080".parse().unwrap());
                assert_eq!(profile, Some(PathBuf::from("/data/profiles/zeabur.yaml")));
                assert_eq!(static_dir, Some(PathBuf::from("/app/public")));
                assert_eq!(data_dir, Some(PathBuf::from("/data")));
                assert_eq!(access_token, Some("token".to_string()));
                assert_eq!(app_base_domain, Some("apps.example.com".to_string()));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn resolve_profile_relative_event_store_under_profile_dir() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let profile_path = tmp.path().join("profiles/default.yaml");
        let event_path = resolve_profile_path(&profile_path, PathBuf::from("events.sqlite"));
        assert_eq!(event_path, tmp.path().join("profiles/events.sqlite"));
        Ok(())
    }

    #[test]
    fn autoload_skip_helper_only_skips_missing_store_manifests() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let data = tmp.path();
        let store = data.join("store");
        fs::create_dir_all(&store)?;

        let missing_store_manifest = data.join("store/sha256-old/manifest.yaml");
        assert!(should_skip_dangling_store_autoload_in_store(
            &missing_store_manifest,
            &store
        ));

        let missing_non_store_manifest = data.join("packages/missing/manifest.yaml");
        assert!(!should_skip_dangling_store_autoload_in_store(
            &missing_non_store_manifest,
            &store
        ));

        let existing_store_manifest = data.join("store/sha256-current/manifest.yaml");
        fs::create_dir_all(existing_store_manifest.parent().unwrap())?;
        fs::write(&existing_store_manifest, "id: fixture/current\n")?;
        assert!(!should_skip_dangling_store_autoload_in_store(
            &existing_store_manifest,
            &store
        ));
        Ok(())
    }

    #[tokio::test]
    async fn host_stdio_accepts_only_current_method_ids() {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        let context = ProtocolContext::host_dev("host_stdio_test");
        let current = host_stdio_response(
            &runtime,
            &context,
            r#"{"id":"current","method":"host.info","params":{}}"#,
        )
        .await;
        assert!(current.result.is_some());

        let removed = host_stdio_response(
            &runtime,
            &context,
            r#"{"id":"removed","method":"platform.host.info","params":{}}"#,
        )
        .await;
        assert_eq!(
            removed.error.expect("removed method must fail").code,
            "runtime/error/invalid_request"
        );

        let malformed_contract = host_stdio_response(
            &runtime,
            &context,
            r#"{"id":"contract-error","method":"host.info","contract":"bad","params":{}}"#,
        )
        .await;
        assert_eq!(malformed_contract.id, "contract-error");
        assert_eq!(
            malformed_contract
                .error
                .expect("invalid contract must fail")
                .code,
            "runtime/error/invalid_request"
        );
    }

    #[test]
    fn managed_host_listen_handshake_is_stable() {
        let addr: SocketAddr = "127.0.0.1:43117".parse().unwrap();
        assert_eq!(
            managed_host_listen_line(addr),
            "PLURORA_HOST_LISTEN_ADDR=127.0.0.1:43117"
        );
    }

    #[tokio::test]
    async fn installation_hydration_requires_the_live_exclusive_host_lease() -> Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(InMemoryObjectStore::default());
        let installations =
            plurora_service::InstallationRegistry::ephemeral(store.clone(), objects)?;
        let owner_registry = plurora_service::development_registry();
        let owner =
            plurora_service::acquire_development_host_lease(store.clone(), owner_registry).await?;
        assert!(plurora_service::acquire_development_host_lease(
            store.clone(),
            plurora_service::development_registry(),
        )
        .await
        .is_err());
        assert_eq!(
            hydrate_installations_as_host_owner(installations.clone(), &owner).await?,
            0
        );

        plurora_service::release_development_host_lease(store.clone(), &owner).await?;
        let event_count = store.list_all().await?.len();
        assert!(hydrate_installations_as_host_owner(installations, &owner)
            .await
            .is_err());
        assert_eq!(store.list_all().await?.len(), event_count);
        Ok(())
    }

    #[tokio::test]
    async fn profile_memory_stores_do_not_fork_installation_authority() -> Result<()> {
        let data = tempfile::tempdir()?;
        let installation_store = Arc::new(InMemoryEventStore::default());
        let owner = plurora_service::acquire_development_host_lease(
            installation_store.clone(),
            plurora_service::development_registry(),
        )
        .await?;
        let profile_a = Arc::new(InMemoryEventStore::default());
        let (_, installations_a, _, _, _) = runtime_with_installations(
            profile_a.clone(),
            RuntimeConfig::default(),
            data.path(),
            installation_store.clone(),
            &owner,
        )
        .await?;
        assert_eq!(installations_a.hydrate().await?, 0);
        assert!(profile_a.list_all().await?.is_empty());

        let profile_b = Arc::new(InMemoryEventStore::default());
        let (_, installations_b, _, _, _) = runtime_with_installations(
            profile_b.clone(),
            RuntimeConfig::default(),
            data.path(),
            installation_store.clone(),
            &owner,
        )
        .await?;
        assert_eq!(installations_b.hydrate().await?, 0);
        assert!(profile_b.list_all().await?.is_empty());
        assert!(installation_store.list_all().await?.len() >= 1);
        plurora_service::release_development_host_lease(installation_store, &owner).await?;
        Ok(())
    }

    #[tokio::test]
    async fn runtime_wiring_rejects_a_public_control_store_alias() -> Result<()> {
        let data = tempfile::tempdir()?;
        let shared = Arc::new(InMemoryEventStore::default());
        let owner = plurora_service::acquire_development_host_lease(
            shared.clone(),
            plurora_service::development_registry(),
        )
        .await?;
        let result = runtime_with_installations(
            shared.clone(),
            RuntimeConfig::default(),
            data.path(),
            shared.clone(),
            &owner,
        )
        .await;
        let error = match result {
            Ok(_) => panic!("Runtime and Powerbox control journals must not alias"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("physically distinct"));
        plurora_service::release_development_host_lease(shared, &owner).await?;
        Ok(())
    }

    #[test]
    fn default_runtime_journal_path_is_distinct_from_host_authority() -> Result<()> {
        let data = tempfile::tempdir()?;
        let runtime_root = data.path().join("runtime");
        fs::create_dir(&runtime_root)?;
        let runtime_journal =
            prepare_host_event_journal(&runtime_root, RUNTIME_PUBLIC_JOURNAL_FILE)?;
        let control_journal = runtime_root.join("installations.sqlite3");
        ensure_distinct_journal_paths(&runtime_journal, &control_journal)?;
        assert!(ensure_distinct_journal_paths(&control_journal, &control_journal).is_err());
        Ok(())
    }
}

fn managed_host_listen_line(addr: SocketAddr) -> String {
    format!("PLURORA_HOST_LISTEN_ADDR={addr}")
}

async fn serve_runtime<S>(
    http: SocketAddr,
    runtime: Arc<Runtime<S>>,
    installations: Arc<plurora_service::InstallationRegistry>,
    powerbox: Arc<plurora_service::PowerboxRegistry>,
    realizations: Arc<plurora_service::RealizationRegistry>,
    runs: Arc<plurora_service::RunRegistry>,
    backend_kind: &'static str,
    static_dir: Option<PathBuf>,
    access_token: Option<String>,
    app_base_domain: Option<String>,
    development: Arc<plurora_service::DevelopmentRegistry>,
    owner_lease: plurora_service::DevelopmentHostLease,
) -> Result<()>
where
    S: EventStore,
{
    owner_lease
        .ensure_durable_owner()
        .await
        .context("Host owner lease is not current before control-plane hydration")?;
    anyhow::ensure!(
        http.ip().is_loopback()
            || access_token
                .as_deref()
                .is_some_and(|token| !token.trim().is_empty()),
        "host serve requires a non-empty access token when binding a non-loopback address"
    );
    let host_access = plurora_service::host_access_registry();
    let host_access_events =
        plurora_service::hydrate_host_access_control_plane(runtime.store(), host_access.clone())
            .await
            .context("failed to hydrate durable Host access control plane")?;
    println!("  Host access journal events loaded: {host_access_events}");
    powerbox
        .install_authority_refresh(plurora_service::host_powerbox_authority_refresh(
            runtime.store(),
            host_access.clone(),
        ))
        .context("failed to install durable Powerbox authority validation")?;
    let target_agents = plurora_service::target_agent_registry();
    let target_agent_events = plurora_service::hydrate_target_agent_control_plane(
        runtime.store(),
        target_agents.clone(),
        runtime.config().target_registry.clone(),
    )
    .await
    .context("failed to hydrate durable target agent control plane")?;
    println!("  target agent journal events loaded: {target_agent_events}");
    let build_jobs = plurora_service::build_workload_job_registry();
    let state = plurora_service::AppState {
        runtime: runtime.clone(),
        static_dir: static_dir.clone(),
        access_token: access_token.clone(),
        app_base_domain: app_base_domain.clone(),
        build_jobs: build_jobs.clone(),
        development: development.clone(),
        host_access: host_access.clone(),
        installations: installations.clone(),
        target_agents: target_agents.clone(),
    };
    realizations.install_driver(Arc::new(plurora_service::ServiceRealizationExecutor::new(
        &state,
    )));
    runtime
        .hydrate_workload_from_events()
        .await
        .context("failed to rehydrate managed Realization backend state")?;
    match plurora_service::reconcile_realization_backends(&state).await {
        Ok(summary) => println!(
            "  Realization backend reconcile: target_workloads_projected={} routes_promoted={} routes_removed={} leases_promoted={} leases_released={}",
            summary.target_workloads_projected,
            summary.runtime.routes_promoted,
            summary.runtime.routes_removed,
            summary.runtime.leases_promoted,
            summary.runtime.leases_released
        ),
        Err(error) => eprintln!(
            "warning: Realization backend reconcile paused; durable records were preserved: {error}"
        ),
    }
    let installation_count =
        hydrate_installations_as_host_owner(installations.clone(), &owner_lease)
            .await
            .context("failed to hydrate Installation registry as Host owner")?;
    println!("  installations loaded: {installation_count}");
    owner_lease
        .ensure_durable_owner()
        .await
        .context("Powerbox recovery requires the active Host owner lease")?;
    let powerbox_events = powerbox
        .hydrate()
        .await
        .context("failed to hydrate durable Powerbox registry")?;
    println!("  Powerbox journal events loaded: {powerbox_events}");
    owner_lease
        .ensure_durable_owner()
        .await
        .context("Realization recovery requires the active Host owner lease")?;
    let realization_events = realizations
        .hydrate()
        .await
        .context("failed to hydrate durable Realization registry")?;
    println!("  Realization journal events loaded: {realization_events}");
    owner_lease
        .ensure_durable_owner()
        .await
        .context("Run recovery requires the active Host owner lease")?;
    runs.hydrate().await.context("Run recovery failed")?;
    let interrupted = powerbox
        .reconcile_runs()
        .await
        .context("failed to reconcile Powerbox decisions with recovered Runs")?;
    for binding_id in interrupted.affected_binding_ids {
        runtime
            .run_binding_broker()
            .close_binding(&binding_id, "run_interrupted")
            .await;
    }
    let development_events =
        plurora_service::hydrate_development_control_plane(runtime.store(), development.clone())
            .await
            .context("failed to hydrate durable development control plane")?;
    println!("  development journal events loaded: {development_events}");
    let listener = tokio::net::TcpListener::bind(http).await?;
    let bound_http = listener.local_addr()?;
    println!("{}", managed_host_listen_line(bound_http));
    println!("Plurora host serving http://{bound_http}");
    println!("  event store: {backend_kind} (config redacted)");
    println!("  RPC: POST http://{bound_http}/rpc");
    println!("  SSE: GET  http://{bound_http}/journal/subscribe/:session_id");
    if let Some(static_dir) = &static_dir {
        println!(
            "  static: GET http://{bound_http}/ -> {}",
            static_dir.display()
        );
    }
    if access_token
        .as_deref()
        .is_some_and(|token| !token.is_empty())
    {
        println!("  access token: enabled (value redacted)");
    } else {
        println!("  access token: disabled (local/dev only)");
    }
    if let Some(domain) = app_base_domain
        .as_deref()
        .filter(|domain| !domain.is_empty())
    {
        println!("  app vhost base domain: {domain}");
    }
    let _powerbox_expiry = spawn_powerbox_expiry_supervisor(powerbox, runtime.clone());
    let bootstrap_token = std::env::var("PLURORA_HTTP_BOOTSTRAP_TOKEN")
        .ok()
        .filter(|token| !token.is_empty());
    let app = plurora_service::app_with_state_and_bootstrap_token(state, bootstrap_token);
    axum::serve(listener, app).await?;
    Ok(())
}

fn spawn_powerbox_expiry_supervisor<S>(
    powerbox: Arc<plurora_service::PowerboxRegistry>,
    runtime: Arc<Runtime<S>>,
) -> tokio::task::JoinHandle<()>
where
    S: EventStore,
{
    tokio::spawn(async move {
        loop {
            match powerbox.next_expiry().await {
                Some(expires_at) => {
                    let delay = (expires_at - chrono::Utc::now())
                        .to_std()
                        .unwrap_or_default();
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {}
                        _ = powerbox.expiry_changed() => continue,
                    }
                }
                None => powerbox.expiry_changed().await,
            }
            match powerbox.sweep_expired().await {
                Ok(invalidated) => {
                    for binding_id in invalidated.affected_binding_ids {
                        runtime
                            .run_binding_broker()
                            .close_binding(&binding_id, "powerbox_expired")
                            .await;
                    }
                }
                Err(error) => {
                    eprintln!("Powerbox expiry reconciliation will retry: {error}");
                    tokio::select! {
                        _ = tokio::time::sleep(runtime.config().package_activation_loss_retry_delay) => {}
                        _ = powerbox.expiry_changed() => {}
                    }
                }
            }
        }
    })
}

async fn hydrate_installations_as_host_owner(
    installations: Arc<plurora_service::InstallationRegistry>,
    owner_lease: &plurora_service::DevelopmentHostLease,
) -> Result<usize> {
    owner_lease
        .ensure_durable_owner()
        .await
        .context("Installation recovery requires the active Host owner lease")?;
    installations.install_owner_lease(owner_lease.clone())?;
    installations
        .hydrate()
        .await
        .context("Installation recovery failed")
}

pub(crate) fn resolve_profile_path(profile_path: &std::path::Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        profile_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(path)
    }
}

pub(crate) async fn load_host_profile<S>(
    runtime: Arc<Runtime<S>>,
    profile_path: PathBuf,
) -> Result<()>
where
    S: EventStore,
{
    let raw = fs::read_to_string(&profile_path)
        .with_context(|| format!("failed to read host profile {}", profile_path.display()))?;
    let profile: HostProfile = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse host profile {}", profile_path.display()))?;
    load_profile_packages(runtime, profile, profile_path).await
}

async fn load_profile_packages<S>(
    runtime: Arc<Runtime<S>>,
    profile: HostProfile,
    profile_path: PathBuf,
) -> Result<()>
where
    S: EventStore,
{
    if let Some(title) = &profile.title {
        println!("loading host profile: {title}");
    }
    let base = profile_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let autoload_count = profile.autoload.len();
    println!("host autoload manifests: {autoload_count}");
    for (index, manifest_path) in profile.autoload.into_iter().enumerate() {
        let resolved = if manifest_path.is_absolute() {
            manifest_path
        } else {
            base.join(manifest_path)
        };
        println!(
            "autoloading package manifest {}/{}: {}",
            index + 1,
            autoload_count,
            resolved.display()
        );
        if should_skip_dangling_store_autoload(&resolved) {
            continue;
        }
        let manifest = read_manifest(resolved.clone()).await.with_context(|| {
            format!("failed to autoload package manifest {}", resolved.display())
        })?;
        let package_id = manifest.id.clone();
        let is_subprocess = matches!(
            manifest.entry.kind,
            plurora_core::PackageEntry::Subprocess { .. }
        );
        let record = match runtime.load_package(manifest).await.with_context(|| {
            format!(
                "failed to load autoload package {} from {}",
                package_id,
                resolved.display()
            )
        }) {
            Ok(record) => record,
            Err(error) if is_subprocess => {
                eprintln!("autoloaded subprocess package failed and was left degraded: {error:#}");
                continue;
            }
            Err(error) => return Err(error),
        };
        println!(
            "autoloaded package: {}@{} ({:?})",
            record.id, record.version, record.state
        );
    }
    Ok(())
}

pub(crate) async fn host_stdio() -> Result<()> {
    let runtime_store = Arc::new(InMemoryEventStore::default());
    let control_store = Arc::new(InMemoryEventStore::default());
    let object_store = Arc::new(InMemoryObjectStore::default());
    let installations = plurora_service::InstallationRegistry::ephemeral(
        control_store.clone(),
        object_store.clone(),
    )?;
    installations
        .hydrate()
        .await
        .context("failed to hydrate ephemeral Installation registry")?;
    let runs = plurora_service::RunRegistry::new(control_store.clone());
    let powerbox = plurora_service::PowerboxRegistry::new(
        control_store.clone(),
        runtime_store.clone(),
        installations.clone(),
        runs.clone(),
    )?;
    let target_registry = Arc::new(plurora_runtime::ExecutionTargetRegistry::default());
    let realizations = plurora_service::RealizationRegistry::new(
        control_store,
        runtime_store.clone(),
        object_store.clone(),
        installations.clone(),
        target_registry.clone(),
    )?;
    let runtime = Arc::new(Runtime::new(
        runtime_store,
        RuntimeConfig {
            object_store,
            installation_control: installations,
            run_control: runs.clone(),
            powerbox_control: powerbox.clone(),
            realization_control: realizations.clone(),
            target_registry,
            ..RuntimeConfig::default()
        },
    ));
    runs.install_driver(Arc::new(plurora_runtime::AssemblyRuntimeDriver::new(
        Arc::downgrade(&runtime),
    )))?;
    powerbox.install_inspector(Arc::new(plurora_service::RuntimePowerboxInspector::new(
        Arc::downgrade(&runtime),
    )))?;
    powerbox.hydrate().await?;
    realizations.hydrate().await?;
    runs.hydrate().await?;
    let context = ProtocolContext::host_dev("host_stdio");
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let response = host_stdio_response(&runtime, &context, &line).await;
        stdout
            .write_all(serde_json::to_string(&response)?.as_bytes())
            .await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }
    Ok(())
}

async fn host_stdio_response<S>(
    runtime: &Runtime<S>,
    context: &ProtocolContext,
    line: &str,
) -> plurora_runtime::ProtocolResponse
where
    S: EventStore,
{
    let raw = match serde_json::from_str::<serde_json::Value>(line) {
        Ok(raw) => raw,
        Err(error) => {
            return plurora_runtime::ProtocolResponse {
                id: "invalid".to_string(),
                result: None,
                error: Some(plurora_runtime::ProtocolError::invalid_request(
                    error.to_string(),
                )),
            };
        }
    };
    let id = raw
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("invalid")
        .to_string();
    let request = match serde_json::from_value::<plurora_runtime::ProtocolRequest>(raw) {
        Ok(request) => request,
        Err(error) => {
            return plurora_runtime::ProtocolResponse {
                id,
                result: None,
                error: Some(plurora_runtime::ProtocolError::invalid_request(
                    error.to_string(),
                )),
            };
        }
    };
    let plurora_runtime::ProtocolRequest {
        id,
        method,
        session_id,
        contract,
        params,
    } = request;
    let mut request_context = context.clone();
    request_context.session_id = session_id;
    match runtime
        .call_protocol_negotiated(&request_context, &method, params, contract.as_ref())
        .await
    {
        Ok(result) => plurora_runtime::ProtocolResponse {
            id,
            result: Some(result),
            error: None,
        },
        Err(error) => plurora_runtime::ProtocolResponse {
            id,
            result: None,
            error: Some(error),
        },
    }
}
