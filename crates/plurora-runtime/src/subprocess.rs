//! JSON-RPC-over-stdio subprocess supervisor.
//!
//! In addition to host-initiated `package.handshake` and
//! `capability.invoke` calls, subprocess packages may initiate reverse
//! public kernel calls by writing JSON-RPC requests whose method starts with
//! `kernel.v1.` to stdout.  The supervisor dispatches those requests with the
//! caller principal locked to the subprocess package id and writes responses
//! back to the child's stdin.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
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
    contract_diagnostics, resolve_contract_method, EventStore, KernelMethod, ProtocolContext,
    ProtocolError, Runtime,
};

#[derive(Default)]
pub struct SubprocessSupervisor {
    handles: RwLock<HashMap<PackageId, Arc<SubprocessHandle>>>,
}

pub struct SubprocessHandle {
    package_id: PackageId,
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    stdout: Mutex<BufReader<ChildStdout>>,
    stderr: Mutex<BufReader<ChildStderr>>,
    invoke_timeout: Duration,
    pending_responses: Mutex<HashMap<String, oneshot::Sender<Value>>>,
    reverse_kernel_requests: Mutex<HashSet<String>>,
    current_session_id: Mutex<Option<String>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, JsonSchema)]
pub struct SubprocessLogLine {
    pub package_id: PackageId,
    pub stream: String,
    pub line: String,
}

impl SubprocessSupervisor {
    pub async fn start<S>(
        &self,
        manifest: &PackageManifest,
        runtime: Runtime<S>,
        bindings: HashMap<String, CapHandleId>,
    ) -> anyhow::Result<()>
    where
        S: EventStore,
    {
        let PackageEntry::Subprocess { command, transport } = &manifest.entry.kind else {
            return Ok(());
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
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            stderr: Mutex::new(BufReader::new(stderr)),
            invoke_timeout: Duration::from_millis(manifest.sandbox_policy.cpu_quota_ms_per_invoke),
            pending_responses: Mutex::new(HashMap::new()),
            reverse_kernel_requests: Mutex::new(HashSet::new()),
            current_session_id: Mutex::new(None),
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
                "protocol_version": crate::KERNEL_PROTOCOL_VERSION,
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

        let reverse_handle = handle.clone();
        tokio::spawn(async move {
            reverse_handle.pump_reverse_kernel_requests(runtime).await;
        });

        self.handles
            .write()
            .await
            .insert(manifest.id.clone(), handle);
        Ok(())
    }

    pub async fn invoke(
        &self,
        package_id: &PackageId,
        capability_id: &str,
        session_id: Option<String>,
        input: Value,
    ) -> anyhow::Result<Value> {
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
            Ok(result) => {
                *handle.current_session_id.lock().await = None;
                result?
            }
            Err(_) => {
                *handle.current_session_id.lock().await = None;
                handle.pending_responses.lock().await.remove("invoke-1");
                handle.kill().await;
                self.handles.write().await.remove(package_id);
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

    pub async fn stop(&self, package_id: &PackageId) {
        if let Some(handle) = self.handles.write().await.remove(package_id) {
            handle.kill().await;
        }
    }

    pub async fn restart<S>(
        &self,
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

    async fn pump_reverse_kernel_requests<S>(self: Arc<Self>, runtime: Runtime<S>)
    where
        S: EventStore,
    {
        loop {
            let mut line = String::new();
            let read = {
                let mut stdout = self.stdout.lock().await;
                match stdout.read_line(&mut line).await {
                    Ok(read) => read,
                    Err(_) => 0,
                }
            };
            if read == 0 {
                break;
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
            self.reverse_kernel_requests
                .lock()
                .await
                .insert(request_id.clone());

            let Ok(resolved) = resolve_contract_method(method) else {
                let error = ProtocolError::invalid_request(format!(
                    "protocol method '{}' is not a known contract method",
                    method
                ));
                let _ = self
                    .write_json_frame(json!({"jsonrpc": "2.0", "id": id, "error": error}))
                    .await;
                self.reverse_kernel_requests
                    .lock()
                    .await
                    .remove(&request_id);
                continue;
            };
            let kernel_method = resolved.method;

            let stream_events = if kernel_method.streaming() {
                Some(runtime.store().subscribe())
            } else {
                None
            };
            let session_id = self.current_session_id.lock().await.clone();
            let response =
                dispatch_reverse_kernel_frame(&runtime, &self.package_id, session_id, frame).await;
            let stream_id = if kernel_method.streaming() {
                response
                    .get("result")
                    .and_then(|result| result.get("stream_id"))
                    .or_else(|| {
                        if matches!(kernel_method, KernelMethod::OutboundWebSocketOpen) {
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
                self.reverse_kernel_requests
                    .lock()
                    .await
                    .remove(&request_id);
                break;
            }

            if let (Some(stream_id), Some(stream_events)) = (stream_id, stream_events) {
                let stream_handle = self.clone();
                let runtime_for_stream = runtime.clone();
                tokio::spawn(async move {
                    stream_handle
                        .pipe_reverse_stream(
                            runtime_for_stream,
                            id,
                            request_id,
                            stream_id,
                            stream_events,
                        )
                        .await;
                });
            } else {
                self.reverse_kernel_requests
                    .lock()
                    .await
                    .remove(&request_id);
            }
        }
    }

    async fn pipe_reverse_stream<S>(
        self: Arc<Self>,
        runtime: Runtime<S>,
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
            let _ = self
                .write_json_frame(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "stream.error",
                    "stream_id": stream_id,
                    "error": "stream not found"
                }))
                .await;
            self.reverse_kernel_requests
                .lock()
                .await
                .remove(&request_id);
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
                    "kind": "kernel/v1/outbound.websocket.opened",
                    "connection_id": stream_id,
                    "payload": event.payload,
                }),
                plurora_core::EVENT_OUTBOUND_WEBSOCKET_FRAME => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "kernel/v1/outbound.websocket.frame",
                    "connection_id": stream_id,
                    "payload": event.payload,
                }),
                plurora_core::EVENT_OUTBOUND_WEBSOCKET_ERROR => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "kernel/v1/outbound.websocket.error",
                    "connection_id": stream_id,
                    "payload": event.payload,
                }),
                plurora_core::EVENT_OUTBOUND_WEBSOCKET_COMPLETED => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "kernel/v1/outbound.websocket.completed",
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
                        "kind": "kernel/v1/stream.chunk",
                        "stream_id": stream_id,
                        "sequence": sequence,
                        "data": event.payload,
                    })
                }
                plurora_core::EVENT_STREAM_ENDED => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "kernel/v1/stream.ended",
                    "stream_id": stream_id,
                    "summary": event.payload,
                }),
                plurora_core::EVENT_STREAM_ERROR => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "kernel/v1/stream.error",
                    "stream_id": stream_id,
                    "error": event.payload.get("error").cloned().unwrap_or(event.payload),
                }),
                plurora_core::EVENT_STREAM_CANCELLED => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "kernel/v1/stream.cancelled",
                    "stream_id": stream_id,
                }),
                plurora_core::EVENT_STREAM_TIMEOUT => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "kind": "kernel/v1/stream.timeout",
                    "stream_id": stream_id,
                }),
                _ => continue,
            };

            let terminal = matches!(
                frame.get("kind").and_then(Value::as_str),
                Some(
                    "kernel/v1/stream.ended"
                        | "kernel/v1/stream.error"
                        | "kernel/v1/stream.cancelled"
                        | "kernel/v1/stream.timeout"
                        | "kernel/v1/outbound.websocket.completed"
                )
            );
            if self.write_json_frame(frame).await.is_err() || terminal {
                break;
            }
        }
        self.reverse_kernel_requests
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

