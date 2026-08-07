use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::convert::Infallible;
use std::fmt;
use std::fs;
use std::path::{Component, Path as FsPath, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use anyhow::Context;
use axum::body::to_bytes;
use axum::extract::ws::{Message as AxumWsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{Extension, FromRequestParts, OriginalUri, Path, Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::{any, get, post};
use axum::{Json, Router};
use bollard::models::{ContainerCreateBody, HostConfig, PortBinding, PortMap};
use bollard::models::{Mount, MountType};
use bollard::query_parameters::CreateContainerOptionsBuilder;
use bollard::Docker;
use futures::{SinkExt, Stream, StreamExt};
use plurora_core::{
    ArtifactDescriptor, EventEnvelope, EventSequence, PackageId, ProjectId, SessionId,
};
use plurora_runtime::{
    resolve_contract_method, EventListRequest, ExecutionTargetCapability,
    ExecutionTargetReachability, ExecutionTargetStatusKind, ProtocolContext, ProtocolError,
    ProtocolRequest, ProtocolResourceSelector, ProtocolResponse,
};
use plurora_runtime::{
    AppendEventRequest, EventStore, InMemoryEventStore, OpenSessionRequest, Runtime, RuntimeConfig,
};
use plurora_runtime::{
    PortBindScope, PortLeaseStatusKind, ProxyProtocol, ProxyRouteAccess, ProxyRouteStatusKind,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, OwnedSemaphorePermit, Semaphore};
use tokio::time::{sleep, timeout, Instant};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tower_http::cors::{Any, CorsLayer};

mod development;
mod host_access;
mod target_agent;

pub use development::{
    acquire_development_host_lease, development_registry, hydrate_development_control_plane,
    release_development_host_lease, release_owned_development_host_lease,
    spawn_development_host_lease_heartbeat, DevelopmentChangeRecord, DevelopmentChangeStatus,
    DevelopmentDraftRequest, DevelopmentFileOperationRequest, DevelopmentHostLease,
    DevelopmentManagedPromotion, DevelopmentNetworkMode, DevelopmentRecoveryKind,
    DevelopmentRegistry, DevelopmentVerificationPlan, DevelopmentVerificationResult,
    DevelopmentWorkspaceOwnership,
};
pub use host_access::{
    host_access_registry, hydrate_host_access_control_plane, HostAccessGrantView,
    HostAccessIdentity, HostAccessIdentityKind, HostAccessRegistry, HostAccessResourceKind,
    HostAccessResourceSelector, HostAccessScope,
};
pub use target_agent::{
    decode_target_tunnel_data, encode_target_tunnel_data, hydrate_target_agent_control_plane,
    reconcile_target_deployment_control_plane, target_agent_registry,
    verify_target_operation_authority, ClaimTargetEnrollmentRequest, ClaimTargetEnrollmentResponse,
    CreateTargetOperationRequest, CreateTargetOperationResponse, DeclarativeVerifierDescriptor,
    NextTargetOperationResponse, TargetAgentHeartbeatRequest, TargetAgentHeartbeatResponse,
    TargetAgentRegistry, TargetDeploymentDescriptor, TargetDeploymentRef, TargetOperationAuthority,
    TargetOperationEffect, TargetOperationProgressRequest, TargetOperationReceipt,
    TargetOperationReceiptStatus, TargetOperationRecord, TargetOperationSpec,
    TargetOperationStatusKind, TargetTunnelAgentMessage, TargetTunnelHostMessage, TargetTunnelOpen,
    TARGET_TUNNEL_DATA_CHUNK_BYTES, TARGET_TUNNEL_MAX_STREAMS,
};

const PROXY_REQUEST_BODY_LIMIT_BYTES: usize = 64 * 1024 * 1024;
const PROXY_RESPONSE_BODY_LIMIT_BYTES: usize = 64 * 1024 * 1024;
const PROXY_WEBSOCKET_FRAME_LIMIT_BYTES: usize = 16 * 1024 * 1024;
const PROXY_WEBSOCKET_SUBPROTOCOL_LIMIT: usize = 32;
const PROXY_WEBSOCKET_SUBPROTOCOL_BYTES: usize = 128;
const TARGET_TUNNEL_BRIDGE_HEADER: &str = "x-plurora-tunnel-bridge";
const TARGET_TUNNEL_BRIDGE_HEADER_LIMIT_BYTES: usize = 64 * 1024;
const TARGET_TUNNEL_BRIDGE_LIMIT: usize = 256;
const DEPLOY_READINESS_TIMEOUT: Duration = Duration::from_secs(15);
const DEPLOY_READINESS_INTERVAL: Duration = Duration::from_millis(500);
const DEPLOY_READINESS_CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
const HEALTH_POLL_INTERVAL: Duration = Duration::from_secs(5);
const HEALTH_PROBE_TIMEOUT: Duration = Duration::from_millis(1000);
const HEALTH_FAILURE_THRESHOLD: u32 = 3;
const HEALTH_RECOVERY_THRESHOLD: u32 = 2;
const MAX_RUNTIME_ENV_ENTRIES: usize = 128;
const MAX_RUNTIME_ENV_VALUE_LEN: usize = 8192;
const MAX_RUNTIME_ENV_TOTAL_BYTES: usize = 64 * 1024;
const MAX_RUNTIME_MOUNTS: usize = 32;
const DEPLOYMENT_WORKSPACE_MAX_FILES: u64 = 100_000;
const DEPLOYMENT_WORKSPACE_MAX_DIRECTORIES: u64 = 100_000;
const DEPLOYMENT_WORKSPACE_MAX_BYTES: u64 = 1024 * 1024 * 1024;
const DEPLOYMENT_GIT_DOWNLOAD_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const DOCKER_RUNTIME_PACKAGE_ID: &str = "plurora/docker-runtime-lab";
const BUILD_DEPLOY_MAX_GLOBAL_ACTIVE: usize = 2;
const BUILD_DEPLOY_MAX_PER_PROJECT_ACTIVE: usize = 1;
const BUILD_DEPLOY_MAX_RETAINED_JOBS: usize = 128;
const BUILD_DEPLOY_MAX_REVISIONS_PER_PROJECT: usize = 64;
const BUILD_DEPLOY_LOG_RING: usize = 256;
const BUILD_DEPLOY_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
const SURFACE_ASSET_LEASE_TTL_MS: i64 = 5 * 60 * 1_000;
const SURFACE_ASSET_LEASE_LIMIT: usize = 1_024;
const HOST_SESSION_COOKIE: &str = "plurora_host_session";
const DESKTOP_BOOTSTRAP_PATH: &str = "/host/bootstrap";
#[cfg(test)]
const DEPLOYMENT_JOURNAL_PREFIX: &str = "host/control/v1/deployment.";
const DEPLOYMENT_JOB_SNAPSHOT_EVENT: &str = "host/control/v1/deployment.job.snapshot";
const DEPLOYMENT_REVISION_ACTIVATED_EVENT: &str = "host/control/v1/deployment.revision.activated";
const DEPLOYMENT_DIRECT_ROUTE_OWNED_EVENT: &str = "host/control/v1/deployment.direct_route.owned";
const DEPLOYMENT_DIRECT_ROUTE_RELEASED_EVENT: &str =
    "host/control/v1/deployment.direct_route.released";

#[derive(Debug, Clone)]
struct SurfaceAssetLease {
    root: String,
    grant_id: Option<String>,
    host_access_instance_id: uuid::Uuid,
    expires_at_ms: i64,
}

static SURFACE_ASSET_LEASES: OnceLock<Mutex<HashMap<String, SurfaceAssetLease>>> = OnceLock::new();
const DEPLOYMENT_REVISION_DEACTIVATED_EVENT: &str =
    "host/control/v1/deployment.revision.deactivated";
const DEPLOYMENT_JOURNAL_SESSION: &str = "host_control_deployments";
const DEPLOYMENT_JOURNAL_WRITER: &str = "host/control-plane";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProxyAccessMode {
    PathPrefix,
    Vhost,
}

pub type AppRuntime = Runtime<InMemoryEventStore>;

struct AccessControl<S>
where
    S: EventStore,
{
    access_token: Option<String>,
    bootstrap_token: Arc<Mutex<Option<String>>>,
    store: Arc<S>,
    host_access: Arc<HostAccessRegistry>,
}

impl<S> Clone for AccessControl<S>
where
    S: EventStore,
{
    fn clone(&self) -> Self {
        Self {
            access_token: self.access_token.clone(),
            bootstrap_token: self.bootstrap_token.clone(),
            store: self.store.clone(),
            host_access: self.host_access.clone(),
        }
    }
}

impl<S> AccessControl<S>
where
    S: EventStore,
{
    fn new(
        access_token: Option<String>,
        bootstrap_token: Option<String>,
        store: Arc<S>,
        host_access: Arc<HostAccessRegistry>,
    ) -> Self {
        Self {
            access_token,
            bootstrap_token: Arc::new(Mutex::new(bootstrap_token)),
            store,
            host_access,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostCredentialSource {
    OptionalLoopback,
    Authorization,
    RootCookie,
    DeviceCookie,
    EventQuery,
}

#[derive(Debug, Default)]
struct PresentedHostCredentials {
    authorization: Option<String>,
    root_cookie: Option<String>,
    device_cookie: Option<String>,
    event_query: Option<String>,
}

pub struct AppState<S = InMemoryEventStore>
where
    S: EventStore,
{
    pub runtime: Arc<Runtime<S>>,
    pub static_dir: Option<PathBuf>,
    pub access_token: Option<String>,
    pub app_base_domain: Option<String>,
    pub build_jobs: Arc<BuildDeployJobRegistry>,
    pub development: Arc<DevelopmentRegistry>,
    pub host_access: Arc<HostAccessRegistry>,
    pub target_agents: Arc<TargetAgentRegistry>,
}

impl<S> Clone for AppState<S>
where
    S: EventStore,
{
    fn clone(&self) -> Self {
        Self {
            runtime: self.runtime.clone(),
            static_dir: self.static_dir.clone(),
            access_token: self.access_token.clone(),
            app_base_domain: self.app_base_domain.clone(),
            build_jobs: self.build_jobs.clone(),
            development: self.development.clone(),
            host_access: self.host_access.clone(),
            target_agents: self.target_agents.clone(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct EventListQuery {
    #[serde(default)]
    pub after_sequence: Option<u64>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub kind_prefix: Option<String>,
    #[serde(default)]
    pub writer_package_id: Option<PackageId>,
}

pub fn app() -> Router {
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
    app_with_state(AppState {
        runtime,
        static_dir: None,
        access_token: None,
        app_base_domain: None,
        build_jobs: Arc::new(BuildDeployJobRegistry::default()),
        development: development_registry(),
        host_access: host_access_registry(),
        target_agents: target_agent_registry(),
    })
}

pub fn app_with_state<S>(state: AppState<S>) -> Router
where
    S: EventStore,
{
    app_with_state_and_bootstrap_token(state, None)
}

pub fn app_with_state_and_bootstrap_token<S>(
    state: AppState<S>,
    bootstrap_token: Option<String>,
) -> Router
where
    S: EventStore,
{
    let access_control = AccessControl::new(
        state.access_token.clone(),
        bootstrap_token,
        state.runtime.store(),
        state.host_access.clone(),
    );
    let protected_control = Router::new()
        .route("/journal/subscribe/:session_id", get(subscribe_events::<S>))
        .route("/host/v1/deploy", post(deploy_project::<S>))
        .route("/host/v1/deploy/stop", post(stop_project_deployment::<S>))
        .route("/host/v1/build-deploy", post(build_deploy_project::<S>))
        .route(
            "/host/v1/build-deploy/:job_id",
            get(build_deploy_job_status::<S>),
        )
        .route(
            "/host/v1/build-deploy/:job_id/events",
            get(build_deploy_job_events::<S>),
        )
        .route(
            "/host/v1/build-deploy/:job_id/cancel",
            post(cancel_build_deploy_job::<S>),
        )
        .route(
            "/host/v1/projects/:project_id/deployments",
            get(project_deployments::<S>),
        )
        .route(
            "/host/v1/projects/:project_id/deployments/recover",
            post(recover_project_deployment::<S>),
        )
        .route(
            "/host/v1/projects/:project_id/deployments/rollback",
            post(rollback_project_deployment::<S>),
        )
        .merge(development::routes::<S>())
        .merge(host_access::protected_routes::<S>())
        .merge(target_agent::protected_routes::<S>())
        .route("/rpc", post(rpc::<S>))
        .route_layer(middleware::from_fn_with_state(
            access_control.clone(),
            require_access_token::<S>,
        ))
        .layer(browser_client_cors());
    let protected_passthrough = Router::new()
        .route("/p/:route_id", any(proxy_root::<S>))
        .route("/p/:route_id/*path", any(proxy_path::<S>))
        .route(
            "/surface-bundles/:prefix/*file",
            get(surface_bundle_file::<S>),
        )
        .route_layer(middleware::from_fn_with_state(
            access_control.clone(),
            require_access_token::<S>,
        ));

    Router::new()
        .route("/livez", get(health))
        .route("/health", get(health))
        .route("/healthz", get(health))
        .route("/readyz", get(readiness::<S>))
        .route(
            "/surface-assets/:lease_id/:prefix/*file",
            get(surface_asset_file::<S>),
        )
        .merge(host_access::public_routes::<S>())
        .merge(target_agent::public_routes::<S>())
        .merge(protected_control)
        .merge(protected_passthrough)
        .fallback(static_fallback::<S>)
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(
            state,
            vhost_proxy_middleware::<S>,
        ))
        .layer(middleware::from_fn_with_state(
            access_control,
            desktop_bootstrap_middleware::<S>,
        ))
}

fn browser_client_cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .max_age(Duration::from_secs(60 * 60))
}

pub fn spawn_health_supervisor<S>(state: AppState<S>) -> tokio::task::JoinHandle<()>
where
    S: EventStore,
{
    tokio::spawn(run_health_supervisor(state))
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Debug, Serialize)]
struct HostReadinessResponse {
    status: &'static str,
    ready: bool,
    components: HostReadinessComponents,
}

#[derive(Debug, Serialize)]
struct HostReadinessComponents {
    event_store: HostReadinessComponent,
    control_plane_lease: HostReadinessComponent,
    deployments: HostDeploymentReadiness,
}

#[derive(Debug, Serialize)]
struct HostReadinessComponent {
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct HostDeploymentReadiness {
    status: &'static str,
    durable: usize,
    ready: usize,
    degraded: usize,
}

async fn readiness<S>(State(state): State<AppState<S>>) -> Response
where
    S: EventStore,
{
    let store_ready = state
        .runtime
        .store()
        .list_session_range(&DEPLOYMENT_JOURNAL_SESSION.to_string(), None, Some(1))
        .await
        .is_ok();
    let lease_ready = store_ready
        && development::verify_host_control_plane_lease_if_installed(
            state.runtime.store().as_ref(),
            state.development.as_ref(),
        )
        .await
        .is_ok();

    let durable_routes = state.build_jobs.durable_routes();
    let mut ready_routes = 0usize;
    for durable in &durable_routes {
        let route_ready = state
            .runtime
            .config()
            .proxy_route_registry
            .status(&durable.route_id)
            .await
            .is_some_and(|route| {
                route.status == ProxyRouteStatusKind::Active
                    && route.ready
                    && route.upstream.port_lease_id == durable.port_lease_id
            });
        let lease_ready = state
            .runtime
            .config()
            .port_lease_registry
            .status(&durable.port_lease_id)
            .await
            .is_some_and(|lease| lease.status == PortLeaseStatusKind::Active);
        if route_ready && lease_ready {
            ready_routes += 1;
        }
    }
    let degraded_routes = durable_routes.len().saturating_sub(ready_routes);
    let ready = store_ready && lease_ready;
    let status = if !ready {
        "unready"
    } else if degraded_routes > 0 {
        "degraded"
    } else {
        "ready"
    };
    let response = HostReadinessResponse {
        status,
        ready,
        components: HostReadinessComponents {
            event_store: HostReadinessComponent {
                status: if store_ready { "ok" } else { "failed" },
            },
            control_plane_lease: HostReadinessComponent {
                status: if lease_ready { "ok" } else { "failed" },
            },
            deployments: HostDeploymentReadiness {
                status: if degraded_routes == 0 {
                    "ok"
                } else {
                    "degraded"
                },
                durable: durable_routes.len(),
                ready: ready_routes,
                degraded: degraded_routes,
            },
        },
    };
    (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(response),
    )
        .into_response()
}

async fn require_access_token<S>(
    State(access_control): State<AccessControl<S>>,
    mut request: Request,
    next: Next,
) -> Response
where
    S: EventStore,
{
    let credentials = presented_host_credentials(&request);
    let (identity, source) = match authenticate_host_request(&access_control, credentials).await {
        Ok(Some(authenticated)) => authenticated,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                "missing or invalid Host credential",
            )
                .into_response()
        }
        Err(error) => {
            tracing::warn!(error = %error, "Host access journal could not be refreshed");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Host access control plane is unavailable",
            )
                .into_response();
        }
    };
    if unsafe_cookie_origin_mismatch(&request, source) {
        return (
            StatusCode::FORBIDDEN,
            "cookie-authenticated Host mutation requires a same-origin request",
        )
            .into_response();
    }
    if let Some(scope) = required_host_scope_for_http(request.method(), request.uri().path()) {
        if !identity.allows(scope) {
            return (
                StatusCode::FORBIDDEN,
                "the Host access grant does not include the required scope",
            )
                .into_response();
        }
    }
    if let Some(project_id) = project_id_from_host_path(request.uri().path()) {
        if !identity.allows_project(project_id) {
            return (
                StatusCode::FORBIDDEN,
                "the Host access grant does not include this project",
            )
                .into_response();
        }
    }
    if requires_global_project_authority(request.uri().path())
        && identity.kind == HostAccessIdentityKind::Device
        && !identity.allows_all(HostAccessResourceKind::Project)
    {
        return (
            StatusCode::FORBIDDEN,
            "this Host-global catalogue requires all-project authority",
        )
            .into_response();
    }
    request.extensions_mut().insert(identity);
    strip_host_session_cookie(request.headers_mut());
    next.run(request).await
}

async fn desktop_bootstrap_middleware<S>(
    State(access_control): State<AccessControl<S>>,
    request: Request,
    next: Next,
) -> Response
where
    S: EventStore,
{
    if request.uri().path() != DESKTOP_BOOTSTRAP_PATH {
        return next.run(request).await;
    }
    if request.method() != Method::GET {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }

    let Some(access_token) = access_control
        .access_token
        .as_deref()
        .filter(|token| !token.is_empty())
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let candidate = request.uri().query().and_then(|query| {
        url::form_urlencoded::parse(query.as_bytes())
            .find(|(key, _)| key == "nonce")
            .map(|(_, value)| value.into_owned())
    });
    let accepted = {
        let mut pending = access_control
            .bootstrap_token
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if candidate.as_deref().is_some_and(|value| {
            !value.is_empty() && pending.as_deref().is_some_and(|expected| value == expected)
        }) {
            pending.take();
            true
        } else {
            false
        }
    };
    if !accepted {
        return (
            StatusCode::UNAUTHORIZED,
            "invalid or expired bootstrap nonce",
        )
            .into_response();
    }

    let mut response = StatusCode::SEE_OTHER.into_response();
    response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_static("/?plurora_platform=desktop"),
    );
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{HOST_SESSION_COOKIE}={}; Path=/; HttpOnly; SameSite=Strict",
            host_session_cookie_value(access_token)
        ))
        .expect("SHA-256 cookie value is always a valid header"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

async fn vhost_proxy_middleware<S>(
    State(state): State<AppState<S>>,
    request: Request,
    next: Next,
) -> Response
where
    S: EventStore,
{
    let host = request.headers().get(header::HOST).cloned();
    let Some(match_result) = vhost_route_match(&state, host.as_ref()).await else {
        return next.run(request).await;
    };

    match match_result {
        Ok(route_id) => {
            let uri = request.uri().clone();
            let path = uri.path().trim_start_matches('/').to_string();
            proxy_request(state, route_id, path, uri, request, ProxyAccessMode::Vhost).await
        }
        Err(status) => status.into_response(),
    }
}

async fn vhost_route_match<S>(
    state: &AppState<S>,
    host_header: Option<&HeaderValue>,
) -> Option<Result<String, StatusCode>>
where
    S: EventStore,
{
    let base = state
        .app_base_domain
        .as_deref()
        .and_then(normalize_app_base_domain)?;
    let host = match host_header.and_then(|value| value.to_str().ok()) {
        Some(value) => value,
        None => return Some(Err(StatusCode::BAD_REQUEST)),
    };
    let Some(host) = normalize_host_authority(host) else {
        return Some(Err(StatusCode::BAD_REQUEST));
    };
    let suffix = format!(".{base}");
    if host == base {
        return None;
    }
    let Some(slug) = host.strip_suffix(&suffix) else {
        return None;
    };
    if !valid_vhost_slug(slug) {
        return Some(Err(StatusCode::BAD_REQUEST));
    }
    for route in state.runtime.config().proxy_route_registry.list().await {
        if route_slug(&route.id) == slug {
            return Some(if route.access == ProxyRouteAccess::Public {
                Ok(route.id)
            } else {
                Err(StatusCode::NOT_FOUND)
            });
        }
    }
    Some(Err(StatusCode::NOT_FOUND))
}

fn normalize_app_base_domain(input: &str) -> Option<String> {
    let trimmed = input.trim().trim_end_matches('.').to_ascii_lowercase();
    if trimmed.is_empty()
        || trimmed.contains("://")
        || trimmed.contains('/')
        || trimmed.contains('@')
        || trimmed.contains(':')
        || !trimmed.is_ascii()
    {
        return None;
    }
    if !trimmed
        .split('.')
        .all(|label| valid_dns_label(label) && !is_reserved_vhost_slug(label))
    {
        return None;
    }
    Some(trimmed)
}

fn normalize_host_authority(input: &str) -> Option<String> {
    let trimmed = input.trim().trim_end_matches('.').to_ascii_lowercase();
    if trimmed.is_empty()
        || !trimmed.is_ascii()
        || trimmed.contains("\r")
        || trimmed.contains("\n")
        || trimmed.contains("://")
        || trimmed.contains('/')
        || trimmed.contains('@')
    {
        return None;
    }
    let host = if let Some((host, port)) = trimmed.rsplit_once(':') {
        if port.is_empty() || !port.chars().all(|ch| ch.is_ascii_digit()) {
            return None;
        }
        host
    } else {
        trimmed.as_str()
    };
    if host.is_empty() || !host.split('.').all(valid_dns_label) {
        return None;
    }
    Some(host.to_string())
}

fn route_slug(route_id: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in route_id.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-');
    let prefix = if slug.is_empty() || is_reserved_vhost_slug(slug) {
        "app"
    } else {
        slug
    };
    let max_prefix_len = 54usize;
    let mut prefix = prefix.chars().take(max_prefix_len).collect::<String>();
    while prefix.ends_with('-') {
        prefix.pop();
    }
    if prefix.is_empty() {
        prefix.push_str("app");
    }
    let hash = short_route_hash(route_id);
    format!("{prefix}-{hash}")
}

fn service_public_url_for_route<S>(
    state: &AppState<S>,
    route_id: &str,
    fallback_public_url: &str,
    route_access: ProxyRouteAccess,
) -> String
where
    S: EventStore,
{
    if route_access != ProxyRouteAccess::Public {
        return fallback_public_url.to_string();
    }
    let Some(base) = state
        .app_base_domain
        .as_deref()
        .and_then(normalize_app_base_domain)
    else {
        return fallback_public_url.to_string();
    };
    format!("https://{}.{base}/", route_slug(route_id))
}

fn short_route_hash(route_id: &str) -> String {
    let digest = Sha256::digest(route_id.as_bytes());
    digest[..4]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn valid_vhost_slug(slug: &str) -> bool {
    valid_dns_label(slug) && !is_reserved_vhost_slug(slug)
}

fn valid_dns_label(label: &str) -> bool {
    if label.is_empty() || label.len() > 63 || !label.is_ascii() {
        return false;
    }
    let bytes = label.as_bytes();
    if !bytes[0].is_ascii_alphanumeric() || !bytes[label.len() - 1].is_ascii_alphanumeric() {
        return false;
    }
    bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
}

fn is_reserved_vhost_slug(slug: &str) -> bool {
    matches!(
        slug,
        "www"
            | "api"
            | "admin"
            | "host"
            | "rpc"
            | "health"
            | "surface-bundles"
            | "surface-assets"
            | "p"
            | "static"
    )
}

async fn authenticate_host_request<S>(
    access_control: &AccessControl<S>,
    credentials: PresentedHostCredentials,
) -> anyhow::Result<Option<(HostAccessIdentity, HostCredentialSource)>>
where
    S: EventStore,
{
    let Some(root_token) = access_control
        .access_token
        .as_deref()
        .filter(|token| !token.is_empty())
    else {
        return Ok(Some((
            HostAccessIdentity::root(),
            HostCredentialSource::OptionalLoopback,
        )));
    };

    if let Some(token) = credentials.authorization.as_deref() {
        if root_token_matches(token, root_token) {
            return Ok(Some((
                HostAccessIdentity::root(),
                HostCredentialSource::Authorization,
            )));
        }
        return Ok(host_access::authenticate_host_access_token(
            access_control.store.as_ref(),
            access_control.host_access.as_ref(),
            token,
        )
        .await?
        .map(|identity| (identity, HostCredentialSource::Authorization)));
    }

    let expected_root_cookie = host_session_cookie_value(root_token);
    if credentials.root_cookie.as_deref().is_some_and(|candidate| {
        host_access::constant_time_eq(candidate.as_bytes(), expected_root_cookie.as_bytes())
    }) {
        return Ok(Some((
            HostAccessIdentity::root(),
            HostCredentialSource::RootCookie,
        )));
    }

    if let Some(token) = credentials.device_cookie.as_deref() {
        return Ok(host_access::authenticate_host_access_token(
            access_control.store.as_ref(),
            access_control.host_access.as_ref(),
            token,
        )
        .await?
        .map(|identity| (identity, HostCredentialSource::DeviceCookie)));
    }

    if let Some(token) = credentials.event_query {
        if root_token_matches(&token, root_token) {
            return Ok(Some((
                HostAccessIdentity::root(),
                HostCredentialSource::EventQuery,
            )));
        }
        return Ok(host_access::authenticate_host_access_token(
            access_control.store.as_ref(),
            access_control.host_access.as_ref(),
            &token,
        )
        .await?
        .map(|identity| (identity, HostCredentialSource::EventQuery)));
    }

    Ok(None)
}

fn presented_host_credentials(request: &Request) -> PresentedHostCredentials {
    let cookies = request
        .headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok());
    PresentedHostCredentials {
        authorization: bearer_token(request.headers()).map(str::to_owned),
        root_cookie: cookies
            .and_then(|cookies| cookie_value(cookies, HOST_SESSION_COOKIE))
            .map(str::to_owned),
        device_cookie: cookies
            .and_then(|cookies| cookie_value(cookies, host_access::REMOTE_HOST_SESSION_COOKIE))
            .map(str::to_owned),
        event_query: event_stream_query_credentials_allowed(request)
            .then(|| query_access_token(request.uri()))
            .flatten(),
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
}

fn query_access_token(uri: &Uri) -> Option<String> {
    uri.query().and_then(|query| {
        url::form_urlencoded::parse(query.as_bytes())
            .find(|(key, _)| key == "access_token")
            .map(|(_, value)| value.into_owned())
            .filter(|token| !token.is_empty())
    })
}

fn event_stream_query_credentials_allowed(request: &Request) -> bool {
    let path = request.uri().path();
    request.method() == Method::GET
        && (path.starts_with("/journal/subscribe/")
            || (path.starts_with("/host/v1/build-deploy/") && path.ends_with("/events")))
}

fn unsafe_cookie_origin_mismatch(request: &Request, source: HostCredentialSource) -> bool {
    if !matches!(
        source,
        HostCredentialSource::RootCookie | HostCredentialSource::DeviceCookie
    ) || ![Method::POST, Method::PUT, Method::PATCH, Method::DELETE].contains(request.method())
    {
        return false;
    }
    let Some(origin) = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    else {
        // Native/Desktop clients do not necessarily send Origin. Browser requests do,
        // and SameSite=Strict remains an additional boundary for cookie credentials.
        return false;
    };
    let Some(host) = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return true;
    };
    !origin_authority_matches_host(origin, host)
}

fn origin_authority_matches_host(origin: &str, host: &str) -> bool {
    let Ok(origin) = url::Url::parse(origin) else {
        return false;
    };
    if !matches!(origin.scheme(), "http" | "https")
        || !origin.username().is_empty()
        || origin.password().is_some()
        || origin.path() != "/"
        || origin.query().is_some()
        || origin.fragment().is_some()
    {
        return false;
    }
    let Ok(host) = host.parse::<axum::http::uri::Authority>() else {
        return false;
    };
    if !origin
        .host_str()
        .is_some_and(|origin_host| origin_host.eq_ignore_ascii_case(host.host()))
    {
        return false;
    }
    match host.port_u16() {
        Some(port) => origin.port_or_known_default() == Some(port),
        None => origin.port().is_none(),
    }
}

fn required_host_scope_for_http(method: &Method, path: &str) -> Option<HostAccessScope> {
    if path == "/rpc" || path == "/host/v1/access/me" || path == "/host/v1/access/logout" {
        return None;
    }
    if path == "/host/v1/access"
        || path == "/host/v1/access/pairings"
        || path.starts_with("/host/v1/access/pairings/")
        || path.starts_with("/host/v1/access/grants/")
    {
        return Some(HostAccessScope::AccessManage);
    }
    if path.starts_with("/host/v1/projects/") && path.contains("/changes") {
        if path.contains("/deployment/") {
            return Some(HostAccessScope::Deploy);
        }
        if method == Method::GET || path.ends_with("/changes") {
            return Some(HostAccessScope::DevelopPropose);
        }
        if path.ends_with("/approve") {
            return Some(HostAccessScope::DevelopApprove);
        }
        if path.ends_with("/execute") || path.ends_with("/recover") {
            return Some(HostAccessScope::DevelopExecute);
        }
        return Some(HostAccessScope::AccessManage);
    }
    if path.starts_with("/host/v1/") {
        return Some(if method == Method::GET {
            HostAccessScope::Observe
        } else {
            HostAccessScope::Deploy
        });
    }
    if path.starts_with("/journal/subscribe/") {
        return Some(HostAccessScope::Observe);
    }
    if path == "/p" || path.starts_with("/p/") {
        return Some(HostAccessScope::Observe);
    }
    if path.starts_with("/surface-bundles/") {
        return Some(HostAccessScope::Observe);
    }
    Some(HostAccessScope::AccessManage)
}

fn project_id_from_host_path(path: &str) -> Option<&str> {
    let remainder = path
        .strip_prefix("/host/v1/projects/")
        .or_else(|| path.strip_prefix("/surface-bundles/projects/"))?;
    let project_id = remainder.split('/').next()?;
    (!project_id.is_empty()).then_some(project_id)
}

fn requires_global_project_authority(path: &str) -> bool {
    path.starts_with("/surface-bundles/") && !path.starts_with("/surface-bundles/projects/")
}

fn require_identity_project(
    identity: &HostAccessIdentity,
    project_id: &str,
) -> Result<(), ServiceError> {
    if identity.allows_project(project_id) {
        Ok(())
    } else {
        Err(ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "the Host access grant does not include this project",
        ))
    }
}

fn require_identity_target(
    identity: &HostAccessIdentity,
    target_id: &str,
) -> Result<(), ServiceError> {
    if identity.allows_target(target_id) {
        Ok(())
    } else {
        Err(ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "the Host access grant does not include this execution target",
        ))
    }
}

fn host_session_cookie_value(access_token: &str) -> String {
    let digest = Sha256::digest(access_token.as_bytes());
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn root_token_matches(candidate: &str, expected: &str) -> bool {
    let candidate = host_session_cookie_value(candidate);
    let expected = host_session_cookie_value(expected);
    host_access::constant_time_eq(candidate.as_bytes(), expected.as_bytes())
}

fn cookie_value<'a>(cookies: &'a str, name: &str) -> Option<&'a str> {
    cookies.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key == name).then_some(value)
    })
}

fn strip_host_session_cookie(headers: &mut HeaderMap) {
    let Some(cookies) = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
    else {
        headers.remove(header::COOKIE);
        return;
    };
    let retained = cookies
        .split(';')
        .map(str::trim)
        .filter(|part| {
            part.split_once('=').is_none_or(|(key, _)| {
                key != HOST_SESSION_COOKIE && key != host_access::REMOTE_HOST_SESSION_COOKIE
            })
        })
        .collect::<Vec<_>>()
        .join("; ");
    if retained.is_empty() {
        headers.remove(header::COOKIE);
    } else if let Ok(value) = HeaderValue::from_str(&retained) {
        headers.insert(header::COOKIE, value);
    } else {
        headers.remove(header::COOKIE);
    }
}

