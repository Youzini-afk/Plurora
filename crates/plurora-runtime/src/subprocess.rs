//! JSON-RPC-over-stdio subprocess supervisor.
//!
//! In addition to host-initiated `package.handshake` and
//! `capability.invoke` calls, subprocess packages may initiate reverse
//! public platform calls by writing JSON-RPC requests to stdout. The
//! supervisor accepts only exact registered method IDs, dispatches them with the
//! caller principal locked to the subprocess package id and writes responses
//! back to the child's stdin.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use anyhow::Context;
use plurora_core::{
    package_envelope_for_manifest, CapHandleId, ContractMode, PackageEntry, PackageId,
    PackageManifest, PermissionSet, SubprocessTransport,
};
use schemars::JsonSchema;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::{oneshot, Mutex, RwLock};
use tokio::time::timeout;

use crate::{
    resolve_contract_method, EventStore, PackageRecord, PackageRegistry, PackageState,
    PlatformMethod, ProtocolContext, ProtocolError, RunControl, Runtime,
};

pub struct SubprocessSupervisor {
    handles: RwLock<HashMap<PackageId, Arc<SubprocessHandle>>>,
    lifecycle_transition: Mutex<()>,
    pending_activation_losses: Mutex<HashMap<(PackageId, u64), PendingActivationLoss>>,
    loss_context: ActivationLossContext,
    next_generation: AtomicU64,
}

pub struct SubprocessHandle {
    package_id: PackageId,
    generation: u64,
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    stdout: Mutex<BufReader<ChildStdout>>,
    stderr: Mutex<BufReader<ChildStderr>>,
    invoke_timeout: Duration,
    pending_responses: Mutex<HashMap<String, oneshot::Sender<Value>>>,
    reverse_platform_requests: Mutex<HashSet<String>>,
    current_session_id: Mutex<Option<String>>,
    transport_lost: AtomicBool,
}

#[derive(Clone)]
struct ActivationLossContext {
    store: Arc<dyn EventStore>,
    packages: Arc<PackageRegistry>,
    run_control: Arc<dyn RunControl>,
    retry_delay: Duration,
}

#[derive(Clone)]
struct PendingActivationLoss {
    package_id: PackageId,
    reason: &'static str,
    record: PackageRecord,
    run_ids: Vec<plurora_work::RunId>,
    log_tail: Vec<SubprocessLogLine>,
    package_event_committed: bool,
    processing: bool,
    retry_running: bool,
}

impl SubprocessSupervisor {
    pub(crate) fn new<S>(
        store: Arc<S>,
        packages: Arc<PackageRegistry>,
        run_control: Arc<dyn RunControl>,
        retry_delay: Duration,
    ) -> Self
    where
        S: EventStore,
    {
        Self {
            handles: RwLock::new(HashMap::new()),
            lifecycle_transition: Mutex::new(()),
            pending_activation_losses: Mutex::new(HashMap::new()),
            loss_context: ActivationLossContext {
                store,
                packages,
                run_control,
                retry_delay,
            },
            next_generation: AtomicU64::new(1),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, JsonSchema)]
pub struct SubprocessLogLine {
    pub package_id: PackageId,
    pub stream: String,
    pub line: String,
}

impl SubprocessSupervisor {
    pub async fn start<S>(
        self: &Arc<Self>,
        manifest: &PackageManifest,
        runtime: Runtime<S>,
        bindings: HashMap<String, CapHandleId>,
    ) -> anyhow::Result<()>
    where
        S: EventStore,
    {
        self.start_generation(manifest, runtime, bindings)
            .await
            .map(|_| ())
    }

