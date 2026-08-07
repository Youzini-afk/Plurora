//! Host secret resolver trait and configuration.
//!
//! The `HostSecretResolver` trait defines how the runtime resolves
//! `secret_ref` identifiers at execution time. Resolution is only
//! allowed during capability invocation; resolved raw secrets must
//! never be written back into events, proposals, logs, or audit records.
//!
//! This module provides the contract and a host-owned `EnvSecretResolver`
//! that resolves environment-variable-backed secrets via an explicit
//! allowlist. Production vault integrations belong in host-level packages,
//! not the kernel.v1.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

/// A host-level secret resolver that resolves `secret_ref` identifiers
/// to their raw values at runtime.
///
/// ## Contract
///
/// - Resolution is only permitted during capability invocation by the
///   runtime, not in event/proposal/audit paths.
/// - The resolved raw value must never be persisted or logged by the
///   kernel or any package.
/// - Packages reference secrets via `SecretRef` identifiers; they never
///   handle raw secret values.
#[async_trait]
pub trait HostSecretResolver: Send + Sync + 'static {
    /// Resolve a secret reference to its raw value.
    ///
    /// Returns the raw secret string if found, or an error if the
    /// reference cannot be resolved.
    async fn resolve(&self, ref_id: &str) -> anyhow::Result<String>;
}

/// A default resolver that denies all secret resolution.
///
/// Use this when no secret vault is configured. Any attempt to
/// resolve a secret reference will fail with a clear error.
pub struct DenyAllSecretResolver;

#[async_trait]
impl HostSecretResolver for DenyAllSecretResolver {
    async fn resolve(&self, ref_id: &str) -> anyhow::Result<String> {
        anyhow::bail!(
            "secret resolution denied: no secret resolver configured (ref_id='{}')",
            ref_id
        )
    }
}

/// A host-owned environment-variable secret resolver.
///
/// Resolves `secret_ref:env:NAME`, `secretRef:env:NAME`,
/// `secret-ref:env:NAME`, and `host:env:NAME` references by reading
/// the named environment variable. Only environment variable names
/// that appear in the explicit allowlist are permitted; everything
/// else is denied (fail-closed).
///
/// ## Security properties
///
/// - **Deny-all default**: If the allowlist is empty, no env var can
///   be resolved.
/// - **Fail-closed**: An env name not in the allowlist, a missing env
///   var, or a non-env vault reference all produce typed errors.
/// - **No raw leak**: Error messages reference the env *name* but
///   never include the raw value. Raw values are only returned through
///   the resolver API; `Debug`, `Serialize`, and audit paths must not
///   contain them.
/// - **No arbitrary vault**: The helper `extract_env_name` only
///   recognizes the `env` vault; other vault types are rejected.
///
/// ## Example
///
/// ```rust,ignore
/// use std::collections::HashSet;
/// use std::sync::Arc;
/// use plurora_runtime::EnvSecretResolver;
///
/// let resolver = EnvSecretResolver::new(
///     HashSet::from(["MY_API_KEY".to_string()])
/// );
/// // Resolves only if the env var is set AND in the allowlist.
/// ```
pub struct EnvSecretResolver {
    allowed: HashSet<String>,
}

impl EnvSecretResolver {
    /// Create a new `EnvSecretResolver` with the given allowlist of
    /// environment variable names.
    pub fn new(allowed: HashSet<String>) -> Self {
        Self { allowed }
    }

    /// Create a new `EnvSecretResolver` from an iterator of allowed
    /// environment variable names.
    pub fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> Self {
        Self {
            allowed: iter.into_iter().collect(),
        }
    }
}

impl std::fmt::Debug for EnvSecretResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnvSecretResolver")
            .field("allowed_count", &self.allowed.len())
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl HostSecretResolver for EnvSecretResolver {
    async fn resolve(&self, ref_id: &str) -> anyhow::Result<String> {
        let env_name = extract_env_name(ref_id).ok_or_else(|| {
            anyhow::anyhow!(
                "secret resolution denied: not an env-backed reference (ref_id='{}')",
                ref_id
            )
        })?;

        if !self.allowed.contains(env_name) {
            anyhow::bail!(
                "secret resolution denied: env name '{}' not in allowlist (ref_id='{}')",
                env_name,
                ref_id
            );
        }

        match std::env::var(env_name) {
            Ok(value) => Ok(value),
            Err(std::env::VarError::NotPresent) => {
                anyhow::bail!(
                    "secret resolution failed: env var '{}' not set (ref_id='{}')",
                    env_name,
                    ref_id
                );
            }
            Err(std::env::VarError::NotUnicode(_)) => {
                anyhow::bail!(
                    "secret resolution failed: env var '{}' contains non-UTF-8 value (ref_id='{}')",
                    env_name,
                    ref_id
                );
            }
        }
    }
}