async fn subscribe_events<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path(session_id): Path<SessionId>,
    Query(query): Query<EventListQuery>,
) -> anyhow::Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, ServiceError>
where
    S: EventStore,
{
    let request = EventListRequest {
        session_id: session_id.clone(),
        after_sequence: query.after_sequence,
        limit: query.limit,
        kind_prefix: query.kind_prefix.clone(),
        writer_package_id: query.writer_package_id.clone(),
    };
    let replay = state
        .runtime
        .list_events_range_with_context(&identity.protocol_context("http_sse"), &request)
        .await?;
    let replay = VecDeque::from(replay);
    let rx = state.runtime.subscribe_events();
    let stream = futures::stream::unfold(
        (replay, rx, session_id, query),
        |(mut replay, mut rx, session_id, query)| async move {
            if let Some(event) = replay.pop_front() {
                let sse = SseEvent::default()
                    .event("journal.event")
                    .json_data(event)
                    .unwrap_or_else(|_| SseEvent::default().event("runtime.error"));
                return Some((Ok(sse), (replay, rx, session_id, query)));
            }
            loop {
                match rx.recv().await {
                    Ok(event) if event_matches_query(&event, &session_id, &query) => {
                        let sse = SseEvent::default()
                            .event("journal.event")
                            .json_data(event)
                            .unwrap_or_else(|_| SseEvent::default().event("runtime.error"));
                        return Some((Ok(sse), (replay, rx, session_id, query)));
                    }
                    Ok(_) => continue,
                    Err(_) => return None,
                }
            }
        },
    );
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

fn event_matches_query(event: &EventEnvelope, session_id: &str, query: &EventListQuery) -> bool {
    if event.session_id != session_id {
        return false;
    }
    if let Some(after_sequence) = query.after_sequence {
        if event.sequence <= after_sequence {
            return false;
        }
    }
    if let Some(kind_prefix) = &query.kind_prefix {
        if !event.kind.starts_with(kind_prefix) {
            return false;
        }
    }
    if let Some(writer_package_id) = &query.writer_package_id {
        if &event.writer_package_id != writer_package_id {
            return false;
        }
    }
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostDeployRequest {
    #[serde(default)]
    pub project_id: Option<ProjectId>,
    pub image: String,
    pub container_port: u16,
    pub port_name: String,
    pub route_id: String,
    #[serde(default)]
    pub route_access: ProxyRouteAccess,
    #[serde(default)]
    pub health_path: Option<String>,
    #[serde(default)]
    pub pull_if_missing: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostDeployStopRequest {
    pub route_id: String,
}

#[derive(Debug, Serialize)]
pub struct HostDeployResponse {
    pub route_id: String,
    pub public_url: String,
    pub route_access: ProxyRouteAccess,
    pub port_lease_id: String,
    pub container_id: String,
    pub container_name: Option<String>,
}

#[derive(Debug, Clone)]
struct DeployBuiltImageResponse {
    route_id: String,
    public_url: String,
    route_access: ProxyRouteAccess,
    port_lease_id: String,
    container_id: String,
    container_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HostDeployStopResponse {
    pub route_id: String,
    pub stopped: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostBuildDeployRequest {
    pub project_id: ProjectId,
    pub source_url: String,
    pub ref_name: String,
    #[serde(default)]
    pub strategy: Option<String>,
    #[serde(default)]
    pub dockerfile: Option<String>,
    pub container_port: u16,
    pub port_name: String,
    pub route_id: String,
    #[serde(default)]
    pub route_access: ProxyRouteAccess,
    #[serde(default)]
    pub health_path: Option<String>,
    pub approved: bool,
    #[serde(default)]
    pub source_commit: Option<String>,
    #[serde(default)]
    pub build_id: Option<String>,
    #[serde(default)]
    pub runtime_env: Vec<RuntimeEnvSpec>,
    #[serde(default)]
    pub runtime_mounts: Vec<RuntimeMountSpec>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEnvSpec {
    pub name: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub secret_ref: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeMountSpec {
    pub source_host_path: String,
    pub container_path: String,
    #[serde(default = "default_runtime_mount_mode")]
    pub mode: RuntimeMountMode,
    pub approved: bool,
    #[serde(default)]
    pub high_risk_approved: bool,
    pub reason: String,
}

impl fmt::Debug for RuntimeMountSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeMountSpec")
            .field("source_host_path", &"<redacted>")
            .field("container_path", &self.container_path)
            .field("mode", &self.mode)
            .field("approved", &self.approved)
            .field("high_risk_approved", &self.high_risk_approved)
            .field("reason", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeMountMode {
    Ro,
    Rw,
}

fn default_runtime_mount_mode() -> RuntimeMountMode {
    RuntimeMountMode::Ro
}

impl fmt::Debug for RuntimeEnvSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeEnvSpec")
            .field("name", &self.name)
            .field(
                "source",
                &if self.secret_ref.is_some() {
                    "secret_ref"
                } else {
                    "plain"
                },
            )
            .field("value", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeEnvSummary {
    pub name: String,
    pub source: RuntimeEnvSourceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeMountSummary {
    pub container_path: String,
    pub mode: RuntimeMountMode,
    pub source_basename: Option<String>,
    pub source_kind: String,
    pub source_hash: String,
    pub approved: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BuildDeployJobState {
    Queued,
    Cloning,
    Building,
    Starting,
    RegisteringProxy,
    Probing,
    Ready,
    Failed,
    Cancelled,
}

impl BuildDeployJobState {
    fn terminal(self) -> bool {
        matches!(self, Self::Ready | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildDeployJobEvent {
    pub job_id: String,
    pub sequence: u64,
    pub state: BuildDeployJobState,
    pub message: String,
    pub timestamp_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildDeployJobStatusResponse {
    pub job_id: String,
    pub project_id: ProjectId,
    pub route_id: String,
    pub build_id: Option<String>,
    pub state: BuildDeployJobState,
    pub created_at_ms: u128,
    pub updated_at_ms: u128,
    pub result: Option<HostBuildDeployResponse>,
    pub error: Option<String>,
    pub events_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    pub operation: DeploymentOperation,
}

#[derive(Debug, Serialize)]
pub struct BuildDeployJobSubmitResponse {
    pub job_id: String,
    pub status_url: String,
    pub events_url: String,
    pub state: BuildDeployJobState,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum BuildDeploySubmitOrStatusResponse {
    Submitted(BuildDeployJobSubmitResponse),
    Status(BuildDeployJobStatusResponse),
}

#[derive(Debug, Deserialize)]
pub struct BuildDeploySubmitQuery {
    #[serde(default)]
    pub wait: bool,
}

#[derive(Debug, Serialize)]
pub struct BuildDeployCancelResponse {
    pub job_id: String,
    pub state: BuildDeployJobState,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEnvSourceKind {
    Plain,
    SecretRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostBuildDeployResponse {
    pub route_id: String,
    pub public_url: String,
    #[serde(default)]
    pub route_access: ProxyRouteAccess,
    pub port_lease_id: String,
    pub container_id: String,
    pub container_name: Option<String>,
    pub image: String,
    pub build_id: String,
    pub source_commit: String,
    pub build_descriptor_hash: String,
    pub strategy: String,
    pub runtime_env: Vec<RuntimeEnvSummary>,
    pub runtime_mounts: Vec<RuntimeMountSummary>,
    pub warnings: Vec<String>,
}

struct BuildDeployOutcome {
    response: HostBuildDeployResponse,
    previous_revision: Option<DeploymentRevision>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentOperation {
    BuildDeploy,
    VerifiedActivate,
    Recover,
    Rollback,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentSourceKind {
    #[default]
    GitClone,
    VerifiedArtifact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeploymentAuthorityLease {
    operation_id: String,
    #[serde(default = "default_local_target_id")]
    target_id: String,
    identity_kind: HostAccessIdentityKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grant_id: Option<String>,
    scopes: BTreeSet<HostAccessScope>,
    resources: BTreeSet<HostAccessResourceSelector>,
    #[serde(default)]
    delegation_chain: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at_ms: Option<i64>,
}

impl DeploymentAuthorityLease {
    fn from_identity(
        operation_id: String,
        target_id: impl Into<String>,
        identity: &HostAccessIdentity,
    ) -> Self {
        Self {
            operation_id,
            target_id: target_id.into(),
            identity_kind: identity.kind,
            grant_id: identity.grant_id.clone(),
            scopes: identity.scopes.clone(),
            resources: identity.resources.clone(),
            delegation_chain: identity.delegation_chain.clone(),
            expires_at_ms: identity.expires_at_ms,
        }
    }

    fn unavailable(operation_id: String) -> Self {
        Self {
            operation_id,
            target_id: default_local_target_id(),
            identity_kind: HostAccessIdentityKind::Device,
            grant_id: None,
            scopes: BTreeSet::new(),
            resources: BTreeSet::new(),
            delegation_chain: Vec::new(),
            expires_at_ms: Some(0),
        }
    }

    fn allows_resource(&self, kind: HostAccessResourceKind, id: &str) -> bool {
        self.identity_kind == HostAccessIdentityKind::Root
            || self.resources.iter().any(|selector| {
                selector.kind == kind
                    && selector.id.as_deref().is_none_or(|selected| selected == id)
            })
    }

    fn validate(
        &self,
        project_id: &ProjectId,
        registry: &HostAccessRegistry,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.identity_kind == HostAccessIdentityKind::Root
                || self.scopes.contains(&HostAccessScope::Deploy),
            "deployment authority lease does not include deploy"
        );
        anyhow::ensure!(
            self.allows_resource(HostAccessResourceKind::Project, project_id.as_str())
                && self.allows_resource(HostAccessResourceKind::Target, &self.target_id),
            "deployment authority lease does not include the project and target"
        );
        if let Some(expires_at_ms) = self.expires_at_ms {
            anyhow::ensure!(
                expires_at_ms > chrono::Utc::now().timestamp_millis(),
                "deployment authority lease expired"
            );
        }
        if self.identity_kind == HostAccessIdentityKind::Device {
            let grant_id = self
                .grant_id
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("device deployment authority has no grant id"))?;
            anyhow::ensure!(
                registry.grant_is_currently_active(grant_id),
                "deployment authority grant is revoked or expired"
            );
        }
        Ok(())
    }

    fn protocol_context(&self, project_id: &ProjectId, transport: &str) -> ProtocolContext {
        let context = if self.identity_kind == HostAccessIdentityKind::Root {
            ProtocolContext::host_admin(transport)
        } else {
            ProtocolContext::host_device(
                self.grant_id
                    .clone()
                    .expect("device deployment authority always carries a grant id"),
                self.scopes
                    .iter()
                    .map(|scope| scope.as_str().to_string())
                    .collect(),
                self.resources
                    .iter()
                    .map(|selector| ProtocolResourceSelector {
                        owner: "host".to_string(),
                        kind: selector.kind.as_str().to_string(),
                        id: selector.id.clone(),
                    })
                    .collect(),
                self.delegation_chain.clone(),
                transport,
            )
        };
        context.with_host_operation(
            "deploy",
            vec![
                ProtocolResourceSelector {
                    owner: "host".to_string(),
                    kind: "project".to_string(),
                    id: Some(project_id.to_string()),
                },
                ProtocolResourceSelector {
                    owner: "host".to_string(),
                    kind: "target".to_string(),
                    id: Some(self.target_id.clone()),
                },
            ],
        )
    }
}

fn default_local_target_id() -> String {
    "local".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedRuntimeEnvSpec {
    pub name: String,
    pub secret_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentRevision {
    pub revision_id: String,
    pub project_id: ProjectId,
    pub job_id: Option<String>,
    pub operation: DeploymentOperation,
    pub parent_revision_id: Option<String>,
    pub created_at_ms: u128,
    #[serde(default = "default_local_target_id")]
    pub target_id: String,
    #[serde(default)]
    pub source_kind: DeploymentSourceKind,
    pub source_url: String,
    pub ref_name: String,
    pub dockerfile: Option<String>,
    pub container_port: u16,
    pub port_name: String,
    pub route_id: String,
    #[serde(default)]
    pub route_access: ProxyRouteAccess,
    pub health_path: Option<String>,
    pub image: String,
    pub build_id: String,
    pub source_commit: String,
    pub build_descriptor_hash: String,
    pub strategy: String,
    pub runtime_env: Vec<PersistedRuntimeEnvSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_change_set_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_ref: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_context_ref: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_ref: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_ref: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_build_network_mode: Option<plurora_runtime::ManagedTargetBuildNetworkMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_deployment: Option<TargetDeploymentRef>,
    pub recoverable: bool,
    pub recovery_blockers: Vec<String>,
    pub receipt: HostBuildDeployResponse,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDeploymentsResponse {
    pub project_id: ProjectId,
    pub active_revision_id: Option<String>,
    pub active_revision: Option<DeploymentRevision>,
    pub recovery_required: bool,
    pub runtime_ready: bool,
    pub jobs: Vec<BuildDeployJobStatusResponse>,
    pub revisions: Vec<DeploymentRevision>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentRollbackRequest {
    pub revision_id: String,
}

#[derive(Debug, Serialize)]
pub struct DeploymentActionResponse {
    pub operation: DeploymentOperation,
    pub previous_revision_id: Option<String>,
    pub revision: DeploymentRevision,
    pub warnings: Vec<String>,
}

struct ResolvedRuntimeEnv {
    name: String,
    value: String,
    source: RuntimeEnvSourceKind,
}

struct ResolvedRuntimeMount {
    source: PathBuf,
    container_path: String,
    mode: RuntimeMountMode,
    source_kind: String,
    source_basename: Option<String>,
    source_hash: String,
}

impl fmt::Debug for ResolvedRuntimeMount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResolvedRuntimeMount")
            .field("source", &"<redacted>")
            .field("container_path", &self.container_path)
            .field("mode", &self.mode)
            .field("source_kind", &self.source_kind)
            .field("source_hash", &self.source_hash)
            .finish()
    }
}

struct HostDockerStartRequest<'a> {
    image: &'a str,
    container_port: u16,
    host_port: u16,
    route_id: &'a str,
    port_lease_id: &'a str,
    project_id: &'a str,
    build_id: &'a str,
    source_commit: &'a str,
    operation_id: &'a str,
    env: &'a [ResolvedRuntimeEnv],
    mounts: &'a [ResolvedRuntimeMount],
}

#[derive(Debug, Clone)]
struct HostDockerStartedContainer {
    container_id: String,
    container_name: Option<String>,
}

#[derive(Debug)]
struct BuildDeployJobRecord {
    job_id: String,
    project_id: ProjectId,
    route_id: String,
    build_id: Option<String>,
    state: BuildDeployJobState,
    created_at_ms: u128,
    updated_at_ms: u128,
    result: Option<HostBuildDeployResponse>,
    error: Option<String>,
    events: VecDeque<BuildDeployJobEvent>,
    next_sequence: u64,
    cancel: Arc<AtomicBool>,
    idempotency_key: Option<String>,
    request_fingerprint: String,
    operation: DeploymentOperation,
    authority: DeploymentAuthorityLease,
}

#[derive(Debug, Default)]
struct DeploymentProjection {
    revisions: HashMap<ProjectId, Vec<DeploymentRevision>>,
    active_revisions: HashMap<ProjectId, String>,
    direct_route_owners: HashMap<String, DeploymentDirectRouteOwned>,
}

#[derive(Debug)]
struct CreateBuildDeployJobResult {
    job_id: String,
    created: bool,
    state: BuildDeployJobState,
    permit: Option<OwnedSemaphorePermit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeploymentJobSnapshot {
    status: BuildDeployJobStatusResponse,
    event: Option<BuildDeployJobEvent>,
    #[serde(default)]
    request_fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    authority: Option<DeploymentAuthorityLease>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeploymentRevisionActivated {
    revision: DeploymentRevision,
    #[serde(default)]
    enforce_parent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    job: Option<DeploymentJobSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    authority: Option<DeploymentAuthorityLease>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeploymentRevisionDeactivated {
    project_id: ProjectId,
    revision_id: String,
    route_id: String,
    reason: String,
    timestamp_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeploymentDirectRouteOwned {
    route_id: String,
    project_id: ProjectId,
    #[serde(default)]
    port_name: String,
    #[serde(default)]
    route_access: ProxyRouteAccess,
    port_lease_id: String,
    container_id: String,
    timestamp_ms: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    authority: Option<DeploymentAuthorityLease>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeploymentDirectRouteReleased {
    route_id: String,
    project_id: ProjectId,
    timestamp_ms: u128,
}

#[derive(Debug, Clone)]
struct DurableDeploymentRoute {
    route_id: String,
    port_name: String,
    route_access: ProxyRouteAccess,
    port_lease_id: String,
}

#[derive(Debug)]
pub struct BuildDeployJobRegistry {
    jobs: Mutex<HashMap<String, BuildDeployJobRecord>>,
    notifier: broadcast::Sender<BuildDeployJobEvent>,
    global_sem: Arc<Semaphore>,
    project_active: Mutex<HashSet<ProjectId>>,
    deployment: Mutex<DeploymentProjection>,
    journal_apply: Mutex<()>,
    journal_next_sequence: Mutex<EventSequence>,
}

struct BuildDeployProjectGuard {
    registry: Arc<BuildDeployJobRegistry>,
    project_id: ProjectId,
}

impl Drop for BuildDeployProjectGuard {
    fn drop(&mut self) {
        self.registry.release_project(&self.project_id);
    }
}

impl Default for BuildDeployJobRegistry {
    fn default() -> Self {
        let (notifier, _) = broadcast::channel(512);
        Self {
            jobs: Mutex::new(HashMap::new()),
            notifier,
            global_sem: Arc::new(Semaphore::new(BUILD_DEPLOY_MAX_GLOBAL_ACTIVE)),
            project_active: Mutex::new(HashSet::new()),
            deployment: Mutex::new(DeploymentProjection::default()),
            journal_apply: Mutex::new(()),
            journal_next_sequence: Mutex::new(0),
        }
    }
}

pub fn build_deploy_job_registry() -> Arc<BuildDeployJobRegistry> {
    Arc::new(BuildDeployJobRegistry::default())
}

impl BuildDeployJobRegistry {
    fn journal_next_sequence(&self) -> EventSequence {
        *self
            .journal_next_sequence
            .lock()
            .expect("deployment journal tail lock poisoned")
    }

    fn apply_journal_event(&self, event: &EventEnvelope) -> anyhow::Result<()> {
        let _apply_guard = self
            .journal_apply
            .lock()
            .expect("deployment journal apply lock poisoned");
        anyhow::ensure!(
            event.session_id == DEPLOYMENT_JOURNAL_SESSION,
            "deployment event was written to the wrong journal"
        );
        let expected = self.journal_next_sequence();
        if event.sequence < expected {
            return Ok(());
        }
        anyhow::ensure!(
            event.sequence == expected,
            "deployment journal sequence is not contiguous"
        );
        match event.kind.as_str() {
            DEPLOYMENT_JOB_SNAPSHOT_EVENT => {
                self.restore_job_snapshot(
                    serde_json::from_value(event.payload.clone()).with_context(|| {
                        format!("invalid durable deployment job snapshot {}", event.id)
                    })?,
                );
            }
            DEPLOYMENT_REVISION_ACTIVATED_EVENT => {
                let activation: DeploymentRevisionActivated =
                    serde_json::from_value(event.payload.clone()).with_context(|| {
                        format!(
                            "invalid durable deployment revision activation {}",
                            event.id
                        )
                    })?;
                if activation.enforce_parent {
                    self.ensure_revision_parent(&activation.revision)?;
                }
                self.register_revision(activation.revision);
                if let Some(job) = activation.job {
                    self.restore_job_snapshot(job);
                }
            }
            DEPLOYMENT_REVISION_DEACTIVATED_EVENT => {
                let deactivation: DeploymentRevisionDeactivated =
                    serde_json::from_value(event.payload.clone()).with_context(|| {
                        format!(
                            "invalid durable deployment revision deactivation {}",
                            event.id
                        )
                    })?;
                self.deactivate_revision(&deactivation);
            }
            DEPLOYMENT_DIRECT_ROUTE_OWNED_EVENT => {
                let ownership: DeploymentDirectRouteOwned =
                    serde_json::from_value(event.payload.clone()).with_context(|| {
                        format!("invalid durable direct route ownership {}", event.id)
                    })?;
                self.register_direct_route_owner(ownership);
            }
            DEPLOYMENT_DIRECT_ROUTE_RELEASED_EVENT => {
                let release: DeploymentDirectRouteReleased =
                    serde_json::from_value(event.payload.clone()).with_context(|| {
                        format!("invalid durable direct route release {}", event.id)
                    })?;
                let mut deployment = self.deployment.lock().expect("deployment lock poisoned");
                if deployment
                    .direct_route_owners
                    .get(&release.route_id)
                    .is_some_and(|ownership| ownership.project_id == release.project_id)
                {
                    deployment.direct_route_owners.remove(&release.route_id);
                }
            }
            _ => anyhow::bail!("unexpected deployment journal event kind"),
        }
        *self
            .journal_next_sequence
            .lock()
            .expect("deployment journal tail lock poisoned") = event.sequence.saturating_add(1);
        Ok(())
    }

    fn create_job(
        &self,
        request: &HostBuildDeployRequest,
        identity: &HostAccessIdentity,
    ) -> anyhow::Result<CreateBuildDeployJobResult> {
        let request_fingerprint = build_deploy_request_fingerprint(request);
        if let Some(idempotency_key) = request.idempotency_key.as_deref() {
            let jobs = self.jobs.lock().expect("jobs lock poisoned");
            if let Some(existing) = jobs.values().find(|job| {
                job.project_id == request.project_id
                    && job.idempotency_key.as_deref() == Some(idempotency_key)
            }) {
                if existing.request_fingerprint != request_fingerprint {
                    anyhow::bail!(
                        "idempotency_key was already used for a different build-deploy request"
                    );
                }
                return Ok(CreateBuildDeployJobResult {
                    job_id: existing.job_id.clone(),
                    created: false,
                    state: existing.state,
                    permit: None,
                });
            }
        }
        let permit = self
            .global_sem
            .clone()
            .try_acquire_owned()
            .map_err(|_| anyhow::anyhow!("build-deploy global concurrency limit reached"))?;
        {
            let mut active = self.project_active.lock().expect("project lock poisoned");
            if active.contains(&request.project_id) {
                anyhow::bail!(
                    "build-deploy project concurrency limit reached (max {BUILD_DEPLOY_MAX_PER_PROJECT_ACTIVE})"
                );
            }
            active.insert(request.project_id.clone());
        }
        let now = now_millis();
        let job_id = format!(
            "bdj-{now}-{}-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..8],
            sanitize_container_name(&request.route_id)
        );
        let authority = DeploymentAuthorityLease::from_identity(job_id.clone(), "local", identity);
        let mut jobs = self.jobs.lock().expect("jobs lock poisoned");
        jobs.insert(
            job_id.clone(),
            BuildDeployJobRecord {
                job_id: job_id.clone(),
                project_id: request.project_id.clone(),
                route_id: request.route_id.clone(),
                build_id: request.build_id.clone(),
                state: BuildDeployJobState::Queued,
                created_at_ms: now,
                updated_at_ms: now,
                result: None,
                error: None,
                events: VecDeque::new(),
                next_sequence: 1,
                cancel: Arc::new(AtomicBool::new(false)),
                idempotency_key: request.idempotency_key.clone(),
                request_fingerprint,
                operation: DeploymentOperation::BuildDeploy,
                authority,
            },
        );
        drop(jobs);
        self.push_event(&job_id, BuildDeployJobState::Queued, "job queued");
        self.prune();
        Ok(CreateBuildDeployJobResult {
            job_id,
            created: true,
            state: BuildDeployJobState::Queued,
            permit: Some(permit),
        })
    }

    fn status(&self, job_id: &str) -> Option<BuildDeployJobStatusResponse> {
        let jobs = self.jobs.lock().expect("jobs lock poisoned");
        let job = jobs.get(job_id)?;
        Some(job_status_response(job))
    }

    fn events(&self, job_id: &str) -> Option<Vec<BuildDeployJobEvent>> {
        let jobs = self.jobs.lock().expect("jobs lock poisoned");
        Some(jobs.get(job_id)?.events.iter().cloned().collect())
    }

    fn subscribe(&self) -> broadcast::Receiver<BuildDeployJobEvent> {
        self.notifier.subscribe()
    }

    fn cancel(&self, job_id: &str) -> Option<(BuildDeployJobState, bool)> {
        let cancel = {
            let jobs = self.jobs.lock().expect("jobs lock poisoned");
            let job = jobs.get(job_id)?;
            if job.state.terminal() {
                return Some((job.state, false));
            }
            job.cancel.clone()
        };
        cancel.store(true, Ordering::SeqCst);
        self.push_event(job_id, BuildDeployJobState::Cancelled, "cancel requested");
        self.status(job_id).map(|status| (status.state, true))
    }

    async fn acquire(&self, project_id: &ProjectId) -> anyhow::Result<OwnedSemaphorePermit> {
        let permit = self
            .global_sem
            .clone()
            .try_acquire_owned()
            .map_err(|_| anyhow::anyhow!("build-deploy global concurrency limit reached"))?;
        let _ = project_id;
        Ok(permit)
    }

    async fn acquire_project_operation(
        &self,
        project_id: &ProjectId,
    ) -> anyhow::Result<OwnedSemaphorePermit> {
        let permit = self.acquire(project_id).await?;
        let mut active = self.project_active.lock().expect("project lock poisoned");
        if !active.insert(project_id.clone()) {
            anyhow::bail!(
                "deployment project concurrency limit reached (max {BUILD_DEPLOY_MAX_PER_PROJECT_ACTIVE})"
            );
        }
        Ok(permit)
    }

    fn release_project(&self, project_id: &ProjectId) {
        self.project_active
            .lock()
            .expect("project lock poisoned")
            .remove(project_id);
    }

    fn discard_job(&self, job_id: &str) {
        let removed = self.jobs.lock().expect("jobs lock poisoned").remove(job_id);
        if let Some(job) = removed {
            self.release_project(&job.project_id);
        }
    }

    fn cancel_flag(&self, job_id: &str) -> Option<Arc<AtomicBool>> {
        self.jobs
            .lock()
            .expect("jobs lock poisoned")
            .get(job_id)
            .map(|job| job.cancel.clone())
    }

    fn transition(&self, job_id: &str, state: BuildDeployJobState, message: &str) {
        self.push_event(job_id, state, message);
    }

    fn complete_ready(&self, job_id: &str, result: HostBuildDeployResponse) {
        let mut jobs = self.jobs.lock().expect("jobs lock poisoned");
        if let Some(job) = jobs.get_mut(job_id) {
            if job.state.terminal() {
                if job.state == BuildDeployJobState::Ready {
                    job.build_id = Some(result.build_id.clone());
                    job.result = Some(result);
                }
                return;
            }
            job.build_id = Some(result.build_id.clone());
            job.result = Some(result);
        }
        drop(jobs);
        self.push_event(job_id, BuildDeployJobState::Ready, "deployment ready");
    }

    fn complete_error(&self, job_id: &str, state: BuildDeployJobState, error: String) {
        let redacted = redact_build_log(&error);
        let mut jobs = self.jobs.lock().expect("jobs lock poisoned");
        if let Some(job) = jobs.get_mut(job_id) {
            if job.state.terminal() {
                return;
            }
            job.error = Some(redacted.clone());
        }
        drop(jobs);
        self.push_event(job_id, state, &redacted);
    }

    fn push_event(&self, job_id: &str, state: BuildDeployJobState, message: &str) {
        let mut jobs = self.jobs.lock().expect("jobs lock poisoned");
        let Some(job) = jobs.get_mut(job_id) else {
            return;
        };
        if job.state.terminal() && state != job.state {
            return;
        }
        let event = BuildDeployJobEvent {
            job_id: job_id.to_string(),
            sequence: job.next_sequence,
            state,
            message: redact_build_log(message),
            timestamp_ms: now_millis(),
        };
        job.next_sequence += 1;
        job.state = state;
        job.updated_at_ms = event.timestamp_ms;
        job.events.push_back(event.clone());
        while job.events.len() > BUILD_DEPLOY_LOG_RING {
            job.events.pop_front();
        }
        drop(jobs);
        let _ = self.notifier.send(event);
    }

    fn prune(&self) {
        let mut jobs = self.jobs.lock().expect("jobs lock poisoned");
        if jobs.len() <= BUILD_DEPLOY_MAX_RETAINED_JOBS {
            return;
        }
        let mut terminal = jobs
            .values()
            .filter(|job| job.state.terminal())
            .map(|job| (job.created_at_ms, job.job_id.clone()))
            .collect::<Vec<_>>();
        terminal.sort();
        for (_, id) in terminal
            .into_iter()
            .take(jobs.len() - BUILD_DEPLOY_MAX_RETAINED_JOBS)
        {
            jobs.remove(&id);
        }
    }

    fn job_snapshot(&self, job_id: &str) -> Option<DeploymentJobSnapshot> {
        let jobs = self.jobs.lock().expect("jobs lock poisoned");
        let job = jobs.get(job_id)?;
        Some(DeploymentJobSnapshot {
            status: job_status_response(job),
            event: job.events.back().cloned(),
            request_fingerprint: job.request_fingerprint.clone(),
            authority: Some(job.authority.clone()),
        })
    }

    fn ready_snapshot(
        &self,
        job_id: &str,
        result: &HostBuildDeployResponse,
    ) -> Option<DeploymentJobSnapshot> {
        let jobs = self.jobs.lock().expect("jobs lock poisoned");
        let job = jobs.get(job_id)?;
        let timestamp_ms = now_millis();
        let event = BuildDeployJobEvent {
            job_id: job_id.to_string(),
            sequence: job.next_sequence,
            state: BuildDeployJobState::Ready,
            message: "deployment ready".to_string(),
            timestamp_ms,
        };
        let mut status = job_status_response(job);
        status.state = BuildDeployJobState::Ready;
        status.updated_at_ms = timestamp_ms;
        status.build_id = Some(result.build_id.clone());
        status.result = Some(result.clone());
        status.error = None;
        Some(DeploymentJobSnapshot {
            status,
            event: Some(event),
            request_fingerprint: job.request_fingerprint.clone(),
            authority: Some(job.authority.clone()),
        })
    }

    fn restore_job_snapshot(&self, snapshot: DeploymentJobSnapshot) {
        let mut jobs = self.jobs.lock().expect("jobs lock poisoned");
        let mut restored_event = None;
        let request_fingerprint = snapshot.request_fingerprint;
        let authority = snapshot.authority.unwrap_or_else(|| {
            DeploymentAuthorityLease::unavailable(snapshot.status.job_id.clone())
        });
        let status = snapshot.status;
        let record = jobs
            .entry(status.job_id.clone())
            .or_insert_with(|| BuildDeployJobRecord {
                job_id: status.job_id.clone(),
                project_id: status.project_id.clone(),
                route_id: status.route_id.clone(),
                build_id: status.build_id.clone(),
                state: status.state,
                created_at_ms: status.created_at_ms,
                updated_at_ms: status.updated_at_ms,
                result: status.result.clone(),
                error: status.error.clone(),
                events: VecDeque::new(),
                next_sequence: 1,
                cancel: Arc::new(AtomicBool::new(false)),
                idempotency_key: status.idempotency_key.clone(),
                request_fingerprint: request_fingerprint.clone(),
                operation: status.operation,
                authority: authority.clone(),
            });
        record.project_id = status.project_id;
        record.route_id = status.route_id;
        record.build_id = status.build_id;
        record.state = status.state;
        record.created_at_ms = status.created_at_ms;
        record.updated_at_ms = status.updated_at_ms;
        record.result = status.result;
        record.error = status.error;
        record.idempotency_key = status.idempotency_key;
        record.request_fingerprint = request_fingerprint;
        record.operation = status.operation;
        record.authority = authority;
        if let Some(event) = snapshot.event {
            if !record
                .events
                .iter()
                .any(|existing| existing.sequence == event.sequence)
            {
                record.next_sequence = record.next_sequence.max(event.sequence + 1);
                record.events.push_back(event.clone());
                restored_event = Some(event);
                while record.events.len() > BUILD_DEPLOY_LOG_RING {
                    record.events.pop_front();
                }
            }
        }
        drop(jobs);
        if let Some(event) = restored_event {
            let _ = self.notifier.send(event);
        }
    }

    fn job_authority(&self, job_id: &str) -> Option<DeploymentAuthorityLease> {
        self.jobs
            .lock()
            .expect("jobs lock poisoned")
            .get(job_id)
            .map(|job| job.authority.clone())
    }

    fn interrupt_incomplete_jobs(&self) -> Vec<String> {
        let ids = {
            let jobs = self.jobs.lock().expect("jobs lock poisoned");
            jobs.values()
                .filter(|job| !job.state.terminal())
                .map(|job| job.job_id.clone())
                .collect::<Vec<_>>()
        };
        for job_id in &ids {
            self.complete_error(
                job_id,
                BuildDeployJobState::Failed,
                "host restarted before the deployment job completed".to_string(),
            );
        }
        ids
    }

    fn register_revision(&self, revision: DeploymentRevision) {
        let mut deployment = self.deployment.lock().expect("deployment lock poisoned");
        deployment.direct_route_owners.remove(&revision.route_id);
        let revisions = deployment
            .revisions
            .entry(revision.project_id.clone())
            .or_default();
        if !revisions
            .iter()
            .any(|existing| existing.revision_id == revision.revision_id)
        {
            revisions.push(revision.clone());
            if revisions.len() > BUILD_DEPLOY_MAX_REVISIONS_PER_PROJECT {
                let excess = revisions.len() - BUILD_DEPLOY_MAX_REVISIONS_PER_PROJECT;
                revisions.drain(..excess);
            }
        }
        deployment
            .active_revisions
            .insert(revision.project_id.clone(), revision.revision_id.clone());
    }

    fn deactivate_revision(&self, deactivation: &DeploymentRevisionDeactivated) {
        let mut deployment = self.deployment.lock().expect("deployment lock poisoned");
        if deployment
            .active_revisions
            .get(&deactivation.project_id)
            .is_some_and(|revision_id| revision_id == &deactivation.revision_id)
        {
            deployment.active_revisions.remove(&deactivation.project_id);
        }
    }

    fn active_revision(&self, project_id: &ProjectId) -> Option<DeploymentRevision> {
        let deployment = self.deployment.lock().expect("deployment lock poisoned");
        let revision_id = deployment.active_revisions.get(project_id)?;
        deployment
            .revisions
            .get(project_id)?
            .iter()
            .find(|revision| &revision.revision_id == revision_id)
            .cloned()
    }

    fn ensure_revision_parent(&self, revision: &DeploymentRevision) -> anyhow::Result<()> {
        let current = self
            .deployment
            .lock()
            .expect("deployment lock poisoned")
            .active_revisions
            .get(&revision.project_id)
            .cloned();
        anyhow::ensure!(
            current == revision.parent_revision_id,
            "deployment activation parent is stale"
        );
        Ok(())
    }

    fn revision(&self, project_id: &ProjectId, revision_id: &str) -> Option<DeploymentRevision> {
        self.deployment
            .lock()
            .expect("deployment lock poisoned")
            .revisions
            .get(project_id)?
            .iter()
            .find(|revision| revision.revision_id == revision_id)
            .cloned()
    }

    fn revisions(&self, project_id: &ProjectId) -> Vec<DeploymentRevision> {
        let mut revisions = self
            .deployment
            .lock()
            .expect("deployment lock poisoned")
            .revisions
            .get(project_id)
            .cloned()
            .unwrap_or_default();
        revisions.sort_by_key(|revision| std::cmp::Reverse(revision.created_at_ms));
        revisions
    }

    fn jobs_for_project(&self, project_id: &ProjectId) -> Vec<BuildDeployJobStatusResponse> {
        let jobs = self.jobs.lock().expect("jobs lock poisoned");
        let mut statuses = jobs
            .values()
            .filter(|job| &job.project_id == project_id)
            .map(job_status_response)
            .collect::<Vec<_>>();
        statuses.sort_by_key(|status| std::cmp::Reverse(status.created_at_ms));
        statuses
    }

    fn active_by_route(&self, route_id: &str) -> Option<DeploymentRevision> {
        let deployment = self.deployment.lock().expect("deployment lock poisoned");
        deployment
            .active_revisions
            .iter()
            .find_map(|(project_id, revision_id)| {
                deployment
                    .revisions
                    .get(project_id)?
                    .iter()
                    .find(|revision| {
                        revision.revision_id == revision_id.as_str()
                            && revision.route_id == route_id
                    })
                    .cloned()
            })
    }

    fn register_direct_route_owner(&self, ownership: DeploymentDirectRouteOwned) {
        let mut deployment = self.deployment.lock().expect("deployment lock poisoned");
        let replaced_active =
            deployment
                .active_revisions
                .iter()
                .find_map(|(active_project_id, revision_id)| {
                    deployment
                        .revisions
                        .get(active_project_id)
                        .and_then(|revisions| {
                            revisions
                                .iter()
                                .any(|revision| {
                                    revision.revision_id == revision_id.as_str()
                                        && revision.route_id == ownership.route_id
                                })
                                .then(|| active_project_id.clone())
                        })
                });
        if let Some(active_project_id) = replaced_active {
            deployment.active_revisions.remove(&active_project_id);
        }
        deployment
            .direct_route_owners
            .insert(ownership.route_id.clone(), ownership);
    }

    fn project_for_route(&self, route_id: &str) -> Option<ProjectId> {
        let deployment = self.deployment.lock().expect("deployment lock poisoned");
        deployment
            .active_revisions
            .iter()
            .find_map(|(project_id, revision_id)| {
                deployment
                    .revisions
                    .get(project_id)?
                    .iter()
                    .any(|revision| {
                        revision.revision_id == revision_id.as_str()
                            && revision.route_id == route_id
                    })
                    .then(|| project_id.clone())
            })
            .or_else(|| {
                deployment
                    .direct_route_owners
                    .get(route_id)
                    .map(|ownership| ownership.project_id.clone())
            })
    }

    fn ensure_route_available_for_project(
        &self,
        route_id: &str,
        project_id: &ProjectId,
    ) -> anyhow::Result<()> {
        if let Some(owner) = self.project_for_route(route_id) {
            anyhow::ensure!(
                &owner == project_id,
                "deployment route is owned by another project"
            );
        }
        Ok(())
    }

    fn durable_routes(&self) -> Vec<DurableDeploymentRoute> {
        let deployment = self.deployment.lock().expect("deployment lock poisoned");
        let mut routes = deployment
            .active_revisions
            .iter()
            .filter_map(|(project_id, revision_id)| {
                deployment
                    .revisions
                    .get(project_id)?
                    .iter()
                    .find(|revision| revision.revision_id == *revision_id)
                    .map(|revision| DurableDeploymentRoute {
                        route_id: revision.route_id.clone(),
                        port_name: revision.port_name.clone(),
                        route_access: revision.route_access,
                        port_lease_id: revision.receipt.port_lease_id.clone(),
                    })
            })
            .collect::<Vec<_>>();
        routes.extend(deployment.direct_route_owners.values().map(|ownership| {
            DurableDeploymentRoute {
                route_id: ownership.route_id.clone(),
                port_name: ownership.port_name.clone(),
                route_access: ownership.route_access,
                port_lease_id: ownership.port_lease_id.clone(),
            }
        }));
        routes
    }
}

fn job_status_response(job: &BuildDeployJobRecord) -> BuildDeployJobStatusResponse {
    BuildDeployJobStatusResponse {
        job_id: job.job_id.clone(),
        project_id: job.project_id.clone(),
        route_id: job.route_id.clone(),
        build_id: job.build_id.clone(),
        state: job.state,
        created_at_ms: job.created_at_ms,
        updated_at_ms: job.updated_at_ms,
        result: job.result.clone(),
        error: job.error.clone(),
        events_url: format!("/host/v1/build-deploy/{}/events", job.job_id),
        idempotency_key: job.idempotency_key.clone(),
        operation: job.operation,
    }
}

async fn append_deployment_journal_event<S, T>(
    store: &S,
    expected_next_sequence: EventSequence,
    kind: &str,
    payload: &T,
) -> anyhow::Result<Option<EventEnvelope>>
where
    S: EventStore,
    T: Serialize,
{
    store
        .append_with_sequence_if_next(
            DEPLOYMENT_JOURNAL_SESSION.to_string(),
            expected_next_sequence,
            DEPLOYMENT_JOURNAL_WRITER.to_string(),
            kind.to_string(),
            1,
            serde_json::to_value(payload)?,
            serde_json::json!({ "owner": "host_control_plane", "redacted": true }),
        )
        .await
}

async fn sync_deployment_journal<S>(
    store: &S,
    registry: &BuildDeployJobRegistry,
) -> anyhow::Result<usize>
where
    S: EventStore,
{
    let mut loaded = 0usize;
    loop {
        let next = registry.journal_next_sequence();
        let events = store
            .list_session_range(
                &DEPLOYMENT_JOURNAL_SESSION.to_string(),
                next.checked_sub(1),
                Some(1_000),
            )
            .await?;
        if events.is_empty() {
            break;
        }
        for event in &events {
            registry.apply_journal_event(event)?;
            loaded = loaded.saturating_add(1);
        }
        if events.len() < 1_000 {
            break;
        }
    }
    Ok(loaded)
}

async fn persist_deployment_journal_event<S, T, F>(
    store: &S,
    registry: &BuildDeployJobRegistry,
    kind: &str,
    payload: &T,
    validate: F,
) -> anyhow::Result<EventEnvelope>
where
    S: EventStore,
    T: Serialize,
    F: Fn() -> anyhow::Result<()>,
{
    for _ in 0..8 {
        sync_deployment_journal(store, registry).await?;
        validate()?;
        let expected_next = registry.journal_next_sequence();
        if let Some(event) =
            append_deployment_journal_event(store, expected_next, kind, payload).await?
        {
            registry.apply_journal_event(&event)?;
            return Ok(event);
        }
    }
    anyhow::bail!("deployment journal changed concurrently")
}

async fn persist_job_snapshot<S>(state: &AppState<S>, job_id: &str) -> anyhow::Result<()>
where
    S: EventStore,
{
    let snapshot = state
        .build_jobs
        .job_snapshot(job_id)
        .ok_or_else(|| anyhow::anyhow!("build-deploy job disappeared before persistence"))?;
    persist_deployment_journal_event(
        state.runtime.store().as_ref(),
        state.build_jobs.as_ref(),
        DEPLOYMENT_JOB_SNAPSHOT_EVENT,
        &snapshot,
        || Ok(()),
    )
    .await
    .map(|_| ())
}

async fn persist_revision_activation<S>(
    state: &AppState<S>,
    revision: &DeploymentRevision,
    job: Option<DeploymentJobSnapshot>,
    authority: Option<DeploymentAuthorityLease>,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    let activation = DeploymentRevisionActivated {
        revision: revision.clone(),
        enforce_parent: true,
        job,
        authority,
    };
    persist_deployment_journal_event(
        state.runtime.store().as_ref(),
        state.build_jobs.as_ref(),
        DEPLOYMENT_REVISION_ACTIVATED_EVENT,
        &activation,
        || state.build_jobs.ensure_revision_parent(revision),
    )
    .await
    .map(|_| ())
}

async fn persist_revision_deactivation<S>(
    state: &AppState<S>,
    deactivation: &DeploymentRevisionDeactivated,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    persist_deployment_journal_event(
        state.runtime.store().as_ref(),
        state.build_jobs.as_ref(),
        DEPLOYMENT_REVISION_DEACTIVATED_EVENT,
        deactivation,
        || Ok(()),
    )
    .await
    .map(|_| ())
}

async fn persist_direct_route_ownership<S>(
    state: &AppState<S>,
    ownership: &DeploymentDirectRouteOwned,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    persist_deployment_journal_event(
        state.runtime.store().as_ref(),
        state.build_jobs.as_ref(),
        DEPLOYMENT_DIRECT_ROUTE_OWNED_EVENT,
        ownership,
        || Ok(()),
    )
    .await
    .map(|_| ())
}

async fn persist_direct_route_release<S>(
    state: &AppState<S>,
    release: &DeploymentDirectRouteReleased,
) -> anyhow::Result<()>
where
    S: EventStore,
{
    persist_deployment_journal_event(
        state.runtime.store().as_ref(),
        state.build_jobs.as_ref(),
        DEPLOYMENT_DIRECT_ROUTE_RELEASED_EVENT,
        release,
        || Ok(()),
    )
    .await
    .map(|_| ())
}

pub async fn hydrate_deployment_control_plane<S>(
    store: Arc<S>,
    registry: Arc<BuildDeployJobRegistry>,
) -> anyhow::Result<usize>
where
    S: EventStore,
{
    let loaded = sync_deployment_journal(store.as_ref(), registry.as_ref()).await?;
    for job_id in registry.interrupt_incomplete_jobs() {
        if let Some(snapshot) = registry.job_snapshot(&job_id) {
            persist_deployment_journal_event(
                store.as_ref(),
                registry.as_ref(),
                DEPLOYMENT_JOB_SNAPSHOT_EVENT,
                &snapshot,
                || Ok(()),
            )
            .await?;
        }
    }
    registry.prune();
    Ok(loaded)
}

#[derive(Debug)]
pub struct DeploymentControlPlaneReconcileSummary {
    pub durable_routes_restored: usize,
    pub target_deployments_projected: usize,
    pub orphan_candidates_found: usize,
    pub runtime: plurora_runtime::DeploymentReconcileSummary,
}

pub async fn reconcile_deployment_control_plane<S>(
    state: &AppState<S>,
) -> anyhow::Result<DeploymentControlPlaneReconcileSummary>
where
    S: EventStore,
{
    development::verify_host_control_plane_lease_if_installed(
        state.runtime.store().as_ref(),
        state.development.as_ref(),
    )
    .await?;
    let context = ProtocolContext::host_dev("host_deployment_startup_reconcile");
    let durable_routes = state.build_jobs.durable_routes();
    let mut durable_by_route = HashMap::new();
    for route in &durable_routes {
        if let Some(existing) = durable_by_route.insert(route.route_id.clone(), route.clone()) {
            anyhow::ensure!(
                existing.port_lease_id == route.port_lease_id,
                "multiple durable deployments claim the same route"
            );
        }
    }

    let mut durable_routes_restored = 0usize;
    for route in &durable_routes {
        let current = state
            .runtime
            .config()
            .proxy_route_registry
            .status(&route.route_id)
            .await;
        if !current.as_ref().is_some_and(|current| {
            current.status != ProxyRouteStatusKind::Removed
                && current.upstream.port_lease_id == route.port_lease_id
        }) {
            anyhow::ensure!(
                !route.port_name.is_empty(),
                "durable deployment route is missing its port name"
            );
            call_host_protocol(
                state,
                &context,
                "host.proxy.register",
                serde_json::json!({
                    "route_id": route.route_id,
                    "protocol": "http",
                    "access": route.route_access,
                    "upstream": {
                        "port_lease_id": route.port_lease_id,
                        "port_name": route.port_name,
                    },
                }),
            )
            .await?;
            durable_routes_restored = durable_routes_restored.saturating_add(1);
        }
        let _ = state
            .runtime
            .config()
            .proxy_route_registry
            .set_status(&route.route_id, ProxyRouteStatusKind::Stale)
            .await;
        let _ = state
            .runtime
            .config()
            .proxy_route_registry
            .set_ready(&route.route_id, false)
            .await;
    }

    // Target-operation receipts are the specific owner of local/Agent target
    // containers. Project them before the generic Docker-broker reconcile so
    // its legacy local-container scan cannot remove their routes or leases.
    // If projection is uncertain, fail closed with the stale records intact.
    let target_deployments_projected =
        target_agent::reconcile_target_deployment_control_plane(state).await?;

    let managed = state
        .runtime
        .config()
        .deployment_reconcile_source
        .list_managed()
        .await?;
    let mut seen = HashSet::new();
    let mut orphan_candidates_found = 0usize;
    let mut orphan_cleanup_warnings = Vec::new();
    let mut legacy_unowned_resource_found = false;
    for container in managed {
        let route_id = container.route_id;
        let port_lease_id = container.port_lease_id;
        if durable_by_route
            .get(&route_id)
            .is_some_and(|route| route.port_lease_id == port_lease_id)
        {
            continue;
        }
        if !durable_by_route.contains_key(&route_id) && container.operation_id.is_none() {
            // Pre-Phase-2 direct deployments have no controller journal or operation label.
            // Preserve them instead of guessing that they are abandoned candidates.
            if !state
                .runtime
                .config()
                .proxy_route_registry
                .status(&route_id)
                .await
                .is_some_and(|route| {
                    route.status != ProxyRouteStatusKind::Removed
                        && route.upstream.port_lease_id == port_lease_id
                })
            {
                legacy_unowned_resource_found = true;
            }
            continue;
        }
        if !seen.insert((
            route_id.clone(),
            port_lease_id.clone(),
            container.container_ref.clone(),
        )) {
            continue;
        }
        orphan_candidates_found = orphan_candidates_found.saturating_add(1);
        let unregister_route = state
            .runtime
            .config()
            .proxy_route_registry
            .status(&route_id)
            .await
            .is_some_and(|route| route.upstream.port_lease_id == port_lease_id);
        orphan_cleanup_warnings.extend(
            cleanup_deployment_resources(
                state,
                &context,
                &route_id,
                unregister_route,
                container.container_ref.as_deref(),
                Some(&port_lease_id),
            )
            .await,
        );
    }
    if !orphan_cleanup_warnings.is_empty() {
        for warning in &orphan_cleanup_warnings {
            eprintln!("warning: startup orphan cleanup incomplete: {warning}");
        }
        anyhow::bail!("deployment reconcile paused because orphan cleanup was not confirmed");
    }
    anyhow::ensure!(
        !legacy_unowned_resource_found,
        "deployment reconcile paused for a pre-Phase-2 container without durable route ownership"
    );

    let runtime = state.runtime.reconcile_deployment().await?;
    Ok(DeploymentControlPlaneReconcileSummary {
        durable_routes_restored,
        target_deployments_projected,
        orphan_candidates_found,
        runtime,
    })
}

async fn deployment_effect_context<S>(
    state: &AppState<S>,
    authority: Option<&DeploymentAuthorityLease>,
    project_id: &ProjectId,
    transport: &str,
) -> anyhow::Result<ProtocolContext>
where
    S: EventStore,
{
    development::verify_host_control_plane_lease_if_installed(
        state.runtime.store().as_ref(),
        state.development.as_ref(),
    )
    .await?;
    let Some(authority) = authority else {
        return Ok(ProtocolContext::host_dev(transport));
    };
    if authority.identity_kind == HostAccessIdentityKind::Device {
        host_access::sync_host_access_journal(
            state.runtime.store().as_ref(),
            state.host_access.as_ref(),
        )
        .await?;
    }
    authority.validate(project_id, state.host_access.as_ref())?;
    Ok(authority.protocol_context(project_id, transport))
}

async fn build_job_effect_context<S>(
    state: &AppState<S>,
    job_id: Option<&str>,
    project_id: &ProjectId,
    transport: &str,
) -> anyhow::Result<ProtocolContext>
where
    S: EventStore,
{
    let authority = job_id
        .map(|job_id| {
            state
                .build_jobs
                .job_authority(job_id)
                .ok_or_else(|| anyhow::anyhow!("build-deploy authority lease disappeared"))
        })
        .transpose()?;
    deployment_effect_context(state, authority.as_ref(), project_id, transport).await
}

async fn deployment_operation_effect_context<S>(
    state: &AppState<S>,
    job_id: Option<&str>,
    authority: Option<&DeploymentAuthorityLease>,
    project_id: &ProjectId,
    transport: &str,
) -> anyhow::Result<ProtocolContext>
where
    S: EventStore,
{
    if authority.is_some() {
        deployment_effect_context(state, authority, project_id, transport).await
    } else {
        build_job_effect_context(state, job_id, project_id, transport).await
    }
}

impl fmt::Debug for ResolvedRuntimeEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResolvedRuntimeEnv")
            .field("name", &self.name)
            .field("source", &self.source)
            .field("value", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectWorkspaceCloneRequest {
    pub project_id: ProjectId,
    pub source_url: String,
    pub ref_name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectWorkspaceCloneResult {
    pub project_id: ProjectId,
    pub ref_name: String,
    pub commit_sha: String,
    pub tree_hash: Option<String>,
    pub files_written: Option<u64>,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitFetchTreeInvocation {
    resolve_ref_params: Value,
    fetch_tree_params: Value,
    workspace_dir: PathBuf,
    staging_dir: PathBuf,
}

async fn deploy_project<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Json(request): Json<HostDeployRequest>,
) -> anyhow::Result<Json<HostDeployResponse>, ServiceError>
where
    S: EventStore,
{
    validate_host_deploy_request(&request)?;
    if let Some(project_id) = request.project_id.as_ref() {
        require_identity_project(&identity, project_id.as_str())?;
    } else if identity.kind == HostAccessIdentityKind::Device
        && !identity.allows_all(HostAccessResourceKind::Project)
    {
        return Err(ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "project-scoped devices must provide project_id for direct deployments",
        ));
    }
    require_identity_target(&identity, "local")?;
    development::verify_host_control_plane_lease_if_installed(
        state.runtime.store().as_ref(),
        state.development.as_ref(),
    )
    .await?;
    let durable_route_project = state.build_jobs.project_for_route(&request.route_id);
    if let Some(project_id) = request.project_id.as_ref() {
        state
            .build_jobs
            .ensure_route_available_for_project(&request.route_id, project_id)
            .map_err(|error| ServiceError::with_status(StatusCode::CONFLICT, error.to_string()))?;
    } else if durable_route_project.is_some() {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "project_id is required when replacing an owned deployment route",
        ));
    }
    let authority = request.project_id.as_ref().map(|_| {
        DeploymentAuthorityLease::from_identity(
            format!("dop-{}", uuid::Uuid::new_v4().simple()),
            "local",
            &identity,
        )
    });
    let _project_operation = if let Some(project_id) = request.project_id.as_ref() {
        let permit = state
            .build_jobs
            .acquire_project_operation(project_id)
            .await
            .map_err(|error| ServiceError::with_status(StatusCode::CONFLICT, error.to_string()))?;
        Some((
            permit,
            BuildDeployProjectGuard {
                registry: state.build_jobs.clone(),
                project_id: project_id.clone(),
            },
        ))
    } else {
        None
    };
    let previous_route = state
        .runtime
        .config()
        .proxy_route_registry
        .status(&request.route_id)
        .await
        .filter(|route| route.status == ProxyRouteStatusKind::Active);
    let mut context = match request.project_id.as_ref() {
        Some(project_id) => {
            deployment_effect_context(
                &state,
                authority.as_ref(),
                project_id,
                "host_deploy_prepare",
            )
            .await?
        }
        None => identity.protocol_context("host_deploy"),
    };
    let previous_container = if let Some(previous_route) = previous_route.as_ref() {
        let output = invoke_docker_runtime_lab(
            &state,
            &context,
            "plurora/docker-runtime-lab/list_managed",
            serde_json::json!({}),
        )
        .await
        .context("managed container lookup before route replacement failed")?;
        find_managed_container_for_route(
            &output,
            &request.route_id,
            Some(&previous_route.upstream.port_lease_id),
        )?
    } else {
        None
    };
    if previous_route.is_some() && durable_route_project.is_none() && previous_container.is_none() {
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            "existing route is not owned by a managed deployment",
        ));
    }
    let lease = match call_host_protocol(
        &state,
        &context,
        "host.port.lease",
        serde_json::json!({
            "target_id": "local",
            "port_name": &request.port_name,
            "protocol": "tcp",
        }),
    )
    .await
    .and_then(|value| value_field(value, "lease", "host.port.lease"))
    {
        Ok(lease) => lease,
        Err(error) => return Err(anyhow::anyhow!("deployment port lease failed: {error}").into()),
    };

    let lease_id = required_string(&lease, "id", "port lease")?;
    let lease_port = required_u16(&lease, "port", "port lease")?;
    let port_lease_id = lease_id.clone();

    if let Some(project_id) = request.project_id.as_ref() {
        context = match deployment_effect_context(
            &state,
            authority.as_ref(),
            project_id,
            "host_deploy_candidate_start",
        )
        .await
        {
            Ok(context) => context,
            Err(error) => {
                let cleanup_context = ProtocolContext::host_dev("host_deploy_compensation");
                rollback_deploy(
                    &state,
                    &cleanup_context,
                    &request.route_id,
                    false,
                    None,
                    Some(&lease_id),
                )
                .await;
                return Err(error.into());
            }
        };
    }

    let start_output = match invoke_docker_runtime_lab(
        &state,
        &context,
        "plurora/docker-runtime-lab/start_container",
        serde_json::json!({
            "image": &request.image,
            "container_port": request.container_port,
            "host_port": lease_port,
            "route_id": &request.route_id,
            "port_lease_id": &port_lease_id,
            "approved": true,
            "pull_if_missing": request.pull_if_missing,
            "operation_id": authority.as_ref().map(|authority| authority.operation_id.as_str()),
        }),
    )
    .await
    {
        Ok(output) => output,
        Err(error) => {
            // Protocol dispatch failed before the Docker provider returned an effect receipt.
            rollback_deploy(
                &state,
                &ProtocolContext::host_dev("host_deploy_compensation"),
                &request.route_id,
                false,
                None,
                Some(&lease_id),
            )
            .await;
            return Err(anyhow::anyhow!("docker container start failed: {error}").into());
        }
    };

    let parsed_container_id = match require_started_container(&start_output) {
        Ok(container_id) => container_id,
        Err(error) => {
            cleanup_candidate_after_unknown_start(
                &state,
                &request.route_id,
                &lease_id,
                "host_deploy_unknown_start",
            )
            .await;
            return Err(error.into());
        }
    };
    let container_name = optional_string(&start_output, "container_name");

    if let Err(error) =
        wait_for_deployment_readiness(lease_port, request.health_path.as_deref()).await
    {
        rollback_deploy(
            &state,
            &ProtocolContext::host_dev("host_deploy_compensation"),
            &request.route_id,
            false,
            Some(&parsed_container_id),
            Some(&lease_id),
        )
        .await;
        return Err(anyhow::anyhow!("deployment did not become ready in time: {error}").into());
    }

    if let Some(project_id) = request.project_id.as_ref() {
        context = match deployment_effect_context(
            &state,
            authority.as_ref(),
            project_id,
            "host_deploy_route_activation",
        )
        .await
        {
            Ok(context) => context,
            Err(error) => {
                rollback_deploy(
                    &state,
                    &ProtocolContext::host_dev("host_deploy_compensation"),
                    &request.route_id,
                    false,
                    Some(&parsed_container_id),
                    Some(&lease_id),
                )
                .await;
                return Err(error.into());
            }
        };
    }
    let route = match call_host_protocol(
        &state,
        &context,
        "host.proxy.register",
        serde_json::json!({
            "route_id": &request.route_id,
            "protocol": "http",
            "access": request.route_access,
            "upstream": {
                "port_lease_id": &port_lease_id,
                "port_name": &request.port_name,
            },
        }),
    )
    .await
    .and_then(|value| value_field(value, "route", "host.proxy.register"))
    {
        Ok(route) => route,
        Err(error) => {
            rollback_deploy(
                &state,
                &context,
                &request.route_id,
                false,
                Some(&parsed_container_id),
                Some(&lease_id),
            )
            .await;
            return Err(anyhow::anyhow!("proxy registration failed: {error}").into());
        }
    };
    let (route_id, fallback_public_url) = match (
        required_string(&route, "id", "proxy route"),
        required_string(&route, "public_url", "proxy route"),
    ) {
        (Ok(route_id), Ok(public_url)) => (route_id, public_url),
        (Err(error), _) | (_, Err(error)) => {
            rollback_deploy(
                &state,
                &ProtocolContext::host_dev("host_deploy_compensation"),
                &request.route_id,
                true,
                Some(&parsed_container_id),
                Some(&lease_id),
            )
            .await;
            return Err(error.into());
        }
    };
    let public_url = service_public_url_for_route(
        &state,
        &request.route_id,
        &fallback_public_url,
        request.route_access,
    );

    if state
        .runtime
        .config()
        .proxy_route_registry
        .set_ready_if_active_with_lease(&route_id, &lease_id, true)
        .await
        .is_none()
    {
        rollback_deploy(
            &state,
            &context,
            &route_id,
            true,
            Some(&parsed_container_id),
            Some(&lease_id),
        )
        .await;
        return Err(anyhow::anyhow!("proxy route disappeared before readiness promotion").into());
    }

    if let Some(project_id) = request.project_id.clone() {
        let ownership = DeploymentDirectRouteOwned {
            route_id: route_id.clone(),
            project_id,
            port_name: request.port_name.clone(),
            route_access: request.route_access,
            port_lease_id: port_lease_id.clone(),
            container_id: parsed_container_id.clone(),
            timestamp_ms: now_millis(),
            authority: authority.clone(),
        };
        if let Err(error) = persist_direct_route_ownership(&state, &ownership).await {
            let mut unregister_candidate = previous_route.is_none();
            if let Some(previous) = previous_route.as_ref() {
                match restore_proxy_route_if_candidate_active(
                    &state,
                    &route_id,
                    &port_lease_id,
                    &previous.upstream.port_lease_id,
                    &previous.upstream.port_name,
                    previous.access,
                    previous.ready,
                    "host_deploy_journal_rollback",
                )
                .await
                {
                    Ok(true) | Ok(false) => unregister_candidate = false,
                    Err(restore_error) => {
                        eprintln!(
                            "warning: failed to restore route after direct deployment journal failure: {restore_error}"
                        );
                        unregister_candidate = true;
                    }
                }
            }
            rollback_deploy(
                &state,
                &ProtocolContext::host_dev("host_deploy_journal_rollback"),
                &route_id,
                unregister_candidate,
                Some(&parsed_container_id),
                Some(&port_lease_id),
            )
            .await;
            return Err(ServiceError::with_status(
                StatusCode::INTERNAL_SERVER_ERROR,
                redacted_failure_message("direct deployment journal commit", &error),
            ));
        }
    }

    if let Some(previous) = previous_route.as_ref() {
        rollback_deploy(
            &state,
            &ProtocolContext::host_dev("host_deploy_previous_route_drain"),
            &route_id,
            false,
            previous_container
                .as_ref()
                .map(|container| container.container.as_str()),
            Some(&previous.upstream.port_lease_id),
        )
        .await;
    }

    Ok(Json(HostDeployResponse {
        route_id,
        public_url,
        route_access: request.route_access,
        port_lease_id,
        container_id: parsed_container_id,
        container_name,
    }))
}

async fn build_deploy_project<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Query(query): Query<BuildDeploySubmitQuery>,
    Json(request): Json<HostBuildDeployRequest>,
) -> anyhow::Result<Json<BuildDeploySubmitOrStatusResponse>, ServiceError>
where
    S: EventStore,
{
    require_identity_project(&identity, request.project_id.as_str())?;
    require_identity_target(&identity, "local")?;
    validate_host_build_deploy_request(&request).map_err(redacted_build_deploy_error)?;
    state
        .build_jobs
        .ensure_route_available_for_project(&request.route_id, &request.project_id)
        .map_err(|error| ServiceError::with_status(StatusCode::CONFLICT, error.to_string()))?;
    let created = state
        .build_jobs
        .create_job(&request, &identity)
        .map_err(|error| {
            let message = error.to_string();
            let status = if message.contains("idempotency_key") {
                StatusCode::CONFLICT
            } else {
                StatusCode::TOO_MANY_REQUESTS
            };
            ServiceError::with_status(status, message)
        })?;
    let CreateBuildDeployJobResult {
        job_id,
        created,
        state: created_state,
        permit,
    } = created;
    if created {
        if let Err(error) = persist_job_snapshot(&state, &job_id).await {
            state.build_jobs.discard_job(&job_id);
            let public_error =
                redacted_failure_message("deployment job intent journal commit", &error);
            return Err(ServiceError::with_status(
                StatusCode::INTERNAL_SERVER_ERROR,
                public_error,
            ));
        }
        let permit = permit.expect("new build-deploy job reserves global capacity");
        let worker_state = state.clone();
        let worker_job_id = job_id.clone();
        tokio::spawn(async move {
            run_build_deploy_job(worker_state, worker_job_id, request, permit).await;
        });
    }
    if query.wait {
        let status = wait_for_build_job(&state, &job_id, BUILD_DEPLOY_WAIT_TIMEOUT).await;
        return Ok(Json(BuildDeploySubmitOrStatusResponse::Status(status)));
    }
    Ok(Json(BuildDeploySubmitOrStatusResponse::Submitted(
        BuildDeployJobSubmitResponse {
            status_url: format!("/host/v1/build-deploy/{job_id}"),
            events_url: format!("/host/v1/build-deploy/{job_id}/events"),
            state: created_state,
            job_id,
        },
    )))
}

async fn build_deploy_job_status<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path(job_id): Path<String>,
) -> anyhow::Result<Json<BuildDeployJobStatusResponse>, ServiceError>
where
    S: EventStore,
{
    let status = state.build_jobs.status(&job_id).ok_or_else(|| {
        ServiceError::with_status(StatusCode::NOT_FOUND, "build-deploy job not found")
    })?;
    require_identity_project(&identity, status.project_id.as_str())?;
    Ok(Json(status))
}

async fn cancel_build_deploy_job<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path(job_id): Path<String>,
) -> anyhow::Result<Json<BuildDeployCancelResponse>, ServiceError>
where
    S: EventStore,
{
    let status = state.build_jobs.status(&job_id).ok_or_else(|| {
        ServiceError::with_status(StatusCode::NOT_FOUND, "build-deploy job not found")
    })?;
    require_identity_project(&identity, status.project_id.as_str())?;
    let (state_value, cancelled) = state.build_jobs.cancel(&job_id).ok_or_else(|| {
        ServiceError::with_status(StatusCode::NOT_FOUND, "build-deploy job not found")
    })?;
    if cancelled {
        persist_job_snapshot(&state, &job_id)
            .await
            .map_err(redacted_build_deploy_error)?;
    }
    Ok(Json(BuildDeployCancelResponse {
        job_id,
        state: state_value,
        cancelled,
    }))
}

async fn build_deploy_job_events<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path(job_id): Path<String>,
) -> anyhow::Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, ServiceError>
where
    S: EventStore,
{
    let status = state.build_jobs.status(&job_id).ok_or_else(|| {
        ServiceError::with_status(StatusCode::NOT_FOUND, "build-deploy job not found")
    })?;
    require_identity_project(&identity, status.project_id.as_str())?;
    let replay = state.build_jobs.events(&job_id).ok_or_else(|| {
        ServiceError::with_status(StatusCode::NOT_FOUND, "build-deploy job not found")
    })?;
    let rx = state.build_jobs.subscribe();
    let stream = futures::stream::unfold((replay, 0usize, rx), move |(replay, idx, mut rx)| {
        let job_id = job_id.clone();
        async move {
            if idx < replay.len() {
                let event = replay[idx].clone();
                return Some((sse_json_event(&event), (replay, idx + 1, rx)));
            }
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if event.job_id == job_id {
                            return Some((sse_json_event(&event), (replay, idx, rx)));
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

fn sse_json_event(event: &BuildDeployJobEvent) -> Result<SseEvent, Infallible> {
    Ok(SseEvent::default()
        .event("build_deploy")
        .data(serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string())))
}

async fn wait_for_build_job<S>(
    state: &AppState<S>,
    job_id: &str,
    max_wait: Duration,
) -> BuildDeployJobStatusResponse
where
    S: EventStore,
{
    let deadline = Instant::now() + max_wait;
    loop {
        if let Some(status) = state.build_jobs.status(job_id) {
            if status.state.terminal() || Instant::now() >= deadline {
                return status;
            }
        }
        sleep(Duration::from_millis(100)).await;
    }
}

async fn run_build_deploy_job<S>(
    state: AppState<S>,
    job_id: String,
    request: HostBuildDeployRequest,
    permit: OwnedSemaphorePermit,
) where
    S: EventStore,
{
    let _permit = permit;
    let _project_guard = BuildDeployProjectGuard {
        registry: state.build_jobs.clone(),
        project_id: request.project_id.clone(),
    };
    let revision_request = request.clone();
    let result = build_deploy_project_minimal_with_job(&state, &job_id, request).await;
    match result {
        Ok(outcome) => {
            let mut result = outcome.response;
            if state
                .build_jobs
                .cancel_flag(&job_id)
                .is_some_and(|flag| flag.load(Ordering::SeqCst))
            {
                compensate_candidate_after_activation_failure(
                    &state,
                    &result,
                    outcome.previous_revision.as_ref(),
                    "host_build_deploy_cancel_rollback",
                )
                .await;
                if let Err(persist_error) = persist_job_snapshot(&state, &job_id).await {
                    eprintln!("failed to persist cancelled build-deploy job: {persist_error}");
                }
                return;
            }
            let revision = deployment_revision_from_build(
                &revision_request,
                &result,
                &job_id,
                outcome
                    .previous_revision
                    .as_ref()
                    .map(|revision| revision.revision_id.clone()),
            );
            let ready_snapshot = state.build_jobs.ready_snapshot(&job_id, &result);
            let authority = state.build_jobs.job_authority(&job_id);
            if let Err(error) =
                persist_revision_activation(&state, &revision, ready_snapshot, authority).await
            {
                compensate_candidate_after_activation_failure(
                    &state,
                    &result,
                    outcome.previous_revision.as_ref(),
                    "host_build_deploy_journal_rollback",
                )
                .await;
                let public_error = redacted_failure_message("deployment journal commit", &error);
                state
                    .build_jobs
                    .complete_error(&job_id, BuildDeployJobState::Failed, public_error);
                if let Err(persist_error) = persist_job_snapshot(&state, &job_id).await {
                    eprintln!("failed to persist journal rollback failure: {persist_error}");
                }
                return;
            }
            if let Some(previous) = outcome.previous_revision.as_ref() {
                let route_id = result.route_id.clone();
                let warnings = drain_previous_revision(&state, previous, &route_id).await;
                result.warnings.extend(warnings);
            }
            let cleanup_incomplete = !result.warnings.is_empty();
            state.build_jobs.complete_ready(&job_id, result);
            if cleanup_incomplete {
                if let Err(error) = persist_job_snapshot(&state, &job_id).await {
                    eprintln!("failed to persist deployment cleanup warnings: {error}");
                }
            }
        }
        Err(error) => {
            let state_kind = if state
                .build_jobs
                .cancel_flag(&job_id)
                .is_some_and(|flag| flag.load(Ordering::SeqCst))
            {
                BuildDeployJobState::Cancelled
            } else {
                BuildDeployJobState::Failed
            };
            let public_error = redacted_failure_message("build-deploy", &error);
            state
                .build_jobs
                .complete_error(&job_id, state_kind, public_error);
            if let Err(persist_error) = persist_job_snapshot(&state, &job_id).await {
                eprintln!("failed to persist build-deploy terminal state: {persist_error}");
            }
        }
    }
}

fn deployment_revision_from_build(
    request: &HostBuildDeployRequest,
    result: &HostBuildDeployResponse,
    job_id: &str,
    parent_revision_id: Option<String>,
) -> DeploymentRevision {
    let mut recovery_blockers = Vec::new();
    let mut runtime_env = Vec::new();
    for env in &request.runtime_env {
        match (env.value.as_ref(), env.secret_ref.as_ref()) {
            (None, Some(secret_ref)) => runtime_env.push(PersistedRuntimeEnvSpec {
                name: env.name.clone(),
                secret_ref: secret_ref.clone(),
            }),
            (Some(_), None) => recovery_blockers.push(format!(
                "runtime env {} used a non-persisted plain value",
                env.name
            )),
            _ => recovery_blockers.push(format!(
                "runtime env {} did not have a replay-safe source",
                env.name
            )),
        }
    }
    if !request.runtime_mounts.is_empty() {
        recovery_blockers.push(
            "host mount paths are intentionally not persisted for automatic recovery".to_string(),
        );
    }
    DeploymentRevision {
        revision_id: format!(
            "drv-{}-{}",
            now_millis(),
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        ),
        project_id: request.project_id.clone(),
        job_id: Some(job_id.to_string()),
        operation: DeploymentOperation::BuildDeploy,
        parent_revision_id,
        created_at_ms: now_millis(),
        target_id: default_local_target_id(),
        source_kind: DeploymentSourceKind::GitClone,
        source_url: request.source_url.clone(),
        ref_name: request.ref_name.clone(),
        dockerfile: request.dockerfile.clone(),
        container_port: request.container_port,
        port_name: request.port_name.clone(),
        route_id: request.route_id.clone(),
        route_access: request.route_access,
        health_path: request.health_path.clone(),
        image: result.image.clone(),
        build_id: result.build_id.clone(),
        source_commit: result.source_commit.clone(),
        build_descriptor_hash: result.build_descriptor_hash.clone(),
        strategy: result.strategy.clone(),
        runtime_env,
        verified_change_set_id: None,
        verification_ref: None,
        build_context_ref: None,
        preview_ref: None,
        approval_ref: None,
        verified_build_network_mode: None,
        target_deployment: None,
        recoverable: recovery_blockers.is_empty(),
        recovery_blockers,
        receipt: result.clone(),
    }
}

pub async fn build_deploy_project_minimal<S>(
    state: &AppState<S>,
    request: HostBuildDeployRequest,
) -> anyhow::Result<HostBuildDeployResponse>
where
    S: EventStore,
{
    let mut outcome = build_deploy_project_minimal_inner(state, None, request).await?;
    if let Some(previous) = outcome.previous_revision.as_ref() {
        let route_id = outcome.response.route_id.clone();
        let warnings = drain_previous_revision(state, previous, &route_id).await;
        outcome.response.warnings.extend(warnings);
    }
    Ok(outcome.response)
}

async fn build_deploy_project_minimal_with_job<S>(
    state: &AppState<S>,
    job_id: &str,
    request: HostBuildDeployRequest,
) -> anyhow::Result<BuildDeployOutcome>
where
    S: EventStore,
{
    build_deploy_project_minimal_inner(state, Some(job_id), request).await
}

async fn build_deploy_project_minimal_inner<S>(
    state: &AppState<S>,
    job_id: Option<&str>,
    request: HostBuildDeployRequest,
) -> anyhow::Result<BuildDeployOutcome>
where
    S: EventStore,
{
    validate_host_build_deploy_request(&request)?;
    state
        .build_jobs
        .ensure_route_available_for_project(&request.route_id, &request.project_id)?;
    let previous_revision = state.build_jobs.active_revision(&request.project_id);
    check_job_cancel(state, job_id)?;
    job_transition(
        state,
        job_id,
        BuildDeployJobState::Cloning,
        "cloning source",
    )
    .await;
    let clone_context = build_job_effect_context(
        state,
        job_id,
        &request.project_id,
        "host_build_deploy_clone",
    )
    .await?;
    let clone = clone_project_workspace_from_git_with_context(
        state,
        ProjectWorkspaceCloneRequest {
            project_id: request.project_id.clone(),
            source_url: request.source_url.clone(),
            ref_name: request.ref_name.clone(),
        },
        &clone_context,
    )
    .await
    .context("project workspace clone failed")?;
    check_job_cancel(state, job_id)?;

    let source_commit = request
        .source_commit
        .clone()
        .unwrap_or_else(|| clone.commit_sha.clone());
    if source_commit != clone.commit_sha {
        anyhow::bail!("source_commit did not match resolved clone commit");
    }
    let build_id = request
        .build_id
        .clone()
        .unwrap_or_else(|| generated_build_id(&source_commit));
    let strategy = request
        .strategy
        .as_deref()
        .unwrap_or("dockerfile")
        .to_string();
    let dockerfile = request
        .dockerfile
        .clone()
        .unwrap_or_else(|| "Dockerfile".to_string());
    let build_descriptor_hash = build_deploy_descriptor_hash(&request, &build_id, &source_commit);
    let resolved_env = resolve_runtime_env(state, &request).await?;
    let env_summary = resolved_env
        .iter()
        .map(|env| RuntimeEnvSummary {
            name: env.name.clone(),
            source: env.source,
        })
        .collect::<Vec<_>>();
    let resolved_mounts = resolve_runtime_mounts(&request)?;
    let mount_summary = resolved_mounts
        .iter()
        .map(|mount| RuntimeMountSummary {
            container_path: mount.container_path.clone(),
            mode: mount.mode,
            source_basename: mount.source_basename.clone(),
            source_kind: mount.source_kind.clone(),
            source_hash: mount.source_hash.clone(),
            approved: true,
        })
        .collect::<Vec<_>>();
    let workspace_dir = plurora_core::paths::project_workspace_dir(&request.project_id)?;
    job_transition(
        state,
        job_id,
        BuildDeployJobState::Building,
        "building image",
    )
    .await;
    let context = build_job_effect_context(
        state,
        job_id,
        &request.project_id,
        "host_build_deploy_image_build",
    )
    .await?;
    let build_output = invoke_docker_runtime_lab(
        state,
        &context,
        "plurora/docker-runtime-lab/build_image",
        serde_json::json!({
            "approved": true,
            "strategy": strategy,
            "project_id": request.project_id.as_str(),
            "build_id": build_id,
            "context_dir": workspace_dir.to_string_lossy(),
            "dockerfile": dockerfile,
            "source_commit": source_commit,
            "build_descriptor_hash": build_descriptor_hash,
        }),
    )
    .await
    .context("docker image build failed")?;
    let image = require_built_image(&build_output)?;
    check_job_cancel(state, job_id)?;
    job_transition(
        state,
        job_id,
        BuildDeployJobState::Starting,
        "starting container",
    )
    .await;

    let deploy = deploy_built_image(
        state,
        job_id,
        None,
        &request,
        &image,
        &build_id,
        &source_commit,
        &resolved_env,
        &resolved_mounts,
    )
    .await
    .map_err(|error| {
        anyhow::anyhow!("built image was not garbage-collected after deploy failure: {error}")
    })?;

    Ok(BuildDeployOutcome {
        response: HostBuildDeployResponse {
            route_id: deploy.route_id,
            public_url: deploy.public_url,
            route_access: deploy.route_access,
            port_lease_id: deploy.port_lease_id,
            container_id: deploy.container_id,
            container_name: deploy.container_name,
            image: image.clone(),
            build_id,
            source_commit,
            build_descriptor_hash,
            strategy,
            runtime_env: env_summary,
            runtime_mounts: mount_summary,
            warnings: Vec::new(),
        },
        previous_revision,
    })
}

fn check_job_cancel<S>(state: &AppState<S>, job_id: Option<&str>) -> anyhow::Result<()>
where
    S: EventStore,
{
    if let Some(job_id) = job_id {
        if state
            .build_jobs
            .cancel_flag(job_id)
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
        {
            anyhow::bail!("build-deploy job cancelled");
        }
    }
    Ok(())
}

async fn job_transition<S>(
    state: &AppState<S>,
    job_id: Option<&str>,
    status: BuildDeployJobState,
    message: &str,
) where
    S: EventStore,
{
    if let Some(job_id) = job_id {
        state.build_jobs.transition(job_id, status, message);
        if let Err(error) = persist_job_snapshot(state, job_id).await {
            eprintln!("failed to persist build-deploy transition: {error}");
        }
    }
}

async fn project_deployments<S>(
    State(state): State<AppState<S>>,
    Path(project_id): Path<ProjectId>,
) -> Json<ProjectDeploymentsResponse>
where
    S: EventStore,
{
    let active_revision = state.build_jobs.active_revision(&project_id);
    let runtime_ready = match active_revision.as_ref() {
        Some(revision) => state
            .runtime
            .config()
            .proxy_route_registry
            .status(&revision.route_id)
            .await
            .is_some_and(|route| route.status == ProxyRouteStatusKind::Active && route.ready),
        None => false,
    };
    Json(ProjectDeploymentsResponse {
        project_id: project_id.clone(),
        active_revision_id: active_revision
            .as_ref()
            .map(|revision| revision.revision_id.clone()),
        recovery_required: active_revision.is_some() && !runtime_ready,
        runtime_ready,
        active_revision,
        jobs: state.build_jobs.jobs_for_project(&project_id),
        revisions: state.build_jobs.revisions(&project_id),
    })
}

async fn recover_project_deployment<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path(project_id): Path<ProjectId>,
) -> anyhow::Result<Json<DeploymentActionResponse>, ServiceError>
where
    S: EventStore,
{
    require_identity_project(&identity, project_id.as_str())?;
    let expected_active = state
        .build_jobs
        .active_revision(&project_id)
        .ok_or_else(|| {
            ServiceError::with_status(
                StatusCode::NOT_FOUND,
                "project has no active deployment revision to recover",
            )
        })?;
    require_identity_target(&identity, &expected_active.target_id)?;
    let authority = DeploymentAuthorityLease::from_identity(
        format!("dop-{}", uuid::Uuid::new_v4().simple()),
        expected_active.target_id.clone(),
        &identity,
    );
    let permit = state
        .build_jobs
        .acquire_project_operation(&project_id)
        .await
        .map_err(|error| ServiceError::with_status(StatusCode::CONFLICT, error.to_string()))?;
    let result = async {
        let active = state
            .build_jobs
            .active_revision(&project_id)
            .ok_or_else(|| {
                ServiceError::with_status(
                    StatusCode::NOT_FOUND,
                    "project has no active deployment revision to recover",
                )
            })?;
        if active.target_id != authority.target_id {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "active deployment target changed before recovery began",
            ));
        }
        let runtime_ready = state
            .runtime
            .config()
            .proxy_route_registry
            .status(&active.route_id)
            .await
            .is_some_and(|route| route.status == ProxyRouteStatusKind::Active && route.ready);
        if runtime_ready {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "active deployment is already ready",
            ));
        }
        activate_persisted_revision(
            &state,
            Some(&active),
            &active,
            DeploymentOperation::Recover,
            &authority,
        )
        .await
        .map(Json)
    }
    .await;
    state.build_jobs.release_project(&project_id);
    drop(permit);
    result
}

async fn rollback_project_deployment<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path(project_id): Path<ProjectId>,
    Json(request): Json<DeploymentRollbackRequest>,
) -> anyhow::Result<Json<DeploymentActionResponse>, ServiceError>
where
    S: EventStore,
{
    require_identity_project(&identity, project_id.as_str())?;
    let expected_target = state
        .build_jobs
        .revision(&project_id, request.revision_id.trim())
        .ok_or_else(|| {
            ServiceError::with_status(
                StatusCode::NOT_FOUND,
                "deployment revision was not found for this project",
            )
        })?;
    require_identity_target(&identity, &expected_target.target_id)?;
    let authority = DeploymentAuthorityLease::from_identity(
        format!("dop-{}", uuid::Uuid::new_v4().simple()),
        expected_target.target_id.clone(),
        &identity,
    );
    let permit = state
        .build_jobs
        .acquire_project_operation(&project_id)
        .await
        .map_err(|error| ServiceError::with_status(StatusCode::CONFLICT, error.to_string()))?;
    let result = async {
        let active = state.build_jobs.active_revision(&project_id);
        let target = state
            .build_jobs
            .revision(&project_id, request.revision_id.trim())
            .ok_or_else(|| {
                ServiceError::with_status(
                    StatusCode::NOT_FOUND,
                    "deployment revision was not found for this project",
                )
            })?;
        if target.target_id != authority.target_id {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "deployment revision target changed before rollback began",
            ));
        }
        if active
            .as_ref()
            .is_some_and(|active| target.revision_id == active.revision_id)
        {
            return Err(ServiceError::with_status(
                StatusCode::CONFLICT,
                "requested deployment revision is already active",
            ));
        }
        activate_persisted_revision(
            &state,
            active.as_ref(),
            &target,
            DeploymentOperation::Rollback,
            &authority,
        )
        .await
        .map(Json)
    }
    .await;
    state.build_jobs.release_project(&project_id);
    drop(permit);
    result
}

async fn activate_persisted_revision<S>(
    state: &AppState<S>,
    previous: Option<&DeploymentRevision>,
    target: &DeploymentRevision,
    operation: DeploymentOperation,
    authority: &DeploymentAuthorityLease,
) -> anyhow::Result<DeploymentActionResponse, ServiceError>
where
    S: EventStore,
{
    if !target.recoverable {
        let blockers = if target.recovery_blockers.is_empty() {
            "revision does not contain replay-safe runtime inputs".to_string()
        } else {
            target.recovery_blockers.join("; ")
        };
        return Err(ServiceError::with_status(
            StatusCode::CONFLICT,
            format!("deployment revision is not recoverable: {blockers}"),
        ));
    }

    if target.source_kind == DeploymentSourceKind::VerifiedArtifact {
        return development::activate_verified_persisted_revision(
            state, previous, target, operation, authority,
        )
        .await;
    }

    let replay_request = build_replay_request(target);
    validate_host_build_deploy_request(&replay_request).map_err(redacted_build_deploy_error)?;
    deployment_effect_context(
        state,
        Some(authority),
        &target.project_id,
        "host_deployment_replay_prepare",
    )
    .await
    .map_err(redacted_build_deploy_error)?;
    let resolved_env = resolve_runtime_env(state, &replay_request)
        .await
        .map_err(redacted_build_deploy_error)?;

    let deploy = deploy_built_image(
        state,
        None,
        Some(authority),
        &replay_request,
        &target.image,
        &target.build_id,
        &target.source_commit,
        &resolved_env,
        &[],
    )
    .await
    .map_err(redacted_build_deploy_error)?;
    let receipt = HostBuildDeployResponse {
        route_id: deploy.route_id,
        public_url: deploy.public_url,
        route_access: deploy.route_access,
        port_lease_id: deploy.port_lease_id,
        container_id: deploy.container_id,
        container_name: deploy.container_name,
        image: target.image.clone(),
        build_id: target.build_id.clone(),
        source_commit: target.source_commit.clone(),
        build_descriptor_hash: target.build_descriptor_hash.clone(),
        strategy: target.strategy.clone(),
        runtime_env: resolved_env
            .iter()
            .map(|env| RuntimeEnvSummary {
                name: env.name.clone(),
                source: env.source,
            })
            .collect(),
        runtime_mounts: Vec::new(),
        warnings: Vec::new(),
    };
    let revision = deployment_revision_from_replay(previous, target, operation, receipt);
    if let Err(error) =
        persist_revision_activation(state, &revision, None, Some(authority.clone())).await
    {
        compensate_candidate_after_activation_failure(
            state,
            &revision.receipt,
            previous,
            "host_deployment_replay_journal_rollback",
        )
        .await;
        let public_error = redacted_failure_message("deployment revision journal commit", &error);
        return Err(ServiceError::with_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            public_error,
        ));
    }
    let warnings = match previous {
        Some(previous) => {
            drain_previous_revision(state, previous, &revision.receipt.route_id).await
        }
        None => Vec::new(),
    };
    Ok(DeploymentActionResponse {
        operation,
        previous_revision_id: previous.map(|revision| revision.revision_id.clone()),
        revision,
        warnings,
    })
}

fn build_replay_request(revision: &DeploymentRevision) -> HostBuildDeployRequest {
    HostBuildDeployRequest {
        project_id: revision.project_id.clone(),
        source_url: revision.source_url.clone(),
        ref_name: revision.ref_name.clone(),
        strategy: Some(revision.strategy.clone()),
        dockerfile: revision.dockerfile.clone(),
        container_port: revision.container_port,
        port_name: revision.port_name.clone(),
        route_id: revision.route_id.clone(),
        route_access: revision.route_access,
        health_path: revision.health_path.clone(),
        approved: true,
        source_commit: Some(revision.source_commit.clone()),
        build_id: Some(revision.build_id.clone()),
        runtime_env: revision
            .runtime_env
            .iter()
            .map(|env| RuntimeEnvSpec {
                name: env.name.clone(),
                value: None,
                secret_ref: Some(env.secret_ref.clone()),
            })
            .collect(),
        runtime_mounts: Vec::new(),
        idempotency_key: None,
    }
}

fn deployment_revision_from_replay(
    previous: Option<&DeploymentRevision>,
    target: &DeploymentRevision,
    operation: DeploymentOperation,
    receipt: HostBuildDeployResponse,
) -> DeploymentRevision {
    DeploymentRevision {
        revision_id: format!(
            "drv-{}-{}",
            now_millis(),
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        ),
        project_id: target.project_id.clone(),
        job_id: None,
        operation,
        parent_revision_id: previous.map(|revision| revision.revision_id.clone()),
        created_at_ms: now_millis(),
        target_id: target.target_id.clone(),
        source_kind: target.source_kind,
        source_url: target.source_url.clone(),
        ref_name: target.ref_name.clone(),
        dockerfile: target.dockerfile.clone(),
        container_port: target.container_port,
        port_name: target.port_name.clone(),
        route_id: target.route_id.clone(),
        route_access: target.route_access,
        health_path: target.health_path.clone(),
        image: target.image.clone(),
        build_id: target.build_id.clone(),
        source_commit: target.source_commit.clone(),
        build_descriptor_hash: target.build_descriptor_hash.clone(),
        strategy: target.strategy.clone(),
        runtime_env: target.runtime_env.clone(),
        verified_change_set_id: target.verified_change_set_id.clone(),
        verification_ref: target.verification_ref.clone(),
        build_context_ref: target.build_context_ref.clone(),
        preview_ref: target.preview_ref.clone(),
        approval_ref: target.approval_ref.clone(),
        verified_build_network_mode: target.verified_build_network_mode,
        target_deployment: target.target_deployment.clone(),
        recoverable: target.recoverable,
        recovery_blockers: target.recovery_blockers.clone(),
        receipt,
    }
}

struct DeploymentCleanupResult {
    response: HostDeployStopResponse,
    safe_to_redeploy: bool,
}

async fn stop_project_deployment<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Json(request): Json<HostDeployStopRequest>,
) -> Result<Json<HostDeployStopResponse>, ServiceError>
where
    S: EventStore,
{
    let route_id = request.route_id.trim().to_string();
    let active = state.build_jobs.active_by_route(&route_id);
    let route_project = state.build_jobs.project_for_route(&route_id);
    if let Some(project_id) = route_project.as_ref() {
        require_identity_project(&identity, project_id.as_str())?;
    } else if identity.kind == HostAccessIdentityKind::Device
        && !identity.allows_all(HostAccessResourceKind::Project)
    {
        return Err(ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "project-scoped devices cannot stop an unowned deployment route",
        ));
    }
    development::verify_host_control_plane_lease_if_installed(
        state.runtime.store().as_ref(),
        state.development.as_ref(),
    )
    .await?;
    let _project_operation = if let Some(project_id) = route_project.as_ref() {
        let permit = state
            .build_jobs
            .acquire_project_operation(project_id)
            .await
            .map_err(|error| ServiceError::with_status(StatusCode::CONFLICT, error.to_string()))?;
        Some((
            permit,
            BuildDeployProjectGuard {
                registry: state.build_jobs.clone(),
                project_id: project_id.clone(),
            },
        ))
    } else {
        None
    };
    let context = match route_project.as_ref() {
        Some(project_id) => {
            let authority = DeploymentAuthorityLease::from_identity(
                format!("dop-{}", uuid::Uuid::new_v4().simple()),
                "local",
                &identity,
            );
            deployment_effect_context(&state, Some(&authority), project_id, "host_deploy_stop")
                .await?
        }
        None => identity
            .protocol_context("host_deploy_stop")
            .with_host_operation(
                "deploy",
                vec![ProtocolResourceSelector {
                    owner: "host".to_string(),
                    kind: "target".to_string(),
                    id: Some("local".to_string()),
                }],
            ),
    };
    let mut cleanup = stop_project_deployment_inner(&state, &route_id, &context).await;
    if cleanup.safe_to_redeploy {
        if let Some(revision) = active {
            let deactivation = DeploymentRevisionDeactivated {
                project_id: revision.project_id,
                revision_id: revision.revision_id,
                route_id: revision.route_id,
                reason: "explicit_stop".to_string(),
                timestamp_ms: now_millis(),
            };
            match persist_revision_deactivation(&state, &deactivation).await {
                Ok(()) => {}
                Err(error) => cleanup.response.warnings.push(redacted_failure_message(
                    "deployment stop journal commit",
                    &error,
                )),
            }
        } else if let Some(project_id) = route_project {
            let release = DeploymentDirectRouteReleased {
                route_id: route_id.clone(),
                project_id,
                timestamp_ms: now_millis(),
            };
            if let Err(error) = persist_direct_route_release(&state, &release).await {
                cleanup.response.warnings.push(redacted_failure_message(
                    "direct deployment stop journal commit",
                    &error,
                ));
            }
        }
    }
    Ok(Json(cleanup.response))
}

async fn stop_project_deployment_inner<S>(
    state: &AppState<S>,
    route_id: &str,
    context: &ProtocolContext,
) -> DeploymentCleanupResult
where
    S: EventStore,
{
    let mut warnings = Vec::new();
    let route_id = route_id.trim().to_string();
    if !is_safe_route_token(&route_id) {
        return DeploymentCleanupResult {
            response: HostDeployStopResponse {
                route_id,
                stopped: false,
                warnings: vec!["route_id must be label-safe".to_string()],
            },
            safe_to_redeploy: false,
        };
    }

    let route_record = state
        .runtime
        .config()
        .proxy_route_registry
        .status(&route_id)
        .await;
    let registered_route = route_record
        .as_ref()
        .filter(|route| route.status != ProxyRouteStatusKind::Removed);
    let mut port_lease_id = route_record
        .as_ref()
        .map(|route| route.upstream.port_lease_id.clone());

    let mut container_ref = None;
    let mut safe_to_redeploy = false;
    match invoke_docker_runtime_lab(
        state,
        context,
        "plurora/docker-runtime-lab/list_managed",
        serde_json::json!({}),
    )
    .await
    {
        Ok(output) => {
            match find_managed_container_for_route(&output, &route_id, port_lease_id.as_deref()) {
                Ok(found) => {
                    container_ref = found;
                    if port_lease_id.is_none() {
                        port_lease_id = container_ref
                            .as_ref()
                            .map(|container| container.port_lease_id.clone());
                    }
                    safe_to_redeploy = container_ref.is_none();
                }
                Err(error) => warnings.push(format!("managed container lookup failed: {error}")),
            }
        }
        Err(error) => warnings.push(format!("managed container list unavailable: {error}")),
    }

    let mut stopped = false;
    if let Some(container) = container_ref.as_ref() {
        match invoke_docker_runtime_lab(
            state,
            context,
            "plurora/docker-runtime-lab/stop_container",
            serde_json::json!({
                "approved": true,
                "container_id": container.container,
                "route_id": route_id,
                "port_lease_id": container.port_lease_id,
                "timeout_secs": 10
            }),
        )
        .await
        {
            Ok(output) => {
                stopped = output
                    .get("docker_performed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    && output
                        .get("removed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                if !stopped {
                    warnings.push(
                        output
                            .get("reason")
                            .and_then(Value::as_str)
                            .unwrap_or("docker-runtime-lab did not stop the container")
                            .to_string(),
                    );
                } else {
                    safe_to_redeploy = true;
                }
            }
            Err(error) => warnings.push(format!("container stop failed: {error}")),
        }
    } else {
        warnings.push("no managed container found for route".to_string());
    }

    if safe_to_redeploy {
        if registered_route.is_some() {
            if let Err(error) = call_host_protocol(
                state,
                context,
                "host.proxy.unregister",
                serde_json::json!({ "route_id": route_id }),
            )
            .await
            {
                warnings.push(format!("proxy unregister failed: {error}"));
                safe_to_redeploy = false;
            }
        }
        if let Some(lease_id) = port_lease_id.as_ref() {
            if let Err(error) = call_host_protocol(
                state,
                context,
                "host.port.release",
                serde_json::json!({ "lease_id": lease_id }),
            )
            .await
            {
                warnings.push(format!("port release failed: {error}"));
                safe_to_redeploy = false;
            }
        }
    } else {
        warnings
            .push("route and port lease were preserved because cleanup is incomplete".to_string());
    }

    DeploymentCleanupResult {
        response: HostDeployStopResponse {
            route_id,
            stopped,
            warnings,
        },
        safe_to_redeploy,
    }
}

async fn call_host_protocol<S>(
    state: &AppState<S>,
    context: &ProtocolContext,
    method: &str,
    params: Value,
) -> anyhow::Result<Value>
where
    S: EventStore,
{
    state
        .runtime
        .call_protocol(context, method, params)
        .await
        .map_err(protocol_error_to_anyhow)
}

async fn start_build_deploy_container(
    request: HostDockerStartRequest<'_>,
) -> anyhow::Result<HostDockerStartedContainer> {
    let docker = Docker::connect_with_local_defaults()
        .or_else(|_| Docker::connect_with_defaults())
        .context("docker connection unavailable")?;
    docker.ping().await.context("docker daemon unavailable")?;
    let container_port_key = format!("{}/tcp", request.container_port);
    let mut port_bindings: PortMap = HashMap::new();
    port_bindings.insert(
        container_port_key.clone(),
        Some(vec![PortBinding {
            host_ip: Some("127.0.0.1".to_string()),
            host_port: Some(request.host_port.to_string()),
        }]),
    );
    let labels = HashMap::from([
        ("managed-by".to_string(), "plurora".to_string()),
        (
            "plurora.package_id".to_string(),
            DOCKER_RUNTIME_PACKAGE_ID.to_string(),
        ),
        ("plurora.route_id".to_string(), request.route_id.to_string()),
        (
            "plurora.port_lease_id".to_string(),
            request.port_lease_id.to_string(),
        ),
        (
            "plurora.project_id".to_string(),
            request.project_id.to_string(),
        ),
        ("plurora.build_id".to_string(), request.build_id.to_string()),
        (
            "plurora.source_commit".to_string(),
            request.source_commit.to_string(),
        ),
        (
            "plurora.deployment_operation_id".to_string(),
            request.operation_id.to_string(),
        ),
    ]);
    let env = request
        .env
        .iter()
        .map(|entry| format!("{}={}", entry.name, entry.value))
        .collect::<Vec<_>>();
    let mounts = request
        .mounts
        .iter()
        .map(|mount| Mount {
            typ: Some(MountType::BIND),
            source: Some(mount.source.to_string_lossy().to_string()),
            target: Some(mount.container_path.clone()),
            read_only: Some(mount.mode == RuntimeMountMode::Ro),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let config = ContainerCreateBody {
        image: Some(request.image.to_string()),
        labels: Some(labels),
        exposed_ports: Some(vec![container_port_key]),
        env: (!env.is_empty()).then_some(env),
        host_config: Some(HostConfig {
            network_mode: Some("bridge".to_string()),
            port_bindings: Some(port_bindings),
            mounts: (!mounts.is_empty()).then_some(mounts),
            publish_all_ports: Some(false),
            privileged: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    };
    let container_name = format!(
        "plurora-build-deploy-{}-{}",
        sanitize_container_name(request.route_id),
        request.host_port
    );
    let options = CreateContainerOptionsBuilder::default()
        .name(&container_name)
        .build();
    let created = docker
        .create_container(Some(options), config)
        .await
        .context("docker create failed")?;
    docker
        .start_container(&created.id, None)
        .await
        .context("docker start failed")?;
    Ok(HostDockerStartedContainer {
        container_id: created.id,
        container_name: Some(container_name),
    })
}

async fn deploy_built_image<S>(
    state: &AppState<S>,
    job_id: Option<&str>,
    authority: Option<&DeploymentAuthorityLease>,
    request: &HostBuildDeployRequest,
    image: &str,
    build_id: &str,
    source_commit: &str,
    env: &[ResolvedRuntimeEnv],
    mounts: &[ResolvedRuntimeMount],
) -> anyhow::Result<DeployBuiltImageResponse>
where
    S: EventStore,
{
    let cleanup_context = ProtocolContext::host_dev("host_build_deploy_compensation");
    let operation_id = authority
        .map(|authority| authority.operation_id.clone())
        .or_else(|| job_id.map(str::to_string))
        .unwrap_or_else(|| format!("dop-{}", uuid::Uuid::new_v4().simple()));
    let mut container_id: Option<String> = None;
    let mut context = deployment_operation_effect_context(
        state,
        job_id,
        authority,
        &request.project_id,
        "host_build_deploy_port_lease",
    )
    .await?;
    let lease = match call_host_protocol(
        state,
        &context,
        "host.port.lease",
        serde_json::json!({
            "target_id": "local",
            "port_name": &request.port_name,
            "protocol": "tcp",
        }),
    )
    .await
    .and_then(|value| value_field(value, "lease", "host.port.lease"))
    {
        Ok(lease) => lease,
        Err(error) => return Err(anyhow::anyhow!("deployment port lease failed: {error}")),
    };
    let lease_id = required_string(&lease, "id", "port lease")?;
    let lease_port = required_u16(&lease, "port", "port lease")?;
    if let Err(error) = check_job_cancel(state, job_id) {
        rollback_deploy(
            state,
            &cleanup_context,
            &request.route_id,
            false,
            container_id.as_deref(),
            Some(&lease_id),
        )
        .await;
        return Err(error);
    }

    if let Err(error) = deployment_operation_effect_context(
        state,
        job_id,
        authority,
        &request.project_id,
        "host_build_deploy_candidate_start",
    )
    .await
    {
        rollback_deploy(
            state,
            &cleanup_context,
            &request.route_id,
            false,
            None,
            Some(&lease_id),
        )
        .await;
        return Err(error);
    }
    let started = match start_build_deploy_container(HostDockerStartRequest {
        image,
        container_port: request.container_port,
        host_port: lease_port,
        route_id: &request.route_id,
        port_lease_id: &lease_id,
        project_id: request.project_id.as_str(),
        build_id,
        source_commit,
        operation_id: &operation_id,
        env,
        mounts,
    })
    .await
    {
        Ok(started) => started,
        Err(error) => {
            cleanup_candidate_after_unknown_start(
                state,
                &request.route_id,
                &lease_id,
                "host_build_deploy_unknown_start",
            )
            .await;
            return Err(anyhow::anyhow!(
                "host docker container start failed: {error}"
            ));
        }
    };
    let parsed_container_id = started.container_id.clone();
    container_id = Some(parsed_container_id.clone());
    let container_name = started.container_name.clone();
    if let Err(error) = check_job_cancel(state, job_id) {
        rollback_deploy(
            state,
            &cleanup_context,
            &request.route_id,
            false,
            container_id.as_deref(),
            Some(&lease_id),
        )
        .await;
        return Err(error);
    }

    job_transition(
        state,
        job_id,
        BuildDeployJobState::Probing,
        "probing candidate readiness",
    )
    .await;
    if let Err(error) =
        wait_for_deployment_readiness(lease_port, request.health_path.as_deref()).await
    {
        rollback_deploy(
            state,
            &cleanup_context,
            &request.route_id,
            false,
            container_id.as_deref(),
            Some(&lease_id),
        )
        .await;
        return Err(anyhow::anyhow!(
            "deployment did not become ready in time: {error}"
        ));
    }
    if let Err(error) = check_job_cancel(state, job_id) {
        rollback_deploy(
            state,
            &cleanup_context,
            &request.route_id,
            false,
            container_id.as_deref(),
            Some(&lease_id),
        )
        .await;
        return Err(error);
    }

    job_transition(
        state,
        job_id,
        BuildDeployJobState::RegisteringProxy,
        "activating candidate route",
    )
    .await;
    context = match deployment_operation_effect_context(
        state,
        job_id,
        authority,
        &request.project_id,
        "host_build_deploy_route_activation",
    )
    .await
    {
        Ok(context) => context,
        Err(error) => {
            rollback_deploy(
                state,
                &cleanup_context,
                &request.route_id,
                false,
                container_id.as_deref(),
                Some(&lease_id),
            )
            .await;
            return Err(error);
        }
    };
    let route = match call_host_protocol(
        state,
        &context,
        "host.proxy.register",
        serde_json::json!({
            "route_id": &request.route_id,
            "protocol": "http",
            "access": request.route_access,
            "upstream": {
                "port_lease_id": &lease_id,
                "port_name": &request.port_name,
            },
        }),
    )
    .await
    .and_then(|value| value_field(value, "route", "host.proxy.register"))
    {
        Ok(route) => route,
        Err(error) => {
            rollback_deploy(
                state,
                &cleanup_context,
                &request.route_id,
                false,
                container_id.as_deref(),
                Some(&lease_id),
            )
            .await;
            return Err(anyhow::anyhow!("proxy registration failed: {error}"));
        }
    };
    let (route_id, fallback_public_url) = match (
        required_string(&route, "id", "proxy route"),
        required_string(&route, "public_url", "proxy route"),
    ) {
        (Ok(route_id), Ok(public_url)) => (route_id, public_url),
        (Err(error), _) | (_, Err(error)) => {
            rollback_deploy(
                state,
                &cleanup_context,
                &request.route_id,
                true,
                Some(&parsed_container_id),
                Some(&lease_id),
            )
            .await;
            return Err(error);
        }
    };
    let public_url = service_public_url_for_route(
        state,
        &request.route_id,
        &fallback_public_url,
        request.route_access,
    );
    if let Err(error) = check_job_cancel(state, job_id) {
        rollback_deploy(
            state,
            &cleanup_context,
            &route_id,
            true,
            container_id.as_deref(),
            Some(&lease_id),
        )
        .await;
        return Err(error);
    }

    if state
        .runtime
        .config()
        .proxy_route_registry
        .set_ready_if_active_with_lease(&route_id, &lease_id, true)
        .await
        .is_none()
    {
        rollback_deploy(
            state,
            &cleanup_context,
            &route_id,
            true,
            container_id.as_deref(),
            Some(&lease_id),
        )
        .await;
        return Err(anyhow::anyhow!(
            "proxy route disappeared before readiness promotion"
        ));
    }

    Ok(DeployBuiltImageResponse {
        route_id,
        public_url,
        route_access: request.route_access,
        port_lease_id: lease_id,
        container_id: parsed_container_id,
        container_name,
    })
}

async fn restore_previous_revision_route<S>(
    state: &AppState<S>,
    previous: &DeploymentRevision,
    candidate_lease_id: &str,
    transport: &str,
) -> anyhow::Result<bool>
where
    S: EventStore,
{
    restore_proxy_route_if_candidate_active(
        state,
        &previous.route_id,
        candidate_lease_id,
        &previous.receipt.port_lease_id,
        &previous.port_name,
        previous.route_access,
        true,
        transport,
    )
    .await
}

async fn restore_proxy_route_if_candidate_active<S>(
    state: &AppState<S>,
    route_id: &str,
    candidate_lease_id: &str,
    previous_lease_id: &str,
    previous_port_name: &str,
    previous_access: ProxyRouteAccess,
    previous_ready: bool,
    transport: &str,
) -> anyhow::Result<bool>
where
    S: EventStore,
{
    let current = state
        .runtime
        .config()
        .proxy_route_registry
        .status(route_id)
        .await;
    if !current.is_some_and(|route| {
        route.status == ProxyRouteStatusKind::Active
            && route.upstream.port_lease_id == candidate_lease_id
    }) {
        return Ok(false);
    }
    let context = ProtocolContext::host_dev(transport);
    call_host_protocol(
        state,
        &context,
        "host.proxy.register",
        serde_json::json!({
            "route_id": route_id,
            "protocol": "http",
            "access": previous_access,
            "upstream": {
                "port_lease_id": previous_lease_id,
                "port_name": previous_port_name,
            },
        }),
    )
    .await?;
    anyhow::ensure!(
        state
            .runtime
            .config()
            .proxy_route_registry
            .set_ready_if_active_with_lease(route_id, previous_lease_id, previous_ready,)
            .await
            .is_some(),
        "previous proxy route disappeared during restoration"
    );
    Ok(true)
}

async fn compensate_candidate_after_activation_failure<S>(
    state: &AppState<S>,
    candidate: &HostBuildDeployResponse,
    previous: Option<&DeploymentRevision>,
    transport: &str,
) where
    S: EventStore,
{
    let context = ProtocolContext::host_dev(transport);
    let mut unregister_candidate = true;
    if let Some(previous) = previous.filter(|previous| previous.route_id == candidate.route_id) {
        match restore_previous_revision_route(state, previous, &candidate.port_lease_id, transport)
            .await
        {
            Ok(true) => unregister_candidate = false,
            Ok(false) => {
                // Another activation owns the route now; never unregister its route.
                unregister_candidate = false;
            }
            Err(error) => {
                eprintln!("warning: failed to restore previous deployment route: {error}");
            }
        }
    }
    rollback_deploy(
        state,
        &context,
        &candidate.route_id,
        unregister_candidate,
        Some(&candidate.container_id),
        Some(&candidate.port_lease_id),
    )
    .await;
}

async fn drain_previous_revision<S>(
    state: &AppState<S>,
    previous: &DeploymentRevision,
    active_route_id: &str,
) -> Vec<String>
where
    S: EventStore,
{
    if previous.target_deployment.is_some()
        || previous.source_kind == DeploymentSourceKind::VerifiedArtifact
    {
        return development::drain_target_revision(state, previous, active_route_id).await;
    }
    let context = ProtocolContext::host_dev("host_deployment_previous_revision_drain");
    cleanup_deployment_resources(
        state,
        &context,
        &previous.route_id,
        previous.route_id != active_route_id,
        Some(&previous.receipt.container_id),
        Some(&previous.receipt.port_lease_id),
    )
    .await
}

async fn invoke_docker_runtime_lab<S>(
    state: &AppState<S>,
    context: &ProtocolContext,
    capability_id: &str,
    input: Value,
) -> anyhow::Result<Value>
where
    S: EventStore,
{
    let value = call_host_protocol(
        state,
        context,
        "capability.invoke",
        serde_json::json!({
            "capability_id": capability_id,
            "provider_package_id": "plurora/docker-runtime-lab",
            "input": input,
        }),
    )
    .await?;
    value_field(value, "output", "capability.invoke")
}

pub async fn clone_project_workspace_from_git<S>(
    state: &AppState<S>,
    request: ProjectWorkspaceCloneRequest,
) -> anyhow::Result<ProjectWorkspaceCloneResult>
where
    S: EventStore,
{
    let context = ProtocolContext::host_dev("project_workspace_clone");
    clone_project_workspace_from_git_with_context(state, request, &context).await
}

async fn clone_project_workspace_from_git_with_context<S>(
    state: &AppState<S>,
    request: ProjectWorkspaceCloneRequest,
    context: &ProtocolContext,
) -> anyhow::Result<ProjectWorkspaceCloneResult>
where
    S: EventStore,
{
    validate_workspace_clone_url(&request.source_url)?;
    validate_workspace_clone_ref(&request.ref_name)?;
    let owned_project_dir = canonical_workspace_project_root(&request.project_id, None)?;
    let invocation = build_project_workspace_clone_invocation(&request, None)?;

    let resolved = invoke_git_tools_lab(
        state,
        context,
        "plurora/git-tools-lab/resolve_ref",
        invocation.resolve_ref_params,
    )
    .await?;
    let commit_sha = required_string(&resolved, "commit_sha", "git resolve_ref")?;
    let resolved_ref_name = required_string(&resolved, "ref_name", "git resolve_ref")?;
    validate_git_commit_sha(&commit_sha)?;
    validate_workspace_clone_ref(&resolved_ref_name)?;

    remove_owned_workspace_child_if_exists(
        &owned_project_dir,
        &invocation.staging_dir,
        "workspace staging directory",
    )?;
    let mut fetch_params = invocation.fetch_tree_params;
    fetch_params["commit_sha"] = Value::String(commit_sha.clone());
    fetch_params["ref_name"] = Value::String(resolved_ref_name.clone());
    let fetch_result = async {
        let output = invoke_git_tools_lab(
            state,
            context,
            "plurora/git-tools-lab/fetch_tree",
            fetch_params,
        )
        .await?;
        validate_workspace_staging_containment(&invocation.workspace_dir, &invocation.staging_dir)?;
        replace_workspace_from_staging(
            &owned_project_dir,
            &invocation.workspace_dir,
            &invocation.staging_dir,
        )?;
        anyhow::Ok(output)
    }
    .await;

    if fetch_result.is_err() {
        remove_owned_workspace_child_if_exists(
            &owned_project_dir,
            &invocation.staging_dir,
            "workspace staging directory",
        )
        .ok();
    }
    let output = fetch_result?;

    Ok(ProjectWorkspaceCloneResult {
        project_id: request.project_id,
        ref_name: resolved_ref_name,
        commit_sha,
        tree_hash: output
            .get("tree_hash")
            .and_then(Value::as_str)
            .map(str::to_string),
        files_written: output.get("files_written").and_then(Value::as_u64),
        total_bytes: output.get("total_bytes").and_then(Value::as_u64),
    })
}

async fn invoke_git_tools_lab<S>(
    state: &AppState<S>,
    context: &ProtocolContext,
    capability_id: &str,
    input: Value,
) -> anyhow::Result<Value>
where
    S: EventStore,
{
    let value = call_host_protocol(
        state,
        context,
        "capability.invoke",
        serde_json::json!({
            "capability_id": capability_id,
            "provider_package_id": "plurora/git-tools-lab",
            "input": input,
        }),
    )
    .await?;
    value_field(value, "output", "capability.invoke")
}

fn build_project_workspace_clone_invocation(
    request: &ProjectWorkspaceCloneRequest,
    data_dir_override: Option<&FsPath>,
) -> anyhow::Result<GitFetchTreeInvocation> {
    validate_workspace_clone_url(&request.source_url)?;
    validate_workspace_clone_ref(&request.ref_name)?;
    let workspace_dir = match data_dir_override {
        Some(data_dir) => {
            plurora_core::paths::project_workspace_dir_in(data_dir, &request.project_id)
        }
        None => plurora_core::paths::project_workspace_dir(&request.project_id)?,
    };
    validate_workspace_destination(&request.project_id, data_dir_override, &workspace_dir)?;
    let staging_dir = workspace_dir.with_file_name("workspace.staging");
    validate_workspace_destination(&request.project_id, data_dir_override, &staging_dir)?;
    Ok(GitFetchTreeInvocation {
        resolve_ref_params: serde_json::json!({
            "remote_url": request.source_url,
            "ref": request.ref_name,
        }),
        fetch_tree_params: serde_json::json!({
            "remote_url": request.source_url,
            "ref_name": request.ref_name,
            "dest_dir": staging_dir.to_string_lossy(),
            "max_files": DEPLOYMENT_WORKSPACE_MAX_FILES,
            "max_directories": DEPLOYMENT_WORKSPACE_MAX_DIRECTORIES,
            "max_total_bytes": DEPLOYMENT_WORKSPACE_MAX_BYTES,
            "max_download_bytes": DEPLOYMENT_GIT_DOWNLOAD_MAX_BYTES,
        }),
        workspace_dir,
        staging_dir,
    })
}

fn validate_workspace_clone_url(source_url: &str) -> anyhow::Result<()> {
    let parsed = url::Url::parse(source_url).context("source_url must be an absolute HTTPS URL")?;
    if parsed.scheme() != "https" {
        anyhow::bail!("source_url must use https");
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        anyhow::bail!("source_url must not contain userinfo");
    }
    if parsed.host_str().is_none() {
        anyhow::bail!("source_url must include a host");
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        anyhow::bail!("source_url must not contain query or fragment");
    }
    Ok(())
}

fn validate_workspace_clone_ref(ref_name: &str) -> anyhow::Result<()> {
    let trimmed = ref_name.trim();
    if trimmed.is_empty() || trimmed.len() > 256 {
        anyhow::bail!("git ref must be non-empty and at most 256 bytes");
    }
    if trimmed != ref_name
        || trimmed.starts_with('-')
        || trimmed.contains("..")
        || trimmed.contains("//")
        || trimmed.contains('\\')
        || trimmed.bytes().any(|byte| byte.is_ascii_control())
    {
        anyhow::bail!("git ref contains unsupported characters");
    }
    Ok(())
}

fn validate_git_commit_sha(commit_sha: &str) -> anyhow::Result<()> {
    if commit_sha.len() == 40 && commit_sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        anyhow::bail!("resolved commit_sha must be a 40-character hex SHA")
    }
}

fn validate_workspace_destination(
    project_id: &ProjectId,
    data_dir_override: Option<&FsPath>,
    candidate: &FsPath,
) -> anyhow::Result<()> {
    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        anyhow::bail!("workspace path must not contain parent components");
    }
    let project_dir = match data_dir_override {
        Some(data_dir) => data_dir.join("projects").join(project_id.as_str()),
        None => plurora_core::paths::project_dir(project_id)?,
    };
    if !candidate.starts_with(&project_dir) {
        anyhow::bail!("workspace path escaped project directory");
    }
    Ok(())
}

fn canonical_workspace_project_root(
    project_id: &ProjectId,
    data_dir_override: Option<&FsPath>,
) -> anyhow::Result<PathBuf> {
    let data_dir = match data_dir_override {
        Some(data_dir) => data_dir.to_path_buf(),
        None => plurora_core::paths::data_dir()?,
    };
    let data_dir = std::fs::canonicalize(&data_dir).with_context(|| {
        format!(
            "failed to canonicalize data directory {}",
            data_dir.display()
        )
    })?;
    let projects = data_dir.join("projects");
    let project = projects.join(project_id.as_str());
    for (path, label) in [
        (&projects, "projects root"),
        (&project, "deployment project root"),
    ] {
        let metadata = std::fs::symlink_metadata(path)
            .with_context(|| format!("{label} is unavailable: {}", path.display()))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            anyhow::bail!("{label} must be a real directory: {}", path.display());
        }
    }
    let projects = std::fs::canonicalize(&projects)?;
    let project = std::fs::canonicalize(&project)?;
    if projects.parent() != Some(data_dir.as_path())
        || project.parent() != Some(projects.as_path())
        || project.file_name().and_then(|name| name.to_str()) != Some(project_id.as_str())
    {
        anyhow::bail!(
            "deployment project root escaped the canonical data directory: {}",
            project.display()
        );
    }
    Ok(project)
}

fn validate_owned_workspace_child(
    project_dir: &FsPath,
    child: &FsPath,
    label: &str,
) -> anyhow::Result<bool> {
    let child_parent = child
        .parent()
        .ok_or_else(|| anyhow::anyhow!("{label} has no project parent"))?;
    let child_parent = std::fs::canonicalize(child_parent)?;
    if child_parent != project_dir {
        anyhow::bail!("{label} escaped the deployment project root");
    }
    let metadata = match std::fs::symlink_metadata(child) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        anyhow::bail!("{label} must be a real directory: {}", child.display());
    }
    let canonical = std::fs::canonicalize(child)?;
    if canonical.parent() != Some(project_dir) {
        anyhow::bail!("{label} escaped the deployment project root");
    }
    Ok(true)
}

fn remove_owned_workspace_child_if_exists(
    project_dir: &FsPath,
    child: &FsPath,
    label: &str,
) -> anyhow::Result<()> {
    if validate_owned_workspace_child(project_dir, child, label)? {
        std::fs::remove_dir_all(child)
            .with_context(|| format!("failed to remove {label} {}", child.display()))?;
    }
    Ok(())
}

fn validate_workspace_staging_containment(
    workspace_dir: &FsPath,
    staging_dir: &FsPath,
) -> anyhow::Result<()> {
    let staging = std::fs::canonicalize(staging_dir).with_context(|| {
        format!(
            "failed to canonicalize workspace staging dir {}",
            staging_dir.display()
        )
    })?;
    let project_dir = workspace_dir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("workspace dir has no project parent"))?;
    let project_dir = std::fs::canonicalize(project_dir).with_context(|| {
        format!(
            "failed to canonicalize project dir {}",
            project_dir.display()
        )
    })?;
    if !staging.starts_with(&project_dir) {
        anyhow::bail!("workspace staging dir escaped project directory");
    }
    Ok(())
}

fn replace_workspace_from_staging(
    owned_project_dir: &FsPath,
    workspace_dir: &FsPath,
    staging_dir: &FsPath,
) -> anyhow::Result<()> {
    if !validate_owned_workspace_child(
        owned_project_dir,
        staging_dir,
        "workspace staging directory",
    )? {
        anyhow::bail!("workspace staging directory is unavailable");
    }
    let backup_dir = workspace_dir.with_file_name("workspace.previous");
    remove_owned_workspace_child_if_exists(
        owned_project_dir,
        &backup_dir,
        "workspace backup directory",
    )?;
    if validate_owned_workspace_child(owned_project_dir, workspace_dir, "workspace directory")? {
        std::fs::rename(workspace_dir, &backup_dir).with_context(|| {
            format!(
                "failed to move existing workspace {} to backup",
                workspace_dir.display()
            )
        })?;
    }
    let replace_result = std::fs::rename(staging_dir, workspace_dir).with_context(|| {
        format!(
            "failed to atomically install workspace {}",
            workspace_dir.display()
        )
    });
    if replace_result.is_err() {
        let backup_owned = validate_owned_workspace_child(
            owned_project_dir,
            &backup_dir,
            "workspace backup directory",
        )
        .unwrap_or(false);
        let workspace_absent = std::fs::symlink_metadata(workspace_dir)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound);
        if backup_owned && workspace_absent {
            std::fs::rename(&backup_dir, workspace_dir).ok();
        }
        return replace_result;
    }
    if let Err(error) = remove_owned_workspace_child_if_exists(
        owned_project_dir,
        &backup_dir,
        "workspace backup directory",
    ) {
        eprintln!("workspace installed but previous workspace cleanup failed: {error}");
    }
    Ok(())
}

async fn wait_for_deployment_readiness(port: u16, health_path: Option<&str>) -> anyhow::Result<()> {
    let deadline = Instant::now() + DEPLOY_READINESS_TIMEOUT;

    loop {
        if let Err(error) = probe_loopback_port(port, health_path).await {
            let now = Instant::now();
            if now >= deadline {
                return Err(error.context("readiness deadline expired"));
            }
            sleep(std::cmp::min(DEPLOY_READINESS_INTERVAL, deadline - now)).await;
        } else {
            return Ok(());
        }
    }
}

async fn probe_loopback_port(port: u16, health_path: Option<&str>) -> anyhow::Result<()> {
    timeout(
        DEPLOY_READINESS_CONNECT_TIMEOUT,
        TcpStream::connect(("127.0.0.1", port)),
    )
    .await
    .map_err(|_| anyhow::anyhow!("tcp readiness probe timed out"))?
    .map_err(|error| anyhow::anyhow!("tcp readiness probe failed: {error}"))?;

    if let Some(path) = health_path {
        let url = format!("http://127.0.0.1:{port}{path}");
        let response = hardened_proxy_client()
            .get(url)
            .send()
            .await
            .map_err(|error| anyhow::anyhow!("http readiness probe failed: {error}"))?;
        let status = response.status();
        if status.is_success() || status.is_redirection() || status.is_client_error() {
            return Ok(());
        }
        anyhow::bail!("http readiness probe returned {status}");
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct HealthRouteSnapshot {
    route_id: String,
    port_lease_id: String,
    port: u16,
    ready: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct HealthCounters {
    consecutive_failures: u32,
    consecutive_successes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HealthProbeResult {
    Success,
    Failure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HealthTransition {
    ready: bool,
    reason: &'static str,
}

fn decide_health_transition(
    current_ready: bool,
    counters: &mut HealthCounters,
    probe_result: HealthProbeResult,
) -> Option<HealthTransition> {
    match probe_result {
        HealthProbeResult::Failure => {
            counters.consecutive_failures = counters.consecutive_failures.saturating_add(1);
            counters.consecutive_successes = 0;
            if current_ready && counters.consecutive_failures >= HEALTH_FAILURE_THRESHOLD {
                return Some(HealthTransition {
                    ready: false,
                    reason: "tcp_probe_failed",
                });
            }
        }
        HealthProbeResult::Success => {
            counters.consecutive_successes = counters.consecutive_successes.saturating_add(1);
            counters.consecutive_failures = 0;
            if !current_ready && counters.consecutive_successes >= HEALTH_RECOVERY_THRESHOLD {
                return Some(HealthTransition {
                    ready: true,
                    reason: "recovered",
                });
            }
        }
    }
    None
}

async fn run_health_supervisor<S>(state: AppState<S>)
where
    S: EventStore,
{
    let mut counters: HashMap<String, HealthCounters> = HashMap::new();
    let mut health_session_id: Option<SessionId> = None;
    loop {
        sleep(HEALTH_POLL_INTERVAL).await;
        let snapshots = health_route_snapshots(&state).await;
        let active_route_ids: HashSet<String> = snapshots
            .iter()
            .map(|snapshot| snapshot.route_id.clone())
            .collect();
        counters.retain(|route_id, _| active_route_ids.contains(route_id));

        for snapshot in snapshots {
            let probe_result = if probe_health_tcp(snapshot.port).await.is_ok() {
                HealthProbeResult::Success
            } else {
                HealthProbeResult::Failure
            };
            let route_counters = counters.entry(snapshot.route_id.clone()).or_default();
            let previous_ready = snapshot.ready;
            let Some(transition) =
                decide_health_transition(previous_ready, route_counters, probe_result)
            else {
                continue;
            };

            let updated = state
                .runtime
                .config()
                .proxy_route_registry
                .set_ready_if_active_with_lease(
                    &snapshot.route_id,
                    &snapshot.port_lease_id,
                    transition.ready,
                )
                .await;
            if updated.is_none() {
                counters.remove(&snapshot.route_id);
                continue;
            }

            let failure_count = route_counters.consecutive_failures;
            let payload = serde_json::json!({
                "route_id": snapshot.route_id,
                "port_lease_id": snapshot.port_lease_id,
                "previous_ready": previous_ready,
                "ready": transition.ready,
                "reason": transition.reason,
                "failure_count": failure_count,
                "probe": { "kind": "tcp" },
            });
            if let Err(error) = state
                .runtime
                .append_event_with_context(
                    &ProtocolContext::host_dev("health_supervisor"),
                    AppendEventRequest {
                        session_id: deployment_health_session(&state, &mut health_session_id).await,
                        writer_package_id: plurora_core::PLATFORM_RUNTIME_ID.to_string(),
                        kind: plurora_core::EVENT_DEPLOYMENT_HEALTH.to_string(),
                        payload,
                        metadata: serde_json::json!({}),
                    },
                )
                .await
            {
                eprintln!("deployment health audit append failed: {error}");
            }
        }
    }
}

async fn deployment_health_session<S>(
    state: &AppState<S>,
    cached: &mut Option<SessionId>,
) -> SessionId
where
    S: EventStore,
{
    if let Some(session_id) = cached.as_ref() {
        return session_id.clone();
    }
    match state
        .runtime
        .open_session(OpenSessionRequest {
            labels: vec!["host:deployment-health".to_string()],
            metadata: serde_json::json!({"kind":"deployment_health"}),
            ..OpenSessionRequest::default()
        })
        .await
    {
        Ok(session) => {
            *cached = Some(session.id.clone());
            session.id
        }
        Err(_) => "host_deployment_health".to_string(),
    }
}

async fn health_route_snapshots<S>(state: &AppState<S>) -> Vec<HealthRouteSnapshot>
where
    S: EventStore,
{
    let routes = state.runtime.config().proxy_route_registry.list().await;
    let mut snapshots = Vec::new();
    for route in routes {
        if route.status != ProxyRouteStatusKind::Active {
            continue;
        }
        let port_lease_id = route.upstream.port_lease_id.clone();
        let Some(lease) = state
            .runtime
            .config()
            .port_lease_registry
            .status(&port_lease_id)
            .await
        else {
            continue;
        };
        if lease.status != PortLeaseStatusKind::Active {
            continue;
        }
        snapshots.push(HealthRouteSnapshot {
            route_id: route.id,
            port_lease_id,
            port: lease.port,
            ready: route.ready,
        });
    }
    snapshots
}

async fn probe_health_tcp(port: u16) -> anyhow::Result<()> {
    timeout(
        HEALTH_PROBE_TIMEOUT,
        TcpStream::connect(("127.0.0.1", port)),
    )
    .await
    .map_err(|_| anyhow::anyhow!("tcp health probe timed out"))?
    .map_err(|error| anyhow::anyhow!("tcp health probe failed: {error}"))?;
    Ok(())
}

async fn rollback_deploy<S>(
    state: &AppState<S>,
    context: &ProtocolContext,
    route_id: &str,
    proxy_registered: bool,
    container_id: Option<&str>,
    lease_id: Option<&str>,
) where
    S: EventStore,
{
    for warning in cleanup_deployment_resources(
        state,
        context,
        route_id,
        proxy_registered,
        container_id,
        lease_id,
    )
    .await
    {
        eprintln!("warning: deployment compensation incomplete: {warning}");
    }
}

async fn cleanup_candidate_after_unknown_start<S>(
    state: &AppState<S>,
    route_id: &str,
    lease_id: &str,
    transport: &str,
) where
    S: EventStore,
{
    let context = ProtocolContext::host_dev(transport);
    let managed = invoke_docker_runtime_lab(
        state,
        &context,
        "plurora/docker-runtime-lab/list_managed",
        serde_json::json!({}),
    )
    .await;
    let container = match managed {
        Ok(output) => match find_managed_container_for_route(&output, route_id, Some(lease_id)) {
            Ok(container) => container,
            Err(error) => {
                eprintln!(
                    "warning: candidate start outcome is unknown; resources were preserved for reconcile: {error}"
                );
                return;
            }
        },
        Err(error) => {
            eprintln!(
                "warning: candidate start outcome is unknown; resources were preserved for reconcile: {error}"
            );
            return;
        }
    };
    rollback_deploy(
        state,
        &context,
        route_id,
        false,
        container
            .as_ref()
            .map(|container| container.container.as_str()),
        Some(lease_id),
    )
    .await;
}

async fn cleanup_deployment_resources<S>(
    state: &AppState<S>,
    context: &ProtocolContext,
    route_id: &str,
    proxy_registered: bool,
    container_id: Option<&str>,
    lease_id: Option<&str>,
) -> Vec<String>
where
    S: EventStore,
{
    let mut warnings = Vec::new();
    if proxy_registered {
        if let Err(error) = call_host_protocol(
            state,
            context,
            "host.proxy.unregister",
            serde_json::json!({ "route_id": route_id }),
        )
        .await
        {
            warnings.push(format!("proxy cleanup failed: {error}"));
        }
    }
    let mut safe_to_release_lease = container_id.is_none();
    if let (Some(container_id), Some(port_lease_id)) = (container_id, lease_id) {
        match invoke_docker_runtime_lab(
            state,
            context,
            "plurora/docker-runtime-lab/stop_container",
            serde_json::json!({
                "approved": true,
                "container_id": container_id,
                "route_id": route_id,
                "port_lease_id": port_lease_id,
                "timeout_secs": 5
            }),
        )
        .await
        {
            Ok(output)
                if output
                    .get("docker_performed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    && output
                        .get("removed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false) =>
            {
                safe_to_release_lease = true;
            }
            Ok(output) => warnings.push(
                output
                    .get("reason")
                    .or_else(|| output.get("error").and_then(|error| error.get("message")))
                    .and_then(Value::as_str)
                    .unwrap_or("container cleanup was not confirmed")
                    .to_string(),
            ),
            Err(error) => warnings.push(format!("container cleanup failed: {error}")),
        }
    }
    if let Some(lease_id) = lease_id.filter(|_| safe_to_release_lease) {
        if let Err(error) = call_host_protocol(
            state,
            context,
            "host.port.release",
            serde_json::json!({ "lease_id": lease_id }),
        )
        .await
        {
            warnings.push(format!("port lease cleanup failed: {error}"));
        }
    }
    warnings
}

fn validate_host_deploy_request(request: &HostDeployRequest) -> anyhow::Result<()> {
    let image = request.image.trim();
    if !is_safe_docker_image(image) {
        anyhow::bail!("image must be a safe Docker image reference");
    }
    if request.container_port == 0 {
        anyhow::bail!("container_port must be an integer in 1..=65535");
    }
    if !is_safe_route_token(&request.port_name) {
        anyhow::bail!("port_name must be label-safe");
    }
    if !is_safe_route_token(&request.route_id) {
        anyhow::bail!("route_id must be label-safe");
    }
    if let Some(health_path) = request.health_path.as_deref() {
        if !health_path.starts_with('/') || health_path.len() > 256 {
            anyhow::bail!("health_path must start with / and be at most 256 bytes");
        }
    }
    Ok(())
}

fn validate_host_build_deploy_request(request: &HostBuildDeployRequest) -> anyhow::Result<()> {
    if !request.approved {
        anyhow::bail!("build-deploy requires approved: true");
    }
    validate_workspace_clone_url(&request.source_url)?;
    validate_workspace_clone_ref(&request.ref_name)?;
    if let Some(source_commit) = request.source_commit.as_deref() {
        if !is_full_git_sha(source_commit) {
            anyhow::bail!("source_commit must be a 40-character hex SHA");
        }
    }
    if let Some(build_id) = request.build_id.as_deref() {
        validate_build_id(build_id)?;
    }
    if let Some(idempotency_key) = request.idempotency_key.as_deref() {
        if idempotency_key.is_empty()
            || idempotency_key.len() > 128
            || !idempotency_key.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
            })
        {
            anyhow::bail!("idempotency_key must be 1-128 label-safe characters");
        }
    }
    let strategy = request.strategy.as_deref().unwrap_or("dockerfile");
    if !matches!(strategy, "dockerfile" | "nixpacks") {
        anyhow::bail!("strategy must be dockerfile or nixpacks");
    }
    if let Some(dockerfile) = request.dockerfile.as_deref() {
        validate_relative_dockerfile(dockerfile)?;
    }
    validate_runtime_env_specs(&request.runtime_env)?;
    validate_runtime_mount_specs(&request.runtime_mounts)?;
    validate_host_deploy_request(&HostDeployRequest {
        project_id: Some(request.project_id.clone()),
        image: "plurora/placeholder:build".to_string(),
        container_port: request.container_port,
        port_name: request.port_name.clone(),
        route_id: request.route_id.clone(),
        route_access: request.route_access,
        health_path: request.health_path.clone(),
        pull_if_missing: false,
    })?;
    Ok(())
}

fn validate_runtime_env_specs(specs: &[RuntimeEnvSpec]) -> anyhow::Result<()> {
    if specs.len() > MAX_RUNTIME_ENV_ENTRIES {
        anyhow::bail!("runtime_env may contain at most {MAX_RUNTIME_ENV_ENTRIES} entries");
    }
    let mut seen = HashSet::new();
    let mut total = 0usize;
    for spec in specs {
        validate_env_name(&spec.name)?;
        if !seen.insert(spec.name.as_str()) {
            anyhow::bail!("duplicate runtime_env name");
        }
        let has_value = spec.value.is_some();
        let has_secret_ref = spec.secret_ref.is_some();
        if has_value == has_secret_ref {
            anyhow::bail!("runtime_env entry must contain exactly one of value or secret_ref");
        }
        if let Some(value) = spec.value.as_deref() {
            if value.contains('\0') {
                anyhow::bail!("runtime_env values must not contain NUL bytes");
            }
            if value.len() > MAX_RUNTIME_ENV_VALUE_LEN {
                anyhow::bail!("runtime_env value is too large");
            }
            total = total
                .saturating_add(spec.name.len())
                .saturating_add(value.len());
        }
        if let Some(secret_ref) = spec.secret_ref.as_deref() {
            if secret_ref.is_empty()
                || secret_ref.contains('\0')
                || !plurora_core::SecretRef::is_valid_ref(secret_ref)
            {
                anyhow::bail!("runtime_env secret_ref is invalid");
            }
            total = total
                .saturating_add(spec.name.len())
                .saturating_add(secret_ref.len());
        }
        if total > MAX_RUNTIME_ENV_TOTAL_BYTES {
            anyhow::bail!("runtime_env total size is too large");
        }
    }
    Ok(())
}

fn validate_env_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() || name.len() > 128 || name.contains('\0') {
        anyhow::bail!("runtime_env name is invalid");
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        anyhow::bail!("runtime_env name is invalid");
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        anyhow::bail!("runtime_env name is invalid");
    }
    Ok(())
}

async fn resolve_runtime_env<S>(
    state: &AppState<S>,
    request: &HostBuildDeployRequest,
) -> anyhow::Result<Vec<ResolvedRuntimeEnv>>
where
    S: EventStore,
{
    let mut resolved = Vec::with_capacity(request.runtime_env.len());
    for spec in &request.runtime_env {
        if let Some(value) = spec.value.as_deref() {
            resolved.push(ResolvedRuntimeEnv {
                name: spec.name.clone(),
                value: value.to_string(),
                source: RuntimeEnvSourceKind::Plain,
            });
        } else if let Some(secret_ref) = spec.secret_ref.as_deref() {
            let value = state
                .runtime
                .resolve_secret_ref_for_project(secret_ref, &request.project_id)
                .await
                .map_err(|_| anyhow::anyhow!("runtime_env secret_ref could not be resolved"))?;
            if value.contains('\0') || value.len() > MAX_RUNTIME_ENV_VALUE_LEN {
                anyhow::bail!("runtime_env resolved secret value is invalid");
            }
            resolved.push(ResolvedRuntimeEnv {
                name: spec.name.clone(),
                value,
                source: RuntimeEnvSourceKind::SecretRef,
            });
        }
    }
    Ok(resolved)
}

fn validate_runtime_mount_specs(specs: &[RuntimeMountSpec]) -> anyhow::Result<()> {
    if specs.len() > MAX_RUNTIME_MOUNTS {
        anyhow::bail!("runtime_mounts may contain at most {MAX_RUNTIME_MOUNTS} entries");
    }
    let mut targets = HashSet::new();
    let mut pairs = HashSet::new();
    for spec in specs {
        if !spec.approved {
            anyhow::bail!("runtime mount requires approved: true");
        }
        if spec.mode == RuntimeMountMode::Rw && !spec.high_risk_approved {
            anyhow::bail!("rw runtime mount requires high_risk_approved: true");
        }
        if spec.reason.trim().is_empty() || spec.reason.len() > 512 || spec.reason.contains('\0') {
            anyhow::bail!("runtime mount reason is required and must be at most 512 bytes");
        }
        validate_container_mount_path(&spec.container_path)?;
        let canonical = canonical_runtime_mount_source(&spec.source_host_path)?;
        reject_dangerous_host_mount_source(&canonical)?;
        if !targets.insert(spec.container_path.as_str()) {
            anyhow::bail!("duplicate runtime mount container_path");
        }
        let pair = (canonical, spec.container_path.as_str());
        if !pairs.insert(pair) {
            anyhow::bail!("duplicate runtime mount source/container pair");
        }
    }
    Ok(())
}

fn canonical_runtime_mount_source(source: &str) -> anyhow::Result<PathBuf> {
    if source.is_empty() || source.contains('\0') {
        anyhow::bail!("runtime mount source_host_path is invalid");
    }
    let path = FsPath::new(source);
    if !path.is_absolute() {
        anyhow::bail!("runtime mount source_host_path must be absolute");
    }
    path.canonicalize()
        .with_context(|| "runtime mount source_host_path must exist")
}

fn validate_container_mount_path(path: &str) -> anyhow::Result<()> {
    if path.is_empty()
        || path.contains('\0')
        || path.contains(':')
        || path.contains('\\')
        || !path.starts_with('/')
        || path
            .split('/')
            .skip(1)
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        anyhow::bail!("runtime mount container_path is invalid");
    }
    const DENIED_TARGETS: &[&str] = &[
        "/", "/proc", "/sys", "/dev", "/run", "/var/run", "/etc", "/root", "/home", "/tmp",
    ];
    if DENIED_TARGETS
        .iter()
        .any(|denied| path == *denied || path.starts_with(&format!("{denied}/")))
    {
        anyhow::bail!("runtime mount container_path targets a denied container path");
    }
    Ok(())
}

fn reject_dangerous_host_mount_source(path: &FsPath) -> anyhow::Result<()> {
    let s = path.to_string_lossy();
    let home = std::env::var("HOME").ok().map(PathBuf::from);
    let data_dir = plurora_core::paths::data_dir()?;
    let mut denied = vec![
        PathBuf::from("/"),
        PathBuf::from("/etc"),
        PathBuf::from("/root"),
        PathBuf::from("/proc"),
        PathBuf::from("/sys"),
        PathBuf::from("/dev"),
        PathBuf::from("/run"),
        PathBuf::from("/var/run"),
        PathBuf::from("/var/lib/docker"),
        PathBuf::from("/var/lib/containerd"),
        PathBuf::from("/var/lib/kubelet"),
        PathBuf::from("/var/run/docker.sock"),
        PathBuf::from("/run/docker.sock"),
        PathBuf::from("/run/containerd/containerd.sock"),
        PathBuf::from("/run/podman/podman.sock"),
        data_dir.join("keys"),
        data_dir.join("secrets.dat"),
        data_dir.join("events.db"),
    ];
    if let Some(home) = &home {
        denied.extend([
            home.join(".ssh"),
            home.join(".gnupg"),
            home.join(".aws"),
            home.join(".azure"),
            home.join(".config/gcloud"),
            home.join(".kube"),
            home.join(".docker/config.json"),
        ]);
    }
    for denied_path in denied {
        if path == denied_path
            || (denied_path != FsPath::new("/") && path.starts_with(&denied_path))
            || (path != FsPath::new("/") && denied_path.starts_with(path))
        {
            anyhow::bail!("runtime mount source_host_path is denied");
        }
    }
    if s == "/home"
        || s == "/Users"
        || s.starts_with("/home/") && path.components().count() <= 3
        || s.starts_with("/Users/") && path.components().count() <= 3
    {
        anyhow::bail!("runtime mount source_host_path is too broad");
    }
    Ok(())
}

fn resolve_runtime_mounts(
    request: &HostBuildDeployRequest,
) -> anyhow::Result<Vec<ResolvedRuntimeMount>> {
    let mut resolved = Vec::with_capacity(request.runtime_mounts.len());
    for spec in &request.runtime_mounts {
        let source = canonical_runtime_mount_source(&spec.source_host_path)?;
        reject_dangerous_host_mount_source(&source)?;
        let metadata = fs::metadata(&source)?;
        let source_kind = if metadata.is_dir() {
            "directory"
        } else if metadata.is_file() {
            "file"
        } else {
            anyhow::bail!("runtime mount source_host_path must be a file or directory")
        }
        .to_string();
        let source_basename = source
            .file_name()
            .map(|name| name.to_string_lossy().to_string());
        let source_hash = short_path_hash(&source);
        resolved.push(ResolvedRuntimeMount {
            source,
            container_path: spec.container_path.clone(),
            mode: spec.mode,
            source_kind,
            source_basename,
            source_hash,
        });
    }
    Ok(resolved)
}

fn short_path_hash(path: &FsPath) -> String {
    let digest = Sha256::digest(path.to_string_lossy().as_bytes());
    digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

#[cfg(test)]
fn should_remove_plurora_build_image(
    labels: &HashMap<String, String>,
    project_id: &str,
    build_id: &str,
) -> bool {
    labels.get("managed-by").is_some_and(|v| v == "plurora")
        && labels
            .get("plurora.project_id")
            .is_some_and(|v| v == project_id)
        && labels
            .get("plurora.build_id")
            .is_some_and(|v| v == build_id)
}

fn validate_build_id(build_id: &str) -> anyhow::Result<()> {
    if build_id.len() < 3
        || build_id.len() > 128
        || build_id.starts_with('-')
        || build_id.contains("..")
        || !build_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        anyhow::bail!("build_id must be label-safe");
    }
    Ok(())
}

fn validate_relative_dockerfile(dockerfile: &str) -> anyhow::Result<()> {
    let path = FsPath::new(dockerfile);
    if dockerfile.trim().is_empty()
        || dockerfile.len() > 255
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        anyhow::bail!("dockerfile must be a relative path without parent components");
    }
    Ok(())
}

fn is_full_git_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sanitize_container_name(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('-');
        }
        if out.len() >= 64 {
            break;
        }
    }
    if out.is_empty() {
        "container".to_string()
    } else {
        out
    }
}

fn generated_build_id(source_commit: &str) -> String {
    let prefix: String = source_commit.chars().take(12).collect();
    format!("build-{prefix}")
}

fn build_deploy_request_fingerprint(request: &HostBuildDeployRequest) -> String {
    let canonical = serde_json::json!({
        "version": 1,
        "project_id": request.project_id.as_str(),
        "source_url": request.source_url,
        "ref_name": request.ref_name,
        "strategy": request.strategy,
        "dockerfile": request.dockerfile,
        "container_port": request.container_port,
        "port_name": request.port_name,
        "route_id": request.route_id,
        "health_path": request.health_path,
        "approved": request.approved,
        "source_commit": request.source_commit,
        "build_id": request.build_id,
        "runtime_env": request.runtime_env.iter().map(|env| serde_json::json!({
            "name": env.name,
            "value_hash": env.value.as_deref().map(privacy_preserving_sha256),
            "secret_ref": env.secret_ref,
        })).collect::<Vec<_>>(),
        "runtime_mounts": request.runtime_mounts.iter().map(|mount| serde_json::json!({
            "source_hash": privacy_preserving_sha256(&mount.source_host_path),
            "container_path": mount.container_path,
            "mode": mount.mode,
            "approved": mount.approved,
            "high_risk_approved": mount.high_risk_approved,
            "reason_hash": privacy_preserving_sha256(&mount.reason),
        })).collect::<Vec<_>>(),
    });
    let bytes = serde_json::to_vec(&canonical).expect("build-deploy request serializes");
    privacy_preserving_sha256(&String::from_utf8_lossy(&bytes))
}

fn privacy_preserving_sha256(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut out = String::from("sha256:");
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn build_deploy_descriptor_hash(
    request: &HostBuildDeployRequest,
    build_id: &str,
    source_commit: &str,
) -> String {
    let canonical = serde_json::json!({
        "version": 1,
        "strategy": request.strategy.as_deref().unwrap_or("dockerfile"),
        "project_id": request.project_id.as_str(),
        "source_url": request.source_url,
        "ref_name": request.ref_name,
        "dockerfile": request.dockerfile.as_deref().unwrap_or("Dockerfile"),
        "container_port": request.container_port,
        "port_name": request.port_name,
        "route_id": request.route_id,
        "health_path": request.health_path,
        "build_id": build_id,
        "source_commit": source_commit,
        "runtime_env": request.runtime_env.iter().map(|env| serde_json::json!({
            "name": env.name,
            "source": if env.secret_ref.is_some() { "secret_ref" } else { "plain" },
        })).collect::<Vec<_>>(),
        "runtime_mounts": request.runtime_mounts.iter().map(|mount| serde_json::json!({
            "container_path": mount.container_path,
            "mode": mount.mode,
        })).collect::<Vec<_>>(),
    });
    let bytes = serde_json::to_vec(&canonical).expect("build descriptor serializes");
    let digest = Sha256::digest(bytes);
    let mut out = String::from("sha256:");
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn require_built_image(output: &Value) -> anyhow::Result<String> {
    if output
        .get("docker_performed")
        .and_then(Value::as_bool)
        .is_some_and(|performed| !performed)
        || output
            .get("image_built")
            .and_then(Value::as_bool)
            .is_some_and(|built| !built)
    {
        anyhow::bail!(
            "docker-runtime-lab did not build image: {}",
            output
                .get("reason")
                .or_else(|| output.get("error").and_then(|error| error.get("message")))
                .and_then(Value::as_str)
                .unwrap_or("unknown reason")
        );
    }
    optional_string(output, "image_id")
        .or_else(|| optional_string(output, "image"))
        .ok_or_else(|| anyhow::anyhow!("docker-runtime-lab build_image missing image_id and image"))
}

fn redacted_failure_message(context: &'static str, _error: &impl fmt::Display) -> String {
    tracing::warn!(
        target: "plurora_service::build_deploy",
        context,
        "operation failed; internal error details suppressed"
    );
    format!("{context} failed; details redacted")
}

fn redacted_build_deploy_error(error: anyhow::Error) -> ServiceError {
    ServiceError::from(anyhow::anyhow!(redacted_failure_message(
        "build-deploy",
        &error
    )))
}

fn redact_build_log(input: &str) -> String {
    let mut redacted = input.replace("secret_ref:", "secret_ref:<redacted>:");
    for marker in [
        "/workspace/",
        "/tmp/",
        "/var/run/docker.sock",
        "/run/docker.sock",
    ] {
        redacted = redacted.replace(marker, "<path>/");
    }
    if redacted.len() > 1024 {
        redacted.truncate(1024);
        redacted.push_str("...");
    }
    redacted
}

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn is_safe_docker_image(image: &str) -> bool {
    !image.is_empty()
        && image.len() <= 255
        && !image.starts_with('-')
        && !image.contains("..")
        && image
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | ':' | '_' | '@' | '-'))
}

fn is_safe_route_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.contains("..")
        && value
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn require_started_container(output: &Value) -> anyhow::Result<String> {
    if output
        .get("docker_performed")
        .and_then(Value::as_bool)
        .is_some_and(|performed| !performed)
        || output
            .get("container_started")
            .and_then(Value::as_bool)
            .is_some_and(|started| !started)
    {
        anyhow::bail!(
            "docker-runtime-lab did not start the container: {}",
            output
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("unknown reason")
        );
    }
    required_string(output, "container_id", "docker-runtime-lab start_container")
}

#[derive(Debug, Clone)]
struct ManagedContainerRef {
    container: String,
    port_lease_id: String,
    running: bool,
}

fn find_managed_container_for_route(
    output: &Value,
    route_id: &str,
    preferred_lease_id: Option<&str>,
) -> anyhow::Result<Option<ManagedContainerRef>> {
    let managed = output
        .get("managed")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            anyhow::anyhow!("docker-runtime-lab list_managed response missing managed")
        })?;
    let mut matches = Vec::new();
    for container in managed {
        if container.get("route_id").and_then(Value::as_str) != Some(route_id) {
            continue;
        }
        let Some(port_lease_id) = optional_string(container, "port_lease_id") else {
            continue;
        };
        if preferred_lease_id.is_some_and(|preferred| preferred != port_lease_id) {
            continue;
        }
        for field in ["container_id", "id", "container_name", "name"] {
            if let Some(value) = optional_string(container, field) {
                matches.push(ManagedContainerRef {
                    container: value,
                    port_lease_id,
                    running: container
                        .get("running")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                });
                break;
            }
        }
    }
    if matches.len() <= 1 {
        return Ok(matches.pop());
    }
    let mut running = matches
        .iter()
        .filter(|container| container.running)
        .cloned()
        .collect::<Vec<_>>();
    if running.len() == 1 {
        return Ok(running.pop());
    }
    anyhow::bail!("multiple managed containers matched the deployment route and lease")
}

fn value_field(value: Value, field: &str, context: &str) -> anyhow::Result<Value> {
    value
        .get(field)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("{context} response missing {field}"))
}

fn required_string(value: &Value, field: &str, context: &str) -> anyhow::Result<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("{context} missing {field}"))
}

fn optional_string(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn required_u16(value: &Value, field: &str, context: &str) -> anyhow::Result<u16> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| anyhow::anyhow!("{context} missing valid {field}"))
}

fn protocol_error_to_anyhow(error: ProtocolError) -> anyhow::Error {
    anyhow::anyhow!("{}: {}", error.code, error.message)
}

fn surface_asset_lease_registry() -> &'static Mutex<HashMap<String, SurfaceAssetLease>> {
    SURFACE_ASSET_LEASES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn surface_asset_root(relative: &str) -> Option<String> {
    let path = relative.split('?').next()?;
    let mut parts = path.split('/').filter(|part| !part.is_empty());
    let prefix = parts.next()?;
    if prefix == "projects" {
        let project_id = parts.next()?;
        plurora_core::project::ProjectId::new(project_id).ok()?;
        Some(format!("projects/{project_id}"))
    } else {
        (!prefix.is_empty()).then(|| prefix.to_string())
    }
}

fn rewrite_surface_asset_url(
    value: &str,
    lease_id: &str,
    expected_root: &str,
) -> anyhow::Result<String> {
    let relative = value
        .strip_prefix("/surface-bundles/")
        .ok_or_else(|| anyhow::anyhow!("surface asset URL is outside the Host bundle namespace"))?;
    anyhow::ensure!(
        surface_asset_root(relative).as_deref() == Some(expected_root),
        "surface asset URL escaped its resolved bundle root"
    );
    Ok(format!("/surface-assets/{lease_id}/{relative}"))
}

fn mint_surface_asset_lease(
    result: &mut Value,
    identity: &HostAccessIdentity,
    host_access: &HostAccessRegistry,
) -> anyhow::Result<()> {
    let bundle_url = result
        .get("bundle_url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("resolved surface bundle is missing bundle_url"))?;
    let relative = bundle_url
        .strip_prefix("/surface-bundles/")
        .ok_or_else(|| anyhow::anyhow!("resolved surface bundle URL is outside the Host"))?;
    let root = surface_asset_root(relative)
        .ok_or_else(|| anyhow::anyhow!("resolved surface bundle root is invalid"))?;
    let lease_id = uuid::Uuid::new_v4().simple().to_string();
    let rewritten_bundle_url = rewrite_surface_asset_url(bundle_url, &lease_id, &root)?;
    let rewritten_stylesheets = result
        .get("stylesheets")
        .and_then(Value::as_array)
        .map(|stylesheets| {
            stylesheets
                .iter()
                .map(|stylesheet| {
                    let value = stylesheet.as_str().ok_or_else(|| {
                        anyhow::anyhow!("resolved stylesheet URL is not a string")
                    })?;
                    rewrite_surface_asset_url(value, &lease_id, &root)
                })
                .collect::<anyhow::Result<Vec<_>>>()
        })
        .transpose()?;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let expires_at_ms = now_ms.saturating_add(SURFACE_ASSET_LEASE_TTL_MS);
    {
        let mut leases = surface_asset_lease_registry()
            .lock()
            .expect("surface asset lease lock poisoned");
        leases.retain(|_, lease| lease.expires_at_ms > now_ms);
        if leases.len() >= SURFACE_ASSET_LEASE_LIMIT {
            if let Some(oldest) = leases
                .iter()
                .min_by_key(|(_, lease)| lease.expires_at_ms)
                .map(|(id, _)| id.clone())
            {
                leases.remove(&oldest);
            }
        }
        leases.insert(
            lease_id.clone(),
            SurfaceAssetLease {
                root: root.clone(),
                grant_id: identity.grant_id.clone(),
                host_access_instance_id: host_access.instance_id(),
                expires_at_ms,
            },
        );
    }
    result["bundle_url"] = Value::String(rewritten_bundle_url);
    if let Some(stylesheets) = rewritten_stylesheets {
        result["stylesheets"] = Value::Array(stylesheets.into_iter().map(Value::String).collect());
    }
    Ok(())
}

fn surface_asset_lease_is_valid(
    host_access: &HostAccessRegistry,
    lease_id: &str,
    requested_root: &str,
) -> bool {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut leases = surface_asset_lease_registry()
        .lock()
        .expect("surface asset lease lock poisoned");
    leases.retain(|_, lease| lease.expires_at_ms > now_ms);
    leases.get(lease_id).is_some_and(|lease| {
        lease.host_access_instance_id == host_access.instance_id()
            && lease.root == requested_root
            && lease
                .grant_id
                .as_deref()
                .is_none_or(|grant_id| host_access.grant_is_currently_active(grant_id))
    })
}

async fn surface_asset_file<S>(
    State(state): State<AppState<S>>,
    Path((lease_id, prefix, file)): Path<(String, String, String)>,
) -> impl IntoResponse
where
    S: EventStore,
{
    let Some(root) = surface_asset_root(&format!("{prefix}/{file}")) else {
        return (StatusCode::NOT_FOUND, "surface asset lease not found").into_response();
    };
    if !surface_asset_lease_is_valid(state.host_access.as_ref(), &lease_id, &root) {
        return (StatusCode::NOT_FOUND, "surface asset lease not found").into_response();
    }
    serve_surface_bundle(state, &prefix, &file).await
}

async fn surface_bundle_file<S>(
    State(state): State<AppState<S>>,
    Path((prefix, file)): Path<(String, String)>,
) -> impl IntoResponse
where
    S: EventStore,
{
    serve_surface_bundle(state, &prefix, &file).await
}

async fn serve_surface_bundle<S>(state: AppState<S>, prefix: &str, file: &str) -> Response
where
    S: EventStore,
{
    let Some((root, path)) = surface_bundle_path(state, &prefix, &file) else {
        return (StatusCode::NOT_FOUND, "surface bundle path not found").into_response();
    };
    match read_static_file(&root, &path).await {
        StaticRead::Served(mut response) => {
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("no-cache, must-revalidate"),
            );
            response.headers_mut().insert(
                header::REFERRER_POLICY,
                HeaderValue::from_static("no-referrer"),
            );
            response
        }
        StaticRead::Missing | StaticRead::Forbidden => {
            (StatusCode::NOT_FOUND, "surface bundle file not found").into_response()
        }
    }
}

async fn proxy_root<S>(
    State(state): State<AppState<S>>,
    Path(route_id): Path<String>,
    OriginalUri(uri): OriginalUri,
    request: Request,
) -> impl IntoResponse
where
    S: EventStore,
{
    proxy_request(
        state,
        route_id,
        String::new(),
        uri,
        request,
        ProxyAccessMode::PathPrefix,
    )
    .await
}

async fn proxy_path<S>(
    State(state): State<AppState<S>>,
    Path((route_id, path)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    request: Request,
) -> impl IntoResponse
where
    S: EventStore,
{
    proxy_request(
        state,
        route_id,
        path,
        uri,
        request,
        ProxyAccessMode::PathPrefix,
    )
    .await
}

async fn proxy_request<S>(
    state: AppState<S>,
    route_id: String,
    path: String,
    uri: Uri,
    request: Request,
    access_mode: ProxyAccessMode,
) -> Response
where
    S: EventStore,
{
    if access_mode == ProxyAccessMode::PathPrefix {
        let Some(identity) = request.extensions().get::<HostAccessIdentity>() else {
            return (StatusCode::UNAUTHORIZED, "missing Host access identity").into_response();
        };
        if let Some(project_id) = state
            .build_jobs
            .project_for_route(&route_id)
            .or_else(|| state.target_agents.project_for_operation_route(&route_id))
        {
            if !identity.allows_project(project_id.as_str()) {
                return (
                    StatusCode::FORBIDDEN,
                    "the Host access grant does not include this project route",
                )
                    .into_response();
            }
        } else if identity.kind == HostAccessIdentityKind::Device {
            return (
                StatusCode::FORBIDDEN,
                "device grants cannot access an unowned route",
            )
                .into_response();
        }
    }
    if is_upgrade_request(request.headers()) {
        if !is_websocket_upgrade_request(request.headers()) {
            return (
                StatusCode::NOT_IMPLEMENTED,
                "upgrade proxy is not implemented",
            )
                .into_response();
        }
        let request_headers = request.headers().clone();
        let vhost_authority = match access_mode {
            ProxyAccessMode::Vhost => request_headers
                .get(header::HOST)
                .and_then(|value| value.to_str().ok())
                .and_then(normalize_host_authority),
            ProxyAccessMode::PathPrefix => None,
        };
        let subprotocols = match requested_websocket_subprotocols(&request_headers) {
            Ok(protocols) => protocols,
            Err(response) => return response,
        };
        let origin = request_headers.get(header::ORIGIN).cloned();
        let resolved = match resolve_proxy_upstream(&state, &route_id).await {
            Ok(resolved) => resolved,
            Err(response) => return response,
        };
        if resolved.protocol != ProxyProtocol::Websocket {
            return (
                StatusCode::NOT_IMPLEMENTED,
                "websocket proxy is not configured",
            )
                .into_response();
        }
        let (mut parts, _) = request.into_parts();
        let upgrade = match WebSocketUpgrade::from_request_parts(&mut parts, &state).await {
            Ok(upgrade) => upgrade,
            Err(rejection) => return rejection.into_response(),
        };

        let connection = match proxy_connect_port(&state, &resolved).await {
            Ok(connection) => connection,
            Err(response) => return response,
        };
        let target_url = loopback_websocket_url(connection.port, &path, uri.query());
        let upstream_port = resolved.port;
        let (upstream, selected_protocol) = match connect_proxy_websocket(
            target_url,
            connection.bridge_token.as_deref(),
            upstream_port,
            vhost_authority.as_deref(),
            origin,
            &subprotocols,
        )
        .await
        {
            Ok(connection) => connection,
            Err(response) => return response,
        };
        let upgrade = match selected_protocol {
            Some(protocol) => upgrade.protocols([protocol]),
            None => upgrade,
        };
        return upgrade
            .on_upgrade(move |socket| tunnel_websocket(socket, upstream))
            .into_response();
    }

    let resolved = match resolve_proxy_upstream(&state, &route_id).await {
        Ok(resolved) => resolved,
        Err(response) => return response,
    };
    if resolved.protocol != ProxyProtocol::Http {
        return (
            StatusCode::NOT_IMPLEMENTED,
            "websocket proxy is not implemented",
        )
            .into_response();
    }

    let method = request.method().clone();
    let request_headers = request.headers().clone();
    let vhost_authority = match access_mode {
        ProxyAccessMode::Vhost => request_headers
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
            .and_then(normalize_host_authority),
        ProxyAccessMode::PathPrefix => None,
    };
    let body = match to_bytes(request.into_body(), PROXY_REQUEST_BODY_LIMIT_BYTES).await {
        Ok(body) => body,
        Err(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                "proxy request body too large",
            )
                .into_response()
        }
    };

    let connection = match proxy_connect_port(&state, &resolved).await {
        Ok(connection) => connection,
        Err(response) => return response,
    };
    let target_url = loopback_proxy_url(connection.port, &path, uri.query());
    let client = hardened_proxy_client();
    let mut upstream = client
        .request(method, target_url)
        .header(header::CONNECTION, "close")
        .body(body);
    if let Some(bridge_token) = &connection.bridge_token {
        upstream = upstream.header(TARGET_TUNNEL_BRIDGE_HEADER, bridge_token);
        if vhost_authority.is_none() {
            upstream = upstream.header(header::HOST, format!("127.0.0.1:{}", resolved.port));
        }
    }
    for (name, value) in request_headers.iter() {
        if should_forward_request_header(name) {
            upstream = upstream.header(name, value);
        }
    }
    if let Some(authority) = &vhost_authority {
        upstream = upstream.header(header::HOST, authority);
    }

    let upstream_response = match upstream.send().await {
        Ok(response) => response,
        Err(_) => {
            return (StatusCode::BAD_GATEWAY, "proxy upstream request failed").into_response()
        }
    };
    let status = upstream_response.status();
    let headers = proxied_response_headers(
        upstream_response.headers(),
        access_mode,
        vhost_authority.as_deref(),
        resolved.port,
    );
    match read_limited_response_body(upstream_response, PROXY_RESPONSE_BODY_LIMIT_BYTES).await {
        Ok(body) => (status, headers, body).into_response(),
        Err(ProxyReadError::TooLarge) => {
            (StatusCode::BAD_GATEWAY, "proxy upstream response too large").into_response()
        }
        Err(_) => (StatusCode::BAD_GATEWAY, "proxy upstream response failed").into_response(),
    }
}

struct ResolvedProxyUpstream {
    protocol: ProxyProtocol,
    port: u16,
    transport: ResolvedProxyTransport,
}

enum ResolvedProxyTransport {
    Local,
    TargetTunnel(TargetTunnelOpen),
}

async fn resolve_proxy_upstream<S>(
    state: &AppState<S>,
    route_id: &str,
) -> Result<ResolvedProxyUpstream, Response>
where
    S: EventStore,
{
    let route = match state
        .runtime
        .config()
        .proxy_route_registry
        .status(route_id)
        .await
    {
        Some(route) if route.status == ProxyRouteStatusKind::Active && route.ready => route,
        Some(route) if route.status == ProxyRouteStatusKind::Active => {
            return Err((StatusCode::SERVICE_UNAVAILABLE, "deployment not ready").into_response())
        }
        _ => return Err((StatusCode::NOT_FOUND, "proxy route not found").into_response()),
    };

    let lease = match state
        .runtime
        .config()
        .port_lease_registry
        .status(&route.upstream.port_lease_id)
        .await
    {
        Some(lease) if lease.status == PortLeaseStatusKind::Active => lease,
        _ => return Err((StatusCode::NOT_FOUND, "proxy upstream not found").into_response()),
    };

    if lease.host != "127.0.0.1" || lease.bind != PortBindScope::LoopbackOnly {
        return Err((
            StatusCode::BAD_GATEWAY,
            "proxy upstream is not loopback-only",
        )
            .into_response());
    }
    if lease.port_name != route.upstream.port_name {
        return Err((StatusCode::BAD_GATEWAY, "proxy upstream port name mismatch").into_response());
    }
    if !matches!(lease.protocol, plurora_runtime::PortProtocol::Tcp) {
        return Err((StatusCode::BAD_GATEWAY, "proxy upstream must be tcp").into_response());
    }

    let target = state
        .runtime
        .config()
        .target_registry
        .status(&lease.target_id)
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, "proxy target not found").into_response())?;
    let transport = if lease.target_id == "local" {
        if target.status != ExecutionTargetStatusKind::Available
            || target.reachability != ExecutionTargetReachability::LocalHost
        {
            return Err(
                (StatusCode::SERVICE_UNAVAILABLE, "proxy target unavailable").into_response(),
            );
        }
        ResolvedProxyTransport::Local
    } else {
        if target.status != ExecutionTargetStatusKind::Available
            || target.reachability != ExecutionTargetReachability::ReverseTunnel
            || !target
                .capabilities
                .contains(&ExecutionTargetCapability::Deployment)
            || !state.target_agents.tunnel_connected(&target.id)
        {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "proxy target tunnel unavailable",
            )
                .into_response());
        }
        ResolvedProxyTransport::TargetTunnel(TargetTunnelOpen {
            stream_id: String::new(),
            target_id: target.id,
            route_id: route.id.clone(),
            port_lease_id: lease.id.clone(),
            port_name: lease.port_name.clone(),
            port: lease.port,
            lease_epoch: target.lease_epoch,
            policy_epoch: target.policy_epoch,
        })
    };

    Ok(ResolvedProxyUpstream {
        protocol: route.protocol,
        port: lease.port,
        transport,
    })
}

