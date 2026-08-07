//! Local exec / port / proxy target primitives.
//!
//! Phase 1 is intentionally fail-closed and in-memory only: no OS process
//! launch, no host proxying, and no raw secret material is stored here.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use plurora_core::{ArtifactDescriptor, PrincipalIdentity};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};

pub type ExecutionTargetId = String;
pub type ExecId = String;
pub type PortLeaseId = String;
pub type ProxyRouteId = String;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExecCommand {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecStatusKind {
    Pending,
    Running,
    Stopped,
    Exited,
    Failed,
    Denied,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecLifecyclePolicy {
    StopOnSessionClose,
    KeepAlive,
    StopOnIdle,
}

impl Default for ExecLifecyclePolicy {
    fn default() -> Self {
        Self::StopOnSessionClose
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExecResourceLimits {
    pub max_memory_mb: Option<u64>,
    pub max_cpu_millis: Option<u64>,
    pub max_duration_ms: Option<u64>,
    pub max_log_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessProbeKind {
    None,
    TcpPort,
    HttpGet,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ReadinessProbe {
    pub kind: ReadinessProbeKind,
    pub port_name: Option<String>,
    pub path: Option<String>,
    pub initial_delay_ms: Option<u64>,
    pub timeout_ms: Option<u64>,
}

impl Default for ReadinessProbe {
    fn default() -> Self {
        Self {
            kind: ReadinessProbeKind::None,
            port_name: None,
            path: None,
            initial_delay_ms: None,
            timeout_ms: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExecStatus {
    pub exec_id: Option<ExecId>,
    pub target_id: Option<ExecutionTargetId>,
    pub kind: ExecStatusKind,
    pub exit_code: Option<i32>,
    pub message: Option<String>,
    pub ready: bool,
}

impl ExecStatus {
    fn denied(message: impl Into<String>) -> Self {
        Self {
            exec_id: None,
            target_id: None,
            kind: ExecStatusKind::Denied,
            exit_code: None,
            message: Some(message.into()),
            ready: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LocalExecStartRequest {
    pub target_id: ExecutionTargetId,
    pub command: ExecCommand,
    /// Required by the live executor. Fake/deny executors tolerate omission for
    /// backwards-compatible protocol fixtures.
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    /// Explicit environment to pass to the child. The live executor clears the
    /// inherited environment and forwards only keys allowed by host policy.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub lifecycle: ExecLifecyclePolicy,
    #[serde(default)]
    pub resource_limits: ExecResourceLimits,
    #[serde(default)]
    pub readiness_probe: ReadinessProbe,
    #[serde(default)]
    pub port_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LocalExecStartResponse {
    pub exec_id: Option<ExecId>,
    pub status: ExecStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LocalExecStopRequest {
    pub exec_id: ExecId,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LocalExecStopResponse {
    pub exec_id: ExecId,
    pub status: ExecStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LocalExecStatusRequest {
    pub exec_id: ExecId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LocalExecStatusResponse {
    pub status: ExecStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LocalExecLogsRequest {
    pub exec_id: ExecId,
    pub since_seq: Option<u64>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LocalExecLogStream {
    Stdout,
    Stderr,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LocalExecLogLine {
    pub seq: u64,
    pub stream: LocalExecLogStream,
    pub message_redacted: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LocalExecLogsResponse {
    pub exec_id: ExecId,
    pub lines: Vec<LocalExecLogLine>,
    pub next_seq: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LocalExecListResponse {
    pub executions: Vec<ExecStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ManagedContainerReport {
    pub route_id: String,
    pub port_lease_id: String,
    pub running: bool,
    pub host_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

#[async_trait]
pub trait DeploymentReconcileSource: Send + Sync + 'static {
    async fn list_managed(&self) -> anyhow::Result<Vec<ManagedContainerReport>>;
}

pub struct EmptyReconcileSource;

#[async_trait]
impl DeploymentReconcileSource for EmptyReconcileSource {
    async fn list_managed(&self) -> anyhow::Result<Vec<ManagedContainerReport>> {
        Ok(Vec::new())
    }
}

#[derive(Default)]
pub struct ExecRegistry {
    executions: RwLock<HashMap<ExecId, ExecStatus>>,
    effect_contexts: RwLock<HashMap<ExecId, ExecEffectContext>>,
    terminal_receipts: RwLock<HashMap<ExecId, ArtifactDescriptor>>,
    terminal_receipt_claims: Mutex<HashSet<ExecId>>,
    operation_receipts: RwLock<HashMap<String, ArtifactDescriptor>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecEffectContext {
    pub principal: PrincipalIdentity,
    pub target_id: ExecutionTargetId,
    pub program: String,
    pub arg_count: usize,
    pub port_names: Vec<String>,
    pub lifecycle: ExecLifecyclePolicy,
    pub resource_limits: ExecResourceLimits,
    pub session_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub trace_id: String,
}

impl ExecRegistry {
    pub fn new() -> Self {
        Self {
            executions: RwLock::new(HashMap::new()),
            effect_contexts: RwLock::new(HashMap::new()),
            terminal_receipts: RwLock::new(HashMap::new()),
            terminal_receipt_claims: Mutex::new(HashSet::new()),
            operation_receipts: RwLock::new(HashMap::new()),
        }
    }

    pub async fn upsert_from_status(&self, status: &ExecStatus) {
        if let Some(exec_id) = &status.exec_id {
            self.executions
                .write()
                .await
                .insert(exec_id.clone(), status.clone());
        }
    }

    pub async fn restore(&self, status: ExecStatus) {
        if let Some(exec_id) = &status.exec_id {
            self.executions
                .write()
                .await
                .insert(exec_id.clone(), status);
        }
    }

    pub async fn status(&self, exec_id: &str) -> Option<ExecStatus> {
        self.executions.read().await.get(exec_id).cloned()
    }

    pub(crate) async fn record_effect_context(&self, exec_id: ExecId, context: ExecEffectContext) {
        self.effect_contexts.write().await.insert(exec_id, context);
    }

    pub(crate) async fn effect_context(&self, exec_id: &str) -> Option<ExecEffectContext> {
        self.effect_contexts.read().await.get(exec_id).cloned()
    }

    pub(crate) async fn terminal_receipt(&self, exec_id: &str) -> Option<ArtifactDescriptor> {
        self.terminal_receipts.read().await.get(exec_id).cloned()
    }

    pub(crate) async fn claim_terminal_receipt(&self, exec_id: &str) -> bool {
        let mut claims = self.terminal_receipt_claims.lock().await;
        if self.terminal_receipts.read().await.contains_key(exec_id) {
            return false;
        }
        claims.insert(exec_id.to_string())
    }

    pub(crate) async fn release_terminal_receipt_claim(&self, exec_id: &str) {
        self.terminal_receipt_claims.lock().await.remove(exec_id);
    }

    pub(crate) async fn record_terminal_receipt(
        &self,
        exec_id: ExecId,
        receipt: ArtifactDescriptor,
    ) {
        let mut claims = self.terminal_receipt_claims.lock().await;
        self.terminal_receipts
            .write()
            .await
            .insert(exec_id.clone(), receipt);
        claims.remove(&exec_id);
    }

    pub(crate) async fn operation_receipt(&self, key: &str) -> Option<ArtifactDescriptor> {
        self.operation_receipts.read().await.get(key).cloned()
    }

    pub(crate) async fn record_operation_receipt(&self, key: String, receipt: ArtifactDescriptor) {
        self.operation_receipts.write().await.insert(key, receipt);
    }

    pub async fn list(&self) -> Vec<ExecStatus> {
        self.executions.read().await.values().cloned().collect()
    }

    pub async fn reconcile_unknown_to_failed(&self) -> usize {
        let mut executions = self.executions.write().await;
        let mut changed = 0;
        for status in executions.values_mut() {
            if status.kind == ExecStatusKind::Unknown {
                status.kind = ExecStatusKind::Failed;
                status.ready = false;
                status.message = Some("local exec process lost after host restart".to_string());
                changed += 1;
            }
        }
        changed
    }
}

#[async_trait]
pub trait LocalExecExecutor: Send + Sync + 'static {
    fn supports_terminal_monitoring(&self) -> bool {
        false
    }

    async fn start(&self, request: LocalExecStartRequest)
        -> anyhow::Result<LocalExecStartResponse>;
    async fn stop(&self, request: LocalExecStopRequest) -> anyhow::Result<LocalExecStopResponse>;
    async fn status(
        &self,
        request: LocalExecStatusRequest,
    ) -> anyhow::Result<LocalExecStatusResponse>;
    async fn logs(&self, request: LocalExecLogsRequest) -> anyhow::Result<LocalExecLogsResponse>;
}

#[derive(Clone)]
pub enum LocalExecExecutorConfig {
    DenyAll,
    Custom(Arc<dyn LocalExecExecutor>),
    Fake,
}

impl Default for LocalExecExecutorConfig {
    fn default() -> Self {
        Self::DenyAll
    }
}

#[derive(Debug, Clone)]
pub struct LiveLocalExecExecutorConfig {
    pub allowed_programs: Vec<String>,
    pub allowed_working_dirs: Vec<PathBuf>,
    pub allowed_env_vars: Vec<String>,
    pub max_duration_ms: u64,
    pub max_log_bytes: u64,
}

impl LiveLocalExecExecutorConfig {
    pub fn new(
        allowed_programs: Vec<String>,
        allowed_working_dirs: Vec<PathBuf>,
        allowed_env_vars: Vec<String>,
        max_duration_ms: u64,
        max_log_bytes: u64,
    ) -> anyhow::Result<Self> {
        validate_allowed_programs(&allowed_programs)?;
        validate_allowed_working_dirs(&allowed_working_dirs)?;
        validate_allowed_env_vars(&allowed_env_vars)?;
        if max_duration_ms == 0 {
            anyhow::bail!("local exec max_duration_ms must be greater than zero");
        }
        if max_log_bytes == 0 {
            anyhow::bail!("local exec max_log_bytes must be greater than zero");
        }
        Ok(Self {
            allowed_programs,
            allowed_working_dirs,
            allowed_env_vars,
            max_duration_ms,
            max_log_bytes,
        })
    }
}

pub struct LiveLocalExecExecutor {
    allowed_program_names: HashSet<String>,
    allowed_program_paths: HashSet<PathBuf>,
    allowed_working_dirs: Vec<PathBuf>,
    allowed_env_vars: HashSet<String>,
    max_duration_ms: u64,
    max_log_bytes: u64,
    executions: RwLock<HashMap<ExecId, Arc<LiveExecState>>>,
    next_id: AtomicU64,
}

struct LiveExecState {
    status: RwLock<ExecStatus>,
    logs: RwLock<Vec<LocalExecLogLine>>,
    next_seq: AtomicU64,
    child: Mutex<Option<tokio::process::Child>>,
    log_bytes: AtomicU64,
    max_log_bytes: u64,
}

impl LiveLocalExecExecutor {
    pub fn new(config: LiveLocalExecExecutorConfig) -> anyhow::Result<Self> {
        let mut allowed_program_names = HashSet::new();
        let mut allowed_program_paths = HashSet::new();
        for program in config.allowed_programs {
            let path = PathBuf::from(&program);
            if path.is_absolute() {
                let canonical = std::fs::canonicalize(&path).with_context(|| {
                    format!(
                        "failed to canonicalize allowed local exec program {}",
                        path.display()
                    )
                })?;
                allowed_program_paths.insert(canonical);
            } else {
                allowed_program_names.insert(program);
            }
        }

        let mut allowed_working_dirs = Vec::new();
        for dir in config.allowed_working_dirs {
            let canonical = std::fs::canonicalize(&dir).with_context(|| {
                format!(
                    "failed to canonicalize allowed local exec working dir {}",
                    dir.display()
                )
            })?;
            if !canonical.is_dir() {
                anyhow::bail!(
                    "allowed local exec working dir is not a directory: {}",
                    canonical.display()
                );
            }
            allowed_working_dirs.push(canonical);
        }

        Ok(Self {
            allowed_program_names,
            allowed_program_paths,
            allowed_working_dirs,
            allowed_env_vars: config.allowed_env_vars.into_iter().collect(),
            max_duration_ms: config.max_duration_ms,
            max_log_bytes: config.max_log_bytes,
            executions: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(0),
        })
    }

    fn validate_start(
        &self,
        request: &LocalExecStartRequest,
    ) -> Result<ValidatedLiveExecStart, String> {
        if request.command.program.trim().is_empty() {
            return Err("local exec program is required".to_string());
        }
        if request.command.program.contains('*') {
            return Err("local exec program wildcard is not allowed".to_string());
        }
        if !self.is_program_allowed(&request.command.program) {
            return Err("local exec program is not allowed by host policy".to_string());
        }

        let cwd = request
            .cwd
            .as_ref()
            .ok_or_else(|| "local exec cwd is required by live executor".to_string())?;
        let cwd = std::fs::canonicalize(cwd)
            .map_err(|_| "local exec cwd is missing or unsafe".to_string())?;
        if !cwd.is_dir()
            || !self
                .allowed_working_dirs
                .iter()
                .any(|allowed| cwd.starts_with(allowed))
        {
            return Err("local exec cwd is not allowed by host policy".to_string());
        }

        for key in request.env.keys() {
            if key.trim().is_empty() || key.contains('=') || !self.allowed_env_vars.contains(key) {
                return Err("local exec env key is not allowed by host policy".to_string());
            }
        }

        let requested_duration = request.resource_limits.max_duration_ms;
        if requested_duration == Some(0) {
            return Err("local exec max_duration_ms must be greater than zero".to_string());
        }
        let max_duration_ms = requested_duration
            .map(|requested| requested.min(self.max_duration_ms))
            .unwrap_or(self.max_duration_ms);

        let requested_log_bytes = request.resource_limits.max_log_bytes;
        if requested_log_bytes == Some(0) {
            return Err("local exec max_log_bytes must be greater than zero".to_string());
        }
        let max_log_bytes = requested_log_bytes
            .map(|requested| requested.min(self.max_log_bytes))
            .unwrap_or(self.max_log_bytes);

        Ok(ValidatedLiveExecStart {
            cwd,
            max_duration_ms,
            max_log_bytes,
        })
    }

    fn is_program_allowed(&self, program: &str) -> bool {
        let path = PathBuf::from(program);
        if path.is_absolute() {
            return std::fs::canonicalize(path)
                .ok()
                .is_some_and(|canonical| self.allowed_program_paths.contains(&canonical));
        }
        self.allowed_program_names.contains(program)
    }

    async fn update_status_from_child(&self, exec_id: &str, state: &LiveExecState) {
        let mut child_guard = state.child.lock().await;
        let Some(child) = child_guard.as_mut() else {
            return;
        };
        let Ok(Some(exit)) = child.try_wait() else {
            return;
        };
        *child_guard = None;
        let mut status = state.status.write().await;
        status.kind = if exit.success() {
            ExecStatusKind::Exited
        } else {
            ExecStatusKind::Failed
        };
        status.exit_code = exit.code();
        status.message = Some(if exit.success() {
            "local exec exited".to_string()
        } else {
            "local exec failed".to_string()
        });
        status.ready = false;
        status.exec_id = Some(exec_id.to_string());
    }

    async fn denied_start(message: impl Into<String>) -> LocalExecStartResponse {
        let message = message.into();
        LocalExecStartResponse {
            exec_id: None,
            status: ExecStatus::denied(message.clone()),
            error: Some(message),
        }
    }
}

struct ValidatedLiveExecStart {
    cwd: PathBuf,
    max_duration_ms: u64,
    max_log_bytes: u64,
}

#[async_trait]
impl LocalExecExecutor for LiveLocalExecExecutor {
    fn supports_terminal_monitoring(&self) -> bool {
        true
    }

    async fn start(
        &self,
        request: LocalExecStartRequest,
    ) -> anyhow::Result<LocalExecStartResponse> {
        let validated = match self.validate_start(&request) {
            Ok(validated) => validated,
            Err(message) => return Ok(Self::denied_start(message).await),
        };

        let mut command = Command::new(&request.command.program);
        command
            .args(&request.command.args)
            .current_dir(&validated.cwd)
            .env_clear()
            .envs(&request.env)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(_) => {
                return Ok(Self::denied_start("local exec failed to spawn allowed program").await)
            }
        };
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let sequence = self.next_id.fetch_add(1, Ordering::SeqCst);
        let exec_id = format!("live-exec-{sequence:06}");
        let status = ExecStatus {
            exec_id: Some(exec_id.clone()),
            target_id: Some(request.target_id),
            kind: ExecStatusKind::Running,
            exit_code: None,
            message: Some("local exec running".to_string()),
            ready: true,
        };
        let state = Arc::new(LiveExecState {
            status: RwLock::new(status.clone()),
            logs: RwLock::new(Vec::new()),
            next_seq: AtomicU64::new(1),
            child: Mutex::new(Some(child)),
            log_bytes: AtomicU64::new(0),
            max_log_bytes: validated.max_log_bytes,
        });
        self.executions
            .write()
            .await
            .insert(exec_id.clone(), state.clone());

        append_live_log(
            &state,
            LocalExecLogStream::System,
            "local exec started",
            &[],
        )
        .await;
        let redaction_values: Vec<String> = request
            .env
            .values()
            .filter(|value| !value.is_empty())
            .cloned()
            .collect();
        if let Some(stdout) = stdout {
            tokio::spawn(read_live_log_stream(
                state.clone(),
                stdout,
                LocalExecLogStream::Stdout,
                redaction_values.clone(),
            ));
        }
        if let Some(stderr) = stderr {
            tokio::spawn(read_live_log_stream(
                state.clone(),
                stderr,
                LocalExecLogStream::Stderr,
                redaction_values,
            ));
        }

        let timeout_state = state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(validated.max_duration_ms)).await;
            let mut child_guard = timeout_state.child.lock().await;
            let Some(child) = child_guard.as_mut() else {
                return;
            };
            if let Ok(Some(exit)) = child.try_wait() {
                let mut status = timeout_state.status.write().await;
                status.kind = if exit.success() {
                    ExecStatusKind::Exited
                } else {
                    ExecStatusKind::Failed
                };
                status.exit_code = exit.code();
                status.message = Some(if exit.success() {
                    "local exec exited".to_string()
                } else {
                    "local exec failed".to_string()
                });
                status.ready = false;
                *child_guard = None;
                return;
            }
            let _ = child.start_kill();
            let _ = child.wait().await;
            *child_guard = None;
            let mut status = timeout_state.status.write().await;
            status.kind = ExecStatusKind::Failed;
            status.exit_code = None;
            status.message = Some("local exec timed out and was killed".to_string());
            status.ready = false;
            drop(status);
            append_live_log(
                &timeout_state,
                LocalExecLogStream::System,
                "local exec timed out and was killed",
                &[],
            )
            .await;
        });

        Ok(LocalExecStartResponse {
            exec_id: Some(exec_id),
            status,
            error: None,
        })
    }

    async fn stop(&self, request: LocalExecStopRequest) -> anyhow::Result<LocalExecStopResponse> {
        let Some(state) = self.executions.read().await.get(&request.exec_id).cloned() else {
            let status = ExecStatus {
                exec_id: Some(request.exec_id.clone()),
                target_id: None,
                kind: ExecStatusKind::Unknown,
                exit_code: None,
                message: Some("local exec not found".to_string()),
                ready: false,
            };
            return Ok(LocalExecStopResponse {
                exec_id: request.exec_id,
                status,
                error: Some("local exec not found".to_string()),
            });
        };

        self.update_status_from_child(&request.exec_id, &state)
            .await;
        let observed = state.status.read().await.clone();
        if matches!(
            observed.kind,
            ExecStatusKind::Stopped | ExecStatusKind::Exited | ExecStatusKind::Failed
        ) {
            return Ok(LocalExecStopResponse {
                exec_id: request.exec_id,
                status: observed,
                error: None,
            });
        }

        let mut child_guard = state.child.lock().await;
        if let Some(child) = child_guard.as_mut() {
            // Phase 3b uses tokio Child::start_kill for test-friendly behavior.
            // A later hardening pass can replace this with Unix process-group
            // termination for descendants.
            let _ = child.start_kill();
            let _ = child.wait().await;
            *child_guard = None;
        }
        drop(child_guard);

        let mut status_guard = state.status.write().await;
        if matches!(
            status_guard.kind,
            ExecStatusKind::Stopped | ExecStatusKind::Exited | ExecStatusKind::Failed
        ) {
            let status = status_guard.clone();
            drop(status_guard);
            return Ok(LocalExecStopResponse {
                exec_id: request.exec_id,
                status,
                error: None,
            });
        }
        status_guard.kind = ExecStatusKind::Stopped;
        status_guard.exit_code = None;
        status_guard.message = Some("local exec stopped".to_string());
        status_guard.ready = false;
        let status = status_guard.clone();
        drop(status_guard);
        append_live_log(
            &state,
            LocalExecLogStream::System,
            "local exec stopped",
            &[],
        )
        .await;
        Ok(LocalExecStopResponse {
            exec_id: request.exec_id,
            status,
            error: None,
        })
    }

    async fn status(
        &self,
        request: LocalExecStatusRequest,
    ) -> anyhow::Result<LocalExecStatusResponse> {
        let Some(state) = self.executions.read().await.get(&request.exec_id).cloned() else {
            return Ok(LocalExecStatusResponse {
                status: ExecStatus {
                    exec_id: Some(request.exec_id),
                    target_id: None,
                    kind: ExecStatusKind::Unknown,
                    exit_code: None,
                    message: Some("local exec not found".to_string()),
                    ready: false,
                },
                error: Some("local exec not found".to_string()),
            });
        };
        self.update_status_from_child(&request.exec_id, &state)
            .await;
        let status = state.status.read().await.clone();
        Ok(LocalExecStatusResponse {
            status,
            error: None,
        })
    }

    async fn logs(&self, request: LocalExecLogsRequest) -> anyhow::Result<LocalExecLogsResponse> {
        let Some(state) = self.executions.read().await.get(&request.exec_id).cloned() else {
            return Ok(LocalExecLogsResponse {
                exec_id: request.exec_id,
                lines: Vec::new(),
                next_seq: None,
                error: Some("local exec not found".to_string()),
            });
        };
        let since = request.since_seq.unwrap_or(0);
        let limit = request.limit.unwrap_or(100) as usize;
        let lines: Vec<_> = state
            .logs
            .read()
            .await
            .iter()
            .filter(|line| line.seq > since)
            .take(limit)
            .cloned()
            .collect();
        let next_seq = lines.last().map(|line| line.seq + 1);
        Ok(LocalExecLogsResponse {
            exec_id: request.exec_id,
            lines,
            next_seq,
            error: None,
        })
    }
}

impl LocalExecExecutorConfig {
    pub fn executor(&self) -> Arc<dyn LocalExecExecutor> {
        match self {
            Self::DenyAll => Arc::new(DenyAllLocalExecExecutor),
            Self::Custom(executor) => executor.clone(),
            Self::Fake => Arc::new(FakeLocalExecExecutor),
        }
    }
}

pub struct DenyAllLocalExecExecutor;

#[async_trait]
impl LocalExecExecutor for DenyAllLocalExecExecutor {
    async fn start(
        &self,
        _request: LocalExecStartRequest,
    ) -> anyhow::Result<LocalExecStartResponse> {
        let error = "local exec denied by host policy".to_string();
        Ok(LocalExecStartResponse {
            exec_id: None,
            status: ExecStatus::denied(error.clone()),
            error: Some(error),
        })
    }

    async fn stop(&self, request: LocalExecStopRequest) -> anyhow::Result<LocalExecStopResponse> {
        let error = "local exec stop denied by host policy".to_string();
        let mut status = ExecStatus::denied(error.clone());
        status.exec_id = Some(request.exec_id.clone());
        Ok(LocalExecStopResponse {
            exec_id: request.exec_id,
            status,
            error: Some(error),
        })
    }

    async fn status(
        &self,
        request: LocalExecStatusRequest,
    ) -> anyhow::Result<LocalExecStatusResponse> {
        let error = "local exec status denied by host policy".to_string();
        let mut status = ExecStatus::denied(error.clone());
        status.exec_id = Some(request.exec_id);
        Ok(LocalExecStatusResponse {
            status,
            error: Some(error),
        })
    }

    async fn logs(&self, request: LocalExecLogsRequest) -> anyhow::Result<LocalExecLogsResponse> {
        Ok(LocalExecLogsResponse {
            exec_id: request.exec_id,
            lines: Vec::new(),
            next_seq: None,
            error: Some("local exec logs denied by host policy".to_string()),
        })
    }
}

pub struct FakeLocalExecExecutor;

#[async_trait]
impl LocalExecExecutor for FakeLocalExecExecutor {
    async fn start(
        &self,
        request: LocalExecStartRequest,
    ) -> anyhow::Result<LocalExecStartResponse> {
        let exec_id = fake_exec_id(&request.target_id, &request.command.program);
        let status = ExecStatus {
            exec_id: Some(exec_id.clone()),
            target_id: Some(request.target_id),
            kind: ExecStatusKind::Running,
            exit_code: None,
            message: Some("fake local exec running".to_string()),
            ready: true,
        };
        Ok(LocalExecStartResponse {
            exec_id: Some(exec_id),
            status,
            error: None,
        })
    }

    async fn stop(&self, request: LocalExecStopRequest) -> anyhow::Result<LocalExecStopResponse> {
        Ok(LocalExecStopResponse {
            exec_id: request.exec_id.clone(),
            status: ExecStatus {
                exec_id: Some(request.exec_id.clone()),
                target_id: None,
                kind: ExecStatusKind::Stopped,
                exit_code: Some(0),
                message: Some("fake local exec stopped".to_string()),
                ready: false,
            },
            error: None,
        })
    }

    async fn status(
        &self,
        request: LocalExecStatusRequest,
    ) -> anyhow::Result<LocalExecStatusResponse> {
        Ok(LocalExecStatusResponse {
            status: ExecStatus {
                exec_id: Some(request.exec_id),
                target_id: None,
                kind: ExecStatusKind::Running,
                exit_code: None,
                message: Some("fake local exec running".to_string()),
                ready: true,
            },
            error: None,
        })
    }

    async fn logs(&self, request: LocalExecLogsRequest) -> anyhow::Result<LocalExecLogsResponse> {
        let since = request.since_seq.unwrap_or(0);
        let limit = request.limit.unwrap_or(100) as usize;
        let lines: Vec<_> = [
            LocalExecLogLine {
                seq: 1,
                stream: LocalExecLogStream::System,
                message_redacted: "fake local exec started".to_string(),
            },
            LocalExecLogLine {
                seq: 2,
                stream: LocalExecLogStream::Stdout,
                message_redacted: "fake local exec output".to_string(),
            },
        ]
        .into_iter()
        .filter(|line| line.seq > since)
        .take(limit)
        .collect();
        let next_seq = lines.last().map(|line| line.seq + 1);
        Ok(LocalExecLogsResponse {
            exec_id: request.exec_id,
            lines,
            next_seq,
            error: None,
        })
    }
}

fn fake_exec_id(target_id: &str, program: &str) -> ExecId {
    format!(
        "fake-exec-{}-{}",
        sanitize_id(target_id),
        sanitize_id(program)
    )
}

fn sanitize_id(input: &str) -> String {
    let sanitized: String = input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "id".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_existing(paths: &[&str]) -> Option<PathBuf> {
        paths.iter().map(PathBuf::from).find(|path| path.exists())
    }

    fn request(program: &Path, cwd: &Path, args: Vec<String>) -> LocalExecStartRequest {
        LocalExecStartRequest {
            target_id: "local".to_string(),
            command: ExecCommand {
                program: program.display().to_string(),
                args,
            },
            cwd: Some(cwd.to_path_buf()),
            env: BTreeMap::new(),
            lifecycle: ExecLifecyclePolicy::StopOnSessionClose,
            resource_limits: ExecResourceLimits {
                max_duration_ms: Some(1_000),
                max_log_bytes: Some(4_096),
                ..ExecResourceLimits::default()
            },
            readiness_probe: ReadinessProbe::default(),
            port_names: Vec::new(),
        }
    }

    #[tokio::test]
    async fn live_local_exec_runs_allowed_tiny_command() -> anyhow::Result<()> {
        let Some(echo) = first_existing(&["/bin/echo", "/usr/bin/echo"]) else {
            eprintln!("skipping: echo binary not found");
            return Ok(());
        };
        let tmp = tempfile::tempdir()?;
        let executor = LiveLocalExecExecutor::new(LiveLocalExecExecutorConfig::new(
            vec![echo.display().to_string()],
            vec![tmp.path().to_path_buf()],
            Vec::new(),
            5_000,
            4_096,
        )?)?;

        let response = executor
            .start(request(
                &echo,
                tmp.path(),
                vec!["hello-local-exec".to_string()],
            ))
            .await?;
        let exec_id = response.exec_id.expect("exec id");
        for _ in 0..20 {
            let status = executor
                .status(LocalExecStatusRequest {
                    exec_id: exec_id.clone(),
                })
                .await?
                .status;
            if status.kind == ExecStatusKind::Exited {
                assert_eq!(status.exit_code, Some(0));
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        for _ in 0..20 {
            let logs = executor
                .logs(LocalExecLogsRequest {
                    exec_id: exec_id.clone(),
                    since_seq: None,
                    limit: Some(20),
                })
                .await?;
            if logs
                .lines
                .iter()
                .any(|line| line.message_redacted.contains("hello-local-exec"))
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let logs = executor
            .logs(LocalExecLogsRequest {
                exec_id,
                since_seq: None,
                limit: Some(20),
            })
            .await?;
        assert!(logs
            .lines
            .iter()
            .any(|line| line.message_redacted.contains("hello-local-exec")));
        Ok(())
    }

    #[tokio::test]
    async fn live_local_exec_stop_preserves_natural_exit() -> anyhow::Result<()> {
        let Some(echo) = first_existing(&["/bin/echo", "/usr/bin/echo"]) else {
            eprintln!("skipping: echo binary not found");
            return Ok(());
        };
        let tmp = tempfile::tempdir()?;
        let executor = LiveLocalExecExecutor::new(LiveLocalExecExecutorConfig::new(
            vec![echo.display().to_string()],
            vec![tmp.path().to_path_buf()],
            Vec::new(),
            5_000,
            4_096,
        )?)?;
        let response = executor
            .start(request(&echo, tmp.path(), vec!["done".to_string()]))
            .await?;
        let exec_id = response.exec_id.expect("exec id");
        tokio::time::sleep(Duration::from_millis(50)).await;
        let stopped = executor
            .stop(LocalExecStopRequest {
                exec_id,
                reason: Some("after natural exit".to_string()),
            })
            .await?;
        assert_eq!(stopped.status.kind, ExecStatusKind::Exited);
        assert_eq!(stopped.status.exit_code, Some(0));
        Ok(())
    }

    #[tokio::test]
    async fn live_local_exec_rejects_disallowed_program_and_cwd() -> anyhow::Result<()> {
        let Some(echo) = first_existing(&["/bin/echo", "/usr/bin/echo"]) else {
            eprintln!("skipping: echo binary not found");
            return Ok(());
        };
        let tmp = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        let executor = LiveLocalExecExecutor::new(LiveLocalExecExecutorConfig::new(
            vec![echo.display().to_string()],
            vec![tmp.path().to_path_buf()],
            Vec::new(),
            5_000,
            4_096,
        )?)?;

        let disallowed_program = PathBuf::from("/definitely/not/allowed");
        let rejected = executor
            .start(request(&disallowed_program, tmp.path(), Vec::new()))
            .await?;
        assert_eq!(rejected.status.kind, ExecStatusKind::Denied);
        assert!(rejected.error.unwrap_or_default().contains("program"));

        let rejected = executor
            .start(request(&echo, outside.path(), Vec::new()))
            .await?;
        assert_eq!(rejected.status.kind, ExecStatusKind::Denied);
        assert!(rejected.error.unwrap_or_default().contains("cwd"));
        Ok(())
    }

    #[tokio::test]
    async fn proxy_set_ready_if_active_with_lease_is_guarded() {
        let registry = ProxyRouteRegistry::new();
        let response = registry
            .register(ProxyRouteRegisterRequest {
                route_id: Some("route-guard".to_string()),
                upstream: ProxyRouteUpstream {
                    port_lease_id: "lease-a".to_string(),
                    port_name: "web".to_string(),
                },
                protocol: ProxyProtocol::Http,
                access: ProxyRouteAccess::HostAuthenticated,
            })
            .await;
        assert!(!response.route.ready);

        assert!(registry
            .set_ready_if_active_with_lease("route-guard", "lease-b", true)
            .await
            .is_none());
        assert!(!registry.status("route-guard").await.unwrap().ready);

        let updated = registry
            .set_ready_if_active_with_lease("route-guard", "lease-a", true)
            .await
            .expect("matching active route updates");
        assert!(updated.ready);

        registry.unregister("route-guard").await;
        assert!(registry
            .set_ready_if_active_with_lease("route-guard", "lease-a", false)
            .await
            .is_none());
        assert!(registry.status("route-guard").await.unwrap().ready);
    }

    #[tokio::test]
    async fn actual_port_binding_preserves_target_lease_ownership() {
        let registry = PortLeaseRegistry::new();
        let leased = registry
            .lease(PortLeaseRequest {
                target_id: "remote-1".to_string(),
                port_name: "http".to_string(),
                protocol: PortProtocol::Tcp,
                requested_port: Some(40_001),
            })
            .await
            .lease;
        assert!(registry
            .bind_actual_port(&leased.id, "remote-2", "http", 49_152)
            .await
            .is_none());
        let bound = registry
            .bind_actual_port(&leased.id, "remote-1", "http", 49_152)
            .await
            .expect("matching target lease binds the observed port");
        assert_eq!(bound.port, 49_152);
        assert_eq!(bound.status, PortLeaseStatusKind::Active);
        registry.release(&leased.id).await;
        assert!(registry
            .bind_actual_port(&leased.id, "remote-1", "http", 49_153)
            .await
            .is_none());
    }

    #[tokio::test]
    async fn local_target_supports_verified_artifact_deployment() {
        let target = ExecutionTargetRegistry::new()
            .status("local")
            .await
            .expect("built-in local target");
        for capability in [
            ExecutionTargetCapability::ArtifactTransfer,
            ExecutionTargetCapability::DeclarativeVerifier,
            ExecutionTargetCapability::Deployment,
        ] {
            assert!(target.declared_capabilities.contains(&capability));
            assert!(target.capabilities.contains(&capability));
        }
    }

    #[tokio::test]
    async fn live_local_exec_timeout_kills_long_running_command() -> anyhow::Result<()> {
        let Some(sleep) = first_existing(&["/bin/sleep", "/usr/bin/sleep"]) else {
            eprintln!("skipping: sleep binary not found");
            return Ok(());
        };
        let tmp = tempfile::tempdir()?;
        let executor = LiveLocalExecExecutor::new(LiveLocalExecExecutorConfig::new(
            vec![sleep.display().to_string()],
            vec![tmp.path().to_path_buf()],
            Vec::new(),
            100,
            4_096,
        )?)?;
        let response = executor
            .start(request(&sleep, tmp.path(), vec!["5".to_string()]))
            .await?;
        let exec_id = response.exec_id.expect("exec id");
        tokio::time::sleep(Duration::from_millis(250)).await;
        let status = executor
            .status(LocalExecStatusRequest { exec_id })
            .await?
            .status;
        assert_eq!(status.kind, ExecStatusKind::Failed);
        assert!(status.message.unwrap_or_default().contains("timed out"));
        Ok(())
    }
}

fn validate_allowed_programs(programs: &[String]) -> anyhow::Result<()> {
    if programs.is_empty() {
        anyhow::bail!("local exec allowed_programs must not be empty");
    }
    for program in programs {
        if program.trim().is_empty() || program.contains('*') {
            anyhow::bail!("local exec allowed_programs must not contain empty or wildcard entries");
        }
        let path = Path::new(program);
        if path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            anyhow::bail!(
                "local exec allowed_programs must not contain parent-directory components"
            );
        }
        if path.is_absolute() && !path.exists() {
            anyhow::bail!(
                "local exec allowed program path does not exist: {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn validate_allowed_working_dirs(dirs: &[PathBuf]) -> anyhow::Result<()> {
    if dirs.is_empty() {
        anyhow::bail!("local exec allowed_working_dirs must not be empty");
    }
    for dir in dirs {
        let raw = dir.to_string_lossy();
        if raw.trim().is_empty() || raw.contains('*') {
            anyhow::bail!(
                "local exec allowed_working_dirs must not contain empty or wildcard entries"
            );
        }
        if dir
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            anyhow::bail!(
                "local exec allowed_working_dirs must not contain parent-directory components"
            );
        }
        if !dir.exists() {
            anyhow::bail!(
                "local exec allowed working dir does not exist: {}",
                dir.display()
            );
        }
    }
    Ok(())
}

fn validate_allowed_env_vars(keys: &[String]) -> anyhow::Result<()> {
    for key in keys {
        if key.trim().is_empty() || key.contains('=') || key.contains('*') {
            anyhow::bail!("local exec allowed_env_vars must not contain empty or wildcard entries");
        }
    }
    Ok(())
}

async fn read_live_log_stream<R>(
    state: Arc<LiveExecState>,
    mut reader: R,
    stream: LocalExecLogStream,
    redaction_values: Vec<String>,
) where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buffer = [0_u8; 4096];
    loop {
        let Ok(read) = reader.read(&mut buffer).await else {
            break;
        };
        if read == 0 {
            break;
        }
        let message = String::from_utf8_lossy(&buffer[..read]).to_string();
        append_live_log(&state, stream, &message, &redaction_values).await;
    }
}

async fn append_live_log(
    state: &LiveExecState,
    stream: LocalExecLogStream,
    message: &str,
    redaction_values: &[String],
) {
    let mut redacted = redact_live_log_message(message, redaction_values);
    let remaining = remaining_log_bytes(state);
    if remaining == 0 {
        return;
    }
    let mut bytes = redacted.into_bytes();
    if bytes.len() > remaining {
        bytes.truncate(remaining);
        bytes.extend_from_slice(b"...[truncated]");
    }
    let consumed = bytes.len() as u64;
    let previous = state.log_bytes.fetch_add(consumed, Ordering::SeqCst);
    if previous >= state.max_log_bytes {
        return;
    }
    let allowed = (state.max_log_bytes - previous) as usize;
    if bytes.len() > allowed {
        bytes.truncate(allowed);
    }
    redacted = String::from_utf8_lossy(&bytes).to_string();
    let seq = state.next_seq.fetch_add(1, Ordering::SeqCst);
    state.logs.write().await.push(LocalExecLogLine {
        seq,
        stream,
        message_redacted: redacted,
    });
}

fn remaining_log_bytes(state: &LiveExecState) -> usize {
    let used = state.log_bytes.load(Ordering::SeqCst);
    state.max_log_bytes.saturating_sub(used) as usize
}

fn redact_live_log_message(message: &str, redaction_values: &[String]) -> String {
    let mut redacted = message.to_string();
    for value in redaction_values {
        if !value.is_empty() {
            redacted = redacted.replace(value, "[REDACTED]");
        }
    }
    for token in redacted
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>()
    {
        let lowered = token.to_ascii_lowercase();
        if lowered.contains("secret")
            || lowered.contains("token")
            || lowered.contains("password")
            || lowered.contains("apikey")
            || lowered.contains("api_key")
        {
            redacted = redacted.replace(&token, "[REDACTED]");
        }
    }
    redacted
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionTargetReachability {
    LocalHost,
    Direct,
    ReverseTunnel,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionTargetCapability {
    LocalExec,
    PortLease,
    HttpProxyUpstream,
    WebsocketProxyUpstream,
    ArtifactTransfer,
    DeclarativeVerifier,
    HealthProbe,
    Deployment,
    AuthenticatedTunnel,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionTargetStatusKind {
    Enrolling,
    Available,
    Degraded,
    Offline,
    Draining,
    Incompatible,
    Revoked,
    // Kept for wire compatibility with older local target adapters.
    Unavailable,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExecutionTargetObservedSummary {
    #[serde(default)]
    pub running_operation_count: u64,
    #[serde(default)]
    pub workload_count: u64,
    #[serde(default)]
    pub artifact_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExecutionTarget {
    pub id: ExecutionTargetId,
    pub name: String,
    pub reachability: ExecutionTargetReachability,
    /// Capabilities declared by the target before Host policy is applied.
    #[serde(default)]
    pub declared_capabilities: Vec<ExecutionTargetCapability>,
    /// Effective capabilities after protocol negotiation and Host policy.
    #[serde(default)]
    pub capabilities: Vec<ExecutionTargetCapability>,
    pub status: ExecutionTargetStatusKind,
    #[serde(default)]
    pub protocol_versions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_protocol_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_ref: Option<String>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<ExecutionTargetObservedSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat_expires_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrolled_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at_ms: Option<i64>,
    #[serde(default)]
    pub lease_epoch: u64,
    #[serde(default)]
    pub policy_epoch: u64,
}

pub struct ExecutionTargetRegistry {
    targets: RwLock<HashMap<ExecutionTargetId, ExecutionTarget>>,
}

impl ExecutionTargetRegistry {
    pub fn new() -> Self {
        let capabilities = vec![
            ExecutionTargetCapability::LocalExec,
            ExecutionTargetCapability::PortLease,
            ExecutionTargetCapability::HttpProxyUpstream,
            ExecutionTargetCapability::WebsocketProxyUpstream,
            ExecutionTargetCapability::ArtifactTransfer,
            ExecutionTargetCapability::DeclarativeVerifier,
            ExecutionTargetCapability::HealthProbe,
            ExecutionTargetCapability::Deployment,
        ];
        let target = ExecutionTarget {
            id: "local".to_string(),
            name: "local-host".to_string(),
            reachability: ExecutionTargetReachability::LocalHost,
            declared_capabilities: capabilities.clone(),
            capabilities,
            status: ExecutionTargetStatusKind::Available,
            protocol_versions: vec!["local.v1".to_string()],
            selected_protocol_version: Some("local.v1".to_string()),
            identity_ref: Some("host:local".to_string()),
            labels: BTreeMap::new(),
            observed: None,
            last_seen_at_ms: None,
            heartbeat_expires_at_ms: None,
            enrolled_at_ms: None,
            revoked_at_ms: None,
            lease_epoch: 1,
            policy_epoch: 1,
        };
        let mut targets = HashMap::new();
        targets.insert(target.id.clone(), target);
        Self {
            targets: RwLock::new(targets),
        }
    }

    pub async fn list(&self) -> Vec<ExecutionTarget> {
        let now_ms = Utc::now().timestamp_millis();
        self.targets
            .read()
            .await
            .values()
            .cloned()
            .map(|target| effective_target_status(target, now_ms))
            .collect()
    }

    pub async fn status(&self, target_id: &str) -> Option<ExecutionTarget> {
        let now_ms = Utc::now().timestamp_millis();
        self.targets
            .read()
            .await
            .get(target_id)
            .cloned()
            .map(|target| effective_target_status(target, now_ms))
    }

    /// Updates the Host-owned read projection. This is not an enrollment or
    /// authorization boundary and must never be used to permit target effects.
    #[doc(hidden)]
    pub async fn replace_control_plane_projection(
        &self,
        target: ExecutionTarget,
    ) -> Option<ExecutionTarget> {
        self.targets.write().await.insert(target.id.clone(), target)
    }

    /// Removes a Host-owned read projection without changing durable target identity.
    #[doc(hidden)]
    pub async fn remove_control_plane_projection(
        &self,
        target_id: &str,
    ) -> Option<ExecutionTarget> {
        self.targets.write().await.remove(target_id)
    }
}

fn effective_target_status(mut target: ExecutionTarget, now_ms: i64) -> ExecutionTarget {
    if target.reachability != ExecutionTargetReachability::LocalHost
        && matches!(
            target.status,
            ExecutionTargetStatusKind::Available | ExecutionTargetStatusKind::Degraded
        )
        && target
            .heartbeat_expires_at_ms
            .is_none_or(|expires_at_ms| expires_at_ms <= now_ms)
    {
        target.status = ExecutionTargetStatusKind::Offline;
    }
    target
}

impl Default for ExecutionTargetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PortProtocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PortBindScope {
    LoopbackOnly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PortLeaseStatusKind {
    Active,
    Reserved,
    Released,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PortLeaseRequest {
    pub target_id: ExecutionTargetId,
    pub port_name: String,
    #[serde(default = "default_port_protocol")]
    pub protocol: PortProtocol,
    pub requested_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PortLeaseRecord {
    pub id: PortLeaseId,
    pub target_id: ExecutionTargetId,
    pub port_name: String,
    pub host: String,
    pub port: u16,
    pub protocol: PortProtocol,
    pub bind: PortBindScope,
    pub status: PortLeaseStatusKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PortLeaseResponse {
    pub lease: PortLeaseRecord,
}

pub struct PortLeaseRegistry {
    leases: RwLock<HashMap<PortLeaseId, PortLeaseRecord>>,
    next_id: AtomicU64,
}

impl PortLeaseRegistry {
    pub fn new() -> Self {
        Self {
            leases: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(0),
        }
    }

    pub async fn lease(&self, request: PortLeaseRequest) -> PortLeaseResponse {
        let sequence = self.next_id.fetch_add(1, Ordering::SeqCst);
        let record = PortLeaseRecord {
            id: format!("port-lease-{sequence:06}"),
            target_id: request.target_id,
            port_name: request.port_name,
            host: "127.0.0.1".to_string(),
            port: request.requested_port.unwrap_or(39_000 + sequence as u16),
            protocol: request.protocol,
            bind: PortBindScope::LoopbackOnly,
            status: PortLeaseStatusKind::Active,
        };
        self.leases
            .write()
            .await
            .insert(record.id.clone(), record.clone());
        PortLeaseResponse { lease: record }
    }

    pub async fn release(&self, lease_id: &str) -> Option<PortLeaseRecord> {
        let mut leases = self.leases.write().await;
        let lease = leases.get_mut(lease_id)?;
        lease.status = PortLeaseStatusKind::Released;
        Some(lease.clone())
    }

    pub async fn restore(&self, record: PortLeaseRecord) {
        bump_restored_sequence(&self.next_id, &record.id, "port-lease-");
        self.leases.write().await.insert(record.id.clone(), record);
    }

    pub async fn status(&self, lease_id: &str) -> Option<PortLeaseRecord> {
        self.leases.read().await.get(lease_id).cloned()
    }

    pub async fn set_status(
        &self,
        lease_id: &str,
        status: PortLeaseStatusKind,
    ) -> Option<PortLeaseRecord> {
        let mut leases = self.leases.write().await;
        let lease = leases.get_mut(lease_id)?;
        lease.status = status;
        Some(lease.clone())
    }

    pub async fn bind_actual_port(
        &self,
        lease_id: &str,
        target_id: &str,
        port_name: &str,
        port: u16,
    ) -> Option<PortLeaseRecord> {
        if port == 0 {
            return None;
        }
        let mut leases = self.leases.write().await;
        let lease = leases.get_mut(lease_id)?;
        if lease.target_id != target_id
            || lease.port_name != port_name
            || lease.protocol != PortProtocol::Tcp
            || lease.bind != PortBindScope::LoopbackOnly
            || lease.host != "127.0.0.1"
            || lease.status == PortLeaseStatusKind::Released
        {
            return None;
        }
        lease.port = port;
        lease.status = PortLeaseStatusKind::Active;
        Some(lease.clone())
    }

    pub async fn list(&self) -> Vec<PortLeaseRecord> {
        self.leases.read().await.values().cloned().collect()
    }
}

impl Default for PortLeaseRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn default_port_protocol() -> PortProtocol {
    PortProtocol::Tcp
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProxyProtocol {
    Http,
    Websocket,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProxyRouteStatusKind {
    Active,
    Stale,
    Removed,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProxyRouteAccess {
    #[default]
    HostAuthenticated,
    Public,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProxyRouteUpstream {
    pub port_lease_id: PortLeaseId,
    pub port_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProxyRouteRegisterRequest {
    pub route_id: Option<ProxyRouteId>,
    pub upstream: ProxyRouteUpstream,
    #[serde(default = "default_proxy_protocol")]
    pub protocol: ProxyProtocol,
    #[serde(default)]
    pub access: ProxyRouteAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProxyRouteRecord {
    pub id: ProxyRouteId,
    pub upstream: ProxyRouteUpstream,
    pub protocol: ProxyProtocol,
    #[serde(default)]
    pub access: ProxyRouteAccess,
    pub public_url: String,
    pub iframe_url: String,
    pub status: ProxyRouteStatusKind,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProxyRouteRegisterResponse {
    pub route: ProxyRouteRecord,
}

pub struct ProxyRouteRegistry {
    routes: RwLock<HashMap<ProxyRouteId, ProxyRouteRecord>>,
    next_id: AtomicU64,
}

impl ProxyRouteRegistry {
    pub fn new() -> Self {
        Self {
            routes: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(0),
        }
    }

    pub async fn register(&self, request: ProxyRouteRegisterRequest) -> ProxyRouteRegisterResponse {
        let sequence = self.next_id.fetch_add(1, Ordering::SeqCst);
        let id = request
            .route_id
            .unwrap_or_else(|| format!("proxy-route-{sequence:06}"));
        let route = ProxyRouteRecord {
            public_url: format!("/p/{id}/"),
            iframe_url: format!("/p/{id}/"),
            id: id.clone(),
            upstream: request.upstream,
            protocol: request.protocol,
            access: request.access,
            status: ProxyRouteStatusKind::Active,
            ready: false,
        };
        self.routes.write().await.insert(id, route.clone());
        ProxyRouteRegisterResponse { route }
    }

    pub async fn unregister(&self, route_id: &str) -> Option<ProxyRouteRecord> {
        let mut routes = self.routes.write().await;
        let route = routes.get_mut(route_id)?;
        route.status = ProxyRouteStatusKind::Removed;
        Some(route.clone())
    }

    pub async fn restore(&self, record: ProxyRouteRecord) {
        bump_restored_sequence(&self.next_id, &record.id, "proxy-route-");
        self.routes.write().await.insert(record.id.clone(), record);
    }

    pub async fn status(&self, route_id: &str) -> Option<ProxyRouteRecord> {
        self.routes.read().await.get(route_id).cloned()
    }

    pub async fn set_status(
        &self,
        route_id: &str,
        status: ProxyRouteStatusKind,
    ) -> Option<ProxyRouteRecord> {
        let mut routes = self.routes.write().await;
        let route = routes.get_mut(route_id)?;
        route.status = status;
        Some(route.clone())
    }

    pub async fn set_ready(&self, route_id: &str, ready: bool) -> Option<ProxyRouteRecord> {
        let mut routes = self.routes.write().await;
        let route = routes.get_mut(route_id)?;
        route.ready = ready;
        Some(route.clone())
    }

    pub async fn set_ready_if_active_with_lease(
        &self,
        route_id: &str,
        port_lease_id: &str,
        ready: bool,
    ) -> Option<ProxyRouteRecord> {
        let mut routes = self.routes.write().await;
        let route = routes.get_mut(route_id)?;
        if route.status != ProxyRouteStatusKind::Active
            || route.upstream.port_lease_id != port_lease_id
        {
            return None;
        }
        route.ready = ready;
        Some(route.clone())
    }

    pub async fn list(&self) -> Vec<ProxyRouteRecord> {
        self.routes.read().await.values().cloned().collect()
    }
}

impl Default for ProxyRouteRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn default_proxy_protocol() -> ProxyProtocol {
    ProxyProtocol::Http
}

fn bump_restored_sequence(next_id: &AtomicU64, id: &str, prefix: &str) {
    let Some(suffix) = id.strip_prefix(prefix) else {
        return;
    };
    let Ok(sequence) = suffix.parse::<u64>() else {
        return;
    };
    let desired_next = sequence.saturating_add(1);
    let _ = next_id.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
        (desired_next > current).then_some(desired_next)
    });
}