    pub(crate) async fn start_generation<S>(
        self: &Arc<Self>,
        manifest: &PackageManifest,
        runtime: Runtime<S>,
        bindings: HashMap<String, CapHandleId>,
    ) -> anyhow::Result<u64>
    where
        S: EventStore,
    {
        let PackageEntry::Subprocess { command, transport } = &manifest.entry.kind else {
            return Ok(0);
        };
        if transport != &SubprocessTransport::JsonRpcStdio {
            anyhow::bail!("subprocess transport '{transport:?}' is not supported yet");
        }
        let (program, args) = command
            .split_first()
            .ok_or_else(|| anyhow::anyhow!("subprocess command must not be empty"))?;

        let resolved_program = resolve_subprocess_program(program);
        let mut command_builder = Command::new(&resolved_program);
        command_builder
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let package_root = runtime.config().package_roots.get(&manifest.id).cloned();
        if let Some(package_root) = &package_root {
            command_builder.current_dir(package_root);
        }
        let mut child = command_builder.spawn().with_context(|| {
            let cwd = package_root
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<inherited>".to_string());
            format!(
                "failed to spawn subprocess package {} command {:?} cwd {}",
                manifest.id, command, cwd
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture subprocess stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture subprocess stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture subprocess stderr"))?;
        let handle = Arc::new(SubprocessHandle {
            package_id: manifest.id.clone(),
            generation: self.next_generation.fetch_add(1, Ordering::Relaxed),
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            stderr: Mutex::new(BufReader::new(stderr)),
            invoke_timeout: Duration::from_millis(manifest.sandbox_policy.cpu_quota_ms_per_invoke),
            pending_responses: Mutex::new(HashMap::new()),
            reverse_platform_requests: Mutex::new(HashSet::new()),
            current_session_id: Mutex::new(None),
            transport_lost: AtomicBool::new(false),
        });

        let handshake_timeout =
            Duration::from_millis(manifest.sandbox_policy.wall_clock_ms.min(5_000));
        let package_envelope = package_envelope_for_manifest(manifest)?;
        let participates_in_contract = manifest.entry.contract == ContractMode::V1;
        let permissions = participates_in_contract
            .then(|| manifest.permissions.clone())
            .unwrap_or_else(PermissionSet::default);
        let capabilities = if participates_in_contract {
            manifest
                .provides
                .iter()
                .map(|capability| capability.id.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let request = json!({
            "jsonrpc": "2.0",
            "id": "handshake-1",
            "method": "package.handshake",
            "params": {
                "protocol_version": crate::PLATFORM_PROTOCOL_VERSION,
                "package_id": manifest.id,
                "manifest_version": manifest.version,
                "contract_mode": manifest.entry.contract,
                "foreign_capsule": !participates_in_contract,
                "package_envelope_digest": package_envelope.artifact.digest,
                "components": package_envelope.components,
                "permissions": permissions,
                "capabilities": capabilities,
                "bindings": bindings,
            }
        });
        let response = match timeout(handshake_timeout, handle.call_direct(request)).await {
            Ok(result) => result,
            Err(_) => {
                handle.kill().await;
                anyhow::bail!("subprocess '{}' handshake timed out", manifest.id);
            }
        }?;
        if response.get("error").is_some() {
            handle.kill().await;
            anyhow::bail!("subprocess '{}' handshake failed: {response}", manifest.id);
        }
        let ready = response
            .get("result")
            .and_then(|result| result.get("ready"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !ready {
            handle.kill().await;
            anyhow::bail!("subprocess '{}' did not report ready", manifest.id);
        }

        {
            let mut handles = self.handles.write().await;
            if handles.contains_key(&manifest.id) {
                drop(handles);
                handle.kill().await;
                anyhow::bail!(
                    "subprocess package '{}' already has an active instance",
                    manifest.id
                );
            }
            handles.insert(manifest.id.clone(), handle.clone());
        }
        // Registration must happen before the EOF/read watcher can report a
        // loss. Otherwise a process that exits immediately after handshake can
        // leave a durable Run pointing at an unobservable dead instance.
        let generation = handle.generation;
        let supervisor = self.clone();
        let (watcher_armed, watcher_armed_rx) = oneshot::channel();
        tokio::spawn(async move {
            handle
                .pump_reverse_platform_requests(runtime, supervisor, watcher_armed)
                .await;
        });
        // The watcher sends this signal immediately before its first stdout
        // read. Because sending does not yield, an already-observable EOF is
        // recorded as transport loss before this await can resume.
        let _ = watcher_armed_rx.await;
        tokio::task::yield_now().await;
        Ok(generation)
    }

    pub(crate) async fn complete_start<S>(
        self: &Arc<Self>,
        runtime: &Runtime<S>,
        package_id: &PackageId,
        generation: u64,
        reason: Option<&str>,
    ) -> anyhow::Result<PackageRecord>
    where
        S: EventStore,
    {
        let transition = self.lifecycle_transition.lock().await;
        let Some(handle) = self.handles.read().await.get(package_id).cloned() else {
            drop(transition);
            anyhow::bail!("subprocess package '{package_id}' lost its transport during startup");
        };
        anyhow::ensure!(
            handle.generation == generation,
            "subprocess package '{package_id}' startup generation was replaced"
        );
        if handle.transport_lost.load(Ordering::Acquire) || handle.has_exited().await? {
            handle.transport_lost.store(true, Ordering::Release);
            drop(transition);
            let _ = self
                .report_transport_loss(package_id, generation, "subprocess_transport_eof")
                .await;
            anyhow::bail!("subprocess package '{package_id}' lost its transport during startup");
        }

        let candidate = runtime
            .packages
            .state_transition_candidate(package_id, PackageState::Starting, PackageState::Ready)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "subprocess package '{package_id}' is not Starting at activation commit"
                )
            })?;
        if let Err(error) = runtime
            .append_package_lifecycle_event(&candidate, plurora_core::EVENT_PACKAGE_READY, reason)
            .await
        {
            let mut handles = self.handles.write().await;
            if handles
                .get(package_id)
                .is_some_and(|active| active.generation == generation)
            {
                handles.remove(package_id);
            }
            drop(handles);
            runtime
                .packages
                .set_state(package_id, PackageState::Degraded)
                .await;
            handle.kill().await;
            drop(transition);
            return Err(error);
        }
        if handle.transport_lost.load(Ordering::Acquire) {
            drop(transition);
            let _ = self
                .report_transport_loss(package_id, generation, "subprocess_transport_eof")
                .await;
            anyhow::bail!("subprocess package '{package_id}' lost its transport during startup");
        }
        let ready = runtime
            .packages
            .finish_subprocess_start(package_id)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "subprocess package '{package_id}' changed during activation commit"
                )
            })?;
        drop(transition);
        Ok(ready)
    }

    pub async fn invoke<S>(
        self: &Arc<Self>,
        _runtime: Runtime<S>,
        package_id: &PackageId,
        capability_id: &str,
        session_id: Option<String>,
        input: Value,
    ) -> anyhow::Result<Value>
    where
        S: EventStore,
    {
        let handle = self
            .handles
            .read()
            .await
            .get(package_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("subprocess package '{package_id}' is not ready"))?;
        let request = json!({
            "jsonrpc": "2.0",
            "id": "invoke-1",
            "method": "capability.invoke",
            "params": { "capability_id": capability_id, "session_id": session_id, "input": input }
        });
        *handle.current_session_id.lock().await = session_id;
        let response = match timeout(handle.invoke_timeout, handle.call(request)).await {
            Ok(Ok(response)) => {
                *handle.current_session_id.lock().await = None;
                response
            }
            Ok(Err(error)) => {
                *handle.current_session_id.lock().await = None;
                let _ = self
                    .report_transport_loss(
                        package_id,
                        handle.generation,
                        "subprocess_transport_unavailable",
                    )
                    .await;
                return Err(error);
            }
            Err(_) => {
                *handle.current_session_id.lock().await = None;
                handle.pending_responses.lock().await.remove("invoke-1");
                let _ = self
                    .report_transport_loss(
                        package_id,
                        handle.generation,
                        "subprocess_transport_timeout",
                    )
                    .await;
                anyhow::bail!("subprocess package '{package_id}' invoke timed out");
            }
        };
        if let Some(error) = response.get("error") {
            anyhow::bail!("subprocess package '{package_id}' returned error: {error}");
        }
        let output = response
            .get("result")
            .and_then(|result| result.get("output"))
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!("subprocess package '{package_id}' returned no output")
            })?;
        Ok(output)
    }