struct ProxyConnection {
    port: u16,
    bridge_token: Option<String>,
}

async fn proxy_connect_port<S>(
    state: &AppState<S>,
    resolved: &ResolvedProxyUpstream,
) -> Result<ProxyConnection, Response>
where
    S: EventStore,
{
    let ResolvedProxyTransport::TargetTunnel(stream) = &resolved.transport else {
        return Ok(ProxyConnection {
            port: resolved.port,
            bridge_token: None,
        });
    };
    let permit = target_tunnel_bridge_semaphore()
        .try_acquire_owned()
        .map_err(|_| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "proxy target tunnel bridge limit reached",
            )
                .into_response()
        })?;
    let stream = state
        .target_agents
        .open_tunnel_stream(stream.clone())
        .await
        .map_err(|_| {
            (StatusCode::BAD_GATEWAY, "proxy target tunnel stream failed").into_response()
        })?;
    expose_tunnel_stream_on_loopback(stream, permit)
        .await
        .map_err(|_| (StatusCode::BAD_GATEWAY, "proxy target tunnel bridge failed").into_response())
}

async fn expose_tunnel_stream_on_loopback(
    mut tunnel: tokio::io::DuplexStream,
    permit: OwnedSemaphorePermit,
) -> anyhow::Result<ProxyConnection> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let bridge_token = uuid::Uuid::new_v4().simple().to_string();
    let task_token = bridge_token.clone();
    tokio::spawn(async move {
        let _permit = permit;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return;
            };
            let accepted = tokio::time::timeout(remaining, listener.accept()).await;
            let Ok(Ok((mut local, _))) = accepted else {
                return;
            };
            let authenticated = tokio::time::timeout(
                remaining.min(Duration::from_secs(1)),
                authenticate_tunnel_bridge_request(&mut local, &task_token),
            )
            .await;
            let Ok(Ok(initial_request)) = authenticated else {
                continue;
            };
            if tunnel.write_all(&initial_request).await.is_err() {
                return;
            }
            let _ = tokio::io::copy_bidirectional(&mut local, &mut tunnel).await;
            return;
        }
    });
    Ok(ProxyConnection {
        port,
        bridge_token: Some(bridge_token),
    })
}