/// A host-owned resolver that reads from the encrypted local secret store.
///
/// Resolves `secret_ref:store:NAME` (and prefix variants) by reading the
/// encrypted store file directly. Does not go through the capability fabric;
/// the store file is host-owned state that the resolver reads in-process.
///
/// ## Security properties
///
/// - **Fail-closed**: Missing store file, missing master key, or missing entry
///   all produce typed errors without leaking values.
/// - **Bounded**: Only resolves `store:` references; rejects everything else.
/// - **No raw leak**: Errors mention names but never values.
pub struct StoreSecretResolver {
    store_path: PathBuf,
}

impl StoreSecretResolver {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            store_path: plurora_core::paths::secret_store_path()?,
        })
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self { store_path: path }
    }
}

impl std::fmt::Debug for StoreSecretResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreSecretResolver")
            .field("store_path", &self.store_path)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl HostSecretResolver for StoreSecretResolver {
    async fn resolve(&self, ref_id: &str) -> anyhow::Result<String> {
        let name = plurora_core::secret_ref::extract_store_name(ref_id)
            .ok_or_else(|| anyhow::anyhow!("not a store-backed reference (ref_id='{}')", ref_id))?;

        let (key, _) = crate::secret_store::resolve_master_key()?;
        let store = crate::secret_store::load_store(&self.store_path, &key)?;
        store.secrets.get(name).cloned().ok_or_else(|| {
            anyhow::anyhow!("secret resolution failed: name '{}' not in store", name)
        })
    }
}

/// A resolver that delegates to one of several inner resolvers based on the
/// vault prefix of the secret reference.
///
/// Routing:
/// - `secret_ref:env:` (and variants) → env_resolver
/// - `secret_ref:store:` (and variants) → store_resolver
/// - `secret_ref:project:` → project_resolver
/// - Everything else → DenyAll error
pub struct CompositeSecretResolver {
    env_resolver: Option<Arc<EnvSecretResolver>>,
    store_resolver: Option<Arc<StoreSecretResolver>>,
    project_resolver: Option<Arc<crate::project_secret::ProjectStoreSecretResolver>>,
}

impl CompositeSecretResolver {
    pub fn new() -> Self {
        Self {
            env_resolver: None,
            store_resolver: None,
            project_resolver: None,
        }
    }

    pub fn with_env(mut self, r: Arc<EnvSecretResolver>) -> Self {
        self.env_resolver = Some(r);
        self
    }

    pub fn with_store(mut self, r: Arc<StoreSecretResolver>) -> Self {
        self.store_resolver = Some(r);
        self
    }

    pub fn with_project(
        mut self,
        r: Arc<crate::project_secret::ProjectStoreSecretResolver>,
    ) -> Self {
        self.project_resolver = Some(r);
        self
    }
}

impl Default for CompositeSecretResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HostSecretResolver for CompositeSecretResolver {
    async fn resolve(&self, ref_id: &str) -> anyhow::Result<String> {
        if plurora_core::secret_ref::is_env_backed_ref(ref_id) {
            return match &self.env_resolver {
                Some(r) => r.resolve(ref_id).await,
                None => anyhow::bail!("env resolver not configured (ref_id='{}')", ref_id),
            };
        }
        if plurora_core::secret_ref::is_store_backed_ref(ref_id) {
            return match &self.store_resolver {
                Some(r) => r.resolve(ref_id).await,
                None => anyhow::bail!("store resolver not configured (ref_id='{}')", ref_id),
            };
        }
        if plurora_core::secret_ref::is_project_backed_ref(ref_id) {
            return match &self.project_resolver {
                Some(r) => r.resolve(ref_id).await,
                None => anyhow::bail!("project resolver not configured (ref_id='{}')", ref_id),
            };
        }
        anyhow::bail!("unsupported secret reference scheme (ref_id='{}')", ref_id);
    }
}

