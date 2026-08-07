use std::collections::HashMap;

use plurora_core::{
    CapHandle, CapHandleId, ContractMode, HandleLease, HandleProvenance, HandleScope, PackageEntry,
    PackageId, PackageManifest, RedactionState, EVENT_PACKAGE_DEGRADED, EVENT_PACKAGE_LOADED,
    EVENT_PACKAGE_LOADING, EVENT_PACKAGE_LOG, EVENT_PACKAGE_READY, EVENT_PACKAGE_STARTING,
    EVENT_PACKAGE_STOPPED, EVENT_PACKAGE_STOPPING, EVENT_PACKAGE_UNLOADED, KERNEL_PACKAGE_ID,
};
use serde_json::{json, Value};

use super::Runtime;
use crate::{EventStore, PackageFailureSummary, PackageRecord, PackageState};

const FAILURE_STDERR_TAIL_LIMIT: usize = 8;
const FAILURE_LOG_TAIL_LIMIT: usize = 20;

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub(crate) async fn is_contract_none_package(&self, package_id: &PackageId) -> bool {
        self.packages
            .manifest(package_id)
            .await
            .map(|manifest| manifest.entry.contract == ContractMode::None)
            .unwrap_or(false)
    }

    pub async fn load_package(&self, manifest: PackageManifest) -> anyhow::Result<PackageRecord> {
        manifest.validate_basic()?;
        if let PackageEntry::RustInproc {
            crate_ref, symbol, ..
        } = &manifest.entry.kind
        {
            if manifest.entry.contract == ContractMode::V1
                && !manifest.provides.is_empty()
                && self
                    .config
                    .inproc_packages
                    .lookup(crate_ref, symbol)
                    .is_none()
            {
                anyhow::bail!(
                    "rust_inproc entry '{}::{}' is not available in this host",
                    crate_ref,
                    symbol
                );
            }
        }
        let mut record = self
            .packages
            .load(manifest, &self.config.host_policy)
            .await?;
        record = self
            .packages
            .set_state(&record.id, PackageState::Loading)
            .await
            .unwrap_or(record);
        self.append_package_lifecycle_event(&record, EVENT_PACKAGE_LOADING, None)
            .await?;
        let is_surface_bundle = matches!(
            record.manifest.entry.kind,
            PackageEntry::SurfaceBundle { .. }
        );
        let participates_in_contract =
            !is_surface_bundle && record.manifest.entry.contract == ContractMode::V1;
        if participates_in_contract {
            let component = record
                .components
                .first()
                .ok_or_else(|| anyhow::anyhow!("package '{}' has no component", record.id))?;
            self.capabilities
                .register_component(&record.id, component, &record.manifest.provides)
                .await?;
            self.extensions
                .register_package(&record.id, &record.manifest.contributes.hooks)
                .await;
        }
        let bindings = if participates_in_contract {
            self.mint_package_bindings(&record.manifest).await
        } else {
            HashMap::new()
        };
        match &record.manifest.entry.kind {
            PackageEntry::Subprocess { .. } => {
                record = self
                    .packages
                    .set_state(&record.id, PackageState::Starting)
                    .await
                    .unwrap_or(record);
                self.append_package_lifecycle_event(&record, EVENT_PACKAGE_STARTING, None)
                    .await?;
                if let Err(error) = self
                    .subprocesses
                    .start(&record.manifest, (*self).clone(), bindings)
                    .await
                {
                    let degraded = self
                        .packages
                        .set_state(&record.id, PackageState::Degraded)
                        .await
                        .unwrap_or_else(|| record.clone());
                    self.capabilities.unregister_package(&record.id).await;
                    self.extensions.unregister_package(&record.id).await;
                    self.append_package_degraded_event(&degraded, &error.to_string())
                        .await?;
                    return Err(error);
                }
                record = self
                    .packages
                    .set_state(&record.id, PackageState::Ready)
                    .await
                    .unwrap_or(record);
            }
            PackageEntry::RustInproc {
                crate_ref, symbol, ..
            } => {
                if let Some(package) = self.config.inproc_packages.lookup(crate_ref, symbol) {
                    let env = crate::KernelEnv {
                        package_id: record.id.clone(),
                        component_id: record.components[0].component_id.clone(),
                        component_digest: record.components[0].artifact.digest.clone(),
                        bindings,
                        handles: self.handles.clone(),
                    };
                    package.init(&env);
                }
            }
            PackageEntry::Wasm { .. } => {
                if let Err(error) = super::wasm::load_wasm_placeholder() {
                    let degraded = self
                        .packages
                        .set_state(&record.id, PackageState::Degraded)
                        .await
                        .unwrap_or_else(|| record.clone());
                    self.append_package_degraded_event(&degraded, &error.to_string())
                        .await?;
                    return Err(error);
                }
            }
            PackageEntry::Remote { .. } => {
                if let Err(error) = super::remote::load_remote_placeholder() {
                    let degraded = self
                        .packages
                        .set_state(&record.id, PackageState::Degraded)
                        .await
                        .unwrap_or_else(|| record.clone());
                    self.append_package_degraded_event(&degraded, &error.to_string())
                        .await?;
                    return Err(error);
                }
            }
            PackageEntry::SurfaceBundle { .. } => {
                // Static surface bundles are not executable entrypoints. The
                // manifest contributes surfaces and assets; no runtime process,
                // wasm module, or remote endpoint is started.
            }
        }
        if !matches!(record.state, PackageState::Ready) {
            record = self
                .packages
                .set_state(&record.id, PackageState::Ready)
                .await
                .unwrap_or(record);
        }
        self.append_package_lifecycle_event(&record, EVENT_PACKAGE_READY, None)
            .await?;
        self.append_package_lifecycle_event(&record, EVENT_PACKAGE_LOADED, None)
            .await?;
        Ok(record)
    }

    pub(crate) async fn mint_package_bindings(
        &self,
        manifest: &PackageManifest,
    ) -> HashMap<String, CapHandleId> {
        if manifest.entry.contract == ContractMode::None {
            return HashMap::new();
        }
        let mut bindings = HashMap::new();
        for cap_id in &manifest.permissions.capabilities.invoke {
            let handle_id = self
                .handles
                .mint(package_load_handle(
                    manifest.id.clone(),
                    cap_id.clone(),
                    "1".to_string(),
                    json!({}),
                ))
                .await;
            bindings.insert(logical_binding_name(cap_id), handle_id);
        }

        for declaration in &manifest.permissions.network.declarations {
            let handle_id = self
                .handles
                .mint(package_load_handle(
                    manifest.id.clone(),
                    "kernel.outbound.execute".to_string(),
                    "1".to_string(),
                    json!({
                        "host": declaration.host,
                        "methods": declaration.methods,
                    }),
                ))
                .await;
            bindings.insert(network_binding_name(&declaration.host), handle_id);
        }

        for secret_ref in &manifest.permissions.secret_refs {
            let handle_id = self
                .handles
                .mint(package_load_handle(
                    manifest.id.clone(),
                    "kernel.secret.reveal".to_string(),
                    "1".to_string(),
                    json!({ "secret_ref": secret_ref }),
                ))
                .await;
            bindings.insert(secret_binding_name(secret_ref), handle_id);
        }

        bindings
    }

    pub async fn unload_package(&self, package_id: &PackageId) -> anyhow::Result<PackageRecord> {
        if let Some(stopping) = self
            .packages
            .set_state(package_id, PackageState::Stopping)
            .await
        {
            self.append_package_lifecycle_event(&stopping, EVENT_PACKAGE_STOPPING, None)
                .await?;
        }
        self.subprocesses.stop(package_id).await;
        if let Some(stopped) = self
            .packages
            .set_state(package_id, PackageState::Stopped)
            .await
        {
            self.append_package_lifecycle_event(&stopped, EVENT_PACKAGE_STOPPED, None)
                .await?;
        }
        let record = self.packages.unload(package_id).await?;
        self.capabilities.unregister_package(package_id).await;
        self.extensions.unregister_package(package_id).await;
        self.append_package_lifecycle_event(&record, EVENT_PACKAGE_UNLOADED, None)
            .await?;
        Ok(record)
    }

    pub async fn restart_package(&self, package_id: &PackageId) -> anyhow::Result<PackageRecord> {
        let record = self
            .package_status(package_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("package '{package_id}' is not loaded"))?;
        if !matches!(record.manifest.entry.kind, PackageEntry::Subprocess { .. }) {
            anyhow::bail!(
                "package '{package_id}' entry kind '{}' cannot restart yet",
                record.entry_kind
            );
        }
        if let Some(stopping) = self
            .packages
            .set_state(package_id, PackageState::Stopping)
            .await
        {
            self.append_package_lifecycle_event(&stopping, EVENT_PACKAGE_STOPPING, Some("restart"))
                .await?;
        }
        let bindings = self.mint_package_bindings(&record.manifest).await;
        self.subprocesses
            .restart(&record.manifest, (*self).clone(), bindings)
            .await?;
        let ready = self
            .packages
            .set_state(package_id, PackageState::Ready)
            .await
            .ok_or_else(|| anyhow::anyhow!("package '{package_id}' disappeared during restart"))?;
        self.append_package_lifecycle_event(&ready, EVENT_PACKAGE_READY, Some("restart"))
            .await?;
        Ok(ready)
    }

    pub async fn package_logs(&self, package_id: &PackageId) -> Vec<crate::SubprocessLogLine> {
        let logs = self
            .subprocesses
            .drain_logs(package_id)
            .await
            .into_iter()
            .map(redact_log_line)
            .collect::<Vec<_>>();
        for log in &logs {
            let _ = self
                .append_package_log_event(package_id, &log.stream, &log.line)
                .await;
        }
        logs
    }

    pub async fn host_diagnostics(&self) -> Value {
        let (packages, capabilities, hooks, execs, targets, port_leases, proxy_routes) = tokio::join!(
            self.list_packages(),
            self.discover_capabilities(),
            self.extensions.list_all_hooks(),
            self.config.exec_registry.list(),
            self.config.target_registry.list(),
            self.config.port_lease_registry.list(),
            self.config.proxy_route_registry.list(),
        );
        json!({
            "host_version": env!("CARGO_PKG_VERSION"),
            "package_count": packages.len(),
            "capability_provider_count": capabilities.len(),
            "hook_subscription_count": hooks.len(),
            "runtime": {
                "execution_count": execs.len(),
                "target_count": targets.len(),
                "port_lease_count": port_leases.len(),
                "proxy_route_count": proxy_routes.len(),
                "ready_proxy_route_count": proxy_routes.iter().filter(|route| route.ready).count(),
            },
            "packages": packages,
        })
    }

    pub async fn list_surface_contributions(&self, slot: Option<String>) -> Value {
        let packages = self.list_packages().await;
        let mut contributions = Vec::new();
        for package in packages {
            for contribution in &package.manifest.contributes.surfaces {
                let slot_name = serde_json::to_value(&contribution.slot)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_string))
                    .unwrap_or_else(|| "unknown".to_string());
                if slot.as_ref().map(|slot| slot == &slot_name).unwrap_or(true) {
                    contributions.push(json!({
                        "package_id": package.id,
                        "entry_kind": package.entry_kind,
                        "package_state": package.state,
                        "surface": contribution,
                    }));
                }
            }
        }
        json!(contributions)
    }

    pub async fn describe_surface_contribution(&self, surface_id: &str) -> anyhow::Result<Value> {
        let packages = self.list_packages().await;
        for package in packages {
            for contribution in &package.manifest.contributes.surfaces {
                if contribution.id == surface_id {
                    return Ok(json!({
                        "package_id": package.id,
                        "entry_kind": package.entry_kind,
                        "package_state": package.state,
                        "surface": contribution,
                    }));
                }
            }
        }
        anyhow::bail!("surface contribution '{surface_id}' not found")
    }

    pub async fn list_packages(&self) -> Vec<PackageRecord> {
        self.packages.list().await
    }

    pub async fn package_status(&self, package_id: &PackageId) -> Option<PackageRecord> {
        self.packages.status(package_id).await
    }

    pub(crate) async fn append_package_degraded_event(
        &self,
        record: &PackageRecord,
        reason: &str,
    ) -> anyhow::Result<plurora_core::EventEnvelope> {
        let drained_logs = self.subprocesses.drain_logs(&record.id).await;
        let raw_log_count = drained_logs.len();
        let log_tail = tail_vec(
            drained_logs
                .into_iter()
                .map(redact_log_line)
                .collect::<Vec<_>>(),
            FAILURE_LOG_TAIL_LIMIT,
        );
        let stderr_tail_redacted = tail_vec(
            log_tail
                .iter()
                .filter(|log| log.stream == "stderr")
                .map(|log| log.line.clone())
                .collect::<Vec<_>>(),
            FAILURE_STDERR_TAIL_LIMIT,
        );
        for log in &log_tail {
            let _ = self
                .append_package_log_event(&record.id, &log.stream, &log.line)
                .await;
        }
        let failed_at = chrono::Utc::now();
        let failure = PackageFailureSummary {
            package_id: record.id.clone(),
            reason: redact_line(reason),
            exit_code: None,
            signal: None,
            failed_at,
            stderr_tail_redacted,
            log_tail_redacted: log_tail,
            stderr_truncated: raw_log_count > FAILURE_LOG_TAIL_LIMIT,
            redaction_state: RedactionState::Redacted,
            state: record.state.clone(),
        };
        let updated = self
            .packages
            .set_last_failure(&record.id, failure.clone())
            .await
            .unwrap_or_else(|| {
                let mut fallback = record.clone();
                fallback.last_failure = Some(failure.clone());
                fallback
            });
        self.append_package_lifecycle_event(&updated, EVENT_PACKAGE_DEGRADED, Some(reason))
            .await
    }

    pub(crate) async fn append_package_lifecycle_event(
        &self,
        record: &PackageRecord,
        kind: &'static str,
        reason: Option<&str>,
    ) -> anyhow::Result<plurora_core::EventEnvelope> {
        let session_id = format!("kernel_package_{}", record.id.replace('/', "_"));
        let mut payload = json!({
            "package_id": record.id,
            "version": record.version,
            "state": record.state,
            "entry_kind": record.entry_kind,
            "package_envelope_digest": record.package_envelope.as_ref().map(|envelope| &envelope.artifact.digest),
            "components": record.components.iter().map(|component| json!({
                "component_id": component.component_id,
                "component_digest": component.artifact.digest,
                "behavior_digest": component.behavior.digest,
                "trust_class": component.trust_class,
                "claim_status": component.claim_status,
                "enforced_boundaries": component.enforced_boundaries,
            })).collect::<Vec<_>>(),
            "contract_mode": match record.manifest.entry.contract {
                ContractMode::V1 => "v1",
                ContractMode::None => "none",
            },
            "capability_count": record.capability_count,
            "hook_count": record.hook_count,
            "extension_point_count": record.extension_point_count,
        });
        if let Some(reason) = reason {
            payload["reason"] = json!(redact_line(reason));
        }
        if let Some(last_failure) = &record.last_failure {
            payload["last_failure"] = serde_json::to_value(last_failure)?;
            payload["stderr_tail_redacted"] = json!(last_failure.stderr_tail_redacted);
            payload["log_tail_redacted"] = json!(last_failure.log_tail_redacted);
            payload["redaction_state"] = json!(last_failure.redaction_state);
        }
        self.append_kernel_event(&session_id, kind, payload).await
    }

    pub(crate) async fn append_package_log_event(
        &self,
        package_id: &PackageId,
        stream: &str,
        line: &str,
    ) -> anyhow::Result<plurora_core::EventEnvelope> {
        let session_id = format!("kernel_package_{}", package_id.replace('/', "_"));
        self.append_kernel_event(
            &session_id,
            EVENT_PACKAGE_LOG,
            json!({
                "package_id": package_id,
                "stream": stream,
                "line": redact_line(line),
                "redaction_state": RedactionState::Redacted,
            }),
        )
        .await
    }
}