fn reverse_response_with_diagnostics(method: &str, mut response: Value) -> Value {
    let diagnostics = contract_diagnostics(method);
    if !diagnostics.is_empty() {
        response
            .as_object_mut()
            .expect("reverse response is always a JSON object")
            .insert(
                "diagnostics".to_string(),
                serde_json::to_value(diagnostics).expect("contract diagnostics serialize"),
            );
    }
    response
}

/// Dispatch one reverse contract JSON-RPC frame from a subprocess child.
/// The caller principal is always locked to `package_id`; any package_id in
/// params is treated as untrusted request data by downstream dispatch.
pub async fn dispatch_reverse_kernel_frame<S>(
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
            "error": ProtocolError::invalid_request("reverse kernel frame missing method"),
        });
    };
    let contract = match frame.get("contract") {
        Some(value) => match serde_json::from_value::<crate::ContractSelection>(value.clone()) {
            Ok(contract) => Some(contract),
            Err(error) => {
                return reverse_response_with_diagnostics(
                    method,
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": ProtocolError::invalid_request(format!("invalid contract selection: {error}")),
                    }),
                );
            }
        },
        None => None,
    };
    let mut context = ProtocolContext::package(package_id.to_string(), "subprocess_stdio");
    context.session_id = session_id;
    let response = match runtime
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
    };
    reverse_response_with_diagnostics(method, response)
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
    use std::sync::Arc;

    use super::*;
    use crate::{InMemoryEventStore, RuntimeConfig, DEFAULT_CONTRACT_PROFILE};

    #[tokio::test]
    async fn reverse_dispatch_resolves_aliases_before_the_shared_handler() {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        let canonical = dispatch_reverse_kernel_frame(
            &runtime,
            "example/reverse",
            None,
            json!({"id":"canonical","method":"host.info","params":{}}),
        )
        .await;
        let legacy = dispatch_reverse_kernel_frame(
            &runtime,
            "example/reverse",
            None,
            json!({"id":"legacy","method":"kernel.v1.host.info","params":{}}),
        )
        .await;
        assert_eq!(canonical["result"], legacy["result"]);
        assert!(canonical.get("diagnostics").is_none());
        assert_eq!(
            legacy["diagnostics"][0]["code"],
            "plurora.contract.alias.legacy_adapter"
        );
        assert_eq!(legacy["diagnostics"][0]["replacement"], "host.info");
        assert_eq!(legacy["diagnostics"][0]["maturity"], "legacy_adapter");
        assert!(legacy["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("no new field semantics")));
    }

    #[tokio::test]
    async fn reverse_dispatch_rejects_unsupported_contract_versions() {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        let response = dispatch_reverse_kernel_frame(
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
            "kernel/v1/error/unsupported_contract"
        );
        assert!(response.get("result").is_none());

        let malformed = dispatch_reverse_kernel_frame(
            &runtime,
            "example/reverse",
            None,
            json!({
                "id": "malformed",
                "method": "kernel.v1.host.info",
                "params": {},
                "contract": "bad"
            }),
        )
        .await;
        assert_eq!(
            malformed["error"]["code"],
            "kernel/v1/error/invalid_request"
        );
        assert_eq!(
            malformed["diagnostics"][0]["code"],
            "plurora.contract.alias.legacy_adapter"
        );
    }
}
