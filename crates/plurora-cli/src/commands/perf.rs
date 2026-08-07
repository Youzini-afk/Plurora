use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use plurora_runtime::{
    AppendEventRequest, CapabilityInvocationRequest, EventStore, FakeOutboundExecutor,
    InMemoryEventStore, OpenSessionRequest, OutboundExecutePolicyConfig, OutboundExecutorConfig,
    OutboundExecutorRequest, OutboundRequest, ProtocolContext, ProtocolPrincipal, Runtime,
    RuntimeConfig,
};
use serde::Serialize;
use serde_json::json;

use crate::cli::BaselineFormat;
use crate::commands::manifest::read_manifest;

const SUBPROCESS_ECHO_MANIFEST: &str = "examples/packages/echo-subprocess-python/manifest.yaml";
const SUBPROCESS_ECHO_PACKAGE_ID: &str = "example/echo-subprocess-python";
const SUBPROCESS_ECHO_CAPABILITY_ID: &str = "example/echo-subprocess-python/echo";
const PERF_OUTBOUND_PACKAGE_ID: &str = "example/perf-outbound";
const PERF_OUTBOUND_CAPABILITY_ID: &str = "example/perf-outbound/fetch";
const PERF_OUTBOUND_HOST: &str = "api.example.com";

#[derive(Debug, Clone)]
pub(crate) struct BaselineOptions {
    pub iterations: u32,
    pub warmup: u32,
    pub format: BaselineFormat,
    pub baseline_out: Option<PathBuf>,
    pub compare: Option<PathBuf>,
    pub threshold_pct: f64,
}

// ── Scenario result ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
struct ScenarioResult {
    scenario_id: String,
    iterations: u32,
    total_ms: f64,
    avg_ms: f64,
    min_ms: f64,
    max_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    p50_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    p95_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    p99_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_rss_mb_delta: Option<f64>,
    #[serde(default, skip_serializing_if = "is_default")]
    iterations_capped: bool,
    status: String,
    notes: Vec<String>,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    v == &T::default()
}

#[derive(Debug, Serialize)]
pub struct EnvInfo {
    pub os: String,
    pub target_triple: String,
    pub num_cpus: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rustc_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_brand: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct GitInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ComparisonResult {
    scenario_id: String,
    baseline_avg_ms: f64,
    current_avg_ms: f64,
    delta_pct: f64,
    regression: bool,
}

#[derive(Debug, Serialize)]
struct BenchMeta {
    iterations: u32,
    warmup: u32,
    tool: &'static str,
    version: &'static str,
    note: &'static str,
    ok_count: usize,
    skipped_count: usize,
    error_count: usize,
}

#[derive(Debug, Serialize)]
struct BenchEnvelope {
    schema: &'static str,
    created_at: u64,
    git: GitInfo,
    env: EnvInfo,
    baseline: Vec<ScenarioResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    comparisons: Option<Vec<ComparisonResult>>,
    meta: BenchMeta,
}

// ── Helpers ────────────────────────────────────────────────────────────

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn manifest_path(rel: &str) -> PathBuf {
    workspace_root().join(rel)
}

fn build_result(
    scenario_id: &str,
    iterations: u32,
    samples: &[f64],
    status: &str,
    notes: Vec<String>,
    memory_rss_mb_delta: Option<f64>,
    iterations_capped: bool,
) -> ScenarioResult {
    let total_ms: f64 = samples.iter().sum();
    let avg_ms = if samples.is_empty() {
        0.0
    } else {
        total_ms / samples.len() as f64
    };
    let (min_ms, p50_ms, p95_ms, p99_ms, max_ms) = compute_percentiles(samples);
    let percentile_fields = (!samples.is_empty()).then_some((p50_ms, p95_ms, p99_ms));
    ScenarioResult {
        scenario_id: scenario_id.to_string(),
        iterations,
        total_ms,
        avg_ms,
        min_ms,
        max_ms,
        p50_ms: percentile_fields.map(|p| p.0),
        p95_ms: percentile_fields.map(|p| p.1),
        p99_ms: percentile_fields.map(|p| p.2),
        memory_rss_mb_delta,
        iterations_capped,
        status: status.to_string(),
        notes,
    }
}

fn error_result(
    scenario_id: &'static str,
    iterations: u32,
    err: impl std::fmt::Display,
) -> ScenarioResult {
    build_result(
        scenario_id,
        iterations,
        &[],
        "error",
        vec![format!("error: {err}")],
        None,
        false,
    )
}

fn skipped_result(scenario_id: &'static str, iterations: u32, reason: &str) -> ScenarioResult {
    build_result(
        scenario_id,
        iterations,
        &[],
        "skipped",
        vec![reason.to_string()],
        None,
        false,
    )
}

fn compute_percentiles(samples: &[f64]) -> (f64, f64, f64, f64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0, 0.0, 0.0);
    }

    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pick = |p: f64| -> f64 {
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    };
    (
        sorted[0],
        pick(50.0),
        pick(95.0),
        pick(99.0),
        sorted[sorted.len() - 1],
    )
}

fn read_rss_mb() -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        let s = std::fs::read_to_string("/proc/self/status").ok()?;
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                let kb: f64 = rest.split_whitespace().next()?.parse().ok()?;
                return Some(kb / 1024.0);
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

fn rss_delta(before: Option<f64>, after: Option<f64>) -> Option<f64> {
    Some(after? - before?)
}

fn collect_env() -> EnvInfo {
    EnvInfo {
        os: std::env::consts::OS.to_string(),
        target_triple: std::env::var("TARGET").unwrap_or_else(|_| {
            format!(
                "{}-{}-{}",
                std::env::consts::ARCH,
                std::env::consts::OS,
                std::env::consts::FAMILY
            )
        }),
        num_cpus: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
        rustc_version: option_env!("RUSTC_VERSION").map(String::from),
        cpu_brand: read_cpu_brand(),
    }
}

