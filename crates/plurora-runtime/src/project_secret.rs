//! Project-scoped secret resolver with optional fallback to platform store.
//!
//! Resolves `secret_ref:project:NAME` against a per-project encrypted store.
//! When the active project is known and the entry isn't present locally, may
//! fall back to the platform store based on the project's secret policy.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use plurora_core::project::ProjectId;
use plurora_core::secret_ref::extract_project_name;

use crate::secret::{HostSecretResolver, StoreSecretResolver};
use crate::secret_store::{load_store, resolve_master_key};

/// Resolves project-scoped secret references.
///
/// Construction takes a callback that returns the *current* active project id
/// at call time (or None if no project is in scope). This indirection lets the
/// runtime's active scope evolve (per-call session lookup) without rebuilding
/// the resolver.
///
/// Optional `platform_fallback` resolver is consulted when the project store
/// doesn't contain the requested name AND the resolver was constructed with
/// fallback enabled. Construction-time fallback is the default; per-project
/// secret policy can override at call time via `ProjectScopeContext`.
pub struct ProjectStoreSecretResolver {
    /// Callback returning the active project id and policy at resolution time.
    active_scope: Arc<dyn Fn() -> Option<ProjectScopeContext> + Send + Sync>,
    /// Platform-level fallback resolver (consulted when project resolution misses
    /// AND the active scope's policy permits fallback).
    platform_fallback: Option<Arc<StoreSecretResolver>>,
}

#[derive(Clone, Debug)]
pub struct ProjectScopeContext {
    pub project_id: ProjectId,
    pub project_store_path: PathBuf,
    pub fallback_to_platform: bool,
    /// Names that MUST resolve from project scope (no fallback regardless).
    pub require_per_project: Vec<String>,
}

impl ProjectStoreSecretResolver {
    pub fn new<F>(active_scope: F) -> Self
    where
        F: Fn() -> Option<ProjectScopeContext> + Send + Sync + 'static,
    {
        Self {
            active_scope: Arc::new(active_scope),
            platform_fallback: None,
        }
    }

    pub fn with_platform_fallback(mut self, platform: Arc<StoreSecretResolver>) -> Self {
        self.platform_fallback = Some(platform);
        self
    }
}

impl std::fmt::Debug for ProjectStoreSecretResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectStoreSecretResolver")
            .field("has_platform_fallback", &self.platform_fallback.is_some())
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl HostSecretResolver for ProjectStoreSecretResolver {
    async fn resolve(&self, ref_id: &str) -> anyhow::Result<String> {
        let name = extract_project_name(ref_id).ok_or_else(|| {
            anyhow::anyhow!(
                "secret resolution denied: not a project-backed reference (ref_id='{}')",
                ref_id
            )
        })?;

        let scope = (self.active_scope)().ok_or_else(|| {
            anyhow::anyhow!(
                "secret resolution failed: no active project scope (ref_id='{}')",
                ref_id
            )
        })?;

        // Try project store first.
        let project_result = read_project_secret(&scope.project_store_path, name).await;

        match project_result {
            Ok(Some(value)) => Ok(value),
            Ok(None) => {
                // Not found in project store. Check fallback policy.
                let must_be_per_project = scope.require_per_project.iter().any(|n| n == name);

                if must_be_per_project {
                    anyhow::bail!(
                        "secret resolution failed: '{}' is required at project scope but not configured for project '{}' (ref_id='{}')",
                        name,
                        scope.project_id,
                        ref_id
                    );
                }

                if !scope.fallback_to_platform {
                    anyhow::bail!(
                        "secret resolution failed: name '{}' not in project '{}' store and platform fallback is disabled (ref_id='{}')",
                        name,
                        scope.project_id,
                        ref_id
                    );
                }

                let platform = self.platform_fallback.as_ref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "secret resolution failed: project '{}' has no entry '{}' and no platform fallback resolver is configured (ref_id='{}')",
                        scope.project_id,
                        name,
                        ref_id
                    )
                })?;

                // Translate the project ref to the equivalent store ref for the platform resolver.
                let platform_ref = format!("secret_ref:store:{}", name);
                platform.resolve(&platform_ref).await
            }
            Err(e) => Err(e),
        }
    }
}

/// Read a single secret from a project store file. Returns Ok(None) if the
/// store file doesn't exist or the name isn't present.
async fn read_project_secret(
    store_path: &std::path::Path,
    name: &str,
) -> anyhow::Result<Option<String>> {
    if !store_path.exists() {
        return Ok(None);
    }
    let path_owned = store_path.to_path_buf();
    let name_owned = name.to_string();
    tokio::task::spawn_blocking(move || {
        let (key, _) = resolve_master_key()?;
        let store = load_store(&path_owned, &key)?;
        Ok(store.secrets.get(&name_owned).cloned())
    })
    .await?
}