fn target_tunnel_bridge_semaphore() -> Arc<Semaphore> {
    static BRIDGES: OnceLock<Arc<Semaphore>> = OnceLock::new();
    BRIDGES
        .get_or_init(|| Arc::new(Semaphore::new(TARGET_TUNNEL_BRIDGE_LIMIT)))
        .clone()
}

async fn authenticate_tunnel_bridge_request(
    local: &mut TcpStream,
    expected_token: &str,
) -> anyhow::Result<Vec<u8>> {
    let mut request = Vec::new();
    let header_end = loop {
        anyhow::ensure!(
            request.len() < TARGET_TUNNEL_BRIDGE_HEADER_LIMIT_BYTES,
            "target tunnel bridge request headers are too large"
        );
        let mut chunk = [0u8; 4096];
        let read = local.read(&mut chunk).await?;
        anyhow::ensure!(
            read > 0,
            "target tunnel bridge request ended before headers"
        );
        request.extend_from_slice(&chunk[..read]);
        if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let headers = std::str::from_utf8(&request[..header_end])?;
    let mut authenticated = false;
    let mut credential_seen = false;
    let mut forwarded = Vec::with_capacity(request.len());
    for line in headers[..headers.len() - 4].split("\r\n") {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case(TARGET_TUNNEL_BRIDGE_HEADER) {
                anyhow::ensure!(
                    !credential_seen,
                    "duplicate target tunnel bridge credential"
                );
                credential_seen = true;
                authenticated = host_access::constant_time_eq(
                    value.trim().as_bytes(),
                    expected_token.as_bytes(),
                );
                continue;
            }
        }
        forwarded.extend_from_slice(line.as_bytes());
        forwarded.extend_from_slice(b"\r\n");
    }
    anyhow::ensure!(authenticated, "invalid target tunnel bridge credential");
    forwarded.extend_from_slice(b"\r\n");
    forwarded.extend_from_slice(&request[header_end..]);
    Ok(forwarded)
}

type ProxyUpstreamWebSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

fn requested_websocket_subprotocols(headers: &HeaderMap) -> Result<Vec<String>, Response> {
    let mut protocols = Vec::new();
    for value in headers.get_all(header::SEC_WEBSOCKET_PROTOCOL).iter() {
        let value = value.to_str().map_err(|_| {
            (StatusCode::BAD_REQUEST, "invalid websocket subprotocol").into_response()
        })?;
        for protocol in value.split(',').map(str::trim) {
            if protocol.is_empty()
                || protocol.len() > PROXY_WEBSOCKET_SUBPROTOCOL_BYTES
                || !protocol
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
            {
                return Err(
                    (StatusCode::BAD_REQUEST, "invalid websocket subprotocol").into_response()
                );
            }
            if !protocols.iter().any(|current| current == protocol) {
                if protocols.len() >= PROXY_WEBSOCKET_SUBPROTOCOL_LIMIT {
                    return Err((StatusCode::BAD_REQUEST, "too many websocket subprotocols")
                        .into_response());
                }
                protocols.push(protocol.to_string());
            }
        }
    }
    Ok(protocols)
}

async fn connect_proxy_websocket(
    target_url: String,
    bridge_token: Option<&str>,
    upstream_port: u16,
    upstream_authority: Option<&str>,
    origin: Option<HeaderValue>,
    subprotocols: &[String],
) -> Result<(ProxyUpstreamWebSocket, Option<String>), Response> {
    let mut request = target_url
        .as_str()
        .into_client_request()
        .map_err(|_| (StatusCode::BAD_GATEWAY, "proxy websocket request failed").into_response())?;
    if let Some(bridge_token) = bridge_token {
        let value = HeaderValue::from_str(bridge_token).map_err(|_| {
            (StatusCode::BAD_GATEWAY, "proxy websocket request failed").into_response()
        })?;
        request.headers_mut().insert(
            header::HeaderName::from_static(TARGET_TUNNEL_BRIDGE_HEADER),
            value,
        );
    }
    let upstream_authority = upstream_authority
        .map(str::to_string)
        .or_else(|| bridge_token.map(|_| format!("127.0.0.1:{upstream_port}")));
    if let Some(upstream_authority) = upstream_authority {
        let host = HeaderValue::from_str(&upstream_authority).map_err(|_| {
            (StatusCode::BAD_GATEWAY, "proxy websocket request failed").into_response()
        })?;
        request.headers_mut().insert(header::HOST, host);
    }
    if let Some(origin) = origin {
        request.headers_mut().insert(header::ORIGIN, origin);
    }
    if !subprotocols.is_empty() {
        let protocols = HeaderValue::from_str(&subprotocols.join(", ")).map_err(|_| {
            (StatusCode::BAD_REQUEST, "invalid websocket subprotocol").into_response()
        })?;
        request
            .headers_mut()
            .insert(header::SEC_WEBSOCKET_PROTOCOL, protocols);
    }
    let (upstream, response) = timeout(
        Duration::from_secs(10),
        tokio_tungstenite::connect_async(request),
    )
    .await
    .map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            "proxy websocket handshake timed out",
        )
            .into_response()
    })?
    .map_err(|_| (StatusCode::BAD_GATEWAY, "proxy websocket handshake failed").into_response())?;
    let selected_protocol = response
        .headers()
        .get(header::SEC_WEBSOCKET_PROTOCOL)
        .map(|value| {
            let value = value.to_str().map_err(|_| {
                (
                    StatusCode::BAD_GATEWAY,
                    "invalid upstream websocket subprotocol",
                )
                    .into_response()
            })?;
            if !subprotocols.iter().any(|protocol| protocol == value) {
                return Err((
                    StatusCode::BAD_GATEWAY,
                    "upstream selected an unrequested websocket subprotocol",
                )
                    .into_response());
            }
            Ok(value.to_string())
        })
        .transpose()?;
    Ok((upstream, selected_protocol))
}