fn read_cpu_brand() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let s = std::fs::read_to_string("/proc/cpuinfo").ok()?;
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("model name") {
                return Some(
                    rest.trim_start_matches(|c: char| c == ':' || c.is_whitespace())
                        .to_string(),
                );
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

fn collect_git() -> GitInfo {
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        });
    let branch = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        });
    let dirty = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);
    GitInfo {
        commit,
        branch,
        dirty,
    }
}

fn now_secs_since_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn run_async_safely<F>(future: F) -> F::Output
where
    F: std::future::Future,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(future)
    }
}

// ── Scenarios ──────────────────────────────────────────────────────────

async fn scenario_inproc_echo_invoke(iterations: u32, warmup: u32) -> ScenarioResult {
    let manifest_path = manifest_path("examples/packages/echo-rust-inproc/manifest.yaml");
    let manifest = match read_manifest(manifest_path).await {
        Ok(m) => m,
        Err(e) => return error_result("inproc_echo_invoke", iterations, e),
    };
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());
    if let Err(e) = runtime.load_package(manifest).await {
        return error_result("inproc_echo_invoke", iterations, e);
    }

    for _ in 0..warmup {
        if let Err(e) = scenario_inproc_echo_invoke_sample(&runtime) {
            return error_result("inproc_echo_invoke", iterations, e);
        }
    }

    let before_rss = read_rss_mb();
    let mut durations = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        match scenario_inproc_echo_invoke_sample(&runtime) {
            Ok(ms) => durations.push(ms),
            Err(e) => return error_result("inproc_echo_invoke", iterations, e),
        }
    }
    let memory_delta = rss_delta(before_rss, read_rss_mb());
    build_result(
        "inproc_echo_invoke",
        iterations,
        &durations,
        "ok",
        vec![],
        memory_delta,
        false,
    )
}

fn scenario_inproc_echo_invoke_sample<S>(runtime: &Runtime<S>) -> Result<f64>
where
    S: EventStore,
{
    run_async_safely(async {
        let start = Instant::now();
        runtime
            .invoke_capability(CapabilityInvocationRequest {
                handle: None,
                capability_id: Some("example/echo-rust-inproc/echo".to_string()),
                caller_package_id: None,
                provider_package_id: None,
                version: None,
                session_id: None,
                input: json!({"baseline": true}),
            })
            .await?;
        Ok(start.elapsed().as_secs_f64() * 1000.0)
    })
}

async fn scenario_official_capability_invoke(iterations: u32, warmup: u32) -> ScenarioResult {
    let manifest_path = manifest_path("packages/official/composition-lab/manifest.yaml");
    let manifest = match read_manifest(manifest_path).await {
        Ok(m) => m,
        Err(e) => return error_result("official_capability_invoke", iterations, e),
    };
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());
    if let Err(e) = runtime.load_package(manifest).await {
        return error_result("official_capability_invoke", iterations, e);
    }

    for _ in 0..warmup {
        if let Err(e) = scenario_official_capability_invoke_sample(&runtime) {
            return error_result("official_capability_invoke", iterations, e);
        }
    }

    let before_rss = read_rss_mb();
    let mut durations = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        match scenario_official_capability_invoke_sample(&runtime) {
            Ok(ms) => durations.push(ms),
            Err(e) => return error_result("official_capability_invoke", iterations, e),
        }
    }
    let memory_delta = rss_delta(before_rss, read_rss_mb());
    build_result(
        "official_capability_invoke",
        iterations,
        &durations,
        "ok",
        vec!["official/composition-lab/describe".to_string()],
        memory_delta,
        false,
    )
}

fn scenario_official_capability_invoke_sample<S>(runtime: &Runtime<S>) -> Result<f64>
where
    S: EventStore,
{
    run_async_safely(async {
        let start = Instant::now();
        runtime
            .invoke_capability(CapabilityInvocationRequest {
                handle: None,
                capability_id: Some("official/composition-lab/describe".to_string()),
                caller_package_id: None,
                provider_package_id: None,
                version: None,
                session_id: None,
                input: json!({}),
            })
            .await?;
        Ok(start.elapsed().as_secs_f64() * 1000.0)
    })
}

async fn scenario_event_store_append_list_range(iterations: u32, warmup: u32) -> ScenarioResult {
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store.clone(), RuntimeConfig::default());

    let manifest_path = manifest_path("examples/packages/echo-rust-inproc/manifest.yaml");
    let manifest = match read_manifest(manifest_path).await {
        Ok(m) => m,
        Err(e) => return error_result("event_store_append_list_range", iterations, e),
    };
    if let Err(e) = runtime.load_package(manifest).await {
        return error_result("event_store_append_list_range", iterations, e);
    }
    let session = match runtime.open_session(OpenSessionRequest::default()).await {
        Ok(s) => s,
        Err(e) => return error_result("event_store_append_list_range", iterations, e),
    };

    let event_count: u32 = 100;
    // This scenario intentionally keeps one store/session across iterations, so
    // warmup uses the same body and is excluded from samples while preserving
    // the cumulative-store behavior of measured iterations.
    for _ in 0..warmup {
        if let Err(e) = scenario_event_store_append_list_range_sample(
            &runtime,
            &store,
            &session.id,
            event_count,
        ) {
            return error_result("event_store_append_list_range", iterations, e);
        }
    }

    let before_rss = read_rss_mb();
    let mut durations = Vec::with_capacity(iterations as usize);

    for _ in 0..iterations {
        match scenario_event_store_append_list_range_sample(
            &runtime,
            &store,
            &session.id,
            event_count,
        ) {
            Ok(ms) => durations.push(ms),
            Err(e) => return error_result("event_store_append_list_range", iterations, e),
        }
    }

    let memory_delta = rss_delta(before_rss, read_rss_mb());
    build_result(
        "event_store_append_list_range",
        iterations,
        &durations,
        "ok",
        vec![format!("{event_count} events per iteration")],
        memory_delta,
        false,
    )
}