    pub async fn stop(self: &Arc<Self>, package_id: &PackageId) {
        // Removing the exact generation is the intentional-stop guard. The
        // watcher may observe EOF after kill, but it can no longer degrade a
        // replacement instance or terminalize a Run.
        let _transition = self.lifecycle_transition.lock().await;
        if let Some(handle) = self.handles.write().await.remove(package_id) {
            handle.kill().await;
        }
    }

    pub async fn restart<S>(
        self: &Arc<Self>,
        manifest: &PackageManifest,
        runtime: Runtime<S>,
        bindings: HashMap<String, CapHandleId>,
    ) -> anyhow::Result<()>
    where
        S: EventStore,
    {
        self.stop(&manifest.id).await;
        self.start(manifest, runtime, bindings).await
    }

    pub async fn drain_logs(&self, package_id: &PackageId) -> Vec<SubprocessLogLine> {
        let Some(handle) = self.handles.read().await.get(package_id).cloned() else {
            return Vec::new();
        };
        handle.drain_logs().await
    }

    async fn report_transport_loss(
        self: &Arc<Self>,
        package_id: &PackageId,
        generation: u64,
        reason: &'static str,
    ) -> anyhow::Result<bool> {
        let _transition = self.lifecycle_transition.lock().await;
        if self
            .pending_activation_losses
            .lock()
            .await
            .contains_key(&(package_id.clone(), generation))
        {
            return Ok(false);
        }
        let handle = {
            let mut handles = self.handles.write().await;
            match handles.get(package_id) {
                Some(handle) if handle.generation == generation => handles.remove(package_id),
                _ => None,
            }
        };
        let Some(handle) = handle else {
            return Ok(false);
        };

        handle.transport_lost.store(true, Ordering::Release);
        // Wake all callers before waiting on the Host control plane. No
        // supervisor, stdio, child, or pending-response lock crosses that await.
        handle.pending_responses.lock().await.clear();
        let drained_logs = handle.drain_logs().await;
        handle.kill().await;
        let Some((record, run_ids)) = self
            .loss_context
            .packages
            .mark_activation_lost(package_id)
            .await
        else {
            return Ok(false);
        };
        let (record, log_tail) = crate::runtime::prepare_package_degraded_record(
            &self.loss_context.packages,
            record,
            reason,
            drained_logs,
        )
        .await;
        self.pending_activation_losses.lock().await.insert(
            (package_id.clone(), generation),
            PendingActivationLoss {
                package_id: package_id.clone(),
                reason,
                record,
                run_ids,
                log_tail,
                package_event_committed: false,
                processing: false,
                retry_running: false,
            },
        );
        drop(_transition);

        if self
            .drive_pending_activation_loss(package_id, generation)
            .await
            .is_err()
        {
            self.ensure_activation_loss_retry(package_id.clone(), generation)
                .await;
        }
        Ok(true)
    }

    async fn drive_pending_activation_loss(
        &self,
        package_id: &PackageId,
        generation: u64,
    ) -> anyhow::Result<bool> {
        let key = (package_id.clone(), generation);
        let pending = {
            let mut pending = self.pending_activation_losses.lock().await;
            let Some(loss) = pending.get_mut(&key) else {
                return Ok(true);
            };
            if loss.processing {
                return Ok(false);
            }
            loss.processing = true;
            loss.clone()
        };

        if !pending.package_event_committed {
            if let Err(error) = crate::runtime::append_prepared_package_degraded_event(
                self.loss_context.store.as_ref(),
                &pending.record,
                pending.reason,
                &pending.log_tail,
            )
            .await
            {
                if let Some(loss) = self.pending_activation_losses.lock().await.get_mut(&key) {
                    loss.processing = false;
                }
                return Err(error);
            }
            if let Some(loss) = self.pending_activation_losses.lock().await.get_mut(&key) {
                loss.package_event_committed = true;
            }
        }

        if !pending.run_ids.is_empty() {
            if let Err(error) = self
                .loss_context
                .run_control
                .package_activation_lost(&pending.package_id, pending.run_ids.clone())
                .await
            {
                if let Some(loss) = self.pending_activation_losses.lock().await.get_mut(&key) {
                    loss.processing = false;
                }
                return Err(error);
            }
        }

        self.pending_activation_losses.lock().await.remove(&key);
        Ok(true)
    }

    async fn ensure_activation_loss_retry(
        self: &Arc<Self>,
        package_id: PackageId,
        generation: u64,
    ) {
        let should_spawn = {
            let mut pending = self.pending_activation_losses.lock().await;
            pending
                .get_mut(&(package_id.clone(), generation))
                .is_some_and(|loss| {
                    if loss.retry_running {
                        false
                    } else {
                        loss.retry_running = true;
                        true
                    }
                })
        };
        if !should_spawn {
            return;
        }

        let supervisor: Weak<Self> = Arc::downgrade(self);
        let retry_delay = self.loss_context.retry_delay;
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(retry_delay).await;
                let Some(supervisor) = supervisor.upgrade() else {
                    return;
                };
                match supervisor
                    .drive_pending_activation_loss(&package_id, generation)
                    .await
                {
                    Ok(true) => return,
                    Ok(false) | Err(_) => {}
                }
                // Do not keep the Host alive between retries. Dropping the last
                // Runtime/Supervisor owner terminates this loop on the next tick.
                drop(supervisor);
            }
        });
    }

    #[cfg(test)]
    async fn pending_activation_loss_count(&self) -> usize {
        self.pending_activation_losses.lock().await.len()
    }
}

impl SubprocessHandle {
    async fn call(&self, request: Value) -> anyhow::Result<Value> {
        let id = request
            .get("id")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("subprocess request missing id"))?;
        let id_key = id_to_key(&id);
        let (tx, rx) = oneshot::channel();
        self.pending_responses
            .lock()
            .await
            .insert(id_key.clone(), tx);

        if let Err(error) = self.write_json_frame(request).await {
            self.pending_responses.lock().await.remove(&id_key);
            return Err(error);
        }