async fn tunnel_websocket(downstream: WebSocket, upstream: ProxyUpstreamWebSocket) {
    let (mut downstream_tx, mut downstream_rx) = downstream.split();
    let (mut upstream_tx, mut upstream_rx) = upstream.split();

    loop {
        tokio::select! {
            downstream_msg = downstream_rx.next() => {
                let Some(Ok(message)) = downstream_msg else { break; };
                if axum_ws_message_too_large(&message) { break; }
                match axum_to_tungstenite_message(message) {
                    Some((message, should_close)) => {
                        if upstream_tx.send(message).await.is_err() { break; }
                        if should_close { break; }
                    }
                    None => {}
                }
            }
            upstream_msg = upstream_rx.next() => {
                let Some(Ok(message)) = upstream_msg else { break; };
                if tungstenite_message_too_large(&message) { break; }
                match tungstenite_to_axum_message(message) {
                    Some((message, should_close)) => {
                        if downstream_tx.send(message).await.is_err() { break; }
                        if should_close { break; }
                    }
                    None => {}
                }
            }
        }
    }

    let _ = upstream_tx
        .send(tokio_tungstenite::tungstenite::Message::Close(None))
        .await;
    let _ = downstream_tx.send(AxumWsMessage::Close(None)).await;
}

fn axum_to_tungstenite_message(
    message: AxumWsMessage,
) -> Option<(tokio_tungstenite::tungstenite::Message, bool)> {
    match message {
        AxumWsMessage::Text(text) => {
            Some((tokio_tungstenite::tungstenite::Message::Text(text), false))
        }
        AxumWsMessage::Binary(bytes) => Some((
            tokio_tungstenite::tungstenite::Message::Binary(bytes),
            false,
        )),
        AxumWsMessage::Ping(bytes) => {
            Some((tokio_tungstenite::tungstenite::Message::Ping(bytes), false))
        }
        AxumWsMessage::Pong(bytes) => {
            Some((tokio_tungstenite::tungstenite::Message::Pong(bytes), false))
        }
        AxumWsMessage::Close(_) => {
            Some((tokio_tungstenite::tungstenite::Message::Close(None), true))
        }
    }
}

fn tungstenite_to_axum_message(
    message: tokio_tungstenite::tungstenite::Message,
) -> Option<(AxumWsMessage, bool)> {
    match message {
        tokio_tungstenite::tungstenite::Message::Text(text) => {
            Some((AxumWsMessage::Text(text.to_string()), false))
        }
        tokio_tungstenite::tungstenite::Message::Binary(bytes) => {
            Some((AxumWsMessage::Binary(bytes), false))
        }
        tokio_tungstenite::tungstenite::Message::Ping(bytes) => {
            Some((AxumWsMessage::Ping(bytes), false))
        }
        tokio_tungstenite::tungstenite::Message::Pong(bytes) => {
            Some((AxumWsMessage::Pong(bytes), false))
        }
        tokio_tungstenite::tungstenite::Message::Close(_) => {
            Some((AxumWsMessage::Close(None), true))
        }
        tokio_tungstenite::tungstenite::Message::Frame(_) => None,
    }
}

fn axum_ws_message_too_large(message: &AxumWsMessage) -> bool {
    (match message {
        AxumWsMessage::Text(text) => text.len(),
        AxumWsMessage::Binary(bytes) | AxumWsMessage::Ping(bytes) | AxumWsMessage::Pong(bytes) => {
            bytes.len()
        }
        AxumWsMessage::Close(_) => 0,
    }) > PROXY_WEBSOCKET_FRAME_LIMIT_BYTES
}

fn tungstenite_message_too_large(message: &tokio_tungstenite::tungstenite::Message) -> bool {
    (match message {
        tokio_tungstenite::tungstenite::Message::Text(text) => text.len(),
        tokio_tungstenite::tungstenite::Message::Binary(bytes)
        | tokio_tungstenite::tungstenite::Message::Ping(bytes)
        | tokio_tungstenite::tungstenite::Message::Pong(bytes) => bytes.len(),
        tokio_tungstenite::tungstenite::Message::Close(_)
        | tokio_tungstenite::tungstenite::Message::Frame(_) => 0,
    }) > PROXY_WEBSOCKET_FRAME_LIMIT_BYTES
}

fn hardened_proxy_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("hardened proxy client builds")
    })
}

#[derive(Debug)]
enum ProxyReadError {
    Upstream,
    TooLarge,
}

async fn read_limited_response_body(
    response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, ProxyReadError> {
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ProxyReadError::Upstream)?;
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(ProxyReadError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn loopback_proxy_url(port: u16, path: &str, query: Option<&str>) -> String {
    loopback_proxy_url_with_scheme("http", port, path, query)
}

fn loopback_websocket_url(port: u16, path: &str, query: Option<&str>) -> String {
    loopback_proxy_url_with_scheme("ws", port, path, query)
}

fn loopback_proxy_url_with_scheme(
    scheme: &str,
    port: u16,
    path: &str,
    query: Option<&str>,
) -> String {
    let path = if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", path.trim_start_matches('/'))
    };
    let mut url = format!("{scheme}://127.0.0.1:{port}{path}");
    if let Some(query) = sanitized_proxy_query(query) {
        url.push('?');
        url.push_str(&query);
    }
    url
}

fn sanitized_proxy_query(query: Option<&str>) -> Option<String> {
    let query = query?;
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    let mut any = false;
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        if key == "access_token" {
            continue;
        }
        serializer.append_pair(&key, &value);
        any = true;
    }
    any.then(|| serializer.finish())
}

fn is_upgrade_request(headers: &HeaderMap) -> bool {
    headers.contains_key(header::UPGRADE)
        || headers
            .get(header::CONNECTION)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case("upgrade"))
            })
}

fn is_websocket_upgrade_request(headers: &HeaderMap) -> bool {
    headers
        .get(header::UPGRADE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
        && headers
            .get(header::CONNECTION)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case("upgrade"))
            })
}

fn should_forward_request_header(name: &header::HeaderName) -> bool {
    matches!(
        name,
        &header::ACCEPT
            | &header::ACCEPT_LANGUAGE
            | &header::CACHE_CONTROL
            | &header::CONTENT_TYPE
            | &header::IF_MATCH
            | &header::IF_MODIFIED_SINCE
            | &header::IF_NONE_MATCH
            | &header::IF_UNMODIFIED_SINCE
            | &header::ORIGIN
            | &header::RANGE
            | &header::USER_AGENT
    )
}

fn proxied_response_headers(
    headers: &HeaderMap,
    access_mode: ProxyAccessMode,
    vhost_authority: Option<&str>,
    upstream_port: u16,
) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (name, value) in headers.iter() {
        if should_forward_response_header(name) {
            out.append(name, value.clone());
        }
    }
    if access_mode == ProxyAccessMode::Vhost {
        for value in headers.get_all(header::SET_COOKIE).iter() {
            if let Some(rewritten) = rewrite_vhost_set_cookie(value) {
                out.append(header::SET_COOKIE, rewritten);
            }
        }
        if let (Some(authority), Some(location)) = (
            vhost_authority,
            headers
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok()),
        ) {
            if let Some(rewritten) = rewrite_vhost_location(location, authority, upstream_port) {
                out.insert(header::LOCATION, rewritten);
            }
        }
    }
    out
}

fn should_forward_response_header(name: &header::HeaderName) -> bool {
    !matches!(
        name,
        &header::CONNECTION
            | &header::PROXY_AUTHENTICATE
            | &header::PROXY_AUTHORIZATION
            | &header::TE
            | &header::TRAILER
            | &header::TRANSFER_ENCODING
            | &header::UPGRADE
            | &header::SET_COOKIE
            | &header::LOCATION
    ) && !name.as_str().eq_ignore_ascii_case("keep-alive")
        && !name
            .as_str()
            .to_ascii_lowercase()
            .starts_with("access-control-")
}

fn rewrite_vhost_set_cookie(value: &HeaderValue) -> Option<HeaderValue> {
    let raw = value.to_str().ok()?;
    let mut parts = Vec::new();
    for part in raw.split(';') {
        let trimmed = part.trim();
        if trimmed.is_empty() || trimmed.to_ascii_lowercase().starts_with("domain=") {
            continue;
        }
        parts.push(trimmed);
    }
    if parts.is_empty() {
        return None;
    }
    HeaderValue::from_str(&parts.join("; ")).ok()
}

fn rewrite_vhost_location(
    location: &str,
    authority: &str,
    upstream_port: u16,
) -> Option<HeaderValue> {
    if location.starts_with('/') && !location.starts_with("//") {
        return HeaderValue::from_str(location).ok();
    }
    let parsed = url::Url::parse(location).ok()?;
    if parsed.scheme() != "http"
        || parsed.host_str() != Some("127.0.0.1")
        || parsed.port_or_known_default() != Some(upstream_port)
    {
        return None;
    }
    let mut rewritten = format!("https://{authority}{}", parsed.path());
    if let Some(query) = parsed
        .query()
        .and_then(|query| sanitized_proxy_query(Some(query)))
    {
        rewritten.push('?');
        rewritten.push_str(&query);
    }
    HeaderValue::from_str(&rewritten).ok()
}

async fn static_fallback<S>(
    State(state): State<AppState<S>>,
    method: Method,
    OriginalUri(uri): OriginalUri,
) -> impl IntoResponse
where
    S: EventStore,
{
    if method != Method::GET && method != Method::HEAD {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }

    let Some(static_dir) = state.static_dir.as_ref() else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };

    let Ok(static_root) = std::fs::canonicalize(static_dir) else {
        return (StatusCode::NOT_FOUND, "static root not found").into_response();
    };

    let request_path = uri.path();
    if is_reserved_service_path(request_path) {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }

    let candidate = if request_path == "/" {
        static_root.join("index.html")
    } else {
        let Some(relative) = safe_relative_path(request_path.trim_start_matches('/')) else {
            return (StatusCode::NOT_FOUND, "not found").into_response();
        };
        static_root.join(relative)
    };

    let path = if candidate.is_dir() {
        candidate.join("index.html")
    } else {
        candidate
    };

    match read_static_file(&static_root, &path).await {
        StaticRead::Served(response) => return response,
        StaticRead::Forbidden => {
            return (StatusCode::NOT_FOUND, "static file not found").into_response()
        }
        StaticRead::Missing => {}
    }

    if is_static_asset_path(request_path) {
        return (StatusCode::NOT_FOUND, "static file not found").into_response();
    }

    if !is_spa_fallback_path(request_path) {
        return (StatusCode::NOT_FOUND, "static file not found").into_response();
    }

    let index = static_root.join("index.html");
    match read_static_file(&static_root, &index).await {
        StaticRead::Served(response) => response,
        StaticRead::Missing | StaticRead::Forbidden => {
            (StatusCode::NOT_FOUND, "static file not found").into_response()
        }
    }
}

fn is_reserved_service_path(path: &str) -> bool {
    path == "/rpc"
        || path.starts_with("/rpc/")
        || path == "/p"
        || path.starts_with("/p/")
        || path == "/host"
        || path.starts_with("/host/")
        || path == "/surface-bundles"
        || path.starts_with("/surface-bundles/")
        || path == "/surface-assets"
        || path.starts_with("/surface-assets/")
}

fn is_spa_fallback_path(path: &str) -> bool {
    path == "/"
        || path == "/pair"
        || path
            .strip_prefix("/project/")
            .is_some_and(is_valid_project_path_segment)
}

fn is_valid_project_path_segment(segment: &str) -> bool {
    !segment.is_empty()
        && !segment.contains('/')
        && segment.len() <= 128
        && url::form_urlencoded::parse(format!("id={segment}").as_bytes())
            .next()
            .is_some_and(|(_, decoded)| {
                !decoded.contains('/')
                    && decoded.as_ref() != "."
                    && decoded.as_ref() != ".."
                    && !decoded.starts_with('.')
                    && !decoded.contains("..")
                    && decoded
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphanumeric())
                    && decoded
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            })
}

fn is_static_asset_path(path: &str) -> bool {
    path == "/surface-frame.html"
        || path == "/surface-frame-bootstrap.js"
        || path.starts_with("/assets/")
        || std::path::Path::new(path.trim_start_matches('/'))
            .extension()
            .is_some()
}

enum StaticRead {
    Served(Response),
    Missing,
    Forbidden,
}

async fn read_static_file(static_root: &FsPath, path: &FsPath) -> StaticRead {
    let static_root = match std::fs::canonicalize(static_root) {
        Ok(path) => path,
        Err(_) => return StaticRead::Missing,
    };
    let canonical = match std::fs::canonicalize(path) {
        Ok(path) => path,
        Err(_) => return StaticRead::Missing,
    };
    if !canonical.starts_with(&static_root) || !canonical.is_file() {
        return StaticRead::Forbidden;
    }
    let bytes = match tokio::fs::read(&canonical).await {
        Ok(bytes) => bytes,
        Err(_) => return StaticRead::Missing,
    };
    let cache_control = cache_control_for(&static_root, &canonical);
    StaticRead::Served(
        (
            public_static_headers(content_type_for(&canonical), cache_control),
            bytes,
        )
            .into_response(),
    )
}

fn public_static_headers(content_type: &'static str, cache_control: &'static str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control),
    );
    // Surface frames are sandboxed without `allow-same-origin`; browser module
    // loading treats the frame as an opaque origin. Public shell assets and
    // attenuated `/surface-assets/<lease>/...` responses must be CORS-readable;
    // raw bundle, RPC, and kernel routes keep their Host authority gates.
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers
}

fn cache_control_for(static_root: &FsPath, path: &FsPath) -> &'static str {
    let relative = path.strip_prefix(static_root).unwrap_or(path);
    let filename = relative.file_name().and_then(|name| name.to_str());
    let extension = relative.extension().and_then(|ext| ext.to_str());

    if matches!(filename, Some("index.html" | "sw.js")) || extension == Some("webmanifest") {
        "no-cache"
    } else if relative.starts_with("assets") {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=3600"
    }
}

fn surface_bundle_path<S>(
    state: AppState<S>,
    prefix: &str,
    file: &str,
) -> Option<(PathBuf, PathBuf)>
where
    S: EventStore,
{
    let safe_file = safe_relative_path(file)?;
    if prefix == "projects" {
        let mut parts = safe_file.components();
        let project_id = parts.next()?.as_os_str().to_str()?;
        let project_id = plurora_core::project::ProjectId::new(project_id).ok()?;
        let mut rest = PathBuf::new();
        for part in parts {
            rest.push(part.as_os_str());
        }
        if rest.as_os_str().is_empty() {
            return None;
        }
        let root = recoverable_project_dist_dir(&project_id)?;
        let path = root.join(rest);
        return Some((root, path));
    }

    let Some(base) = state.runtime.config().surface_dev_paths.get(prefix) else {
        return None;
    };
    let root = PathBuf::from(base);
    let path = root.join(safe_file);
    Some((root, path))
}

fn recoverable_project_dist_dir(project_id: &plurora_core::project::ProjectId) -> Option<PathBuf> {
    let project_dir = plurora_core::paths::project_dir(project_id).ok()?;
    let dist = project_dir.join("dist");
    if dist.is_dir() {
        return Some(dist);
    }
    latest_dist_backup(&project_dir)
}

fn latest_dist_backup(project_dir: &FsPath) -> Option<PathBuf> {
    let mut candidates = std::fs::read_dir(project_dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            if !name.starts_with(".dist.bak-") || !path.is_dir() {
                return None;
            }
            let modified = entry.metadata().and_then(|meta| meta.modified()).ok();
            Some((modified, path))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(modified, _)| *modified);
    candidates.pop().map(|(_, path)| path)
}