fn scenario_event_store_append_list_range_sample(
    runtime: &Runtime<InMemoryEventStore>,
    store: &Arc<InMemoryEventStore>,
    session_id: &str,
    event_count: u32,
) -> Result<f64> {
    run_async_safely(async {
        let session_id = session_id.to_string();
        let start = Instant::now();
        for i in 0..event_count {
            runtime
                .append_event(AppendEventRequest {
                    session_id: session_id.clone(),
                    writer_package_id: "example/echo-rust-inproc".to_string(),
                    kind: "example/echo-rust-inproc/baseline.event".to_string(),
                    payload: json!({"i": i}),
                    metadata: json!({}),
                })
                .await?;
        }

        let events = store.list_session(&session_id).await?;
        if events.len() < event_count as usize {
            anyhow::bail!("list_session returned too few events");
        }

        let range_after = if events.len() > 10 {
            (events.len() - 10) as u64
        } else {
            0
        };
        let range = store
            .list_session_range(&session_id, Some(range_after), Some(10))
            .await?;
        if range.is_empty() {
            anyhow::bail!("range query returned empty");
        }

        Ok(start.elapsed().as_secs_f64() * 1000.0)
    })
}

/// Generic event scale scenario for 1k/10k/100k event counts.
/// Uses store-level atomic append directly for maximum throughput.
async fn scenario_event_store_scale(
    event_count: u32,
    iterations: u32,
    warmup: u32,
    scenario_id: &'static str,
) -> ScenarioResult {
    for iteration in 0..warmup {
        let session_id = format!("ses_scale_{}_warmup_{}", event_count, iteration);
        if let Err(e) = scenario_event_store_scale_sample(event_count, session_id) {
            return error_result(scenario_id, iterations, e);
        }
    }

    let before_rss = read_rss_mb();
    let mut durations = Vec::with_capacity(iterations as usize);

    for iteration in 0..iterations {
        let session_id = format!("ses_scale_{}_{}", event_count, iteration);
        match scenario_event_store_scale_sample(event_count, session_id) {
            Ok(ms) => durations.push(ms),
            Err(e) => return error_result(scenario_id, iterations, e),
        }
    }

    let memory_delta = rss_delta(before_rss, read_rss_mb());
    build_result(
        scenario_id,
        iterations,
        &durations,
        "ok",
        vec![format!("{event_count} events per iteration")],
        memory_delta,
        false,
    )
}

fn scenario_event_store_scale_sample(event_count: u32, session_id: String) -> Result<f64> {
    run_async_safely(async move {
        let store = Arc::new(InMemoryEventStore::default());
        let start = Instant::now();

        for i in 0..event_count {
            store
                .append_with_sequence(
                    session_id.clone(),
                    "example/echo-rust-inproc".to_string(),
                    "example/echo-rust-inproc/scale.event".to_string(),
                    1,
                    json!({"i": i}),
                    json!({}),
                )
                .await?;
        }

        let events = store.list_session(&session_id).await?;
        if events.len() < event_count as usize {
            anyhow::bail!("list_session returned too few events");
        }

        let prefix_events = store
            .list_session_kind_prefix(&session_id, "example/echo-rust-inproc/scale")
            .await?;
        if prefix_events.len() < event_count as usize {
            anyhow::bail!("kind-prefix query returned too few events");
        }

        Ok(start.elapsed().as_secs_f64() * 1000.0)
    })
}

async fn scenario_composition_check(iterations: u32, warmup: u32) -> ScenarioResult {
    let composition_path =
        manifest_path("examples/compositions/playable-seed-replacement/composition.yaml");
    let raw = match fs::read_to_string(&composition_path) {
        Ok(r) => r,
        Err(e) => return error_result("composition_check", iterations, e),
    };
    let composition: crate::cli::CompositionDescriptor =
        match composition_path.extension().and_then(|ext| ext.to_str()) {
            Some("yaml") | Some("yml") => match serde_yaml::from_str(&raw) {
                Ok(c) => c,
                Err(e) => return error_result("composition_check", iterations, e),
            },
            _ => match serde_json::from_str(&raw) {
                Ok(c) => c,
                Err(e) => return error_result("composition_check", iterations, e),
            },
        };

    let base = composition_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));

    for _ in 0..warmup {
        if let Err(e) = run_composition_check_body(&composition, base).await {
            return error_result("composition_check", iterations, e);
        }
    }

    let before_rss = read_rss_mb();
    let mut durations = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        match run_composition_check_body(&composition, base).await {
            Ok(ms) => durations.push(ms),
            Err(e) => return error_result("composition_check", iterations, e),
        }
    }

    let memory_delta = rss_delta(before_rss, read_rss_mb());
    build_result(
        "composition_check",
        iterations,
        &durations,
        "ok",
        vec!["playable-seed-replacement".to_string()],
        memory_delta,
        false,
    )
}

async fn run_composition_check_body(
    composition: &crate::cli::CompositionDescriptor,
    base: &Path,
) -> Result<f64> {
    let start = Instant::now();

    // Validate fields
    if composition.id.trim().is_empty() {
        anyhow::bail!("composition id empty");
    }
    if composition.entry_surface_id.trim().is_empty() {
        anyhow::bail!("entry surface empty");
    }

    // Load required packages
    for pkg_path in &composition.packages {
        let resolved = if pkg_path.is_absolute() {
            pkg_path.clone()
        } else {
            base.join(pkg_path)
        };
        let manifest = read_manifest(resolved).await?;
        manifest.validate_basic()?;
    }

    Ok(start.elapsed().as_secs_f64() * 1000.0)
}

fn scenario_profile_load_sample(profile_path: &Path) -> Result<f64> {
    let start = Instant::now();
    let raw = fs::read_to_string(profile_path)?;
    let _profile: crate::cli::HostProfile = serde_yaml::from_str(&raw)?;
    Ok(start.elapsed().as_secs_f64() * 1000.0)
}