        rx.await.map_err(|_| {
            anyhow::anyhow!(
                "subprocess package '{}' response channel closed",
                self.package_id
            )
        })
    }

    async fn call_direct(&self, request: Value) -> anyhow::Result<Value> {
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(serde_json::to_string(&request)?.as_bytes())
            .await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        drop(stdin);

        let mut line = String::new();
        let mut stdout = self.stdout.lock().await;
        let read = stdout.read_line(&mut line).await?;
        if read == 0 {
            anyhow::bail!("subprocess package '{}' exited", self.package_id);
        }
        Ok(serde_json::from_str(&line)?)
    }

    async fn kill(&self) {
        let mut child = self.child.lock().await;
        let _ = child.kill().await;
        let _ = child.wait().await;
    }

    async fn has_exited(&self) -> anyhow::Result<bool> {
        Ok(self.child.lock().await.try_wait()?.is_some())
    }

    async fn drain_logs(&self) -> Vec<SubprocessLogLine> {
        let mut logs = Vec::new();
        let mut stderr = self.stderr.lock().await;
        loop {
            let mut line = String::new();
            match timeout(Duration::from_millis(1), stderr.read_line(&mut line)).await {
                Ok(Ok(read)) if read > 0 => logs.push(SubprocessLogLine {
                    package_id: self.package_id.clone(),
                    stream: "stderr".to_string(),
                    line: line.trim_end().to_string(),
                }),
                _ => break,
            }
        }
        logs
    }

    async fn pump_reverse_platform_requests<S>(
        self: Arc<Self>,
        runtime: Runtime<S>,
        supervisor: Arc<SubprocessSupervisor>,
        watcher_armed: oneshot::Sender<()>,
    ) where
        S: EventStore,
    {
        let _ = watcher_armed.send(());
        let loss_reason = loop {
            let mut line = String::new();
            let read = {
                let mut stdout = self.stdout.lock().await;
                match stdout.read_line(&mut line).await {
                    Ok(read) => read,
                    Err(_) => {
                        break "subprocess_transport_read_failed";
                    }
                }
            };
            if read == 0 {
                break "subprocess_transport_eof";
            }

            let Ok(frame) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if frame.get("method").is_none() {
                if let Some(id) = frame.get("id") {
                    let id_key = id_to_key(id);
                    if let Some(tx) = self.pending_responses.lock().await.remove(&id_key) {
                        let _ = tx.send(frame);
                    }
                }
                continue;
            }
            let Some(method) = frame.get("method").and_then(Value::as_str) else {
                continue;
            };
            let id = frame.get("id").cloned().unwrap_or(Value::Null);
            let request_id = id_to_key(&id);
            self.reverse_platform_requests
                .lock()
                .await
                .insert(request_id.clone());

            let Ok(resolved) = resolve_contract_method(method) else {
                let error = ProtocolError::invalid_request(format!(
                    "protocol method '{}' is not a known contract method",
                    method
                ));
                if self
                    .write_json_frame(json!({"jsonrpc": "2.0", "id": id, "error": error}))
                    .await
                    .is_err()
                {
                    self.reverse_platform_requests
                        .lock()
                        .await
                        .remove(&request_id);
                    break "subprocess_transport_write_failed";
                }
                self.reverse_platform_requests
                    .lock()
                    .await
                    .remove(&request_id);
                continue;
            };
            let platform_method = resolved.method;

            let stream_events = if platform_method.streaming() {
                Some(runtime.store().subscribe())
            } else {
                None
            };
            let session_id = self.current_session_id.lock().await.clone();
            let response =
                dispatch_reverse_platform_frame(&runtime, &self.package_id, session_id, frame)
                    .await;
            let stream_id = if platform_method.streaming() {
                response
                    .get("result")
                    .and_then(|result| result.get("stream_id"))
                    .or_else(|| {
                        if matches!(platform_method, PlatformMethod::OutboundWebSocketOpen) {
                            response
                                .get("result")
                                .and_then(|result| result.get("connection_id"))
                        } else {
                            None
                        }
                    })
                    .and_then(Value::as_str)
                    .map(str::to_string)
            } else {
                None
            };

            if self.write_json_frame(response).await.is_err() {
                self.reverse_platform_requests
                    .lock()
                    .await
                    .remove(&request_id);
                break "subprocess_transport_write_failed";
            }

            if let (Some(stream_id), Some(stream_events)) = (stream_id, stream_events) {
                let stream_handle = self.clone();
                let runtime_for_stream = runtime.clone();
                let supervisor_for_stream = supervisor.clone();
                tokio::spawn(async move {
                    stream_handle
                        .pipe_reverse_stream(
                            runtime_for_stream,
                            supervisor_for_stream,
                            id,
                            request_id,
                            stream_id,
                            stream_events,
                        )
                        .await;
                });
            } else {
                self.reverse_platform_requests
                    .lock()
                    .await
                    .remove(&request_id);
            }
        };
        self.transport_lost.store(true, Ordering::Release);
        let package_id = self.package_id.clone();
        let generation = self.generation;
        if supervisor
            .report_transport_loss(&package_id, generation, loss_reason)
            .await
            .is_err()
        {
            eprintln!(
                "subprocess package '{}' transport loss could not durably update affected Runs",
                package_id
            );
        }
    }

    async fn pipe_reverse_stream<S>(
        self: Arc<Self>,
        runtime: Runtime<S>,
        supervisor: Arc<SubprocessSupervisor>,
        id: Value,
        request_id: String,
        stream_id: String,
        mut events: tokio::sync::broadcast::Receiver<plurora_core::EventEnvelope>,
    ) where
        S: EventStore,
    {
        let Some(_record) = runtime
            .stream_registry()
            .get_invocation_by_stream_id(&stream_id)
            .await
        else {
            let write_failed = self
                .write_json_frame(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "stream.error",
                    "stream_id": stream_id,
                    "error": "stream not found"
                }))
                .await
                .is_err();
            self.reverse_platform_requests
                .lock()
                .await
                .remove(&request_id);
            if write_failed {
                let package_id = self.package_id.clone();
                let _ = supervisor
                    .report_transport_loss(
                        &package_id,
                        self.generation,
                        "subprocess_transport_write_failed",
                    )
                    .await;
            }
            return;
        };

        let mut seen_sequences = HashSet::new();
        loop {
            let Ok(event) = events.recv().await else {
                break;
            };
            if event.payload.get("stream_id").and_then(Value::as_str) != Some(stream_id.as_str()) {
                continue;
            }

            let frame = match event.kind.as_str() {
                plurora_core::EVENT_OUTBOUND_WEBSOCKET_OPENED => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "host/outbound.websocket.opened",
                    "connection_id": stream_id,
                    "payload": event.payload,
                }),
                plurora_core::EVENT_OUTBOUND_WEBSOCKET_FRAME => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "host/outbound.websocket.frame",
                    "connection_id": stream_id,
                    "payload": event.payload,
                }),
                plurora_core::EVENT_OUTBOUND_WEBSOCKET_ERROR => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "host/outbound.websocket.error",
                    "connection_id": stream_id,
                    "payload": event.payload,
                }),
                plurora_core::EVENT_OUTBOUND_WEBSOCKET_COMPLETED => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "host/outbound.websocket.completed",
                    "connection_id": stream_id,
                    "payload": event.payload,
                }),
                plurora_core::EVENT_STREAM_CHUNK => {
                    let sequence = event
                        .payload
                        .get("sequence")
                        .and_then(Value::as_u64)
                        .or_else(|| event.payload.get("outbound_seq").and_then(Value::as_u64))
                        .unwrap_or(event.sequence);
                    if !seen_sequences.insert(sequence) {
                        continue;
                    }
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "kind": "capability/stream.chunk",
                        "stream_id": stream_id,
                        "sequence": sequence,
                        "data": event.payload,
                    })
                }
                plurora_core::EVENT_STREAM_ENDED => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "capability/stream.ended",
                    "stream_id": stream_id,
                    "summary": event.payload,
                }),
                plurora_core::EVENT_STREAM_ERROR => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "capability/stream.error",
                    "stream_id": stream_id,
                    "error": event.payload.get("error").cloned().unwrap_or(event.payload),
                }),
                plurora_core::EVENT_STREAM_CANCELLED => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "capability/stream.cancelled",
                    "stream_id": stream_id,
                }),
                plurora_core::EVENT_STREAM_TIMEOUT => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "capability/stream.timeout",
                    "stream_id": stream_id,
                }),
                _ => continue,
            };

            let terminal = matches!(
                frame.get("kind").and_then(Value::as_str),
                Some(
                    "capability/stream.ended"
                        | "capability/stream.error"
                        | "capability/stream.cancelled"
                        | "capability/stream.timeout"
                        | "host/outbound.websocket.completed"
                )
            );
            if self.write_json_frame(frame).await.is_err() {
                let package_id = self.package_id.clone();
                let _ = supervisor
                    .report_transport_loss(
                        &package_id,
                        self.generation,
                        "subprocess_transport_write_failed",
                    )
                    .await;
                break;
            }
            if terminal {
                break;
            }
        }
        self.reverse_platform_requests
            .lock()
            .await
            .remove(&request_id);
    }

    async fn write_json_frame(&self, frame: Value) -> anyhow::Result<()> {
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(serde_json::to_string(&frame)?.as_bytes())
            .await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }
}

