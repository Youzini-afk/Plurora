use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::{DateTime, Utc};
use plurora_core::{
    package_envelope_for_manifest, ComponentBoundaryClaims, ComponentDescriptor,
    ComponentTrustClass, PackageEntry, PackageEnvelopeDescriptor, PackageId, PackageManifest,
    RedactionState,
};
use plurora_work::RunId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageState {
    Discovered,
    Loading,
    Starting,
    Ready,
    Degraded,
    Stopping,
    Stopped,
    Unloaded,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PackageRecord {
    pub id: PackageId,
    pub version: String,
    pub state: PackageState,
    pub entry_kind: String,
    pub trust_level: TrustLevel,
    /// Canonical Contract v2 trust classification. Unlike `trust_level`, this
    /// distinguishes `contract:none` as a foreign capsule.
    #[serde(default)]
    pub trust_class: ComponentTrustClass,
    #[serde(default)]
    pub enforced_boundaries: ComponentBoundaryClaims,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_envelope: Option<PackageEnvelopeDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ComponentDescriptor>,
    pub capability_count: usize,
    pub hook_count: usize,
    pub extension_point_count: usize,
    pub loaded_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_failure: Option<PackageFailureSummary>,
    pub manifest: PackageManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PackageFailureSummary {
    pub package_id: PackageId,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    pub failed_at: DateTime<Utc>,
    pub stderr_tail_redacted: Vec<String>,
    pub log_tail_redacted: Vec<crate::SubprocessLogLine>,
    pub stderr_truncated: bool,
    pub redaction_state: RedactionState,
    pub state: PackageState,
}

impl PackageRecord {
    pub fn ready(manifest: PackageManifest) -> anyhow::Result<Self> {
        let now = Utc::now();
        let package_envelope = package_envelope_for_manifest(&manifest)?;
        let components = package_envelope.components.clone();
        let primary_component = components
            .first()
            .ok_or_else(|| anyhow::anyhow!("package '{}' has no component", manifest.id))?;
        Ok(Self {
            id: manifest.id.clone(),
            version: manifest.version.clone(),
            state: PackageState::Ready,
            entry_kind: entry_kind(&manifest.entry.kind).to_string(),
            trust_level: trust_level(&manifest.entry.kind),
            trust_class: primary_component.trust_class,
            enforced_boundaries: primary_component.enforced_boundaries.clone(),
            package_envelope: Some(package_envelope),
            components,
            capability_count: manifest.provides.len(),
            hook_count: manifest.contributes.hooks.len(),
            extension_point_count: manifest.contributes.extension_points.len(),
            loaded_at: now,
            updated_at: now,
            last_failure: None,
            manifest,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    TrustedInproc,
    ProcessIsolated,
    WasmSandbox,
    RemoteBoundary,
    StaticSurface,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HostPolicy {
    pub allowed_entry_kinds: Vec<String>,
    pub max_memory_mb: u64,
}

impl Default for HostPolicy {
    fn default() -> Self {
        Self {
            allowed_entry_kinds: vec![
                "rust_inproc".to_string(),
                "subprocess".to_string(),
                "wasm".to_string(),
                "remote".to_string(),
                "surface_bundle".to_string(),
            ],
            max_memory_mb: 512,
        }
    }
}

impl HostPolicy {
    pub fn validate(&self, manifest: &PackageManifest) -> anyhow::Result<()> {
        let kind = entry_kind(&manifest.entry.kind);
        if !self
            .allowed_entry_kinds
            .iter()
            .any(|allowed| allowed == kind)
        {
            anyhow::bail!("entry kind '{kind}' is not allowed by host policy");
        }
        if manifest.sandbox_policy.memory_mb > self.max_memory_mb {
            anyhow::bail!(
                "package '{}' requests {} MiB, above host policy max {} MiB",
                manifest.id,
                manifest.sandbox_policy.memory_mb,
                self.max_memory_mb
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageRunClaim {
    package_id: PackageId,
    package_envelope_digest: Option<String>,
    component_id: String,
    component_artifact: plurora_core::ArtifactDescriptor,
    component_behavior: plurora_core::ArtifactDescriptor,
    trust_class: ComponentTrustClass,
    component_entry_kind: String,
    entry: serde_json::Value,
}

impl PackageRunClaim {
    pub(crate) fn exact(
        package: &PackageRecord,
        component: &ComponentDescriptor,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            package
                .components
                .iter()
                .any(|candidate| candidate == component),
            "Run Package claim does not belong to the selected Package"
        );
        Ok(Self {
            package_id: package.id.clone(),
            package_envelope_digest: package
                .package_envelope
                .as_ref()
                .map(|envelope| envelope.artifact.digest.clone()),
            component_id: component.component_id.clone(),
            component_artifact: component.artifact.clone(),
            component_behavior: component.behavior.clone(),
            trust_class: component.trust_class,
            component_entry_kind: component.entry_kind.clone(),
            entry: serde_json::to_value(&package.manifest.entry)?,
        })
    }

    pub(crate) fn package_id(&self) -> &PackageId {
        &self.package_id
    }
}

#[derive(Default)]
struct PackageRegistryState {
    packages: HashMap<PackageId, PackageRecord>,
    active_run_leases: HashMap<PackageId, BTreeSet<RunId>>,
}

#[derive(Default)]
struct PackageRegistryInner {
    state: Mutex<PackageRegistryState>,
}

/// An exact, reference-counted lease over every Package used by one active Run.
///
/// The lease is intentionally not clonable: one acquisition corresponds to one
/// Run activation. Dropping it releases only that Run's references.
pub(crate) struct PackageRunLease {
    inner: Arc<PackageRegistryInner>,
    run_id: RunId,
    package_ids: Vec<PackageId>,
}

impl std::fmt::Debug for PackageRunLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PackageRunLease")
            .field("run_id", &self.run_id)
            .field("package_ids", &self.package_ids)
            .finish_non_exhaustive()
    }
}

impl Drop for PackageRunLease {
    fn drop(&mut self) {
        let mut state = lock_registry(&self.inner);
        for package_id in &self.package_ids {
            let remove = state
                .active_run_leases
                .get_mut(package_id)
                .is_some_and(|run_ids| {
                    run_ids.remove(&self.run_id);
                    run_ids.is_empty()
                });
            if remove {
                state.active_run_leases.remove(package_id);
            }
        }
    }
}

#[derive(Clone, Default)]
pub struct PackageRegistry {
    inner: Arc<PackageRegistryInner>,
}

impl PackageRegistry {
    pub async fn load(
        &self,
        manifest: PackageManifest,
        policy: &HostPolicy,
    ) -> anyhow::Result<PackageRecord> {
        self.insert(manifest, policy, PackageState::Ready)
    }

    pub(crate) async fn begin_load(
        &self,
        manifest: PackageManifest,
        policy: &HostPolicy,
    ) -> anyhow::Result<PackageRecord> {
        self.insert(manifest, policy, PackageState::Loading)
    }

    fn insert(
        &self,
        manifest: PackageManifest,
        policy: &HostPolicy,
        state: PackageState,
    ) -> anyhow::Result<PackageRecord> {
        manifest.validate_basic()?;
        policy.validate(&manifest)?;

        let mut registry = lock_registry(&self.inner);
        if registry.packages.contains_key(&manifest.id) {
            anyhow::bail!("package '{}' is already loaded", manifest.id);
        }
        let mut record = PackageRecord::ready(manifest)?;
        record.state = state;
        registry.packages.insert(record.id.clone(), record.clone());
        Ok(record)
    }

    pub async fn unload(&self, package_id: &PackageId) -> anyhow::Result<PackageRecord> {
        let mut registry = lock_registry(&self.inner);
        ensure_package_not_in_use(&registry, package_id)?;
        let mut record = registry
            .packages
            .remove(package_id)
            .ok_or_else(|| anyhow::anyhow!("package '{package_id}' is not loaded"))?;
        record.state = PackageState::Unloaded;
        record.updated_at = Utc::now();
        Ok(record)
    }

    pub async fn list(&self) -> Vec<PackageRecord> {
        let mut records: Vec<_> = lock_registry(&self.inner)
            .packages
            .values()
            .cloned()
            .collect();
        records.sort_by(|a, b| a.id.cmp(&b.id));
        records
    }

    pub async fn status(&self, package_id: &PackageId) -> Option<PackageRecord> {
        lock_registry(&self.inner).packages.get(package_id).cloned()
    }

    pub(crate) async fn set_state(
        &self,
        package_id: &PackageId,
        state: PackageState,
    ) -> Option<PackageRecord> {
        let mut registry = lock_registry(&self.inner);
        let record = registry.packages.get_mut(package_id)?;
        record.state = state;
        record.updated_at = Utc::now();
        if matches!(record.state, PackageState::Ready) {
            record.last_failure = None;
        }
        Some(record.clone())
    }

    pub(crate) async fn finish_subprocess_start(
        &self,
        package_id: &PackageId,
    ) -> Option<PackageRecord> {
        self.commit_state_transition(package_id, PackageState::Starting, PackageState::Ready)
    }

    pub(crate) async fn state_transition_candidate(
        &self,
        package_id: &PackageId,
        expected: PackageState,
        next: PackageState,
    ) -> Option<PackageRecord> {
        let registry = lock_registry(&self.inner);
        let mut candidate = registry.packages.get(package_id)?.clone();
        if candidate.state != expected {
            return None;
        }
        candidate.state = next;
        candidate.updated_at = Utc::now();
        if candidate.state == PackageState::Ready {
            candidate.last_failure = None;
        }
        Some(candidate)
    }

    pub(crate) fn commit_state_transition(
        &self,
        package_id: &PackageId,
        expected: PackageState,
        next: PackageState,
    ) -> Option<PackageRecord> {
        let mut registry = lock_registry(&self.inner);
        let record = registry.packages.get_mut(package_id)?;
        if record.state != expected {
            return None;
        }
        record.state = next;
        record.updated_at = Utc::now();
        if record.state == PackageState::Ready {
            record.last_failure = None;
        }
        Some(record.clone())
    }

    pub(crate) async fn begin_unload(
        &self,
        package_id: &PackageId,
    ) -> anyhow::Result<PackageRecord> {
        self.begin_stopping(package_id, false)
    }

    pub(crate) async fn begin_restart(
        &self,
        package_id: &PackageId,
    ) -> anyhow::Result<PackageRecord> {
        self.begin_stopping(package_id, true)
    }

    fn begin_stopping(
        &self,
        package_id: &PackageId,
        require_subprocess: bool,
    ) -> anyhow::Result<PackageRecord> {
        let mut registry = lock_registry(&self.inner);
        ensure_package_not_in_use(&registry, package_id)?;
        let record = registry
            .packages
            .get_mut(package_id)
            .ok_or_else(|| anyhow::anyhow!("package '{package_id}' is not loaded"))?;
        anyhow::ensure!(
            !matches!(record.state, PackageState::Stopping | PackageState::Stopped),
            "package '{package_id}' already has a lifecycle transition in progress"
        );
        if require_subprocess {
            anyhow::ensure!(
                matches!(record.manifest.entry.kind, PackageEntry::Subprocess { .. }),
                "package '{package_id}' entry kind '{}' cannot restart yet",
                record.entry_kind
            );
        }
        record.state = PackageState::Stopping;
        record.updated_at = Utc::now();
        Ok(record.clone())
    }

    pub(crate) async fn acquire_run_lease(
        &self,
        run_id: &RunId,
        claims: &[PackageRunClaim],
    ) -> anyhow::Result<PackageRunLease> {
        let mut registry = lock_registry(&self.inner);
        let mut package_ids = BTreeSet::new();

        // Validate the complete claim set before incrementing any reference so
        // acquisition is all-or-nothing across a multi-Package Assembly.
        for claim in claims {
            let package = registry
                .packages
                .get(&claim.package_id)
                .ok_or_else(|| anyhow::anyhow!("Run Package changed after preflight"))?;
            anyhow::ensure!(
                package.state == PackageState::Ready,
                "Run Package is no longer Ready after preflight"
            );
            anyhow::ensure!(
                package
                    .package_envelope
                    .as_ref()
                    .map(|envelope| envelope.artifact.digest.as_str())
                    == claim.package_envelope_digest.as_deref(),
                "Run Package envelope changed after preflight"
            );
            anyhow::ensure!(
                serde_json::to_value(&package.manifest.entry)? == claim.entry,
                "Run Package entry changed after preflight"
            );
            let component = package
                .components
                .iter()
                .find(|component| component.component_id == claim.component_id)
                .ok_or_else(|| anyhow::anyhow!("Run component changed after preflight"))?;
            anyhow::ensure!(
                component.artifact == claim.component_artifact
                    && component.behavior == claim.component_behavior
                    && component.trust_class == claim.trust_class
                    && component.entry_kind == claim.component_entry_kind
                    && component.entry_kind == package.entry_kind,
                "Run component identity changed after preflight"
            );
            package_ids.insert(claim.package_id.clone());
        }

        let package_ids = package_ids.into_iter().collect::<Vec<_>>();
        anyhow::ensure!(
            package_ids.iter().all(|package_id| {
                !registry
                    .active_run_leases
                    .get(package_id)
                    .is_some_and(|run_ids| run_ids.contains(run_id))
            }),
            "Run already holds a Package activation lease"
        );
        for package_id in &package_ids {
            registry
                .active_run_leases
                .entry(package_id.clone())
                .or_default()
                .insert(run_id.clone());
        }
        drop(registry);
        Ok(PackageRunLease {
            inner: self.inner.clone(),
            run_id: run_id.clone(),
            package_ids,
        })
    }

    #[cfg(test)]
    pub(crate) async fn active_run_lease_count(&self, package_id: &PackageId) -> usize {
        lock_registry(&self.inner)
            .active_run_leases
            .get(package_id)
            .map(BTreeSet::len)
            .unwrap_or(0)
    }

    pub(crate) async fn mark_activation_lost(
        &self,
        package_id: &PackageId,
    ) -> Option<(PackageRecord, Vec<RunId>)> {
        let mut registry = lock_registry(&self.inner);
        let record = registry.packages.get_mut(package_id)?;
        if !matches!(record.state, PackageState::Starting | PackageState::Ready) {
            return None;
        }
        record.state = PackageState::Degraded;
        record.updated_at = Utc::now();
        let record = record.clone();
        let run_ids = registry
            .active_run_leases
            .get(package_id)
            .map(|run_ids| run_ids.iter().cloned().collect())
            .unwrap_or_default();
        Some((record, run_ids))
    }

    pub async fn set_last_failure(
        &self,
        package_id: &PackageId,
        failure: PackageFailureSummary,
    ) -> Option<PackageRecord> {
        let mut registry = lock_registry(&self.inner);
        let record = registry.packages.get_mut(package_id)?;
        record.last_failure = Some(failure);
        record.updated_at = Utc::now();
        Some(record.clone())
    }

    pub async fn permissions(&self, package_id: &PackageId) -> Option<plurora_core::PermissionSet> {
        lock_registry(&self.inner)
            .packages
            .get(package_id)
            .map(|record| record.manifest.permissions.clone())
    }

    pub async fn manifest(&self, package_id: &PackageId) -> Option<PackageManifest> {
        lock_registry(&self.inner)
            .packages
            .get(package_id)
            .map(|record| record.manifest.clone())
    }
}

fn lock_registry(inner: &PackageRegistryInner) -> MutexGuard<'_, PackageRegistryState> {
    inner
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn ensure_package_not_in_use(
    registry: &PackageRegistryState,
    package_id: &PackageId,
) -> anyhow::Result<()> {
    let active_runs = registry
        .active_run_leases
        .get(package_id)
        .map(BTreeSet::len)
        .unwrap_or(0);
    anyhow::ensure!(
        active_runs == 0,
        "package '{package_id}' is in use by {active_runs} active Run(s)"
    );
    Ok(())
}

pub fn entry_kind(entry: &PackageEntry) -> &'static str {
    match entry {
        PackageEntry::RustInproc { .. } => "rust_inproc",
        PackageEntry::Subprocess { .. } => "subprocess",
        PackageEntry::Wasm { .. } => "wasm",
        PackageEntry::Remote { .. } => "remote",
        PackageEntry::SurfaceBundle { .. } => "surface_bundle",
    }
}

pub fn trust_level(entry: &PackageEntry) -> TrustLevel {
    match entry {
        PackageEntry::RustInproc { .. } => TrustLevel::TrustedInproc,
        PackageEntry::Subprocess { .. } => TrustLevel::ProcessIsolated,
        PackageEntry::Wasm { .. } => TrustLevel::WasmSandbox,
        PackageEntry::Remote { .. } => TrustLevel::RemoteBoundary,
        PackageEntry::SurfaceBundle { .. } => TrustLevel::StaticSurface,
    }
}

#[cfg(test)]
mod tests {
    use plurora_core::{
        EntryDescriptor, PackageContributions, PackageEntry, PackageManifest, PermissionSet,
        SandboxPolicy,
    };
    use serde_json::Value;

    use super::*;

    fn manifest(id: &str) -> PackageManifest {
        PackageManifest {
            schema_version: 1,
            id: id.to_string(),
            version: "0.1.0".to_string(),
            display_name: None,
            description: None,
            author: None,
            license: None,
            entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                crate_ref: "example".to_string(),
                symbol: "register".to_string(),
                abi_version: 1,
            }),
            provides: Vec::new(),
            consumes: Vec::new(),
            requires: Vec::new(),
            contributes: PackageContributions::default(),
            permissions: PermissionSet::default(),
            sandbox_policy: SandboxPolicy::default(),
        }
    }

    fn subprocess_manifest(id: &str, command: &str) -> PackageManifest {
        let mut manifest = manifest(id);
        manifest.entry = EntryDescriptor::v1(PackageEntry::Subprocess {
            command: vec![command.to_string()],
            transport: plurora_core::SubprocessTransport::JsonRpcStdio,
        });
        manifest
    }

    #[tokio::test]
    async fn loads_lists_and_unloads_package() -> anyhow::Result<()> {
        let registry = PackageRegistry::default();
        let record = registry
            .load(manifest("org/pkg"), &HostPolicy::default())
            .await?;
        assert_eq!(record.state, PackageState::Ready);
        assert_eq!(registry.list().await.len(), 1);
        assert!(registry.status(&"org/pkg".to_string()).await.is_some());

        let unloaded = registry.unload(&"org/pkg".to_string()).await?;
        assert_eq!(unloaded.state, PackageState::Unloaded);
        assert!(registry.list().await.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn rejects_policy_disallowed_entry() {
        let mut policy = HostPolicy::default();
        policy.allowed_entry_kinds = vec!["subprocess".to_string()];
        let registry = PackageRegistry::default();
        let result = registry.load(manifest("org/pkg"), &policy).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn run_leases_reference_count_and_exclude_unload_and_restart() -> anyhow::Result<()> {
        let registry = PackageRegistry::default();
        let package_id = "org/run-shared".to_string();
        let record = registry
            .load(
                subprocess_manifest(&package_id, "fixture-one"),
                &HostPolicy::default(),
            )
            .await?;
        let claim = PackageRunClaim::exact(&record, &record.components[0])?;
        let run_a = RunId::new();
        let run_b = RunId::new();
        let lease_a = registry
            .acquire_run_lease(&run_a, std::slice::from_ref(&claim))
            .await?;
        let lease_b = registry
            .acquire_run_lease(&run_b, std::slice::from_ref(&claim))
            .await?;
        assert_eq!(registry.active_run_lease_count(&package_id).await, 2);

        assert!(registry.begin_unload(&package_id).await.is_err());
        assert!(registry.begin_restart(&package_id).await.is_err());
        assert!(registry.unload(&package_id).await.is_err());

        drop(lease_a);
        assert_eq!(registry.active_run_lease_count(&package_id).await, 1);
        assert!(registry.begin_unload(&package_id).await.is_err());

        drop(lease_b);
        assert_eq!(registry.active_run_lease_count(&package_id).await, 0);
        let restarting = registry.begin_restart(&package_id).await?;
        assert_eq!(restarting.state, PackageState::Stopping);
        registry.set_state(&package_id, PackageState::Ready).await;
        let unloaded = registry.unload(&package_id).await?;
        assert_eq!(unloaded.state, PackageState::Unloaded);
        Ok(())
    }

    #[tokio::test]
    async fn stale_exact_claim_cannot_lease_a_reloaded_package() -> anyhow::Result<()> {
        let registry = PackageRegistry::default();
        let package_id = "org/run-race".to_string();
        let original = registry
            .load(
                subprocess_manifest(&package_id, "fixture-one"),
                &HostPolicy::default(),
            )
            .await?;
        let claim = PackageRunClaim::exact(&original, &original.components[0])?;
        registry.unload(&package_id).await?;
        registry
            .load(
                subprocess_manifest(&package_id, "fixture-two"),
                &HostPolicy::default(),
            )
            .await?;

        let error = registry
            .acquire_run_lease(&RunId::new(), std::slice::from_ref(&claim))
            .await
            .expect_err("a preflight claim must not bind a replacement Package");
        assert!(error.to_string().contains("changed after preflight"));
        assert_eq!(registry.active_run_lease_count(&package_id).await, 0);
        Ok(())
    }

    #[tokio::test]
    async fn activation_loss_snapshots_exact_runs_and_lease_drop_is_run_scoped(
    ) -> anyhow::Result<()> {
        let registry = PackageRegistry::default();
        let package_a = registry
            .load(manifest("org/package-a"), &HostPolicy::default())
            .await?;
        let package_b = registry
            .load(manifest("org/package-b"), &HostPolicy::default())
            .await?;
        let claim_a = PackageRunClaim::exact(&package_a, &package_a.components[0])?;
        let claim_b = PackageRunClaim::exact(&package_b, &package_b.components[0])?;
        let run_a = RunId::new();
        let run_b = RunId::new();
        let unrelated = RunId::new();
        let lease_a = registry
            .acquire_run_lease(&run_a, std::slice::from_ref(&claim_a))
            .await?;
        let lease_b = registry
            .acquire_run_lease(&run_b, std::slice::from_ref(&claim_a))
            .await?;
        let unrelated_lease = registry
            .acquire_run_lease(&unrelated, std::slice::from_ref(&claim_b))
            .await?;

        let (degraded, affected) = registry
            .mark_activation_lost(&package_a.id)
            .await
            .expect("Ready Package transitions once");
        assert_eq!(degraded.state, PackageState::Degraded);
        let mut expected = vec![run_a.clone(), run_b.clone()];
        expected.sort();
        assert_eq!(affected, expected);
        assert!(registry.mark_activation_lost(&package_a.id).await.is_none());
        assert_eq!(registry.active_run_lease_count(&package_b.id).await, 1);

        drop(lease_a);
        assert_eq!(registry.active_run_lease_count(&package_a.id).await, 1);
        drop(lease_b);
        assert_eq!(registry.active_run_lease_count(&package_a.id).await, 0);
        assert_eq!(registry.active_run_lease_count(&package_b.id).await, 1);
        drop(unrelated_lease);
        assert_eq!(registry.active_run_lease_count(&package_b.id).await, 0);
        Ok(())
    }

    #[test]
    fn entry_kind_names_are_manifest_names() {
        let entry = PackageEntry::Remote {
            endpoint: "https://example.test".to_string(),
            auth: plurora_core::RemoteAuth {
                scheme: "none".to_string(),
                config: Value::Null,
            },
        };
        assert_eq!(entry_kind(&entry), "remote");
        assert_eq!(trust_level(&entry), TrustLevel::RemoteBoundary);

        let surface = PackageEntry::SurfaceBundle {
            bundle: "dist/bundle.mjs".to_string(),
        };
        assert_eq!(entry_kind(&surface), "surface_bundle");
        assert_eq!(trust_level(&surface), TrustLevel::StaticSurface);
    }
}