async fn scenario_profile_load(iterations: u32, warmup: u32) -> ScenarioResult {
    let profile_path = manifest_path("profiles/forge-alpha.yaml");

    for _ in 0..warmup {
        if let Err(e) = scenario_profile_load_sample(&profile_path) {
            return error_result("profile_load", iterations, e);
        }
    }

    let before_rss = read_rss_mb();
    let mut durations = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        match scenario_profile_load_sample(&profile_path) {
            Ok(ms) => durations.push(ms),
            Err(e) => return error_result("profile_load", iterations, e),
        }
    }

    let memory_delta = rss_delta(before_rss, read_rss_mb());
    build_result(
        "profile_load",
        iterations,
        &durations,
        "ok",
        vec!["forge-alpha.yaml parse".to_string()],
        memory_delta,
        false,
    )
}

async fn scenario_subprocess_echo_invoke(iterations: u32, warmup: u32) -> ScenarioResult {
    // Subprocess packages require Python which may not be available in CI.
    // Mark as skipped; subprocess echo will be measured in P1/P3 with
    // explicit environment checks.
    let manifest_path = manifest_path(SUBPROCESS_ECHO_MANIFEST);
    let manifest = match read_manifest(manifest_path).await {
        Ok(m) => m,
        Err(e) => {
            return skipped_result(
                "subprocess_echo_invoke",
                iterations,
                &format!("manifest load failed: {e}; subprocess echo deferred to P1/P3"),
            );
        }
    };

    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());

    match runtime.load_package(manifest).await {
        Ok(_) => {}
        Err(e) => {
            return skipped_result(
                "subprocess_echo_invoke",
                iterations,
                &format!("subprocess start failed: {e}; deferred to P1/P3"),
            );
        }
    }

    for _ in 0..warmup {
        if let Err(e) = scenario_subprocess_echo_invoke_sample(&runtime) {
            return error_result("subprocess_echo_invoke", iterations, e);
        }
    }

    let before_rss = read_rss_mb();
    let mut durations = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        match scenario_subprocess_echo_invoke_sample(&runtime) {
            Ok(ms) => durations.push(ms),
            Err(e) => return error_result("subprocess_echo_invoke", iterations, e),
        }
    }
    let memory_delta = rss_delta(before_rss, read_rss_mb());
    build_result(
        "subprocess_echo_invoke",
        iterations,
        &durations,
        "ok",
        vec![],
        memory_delta,
        false,
    )
}

fn scenario_subprocess_echo_invoke_sample<S>(runtime: &Runtime<S>) -> Result<f64>
where
    S: EventStore,
{
    run_async_safely(async {
        let start = Instant::now();
        runtime
            .invoke_capability(CapabilityInvocationRequest {
                handle: None,
                capability_id: Some(SUBPROCESS_ECHO_CAPABILITY_ID.to_string()),
                caller_package_id: None,
                provider_package_id: None,
                version: None,
                session_id: None,
                input: json!({"baseline": true}),
            })
            .await?;
        Ok(start.elapsed().as_secs_f64() * 1000.0)
    })
}

async fn scenario_subprocess_cold_start_ms(iterations: u32, warmup: u32) -> ScenarioResult {
    for _ in 0..warmup {
        if let Err(e) = scenario_subprocess_cold_start_sample().await {
            return skipped_result(
                "subprocess_cold_start_ms",
                iterations,
                &format!("subprocess cold-start unavailable: {e}"),
            );
        }
    }

    let before_rss = read_rss_mb();
    let mut durations = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        match scenario_subprocess_cold_start_sample().await {
            Ok(ms) => durations.push(ms),
            Err(e) => {
                return skipped_result(
                    "subprocess_cold_start_ms",
                    iterations,
                    &format!("subprocess cold-start unavailable: {e}"),
                );
            }
        }
    }
    let memory_delta = rss_delta(before_rss, read_rss_mb());
    build_result(
        "subprocess_cold_start_ms",
        iterations,
        &durations,
        "ok",
        vec!["fresh subprocess per iteration: load_package handshake + first invoke".to_string()],
        memory_delta,
        false,
    )
}

async fn scenario_subprocess_cold_start_sample() -> Result<f64> {
    let manifest = read_manifest(manifest_path(SUBPROCESS_ECHO_MANIFEST)).await?;
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());
    let start = Instant::now();
    runtime.load_package(manifest).await?;
    runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(SUBPROCESS_ECHO_CAPABILITY_ID.to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"baseline": true}),
        })
        .await?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let _ = runtime
        .unload_package(&SUBPROCESS_ECHO_PACKAGE_ID.to_string())
        .await;
    Ok(elapsed_ms)
}

async fn scenario_subprocess_handshake_ms(iterations: u32, warmup: u32) -> ScenarioResult {
    for _ in 0..warmup {
        if let Err(e) = scenario_subprocess_handshake_sample().await {
            return skipped_result(
                "subprocess_handshake_ms",
                iterations,
                &format!("subprocess handshake measurement unavailable: {e}"),
            );
        }
    }

    let before_rss = read_rss_mb();
    let mut durations = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        match scenario_subprocess_handshake_sample().await {
            Ok(ms) => durations.push(ms),
            Err(e) => {
                return skipped_result(
                    "subprocess_handshake_ms",
                    iterations,
                    &format!("subprocess handshake measurement unavailable: {e}"),
                );
            }
        }
    }
    let memory_delta = rss_delta(before_rss, read_rss_mb());
    build_result(
        "subprocess_handshake_ms",
        iterations,
        &durations,
        "ok",
        vec!["Runtime::load_package includes subprocess spawn + handshake; no separate spawn-only API".to_string()],
        memory_delta,
        false,
    )
}

async fn scenario_subprocess_handshake_sample() -> Result<f64> {
    let manifest = read_manifest(manifest_path(SUBPROCESS_ECHO_MANIFEST)).await?;
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());
    let start = Instant::now();
    runtime.load_package(manifest).await?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let _ = runtime
        .unload_package(&SUBPROCESS_ECHO_PACKAGE_ID.to_string())
        .await;
    Ok(elapsed_ms)
}