/// Extract the environment variable name from a supported secret
/// reference format.
///
/// Supported formats:
/// - `secret_ref:env:NAME`
/// - `secretRef:env:NAME`
/// - `secret-ref:env:NAME`
/// - `host:env:NAME`
///
/// Returns `None` for:
/// - References with a non-`env` vault (e.g. `secret_ref:vault:key`).
/// - Bare `host:<key>` references that don't start with `env:`.
/// - Malformed or unrecognized references.
pub fn extract_env_name(ref_id: &str) -> Option<&str> {
    // Canonical: secret_ref:env:NAME
    if let Some(rest) = ref_id.strip_prefix("secret_ref:") {
        if let Some(name) = rest.strip_prefix("env:") {
            if !name.is_empty() {
                return Some(name);
            }
        }
        return None;
    }
    // camelCase: secretRef:env:NAME
    if let Some(rest) = ref_id.strip_prefix("secretRef:") {
        if let Some(name) = rest.strip_prefix("env:") {
            if !name.is_empty() {
                return Some(name);
            }
        }
        return None;
    }
    // kebab-case: secret-ref:env:NAME
    if let Some(rest) = ref_id.strip_prefix("secret-ref:") {
        if let Some(name) = rest.strip_prefix("env:") {
            if !name.is_empty() {
                return Some(name);
            }
        }
        return None;
    }
    // host:env:NAME (but NOT host:<other-key>)
    if let Some(rest) = ref_id.strip_prefix("host:") {
        if let Some(name) = rest.strip_prefix("env:") {
            if !name.is_empty() {
                return Some(name);
            }
        }
        // host:something_else → not an env reference
        return None;
    }
    None
}

/// Configuration for the host secret resolver.
#[derive(Clone)]
pub struct SecretResolverConfig {
    /// The resolver implementation. Defaults to `DenyAllSecretResolver`.
    pub resolver: std::sync::Arc<dyn HostSecretResolver>,
}

impl Default for SecretResolverConfig {
    fn default() -> Self {
        Self {
            resolver: std::sync::Arc::new(DenyAllSecretResolver),
        }
    }
}

impl SecretResolverConfig {
    /// Create a config with a custom resolver.
    pub fn with_resolver(resolver: std::sync::Arc<dyn HostSecretResolver>) -> Self {
        Self { resolver }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deny_all_resolver_rejects_resolution() {
        let resolver = DenyAllSecretResolver;
        let result = resolver.resolve("secret_ref:env:MY_KEY").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("no secret resolver configured"));
    }

    // --- EnvSecretResolver tests ---

    /// Generate a unique env var name using the process ID to avoid
    /// cross-test pollution.
    fn unique_env_name(suffix: &str) -> String {
        format!("PLURORA_TEST_ENV_{}_{}", std::process::id(), suffix)
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

    #[tokio::test]
    async fn env_resolver_allowed_resolves() {
        let name = unique_env_name("ALLOWED");
        let _guard = EnvVarGuard::set(&name, "test-value-not-a-provider-key");

        let resolver = EnvSecretResolver::new(HashSet::from([name.clone()]));
        let result = resolver.resolve(&format!("secret_ref:env:{}", name)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-value-not-a-provider-key");
    }

    #[tokio::test]
    async fn env_resolver_allowed_secretref_prefix() {
        let name = unique_env_name("CAMEL");
        let _guard = EnvVarGuard::set(&name, "camel-value");

        let resolver = EnvSecretResolver::new(HashSet::from([name.clone()]));
        let result = resolver.resolve(&format!("secretRef:env:{}", name)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "camel-value");
    }

    #[tokio::test]
    async fn env_resolver_allowed_secret_ref_kebab_prefix() {
        let name = unique_env_name("KEBAB");
        let _guard = EnvVarGuard::set(&name, "kebab-value");

        let resolver = EnvSecretResolver::new(HashSet::from([name.clone()]));
        let result = resolver.resolve(&format!("secret-ref:env:{}", name)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "kebab-value");
    }

    #[tokio::test]
    async fn env_resolver_allowed_host_env_prefix() {
        let name = unique_env_name("HOSTENV");
        let _guard = EnvVarGuard::set(&name, "host-env-value");

        let resolver = EnvSecretResolver::new(HashSet::from([name.clone()]));
        let result = resolver.resolve(&format!("host:env:{}", name)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "host-env-value");
    }

    #[tokio::test]
    async fn env_resolver_denied_not_in_allowlist() {
        let name = unique_env_name("DENIED");
        let _guard = EnvVarGuard::set(&name, "should-not-be-returned");

        // Allowlist does NOT include the env name
        let resolver = EnvSecretResolver::new(HashSet::new());
        let result = resolver.resolve(&format!("secret_ref:env:{}", name)).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not in allowlist"),
            "error should mention allowlist: {err_msg}"
        );
        assert!(
            !err_msg.contains("should-not-be-returned"),
            "error must not leak raw value: {err_msg}"
        );
    }

    #[tokio::test]
    async fn env_resolver_missing_env_rejected() {
        let name = unique_env_name("MISSING");
        // Ensure the env var is NOT set
        std::env::remove_var(&name);

        let resolver = EnvSecretResolver::new(HashSet::from([name.clone()]));
        let result = resolver.resolve(&format!("secret_ref:env:{}", name)).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not set"),
            "error should mention not set: {err_msg}"
        );
        // Error must not contain raw value (there is none, but verify structure)
        assert!(
            err_msg.contains(&name),
            "error should contain env name for debugging: {err_msg}"
        );
    }