fn redact_log_line(mut log: crate::SubprocessLogLine) -> crate::SubprocessLogLine {
    log.line = redact_line(&log.line);
    log
}

fn redact_line(line: &str) -> String {
    let mut out = Vec::new();
    let mut redact_next = false;
    for token in line.split_whitespace() {
        let stripped = token
            .trim_matches(|c: char| matches!(c, ',' | ';' | ')' | '(' | '[' | ']' | '"' | '\''));
        let lower = stripped.to_ascii_lowercase();
        let redacted = redact_next
            || lower.starts_with("sk-")
            || lower.starts_with("rk_")
            || lower.starts_with("xai-")
            || lower.starts_with("ghp_")
            || lower.starts_with("github_pat_")
            || lower.starts_with("secret_ref:")
            || lower.contains("authorization:")
            || lower.contains("api_key=")
            || lower.contains("apikey=")
            || lower.contains("bearer");
        out.push(if redacted { "<secret:redacted>" } else { token });
        redact_next = lower == "bearer" || lower.ends_with("authorization:");
    }
    out.join(" ")
}

fn tail_vec<T>(items: Vec<T>, limit: usize) -> Vec<T> {
    let len = items.len();
    items.into_iter().skip(len.saturating_sub(limit)).collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use plurora_core::{
        EntryDescriptor, PackageContributions, PackageEntry, PermissionSet, SandboxPolicy,
        EVENT_PACKAGE_LOADED, EVENT_PACKAGE_LOADING, EVENT_PACKAGE_READY,
    };
    use serde_json::Value;

    use super::*;
    use crate::{InMemoryEventStore, RuntimeConfig};

    #[tokio::test]
    async fn package_load_records_kernel_lifecycle_event() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(store.clone(), RuntimeConfig::default());

        let record = runtime
            .load_package(plurora_core::PackageManifest {
                schema_version: 1,
                id: "org/pkg".to_string(),
                version: "0.1.0".to_string(),
                display_name: None,
                description: None,
                author: None,
                license: None,
                entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                    crate_ref: "org-pkg".to_string(),
                    symbol: "register".to_string(),
                    abi_version: 1,
                }),
                provides: Vec::new(),
                consumes: Vec::new(),
                requires: Vec::new(),
                contributes: PackageContributions::default(),
                permissions: PermissionSet::default(),
                sandbox_policy: SandboxPolicy::default(),
            })
            .await?;

        assert_eq!(record.id, "org/pkg");
        let events = store
            .list_session(&"kernel_package_org_pkg".to_string())
            .await?;
        assert!(events
            .iter()
            .any(|event| event.kind == EVENT_PACKAGE_LOADING));
        assert!(events.iter().any(|event| event.kind == EVENT_PACKAGE_READY));
        assert!(events
            .iter()
            .any(|event| event.kind == EVENT_PACKAGE_LOADED));
        Ok(())
    }

    #[tokio::test]
    async fn rust_inproc_provider_must_exist_in_host_catalog() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(store, RuntimeConfig::default());

        let result = runtime
            .load_package(plurora_core::PackageManifest {
                schema_version: 1,
                id: "example/missing".to_string(),
                version: "0.1.0".to_string(),
                display_name: None,
                description: None,
                author: None,
                license: None,
                entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                    crate_ref: "missing-crate".to_string(),
                    symbol: "register".to_string(),
                    abi_version: 1,
                }),
                provides: vec![plurora_core::CapabilityDescriptor {
                    id: "example/missing/echo".to_string(),
                    version: "0.1.0".to_string(),
                    input_schema: Value::Null,
                    output_schema: Value::Null,
                    streaming: false,
                    side_effects: Vec::new(),
                    description: None,
                }],
                consumes: Vec::new(),
                requires: Vec::new(),
                contributes: PackageContributions::default(),
                permissions: PermissionSet::default(),
                sandbox_policy: SandboxPolicy::default(),
            })
            .await;

        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn surface_bundle_loads_ready_without_executable_placeholder() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(store.clone(), RuntimeConfig::default());

        let record = runtime
            .load_package(plurora_core::PackageManifest {
                schema_version: 1,
                id: "example/surface".to_string(),
                version: "0.1.0".to_string(),
                display_name: None,
                description: None,
                author: None,
                license: None,
                entry: EntryDescriptor::v1(PackageEntry::SurfaceBundle {
                    bundle: "dist/bundle.mjs".to_string(),
                }),
                provides: Vec::new(),
                consumes: Vec::new(),
                requires: Vec::new(),
                contributes: PackageContributions::default(),
                permissions: PermissionSet::default(),
                sandbox_policy: SandboxPolicy::default(),
            })
            .await?;

        assert_eq!(record.state, PackageState::Ready);
        assert_eq!(record.entry_kind, "surface_bundle");
        assert_eq!(record.trust_level, crate::TrustLevel::StaticSurface);
        let events = store
            .list_session(&"kernel_package_example_surface".to_string())
            .await?;
        assert!(events.iter().any(|event| event.kind == EVENT_PACKAGE_READY));
        assert!(!events
            .iter()
            .any(|event| event.kind == EVENT_PACKAGE_DEGRADED));
        Ok(())
    }

    #[tokio::test]
    async fn subprocess_uses_configured_package_root_as_working_directory() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let script = tmp.path().join("server.py");
        std::fs::write(
            &script,
            r#"import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    if msg.get("method") == "package.handshake":
        print(json.dumps({"jsonrpc":"2.0","id":msg.get("id"),"result":{"ready":True}}), flush=True)
    elif msg.get("method") == "capability.invoke":
        print(json.dumps({"jsonrpc":"2.0","id":msg.get("id"),"result":{"output":{"ok":True}}}), flush=True)
"#,
        )?;

        let store = Arc::new(InMemoryEventStore::default());
        let mut config = RuntimeConfig::default();
        config
            .package_roots
            .insert("example/cwd".to_string(), tmp.path().to_path_buf());
        let runtime = Runtime::new(store, config);
        let python = std::env::var("PLURORA_TEST_PYTHON").unwrap_or_else(|_| {
            if cfg!(windows) {
                "python".to_string()
            } else {
                "python3".to_string()
            }
        });

        let record = runtime
            .load_package(plurora_core::PackageManifest {
                schema_version: 1,
                id: "example/cwd".to_string(),
                version: "0.1.0".to_string(),
                display_name: None,
                description: None,
                author: None,
                license: None,
                entry: EntryDescriptor::v1(PackageEntry::Subprocess {
                    command: vec![python, "server.py".to_string()],
                    transport: plurora_core::SubprocessTransport::JsonRpcStdio,
                }),
                provides: Vec::new(),
                consumes: Vec::new(),
                requires: Vec::new(),
                contributes: PackageContributions::default(),
                permissions: PermissionSet::default(),
                sandbox_policy: SandboxPolicy::default(),
            })
            .await?;

        assert_eq!(record.state, PackageState::Ready);
        runtime.unload_package(&"example/cwd".to_string()).await?;
        Ok(())
    }

    #[tokio::test]
    async fn degraded_package_records_bounded_failure_summary() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(store.clone(), RuntimeConfig::default());
        let package_id = "example/failing".to_string();

        runtime
            .load_package(plurora_core::PackageManifest {
                schema_version: 1,
                id: package_id.clone(),
                version: "0.1.0".to_string(),
                display_name: None,
                description: None,
                author: None,
                license: None,
                entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                    crate_ref: "example-echo-rust-inproc".to_string(),
                    symbol: "register".to_string(),
                    abi_version: 1,
                }),
                provides: Vec::new(),
                consumes: Vec::new(),
                requires: Vec::new(),
                contributes: PackageContributions::default(),
                permissions: PermissionSet::default(),
                sandbox_policy: SandboxPolicy::default(),
            })
            .await?;

        let degraded = runtime
            .packages
            .set_state(&package_id, PackageState::Degraded)
            .await
            .expect("package should exist");
        runtime
            .append_package_degraded_event(&degraded, "startup failed")
            .await?;

        let status = runtime
            .package_status(&package_id)
            .await
            .expect("package status should exist");
        let failure = status
            .last_failure
            .expect("degraded package should record failure summary");
        assert_eq!(failure.package_id, package_id);
        assert_eq!(failure.reason, "startup failed");
        assert!(failure.exit_code.is_none());
        assert!(failure.signal.is_none());
        assert!(failure.stderr_tail_redacted.len() <= FAILURE_STDERR_TAIL_LIMIT);
        assert!(failure.log_tail_redacted.len() <= FAILURE_LOG_TAIL_LIMIT);
        assert_eq!(
            failure.redaction_state,
            plurora_core::RedactionState::Redacted
        );

        let events = store
            .list_session(&"kernel_package_example_failing".to_string())
            .await?;
        let event = events
            .iter()
            .find(|event| event.kind == EVENT_PACKAGE_DEGRADED)
            .expect("degraded event should be recorded");
        assert_eq!(event.payload["reason"], json!("startup failed"));
        assert!(event.payload.get("last_failure").is_some());
        assert!(event.payload.get("stderr_tail_redacted").is_some());
        assert!(event.payload.get("redaction_state").is_some());
        Ok(())
    }

    #[test]
    fn package_failure_redacts_secret_like_log_lines() {
        let line = redact_line(
            "Authorization: Bearer sk-test-redacted-example api_key=sk-also-redacted normal",
        );
        assert!(!line.contains("sk-test-redacted-example"));
        assert!(!line.contains("sk-also-redacted"));
        assert!(line.contains("normal"));
        assert!(line.contains("<secret:redacted>"));
    }
}