async fn scenario_subprocess_invoke_steady(
    payload_bytes: usize,
    iterations: u32,
    warmup: u32,
    scenario_id: &'static str,
) -> ScenarioResult {
    let manifest = match read_manifest(manifest_path(SUBPROCESS_ECHO_MANIFEST)).await {
        Ok(m) => m,
        Err(e) => {
            return skipped_result(
                scenario_id,
                iterations,
                &format!("manifest load failed: {e}"),
            );
        }
    };
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());
    if let Err(e) = runtime.load_package(manifest).await {
        return skipped_result(
            scenario_id,
            iterations,
            &format!("subprocess start failed: {e}"),
        );
    }

    for _ in 0..warmup {
        if let Err(e) = scenario_subprocess_invoke_steady_sample(&runtime, payload_bytes).await {
            let _ = runtime
                .unload_package(&SUBPROCESS_ECHO_PACKAGE_ID.to_string())
                .await;
            return error_result(scenario_id, iterations, e);
        }
    }

    let before_rss = read_rss_mb();
    let mut durations = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        match scenario_subprocess_invoke_steady_sample(&runtime, payload_bytes).await {
            Ok(ms) => durations.push(ms),
            Err(e) => {
                let _ = runtime
                    .unload_package(&SUBPROCESS_ECHO_PACKAGE_ID.to_string())
                    .await;
                return error_result(scenario_id, iterations, e);
            }
        }
    }
    let memory_delta = rss_delta(before_rss, read_rss_mb());
    let _ = runtime
        .unload_package(&SUBPROCESS_ECHO_PACKAGE_ID.to_string())
        .await;
    build_result(
        scenario_id,
        iterations,
        &durations,
        "ok",
        vec![format!("echo payload data field is {payload_bytes} bytes")],
        memory_delta,
        false,
    )
}

async fn scenario_subprocess_invoke_steady_sample<S>(
    runtime: &Runtime<S>,
    payload_bytes: usize,
) -> Result<f64>
where
    S: EventStore,
{
    let input = json!({"data": "x".repeat(payload_bytes)});
    let start = Instant::now();
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(SUBPROCESS_ECHO_CAPABILITY_ID.to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input,
        })
        .await?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let echoed_len = result
        .output
        .get("data")
        .and_then(|v| v.as_str())
        .map(str::len)
        .unwrap_or(0);
    if echoed_len != payload_bytes {
        anyhow::bail!("subprocess echo returned {echoed_len} bytes, expected {payload_bytes}");
    }
    Ok(elapsed_ms)
}

fn perf_outbound_manifest() -> plurora_core::PackageManifest {
    plurora_core::PackageManifest {
        schema_version: 1,
        id: PERF_OUTBOUND_PACKAGE_ID.to_string(),
        version: "0.1.0".to_string(),
        display_name: None,
        description: None,
        author: None,
        license: None,
        entry: plurora_core::EntryDescriptor::v1(plurora_core::PackageEntry::RustInproc {
            crate_ref: "example-echo-rust-inproc".to_string(),
            symbol: "register".to_string(),
            abi_version: 1,
        }),
        provides: vec![plurora_core::CapabilityDescriptor {
            id: PERF_OUTBOUND_CAPABILITY_ID.to_string(),
            version: "0.1.0".to_string(),
            input_schema: serde_json::Value::Null,
            output_schema: serde_json::Value::Null,
            streaming: false,
            side_effects: vec!["network".to_string()],
            description: None,
        }],
        consumes: Vec::new(),
        requires: Vec::new(),
        contributes: plurora_core::PackageContributions::default(),
        permissions: plurora_core::PermissionSet {
            network: plurora_core::NetworkPermissions {
                declarations: vec![plurora_core::NetworkDeclaration {
                    host: PERF_OUTBOUND_HOST.to_string(),
                    methods: vec!["POST".to_string()],
                    purpose: Some("perf fake outbound".to_string()),
                }],
                hosts: Vec::new(),
            },
            ..plurora_core::PermissionSet::default()
        },
        sandbox_policy: plurora_core::SandboxPolicy::default(),
    }
}

fn runtime_with_fake_outbound() -> Runtime<InMemoryEventStore> {
    let store = Arc::new(InMemoryEventStore::default());
    let fake = Arc::new(FakeOutboundExecutor::new());
    let config = RuntimeConfig {
        outbound_executor: OutboundExecutorConfig::Custom(fake),
        outbound_execute_policy: OutboundExecutePolicyConfig {
            enabled: true,
            allowed_hosts: vec![PERF_OUTBOUND_HOST.to_string()],
            https_only: true,
            timeout_ms: 30_000,
            allow_redirects: false,
            allow_insecure_loopback_for_tests: false,
        },
        ..RuntimeConfig::default()
    };
    Runtime::new(store, config)
}

fn outbound_policy_request() -> OutboundRequest {
    OutboundRequest {
        principal: ProtocolPrincipal::Package {
            package_id: PERF_OUTBOUND_PACKAGE_ID.to_string(),
        },
        package_id: PERF_OUTBOUND_PACKAGE_ID.to_string(),
        capability_id: PERF_OUTBOUND_CAPABILITY_ID.to_string(),
        destination_host: PERF_OUTBOUND_HOST.to_string(),
        method: "POST".to_string(),
        purpose: Some("perf fake outbound".to_string()),
        secret_refs_used: Vec::new(),
        correlation_id: None,
    }
}