fn safe_relative_path(path: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in std::path::Path::new(path).components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

fn content_type_for(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("mjs") | Some("js") => "application/javascript",
        Some("css") => "text/css",
        Some("json") | Some("map") => "application/json",
        Some("webmanifest") => "application/manifest+json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

async fn rpc<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Json(raw): Json<Value>,
) -> Json<ProtocolResponse>
where
    S: EventStore,
{
    let response_id = raw
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("invalid")
        .to_string();
    let request = match serde_json::from_value::<ProtocolRequest>(raw) {
        Ok(request) => request,
        Err(error) => {
            return Json(ProtocolResponse {
                id: response_id,
                result: None,
                error: Some(ProtocolError::invalid_request(error.to_string())),
            });
        }
    };
    let ProtocolRequest {
        id,
        method,
        session_id,
        contract,
        params,
    } = request;
    let resolved_method = resolve_contract_method(&method).ok();
    let policy_method = resolved_method
        .as_ref()
        .map(|resolved| resolved.contract.id.as_str())
        .unwrap_or(method.as_str());
    let required_scope = required_host_scope_for_protocol_method(policy_method);
    if !identity.allows(required_scope) {
        return Json(ProtocolResponse {
            id,
            result: None,
            error: Some(ProtocolError::new(
                "runtime/error/permission_denied",
                "Host access grant does not include the required scope",
                serde_json::json!({ "required_scope": required_scope }),
            )),
        });
    }
    let operation_resources = host_operation_resources_for_protocol_method(policy_method, &params);
    let mut context = identity.protocol_context("http_rpc");
    if identity.kind == HostAccessIdentityKind::Device && !operation_resources.is_empty() {
        context = context.with_host_operation(required_scope.as_str(), operation_resources);
    }
    context.session_id = session_id;
    let result = state
        .runtime
        .call_protocol_negotiated(&context, &method, params, contract.as_ref())
        .await
        .and_then(|mut result| {
            if policy_method == "host.surface.bundle.resolve" {
                mint_surface_asset_lease(&mut result, &identity, state.host_access.as_ref())
                    .map_err(ProtocolError::from_anyhow)?;
            }
            Ok(result)
        });
    match result {
        Ok(result) => Json(ProtocolResponse {
            id,
            result: Some(result),
            error: None,
        }),
        Err(error) => Json(ProtocolResponse {
            id,
            result: None,
            error: Some(error),
        }),
    }
}

fn required_host_scope_for_protocol_method(method: &str) -> HostAccessScope {
    match method {
        "host.info"
        | "host.ping"
        | "host.diagnostics"
        | "host.package.logs"
        | "host.package.list"
        | "host.package.status"
        | "host.package.describe"
        | "host.project.list"
        | "host.project.get"
        | "host.project.status"
        | "host.target.list"
        | "host.target.status"
        | "host.exec.list"
        | "host.exec.status"
        | "host.exec.logs"
        | "host.port.list"
        | "host.port.status"
        | "host.proxy.list"
        | "host.proxy.status"
        | "host.surface.bundle.resolve"
        | "shell.contribution.list"
        | "shell.contribution.describe"
        | "change.proposal.get"
        | "change.proposal.list"
        | "projection.get"
        | "projection.list"
        | "context.branch.list"
        | "context.get"
        | "context.list"
        | "journal.list"
        | "journal.subscribe"
        | "capability.discover"
        | "capability.describe"
        | "protocol.extension.list"
        | "protocol.extension.describe"
        | "protocol.hook.list"
        | "object.get"
        | "object.list" => HostAccessScope::Observe,

        "host.project.start" | "host.project.stop" | "context.open" | "context.close"
        | "context.fork" => HostAccessScope::ProjectOperate,

        "host.target.register"
        | "host.target.unregister"
        | "host.exec.start"
        | "host.exec.stop"
        | "host.port.lease"
        | "host.port.release"
        | "host.proxy.register"
        | "host.proxy.unregister" => HostAccessScope::Deploy,

        "change.proposal.create" => HostAccessScope::DevelopPropose,
        "change.proposal.approve" | "change.proposal.reject" => HostAccessScope::DevelopApprove,
        "change.proposal.apply" => HostAccessScope::DevelopExecute,

        // Unknown and broad administrative methods fail closed for scoped devices.
        _ => HostAccessScope::AccessManage,
    }
}

fn host_operation_resources_for_protocol_method(
    method: &str,
    params: &Value,
) -> Vec<ProtocolResourceSelector> {
    if matches!(
        method,
        "host.project.get" | "host.project.start" | "host.project.stop" | "host.project.status"
    ) {
        return params
            .get("project_id")
            .and_then(Value::as_str)
            .map(|project_id| {
                vec![ProtocolResourceSelector {
                    owner: "host".to_string(),
                    kind: "project".to_string(),
                    id: Some(project_id.to_string()),
                }]
            })
            .unwrap_or_default();
    }

    if matches!(
        method,
        "host.target.status"
            | "host.target.register"
            | "host.target.unregister"
            | "host.exec.start"
            | "host.port.lease"
    ) {
        return params
            .get("target_id")
            .and_then(Value::as_str)
            .map(|target_id| {
                vec![ProtocolResourceSelector {
                    owner: "host".to_string(),
                    kind: "target".to_string(),
                    id: Some(target_id.to_string()),
                }]
            })
            .unwrap_or_default();
    }
    Vec::new()
}

pub struct ServiceError {
    error: anyhow::Error,
    status: Option<StatusCode>,
}

impl ServiceError {
    fn with_status(status: StatusCode, message: impl Into<String>) -> Self {
        let code = match status {
            StatusCode::UNAUTHORIZED => "runtime/error/unauthorized",
            StatusCode::FORBIDDEN => "runtime/error/permission_denied",
            StatusCode::NOT_FOUND => "runtime/error/not_found",
            StatusCode::TOO_MANY_REQUESTS => "runtime/error/package_state",
            StatusCode::BAD_REQUEST => "runtime/error/invalid_request",
            _ => "runtime/error/internal",
        };
        Self {
            error: anyhow::anyhow!("{}: {}", code, message.into()),
            status: Some(status),
        }
    }
}

impl<E> From<E> for ServiceError
where
    E: Into<anyhow::Error>,
{
    fn from(value: E) -> Self {
        Self {
            error: value.into(),
            status: None,
        }
    }
}

impl axum::response::IntoResponse for ServiceError {
    fn into_response(self) -> axum::response::Response {
        let error = ProtocolError::from_anyhow(self.error);
        let status = self.status.unwrap_or_else(|| match error.code.as_str() {
            "runtime/error/permission_denied" => StatusCode::FORBIDDEN,
            "runtime/error/not_found" => StatusCode::NOT_FOUND,
            "runtime/error/schema_invalid" | "runtime/error/invalid_request" => {
                StatusCode::BAD_REQUEST
            }
            "runtime/error/ambiguous_route" | "runtime/error/package_state" => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        });
        (status, Json(serde_json::json!({ "error": error }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::net::SocketAddr;

    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use plurora_core::project::{ProjectDescriptor, ProjectInner, ProjectType, SecretPolicy};
    use plurora_runtime::{
        ExecutionTargetId, PortLeaseRequest, PortProtocol, ProjectRegistry, ProxyProtocol,
        ProxyRouteRegisterRequest, ProxyRouteUpstream,
    };
    use serde_json::json;
    use tokio::net::TcpStream;
    use tokio::sync::Mutex;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::Message;
    use tower::ServiceExt;

    use super::*;

    #[test]
    fn root_token_comparison_accepts_only_the_exact_credential() {
        assert!(root_token_matches("secret-token", "secret-token"));
        assert!(!root_token_matches("secret-token", "secret-token-2"));
        assert!(!root_token_matches("", "secret-token"));
    }

    #[test]
    fn query_credentials_are_limited_to_get_event_streams() -> anyhow::Result<()> {
        let event_request = Request::builder()
            .method(Method::GET)
            .uri("/journal/subscribe/session-1?access_token=event-token")
            .body(Body::empty())?;
        assert_eq!(
            presented_host_credentials(&event_request)
                .event_query
                .as_deref(),
            Some("event-token")
        );

        let job_events = Request::builder()
            .method(Method::GET)
            .uri("/host/v1/build-deploy/job-1/events?access_token=device-token")
            .body(Body::empty())?;
        assert_eq!(
            presented_host_credentials(&job_events)
                .event_query
                .as_deref(),
            Some("device-token")
        );

        for request in [
            Request::builder()
                .method(Method::GET)
                .uri("/rpc?access_token=leak")
                .body(Body::empty())?,
            Request::builder()
                .method(Method::POST)
                .uri("/host/v1/build-deploy/job-1/events?access_token=leak")
                .body(Body::empty())?,
        ] {
            assert!(presented_host_credentials(&request).event_query.is_none());
        }
        Ok(())
    }

    #[test]
    fn cookie_mutations_require_same_origin_when_origin_is_present() -> anyhow::Result<()> {
        let same_origin = Request::builder()
            .method(Method::POST)
            .uri("/host/v1/deploy")
            .header(header::HOST, "host.example.test:443")
            .header(header::ORIGIN, "https://host.example.test")
            .body(Body::empty())?;
        assert!(!unsafe_cookie_origin_mismatch(
            &same_origin,
            HostCredentialSource::DeviceCookie
        ));

        let cross_origin = Request::builder()
            .method(Method::POST)
            .uri("/host/v1/deploy")
            .header(header::HOST, "host.example.test")
            .header(header::ORIGIN, "https://evil.example.test")
            .body(Body::empty())?;
        assert!(unsafe_cookie_origin_mismatch(
            &cross_origin,
            HostCredentialSource::DeviceCookie
        ));

        let bearer_cross_origin = Request::builder()
            .method(Method::POST)
            .uri("/host/v1/deploy")
            .header(header::HOST, "host.example.test")
            .header(header::ORIGIN, "https://evil.example.test")
            .body(Body::empty())?;
        assert!(!unsafe_cookie_origin_mismatch(
            &bearer_cross_origin,
            HostCredentialSource::Authorization
        ));
        Ok(())
    }

    #[test]
    fn host_scope_mapping_is_explicit_and_fails_closed() {
        assert_eq!(
            required_host_scope_for_http(&Method::GET, "/host/v1/projects/demo/changes"),
            Some(HostAccessScope::DevelopPropose)
        );
        assert_eq!(
            required_host_scope_for_http(
                &Method::POST,
                "/host/v1/projects/demo/changes/change-1/approve"
            ),
            Some(HostAccessScope::DevelopApprove)
        );
        assert_eq!(
            required_host_scope_for_http(
                &Method::POST,
                "/host/v1/projects/demo/changes/change-1/execute"
            ),
            Some(HostAccessScope::DevelopExecute)
        );
        assert_eq!(
            required_host_scope_for_http(
                &Method::POST,
                "/host/v1/projects/demo/changes/change-1/deployment/preview"
            ),
            Some(HostAccessScope::Deploy)
        );
        for action in ["approve", "activate", "reconcile"] {
            assert_eq!(
                required_host_scope_for_http(
                    &Method::POST,
                    &format!("/host/v1/projects/demo/changes/change-1/deployment/{action}")
                ),
                Some(HostAccessScope::Deploy)
            );
        }
        assert_eq!(
            required_host_scope_for_http(&Method::POST, "/host/v1/deploy"),
            Some(HostAccessScope::Deploy)
        );
        assert_eq!(
            required_host_scope_for_http(&Method::POST, "/unrecognized"),
            Some(HostAccessScope::AccessManage)
        );
        assert_eq!(
            required_host_scope_for_protocol_method("host.project.start"),
            HostAccessScope::ProjectOperate
        );
        assert_eq!(
            required_host_scope_for_protocol_method("unknown.future.method"),
            HostAccessScope::AccessManage
        );
    }

    #[test]
    fn workspace_clone_url_policy_rejects_unsafe_sources() {
        for url in [
            "file:///tmp/repo",
            "ssh://git@example.com/repo.git",
            "git@example.com:repo.git",
            "https://user@example.com/repo.git",
            "https://user:token@example.com/repo.git",
            "https://example.com/repo.git?token=secret",
            "https://example.com/repo.git#token",
            "/tmp/local-repo",
        ] {
            assert!(validate_workspace_clone_url(url).is_err(), "accepted {url}");
        }
        assert!(validate_workspace_clone_url("https://example.com/org/repo.git").is_ok());
    }

    #[test]
    fn workspace_clone_invocation_uses_expected_git_tools_shape() -> anyhow::Result<()> {
        let project_id = ProjectId::new("clone__abc123")?;
        let request = ProjectWorkspaceCloneRequest {
            project_id: project_id.clone(),
            source_url: "https://example.com/org/repo.git".to_string(),
            ref_name: "refs/heads/main".to_string(),
        };
        let data_dir = tempfile::tempdir()?;
        let invocation = build_project_workspace_clone_invocation(&request, Some(data_dir.path()))?;

        assert_eq!(
            invocation.workspace_dir,
            data_dir.path().join("projects/clone__abc123/workspace")
        );
        assert_eq!(
            invocation.staging_dir,
            data_dir
                .path()
                .join("projects/clone__abc123/workspace.staging")
        );
        assert_eq!(
            invocation.resolve_ref_params,
            json!({"remote_url":"https://example.com/org/repo.git","ref":"refs/heads/main"})
        );
        assert_eq!(
            invocation.fetch_tree_params,
            json!({
                "remote_url":"https://example.com/org/repo.git",
                "ref_name":"refs/heads/main",
                "dest_dir": invocation.staging_dir.to_string_lossy(),
                "max_files": DEPLOYMENT_WORKSPACE_MAX_FILES,
                "max_directories": DEPLOYMENT_WORKSPACE_MAX_DIRECTORIES,
                "max_total_bytes": DEPLOYMENT_WORKSPACE_MAX_BYTES,
                "max_download_bytes": DEPLOYMENT_GIT_DOWNLOAD_MAX_BYTES,
            })
        );
        Ok(())
    }

    #[test]
    fn workspace_destination_rejects_escape() -> anyhow::Result<()> {
        let project_id = ProjectId::new("clone__abc123")?;
        let data_dir = tempfile::tempdir()?;
        assert!(validate_workspace_destination(
            &project_id,
            Some(data_dir.path()),
            &data_dir.path().join("projects/clone__abc123/workspace")
        )
        .is_ok());
        assert!(validate_workspace_destination(
            &project_id,
            Some(data_dir.path()),
            &data_dir.path().join("projects/other__abc123/workspace")
        )
        .is_err());
        assert!(validate_workspace_destination(
            &project_id,
            Some(data_dir.path()),
            &data_dir
                .path()
                .join("projects/clone__abc123/../other/workspace")
        )
        .is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn workspace_clone_rejects_symlinked_projects_root() -> anyhow::Result<()> {
        let project_id = ProjectId::new("clone__abc123")?;
        let data_dir = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        std::fs::create_dir_all(outside.path().join(project_id.as_str()))?;
        std::os::unix::fs::symlink(outside.path(), data_dir.path().join("projects"))?;

        assert!(canonical_workspace_project_root(&project_id, Some(data_dir.path())).is_err());
        Ok(())
    }

    fn valid_build_deploy_request() -> HostBuildDeployRequest {
        HostBuildDeployRequest {
            project_id: ProjectId::new("build__abc123").unwrap(),
            source_url: "https://example.com/org/repo.git".to_string(),
            ref_name: "refs/heads/main".to_string(),
            strategy: Some("dockerfile".to_string()),
            dockerfile: Some("Dockerfile".to_string()),
            container_port: 3000,
            port_name: "web".to_string(),
            route_id: "route-build".to_string(),
            route_access: ProxyRouteAccess::HostAuthenticated,
            health_path: Some("/health".to_string()),
            approved: true,
            source_commit: Some("0123456789abcdef0123456789abcdef01234567".to_string()),
            build_id: Some("build-001".to_string()),
            runtime_env: Vec::new(),
            runtime_mounts: Vec::new(),
            idempotency_key: None,
        }
    }

    #[test]
    fn build_deploy_request_validation_blocks_unapproved_and_unsafe() {
        let mut request = valid_build_deploy_request();
        assert!(validate_host_build_deploy_request(&request).is_ok());

        request.approved = false;
        assert!(validate_host_build_deploy_request(&request).is_err());

        let mut request = valid_build_deploy_request();
        request.source_url = "file:///tmp/repo".to_string();
        assert!(validate_host_build_deploy_request(&request).is_err());

        let mut request = valid_build_deploy_request();
        request.dockerfile = Some("../Dockerfile".to_string());
        assert!(validate_host_build_deploy_request(&request).is_err());

        let mut request = valid_build_deploy_request();
        request.build_id = Some("../bad".to_string());
        assert!(validate_host_build_deploy_request(&request).is_err());

        let mut request = valid_build_deploy_request();
        request.strategy = Some("compose".to_string());
        assert!(validate_host_build_deploy_request(&request).is_err());

        let mut request = valid_build_deploy_request();
        request.strategy = Some("nixpacks".to_string());
        assert!(validate_host_build_deploy_request(&request).is_ok());
    }

    #[test]
    fn build_deploy_runtime_env_validation_rejects_bad_specs() {
        let mut request = valid_build_deploy_request();
        request.runtime_env = vec![RuntimeEnvSpec {
            name: "GOOD_NAME".to_string(),
            value: Some("ok".to_string()),
            secret_ref: None,
        }];
        assert!(validate_host_build_deploy_request(&request).is_ok());

        let mut request = valid_build_deploy_request();
        request.runtime_env = vec![RuntimeEnvSpec {
            name: "1BAD".to_string(),
            value: Some("ok".to_string()),
            secret_ref: None,
        }];
        assert!(validate_host_build_deploy_request(&request).is_err());

        let mut request = valid_build_deploy_request();
        request.runtime_env = vec![
            RuntimeEnvSpec {
                name: "DUP".to_string(),
                value: Some("one".to_string()),
                secret_ref: None,
            },
            RuntimeEnvSpec {
                name: "DUP".to_string(),
                value: Some("two".to_string()),
                secret_ref: None,
            },
        ];
        assert!(validate_host_build_deploy_request(&request).is_err());

        let mut request = valid_build_deploy_request();
        request.runtime_env = vec![RuntimeEnvSpec {
            name: "BOTH".to_string(),
            value: Some("plain".to_string()),
            secret_ref: Some("secret_ref:env:KEY".to_string()),
        }];
        assert!(validate_host_build_deploy_request(&request).is_err());

        let mut request = valid_build_deploy_request();
        request.runtime_env = vec![RuntimeEnvSpec {
            name: "NEITHER".to_string(),
            value: None,
            secret_ref: None,
        }];
        assert!(validate_host_build_deploy_request(&request).is_err());

        let mut request = valid_build_deploy_request();
        request.runtime_env = vec![RuntimeEnvSpec {
            name: "NUL".to_string(),
            value: Some("bad\0value".to_string()),
            secret_ref: None,
        }];
        assert!(validate_host_build_deploy_request(&request).is_err());

        let mut request = valid_build_deploy_request();
        request.runtime_env = (0..=MAX_RUNTIME_ENV_ENTRIES)
            .map(|idx| RuntimeEnvSpec {
                name: format!("ENV_{idx}"),
                value: Some("x".to_string()),
                secret_ref: None,
            })
            .collect();
        assert!(validate_host_build_deploy_request(&request).is_err());
    }

    #[test]
    fn resolved_runtime_env_debug_redacts_values_and_response_has_names_only() {
        let resolved = ResolvedRuntimeEnv {
            name: "TOKEN".to_string(),
            value: "super-secret-value".to_string(),
            source: RuntimeEnvSourceKind::SecretRef,
        };
        let debug = format!("{resolved:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("super-secret-value"));

        let summary = RuntimeEnvSummary {
            name: resolved.name,
            source: resolved.source,
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("TOKEN"));
        assert!(!json.contains("super-secret-value"));
    }

    fn approved_ro_mount(source: &FsPath, target: &str) -> RuntimeMountSpec {
        RuntimeMountSpec {
            source_host_path: source.to_string_lossy().to_string(),
            container_path: target.to_string(),
            mode: RuntimeMountMode::Ro,
            approved: true,
            high_risk_approved: false,
            reason: "needed for runtime data".to_string(),
        }
    }

    #[cfg(unix)]
    fn create_test_file_symlink(source: &FsPath, target: &FsPath) -> std::io::Result<bool> {
        std::os::unix::fs::symlink(source, target)?;
        Ok(true)
    }

    #[cfg(windows)]
    fn create_test_file_symlink(source: &FsPath, target: &FsPath) -> std::io::Result<bool> {
        match std::os::windows::fs::symlink_file(source, target) {
            Ok(()) => Ok(true),
            Err(error) if error.raw_os_error() == Some(1314) => Ok(false),
            Err(error) => Err(error),
        }
    }

    #[test]
    fn runtime_mount_validation_accepts_temp_ro_mount_and_redacts_summary() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let source = tmp.path().join("data");
        std::fs::create_dir(&source)?;
        let mut request = valid_build_deploy_request();
        request.runtime_mounts = vec![approved_ro_mount(&source, "/data/app")];
        validate_host_build_deploy_request(&request)?;
        let resolved = resolve_runtime_mounts(&request)?;
        assert_eq!(resolved.len(), 1);
        let debug = format!("{:?}", resolved[0]);
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains(&source.to_string_lossy().to_string()));
        let summary = RuntimeMountSummary {
            container_path: resolved[0].container_path.clone(),
            mode: resolved[0].mode,
            source_basename: resolved[0].source_basename.clone(),
            source_kind: resolved[0].source_kind.clone(),
            source_hash: resolved[0].source_hash.clone(),
            approved: true,
        };
        let json = serde_json::to_string(&summary)?;
        assert!(json.contains("/data/app"));
        assert!(!json.contains(&tmp.path().to_string_lossy().to_string()));
        Ok(())
    }

    #[test]
    fn runtime_mount_validation_rejects_unsafe_specs() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let source = tmp.path().join("data");
        std::fs::create_dir(&source)?;

        let mut request = valid_build_deploy_request();
        let mut mount = approved_ro_mount(&source, "/data/app");
        mount.approved = false;
        request.runtime_mounts = vec![mount];
        assert!(validate_host_build_deploy_request(&request).is_err());

        let mut request = valid_build_deploy_request();
        let mut mount = approved_ro_mount(&source, "/data/app");
        mount.mode = RuntimeMountMode::Rw;
        request.runtime_mounts = vec![mount];
        assert!(validate_host_build_deploy_request(&request).is_err());

        let mut request = valid_build_deploy_request();
        request.runtime_mounts = vec![approved_ro_mount(&source, "/etc/config")];
        assert!(validate_host_build_deploy_request(&request).is_err());

        let mut request = valid_build_deploy_request();
        request.runtime_mounts = vec![approved_ro_mount(FsPath::new("/"), "/data/root")];
        assert!(validate_host_build_deploy_request(&request).is_err());

        let mut request = valid_build_deploy_request();
        request.runtime_mounts = vec![
            approved_ro_mount(&source, "/data/app"),
            approved_ro_mount(&source, "/data/app"),
        ];
        assert!(validate_host_build_deploy_request(&request).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn runtime_mount_validation_rejects_symlink_to_denied_path() -> anyhow::Result<()> {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir()?;
        let link = tmp.path().join("etc-link");
        symlink("/etc", &link)?;
        let mut request = valid_build_deploy_request();
        request.runtime_mounts = vec![approved_ro_mount(&link, "/data/etc")];
        assert!(validate_host_build_deploy_request(&request).is_err());
        Ok(())
    }

    #[test]
    fn build_deploy_job_registry_cancel_terminal_and_redacts_logs() -> anyhow::Result<()> {
        let registry = BuildDeployJobRegistry::default();
        let request = valid_build_deploy_request();
        let job_id = registry
            .create_job(&request, &HostAccessIdentity::root())?
            .job_id;
        registry.transition(
            &job_id,
            BuildDeployJobState::Building,
            "building /tmp/secret_ref:env:TOKEN",
        );
        let events = registry.events(&job_id).unwrap();
        assert!(events.iter().any(|event| event.message.contains("<path>/")));
        assert!(!events.iter().any(|event| event.message.contains("/tmp/")));
        assert!(!events
            .iter()
            .any(|event| event.message.contains("secret_ref:env:TOKEN")));

        let (state, cancelled) = registry.cancel(&job_id).unwrap();
        assert_eq!(state, BuildDeployJobState::Cancelled);
        assert!(cancelled);
        registry.complete_error(
            &job_id,
            BuildDeployJobState::Failed,
            "late failure".to_string(),
        );
        let status = registry.status(&job_id).unwrap();
        assert_eq!(status.state, BuildDeployJobState::Cancelled);
        Ok(())
    }

    #[test]
    fn build_deploy_job_registry_reuses_project_idempotency_key() -> anyhow::Result<()> {
        let registry = BuildDeployJobRegistry::default();
        let mut request = valid_build_deploy_request();
        request.idempotency_key = Some("web-retry-001".to_string());
        let first = registry.create_job(&request, &HostAccessIdentity::root())?;
        let second = registry.create_job(&request, &HostAccessIdentity::root())?;
        assert!(first.created);
        assert!(!second.created);
        assert_eq!(first.job_id, second.job_id);
        let mut changed = request.clone();
        changed.build_id = Some("build-002".to_string());
        assert!(registry
            .create_job(&changed, &HostAccessIdentity::root())
            .unwrap_err()
            .to_string()
            .contains("different build-deploy request"));
        Ok(())
    }

    fn successful_build_result() -> HostBuildDeployResponse {
        HostBuildDeployResponse {
            route_id: "route-build".to_string(),
            public_url: "/p/route-build/".to_string(),
            route_access: ProxyRouteAccess::HostAuthenticated,
            port_lease_id: "port-lease-000001".to_string(),
            container_id: "container-000001".to_string(),
            container_name: Some("plurora-build-deploy-route-build-3000".to_string()),
            image: "plurora/build:build-001".to_string(),
            build_id: "build-001".to_string(),
            source_commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
            build_descriptor_hash: "descriptor-hash".to_string(),
            strategy: "dockerfile".to_string(),
            runtime_env: Vec::new(),
            runtime_mounts: Vec::new(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn deployment_revision_persists_only_replay_safe_runtime_inputs() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let source = tmp.path().join("private-data");
        std::fs::create_dir(&source)?;
        let mut request = valid_build_deploy_request();
        request.runtime_env = vec![
            RuntimeEnvSpec {
                name: "DATABASE_URL".to_string(),
                value: None,
                secret_ref: Some("secret_ref:project:database-url".to_string()),
            },
            RuntimeEnvSpec {
                name: "ONE_TIME_TOKEN".to_string(),
                value: Some("must-not-be-journaled".to_string()),
                secret_ref: None,
            },
        ];
        request.runtime_mounts = vec![approved_ro_mount(&source, "/data/private")];
        let revision =
            deployment_revision_from_build(&request, &successful_build_result(), "bdj-test", None);
        assert!(!revision.recoverable);
        assert_eq!(revision.runtime_env.len(), 1);
        let json = serde_json::to_string(&revision)?;
        assert!(json.contains("secret_ref:project:database-url"));
        assert!(!json.contains("must-not-be-journaled"));
        assert!(!json.contains(&source.to_string_lossy().to_string()));
        Ok(())
    }

    #[test]
    fn legacy_deployment_revision_defaults_to_local_git_source() -> anyhow::Result<()> {
        let request = valid_build_deploy_request();
        let revision =
            deployment_revision_from_build(&request, &successful_build_result(), "bdj-test", None);
        let mut value = serde_json::to_value(revision)?;
        let object = value
            .as_object_mut()
            .expect("revision serializes as object");
        for field in [
            "target_id",
            "source_kind",
            "verified_change_set_id",
            "verification_ref",
            "build_context_ref",
            "preview_ref",
            "approval_ref",
            "verified_build_network_mode",
            "target_deployment",
        ] {
            object.remove(field);
        }

        let restored: DeploymentRevision = serde_json::from_value(value)?;
        assert_eq!(restored.target_id, "local");
        assert_eq!(restored.source_kind, DeploymentSourceKind::GitClone);
        assert!(restored.target_deployment.is_none());
        Ok(())
    }

    #[test]
    fn deployment_projection_retains_a_bounded_recent_revision_window() {
        let registry = BuildDeployJobRegistry::default();
        let request = valid_build_deploy_request();
        let base =
            deployment_revision_from_build(&request, &successful_build_result(), "bdj-test", None);
        let total = BUILD_DEPLOY_MAX_REVISIONS_PER_PROJECT + 5;
        for index in 0..total {
            let mut revision = base.clone();
            revision.revision_id = format!("revision-{index:03}");
            revision.created_at_ms = index as u128;
            registry.register_revision(revision);
        }

        let revisions = registry.revisions(&request.project_id);
        assert_eq!(revisions.len(), BUILD_DEPLOY_MAX_REVISIONS_PER_PROJECT);
        assert_eq!(
            revisions.first().unwrap().revision_id,
            format!("revision-{:03}", total - 1)
        );
        assert_eq!(
            registry
                .active_revision(&request.project_id)
                .unwrap()
                .revision_id,
            format!("revision-{:03}", total - 1)
        );
    }

    #[test]
    fn replay_revision_can_reactivate_history_without_an_active_pointer() {
        let request = valid_build_deploy_request();
        let target =
            deployment_revision_from_build(&request, &successful_build_result(), "bdj-test", None);
        let replay = deployment_revision_from_replay(
            None,
            &target,
            DeploymentOperation::Rollback,
            successful_build_result(),
        );
        assert_eq!(replay.parent_revision_id, None);
        assert_eq!(replay.operation, DeploymentOperation::Rollback);
    }

    #[test]
    fn deployment_activation_rejects_a_stale_parent_revision() {
        let registry = BuildDeployJobRegistry::default();
        let request = valid_build_deploy_request();
        let base =
            deployment_revision_from_build(&request, &successful_build_result(), "bdj-base", None);
        registry.register_revision(base.clone());

        let mut current = base.clone();
        current.revision_id = "revision-current".to_string();
        current.parent_revision_id = Some(base.revision_id.clone());
        assert!(registry.ensure_revision_parent(&current).is_ok());
        registry.register_revision(current);

        let mut stale = base.clone();
        stale.revision_id = "revision-stale".to_string();
        stale.parent_revision_id = Some(base.revision_id);
        assert!(registry.ensure_revision_parent(&stale).is_err());
    }

    #[test]
    fn deployment_authority_lease_expires_before_new_effects() {
        let mut authority = DeploymentAuthorityLease::from_identity(
            "deployment-expired".to_string(),
            "local",
            &HostAccessIdentity::root(),
        );
        authority.expires_at_ms = Some(chrono::Utc::now().timestamp_millis() - 1);
        assert!(authority
            .validate(
                &ProjectId::new("project-expired").unwrap(),
                &HostAccessRegistry::default()
            )
            .is_err());
    }

    #[tokio::test]
    async fn deployment_journal_hydrates_and_releases_direct_route_ownership() -> anyhow::Result<()>
    {
        let store = Arc::new(InMemoryEventStore::default());
        let ownership = DeploymentDirectRouteOwned {
            route_id: "route-direct".to_string(),
            project_id: ProjectId::new("project-direct")?,
            port_name: "web".to_string(),
            route_access: ProxyRouteAccess::HostAuthenticated,
            port_lease_id: "port-lease-direct".to_string(),
            container_id: "container-direct".to_string(),
            timestamp_ms: now_millis(),
            authority: None,
        };
        assert!(append_deployment_journal_event(
            store.as_ref(),
            0,
            DEPLOYMENT_DIRECT_ROUTE_OWNED_EVENT,
            &ownership,
        )
        .await?
        .is_some());

        let registry = Arc::new(BuildDeployJobRegistry::default());
        assert_eq!(
            hydrate_deployment_control_plane(store.clone(), registry.clone()).await?,
            1
        );
        assert_eq!(
            registry.project_for_route("route-direct"),
            Some(ProjectId::new("project-direct")?)
        );
        assert_eq!(
            registry.durable_routes()[0].port_lease_id,
            "port-lease-direct"
        );

        let release = DeploymentDirectRouteReleased {
            route_id: ownership.route_id,
            project_id: ownership.project_id,
            timestamp_ms: now_millis(),
        };
        assert!(append_deployment_journal_event(
            store.as_ref(),
            1,
            DEPLOYMENT_DIRECT_ROUTE_RELEASED_EVENT,
            &release,
        )
        .await?
        .is_some());
        assert_eq!(
            sync_deployment_journal(store.as_ref(), registry.as_ref()).await?,
            1
        );
        assert!(registry.project_for_route("route-direct").is_none());
        Ok(())
    }

    #[tokio::test]
    async fn deployment_journal_hydrates_revisions_and_interrupts_incomplete_jobs(
    ) -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let source_registry = Arc::new(BuildDeployJobRegistry::default());
        let request = valid_build_deploy_request();
        let created = source_registry.create_job(&request, &HostAccessIdentity::root())?;
        let snapshot = source_registry.job_snapshot(&created.job_id).unwrap();
        assert!(append_deployment_journal_event(
            store.as_ref(),
            0,
            DEPLOYMENT_JOB_SNAPSHOT_EVENT,
            &snapshot,
        )
        .await?
        .is_some());
        let revision = deployment_revision_from_build(
            &request,
            &successful_build_result(),
            &created.job_id,
            None,
        );
        assert!(append_deployment_journal_event(
            store.as_ref(),
            1,
            DEPLOYMENT_REVISION_ACTIVATED_EVENT,
            &DeploymentRevisionActivated {
                revision: revision.clone(),
                enforce_parent: false,
                job: None,
                authority: None,
            },
        )
        .await?
        .is_some());

        let hydrated = Arc::new(BuildDeployJobRegistry::default());
        assert_eq!(
            hydrate_deployment_control_plane(store.clone(), hydrated.clone()).await?,
            2
        );
        assert_eq!(
            hydrated
                .active_revision(&request.project_id)
                .unwrap()
                .revision_id,
            revision.revision_id
        );
        let status = hydrated.status(&created.job_id).unwrap();
        assert_eq!(status.state, BuildDeployJobState::Failed);
        assert!(status.error.unwrap().contains("host restarted"));
        assert_eq!(
            store
                .list_kind_prefix(DEPLOYMENT_JOURNAL_PREFIX)
                .await?
                .len(),
            3
        );
        Ok(())
    }

    #[test]
    fn build_deploy_job_registry_enforces_project_concurrency() -> anyhow::Result<()> {
        let registry = BuildDeployJobRegistry::default();
        let request = valid_build_deploy_request();
        registry
            .project_active
            .lock()
            .unwrap()
            .insert(request.project_id.clone());
        assert!(registry
            .create_job(&request, &HostAccessIdentity::root())
            .is_err());
        Ok(())
    }

    #[test]
    fn build_image_gc_policy_requires_plurora_project_and_build_labels() {
        let labels = HashMap::from([
            ("managed-by".to_string(), "plurora".to_string()),
            ("plurora.project_id".to_string(), "project-1".to_string()),
            ("plurora.build_id".to_string(), "build-1".to_string()),
        ]);
        assert!(should_remove_plurora_build_image(
            &labels,
            "project-1",
            "build-1"
        ));
        assert!(!should_remove_plurora_build_image(
            &labels,
            "project-2",
            "build-1"
        ));
        assert!(!should_remove_plurora_build_image(
            &HashMap::new(),
            "project-1",
            "build-1"
        ));
    }

    #[test]
    fn build_deploy_prefers_content_addressable_image_id() -> anyhow::Result<()> {
        let image = require_built_image(&serde_json::json!({
            "docker_performed": true,
            "image_built": true,
            "image": "plurora/project:mutable",
            "image_id": "sha256:0123456789abcdef",
        }))?;
        assert_eq!(image, "sha256:0123456789abcdef");
        Ok(())
    }

    #[test]
    fn build_deploy_descriptor_hash_is_deterministic_and_sensitive() {
        let request = valid_build_deploy_request();
        let hash1 = build_deploy_descriptor_hash(
            &request,
            "build-001",
            "0123456789abcdef0123456789abcdef01234567",
        );
        let hash2 = build_deploy_descriptor_hash(
            &request,
            "build-001",
            "0123456789abcdef0123456789abcdef01234567",
        );
        assert_eq!(hash1, hash2);
        assert!(hash1.starts_with("sha256:"));
        assert_eq!(hash1.len(), "sha256:".len() + 64);

        let changed = build_deploy_descriptor_hash(
            &request,
            "build-002",
            "0123456789abcdef0123456789abcdef01234567",
        );
        assert_ne!(hash1, changed);
    }

    #[test]
    fn build_deploy_build_id_generation_is_safe() {
        assert_eq!(
            generated_build_id("0123456789abcdef0123456789abcdef01234567"),
            "build-0123456789ab"
        );
        validate_build_id(&generated_build_id(
            "0123456789abcdef0123456789abcdef01234567",
        ))
        .unwrap();
    }

    #[test]
    fn build_deploy_public_error_redacts_host_paths() {
        let response = redacted_build_deploy_error(anyhow::anyhow!(
            "failed to read /tmp/plurora-secret-workspace/Dockerfile"
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn health_transition_decision_respects_thresholds() {
        let mut counters = HealthCounters::default();
        assert!(
            decide_health_transition(true, &mut counters, HealthProbeResult::Failure).is_none()
        );
        assert!(
            decide_health_transition(true, &mut counters, HealthProbeResult::Failure).is_none()
        );
        assert_eq!(
            decide_health_transition(true, &mut counters, HealthProbeResult::Failure),
            Some(HealthTransition {
                ready: false,
                reason: "tcp_probe_failed"
            })
        );
        assert_eq!(counters.consecutive_failures, HEALTH_FAILURE_THRESHOLD);
        assert_eq!(counters.consecutive_successes, 0);

        assert!(
            decide_health_transition(false, &mut counters, HealthProbeResult::Success).is_none()
        );
        assert_eq!(counters.consecutive_failures, 0);
        assert_eq!(counters.consecutive_successes, 1);
        assert_eq!(
            decide_health_transition(false, &mut counters, HealthProbeResult::Success),
            Some(HealthTransition {
                ready: true,
                reason: "recovered"
            })
        );
        assert_eq!(counters.consecutive_successes, HEALTH_RECOVERY_THRESHOLD);
    }

    #[tokio::test]
    async fn rpc_accepts_only_current_method_ids() -> anyhow::Result<()> {
        async fn call_rpc(method: &str) -> anyhow::Result<serde_json::Value> {
            let response = app()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/rpc")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            json!({"id": "method-test", "method": method, "params": {}})
                                .to_string(),
                        ))?,
                )
                .await?;
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = to_bytes(response.into_body(), usize::MAX).await?;
            Ok(serde_json::from_slice(&bytes)?)
        }

        let current = call_rpc("host.info").await?;
        assert!(current["result"]["supported_transports"].is_array());
        assert!(current.get("diagnostics").is_none());
        assert!(current["result"].get("aliases").is_none());

        let removed = call_rpc("platform.host.info").await?;
        assert_eq!(removed["error"]["code"], "runtime/error/invalid_request");
        assert!(removed.get("diagnostics").is_none());
        Ok(())
    }

    #[tokio::test]
    async fn removed_ad_hoc_contract_routes_are_not_exposed() -> anyhow::Result<()> {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/unknown-contract-route")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn malformed_contract_has_no_compatibility_diagnostic() -> anyhow::Result<()> {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rpc")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "id": "contract-error",
                            "method": "host.info",
                            "contract": "bad",
                            "params": {}
                        })
                        .to_string(),
                    ))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(value["id"], "contract-error");
        assert_eq!(value["error"]["code"], "runtime/error/invalid_request");
        assert!(value.get("diagnostics").is_none());
        Ok(())
    }

    #[tokio::test]
    async fn rpc_rejects_unsupported_contract_without_downgrade() -> anyhow::Result<()> {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rpc")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "id": "contract-1",
                            "method": "host.info",
                            "params": {},
                            "contract": {
                                "profile": plurora_runtime::DEFAULT_CONTRACT_PROFILE,
                                "versions": [{"layer": "host", "version": "999.0.0"}]
                            }
                        })
                        .to_string(),
                    ))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert!(value["result"].is_null());
        assert_eq!(
            value["error"]["code"],
            "protocol/error/unsupported_contract"
        );
        assert_eq!(value["error"]["details"]["reason"], "unsupported_version");
        Ok(())
    }

    #[tokio::test]
    async fn rpc_returns_structured_error() -> anyhow::Result<()> {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rpc")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"id": "1", "method": "journal.list", "params": {}}).to_string(),
                    ))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(value["error"]["code"], "runtime/error/internal");
        Ok(())
    }

    #[tokio::test]
    async fn healthz_returns_ok() -> anyhow::Result<()> {
        let response = app()
            .oneshot(Request::builder().uri("/healthz").body(Body::empty())?)
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn readyz_reports_component_readiness_without_resource_ids() -> anyhow::Result<()> {
        let response = app()
            .oneshot(Request::builder().uri("/readyz").body(Body::empty())?)
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        let value: Value = serde_json::from_slice(&body)?;
        assert_eq!(value["status"], "ready");
        assert_eq!(value["ready"], true);
        assert_eq!(value["components"]["event_store"]["status"], "ok");
        assert_eq!(value["components"]["control_plane_lease"]["status"], "ok");
        assert_eq!(value["components"]["deployments"]["durable"], 0);
        assert!(!String::from_utf8(body.to_vec())?.contains("route_id"));
        Ok(())
    }

    #[tokio::test]
    async fn readyz_is_unready_after_control_plane_lease_loss() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let development = development_registry();
        let lease = acquire_development_host_lease(store.clone(), development.clone()).await?;
        release_development_host_lease(store.clone(), &lease).await?;
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let response = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: None,
            app_base_domain: None,
            build_jobs: build_deploy_job_registry(),
            development,
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        })
        .oneshot(Request::builder().uri("/readyz").body(Body::empty())?)
        .await?;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        let value: Value = serde_json::from_slice(&body)?;
        assert_eq!(value["status"], "unready");
        assert_eq!(value["ready"], false);
        assert_eq!(
            value["components"]["control_plane_lease"]["status"],
            "failed"
        );
        Ok(())
    }

    #[tokio::test]
    async fn readyz_keeps_host_ready_when_a_workload_is_degraded() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let build_jobs = build_deploy_job_registry();
        build_jobs.register_direct_route_owner(DeploymentDirectRouteOwned {
            route_id: "private-route".to_string(),
            project_id: ProjectId::new("project-readyz")?,
            port_name: "http".to_string(),
            route_access: ProxyRouteAccess::HostAuthenticated,
            port_lease_id: "private-lease".to_string(),
            container_id: "private-container".to_string(),
            timestamp_ms: 1,
            authority: None,
        });
        let response = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: None,
            app_base_domain: None,
            build_jobs,
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        })
        .oneshot(Request::builder().uri("/readyz").body(Body::empty())?)
        .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        let value: Value = serde_json::from_slice(&body)?;
        assert_eq!(value["status"], "degraded");
        assert_eq!(value["ready"], true);
        assert_eq!(value["components"]["deployments"]["durable"], 1);
        assert_eq!(value["components"]["deployments"]["degraded"], 1);
        assert!(!String::from_utf8(body.to_vec())?.contains("private-route"));
        Ok(())
    }

    #[tokio::test]
    async fn serves_static_files_when_configured() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        std::fs::write(dir.path().join("index.html"), "<main>Ygg web</main>")?;
        std::fs::write(
            dir.path().join("sw.js"),
            "self.addEventListener('fetch',()=>{});",
        )?;
        std::fs::write(dir.path().join("manifest.webmanifest"), "{}")?;
        std::fs::create_dir_all(dir.path().join("assets"))?;
        std::fs::write(dir.path().join("assets/app.js"), "console.log('plurora');")?;

        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let app = app_with_state(AppState {
            runtime,
            static_dir: Some(dir.path().to_path_buf()),
            access_token: None,
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/assets/app.js")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/javascript"
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=31536000, immutable"
        );
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        assert_eq!(&bytes[..], b"console.log('plurora');");

        let project_route = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/project/demo-project")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(project_route.status(), StatusCode::OK);
        assert_eq!(
            project_route.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-cache"
        );

        let pairing_route = app
            .clone()
            .oneshot(Request::builder().uri("/pair").body(Body::empty())?)
            .await?;
        assert_eq!(pairing_route.status(), StatusCode::OK);
        assert_eq!(
            pairing_route.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-cache"
        );

        let manifest = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/manifest.webmanifest")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(manifest.status(), StatusCode::OK);
        assert_eq!(
            manifest.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/manifest+json"
        );
        assert_eq!(
            manifest.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-cache"
        );

        let random_route = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/unknown-route")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(random_route.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn token_gate_protects_rpc_but_not_healthz_or_static() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        std::fs::write(dir.path().join("index.html"), "public")?;
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let app = app_with_state(AppState {
            runtime,
            static_dir: Some(dir.path().to_path_buf()),
            access_token: Some("secret-token".to_string()),
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let health = app
            .clone()
            .oneshot(Request::builder().uri("/healthz").body(Body::empty())?)
            .await?;
        assert_eq!(health.status(), StatusCode::OK);

        let static_response = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty())?)
            .await?;
        assert_eq!(static_response.status(), StatusCode::OK);

        let denied = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rpc")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"id":"1","method":"host.info","params":{}}).to_string(),
                    ))?,
            )
            .await?;
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

        let preflight = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/rpc")
                    .header(header::ORIGIN, "https://client.example")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                    .header(
                        header::ACCESS_CONTROL_REQUEST_HEADERS,
                        "authorization,content-type",
                    )
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(preflight.status(), StatusCode::OK);
        assert_eq!(
            preflight.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            Some(&HeaderValue::from_static("*"))
        );
        assert!(preflight
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .is_none());

        let allowed = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rpc")
                    .header("content-type", "application/json")
                    .header(header::ORIGIN, "https://client.example")
                    .header(header::AUTHORIZATION, "Bearer secret-token")
                    .body(Body::from(
                        json!({"id":"1","method":"host.info","params":{}}).to_string(),
                    ))?,
            )
            .await?;
        assert_eq!(allowed.status(), StatusCode::OK);
        assert_eq!(
            allowed.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            Some(&HeaderValue::from_static("*"))
        );

        let access_identity = app
            .oneshot(
                Request::builder()
                    .uri("/host/v1/access/me")
                    .header(header::AUTHORIZATION, "Bearer secret-token")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(access_identity.status(), StatusCode::OK);
        assert_eq!(
            access_identity
                .headers()
                .get(header::CACHE_CONTROL)
                .unwrap(),
            "no-store"
        );
        assert_eq!(
            access_identity
                .headers()
                .get(header::REFERRER_POLICY)
                .unwrap(),
            "no-referrer"
        );
        assert_eq!(
            access_identity
                .headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .unwrap(),
            "nosniff"
        );
        Ok(())
    }

    #[tokio::test]
    async fn device_authority_is_resource_exact_across_http_and_rpc() -> anyhow::Result<()> {
        fn project(id: &str, title: &str) -> ProjectDescriptor {
            ProjectDescriptor {
                schema_version: 1,
                project: ProjectInner {
                    id: ProjectId::new(id).expect("valid project id"),
                    title: title.to_string(),
                    description: String::new(),
                    project_type: ProjectType::PluroraNative,
                    icon: None,
                    entry_surface_id: Some("packages/test/main".to_string()),
                    packages: vec!["packages/test/manifest.yaml".to_string()],
                    optional_packages: Vec::new(),
                    required_surfaces: Vec::new(),
                    required_capabilities: Vec::new(),
                    secret_policy: SecretPolicy::default(),
                    external: None,
                    metadata: BTreeMap::new(),
                },
            }
        }

        async fn rpc(
            app: Router,
            token: &str,
            method: &str,
            params: Value,
        ) -> anyhow::Result<Value> {
            let response = app
                .oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri("/rpc")
                        .header(header::CONTENT_TYPE, "application/json")
                        .header(header::AUTHORIZATION, format!("Bearer {token}"))
                        .body(Body::from(
                            json!({"id": "authority-test", "method": method, "params": params})
                                .to_string(),
                        ))?,
                )
                .await?;
            assert_eq!(response.status(), StatusCode::OK);
            Ok(serde_json::from_slice(
                &to_bytes(response.into_body(), usize::MAX).await?,
            )?)
        }

        let project_a = "authority_project_a__abc12345";
        let project_b = "authority_project_b__abc12345";
        let projects = Arc::new(ProjectRegistry::new());
        projects.register(project(project_a, "Project A"))?;
        projects.register(project(project_b, "Project B"))?;
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(
            store.clone(),
            RuntimeConfig {
                project_registry: projects,
                ..RuntimeConfig::default()
            },
        ));
        let access_registry = Arc::new(HostAccessRegistry::default());
        let app = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: Some("root-authority-token".to_string()),
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: access_registry.clone(),
            target_agents: target_agent_registry(),
        });

        let unauthenticated_surface = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/surface-bundles/projects/{project_a}/bundle.mjs"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(unauthenticated_surface.status(), StatusCode::UNAUTHORIZED);

        let pairing = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/host/v1/access/pairings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, "Bearer root-authority-token")
                    .body(Body::from(
                        json!({
                            "device_name": "Project A device",
                            "scopes": ["observe", "project_operate", "deploy", "develop_propose", "access_manage"],
                            "resources": [
                                {"kind": "project", "id": project_a},
                                {"kind": "target", "id": "local"}
                            ],
                            "pairing_ttl_secs": 60,
                            "grant_ttl_secs": 7200
                        })
                        .to_string(),
                    ))?,
            )
            .await?;
        assert_eq!(pairing.status(), StatusCode::CREATED);
        let pairing_body: Value =
            serde_json::from_slice(&to_bytes(pairing.into_body(), usize::MAX).await?)?;
        let pairing_token = pairing_body["pairing_token"]
            .as_str()
            .expect("pairing token returned");

        let claim = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/host/v1/access/pair")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({"pairing_token": pairing_token}).to_string(),
                    ))?,
            )
            .await?;
        assert_eq!(claim.status(), StatusCode::CREATED);
        let cookie = claim
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .expect("pairing claim returns a device cookie");
        let access_token = cookie
            .strip_prefix(&format!("{}=", host_access::REMOTE_HOST_SESSION_COOKIE))
            .and_then(|value| value.split(';').next())
            .expect("device access token is encoded in the cookie")
            .to_string();

        let unauthenticated_operations = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/host/v1/targets/local/operations")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(
            unauthenticated_operations.status(),
            StatusCode::UNAUTHORIZED
        );

        let visible_operations = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/host/v1/targets/local/operations")
                    .header(header::ORIGIN, "https://client.example")
                    .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(visible_operations.status(), StatusCode::OK);
        assert_eq!(
            visible_operations
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            Some(&HeaderValue::from_static("*"))
        );
        let visible_operations_body = to_bytes(visible_operations.into_body(), usize::MAX).await?;
        assert_eq!(
            serde_json::from_slice::<Value>(&visible_operations_body)?,
            json!([])
        );

        let denied_target_operations = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/host/v1/targets/other/operations")
                    .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(denied_target_operations.status(), StatusCode::FORBIDDEN);

        let denied_project_operation = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/host/v1/targets/local/operations")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                    .body(Body::from(
                        json!({"project_id": project_b, "spec": {"kind": "health_probe"}})
                            .to_string(),
                    ))?,
            )
            .await?;
        assert_eq!(denied_project_operation.status(), StatusCode::FORBIDDEN);

        let unknown_operation = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/host/v1/targets/local/operations/unknown")
                    .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(unknown_operation.status(), StatusCode::NOT_FOUND);

        let overbroad_delegation = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/host/v1/access/pairings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                    .body(Body::from(
                        json!({
                            "device_name": "Project B child",
                            "scopes": ["observe"],
                            "resources": [{"kind": "project", "id": project_b}],
                            "pairing_ttl_secs": 60,
                            "grant_ttl_secs": 3600
                        })
                        .to_string(),
                    ))?,
            )
            .await?;
        assert_eq!(overbroad_delegation.status(), StatusCode::FORBIDDEN);

        let attenuated_delegation = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/host/v1/access/pairings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                    .body(Body::from(
                        json!({
                            "device_name": "Project A child",
                            "scopes": ["observe"],
                            "resources": [{"kind": "project", "id": project_a}],
                            "pairing_ttl_secs": 60,
                            "grant_ttl_secs": 3600
                        })
                        .to_string(),
                    ))?,
            )
            .await?;
        assert_eq!(attenuated_delegation.status(), StatusCode::CREATED);

        for path in [
            format!("/surface-bundles/projects/{project_b}/bundle.mjs"),
            "/surface-bundles/ydltavern/bundle.mjs".to_string(),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::GET)
                        .uri(path)
                        .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                        .body(Body::empty())?,
                )
                .await?;
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "project-scoped device must not read another project or Host-global catalogue"
            );
        }

        for method in ["host.package.list", "capability.discover"] {
            let denied_global = rpc(app.clone(), &access_token, method, json!({})).await?;
            assert_eq!(
                denied_global["error"]["code"], "runtime/error/permission_denied",
                "project-scoped device must not enumerate a Host-global catalogue"
            );
        }

        let listed = rpc(app.clone(), &access_token, "host.project.list", json!({})).await?;
        let visible = listed["result"]["projects"]
            .as_array()
            .expect("project list response");
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0]["id"], project_a);

        let denied_rpc = rpc(
            app.clone(),
            &access_token,
            "host.project.get",
            json!({"project_id": project_b}),
        )
        .await?;
        assert_eq!(
            denied_rpc["error"]["code"],
            "runtime/error/permission_denied"
        );
        let authority_events = store
            .list_session_range(&"host_control_authority".to_string(), None, None)
            .await?;
        assert!(authority_events.iter().any(|event| {
            event.kind == "host/control/v1/authority.decision"
                && event.payload["method"] == "host.project.get"
                && event.payload["decision"] == "deny"
                && event.payload["operation_resources"][0]["id"] == project_b
        }));
        assert!(!serde_json::to_string(&authority_events)?.contains(&access_token));

        let denied_path = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/host/v1/projects/{project_b}/changes"))
                    .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(denied_path.status(), StatusCode::FORBIDDEN);

        let device_identity = host_access::authenticate_host_access_token(
            store.as_ref(),
            access_registry.as_ref(),
            &access_token,
        )
        .await?
        .expect("device remains authenticated before revocation");
        let grant_id = device_identity
            .grant_id
            .clone()
            .expect("device identity has a grant id");
        let mut resolved = json!({
            "bundle_url": format!("/surface-bundles/projects/{project_a}/bundle.mjs"),
            "stylesheets": []
        });
        mint_surface_asset_lease(&mut resolved, &device_identity, access_registry.as_ref())?;
        let leased_url = resolved["bundle_url"].as_str().expect("leased URL");
        let lease_id = leased_url
            .trim_start_matches('/')
            .split('/')
            .nth(1)
            .expect("lease id");
        let lease_root = format!("projects/{project_a}");
        assert!(surface_asset_lease_is_valid(
            access_registry.as_ref(),
            lease_id,
            &lease_root
        ));

        let revoked = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/host/v1/access/grants/{grant_id}/revoke"))
                    .header(header::AUTHORIZATION, "Bearer root-authority-token")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(revoked.status(), StatusCode::OK);
        assert!(
            !surface_asset_lease_is_valid(access_registry.as_ref(), lease_id, &lease_root),
            "revoking a device grant must invalidate its outstanding asset leases"
        );
        Ok(())
    }

    #[tokio::test]
    async fn desktop_bootstrap_nonce_is_one_time_and_issues_http_only_session() -> anyhow::Result<()>
    {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let app = app_with_state_and_bootstrap_token(
            AppState {
                runtime,
                static_dir: None,
                access_token: Some("long-lived-host-token".to_string()),
                app_base_domain: None,
                build_jobs: Arc::new(BuildDeployJobRegistry::default()),
                development: development_registry(),
                host_access: host_access_registry(),
                target_agents: target_agent_registry(),
            },
            Some("single-use-bootstrap-nonce".to_string()),
        );

        let bootstrap = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/host/bootstrap?nonce=single-use-bootstrap-nonce")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(bootstrap.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            bootstrap.headers().get(header::LOCATION).unwrap(),
            "/?plurora_platform=desktop"
        );
        assert_eq!(
            bootstrap.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        assert_eq!(
            bootstrap.headers().get(header::REFERRER_POLICY).unwrap(),
            "no-referrer"
        );
        let set_cookie = bootstrap
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()?;
        assert!(set_cookie.starts_with("plurora_host_session="));
        assert!(set_cookie.contains("HttpOnly"));
        assert!(set_cookie.contains("SameSite=Strict"));
        assert!(!set_cookie.contains("long-lived-host-token"));
        let cookie = set_cookie.split(';').next().unwrap();

        let authenticated = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/rpc")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, cookie)
                    .body(Body::from(
                        json!({"id": "desktop-cookie", "method": "host.info", "params": {}})
                            .to_string(),
                    ))?,
            )
            .await?;
        assert_eq!(authenticated.status(), StatusCode::OK);
        let bytes = to_bytes(authenticated.into_body(), usize::MAX).await?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(value["result"]["protocol_version"], "0.1.0");

        let replay = app
            .oneshot(
                Request::builder()
                    .uri("/host/bootstrap?nonce=single-use-bootstrap-nonce")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[test]
    fn host_session_cookie_is_removed_before_proxy_or_rpc_handlers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static(
                "project_session=keep; plurora_host_session=remove; __Host-plurora_remote_session=remove-too; theme=warm",
            ),
        );
        strip_host_session_cookie(&mut headers);
        assert_eq!(
            headers.get(header::COOKIE).unwrap(),
            "project_session=keep; theme=warm"
        );
    }

    #[tokio::test]
    async fn token_gate_protects_host_deploy() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let app = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: Some("deploy-token".to_string()),
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let denied = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/host/v1/deploy")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "image": "example/app:latest",
                            "container_port": 8080,
                            "port_name": "web",
                            "route_id": "route-1",
                            "pull_if_missing": false,
                        })
                        .to_string(),
                    ))?,
            )
            .await?;
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn host_deploy_rolls_back_port_lease_when_docker_start_fails() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let app = app_with_state(AppState {
            runtime: runtime.clone(),
            static_dir: None,
            access_token: None,
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/host/v1/deploy")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "image": "example/app:latest",
                            "container_port": 8080,
                            "port_name": "web",
                            "route_id": "rollback-route",
                            "pull_if_missing": false,
                        })
                        .to_string(),
                    ))?,
            )
            .await?;
        assert!(!response.status().is_success());

        let context = ProtocolContext::host_dev("host_deploy_test");
        let leases = runtime
            .call_protocol(&context, "host.port.list", json!({}))
            .await
            .map_err(protocol_error_to_anyhow)?;
        assert!(leases
            .as_array()
            .expect("port.list returns array")
            .iter()
            .all(|lease| lease["status"] != "active"));

        let routes = runtime
            .call_protocol(&context, "host.proxy.list", json!({}))
            .await
            .map_err(protocol_error_to_anyhow)?;
        assert!(routes
            .as_array()
            .expect("proxy.list returns array")
            .iter()
            .all(|route| route["status"] != "active"));
        Ok(())
    }

    #[tokio::test]
    async fn token_gate_accepts_query_token_for_sse() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let app = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: Some("event-token".to_string()),
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let denied = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/journal/subscribe/session-1")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

        let allowed = app
            .oneshot(
                Request::builder()
                    .uri("/journal/subscribe/session-1?access_token=event-token")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(allowed.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn proxy_route_is_token_protected() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let app = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: Some("proxy-token".to_string()),
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let denied = app
            .oneshot(
                Request::builder()
                    .uri("/p/missing/app")
                    .body(Body::empty())?,
            )
            .await?;

        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn proxy_route_not_found_returns_404() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let app = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: Some("proxy-token".to_string()),
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/p/missing/app")
                    .header(header::AUTHORIZATION, "Bearer proxy-token")
                    .body(Body::empty())?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn tunnel_loopback_bridge_requires_and_strips_its_one_time_credential(
    ) -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let accepted = tokio::spawn(async move {
            let (mut server, _) = listener.accept().await?;
            authenticate_tunnel_bridge_request(&mut server, "bridge-token").await
        });
        let mut client = TcpStream::connect(address).await?;
        client
            .write_all(
                b"POST /preview HTTP/1.1\r\nhost: 127.0.0.1\r\nx-plurora-tunnel-bridge: bridge-token\r\ncontent-length: 4\r\n\r\nbody",
            )
            .await?;
        let forwarded = accepted.await??;
        let forwarded = String::from_utf8(forwarded)?;
        assert_eq!(
            forwarded,
            "POST /preview HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-length: 4\r\n\r\nbody"
        );
        Ok(())
    }

    #[tokio::test]
    async fn proxy_route_requires_ready_before_forwarding() -> anyhow::Result<()> {
        let upstream = Router::new().fallback(any(|| async { "ready upstream" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let upstream_addr: SocketAddr = listener.local_addr()?;
        tokio::spawn(async move {
            axum::serve(listener, upstream)
                .await
                .expect("test upstream serves");
        });

        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let lease = runtime
            .config()
            .port_lease_registry
            .lease(PortLeaseRequest {
                target_id: ExecutionTargetId::from("local"),
                port_name: "web".to_string(),
                protocol: PortProtocol::Tcp,
                requested_port: Some(upstream_addr.port()),
            })
            .await;
        runtime
            .config()
            .proxy_route_registry
            .register(ProxyRouteRegisterRequest {
                route_id: Some("readiness-route".to_string()),
                upstream: ProxyRouteUpstream {
                    port_lease_id: lease.lease.id.clone(),
                    port_name: "web".to_string(),
                },
                protocol: ProxyProtocol::Http,
                access: ProxyRouteAccess::HostAuthenticated,
            })
            .await;
        let app = app_with_state(AppState {
            runtime: runtime.clone(),
            static_dir: None,
            access_token: None,
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let not_ready = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/p/readiness-route/app")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(not_ready.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = to_bytes(not_ready.into_body(), usize::MAX).await?;
        assert_eq!(&body[..], b"deployment not ready");

        runtime
            .config()
            .proxy_route_registry
            .set_ready("readiness-route", true)
            .await;

        let ready = app
            .oneshot(
                Request::builder()
                    .uri("/p/readiness-route/app")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(ready.status(), StatusCode::OK);
        let body = to_bytes(ready.into_body(), usize::MAX).await?;
        assert_eq!(&body[..], b"ready upstream");
        Ok(())
    }

    #[derive(Debug, Default)]
    struct ObservedProxyRequest {
        method: String,
        path: String,
        query: Option<String>,
        host: Option<String>,
        authorization: Option<String>,
        body: Vec<u8>,
    }

    #[tokio::test]
    async fn proxy_forwards_request_and_strips_plurora_credentials() -> anyhow::Result<()> {
        let observed = Arc::new(Mutex::new(None::<ObservedProxyRequest>));
        let upstream_observed = observed.clone();
        let upstream = Router::new()
            .fallback(any(
                move |State(observed): State<Arc<Mutex<Option<ObservedProxyRequest>>>>,
                      OriginalUri(uri): OriginalUri,
                      request: axum::extract::Request| async move {
                    let method = request.method().to_string();
                    let authorization = request
                        .headers()
                        .get(header::AUTHORIZATION)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    let host = request
                        .headers()
                        .get(header::HOST)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    let body = to_bytes(request.into_body(), usize::MAX)
                        .await
                        .expect("read upstream body")
                        .to_vec();
                    *observed.lock().await = Some(ObservedProxyRequest {
                        method,
                        path: uri.path().to_string(),
                        query: uri.query().map(str::to_string),
                        host,
                        authorization,
                        body,
                    });
                    (
                        StatusCode::CREATED,
                        [("x-upstream-safe", "yes")],
                        "proxied response",
                    )
                },
            ))
            .with_state(upstream_observed);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let upstream_addr: SocketAddr = listener.local_addr()?;
        tokio::spawn(async move {
            axum::serve(listener, upstream)
                .await
                .expect("test upstream serves");
        });

        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let lease = runtime
            .config()
            .port_lease_registry
            .lease(PortLeaseRequest {
                target_id: ExecutionTargetId::from("local"),
                port_name: "web".to_string(),
                protocol: PortProtocol::Tcp,
                requested_port: Some(upstream_addr.port()),
            })
            .await;
        runtime
            .config()
            .proxy_route_registry
            .register(ProxyRouteRegisterRequest {
                route_id: Some("route-1".to_string()),
                upstream: ProxyRouteUpstream {
                    port_lease_id: lease.lease.id.clone(),
                    port_name: "web".to_string(),
                },
                protocol: ProxyProtocol::Http,
                access: ProxyRouteAccess::HostAuthenticated,
            })
            .await;
        runtime
            .config()
            .proxy_route_registry
            .set_ready("route-1", true)
            .await;
        let app = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: Some("proxy-token".to_string()),
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/p/route-1/api/widgets?keep=1&access_token=proxy-token")
                    .header(header::AUTHORIZATION, "Bearer proxy-token")
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Body::from("hello upstream"))?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response.headers().get("x-upstream-safe"),
            Some(&HeaderValue::from_static("yes"))
        );
        let response_body = to_bytes(response.into_body(), usize::MAX).await?;
        assert_eq!(&response_body[..], b"proxied response");

        let observed = observed.lock().await.take().expect("upstream request");
        assert_eq!(observed.method, "POST");
        assert_eq!(observed.path, "/api/widgets");
        assert_eq!(observed.query.as_deref(), Some("keep=1"));
        assert_eq!(observed.body, b"hello upstream");
        assert!(observed.authorization.is_none());
        assert!(observed
            .host
            .as_deref()
            .is_some_and(|host| host.starts_with("127.0.0.1:")));
        Ok(())
    }

    #[tokio::test]
    async fn vhost_proxy_owns_root_paths_and_sets_host_header() -> anyhow::Result<()> {
        let observed = Arc::new(Mutex::new(None::<ObservedProxyRequest>));
        let upstream_observed = observed.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let upstream_addr: SocketAddr = listener.local_addr()?;
        let redirect_location = format!(
            "http://127.0.0.1:{}/next?access_token=leak&keep=1",
            upstream_addr.port()
        );
        let upstream = Router::new()
            .fallback(any(
                move |State(observed): State<Arc<Mutex<Option<ObservedProxyRequest>>>>,
                      OriginalUri(uri): OriginalUri,
                      request: axum::extract::Request| {
                    let redirect_location = redirect_location.clone();
                    async move {
                        let location_header = HeaderValue::from_str(&redirect_location)
                            .expect("test redirect header is valid");
                        let method = request.method().to_string();
                        let host = request
                            .headers()
                            .get(header::HOST)
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_string);
                        let authorization = request
                            .headers()
                            .get(header::AUTHORIZATION)
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_string);
                        let body = to_bytes(request.into_body(), usize::MAX)
                            .await
                            .expect("read upstream body")
                            .to_vec();
                        *observed.lock().await = Some(ObservedProxyRequest {
                            method,
                            path: uri.path().to_string(),
                            query: uri.query().map(str::to_string),
                            host,
                            authorization,
                            body,
                        });
                        (
                            StatusCode::OK,
                            [
                                (
                                    header::SET_COOKIE,
                                    HeaderValue::from_static(
                                        "sid=abc; Domain=.apps.example.test; Path=/",
                                    ),
                                ),
                                (header::LOCATION, location_header),
                            ],
                            "vhost response",
                        )
                    }
                },
            ))
            .with_state(upstream_observed);
        tokio::spawn(async move {
            axum::serve(listener, upstream)
                .await
                .expect("test upstream serves");
        });

        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let lease = runtime
            .config()
            .port_lease_registry
            .lease(PortLeaseRequest {
                target_id: ExecutionTargetId::from("local"),
                port_name: "web".to_string(),
                protocol: PortProtocol::Tcp,
                requested_port: Some(upstream_addr.port()),
            })
            .await;
        let route_id = "My_App/Main";
        runtime
            .config()
            .proxy_route_registry
            .register(ProxyRouteRegisterRequest {
                route_id: Some(route_id.to_string()),
                upstream: ProxyRouteUpstream {
                    port_lease_id: lease.lease.id.clone(),
                    port_name: "web".to_string(),
                },
                protocol: ProxyProtocol::Http,
                access: ProxyRouteAccess::Public,
            })
            .await;
        runtime
            .config()
            .proxy_route_registry
            .set_ready(route_id, true)
            .await;
        let slug = route_slug(route_id);
        let vhost = format!("{slug}.apps.example.test");
        let app = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: Some("proxy-token".to_string()),
            app_base_domain: Some("apps.example.test".to_string()),
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rpc?keep=1")
                    .header(header::HOST, &vhost)
                    .body(Body::from("vhost body"))?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::SET_COOKIE).unwrap(),
            "sid=abc; Path=/"
        );
        assert_eq!(
            response.headers().get(header::LOCATION).unwrap(),
            &HeaderValue::from_str(&format!("https://{vhost}/next?keep=1"))?
        );
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        assert_eq!(&body[..], b"vhost response");

        let observed = observed.lock().await.take().expect("upstream request");
        assert_eq!(observed.method, "POST");
        assert_eq!(observed.path, "/rpc");
        assert_eq!(observed.query.as_deref(), Some("keep=1"));
        assert_eq!(observed.host.as_deref(), Some(vhost.as_str()));
        assert!(observed.authorization.is_none());
        assert_eq!(observed.body, b"vhost body");
        Ok(())
    }

    #[tokio::test]
    async fn vhost_does_not_trust_arbitrary_hosts() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let private_route_id = "private-route";
        let private_lease = runtime
            .config()
            .port_lease_registry
            .lease(PortLeaseRequest {
                target_id: ExecutionTargetId::from("local"),
                port_name: "private-web".to_string(),
                protocol: PortProtocol::Tcp,
                requested_port: None,
            })
            .await;
        runtime
            .config()
            .proxy_route_registry
            .register(ProxyRouteRegisterRequest {
                route_id: Some(private_route_id.to_string()),
                upstream: ProxyRouteUpstream {
                    port_lease_id: private_lease.lease.id,
                    port_name: "private-web".to_string(),
                },
                protocol: ProxyProtocol::Http,
                access: ProxyRouteAccess::HostAuthenticated,
            })
            .await;
        runtime
            .config()
            .proxy_route_registry
            .set_ready(private_route_id, true)
            .await;
        let private_vhost = format!("{}.apps.example.test", route_slug(private_route_id));
        let app = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: Some("proxy-token".to_string()),
            app_base_domain: Some("apps.example.test".to_string()),
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let evil_suffix = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/rpc")
                    .header(header::HOST, "route.apps.example.test.evil.test")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(evil_suffix.status(), StatusCode::UNAUTHORIZED);

        let private_route = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(header::HOST, private_vhost)
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(private_route.status(), StatusCode::NOT_FOUND);

        let unknown_route = app
            .oneshot(
                Request::builder()
                    .uri("/rpc")
                    .header(header::HOST, "unknown.apps.example.test")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(unknown_route.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn vhost_public_url_is_derived_without_platform_schema_change() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let state = AppState {
            runtime,
            static_dir: None,
            access_token: None,
            app_base_domain: Some("apps.example.test".to_string()),
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        };
        let route_id = "My_App/Main";
        let url = service_public_url_for_route(
            &state,
            route_id,
            "/p/My_App/Main/",
            ProxyRouteAccess::Public,
        );
        assert_eq!(
            url,
            format!("https://{}.apps.example.test/", route_slug(route_id))
        );
        assert_eq!(
            service_public_url_for_route(
                &state,
                route_id,
                "/p/My_App/Main/",
                ProxyRouteAccess::HostAuthenticated,
            ),
            "/p/My_App/Main/"
        );
        Ok(())
    }

    #[tokio::test]
    async fn proxy_does_not_forward_referer_or_follow_redirects_and_strips_dangerous_response_headers(
    ) -> anyhow::Result<()> {
        let observed_referer = Arc::new(Mutex::new(None::<String>));
        let upstream_referer = observed_referer.clone();
        let upstream = Router::new()
            .fallback(any(
                move |State(observed_referer): State<Arc<Mutex<Option<String>>>>,
                      headers: HeaderMap| async move {
                    *observed_referer.lock().await = headers
                        .get(header::REFERER)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    (
                        StatusCode::FOUND,
                        [
                            (header::LOCATION, "http://169.254.169.254/latest/meta-data"),
                            (header::SET_COOKIE, "session=leak; Path=/"),
                            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
                        ],
                        "redirect body",
                    )
                },
            ))
            .with_state(upstream_referer);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let upstream_addr: SocketAddr = listener.local_addr()?;
        tokio::spawn(async move {
            axum::serve(listener, upstream)
                .await
                .expect("test upstream serves");
        });

        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let lease = runtime
            .config()
            .port_lease_registry
            .lease(PortLeaseRequest {
                target_id: ExecutionTargetId::from("local"),
                port_name: "web".to_string(),
                protocol: PortProtocol::Tcp,
                requested_port: Some(upstream_addr.port()),
            })
            .await;
        runtime
            .config()
            .proxy_route_registry
            .register(ProxyRouteRegisterRequest {
                route_id: Some("redirect-route".to_string()),
                upstream: ProxyRouteUpstream {
                    port_lease_id: lease.lease.id.clone(),
                    port_name: "web".to_string(),
                },
                protocol: ProxyProtocol::Http,
                access: ProxyRouteAccess::HostAuthenticated,
            })
            .await;
        runtime
            .config()
            .proxy_route_registry
            .set_ready("redirect-route", true)
            .await;
        let app = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: Some("proxy-token".to_string()),
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/p/redirect-route/redirect?access_token=proxy-token")
                    .header(header::AUTHORIZATION, "Bearer proxy-token")
                    .header(
                        header::REFERER,
                        "http://localhost/p/redirect-route/redirect?access_token=proxy-token",
                    )
                    .body(Body::empty())?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::FOUND);
        assert!(response.headers().get(header::LOCATION).is_none());
        assert!(response.headers().get(header::SET_COOKIE).is_none());
        assert!(response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        assert_eq!(&body[..], b"redirect body");
        assert!(observed_referer.lock().await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn websocket_proxy_route_is_token_protected() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let app = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: Some("ws-token".to_string()),
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let denied = app
            .oneshot(
                Request::builder()
                    .uri("/p/missing/ws")
                    .header(header::CONNECTION, "upgrade")
                    .header(header::UPGRADE, "websocket")
                    .header("sec-websocket-version", "13")
                    .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                    .body(Body::empty())?,
            )
            .await?;

        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn websocket_proxy_requires_websocket_route() -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let upstream_addr: SocketAddr = listener.local_addr()?;
        drop(listener);

        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let lease = runtime
            .config()
            .port_lease_registry
            .lease(PortLeaseRequest {
                target_id: ExecutionTargetId::from("local"),
                port_name: "web".to_string(),
                protocol: PortProtocol::Tcp,
                requested_port: Some(upstream_addr.port()),
            })
            .await;
        runtime
            .config()
            .proxy_route_registry
            .register(ProxyRouteRegisterRequest {
                route_id: Some("http-only".to_string()),
                upstream: ProxyRouteUpstream {
                    port_lease_id: lease.lease.id.clone(),
                    port_name: "web".to_string(),
                },
                protocol: ProxyProtocol::Http,
                access: ProxyRouteAccess::HostAuthenticated,
            })
            .await;
        runtime
            .config()
            .proxy_route_registry
            .set_ready("http-only", true)
            .await;
        let app = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: None,
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/p/http-only/ws")
                    .header(header::CONNECTION, "upgrade")
                    .header(header::UPGRADE, "websocket")
                    .header("sec-websocket-version", "13")
                    .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                    .body(Body::empty())?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
        Ok(())
    }

    #[tokio::test]
    async fn websocket_proxy_echoes_text_and_strips_access_token() -> anyhow::Result<()> {
        let observed = Arc::new(Mutex::new(
            None::<(
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            )>,
        ));
        let upstream_observed = observed.clone();
        let upstream = Router::new()
            .fallback(any(
                move |State(observed): State<
                    Arc<
                        Mutex<
                            Option<(
                                String,
                                Option<String>,
                                Option<String>,
                                Option<String>,
                                Option<String>,
                                Option<String>,
                                Option<String>,
                            )>,
                        >,
                    >,
                >,
                      ws: WebSocketUpgrade,
                      OriginalUri(uri): OriginalUri,
                      headers: HeaderMap| async move {
                    let authorization = headers
                        .get(header::AUTHORIZATION)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    let cookie = headers
                        .get(header::COOKIE)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    let origin = headers
                        .get(header::ORIGIN)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    let subprotocol = headers
                        .get(header::SEC_WEBSOCKET_PROTOCOL)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    let host = headers
                        .get(header::HOST)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    *observed.lock().await = Some((
                        uri.path().to_string(),
                        uri.query().map(str::to_string),
                        authorization,
                        cookie,
                        origin,
                        subprotocol,
                        host,
                    ));
                    ws.protocols(["plurora.test"])
                        .on_upgrade(|mut socket| async move {
                            if let Some(Ok(message)) = socket.recv().await {
                                let _ = socket.send(message).await;
                            }
                        })
                },
            ))
            .with_state(upstream_observed);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let upstream_addr: SocketAddr = listener.local_addr()?;
        tokio::spawn(async move {
            axum::serve(listener, upstream)
                .await
                .expect("test upstream serves");
        });

        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let lease = runtime
            .config()
            .port_lease_registry
            .lease(PortLeaseRequest {
                target_id: ExecutionTargetId::from("local"),
                port_name: "ws".to_string(),
                protocol: PortProtocol::Tcp,
                requested_port: Some(upstream_addr.port()),
            })
            .await;
        runtime
            .config()
            .proxy_route_registry
            .register(ProxyRouteRegisterRequest {
                route_id: Some("ws-route".to_string()),
                upstream: ProxyRouteUpstream {
                    port_lease_id: lease.lease.id.clone(),
                    port_name: "ws".to_string(),
                },
                protocol: ProxyProtocol::Websocket,
                access: ProxyRouteAccess::HostAuthenticated,
            })
            .await;
        runtime
            .config()
            .proxy_route_registry
            .set_ready("ws-route", true)
            .await;
        let app = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: Some("ws-token".to_string()),
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let proxy_addr = listener.local_addr()?;
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("test proxy serves");
        });

        let mut request =
            format!("ws://{proxy_addr}/p/ws-route/socket/echo?keep=1&access_token=ws-token")
                .into_client_request()?;
        request.headers_mut().insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer ws-token"),
        );
        request
            .headers_mut()
            .insert(header::COOKIE, HeaderValue::from_static("session=secret"));
        request.headers_mut().insert(
            header::ORIGIN,
            HeaderValue::from_static("https://client.example"),
        );
        request.headers_mut().insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("plurora.test, plurora.fallback"),
        );
        let stream = TcpStream::connect(proxy_addr).await?;
        let (mut socket, response) = tokio_tungstenite::client_async(request, stream).await?;
        assert_eq!(
            response
                .headers()
                .get(header::SEC_WEBSOCKET_PROTOCOL)
                .and_then(|value| value.to_str().ok()),
            Some("plurora.test")
        );
        socket.send(Message::Text("hello ws".into())).await?;

        match socket.next().await.transpose()? {
            Some(Message::Text(text)) => assert_eq!(text, "hello ws"),
            other => panic!("unexpected websocket response: {other:?}"),
        }

        let observed = observed
            .lock()
            .await
            .take()
            .expect("upstream websocket request");
        assert_eq!(observed.0, "/socket/echo");
        assert_eq!(observed.1.as_deref(), Some("keep=1"));
        assert!(observed.2.is_none());
        assert!(observed.3.is_none());
        assert_eq!(observed.4.as_deref(), Some("https://client.example"));
        assert_eq!(
            observed.5.as_deref(),
            Some("plurora.test, plurora.fallback")
        );
        assert_eq!(
            observed.6.as_deref(),
            Some(format!("127.0.0.1:{}", upstream_addr.port()).as_str())
        );
        Ok(())
    }

    #[tokio::test]
    async fn surface_bundles_require_host_auth_when_token_gate_enabled() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let bundle_dir = dir.path().join("surface");
        std::fs::create_dir_all(&bundle_dir)?;
        std::fs::write(bundle_dir.join("main.mjs"), "export const ok = true;")?;

        let store = Arc::new(InMemoryEventStore::default());
        let mut config = RuntimeConfig::default();
        config
            .surface_dev_paths
            .insert("test".to_string(), bundle_dir.to_string_lossy().to_string());
        let runtime = Arc::new(Runtime::new(store, config));
        let access_registry = host_access_registry();
        let app = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: Some("bundle-token".to_string()),
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: access_registry.clone(),
            target_agents: target_agent_registry(),
        });

        let denied = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/surface-bundles/test/main.mjs")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/surface-bundles/test/main.mjs")
                    .header(header::AUTHORIZATION, "Bearer bundle-token")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            Some(&HeaderValue::from_static("*"))
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("no-cache, must-revalidate"))
        );
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        assert_eq!(&bytes[..], b"export const ok = true;");

        let forged_lease = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/surface-assets/not-a-lease/test/main.mjs")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(forged_lease.status(), StatusCode::NOT_FOUND);

        let mut resolved = json!({
            "bundle_url": "/surface-bundles/test/main.mjs?v=lease-test",
            "stylesheets": []
        });
        mint_surface_asset_lease(
            &mut resolved,
            &HostAccessIdentity::root(),
            access_registry.as_ref(),
        )?;
        let leased = app
            .oneshot(
                Request::builder()
                    .uri(resolved["bundle_url"].as_str().expect("leased bundle URL"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(leased.status(), StatusCode::OK);
        let bytes = to_bytes(leased.into_body(), usize::MAX).await?;
        assert_eq!(&bytes[..], b"export const ok = true;");
        Ok(())
    }

    #[test]
    fn surface_asset_lease_is_root_scoped_and_preserves_cache_keys() -> anyhow::Result<()> {
        let project_id = "lease_project__abc12345";
        let registry = HostAccessRegistry::default();
        let mut resolved = json!({
            "bundle_url": format!("/surface-bundles/projects/{project_id}/bundle.mjs?v=abc"),
            "stylesheets": [
                format!("/surface-bundles/projects/{project_id}/styles/surface.css?v=def")
            ]
        });
        mint_surface_asset_lease(&mut resolved, &HostAccessIdentity::root(), &registry)?;
        let bundle_url = resolved["bundle_url"].as_str().expect("bundle URL");
        let mut segments = bundle_url.trim_start_matches('/').split('/');
        assert_eq!(segments.next(), Some("surface-assets"));
        let lease_id = segments.next().expect("lease id");
        assert!(bundle_url.ends_with("bundle.mjs?v=abc"));
        assert!(resolved["stylesheets"][0]
            .as_str()
            .expect("stylesheet URL")
            .contains(&format!(
                "/surface-assets/{lease_id}/projects/{project_id}/"
            )));
        assert!(surface_asset_lease_is_valid(
            &registry,
            lease_id,
            &format!("projects/{project_id}")
        ));
        assert!(!surface_asset_lease_is_valid(
            &registry,
            lease_id,
            "projects/another_project__abc12345"
        ));
        assert!(!surface_asset_lease_is_valid(
            &HostAccessRegistry::default(),
            lease_id,
            &format!("projects/{project_id}")
        ));
        surface_asset_lease_registry()
            .lock()
            .expect("surface asset lease lock")
            .insert(
                "expired-lease".to_string(),
                SurfaceAssetLease {
                    root: format!("projects/{project_id}"),
                    grant_id: None,
                    host_access_instance_id: registry.instance_id(),
                    expires_at_ms: 0,
                },
            );
        assert!(!surface_asset_lease_is_valid(
            &registry,
            "expired-lease",
            &format!("projects/{project_id}")
        ));
        Ok(())
    }

    #[tokio::test]
    async fn surface_bundle_serving_rejects_symlink_escape() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        let bundle_dir = dir.path().join("surface");
        std::fs::create_dir_all(&bundle_dir)?;
        std::fs::write(outside.path().join("secret.mjs"), "do-not-serve")?;

        if !create_test_file_symlink(
            &outside.path().join("secret.mjs"),
            &bundle_dir.join("leak.mjs"),
        )? {
            return Ok(());
        }

        let store = Arc::new(InMemoryEventStore::default());
        let mut config = RuntimeConfig::default();
        config
            .surface_dev_paths
            .insert("test".to_string(), bundle_dir.to_string_lossy().to_string());
        let runtime = Arc::new(Runtime::new(store, config));
        let app = app_with_state(AppState {
            runtime,
            static_dir: None,
            access_token: None,
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/surface-bundles/test/leak.mjs")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn static_js_is_cors_readable_for_sandboxed_surface_frame() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        std::fs::write(
            dir.path().join("surface-frame-bootstrap.js"),
            "export const ok = true;",
        )?;

        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let app = app_with_state(AppState {
            runtime,
            static_dir: Some(dir.path().to_path_buf()),
            access_token: Some("static-token".to_string()),
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/surface-frame-bootstrap.js")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            Some(&HeaderValue::from_static("*"))
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("public, max-age=3600")),
            "generic static assets should use the short-lived shell cache policy"
        );
        Ok(())
    }

    #[tokio::test]
    async fn reserved_paths_do_not_fallback_to_static_index() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        std::fs::write(dir.path().join("index.html"), "public")?;
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let app = app_with_state(AppState {
            runtime,
            static_dir: Some(dir.path().to_path_buf()),
            access_token: None,
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        for path in [
            "/rpc/anything",
            "/p/anything",
            "/surface-bundles/anything",
            "/surface-assets/anything",
            "/surface-bundlesx",
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty())?)
                .await?;
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "path {path}");
        }
        for path in ["/project/bad/id", "/project/bad%2Fid"] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty())?)
                .await?;
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "path {path}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn static_serving_rejects_symlink_escape() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        std::fs::write(dir.path().join("index.html"), "public")?;
        std::fs::write(outside.path().join("secret.txt"), "do-not-serve")?;

        if !create_test_file_symlink(
            &outside.path().join("secret.txt"),
            &dir.path().join("leak.txt"),
        )? {
            return Ok(());
        }

        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
        let app = app_with_state(AppState {
            runtime,
            static_dir: Some(dir.path().to_path_buf()),
            access_token: None,
            app_base_domain: None,
            build_jobs: Arc::new(BuildDeployJobRegistry::default()),
            development: development_registry(),
            host_access: host_access_registry(),
            target_agents: target_agent_registry(),
        });

        let response = app
            .oneshot(Request::builder().uri("/leak.txt").body(Body::empty())?)
            .await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }
}