    #[tokio::test]
    async fn env_resolver_non_env_vault_rejected() {
        let resolver = EnvSecretResolver::new(HashSet::from(["SOME_KEY".to_string()]));
        let result = resolver.resolve("secret_ref:vault:prod/openai").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not an env-backed reference"),
            "error should mention non-env: {err_msg}"
        );
    }

    #[tokio::test]
    async fn env_resolver_host_non_env_rejected() {
        // host:<key> where key doesn't start with env: should not be resolved
        let resolver = EnvSecretResolver::new(HashSet::from(["my_secret".to_string()]));
        let result = resolver.resolve("host:my_secret").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not an env-backed reference"),
            "host:my_secret should not be treated as env: {err_msg}"
        );
    }

    #[tokio::test]
    async fn env_resolver_error_does_not_leak_raw_value() {
        let name = unique_env_name("NOLEAK");
        let _guard = EnvVarGuard::set(&name, "super-secret-value-xyz");

        // Deny the env name so we get an error
        let resolver = EnvSecretResolver::new(HashSet::new());
        let result = resolver.resolve(&format!("secret_ref:env:{}", name)).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            !err_msg.contains("super-secret-value-xyz"),
            "error must not leak raw value: {err_msg}"
        );
    }

    #[tokio::test]
    async fn env_resolver_empty_allowlist_denies_all() {
        let name = unique_env_name("EMPTY");
        let _guard = EnvVarGuard::set(&name, "value");

        let resolver = EnvSecretResolver::new(HashSet::new());
        let result = resolver.resolve(&format!("secret_ref:env:{}", name)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn env_resolver_debug_does_not_leak_values() {
        let resolver = EnvSecretResolver::new(HashSet::from(["MY_KEY".to_string()]));
        let debug_str = format!("{:?}", resolver);
        assert!(
            !debug_str.contains("MY_KEY"),
            "Debug should not contain env names: {debug_str}"
        );
        assert!(
            debug_str.contains("allowed_count"),
            "Debug should show count: {debug_str}"
        );
    }

    // --- extract_env_name tests ---

    #[test]
    fn extract_env_name_canonical() {
        assert_eq!(extract_env_name("secret_ref:env:MY_KEY"), Some("MY_KEY"));
    }

    #[test]
    fn extract_env_name_camel_case() {
        assert_eq!(extract_env_name("secretRef:env:MY_KEY"), Some("MY_KEY"));
    }

    #[test]
    fn extract_env_name_kebab_case() {
        assert_eq!(extract_env_name("secret-ref:env:MY_KEY"), Some("MY_KEY"));
    }

    #[test]
    fn extract_env_name_host_env() {
        assert_eq!(extract_env_name("host:env:MY_KEY"), Some("MY_KEY"));
    }

    #[test]
    fn extract_env_name_non_env_vault() {
        assert_eq!(extract_env_name("secret_ref:vault:prod/key"), None);
    }

    #[test]
    fn extract_env_name_host_non_env() {
        assert_eq!(extract_env_name("host:my_secret"), None);
    }

    #[test]
    fn extract_env_name_empty_name() {
        assert_eq!(extract_env_name("secret_ref:env:"), None);
        assert_eq!(extract_env_name("host:env:"), None);
    }

    #[test]
    fn extract_env_name_unrecognized() {
        assert_eq!(extract_env_name("not_a_ref"), None);
        assert_eq!(extract_env_name(""), None);
    }

    #[tokio::test]
    async fn composite_routes_env_refs() {
        let name = unique_env_name("COMPOSITE");
        let _guard = EnvVarGuard::set(&name, "composite-value");
        let resolver = CompositeSecretResolver::new().with_env(Arc::new(EnvSecretResolver::new(
            HashSet::from([name.clone()]),
        )));

        let result = resolver.resolve(&format!("secret_ref:env:{name}")).await;
        assert_eq!(result.unwrap(), "composite-value");
    }

    #[tokio::test]
    async fn composite_rejects_unconfigured_store_refs() {
        let resolver = CompositeSecretResolver::new();
        let result = resolver.resolve("secret_ref:store:MY_KEY").await;
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("store resolver not configured"));
    }
}
