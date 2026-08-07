use std::collections::HashMap;

use chrono::{DateTime, Utc};
use plurora_core::{
    package_envelope_for_manifest, ComponentBoundaryClaims, ComponentDescriptor,
    ComponentTrustClass, PackageEntry, PackageEnvelopeDescriptor, PackageId, PackageManifest,
    RedactionState,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

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

#[derive(Default)]
pub struct PackageRegistry {
    packages: RwLock<HashMap<PackageId, PackageRecord>>,
}

impl PackageRegistry {
    pub async fn load(
        &self,
        manifest: PackageManifest,
        policy: &HostPolicy,
    ) -> anyhow::Result<PackageRecord> {
        manifest.validate_basic()?;
        policy.validate(&manifest)?;

        let mut packages = self.packages.write().await;
        if packages.contains_key(&manifest.id) {
            anyhow::bail!("package '{}' is already loaded", manifest.id);
        }
        let record = PackageRecord::ready(manifest)?;
        packages.insert(record.id.clone(), record.clone());
        Ok(record)
    }

    pub async fn unload(&self, package_id: &PackageId) -> anyhow::Result<PackageRecord> {
        let mut packages = self.packages.write().await;
        let mut record = packages
            .remove(package_id)
            .ok_or_else(|| anyhow::anyhow!("package '{package_id}' is not loaded"))?;
        record.state = PackageState::Unloaded;
        record.updated_at = Utc::now();
        Ok(record)
    }

    pub async fn list(&self) -> Vec<PackageRecord> {
        let mut records: Vec<_> = self.packages.read().await.values().cloned().collect();
        records.sort_by(|a, b| a.id.cmp(&b.id));
        records
    }

    pub async fn status(&self, package_id: &PackageId) -> Option<PackageRecord> {
        self.packages.read().await.get(package_id).cloned()
    }

    pub async fn set_state(
        &self,
        package_id: &PackageId,
        state: PackageState,
    ) -> Option<PackageRecord> {
        let mut packages = self.packages.write().await;
        let record = packages.get_mut(package_id)?;
        record.state = state;
        record.updated_at = Utc::now();
        if matches!(record.state, PackageState::Ready) {
            record.last_failure = None;
        }
        Some(record.clone())
    }

    pub async fn set_last_failure(
        &self,
        package_id: &PackageId,
        failure: PackageFailureSummary,
    ) -> Option<PackageRecord> {
        let mut packages = self.packages.write().await;
        let record = packages.get_mut(package_id)?;
        record.last_failure = Some(failure);
        record.updated_at = Utc::now();
        Some(record.clone())
    }

    pub async fn permissions(&self, package_id: &PackageId) -> Option<plurora_core::PermissionSet> {
        self.packages
            .read()
            .await
            .get(package_id)
            .map(|record| record.manifest.permissions.clone())
    }

    pub async fn manifest(&self, package_id: &PackageId) -> Option<PackageManifest> {
        self.packages
            .read()
            .await
            .get(package_id)
            .map(|record| record.manifest.clone())
    }
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