fn outbound_executor_request() -> OutboundExecutorRequest {
    OutboundExecutorRequest {
        package_id: PERF_OUTBOUND_PACKAGE_ID.to_string(),
        capability_id: PERF_OUTBOUND_CAPABILITY_ID.to_string(),
        destination_host: PERF_OUTBOUND_HOST.to_string(),
        method: "POST".to_string(),
        path: Some("/v1/perf".to_string()),
        purpose: Some("perf fake outbound".to_string()),
        secret_refs: Vec::new(),
        redaction_state: None,
        timeout_ms: Some(30_000),
        metadata: json!({"provider": "perf_fake"}),
        body_shape: Some(json!({"model": "perf", "messages": []})),
        secret_headers: Vec::new(),
        resolved_secret_headers: Vec::new(),
        static_headers: Vec::new(),
    }
}

async fn setup_fake_outbound_runtime() -> Result<Runtime<InMemoryEventStore>> {
    let runtime = runtime_with_fake_outbound();
    runtime.load_package(perf_outbound_manifest()).await?;
    Ok(runtime)
}

async fn scenario_outbound_execute_fake_throughput_req_s(
    iterations: u32,
    warmup: u32,
) -> ScenarioResult {
    let runtime = match setup_fake_outbound_runtime().await {
        Ok(r) => r,
        Err(e) => return error_result("outbound_execute_fake_throughput_req_s", iterations, e),
    };
    let batch_size = 1_000u32;

    for _ in 0..warmup {
        if let Err(e) = scenario_outbound_execute_fake_batch(&runtime, batch_size).await {
            return error_result("outbound_execute_fake_throughput_req_s", iterations, e);
        }
    }

    let before_rss = read_rss_mb();
    let mut durations = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        match scenario_outbound_execute_fake_batch(&runtime, batch_size).await {
            Ok(ms) => durations.push(ms),
            Err(e) => return error_result("outbound_execute_fake_throughput_req_s", iterations, e),
        }
    }
    let memory_delta = rss_delta(before_rss, read_rss_mb());
    let total_requests = batch_size as f64 * durations.len() as f64;
    let total_seconds = durations.iter().sum::<f64>() / 1000.0;
    let req_s = if total_seconds > 0.0 {
        total_requests / total_seconds
    } else {
        0.0
    };
    build_result(
        "outbound_execute_fake_throughput_req_s",
        iterations,
        &durations,
        "ok",
        vec![format!(
            "{batch_size} execute_outbound_with_policy calls per iteration; {req_s:.2} req/s"
        )],
        memory_delta,
        false,
    )
}

async fn scenario_outbound_execute_fake_batch<S>(
    runtime: &Runtime<S>,
    batch_size: u32,
) -> Result<f64>
where
    S: EventStore,
{
    let start = Instant::now();
    for _ in 0..batch_size {
        runtime
            .execute_outbound_with_policy(outbound_policy_request(), outbound_executor_request())
            .await?;
    }
    Ok(start.elapsed().as_secs_f64() * 1000.0)
}

async fn scenario_outbound_stream_fake_ttft_ms(iterations: u32, warmup: u32) -> ScenarioResult {
    let runtime = match setup_fake_outbound_runtime().await {
        Ok(r) => r,
        Err(e) => return error_result("outbound_stream_fake_ttft_ms", iterations, e),
    };

    for _ in 0..warmup {
        if let Err(e) = scenario_outbound_stream_ttft_sample(&runtime).await {
            return error_result("outbound_stream_fake_ttft_ms", iterations, e);
        }
    }

    let before_rss = read_rss_mb();
    let mut durations = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        match scenario_outbound_stream_ttft_sample(&runtime).await {
            Ok(ms) => durations.push(ms),
            Err(e) => return error_result("outbound_stream_fake_ttft_ms", iterations, e),
        }
    }
    let memory_delta = rss_delta(before_rss, read_rss_mb());
    build_result(
        "outbound_stream_fake_ttft_ms",
        iterations,
        &durations,
        "ok",
        vec!["FakeOutboundExecutor emits 3 canned SSE events; drained to completion".to_string()],
        memory_delta,
        false,
    )
}

async fn scenario_outbound_stream_ttft_sample<S>(runtime: &Runtime<S>) -> Result<f64>
where
    S: EventStore,
{
    let session_id = format!(
        "kernel_outbound_stream_{}",
        PERF_OUTBOUND_PACKAGE_ID.replace('/', "_")
    );
    let mut rx = runtime.subscribe_events();
    let start = Instant::now();
    let response = start_fake_outbound_stream(runtime).await?;
    let stream_id = response
        .get("stream_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("kernel.v1.outbound.stream returned no stream_id"))?
        .to_string();
    let mut ttft_ms = None;
    let mut chunks = 0usize;
    let deadline = std::time::Duration::from_secs(2);
    loop {
        let event = tokio::time::timeout(deadline, rx.recv()).await??;
        if event.session_id != session_id || event.kind != plurora_core::EVENT_STREAM_CHUNK {
            continue;
        }
        let event_stream_id = event
            .payload
            .get("stream_id")
            .and_then(|v| v.as_str())
            .or_else(|| {
                event
                    .payload
                    .get("data")
                    .and_then(|d| d.get("stream_id"))
                    .and_then(|v| v.as_str())
            });
        if event_stream_id != Some(stream_id.as_str()) {
            continue;
        }
        chunks += 1;
        if ttft_ms.is_none() {
            ttft_ms = Some(start.elapsed().as_secs_f64() * 1000.0);
        }
        if chunks >= 3 {
            break;
        }
    }
    wait_for_outbound_stream_completion(runtime, &session_id, &stream_id).await?;
    Ok(ttft_ms.unwrap_or(0.0))
}

async fn scenario_outbound_stream_fake_steady_events_s(
    iterations: u32,
    warmup: u32,
) -> ScenarioResult {
    let _ = warmup;
    skipped_result(
        "outbound_stream_fake_steady_events_s",
        iterations,
        "FakeOutboundExecutor currently emits a fixed 3-event stream and exposes no public N-frame fixture API; needs runtime support to measure 100 steady events",
    )
}

async fn start_fake_outbound_stream<S>(runtime: &Runtime<S>) -> Result<serde_json::Value>
where
    S: EventStore,
{
    let context = ProtocolContext::package(PERF_OUTBOUND_PACKAGE_ID, "perf_baseline");
    runtime
        .call_protocol(
            &context,
            "kernel.v1.outbound.stream",
            json!({
                "capability_id": PERF_OUTBOUND_CAPABILITY_ID,
                "destination_host": PERF_OUTBOUND_HOST,
                "method": "POST",
                "path": "/v1/perf/stream",
                "stream_format": "sse",
                "body_shape": {"model": "perf", "stream": true},
            }),
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))
}