pub(crate) fn id_to_key(id: &Value) -> String {
    match id {
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

/// Dispatch one reverse public-contract JSON-RPC frame from a subprocess child.
/// The caller principal is always locked to `package_id`; any package_id in
/// params is treated as untrusted request data by downstream dispatch.
pub async fn dispatch_reverse_platform_frame<S>(
    runtime: &Runtime<S>,
    package_id: &str,
    session_id: Option<String>,
    frame: Value,
) -> Value
where
    S: EventStore,
{
    let id = frame.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = frame.get("method").and_then(Value::as_str) else {
        return json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": ProtocolError::invalid_request("reverse platform frame missing method"),
        });
    };
    let contract = match frame.get("contract") {
        Some(value) => match serde_json::from_value::<crate::ContractSelection>(value.clone()) {
            Ok(contract) => Some(contract),
            Err(error) => {
                return json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": ProtocolError::invalid_request(format!("invalid contract selection: {error}")),
                });
            }
        },
        None => None,
    };
    let mut context = ProtocolContext::package(package_id.to_string(), "subprocess_stdio");
    context.session_id = session_id;
    match runtime
        .call_subprocess_protocol_negotiated(
            &context,
            method,
            frame.get("params").cloned().unwrap_or(Value::Null),
            contract.as_ref(),
        )
        .await
    {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
    }
}