fn package_load_handle(
    holder_package_id: PackageId,
    cap_type: String,
    cap_version: String,
    constraints: Value,
) -> CapHandle {
    CapHandle {
        id: CapHandleId::new(),
        cap_type,
        cap_version,
        scope: HandleScope {
            holder_package_id,
            session_id: None,
        },
        constraints,
        lease: HandleLease::default(),
        provenance: HandleProvenance {
            granted_at: chrono::Utc::now(),
            granted_by_package_id: KERNEL_PACKAGE_ID.to_string(),
            via_method: "package_load".to_string(),
        },
        parent: None,
        revoked: false,
    }
}

fn logical_binding_name(id: &str) -> String {
    camel_binding_name(id, "binding")
}

fn network_binding_name(host: &str) -> String {
    format!("network{}", pascal_binding_name(host))
}

fn secret_binding_name(secret_ref: &str) -> String {
    let tail = secret_ref.rsplit(':').next().unwrap_or(secret_ref);
    format!("secret{}", pascal_binding_name(tail))
}

fn pascal_binding_name(input: &str) -> String {
    let camel = camel_binding_name(input, "value");
    let mut chars = camel.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn camel_binding_name(input: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if out.is_empty() {
                out.push(ch.to_ascii_lowercase());
            } else if uppercase_next {
                out.push(ch.to_ascii_uppercase());
            } else {
                out.push(ch.to_ascii_lowercase());
            }
            uppercase_next = false;
        } else {
            uppercase_next = !out.is_empty();
        }
    }
    if out.is_empty() {
        fallback.to_string()
    } else {
        out
    }
}