async fn wait_for_outbound_stream_completion<S>(
    runtime: &Runtime<S>,
    session_id: &str,
    stream_id: &str,
) -> Result<()>
where
    S: EventStore,
{
    let deadline = std::time::Duration::from_secs(2);
    let started = Instant::now();
    loop {
        let events = runtime
            .store()
            .list_session(&session_id.to_string())
            .await?;
        if events.iter().any(|event| {
            event.kind == plurora_core::EVENT_STREAM_ENDED
                && event.payload.get("stream_id").and_then(|v| v.as_str()) == Some(stream_id)
        }) {
            return Ok(());
        }
        if started.elapsed() > deadline {
            anyhow::bail!("timed out waiting for outbound stream completion");
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
}

// ── Entry point ────────────────────────────────────────────────────────

async fn run_scenarios(iterations: u32, warmup: u32) -> Vec<ScenarioResult> {
    let mut results: Vec<ScenarioResult> = Vec::new();

    // 1. Inproc echo invoke
    results.push(scenario_inproc_echo_invoke(iterations, warmup).await);

    // 2. Official capability invoke
    results.push(scenario_official_capability_invoke(iterations, warmup).await);

    // 3. Event store append/list/range (100 events)
    results.push(scenario_event_store_append_list_range(iterations, warmup).await);

    // 4. Event store scale 1k
    results.push(
        scenario_event_store_scale(
            1_000,
            iterations,
            warmup,
            "event_store_append_list_range_1k",
        )
        .await,
    );

    // 5. Event store scale 10k
    results.push(
        scenario_event_store_scale(
            10_000,
            iterations,
            warmup,
            "event_store_append_list_range_10k",
        )
        .await,
    );

    // 6. Event store scale 100k (may be slow; run 1 iteration if >1 requested)
    let scale_100k_iters = if iterations > 1 { 1 } else { iterations };
    let mut scale_100k = scenario_event_store_scale(
        100_000,
        scale_100k_iters,
        warmup.min(1),
        "event_store_append_list_range_100k",
    )
    .await;
    if iterations > 1 && scale_100k.status == "ok" {
        scale_100k.iterations_capped = true;
        scale_100k.notes = vec![format!(
            "100000 events per iteration (capped to 1 iteration from {})",
            iterations
        )];
    }
    results.push(scale_100k);

    // 7. Composition check
    results.push(scenario_composition_check(iterations, warmup).await);

    // 8. Profile load
    results.push(scenario_profile_load(iterations, warmup).await);

    // 9. Subprocess echo (may skip)
    results.push(scenario_subprocess_echo_invoke(iterations, warmup).await);

    // 10. Subprocess cold start (fresh subprocess each iteration)
    results.push(scenario_subprocess_cold_start_ms(iterations, warmup).await);

    // 11. Subprocess handshake / manifest exchange
    results.push(scenario_subprocess_handshake_ms(iterations, warmup).await);

    // 12-14. Subprocess steady-state invoke payload sizes
    results.push(
        scenario_subprocess_invoke_steady(
            1_024,
            iterations,
            warmup,
            "subprocess_invoke_steady_1kb",
        )
        .await,
    );
    results.push(
        scenario_subprocess_invoke_steady(
            10_240,
            iterations,
            warmup,
            "subprocess_invoke_steady_10kb",
        )
        .await,
    );
    results.push(
        scenario_subprocess_invoke_steady(
            102_400,
            iterations,
            warmup,
            "subprocess_invoke_steady_100kb",
        )
        .await,
    );

    // 15. Fake outbound execute throughput
    results.push(scenario_outbound_execute_fake_throughput_req_s(iterations, warmup).await);

    // 16. Fake outbound stream time-to-first-frame
    results.push(scenario_outbound_stream_fake_ttft_ms(iterations, warmup).await);

    // 17. Fake outbound stream steady event rate
    results.push(scenario_outbound_stream_fake_steady_events_s(iterations, warmup).await);

    results
}

fn result_counts(results: &[ScenarioResult]) -> (usize, usize, usize) {
    let ok_count = results.iter().filter(|r| r.status == "ok").count();
    let skip_count = results.iter().filter(|r| r.status == "skipped").count();
    let err_count = results.iter().filter(|r| r.status == "error").count();
    (ok_count, skip_count, err_count)
}

fn load_baseline_averages(path: &Path) -> Result<HashMap<String, f64>> {
    let raw = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&raw)?;
    let mut averages = HashMap::new();

    if let Some(items) = value.get("baseline").and_then(|v| v.as_array()) {
        for item in items {
            if let (Some(id), Some(avg)) = (
                item.get("scenario_id").and_then(|v| v.as_str()),
                item.get("avg_ms").and_then(|v| v.as_f64()),
            ) {
                averages.insert(id.to_string(), avg);
            }
        }
    }

    Ok(averages)
}

fn compare_results(
    results: &[ScenarioResult],
    compare_path: Option<&Path>,
    threshold_pct: f64,
) -> Result<Vec<ComparisonResult>> {
    let Some(path) = compare_path else {
        return Ok(Vec::new());
    };
    let baseline = load_baseline_averages(path)?;
    let mut comparisons = Vec::new();

    for result in results {
        if let Some(&baseline_avg_ms) = baseline.get(&result.scenario_id) {
            if baseline_avg_ms <= 0.0 {
                continue;
            }
            let delta_pct = (result.avg_ms - baseline_avg_ms) / baseline_avg_ms * 100.0;
            comparisons.push(ComparisonResult {
                scenario_id: result.scenario_id.clone(),
                baseline_avg_ms,
                current_avg_ms: result.avg_ms,
                delta_pct,
                regression: delta_pct > threshold_pct,
            });
        }
    }

    Ok(comparisons)
}

fn baseline_envelope(
    results: &[ScenarioResult],
    comparisons: &[ComparisonResult],
    iterations: u32,
    warmup: u32,
) -> BenchEnvelope {
    let (ok_count, skip_count, err_count) = result_counts(results);
    BenchEnvelope {
        schema: "plurora.bench.v1",
        created_at: now_secs_since_epoch(),
        git: collect_git(),
        env: collect_env(),
        baseline: results.to_vec(),
        comparisons: (!comparisons.is_empty()).then_some(comparisons.to_vec()),
        meta: BenchMeta {
            iterations,
            warmup,
            tool: "plurora-cli perf baseline",
            version: "0.1.0",
            note: "developer-machine reference, not CI budget; no-network/deterministic",
            ok_count,
            skipped_count: skip_count,
            error_count: err_count,
        },
    }
}

fn print_text_results(results: &[ScenarioResult], iterations: u32, warmup: u32) {
    println!(
        "plurora perf baseline — {} iterations per scenario, {} warmup",
        iterations, warmup
    );
    println!();
    println!(
        "{:<34} {:>5} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>8} {:>8}  {}",
        "scenario", "iters", "total", "avg", "min", "p50", "p95", "p99", "max", "rssΔ", "status"
    );
    println!("{}", "-".repeat(130));
    for r in results {
        let notes = if r.notes.is_empty() {
            String::new()
        } else {
            format!(" [{}]", r.notes.join("; "))
        };
        let capped = if r.iterations_capped { " capped" } else { "" };
        let rss = r
            .memory_rss_mb_delta
            .map(|v| format!("{v:.2}"))
            .unwrap_or_else(|| "n/a".to_string());
        println!(
            "{:<34} {:>5} {:>9.2} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>8.3} {:>8}  {}{}{}",
            r.scenario_id,
            r.iterations,
            r.total_ms,
            r.avg_ms,
            r.min_ms,
            r.p50_ms.unwrap_or(0.0),
            r.p95_ms.unwrap_or(0.0),
            r.p99_ms.unwrap_or(0.0),
            r.max_ms,
            rss,
            r.status,
            capped,
            notes,
        );
    }
}

fn print_comparisons(comparisons: &[ComparisonResult], threshold_pct: f64) {
    if comparisons.is_empty() {
        return;
    }

    println!();
    println!("comparison (regression threshold > {threshold_pct:.2}%):");
    println!(
        "{:<34} {:>13} {:>13} {:>9}  {}",
        "scenario", "baseline avg", "current avg", "delta%", "status"
    );
    println!("{}", "-".repeat(90));
    for c in comparisons {
        println!(
            "{:<34} {:>13.3} {:>13.3} {:>8.2}%  {}",
            c.scenario_id,
            c.baseline_avg_ms,
            c.current_avg_ms,
            c.delta_pct,
            if c.regression { "regression" } else { "ok" }
        );
    }
    let regressions = comparisons.iter().filter(|c| c.regression).count();
    println!(
        "comparison: {} checked, {} regression",
        comparisons.len(),
        regressions
    );
}

pub(crate) async fn perf_baseline(options: BaselineOptions) -> Result<()> {
    let BaselineOptions {
        iterations,
        warmup,
        format,
        baseline_out,
        compare,
        threshold_pct,
    } = options;

    if iterations == 0 {
        anyhow::bail!("--iterations must be greater than 0");
    }

    let results = run_scenarios(iterations, warmup).await;
    let comparisons = compare_results(&results, compare.as_deref(), threshold_pct)?;
    let envelope = baseline_envelope(&results, &comparisons, iterations, warmup);

    if let Some(path) = baseline_out.as_deref() {
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&envelope)?),
        )?;
    }

    match format {
        BaselineFormat::Text => {
            print_text_results(&results, iterations, warmup);
            print_comparisons(&comparisons, threshold_pct);
        }
        BaselineFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&envelope)?);
        }
    }

    let (ok_count, skip_count, err_count) = result_counts(&results);

    if matches!(format, BaselineFormat::Text) {
        println!();
        println!(
            "baseline: {} ok, {} skipped, {} error ({} scenarios)",
            ok_count,
            skip_count,
            err_count,
            results.len()
        );
    }

    if err_count > 0 {
        anyhow::bail!("baseline had errors");
    }
    if comparisons.iter().any(|c| c.regression) {
        std::process::exit(2);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_basic() {
        let s = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let (min, p50, p95, p99, max) = compute_percentiles(&s);
        assert_eq!(min, 1.0);
        assert_eq!(max, 10.0);
        assert!((p50 - 5.0).abs() <= 1.0);
        assert!(p95 >= 9.0);
        assert!(p99 >= 9.0);
    }

    #[test]
    fn percentiles_empty() {
        let (min, p50, p95, p99, max) = compute_percentiles(&[]);
        assert_eq!(min, 0.0);
        assert_eq!(p50, 0.0);
        assert_eq!(p95, 0.0);
        assert_eq!(p99, 0.0);
        assert_eq!(max, 0.0);
    }

    #[test]
    fn percentiles_single() {
        let s = vec![42.0];
        let (min, p50, p95, p99, max) = compute_percentiles(&s);
        assert_eq!(min, 42.0);
        assert_eq!(p50, 42.0);
        assert_eq!(p95, 42.0);
        assert_eq!(p99, 42.0);
        assert_eq!(max, 42.0);
    }

    #[test]
    fn read_rss_mb_works_or_none() {
        let _ = read_rss_mb();
    }

    #[test]
    fn env_collect_works() {
        let env = collect_env();
        assert!(!env.os.is_empty());
        assert!(env.num_cpus >= 1);
    }
}