fn resolve_subprocess_program(program: &str) -> String {
    #[cfg(windows)]
    if program == "python3" {
        return std::env::var("PLURORA_PYTHON").unwrap_or_else(|_| "python".to_string());
    }
    program.to_string()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::{Arc, Mutex as StdMutex};

    use super::*;
    use crate::{
        InMemoryEventStore, PackageState, RunControl, RunGetRequest, RunListRequest,
        RunMutationResult, RunStartRequest, RunStartResult, RunStatusRequest, RunStatusView,
        RunStopRequest, RunView, RuntimeConfig, DEFAULT_CONTRACT_PROFILE,
    };
    use async_trait::async_trait;
    use plurora_core::{
        CapabilityDescriptor, EntryDescriptor, PackageContributions, SandboxPolicy,
        EVENT_PACKAGE_DEGRADED,
    };
    use plurora_work::RunId;

    #[derive(Default)]
    struct LossSpy {
        calls: StdMutex<Vec<(PackageId, Vec<RunId>)>>,
    }

    #[derive(Default)]
    struct RetryingLossSpy {
        attempts: AtomicUsize,
        terminal_commits: AtomicUsize,
        calls: StdMutex<Vec<(PackageId, Vec<RunId>)>>,
        lease: StdMutex<Option<crate::package::PackageRunLease>>,
    }

    #[async_trait]
    impl RunControl for LossSpy {
        async fn list(&self, _request: RunListRequest) -> anyhow::Result<Vec<RunView>> {
            anyhow::bail!("not used")
        }

        async fn get(&self, _request: RunGetRequest) -> anyhow::Result<Option<RunView>> {
            anyhow::bail!("not used")
        }

        async fn status(&self, _request: RunStatusRequest) -> anyhow::Result<RunStatusView> {
            anyhow::bail!("not used")
        }

        async fn start(&self, _request: RunStartRequest) -> anyhow::Result<RunStartResult> {
            anyhow::bail!("not used")
        }

        async fn stop(&self, _request: RunStopRequest) -> anyhow::Result<RunMutationResult> {
            anyhow::bail!("not used")
        }

        async fn package_activation_lost(
            &self,
            package_id: &PackageId,
            run_ids: Vec<RunId>,
        ) -> anyhow::Result<()> {
            self.calls
                .lock()
                .expect("loss spy lock")
                .push((package_id.clone(), run_ids));
            Ok(())
        }
    }

    #[async_trait]
    impl RunControl for RetryingLossSpy {
        async fn list(&self, _request: RunListRequest) -> anyhow::Result<Vec<RunView>> {
            anyhow::bail!("not used")
        }

        async fn get(&self, _request: RunGetRequest) -> anyhow::Result<Option<RunView>> {
            anyhow::bail!("not used")
        }

        async fn status(&self, _request: RunStatusRequest) -> anyhow::Result<RunStatusView> {
            anyhow::bail!("not used")
        }

        async fn start(&self, _request: RunStartRequest) -> anyhow::Result<RunStartResult> {
            anyhow::bail!("not used")
        }

        async fn stop(&self, _request: RunStopRequest) -> anyhow::Result<RunMutationResult> {
            anyhow::bail!("not used")
        }

        async fn package_activation_lost(
            &self,
            package_id: &PackageId,
            run_ids: Vec<RunId>,
        ) -> anyhow::Result<()> {
            let attempt = self.attempts.fetch_add(1, AtomicOrdering::SeqCst);
            self.calls
                .lock()
                .expect("retry loss spy calls lock")
                .push((package_id.clone(), run_ids));
            if attempt == 0 {
                anyhow::bail!("injected ordinary Run control failure");
            }
            self.terminal_commits.fetch_add(1, AtomicOrdering::SeqCst);
            drop(self.lease.lock().expect("retry loss spy lease lock").take());
            Ok(())
        }
    }

    fn python_program() -> String {
        std::env::var("PLURORA_TEST_PYTHON").unwrap_or_else(|_| {
            if cfg!(windows) {
                "python".to_string()
            } else {
                "python3".to_string()
            }
        })
    }

    fn subprocess_manifest(
        package_id: &str,
        script_name: &str,
        capability: bool,
        invoke_timeout_ms: u64,
    ) -> PackageManifest {
        PackageManifest {
            schema_version: 1,
            id: package_id.to_string(),
            version: "0.1.0".to_string(),
            display_name: None,
            description: None,
            author: None,
            license: None,
            entry: EntryDescriptor::v1(PackageEntry::Subprocess {
                command: vec![python_program(), script_name.to_string()],
                transport: SubprocessTransport::JsonRpcStdio,
            }),
            provides: capability
                .then(|| CapabilityDescriptor {
                    id: format!("{package_id}/invoke"),
                    version: "0.1.0".to_string(),
                    input_schema: Value::Null,
                    output_schema: Value::Null,
                    streaming: false,
                    side_effects: Vec::new(),
                    description: None,
                })
                .into_iter()
                .collect(),
            consumes: Vec::new(),
            requires: Vec::new(),
            contributes: PackageContributions::default(),
            permissions: PermissionSet::default(),
            sandbox_policy: SandboxPolicy {
                cpu_quota_ms_per_invoke: invoke_timeout_ms,
                ..SandboxPolicy::default()
            },
        }
    }

    async fn wait_for_package_state(
        runtime: &Runtime<InMemoryEventStore>,
        package_id: &PackageId,
        expected: PackageState,
    ) -> anyhow::Result<()> {
        timeout(Duration::from_secs(5), async {
            loop {
                if runtime
                    .package_status(package_id)
                    .await
                    .is_some_and(|record| record.state == expected)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("timed out waiting for Package state"))?;
        Ok(())
    }

    #[tokio::test]
    async fn reverse_dispatch_accepts_only_registered_method_ids() {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        let current = dispatch_reverse_platform_frame(
            &runtime,
            "example/reverse",
            None,
            json!({"id":"current","method":"host.info","params":{}}),
        )
        .await;
        assert!(current.get("result").is_some());

        let removed = dispatch_reverse_platform_frame(
            &runtime,
            "example/reverse",
            None,
            json!({"id":"removed","method":"platform.host.info","params":{}}),
        )
        .await;
        assert_eq!(removed["error"]["code"], "runtime/error/invalid_request");
        assert!(removed.get("diagnostics").is_none());
    }

    #[tokio::test]
    async fn reverse_dispatch_rejects_unsupported_contract_versions() {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        let response = dispatch_reverse_platform_frame(
            &runtime,
            "example/reverse",
            None,
            json!({
                "id": "unsupported",
                "method": "host.info",
                "params": {},
                "contract": {
                    "profile": DEFAULT_CONTRACT_PROFILE,
                    "versions": [{"layer":"host","version":"999.0.0"}]
                }
            }),
        )
        .await;
        assert_eq!(
            response["error"]["code"],
            "protocol/error/unsupported_contract"
        );
        assert!(response.get("result").is_none());

        let malformed = dispatch_reverse_platform_frame(
            &runtime,
            "example/reverse",
            None,
            json!({
                "id": "malformed",
                "method": "host.info",
                "params": {},
                "contract": "bad"
            }),
        )
        .await;
        assert_eq!(malformed["error"]["code"], "runtime/error/invalid_request");
        assert!(malformed.get("diagnostics").is_none());
    }

    #[tokio::test]
    async fn idle_eof_degrades_package_and_reports_exact_run_once() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        std::fs::write(
            temp.path().join("idle_eof.py"),
            r#"import json, sys, time
line = sys.stdin.readline()
msg = json.loads(line)
print(json.dumps({"jsonrpc":"2.0","id":msg.get("id"),"result":{"ready":True}}), flush=True)
time.sleep(0.5)
"#,
        )?;
        let store = Arc::new(InMemoryEventStore::default());
        let spy = Arc::new(LossSpy::default());
        let package_id = "example/idle-eof".to_string();
        let mut config = RuntimeConfig {
            run_control: spy.clone(),
            ..RuntimeConfig::default()
        };
        config
            .package_roots
            .insert(package_id.clone(), temp.path().to_path_buf());
        let runtime = Runtime::new(store.clone(), config);
        let record = runtime
            .load_package(subprocess_manifest(
                &package_id,
                "idle_eof.py",
                false,
                1_000,
            ))
            .await?;
        let run_id = RunId::new();
        let claim = crate::package::PackageRunClaim::exact(&record, &record.components[0])?;
        let lease = runtime
            .packages
            .acquire_run_lease(&run_id, std::slice::from_ref(&claim))
            .await?;

        wait_for_package_state(&runtime, &package_id, PackageState::Degraded).await?;
        let calls = spy.calls.lock().expect("loss spy lock").clone();
        assert_eq!(calls, vec![(package_id.clone(), vec![run_id])]);
        assert_eq!(
            store
                .list_session(&"platform_package_example_idle-eof".to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_PACKAGE_DEGRADED)
                .count(),
            1
        );
        drop(lease);
        Ok(())
    }

    #[tokio::test]
    async fn ordinary_run_control_failure_retries_pending_loss_until_cleanup() -> anyhow::Result<()>
    {
        let temp = tempfile::tempdir()?;
        std::fs::write(
            temp.path().join("retry_eof.py"),
            r#"import json, sys, time
line = sys.stdin.readline()
msg = json.loads(line)
print(json.dumps({"jsonrpc":"2.0","id":msg.get("id"),"result":{"ready":True}}), flush=True)
time.sleep(0.25)
"#,
        )?;
        let store = Arc::new(InMemoryEventStore::default());
        let spy = Arc::new(RetryingLossSpy::default());
        let package_id = "example/retry-loss".to_string();
        let mut config = RuntimeConfig {
            run_control: spy.clone(),
            package_activation_loss_retry_delay: Duration::from_millis(100),
            ..RuntimeConfig::default()
        };
        config
            .package_roots
            .insert(package_id.clone(), temp.path().to_path_buf());
        let runtime = Runtime::new(store.clone(), config);
        let record = runtime
            .load_package(subprocess_manifest(
                &package_id,
                "retry_eof.py",
                false,
                1_000,
            ))
            .await?;
        let generation = runtime
            .subprocesses
            .handles
            .read()
            .await
            .get(&package_id)
            .expect("active generation")
            .generation;
        let run_id = RunId::new();
        let claim = crate::package::PackageRunClaim::exact(&record, &record.components[0])?;
        let lease = runtime
            .packages
            .acquire_run_lease(&run_id, std::slice::from_ref(&claim))
            .await?;
        *spy.lease.lock().expect("retry loss spy lease lock") = Some(lease);

        wait_for_package_state(&runtime, &package_id, PackageState::Degraded).await?;
        assert!(
            !runtime
                .subprocesses
                .report_transport_loss(
                    &package_id,
                    generation,
                    "subprocess_transport_write_failed",
                )
                .await?
        );
        timeout(Duration::from_secs(5), async {
            loop {
                if spy.attempts.load(AtomicOrdering::SeqCst) >= 2
                    && runtime.subprocesses.pending_activation_loss_count().await == 0
                    && runtime.packages.active_run_lease_count(&package_id).await == 0
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("timed out waiting for activation-loss retry"))?;

        assert_eq!(spy.terminal_commits.load(AtomicOrdering::SeqCst), 1);
        let calls = spy.calls.lock().expect("retry loss spy calls lock");
        assert_eq!(calls.len(), 2);
        assert!(calls
            .iter()
            .all(|(reported_package, runs)| reported_package == &package_id
                && runs == std::slice::from_ref(&run_id)));
        drop(calls);
        assert_eq!(
            store
                .list_session(&"platform_package_example_retry-loss".to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_PACKAGE_DEGRADED)
                .count(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn restart_immediate_eof_never_commits_replacement_ready() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        std::fs::write(
            temp.path().join("restart_eof.py"),
            r#"import json, os, pathlib, sys, time
counter = pathlib.Path("restart-count.txt")
count = int(counter.read_text()) + 1 if counter.exists() else 1
counter.write_text(str(count))
line = sys.stdin.readline()
msg = json.loads(line)
print(json.dumps({"jsonrpc":"2.0","id":msg.get("id"),"result":{"ready":True}}), flush=True)
if count >= 2:
    os.close(sys.stdout.fileno())
    time.sleep(0.1)
    sys.exit(0)
for line in sys.stdin:
    pass
"#,
        )?;
        let store = Arc::new(InMemoryEventStore::default());
        let package_id = "example/restart-eof".to_string();
        let mut config = RuntimeConfig::default();
        config
            .package_roots
            .insert(package_id.clone(), temp.path().to_path_buf());
        let runtime = Runtime::new(store.clone(), config);
        runtime
            .load_package(subprocess_manifest(
                &package_id,
                "restart_eof.py",
                false,
                1_000,
            ))
            .await?;

        let error = runtime
            .restart_package(&package_id)
            .await
            .expect_err("replacement exits immediately after handshake");
        assert!(error.to_string().contains("lost its transport"));
        wait_for_package_state(&runtime, &package_id, PackageState::Degraded).await?;
        assert!(!runtime
            .subprocesses
            .handles
            .read()
            .await
            .contains_key(&package_id));
        assert_eq!(
            runtime.packages.active_run_lease_count(&package_id).await,
            0
        );

        let events = store
            .list_session(&"platform_package_example_restart-eof".to_string())
            .await?;
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == plurora_core::EVENT_PACKAGE_READY)
                .count(),
            1,
            "only the initial generation may commit Ready"
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == EVENT_PACKAGE_DEGRADED)
                .count(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn provider_business_error_keeps_transport_and_package_ready() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        std::fs::write(
            temp.path().join("business_error.py"),
            r#"import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    if msg.get("method") == "package.handshake":
        print(json.dumps({"jsonrpc":"2.0","id":msg.get("id"),"result":{"ready":True}}), flush=True)
    elif msg.get("method") == "capability.invoke":
        print(json.dumps({"jsonrpc":"2.0","id":msg.get("id"),"error":{"code":"business_rejected","message":"fixture"}}), flush=True)
"#,
        )?;
        let store = Arc::new(InMemoryEventStore::default());
        let spy = Arc::new(LossSpy::default());
        let package_id = "example/business-error".to_string();
        let mut config = RuntimeConfig {
            run_control: spy.clone(),
            ..RuntimeConfig::default()
        };
        config
            .package_roots
            .insert(package_id.clone(), temp.path().to_path_buf());
        let runtime = Runtime::new(store, config);
        runtime
            .load_package(subprocess_manifest(
                &package_id,
                "business_error.py",
                true,
                1_000,
            ))
            .await?;

        let error = runtime
            .invoke_capability(crate::CapabilityInvocationRequest {
                handle: None,
                capability_id: Some(format!("{package_id}/invoke")),
                caller_package_id: None,
                provider_package_id: Some(package_id.clone()),
                version: None,
                session_id: None,
                input: json!({}),
            })
            .await
            .expect_err("provider business error is returned");
        assert!(error.to_string().contains("returned error"));
        assert_eq!(
            runtime.package_status(&package_id).await.unwrap().state,
            PackageState::Ready
        );
        assert!(spy.calls.lock().expect("loss spy lock").is_empty());
        runtime.unload_package(&package_id).await?;
        Ok(())
    }

    #[tokio::test]
    async fn stale_generation_and_duplicate_loss_cannot_affect_restarted_instance(
    ) -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        std::fs::write(
            temp.path().join("persistent.py"),
            r#"import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    if msg.get("method") == "package.handshake":
        print(json.dumps({"jsonrpc":"2.0","id":msg.get("id"),"result":{"ready":True}}), flush=True)
"#,
        )?;
        let store = Arc::new(InMemoryEventStore::default());
        let spy = Arc::new(LossSpy::default());
        let package_id = "example/generation".to_string();
        let mut config = RuntimeConfig {
            run_control: spy.clone(),
            ..RuntimeConfig::default()
        };
        config
            .package_roots
            .insert(package_id.clone(), temp.path().to_path_buf());
        let runtime = Runtime::new(store.clone(), config);
        runtime
            .load_package(subprocess_manifest(
                &package_id,
                "persistent.py",
                false,
                1_000,
            ))
            .await?;
        let old_generation = runtime
            .subprocesses
            .handles
            .read()
            .await
            .get(&package_id)
            .unwrap()
            .generation;
        runtime.restart_package(&package_id).await?;
        let new_generation = runtime
            .subprocesses
            .handles
            .read()
            .await
            .get(&package_id)
            .unwrap()
            .generation;
        assert_ne!(old_generation, new_generation);

        assert!(
            !runtime
                .subprocesses
                .report_transport_loss(&package_id, old_generation, "subprocess_transport_eof",)
                .await?
        );
        assert_eq!(
            runtime.package_status(&package_id).await.unwrap().state,
            PackageState::Ready
        );
        assert!(
            runtime
                .subprocesses
                .report_transport_loss(&package_id, new_generation, "subprocess_transport_eof",)
                .await?
        );
        assert!(
            !runtime
                .subprocesses
                .report_transport_loss(
                    &package_id,
                    new_generation,
                    "subprocess_transport_write_failed",
                )
                .await?
        );
        assert!(spy.calls.lock().expect("loss spy lock").is_empty());
        assert_eq!(
            store
                .list_session(&"platform_package_example_generation".to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_PACKAGE_DEGRADED)
                .count(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn invoke_timeout_uses_same_idempotent_transport_loss_reporter() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        std::fs::write(
            temp.path().join("timeout.py"),
            r#"import json, sys, time
for line in sys.stdin:
    msg = json.loads(line)
    if msg.get("method") == "package.handshake":
        print(json.dumps({"jsonrpc":"2.0","id":msg.get("id"),"result":{"ready":True}}), flush=True)
    elif msg.get("method") == "capability.invoke":
        time.sleep(5)
"#,
        )?;
        let store = Arc::new(InMemoryEventStore::default());
        let spy = Arc::new(LossSpy::default());
        let package_id = "example/timeout".to_string();
        let mut config = RuntimeConfig {
            run_control: spy.clone(),
            ..RuntimeConfig::default()
        };
        config
            .package_roots
            .insert(package_id.clone(), temp.path().to_path_buf());
        let runtime = Runtime::new(store, config);
        let record = runtime
            .load_package(subprocess_manifest(&package_id, "timeout.py", true, 20))
            .await?;
        let run_id = RunId::new();
        let claim = crate::package::PackageRunClaim::exact(&record, &record.components[0])?;
        let lease = runtime
            .packages
            .acquire_run_lease(&run_id, std::slice::from_ref(&claim))
            .await?;

        let error = runtime
            .invoke_capability(crate::CapabilityInvocationRequest {
                handle: None,
                capability_id: Some(format!("{package_id}/invoke")),
                caller_package_id: None,
                provider_package_id: Some(package_id.clone()),
                version: None,
                session_id: None,
                input: json!({}),
            })
            .await
            .expect_err("fixture times out");
        assert!(error.to_string().contains("timed out"));
        assert_eq!(
            runtime.package_status(&package_id).await.unwrap().state,
            PackageState::Degraded
        );
        assert_eq!(
            spy.calls.lock().expect("loss spy lock").as_slice(),
            &[(package_id, vec![run_id])]
        );
        drop(lease);
        Ok(())
    }
}
