//! Handler for `plurora/docker-runtime-lab` capabilities.
//!
//! Docker Runtime Lab is an ordinary package. It performs Docker container
//! lifecycle actions only after host-provided approval and metadata are present.
//! The Host/runtime owns port leases and proxy routes; this package never invokes
//! HostAdmin-only kernel port/proxy capabilities.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use bollard::body_full;
use bollard::container::LogOutput;
use bollard::models::{
    ContainerCreateBody, ContainerInspectResponse, ContainerSummary, ContainerSummaryStateEnum,
    HostConfig, PortBinding, PortMap,
};
use bollard::query_parameters::{
    BuildImageOptionsBuilder, CreateContainerOptionsBuilder, CreateImageOptionsBuilder,
    InspectContainerOptionsBuilder, ListContainersOptionsBuilder, LogsOptionsBuilder,
    RemoveContainerOptionsBuilder, RemoveImageOptionsBuilder, StopContainerOptionsBuilder,
};
use bollard::Docker;
use bytes::Bytes;
use futures::StreamExt;
use plurora_work::{InstallationId, WorkspaceId};
use serde_json::Value;

use super::safety;
use super::InprocInvocation;
use crate::object_store::sha256_digest;
use crate::runtime::{DeploymentReconcileSource, ManagedContainerReport};

const PACKAGE_ID: &str = "plurora/docker-runtime-lab";
const BIND_HOST: &str = "127.0.0.1";
const DEFAULT_TAIL: u32 = 200;
const DEFAULT_MAX_BYTES: usize = 65_536;

#[derive(Debug, Default)]
pub struct DockerDeploymentReconcileSource;

#[async_trait::async_trait]
impl DeploymentReconcileSource for DockerDeploymentReconcileSource {
    async fn list_managed(&self) -> anyhow::Result<Vec<ManagedContainerReport>> {
        let output = list_managed_async().await.map_err(anyhow::Error::msg)?;
        Ok(output
            .get("managed")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|container| {
                Some(ManagedContainerReport {
                    route_id: container.get("route_id")?.as_str()?.to_string(),
                    port_lease_id: container.get("port_lease_id")?.as_str()?.to_string(),
                    running: container.get("running")?.as_bool()?,
                    host_port: container
                        .get("host_port")
                        .and_then(Value::as_u64)
                        .and_then(|port| u16::try_from(port).ok()),
                    container_ref: ["container_id", "id", "container_name", "name"]
                        .into_iter()
                        .find_map(|field| {
                            container
                                .get(field)
                                .and_then(Value::as_str)
                                .map(str::to_string)
                        }),
                    operation_id: container
                        .get("operation_id")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                })
            })
            .collect())
    }
}
const MAX_TAIL: u32 = 5_000;
const DEFAULT_MAX_CONTEXT_BYTES: u64 = 256 * 1024 * 1024;
const DEFAULT_MAX_CONTEXT_FILES: u64 = 25_000;
const MAX_BUILD_LOG_BYTES: usize = 64 * 1024;
const DEFAULT_BUILD_TIMEOUT_SECS: u64 = 15 * 60;
const MAX_BUILD_TIMEOUT_SECS: u64 = 60 * 60;
const DEFAULT_BUILD_MEMORY_BYTES: i32 = 1024 * 1024 * 1024;
const DEFAULT_BUILD_CPU_QUOTA: i32 = 100_000;
const NIXPACKS_BINARY: &str = "nixpacks";
const NIXPACKS_GENERATED_DOCKERFILE: &str = "Dockerfile";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuildStrategy {
    Dockerfile,
    Nixpacks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuildNetworkMode {
    None,
    Bridge,
}

impl BuildNetworkMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bridge => "bridge",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BuildContextScope {
    Workspace {
        workspace_id: WorkspaceId,
    },
    DevelopmentScratch {
        workspace_id: WorkspaceId,
        change_set_id: String,
    },
}

impl BuildStrategy {
    fn as_str(self) -> &'static str {
        match self {
            Self::Dockerfile => "dockerfile",
            Self::Nixpacks => "nixpacks",
        }
    }
}

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }

    let id = request.capability_id.as_str();
    if id.ends_with("/describe_contract") {
        Some(describe_contract(request))
    } else if id.ends_with("/validate_spec") {
        Some(validate_spec(request))
    } else if id.ends_with("/plan_container") {
        Some(plan_container(request))
    } else if id.ends_with("/build_image") {
        Some(build_image(request))
    } else if id.ends_with("/remove_image") {
        Some(remove_image(request))
    } else if id.ends_with("/start_container") {
        Some(start_container(request))
    } else if id.ends_with("/status") {
        Some(status(request))
    } else if id.ends_with("/logs") {
        Some(logs(request))
    } else if id.ends_with("/stop_container") {
        Some(stop_container(request))
    } else if id.ends_with("/list_managed") {
        Some(list_managed(request))
    } else {
        None
    }
}

fn describe_contract(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "docker_runtime_lab_contract",
        "package_id": PACKAGE_ID,
        "package_kind": "ordinary",
        "capabilities": [
            {"id": "plurora/docker-runtime-lab/describe_contract", "docker_performed": false},
            {"id": "plurora/docker-runtime-lab/validate_spec", "docker_performed": false},
            {"id": "plurora/docker-runtime-lab/plan_container", "docker_performed": false},
            {"id": "plurora/docker-runtime-lab/build_image", "docker_performed": true, "requires_approved_true": true, "strategies": ["dockerfile", "nixpacks"], "experimental_strategies": ["nixpacks"], "network_modes": ["none", "bridge"], "development_scratch_default_network": "none"},
            {"id": "plurora/docker-runtime-lab/remove_image", "docker_performed": true, "requires_approved_true": true, "label_guarded": true},
            {"id": "plurora/docker-runtime-lab/start_container", "docker_performed": true, "requires_approved_true": true},
            {"id": "plurora/docker-runtime-lab/status", "docker_performed": true, "ownership_label_guarded": true},
            {"id": "plurora/docker-runtime-lab/logs", "docker_performed": true, "bounded": true, "redacted": true, "ownership_label_guarded": true},
            {"id": "plurora/docker-runtime-lab/stop_container", "docker_performed": true, "requires_approved_true": true, "ownership_label_guarded": true},
            {"id": "plurora/docker-runtime-lab/list_managed", "docker_performed": true, "label_filter": "managed-by=plurora"}
        ],
        "host_owned_resources": {
            "port_lease": true,
            "proxy_route": true,
            "accepted_metadata_fields": ["host_port", "port_lease_id", "route_id"]
        },
        "enforced_policy": {
            "privileged": false,
            "network_mode": "bridge",
            "bind_host": BIND_HOST,
            "bind_mounts": false,
            "mounts": false,
            "arbitrary_env": false,
            "secrets_injection": false,
            "publish_all_ports": false
        },
        "inference_performed": false,
        "network_performed": false,
        "execution_performed": false,
        "docker_performed": false,
        "provenance": provenance(request)
    }))
}

fn validate_spec(request: &InprocInvocation) -> anyhow::Result<Value> {
    let mut diagnostics = validate_input(&request.input);
    if safety::contains_raw_secret(&request.input) {
        diagnostics.push(error(
            "raw_secret_blocked",
            "input contains raw-secret-like content; use secret_ref references instead",
        ));
    }
    let valid = diagnostics.iter().all(|d| d.severity != "error");

    Ok(serde_json::json!({
        "kind": "docker_runtime_lab_validation",
        "valid": valid,
        "diagnostics": diagnostics_to_json(&mut diagnostics),
        "docker_performed": false,
        "provenance": provenance(request)
    }))
}

fn plan_container(request: &InprocInvocation) -> anyhow::Result<Value> {
    let mut diagnostics = validate_input(&request.input);
    if safety::contains_raw_secret(&request.input) {
        diagnostics.push(error(
            "raw_secret_blocked",
            "input contains raw-secret-like content; use secret_ref references instead",
        ));
    }
    let valid = diagnostics.iter().all(|d| d.severity != "error");
    let image = string_field(&request.input, "image");
    let container_port = u16_field(&request.input, "container_port");
    let host_port = u16_field(&request.input, "host_port");
    let route_id = string_field(&request.input, "route_id");
    let port_lease_id = string_field(&request.input, "port_lease_id");
    let installation_id = string_field(&request.input, "installation_id");

    Ok(serde_json::json!({
        "kind": "docker_runtime_lab_container_plan",
        "valid": valid,
        "diagnostics": diagnostics_to_json(&mut diagnostics),
        "image": value_or_null(image),
        "container_port": container_port,
        "host_port": host_port,
        "bind_host": BIND_HOST,
        "route_id": value_or_null(route_id),
        "port_lease_id": value_or_null(port_lease_id),
        "installation_id": value_or_null(installation_id),
        "requires_host_port_lease": true,
        "requires_proxy_route": true,
        "docker_performed": false,
        "host_owned_resources": ["host_port", "port_lease_id", "route_id"],
        "enforced_policy": {
            "privileged": false,
            "network_mode": "bridge",
            "bind_mounts": false,
            "mounts": false,
            "env": [],
            "publish_all_ports": false
        },
        "provenance": provenance(request)
    }))
}

fn build_image(request: &InprocInvocation) -> anyhow::Result<Value> {
    match parse_build_image_request(&request.input) {
        Ok(spec) => {
            let result = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(async move { build_image_async(spec).await })
            });
            match result {
                Ok(value) => Ok(with_provenance(value, request)),
                Err(error) => Ok(with_provenance(
                    docker_error_output("build_image", error),
                    request,
                )),
            }
        }
        Err(reason) => Ok(serde_json::json!({
            "kind": "docker_runtime_lab_rejected",
            "reason": reason,
            "docker_performed": false,
            "image_built": false,
            "provenance": provenance(request)
        })),
    }
}

fn remove_image(request: &InprocInvocation) -> anyhow::Result<Value> {
    let parsed = (|| -> Result<(String, String, String), String> {
        if !request
            .input
            .get("approved")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("remove_image requires approved: true; fail-closed".to_string());
        }
        let installation_id = string_field(&request.input, "installation_id")
            .ok_or_else(|| "installation_id is required".to_string())?;
        InstallationId::parse(installation_id.as_str())
            .map_err(|_| "installation_id must be a valid installation id".to_string())?;
        let build_id = string_field(&request.input, "build_id")
            .ok_or_else(|| "build_id is required".to_string())?;
        if !valid_build_id(&build_id) {
            return Err("build_id must be label-safe".to_string());
        }
        let change_set_id = string_field(&request.input, "development_change_id")
            .ok_or_else(|| "development_change_id is required".to_string())?;
        if !valid_development_change_id(&change_set_id) {
            return Err("development_change_id must be a valid Host change id".to_string());
        }
        Ok((installation_id, build_id, change_set_id))
    })();
    let (installation_id, build_id, change_set_id) = match parsed {
        Ok(parsed) => parsed,
        Err(reason) => {
            return Ok(serde_json::json!({
                "kind": "docker_runtime_lab_rejected",
                "reason": reason,
                "docker_performed": false,
                "image_removed": false,
                "provenance": provenance(request)
            }))
        }
    };
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async move {
            remove_image_async(installation_id, build_id, change_set_id).await
        })
    });
    match result {
        Ok(value) => Ok(with_provenance(value, request)),
        Err(error) => Ok(with_provenance(
            docker_error_output("remove_image", error),
            request,
        )),
    }
}

fn start_container(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }
    if !request
        .input
        .get("approved")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(serde_json::json!({
            "kind": "docker_runtime_lab_rejected",
            "reason": "start_container requires approved: true; fail-closed",
            "docker_performed": false,
            "container_started": false,
            "provenance": provenance(request)
        }));
    }

    let diagnostics = validate_input(&request.input);
    if diagnostics.iter().any(|d| d.severity == "error") {
        let mut diagnostics = diagnostics;
        return Ok(serde_json::json!({
            "kind": "docker_runtime_lab_rejected",
            "reason": "container spec failed validation; fail-closed",
            "diagnostics": diagnostics_to_json(&mut diagnostics),
            "docker_performed": false,
            "container_started": false,
            "provenance": provenance(request)
        }));
    }

    let image = string_field(&request.input, "image").unwrap_or_default();
    let installation_id = string_field(&request.input, "installation_id").unwrap_or_default();
    let container_port = u16_field(&request.input, "container_port").unwrap_or_default();
    let host_port = u16_field(&request.input, "host_port").unwrap_or_default();
    let route_id = string_field(&request.input, "route_id").unwrap_or_default();
    let port_lease_id = string_field(&request.input, "port_lease_id").unwrap_or_default();
    let operation_id = string_field(&request.input, "operation_id");
    let pull_if_missing = request
        .input
        .get("pull_if_missing")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let container_name = string_field(&request.input, "container_name")
        .or_else(|| string_field(&request.input, "name"))
        .unwrap_or_else(|| {
            format!(
                "plurora-docker-runtime-lab-{}-{}",
                sanitize_name(&route_id),
                host_port
            )
        });

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async move {
            start_container_async(
                image,
                installation_id,
                container_port,
                host_port,
                route_id,
                port_lease_id,
                operation_id,
                container_name,
                pull_if_missing,
            )
            .await
        })
    });

    match result {
        Ok(value) => Ok(value),
        Err(error) => Ok(with_provenance(
            docker_error_output("start_container", error),
            request,
        )),
    }
}

fn status(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }
    let Some(container) = container_ref(&request.input) else {
        return Ok(missing_container_ref_output(request));
    };
    let (installation_id, route_id, port_lease_id) = match managed_container_scope(&request.input) {
        Ok(scope) => scope,
        Err(reason) => return Ok(operation_rejected_output(request, "status", &reason)),
    };
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async move {
            status_async(container, installation_id, route_id, port_lease_id).await
        })
    });
    match result {
        Ok(value) => Ok(with_provenance(value, request)),
        Err(error) => Ok(with_provenance(
            docker_error_output("status", error),
            request,
        )),
    }
}

fn logs(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }
    let Some(container) = container_ref(&request.input) else {
        return Ok(missing_container_ref_output(request));
    };
    let (installation_id, route_id, port_lease_id) = match managed_container_scope(&request.input) {
        Ok(scope) => scope,
        Err(reason) => return Ok(operation_rejected_output(request, "logs", &reason)),
    };
    let tail = request
        .input
        .get("tail")
        .and_then(Value::as_u64)
        .map(|n| n.min(MAX_TAIL as u64) as u32)
        .unwrap_or(DEFAULT_TAIL);
    let max_bytes = request
        .input
        .get("max_bytes")
        .and_then(Value::as_u64)
        .map(|n| n.min(DEFAULT_MAX_BYTES as u64) as usize)
        .unwrap_or(DEFAULT_MAX_BYTES);
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async move {
            logs_async(
                container,
                installation_id,
                route_id,
                port_lease_id,
                tail,
                max_bytes,
            )
            .await
        })
    });
    match result {
        Ok(value) => Ok(with_provenance(value, request)),
        Err(error) => Ok(with_provenance(docker_error_output("logs", error), request)),
    }
}

fn stop_container(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }
    if !request
        .input
        .get("approved")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(operation_rejected_output(
            request,
            "stop_container",
            "stop_container requires approved: true; fail-closed",
        ));
    }
    let Some(container) = container_ref(&request.input) else {
        return Ok(missing_container_ref_output(request));
    };
    let (installation_id, route_id, port_lease_id) = match managed_container_scope(&request.input) {
        Ok(scope) => scope,
        Err(reason) => {
            return Ok(operation_rejected_output(
                request,
                "stop_container",
                &reason,
            ))
        }
    };
    let timeout_secs = request
        .input
        .get("timeout_secs")
        .and_then(Value::as_i64)
        .map(|n| n.clamp(0, 60) as i32)
        .unwrap_or(10);
    let force = request
        .input
        .get("force")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async move {
            stop_container_async(
                container,
                installation_id,
                route_id,
                port_lease_id,
                timeout_secs,
                force,
            )
            .await
        })
    });
    match result {
        Ok(value) => Ok(with_provenance(value, request)),
        Err(error) => Ok(with_provenance(
            docker_error_output("stop_container", error),
            request,
        )),
    }
}

fn list_managed(request: &InprocInvocation) -> anyhow::Result<Value> {
    if safety::contains_raw_secret(&request.input) {
        return Ok(rejected_output(request));
    }
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async move { list_managed_async().await })
    });
    match result {
        Ok(value) => Ok(with_provenance(value, request)),
        Err(error) => Ok(with_provenance(
            docker_error_output("list_managed", error),
            request,
        )),
    }
}

async fn docker() -> Result<Docker, String> {
    let docker = Docker::connect_with_local_defaults()
        .or_else(|_| Docker::connect_with_defaults())
        .map_err(|e| format!("docker connection unavailable: {e}"))?;
    docker
        .ping()
        .await
        .map_err(|e| format!("docker daemon unavailable: {e}"))?;
    Ok(docker)
}

async fn start_container_async(
    image: String,
    installation_id: String,
    container_port: u16,
    host_port: u16,
    route_id: String,
    port_lease_id: String,
    operation_id: Option<String>,
    container_name: String,
    pull_if_missing: bool,
) -> Result<Value, String> {
    let docker = docker().await?;
    if pull_if_missing {
        let options = CreateImageOptionsBuilder::default()
            .from_image(&image)
            .build();
        let mut stream = docker.create_image(Some(options), None, None);
        while let Some(item) = stream.next().await {
            item.map_err(|e| format!("docker image pull failed: {e}"))?;
        }
    }
    let image_id = docker
        .inspect_image(&image)
        .await
        .map_err(|e| format!("docker image inspect failed: {e}"))?
        .id
        .ok_or_else(|| {
            "docker image inspect did not return a content-addressable id".to_string()
        })?;

    let container_port_key = format!("{container_port}/tcp");
    let mut port_bindings: PortMap = HashMap::new();
    port_bindings.insert(
        container_port_key.clone(),
        Some(vec![PortBinding {
            host_ip: Some(BIND_HOST.to_string()),
            host_port: Some(host_port.to_string()),
        }]),
    );

    let mut labels = HashMap::from([
        ("managed-by".to_string(), "plurora".to_string()),
        ("plurora.package_id".to_string(), PACKAGE_ID.to_string()),
        (
            "plurora.installation_id".to_string(),
            installation_id.clone(),
        ),
        ("plurora.route_id".to_string(), route_id.clone()),
        ("plurora.port_lease_id".to_string(), port_lease_id.clone()),
    ]);
    if let Some(operation_id) = operation_id.as_ref() {
        if !valid_label_value(operation_id) {
            return Err("deployment operation id must be label-safe".to_string());
        }
        labels.insert(
            "plurora.deployment_operation_id".to_string(),
            operation_id.clone(),
        );
    }
    let config = ContainerCreateBody {
        image: Some(image_id.clone()),
        labels: Some(labels),
        exposed_ports: Some(vec![container_port_key]),
        env: None,
        host_config: Some(HostConfig {
            binds: None,
            mounts: None,
            network_mode: Some("bridge".to_string()),
            port_bindings: Some(port_bindings),
            privileged: Some(false),
            publish_all_ports: Some(false),
            ..Default::default()
        }),
        network_disabled: Some(false),
        ..Default::default()
    };
    let options = CreateContainerOptionsBuilder::default()
        .name(&container_name)
        .build();
    let created = docker
        .create_container(Some(options), config)
        .await
        .map_err(|e| format!("docker create failed: {e}"))?;
    docker
        .start_container(&created.id, None)
        .await
        .map_err(|e| format!("docker start failed: {e}"))?;

    Ok(serde_json::json!({
        "kind": "docker_runtime_lab_container_started",
        "container_id": created.id,
        "container_name": container_name,
        "status": "started",
        "image": image,
        "installation_id": installation_id,
        "image_id": image_id,
        "container_port": container_port,
        "host_port": host_port,
        "bind_host": BIND_HOST,
        "route_id": route_id,
        "port_lease_id": port_lease_id,
        "operation_id": operation_id,
        "docker_performed": true,
        "container_started": true,
        "warnings": created.warnings
    }))
}

#[derive(Debug, Clone)]
struct BuildImageSpec {
    strategy: BuildStrategy,
    installation_id: String,
    workspace_id: WorkspaceId,
    build_id: String,
    context_scope: BuildContextScope,
    network_mode: BuildNetworkMode,
    dockerfile: String,
    source_commit: Option<String>,
    build_descriptor_hash: Option<String>,
    expected_context_digest: Option<String>,
    build_args: HashMap<String, String>,
    max_context_bytes: u64,
    max_context_files: u64,
    build_timeout_secs: u64,
}

#[derive(Debug, Clone)]
struct ContextTar {
    bytes: Vec<u8>,
    files: u64,
    total_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct PreparedDockerBuildContext {
    pub bytes: Vec<u8>,
    pub files: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone)]
struct PreparedBuildContext {
    context: ContextTar,
    dockerfile: String,
    buildpack_version: Option<String>,
    generated_dockerfile: Option<String>,
}

#[derive(Debug)]
struct ValidatedBuildContextRoot {
    path: PathBuf,
    handle: same_file::Handle,
}

async fn build_image_async(spec: BuildImageSpec) -> Result<Value, String> {
    let tag = image_tag(&spec.installation_id, &spec.build_id);
    let prepared = tokio::task::spawn_blocking({
        let spec = spec.clone();
        move || prepare_build_context(&spec)
    })
    .await
    .map_err(|e| format!("build context task failed: {e}"))??;
    let context_digest = verified_context_digest(&spec, &prepared.context)?;
    let docker = docker().await?;

    let mut labels = build_labels(&spec);
    labels.insert("managed-by".to_string(), "plurora".to_string());
    labels.insert("plurora.package_id".to_string(), PACKAGE_ID.to_string());
    if let Some(version) = prepared.buildpack_version.as_deref() {
        labels.insert("plurora.buildpack_version".to_string(), version.to_string());
    }

    let options = BuildImageOptionsBuilder::default()
        .dockerfile(&prepared.dockerfile)
        .t(&tag)
        .q(false)
        .rm(true)
        .forcerm(true)
        .memory(DEFAULT_BUILD_MEMORY_BYTES)
        .cpuquota(DEFAULT_BUILD_CPU_QUOTA)
        .networkmode(spec.network_mode.as_str())
        .labels(&labels)
        .buildargs(&spec.build_args)
        .build();
    let body = body_full(Bytes::from(prepared.context.bytes));

    let build = async {
        let mut stream = docker.build_image(options, None, Some(body));
        let mut log_tail = String::new();
        while let Some(item) = stream.next().await {
            let info = item.map_err(|e| format!("docker build failed: {e}"))?;
            if let Some(error) = info.error_detail.and_then(|detail| detail.message) {
                append_log_tail(&mut log_tail, &redact_log_text(&error));
                return Err(format!("docker build failed: {}", redact_log_line(&error)));
            }
            if let Some(stream) = info.stream {
                append_log_tail(&mut log_tail, &redact_log_text(&stream));
            } else if let Some(status) = info.status {
                append_log_tail(&mut log_tail, &redact_log_text(&status));
            }
        }
        Ok(log_tail)
    };

    let log_tail = tokio::time::timeout(Duration::from_secs(spec.build_timeout_secs), build)
        .await
        .map_err(|_| "docker build timed out".to_string())??;
    let image_id = docker
        .inspect_image(&tag)
        .await
        .map_err(|e| format!("docker built image inspect failed: {e}"))?
        .id
        .ok_or_else(|| "docker built image has no content-addressable id".to_string())?;

    Ok(serde_json::json!({
        "kind": "docker_runtime_lab_image_built",
        "image": tag,
        "image_id": image_id,
        "installation_id": spec.installation_id,
        "workspace_id": spec.workspace_id,
        "build_id": spec.build_id,
        "strategy": spec.strategy.as_str(),
        "network_mode": spec.network_mode.as_str(),
        "buildpack": if spec.strategy == BuildStrategy::Nixpacks { Some("nixpacks") } else { None::<&str> },
        "buildpack_version": prepared.buildpack_version,
        "generated_dockerfile": prepared.generated_dockerfile,
        "source_commit": spec.source_commit,
        "build_descriptor_hash": spec.build_descriptor_hash,
        "context_digest": context_digest,
        "context": {
            "files": prepared.context.files,
            "total_bytes": prepared.context.total_bytes,
            "max_files": spec.max_context_files,
            "max_bytes": spec.max_context_bytes,
        },
        "log_tail": log_tail,
        "docker_performed": true,
        "image_built": true,
    }))
}

async fn remove_image_async(
    installation_id: String,
    build_id: String,
    change_set_id: String,
) -> Result<Value, String> {
    let image = image_tag(&installation_id, &build_id);
    let docker = docker().await?;
    let inspected = match docker.inspect_image(&image).await {
        Ok(inspected) => inspected,
        Err(error) if docker_not_found_error(&error.to_string()) => {
            return Ok(serde_json::json!({
                "kind": "docker_runtime_lab_image_absent",
                "image": image,
                "installation_id": installation_id,
                "build_id": build_id,
                "development_change_id": change_set_id,
                "image_removed": true,
                "image_was_present": false,
                "docker_performed": true
            }));
        }
        Err(error) => return Err(format!("docker image inspect failed: {error}")),
    };
    let labels = inspected
        .config
        .and_then(|config| config.labels)
        .ok_or_else(|| "docker image is missing Plurora ownership labels".to_string())?;
    let expected = [
        ("managed-by", "plurora"),
        ("plurora.package_id", PACKAGE_ID),
        ("plurora.installation_id", installation_id.as_str()),
        ("plurora.build_id", build_id.as_str()),
        ("plurora.development_change_id", change_set_id.as_str()),
    ];
    if expected
        .iter()
        .any(|(key, value)| labels.get(*key).map(String::as_str) != Some(*value))
    {
        return Err(
            "docker image ownership labels did not match the requested development build"
                .to_string(),
        );
    }
    let options = RemoveImageOptionsBuilder::default()
        .force(false)
        .noprune(false)
        .build();
    docker
        .remove_image(&image, Some(options), None)
        .await
        .map_err(|e| format!("docker image removal failed: {e}"))?;
    Ok(serde_json::json!({
        "kind": "docker_runtime_lab_image_removed",
        "image": image,
        "installation_id": installation_id,
        "build_id": build_id,
        "development_change_id": change_set_id,
        "image_removed": true,
        "image_was_present": true,
        "docker_performed": true
    }))
}

fn docker_not_found_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("404") || lower.contains("no such image")
}

async fn inspect_managed_container(
    docker: &Docker,
    container: &str,
    installation_id: &str,
    route_id: &str,
    port_lease_id: &str,
) -> Result<ContainerInspectResponse, String> {
    let options = InspectContainerOptionsBuilder::default()
        .size(false)
        .build();
    let inspected = docker
        .inspect_container(container, Some(options))
        .await
        .map_err(|e| format!("docker inspect failed: {e}"))?;
    let labels = inspected
        .config
        .as_ref()
        .and_then(|config| config.labels.as_ref())
        .ok_or_else(|| "Docker container is missing Plurora ownership labels".to_string())?;
    let expected = [
        ("managed-by", "plurora"),
        ("plurora.package_id", PACKAGE_ID),
        ("plurora.installation_id", installation_id),
        ("plurora.route_id", route_id),
        ("plurora.port_lease_id", port_lease_id),
    ];
    if expected
        .iter()
        .any(|(key, value)| labels.get(*key).map(String::as_str) != Some(*value))
    {
        return Err(
            "Docker container ownership labels do not match the requested Host route".to_string(),
        );
    }
    Ok(inspected)
}

async fn status_async(
    container: String,
    installation_id: String,
    route_id: String,
    port_lease_id: String,
) -> Result<Value, String> {
    let docker = docker().await?;
    let inspected = inspect_managed_container(
        &docker,
        &container,
        &installation_id,
        &route_id,
        &port_lease_id,
    )
    .await?;
    let state = inspected.state;
    let running = state.as_ref().and_then(|s| s.running).unwrap_or(false);
    let status = state
        .as_ref()
        .and_then(|s| s.status.map(|status| status.to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    let exit_code = state.as_ref().and_then(|s| s.exit_code);
    Ok(serde_json::json!({
        "kind": "docker_runtime_lab_status",
        "container_ref": container,
        "installation_id": installation_id,
        "container_id": inspected.id,
        "container_name": inspected.name.map(|name| name.trim_start_matches('/').to_string()),
        "running": running,
        "state": status,
        "exit_code": exit_code,
        "docker_performed": true
    }))
}

async fn logs_async(
    container: String,
    installation_id: String,
    route_id: String,
    port_lease_id: String,
    tail: u32,
    max_bytes: usize,
) -> Result<Value, String> {
    let docker = docker().await?;
    inspect_managed_container(
        &docker,
        &container,
        &installation_id,
        &route_id,
        &port_lease_id,
    )
    .await?;
    let options = LogsOptionsBuilder::default()
        .stdout(true)
        .stderr(true)
        .follow(false)
        .tail(&tail.to_string())
        .build();
    let mut stream = docker.logs(&container, Some(options));
    let mut bytes = Vec::new();
    let mut truncated = false;
    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| format!("docker logs failed: {e}"))?;
        let message = match chunk {
            LogOutput::StdErr { message }
            | LogOutput::StdOut { message }
            | LogOutput::StdIn { message }
            | LogOutput::Console { message } => message,
        };
        let remaining = max_bytes.saturating_sub(bytes.len());
        if message.len() > remaining {
            bytes.extend_from_slice(&message[..remaining]);
            truncated = true;
            break;
        }
        bytes.extend_from_slice(&message);
        if bytes.len() >= max_bytes {
            truncated = true;
            break;
        }
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok(serde_json::json!({
        "kind": "docker_runtime_lab_logs",
        "container_ref": container,
        "installation_id": installation_id,
        "tail": tail,
        "max_bytes": max_bytes,
        "bytes_returned": bytes.len(),
        "truncated": truncated,
        "redaction_applied": true,
        "logs": redact_log_text(&text),
        "docker_performed": true
    }))
}

async fn stop_container_async(
    container: String,
    installation_id: String,
    route_id: String,
    port_lease_id: String,
    timeout_secs: i32,
    force: bool,
) -> Result<Value, String> {
    let docker = docker().await?;
    inspect_managed_container(
        &docker,
        &container,
        &installation_id,
        &route_id,
        &port_lease_id,
    )
    .await?;
    let stop_options = StopContainerOptionsBuilder::default()
        .t(timeout_secs)
        .build();
    let stop_result = docker.stop_container(&container, Some(stop_options)).await;
    let mut stop_error = None;
    if let Err(error) = stop_result {
        let msg = error.to_string();
        if !msg.contains("304") && !msg.to_lowercase().contains("not modified") {
            stop_error = Some(msg);
        }
    }
    let remove_options = RemoveContainerOptionsBuilder::default()
        .force(force)
        .v(false)
        .build();
    docker
        .remove_container(&container, Some(remove_options))
        .await
        .map_err(|e| format!("docker remove failed: {e}"))?;

    Ok(serde_json::json!({
        "kind": "docker_runtime_lab_container_stopped",
        "container_ref": container,
        "installation_id": installation_id,
        "stopped": stop_error.is_none(),
        "removed": true,
        "force": force,
        "timeout_secs": timeout_secs,
        "stop_error": stop_error,
        "docker_performed": true
    }))
}

async fn list_managed_async() -> Result<Value, String> {
    let docker = docker().await?;
    let filters = HashMap::from([(
        "label".to_string(),
        vec![
            "managed-by=plurora".to_string(),
            format!("plurora.package_id={PACKAGE_ID}"),
        ],
    )]);
    let options = ListContainersOptionsBuilder::default()
        .all(true)
        .filters(&filters)
        .build();
    let containers = docker
        .list_containers(Some(options))
        .await
        .map_err(|e| format!("docker list managed failed: {e}"))?;
    let managed: Vec<Value> = containers
        .iter()
        .filter_map(managed_container_json)
        .collect();
    let count = managed.len();
    Ok(serde_json::json!({
        "kind": "docker_runtime_lab_managed_containers",
        "managed": managed,
        "count": count,
        "docker_performed": true
    }))
}

fn managed_container_json(container: &ContainerSummary) -> Option<Value> {
    let labels = container.labels.as_ref()?;
    if labels.get("managed-by").map(String::as_str) != Some("plurora")
        || labels.get("plurora.package_id").map(String::as_str) != Some(PACKAGE_ID)
    {
        return None;
    }
    let installation_id = labels.get("plurora.installation_id")?.clone();
    if InstallationId::parse(installation_id.as_str()).is_err() {
        return None;
    }
    let route_id = labels.get("plurora.route_id")?.clone();
    let port_lease_id = labels.get("plurora.port_lease_id")?.clone();
    let operation_id = labels.get("plurora.deployment_operation_id").cloned();
    let running = matches!(container.state, Some(ContainerSummaryStateEnum::RUNNING));
    let host_port = container
        .ports
        .as_ref()
        .and_then(|ports| ports.iter().find_map(|port| port.public_port));
    let container_id = container.id.clone();
    let container_name = container
        .names
        .as_ref()
        .and_then(|names| names.first())
        .map(|name| name.trim_start_matches('/').to_string());
    Some(serde_json::json!({
        "container_id": container_id,
        "container_name": container_name,
        "installation_id": installation_id,
        "route_id": route_id,
        "port_lease_id": port_lease_id,
        "operation_id": operation_id,
        "running": running,
        "host_port": host_port
    }))
}

#[derive(Debug)]
struct Diagnostic {
    severity: &'static str,
    code: &'static str,
    message: String,
}

fn validate_input(input: &Value) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let Some(image) = string_field(input, "image") else {
        diagnostics.push(error("image_required", "image is required"));
        return diagnostics;
    };
    if !valid_image(&image) {
        diagnostics.push(error(
            "image_invalid",
            "image must be a safe docker image reference without whitespace or shell metacharacters",
        ));
    }
    match string_field(input, "installation_id") {
        Some(value) if InstallationId::parse(value.as_str()).is_ok() => {}
        _ => diagnostics.push(error(
            "installation_id_invalid",
            "installation_id is required and must be an opaque UUID",
        )),
    }

    for (field, label) in [
        ("container_port", "container_port"),
        ("host_port", "host_port"),
    ] {
        match u16_field(input, field) {
            Some(port) if port > 0 => {}
            _ => diagnostics.push(error(
                "port_invalid",
                format!("{label} must be an integer in 1..=65535"),
            )),
        }
    }

    for field in ["route_id", "port_lease_id"] {
        match string_field(input, field) {
            Some(value) if valid_label_value(&value) => {}
            _ => diagnostics.push(error(
                "label_invalid",
                format!("{field} is required and must be label-safe"),
            )),
        }
    }

    if input
        .get("privileged")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        diagnostics.push(error(
            "privileged_blocked",
            "privileged containers are blocked",
        ));
    }
    if input
        .get("network_mode")
        .and_then(Value::as_str)
        .is_some_and(|mode| mode != "bridge")
    {
        diagnostics.push(error(
            "network_mode_blocked",
            "only bridge network_mode is allowed; host network is blocked",
        ));
    }
    for field in ["binds", "mounts", "volumes"] {
        if input.get(field).is_some_and(|v| !v.is_null()) {
            diagnostics.push(error(
                "mounts_blocked",
                format!("{field} are blocked; host bind mounts are not allowed"),
            ));
        }
    }
    for field in ["env", "environment", "secrets"] {
        if input.get(field).is_some_and(|v| !empty_collection(v)) {
            diagnostics.push(error(
                "env_or_secret_blocked",
                format!("{field} is blocked; arbitrary env/secrets are not accepted"),
            ));
        }
    }
    if input
        .get("publish_all_ports")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        diagnostics.push(error(
            "publish_all_ports_blocked",
            "publish_all_ports is blocked; host_port must come from host lease metadata",
        ));
    }
    diagnostics
}

fn parse_build_image_request(input: &Value) -> Result<BuildImageSpec, String> {
    if has_build_secret_request(input) || contains_raw_build_secret(input) {
        return Err("build-time secrets are not supported; fail-closed".to_string());
    }
    if input.get("nixpacks_binary").is_some() {
        return Err("nixpacks binary override is not supported".to_string());
    }
    if !input
        .get("approved")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err("build_image requires approved: true; fail-closed".to_string());
    }
    let strategy = match string_field(input, "strategy")
        .unwrap_or_else(|| "dockerfile".to_string())
        .as_str()
    {
        "dockerfile" => BuildStrategy::Dockerfile,
        "nixpacks" => BuildStrategy::Nixpacks,
        other => return Err(format!("unsupported build strategy '{other}'")),
    };
    let installation_id =
        string_field(input, "installation_id").ok_or("installation_id is required")?;
    InstallationId::parse(installation_id.as_str())
        .map_err(|_| "installation_id must be a valid installation id".to_string())?;
    let workspace_id = string_field(input, "workspace_id").ok_or("workspace_id is required")?;
    let workspace_id = WorkspaceId::parse(workspace_id.as_str())
        .map_err(|_| "workspace_id must be an opaque UUID".to_string())?;
    if input.get("context_dir").is_some() {
        return Err("context_dir is not accepted; use workspace_id".to_string());
    }
    let build_id = string_field(input, "build_id").ok_or("build_id is required")?;
    if !valid_build_id(&build_id) {
        return Err("build_id must be label-safe".to_string());
    }
    let context_scope = if let Some(change_set_id) = string_field(input, "development_change_id") {
        if !valid_development_change_id(&change_set_id) {
            return Err("development_change_id must be a valid Host change id".to_string());
        }
        if strategy != BuildStrategy::Dockerfile {
            return Err(
                "development scratch verification only supports dockerfile strategy".to_string(),
            );
        }
        BuildContextScope::DevelopmentScratch {
            workspace_id: workspace_id.clone(),
            change_set_id,
        }
    } else {
        BuildContextScope::Workspace {
            workspace_id: workspace_id.clone(),
        }
    };
    let network_mode = match input.get("network_mode").and_then(Value::as_str) {
        Some("none") => BuildNetworkMode::None,
        Some("bridge") => BuildNetworkMode::Bridge,
        Some(_) => return Err("network_mode must be 'none' or 'bridge'".to_string()),
        None if matches!(context_scope, BuildContextScope::DevelopmentScratch { .. }) => {
            BuildNetworkMode::None
        }
        None => BuildNetworkMode::Bridge,
    };
    let dockerfile = string_field(input, "dockerfile").unwrap_or_else(|| "Dockerfile".to_string());
    validate_dockerfile_path(&dockerfile)?;
    let source_commit = optional_labelish(input, "source_commit")?;
    let build_descriptor_hash = optional_labelish(input, "build_descriptor_hash")?;
    let expected_context_digest = input
        .get("expected_context_digest")
        .and_then(Value::as_str)
        .map(str::to_string);
    if expected_context_digest
        .as_deref()
        .is_some_and(|digest| !is_sha256_digest(digest))
    {
        return Err("expected_context_digest must be a lowercase sha256 digest".to_string());
    }
    let build_args = parse_build_args(input.get("build_args"))?;
    let max_context_bytes = input
        .get("max_context_bytes")
        .and_then(Value::as_u64)
        .map(|n| n.min(DEFAULT_MAX_CONTEXT_BYTES))
        .unwrap_or(DEFAULT_MAX_CONTEXT_BYTES);
    let max_context_files = input
        .get("max_context_files")
        .and_then(Value::as_u64)
        .map(|n| n.min(DEFAULT_MAX_CONTEXT_FILES))
        .unwrap_or(DEFAULT_MAX_CONTEXT_FILES);
    let build_timeout_secs = input
        .get("build_timeout_secs")
        .and_then(Value::as_u64)
        .map(|n| n.clamp(1, MAX_BUILD_TIMEOUT_SECS))
        .unwrap_or(DEFAULT_BUILD_TIMEOUT_SECS);

    Ok(BuildImageSpec {
        strategy,
        installation_id,
        workspace_id,
        build_id,
        context_scope,
        network_mode,
        dockerfile,
        source_commit,
        build_descriptor_hash,
        expected_context_digest,
        build_args,
        max_context_bytes,
        max_context_files,
        build_timeout_secs,
    })
}

pub fn prepare_docker_build_context(input: &Value) -> anyhow::Result<PreparedDockerBuildContext> {
    let spec = parse_build_image_request(input).map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        spec.strategy == BuildStrategy::Dockerfile,
        "deployable build contexts only support dockerfile strategy"
    );
    let prepared = prepare_build_context(&spec).map_err(anyhow::Error::msg)?;
    verified_context_digest(&spec, &prepared.context).map_err(anyhow::Error::msg)?;
    Ok(PreparedDockerBuildContext {
        bytes: prepared.context.bytes,
        files: prepared.context.files,
        total_bytes: prepared.context.total_bytes,
    })
}

fn verified_context_digest(spec: &BuildImageSpec, context: &ContextTar) -> Result<String, String> {
    let digest = sha256_digest(&context.bytes);
    if spec
        .expected_context_digest
        .as_deref()
        .is_some_and(|expected| expected != digest)
    {
        return Err("build context digest did not match the verified artifact".to_string());
    }
    Ok(digest)
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_development_change_id(value: &str) -> bool {
    value.len() >= 12
        && value.len() <= 64
        && value.starts_with("chg-")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.contains("..")
}

fn contains_raw_build_secret(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            if key == "context_dir" {
                return false;
            }
            contains_raw_build_secret(value)
        }),
        Value::Array(items) => items.iter().any(contains_raw_build_secret),
        Value::String(value) => safety::looks_like_raw_secret_value(value),
        _ => false,
    }
}

fn has_build_secret_request(input: &Value) -> bool {
    const BLOCKED: &[&str] = &[
        "secret",
        "secrets",
        "secret_refs",
        "build_secrets",
        "build_secret",
    ];
    match input {
        Value::Object(map) => map.iter().any(|(key, value)| {
            BLOCKED
                .iter()
                .any(|blocked| key.eq_ignore_ascii_case(blocked))
                || has_build_secret_request(value)
        }),
        Value::Array(items) => items.iter().any(has_build_secret_request),
        Value::String(value) => safety::is_secret_ref_value(value),
        _ => false,
    }
}

fn validate_dockerfile_path(path: &str) -> Result<(), String> {
    if path.trim().is_empty() || path.len() > 255 {
        return Err("dockerfile must be non-empty and at most 255 bytes".to_string());
    }
    let path = Path::new(path);
    if path.is_absolute() {
        return Err("dockerfile must be relative".to_string());
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => return Err("dockerfile must not contain parent or special components".to_string()),
        }
    }
    Ok(())
}

fn parse_build_args(value: Option<&Value>) -> Result<HashMap<String, String>, String> {
    let Some(value) = value else {
        return Ok(HashMap::new());
    };
    let object = value
        .as_object()
        .ok_or("build_args must be an object of string values")?;
    if object.len() > 64 {
        return Err("build_args may contain at most 64 entries".to_string());
    }
    let mut args = HashMap::new();
    for (key, value) in object {
        if !valid_build_arg_key(key) {
            return Err(format!("build arg key '{key}' is invalid"));
        }
        let Some(value) = value.as_str() else {
            return Err("build arg values must be strings".to_string());
        };
        if value.len() > 4096
            || safety::is_secret_ref_value(value)
            || safety::looks_like_raw_secret_value(value)
        {
            return Err(format!(
                "build arg '{key}' contains unsupported secret-like value"
            ));
        }
        args.insert(key.clone(), value.to_string());
    }
    Ok(args)
}

fn valid_build_arg_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 128
        && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !key.to_lowercase().contains("secret")
        && !key.to_lowercase().contains("token")
        && !key.to_lowercase().contains("key")
        && !key.to_lowercase().contains("password")
        && !key.to_lowercase().contains("auth")
}

fn optional_labelish(input: &Value, field: &str) -> Result<Option<String>, String> {
    let Some(value) = string_field(input, field) else {
        return Ok(None);
    };
    if value.len() <= 128 && valid_label_value(&value) {
        Ok(Some(value))
    } else {
        Err(format!("{field} must be label-safe"))
    }
}

fn valid_build_id(value: &str) -> bool {
    value.len() <= 128
        && value.len() >= 3
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && !value.starts_with('-')
        && !value.contains("..")
}

fn image_tag(installation_id: &str, build_id: &str) -> String {
    format!(
        "plurora/{}:{}",
        sanitize_image_component(installation_id, 80),
        sanitize_image_component(build_id, 120)
    )
}

fn sanitize_image_component(value: &str, max_len: usize) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in value.chars() {
        let mapped = if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
            c.to_ascii_lowercase()
        } else {
            '-'
        };
        if mapped == '-' && prev_dash {
            continue;
        }
        prev_dash = mapped == '-';
        out.push(mapped);
        if out.len() >= max_len {
            break;
        }
    }
    let trimmed = out.trim_matches(&['-', '.'][..]).to_string();
    if trimmed.is_empty() {
        "build".to_string()
    } else {
        trimmed
    }
}

fn build_labels(spec: &BuildImageSpec) -> HashMap<String, String> {
    let mut labels = HashMap::from([
        (
            "plurora.installation_id".to_string(),
            spec.installation_id.clone(),
        ),
        (
            "plurora.workspace_id".to_string(),
            spec.workspace_id.to_string(),
        ),
        ("plurora.build_id".to_string(), spec.build_id.clone()),
        (
            "plurora.strategy".to_string(),
            spec.strategy.as_str().to_string(),
        ),
        (
            "plurora.dockerfile_path".to_string(),
            spec.dockerfile.clone(),
        ),
        (
            "plurora.build_network_mode".to_string(),
            spec.network_mode.as_str().to_string(),
        ),
    ]);
    if let BuildContextScope::DevelopmentScratch { change_set_id, .. } = &spec.context_scope {
        labels.insert(
            "plurora.development_change_id".to_string(),
            change_set_id.clone(),
        );
    }
    if spec.strategy == BuildStrategy::Nixpacks {
        labels.insert("plurora.buildpack".to_string(), "nixpacks".to_string());
    }
    if let Some(value) = &spec.source_commit {
        labels.insert("plurora.source_commit".to_string(), value.clone());
    }
    if let Some(value) = &spec.build_descriptor_hash {
        labels.insert("plurora.build_descriptor_hash".to_string(), value.clone());
    }
    labels
}

fn prepare_build_context(spec: &BuildImageSpec) -> Result<PreparedBuildContext, String> {
    let validated_root = validate_build_context_scope(spec)?;
    match spec.strategy {
        BuildStrategy::Dockerfile => Ok(PreparedBuildContext {
            context: create_context_tar_at(spec, &validated_root)?,
            dockerfile: spec.dockerfile.clone(),
            buildpack_version: None,
            generated_dockerfile: None,
        }),
        BuildStrategy::Nixpacks => prepare_nixpacks_context(spec, &validated_root),
    }
}

fn validate_build_context_scope(
    spec: &BuildImageSpec,
) -> Result<ValidatedBuildContextRoot, String> {
    let data_dir = plurora_core::paths::data_dir()
        .map_err(|_| "failed to resolve Plurora data directory".to_string())?;
    let data_dir = canonical_real_directory(&data_dir, "data directory")?;
    let workspaces = canonical_owned_directory(&data_dir, "workspaces", "workspaces root")?;
    let (scope_workspace_id, expected) = match &spec.context_scope {
        BuildContextScope::Workspace { workspace_id } => {
            let workspace =
                canonical_owned_directory(&workspaces, workspace_id.as_str(), "workspace root")?;
            let source = canonical_owned_directory(&workspace, "source", "workspace source")?;
            (workspace_id, source)
        }
        BuildContextScope::DevelopmentScratch {
            workspace_id,
            change_set_id,
        } => {
            if !valid_development_change_id(change_set_id) {
                return Err("development_change_id must be a valid Host change id".to_string());
            }
            let workspace =
                canonical_owned_directory(&workspaces, workspace_id.as_str(), "workspace root")?;
            let development =
                canonical_owned_directory(&workspace, "development", "development root")?;
            let change_root =
                canonical_owned_directory(&development, change_set_id, "development change root")?;
            let scratch =
                canonical_owned_directory(&change_root, "workspace", "scratch workspace")?;
            (workspace_id, scratch)
        }
    };
    if scope_workspace_id != &spec.workspace_id {
        return Err("build context scope did not match workspace_id".to_string());
    }
    let handle = validated_context_directory_handle(&expected, &expected)?;
    Ok(ValidatedBuildContextRoot {
        path: expected,
        handle,
    })
}

fn revalidate_build_context_root(validated: &ValidatedBuildContextRoot) -> Result<PathBuf, String> {
    let current = canonical_real_directory(&validated.path, "build context root")?;
    let current_handle = validated_context_directory_handle(&current, &current)?;
    if current != validated.path || current_handle != validated.handle {
        return Err("build context root changed after Host ownership validation".to_string());
    }
    Ok(current)
}

fn canonical_real_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|e| format!("failed to inspect {label}: {e}"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(format!("{label} must be a real directory, not a symlink"));
    }
    std::fs::canonicalize(path).map_err(|e| format!("failed to canonicalize {label}: {e}"))
}

fn canonical_owned_directory(parent: &Path, name: &str, label: &str) -> Result<PathBuf, String> {
    let path = parent.join(name);
    let canonical = canonical_real_directory(&path, label)?;
    if canonical.parent() != Some(parent) {
        return Err(format!("{label} escaped its Host-owned parent"));
    }
    Ok(canonical)
}

fn prepare_nixpacks_context(
    spec: &BuildImageSpec,
    validated_root: &ValidatedBuildContextRoot,
) -> Result<PreparedBuildContext, String> {
    prepare_nixpacks_context_with_binary(spec, validated_root, NIXPACKS_BINARY)
}

fn prepare_nixpacks_context_with_binary(
    spec: &BuildImageSpec,
    validated_root: &ValidatedBuildContextRoot,
    binary: &str,
) -> Result<PreparedBuildContext, String> {
    validate_nixpacks_binary(binary)?;
    let root = revalidate_build_context_root(validated_root)?;
    let out_dir = root.join(".plurora-nixpacks").join(&spec.build_id);
    if out_dir.exists() {
        std::fs::remove_dir_all(&out_dir)
            .map_err(|e| format!("failed to clear nixpacks output dir: {e}"))?;
    }
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("failed to create nixpacks output dir: {e}"))?;
    let version = nixpacks_version(binary);
    let output = nixpacks_command(binary, &root, &out_dir)
        .output()
        .map_err(|e| format!("nixpacks unavailable or failed to start: {e}"))?;
    if !output.status.success() {
        let stderr = redact_log_text(&String::from_utf8_lossy(&output.stderr));
        let stdout = redact_log_text(&String::from_utf8_lossy(&output.stdout));
        let mut message = format!("nixpacks exited with status {}", output.status);
        if !stderr.trim().is_empty() {
            message.push_str(": ");
            message.push_str(stderr.trim());
        } else if !stdout.trim().is_empty() {
            message.push_str(": ");
            message.push_str(stdout.trim());
        }
        return Err(message);
    }
    let mut generated = spec.clone();
    generated.dockerfile = NIXPACKS_GENERATED_DOCKERFILE.to_string();
    let generated_root = validated_context_root(&out_dir, "generated context")?;
    Ok(PreparedBuildContext {
        context: create_context_tar_at(&generated, &generated_root)?,
        dockerfile: NIXPACKS_GENERATED_DOCKERFILE.to_string(),
        buildpack_version: version,
        generated_dockerfile: Some(NIXPACKS_GENERATED_DOCKERFILE.to_string()),
    })
}

fn validate_nixpacks_binary(binary: &str) -> Result<(), String> {
    if binary.is_empty()
        || binary.contains('/')
        || binary.contains('\\')
        || binary
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b' ')
    {
        return Err("nixpacks binary name is invalid".to_string());
    }
    Ok(())
}

fn nixpacks_command(binary: &str, context_dir: &Path, out_dir: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .arg("build")
        .arg(context_dir)
        .arg("--out")
        .arg(out_dir);
    command
}

fn nixpacks_version(binary: &str) -> Option<String> {
    validate_nixpacks_binary(binary).ok()?;
    let output = Command::new(binary).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!raw.is_empty()).then_some(raw)
}

#[cfg(test)]
fn create_context_tar(spec: &BuildImageSpec, root: &Path) -> Result<ContextTar, String> {
    let root = validated_context_root(root, "test build context")?;
    create_context_tar_at(spec, &root)
}

fn create_context_tar_at(
    spec: &BuildImageSpec,
    validated_root: &ValidatedBuildContextRoot,
) -> Result<ContextTar, String> {
    let root = revalidate_build_context_root(validated_root)?;
    let dockerfile = root.join(&spec.dockerfile);
    let dockerfile_metadata = std::fs::symlink_metadata(&dockerfile)
        .map_err(|e| format!("dockerfile not found or inaccessible: {e}"))?;
    if dockerfile_metadata.file_type().is_symlink() || is_reparse_point(&dockerfile_metadata) {
        return Err("dockerfile must be a regular file without links".to_string());
    }
    let dockerfile = std::fs::canonicalize(&dockerfile)
        .map_err(|e| format!("dockerfile not found or inaccessible: {e}"))?;
    if !dockerfile.starts_with(&root) || !dockerfile.is_file() {
        return Err("dockerfile must resolve inside context_dir".to_string());
    }

    let ignore =
        DockerIgnore::load(&root).map_err(|e| format!("failed to read .dockerignore: {e}"))?;
    let mut tar = tar::Builder::new(Vec::new());
    let mut stats = ContextStats::default();
    add_context_dir(
        &root,
        &validated_root.handle,
        &root,
        &ignore,
        spec,
        &mut stats,
        &mut tar,
    )?;
    revalidate_build_context_root(validated_root)?;
    let bytes = tar
        .into_inner()
        .map_err(|e| format!("failed to finish context tar: {e}"))?;
    Ok(ContextTar {
        bytes,
        files: stats.files,
        total_bytes: stats.total_bytes,
    })
}

fn validated_context_root(path: &Path, label: &str) -> Result<ValidatedBuildContextRoot, String> {
    let path = canonical_real_directory(path, label)?;
    let handle = validated_context_directory_handle(&path, &path)?;
    Ok(ValidatedBuildContextRoot { path, handle })
}

#[derive(Debug, Default)]
struct ContextStats {
    files: u64,
    total_bytes: u64,
}

#[derive(Debug, Default)]
struct DockerIgnore {
    patterns: Vec<String>,
}

impl DockerIgnore {
    fn load(root: &Path) -> std::io::Result<Self> {
        let path = root.join(".dockerignore");
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default())
            }
            Err(error) => return Err(error),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                ".dockerignore must be a regular file",
            ));
        }
        let mut file = std::fs::File::open(&path)?;
        let opened = same_file::Handle::from_file(file.try_clone()?)?;
        let current = std::fs::symlink_metadata(&path)?;
        if !current.is_file()
            || current.file_type().is_symlink()
            || is_reparse_point(&current)
            || same_file::Handle::from_path(&path)? != opened
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                ".dockerignore changed while it was being opened",
            ));
        }
        let mut bytes = Vec::new();
        file.by_ref()
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 1024 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                ".dockerignore exceeds the 1 MiB limit",
            ));
        }
        let raw = String::from_utf8(bytes).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                ".dockerignore must be UTF-8",
            )
        })?;
        let patterns = raw
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('!'))
            .map(|line| {
                line.trim_start_matches('/')
                    .trim_end_matches('/')
                    .to_string()
            })
            .filter(|line| !line.is_empty() && !line.contains(".."))
            .collect();
        Ok(Self { patterns })
    }

    fn is_ignored(&self, relative: &str, file_name: &str) -> bool {
        if matches!(file_name, ".git" | "node_modules" | "target" | ".plurora") {
            return true;
        }
        self.patterns.iter().any(|pattern| {
            relative == pattern
                || relative.starts_with(&format!("{pattern}/"))
                || file_name == pattern
        })
    }
}

fn add_context_dir(
    root: &Path,
    root_handle: &same_file::Handle,
    dir: &Path,
    ignore: &DockerIgnore,
    spec: &BuildImageSpec,
    stats: &mut ContextStats,
    tar: &mut tar::Builder<Vec<u8>>,
) -> Result<(), String> {
    let directory_handle = validated_context_directory_handle(root, dir)?;
    if dir == root && &directory_handle != root_handle {
        return Err("build context root changed before traversal".to_string());
    }
    let mut entries = std::fs::read_dir(dir)
        .map_err(|e| format!("failed to read context dir {}: {e}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("failed to read context entry: {e}"))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        let rel = relative_tar_path(root, &path)?;
        if ignore.is_ignored(&rel, &file_name) {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("failed to stat {}: {e}", path.display()))?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(format!(
                "symlinks are not supported in build context: {rel}"
            ));
        }
        if metadata.is_dir() {
            validated_context_directory_handle(root, &path)?;
            add_context_dir(root, root_handle, &path, ignore, spec, stats, tar)?;
        } else if metadata.is_file() {
            stats.files += 1;
            if stats.files > spec.max_context_files {
                return Err("build context file count limit exceeded".to_string());
            }
            let mut file = std::fs::File::open(&path)
                .map_err(|e| format!("failed to open context file {rel}: {e}"))?;
            let opened_handle = same_file::Handle::from_file(
                file.try_clone()
                    .map_err(|e| format!("failed to clone context file {rel}: {e}"))?,
            )
            .map_err(|e| format!("failed to identify context file {rel}: {e}"))?;
            let opened_metadata = file
                .metadata()
                .map_err(|e| format!("failed to inspect opened context file {rel}: {e}"))?;
            let current_metadata = std::fs::symlink_metadata(&path)
                .map_err(|e| format!("failed to restat context file {rel}: {e}"))?;
            let canonical = std::fs::canonicalize(&path)
                .map_err(|e| format!("failed to canonicalize context file {rel}: {e}"))?;
            if !opened_metadata.is_file()
                || !current_metadata.is_file()
                || current_metadata.file_type().is_symlink()
                || is_reparse_point(&opened_metadata)
                || is_reparse_point(&current_metadata)
                || !canonical.starts_with(root)
                || same_file::Handle::from_path(&path)
                    .map_err(|e| format!("failed to re-identify context file {rel}: {e}"))?
                    != opened_handle
            {
                return Err(format!(
                    "context file changed while it was being opened: {rel}"
                ));
            }
            ensure_single_link_context_file(&opened_metadata)?;
            let remaining = spec
                .max_context_bytes
                .checked_sub(stats.total_bytes)
                .ok_or_else(|| "build context byte limit exceeded".to_string())?;
            let mut data =
                Vec::with_capacity(opened_metadata.len().min(remaining).min(1024 * 1024) as usize);
            file.by_ref()
                .take(remaining.saturating_add(1))
                .read_to_end(&mut data)
                .map_err(|e| format!("failed to read context file {rel}: {e}"))?;
            if data.len() as u64 > remaining {
                return Err("build context byte limit exceeded".to_string());
            }
            let after = std::fs::symlink_metadata(&path)
                .map_err(|e| format!("failed to restat context file {rel}: {e}"))?;
            let after_canonical = std::fs::canonicalize(&path)
                .map_err(|e| format!("failed to recanonicalize context file {rel}: {e}"))?;
            if after.file_type().is_symlink()
                || !after.is_file()
                || is_reparse_point(&after)
                || !after_canonical.starts_with(root)
                || same_file::Handle::from_path(&path)
                    .map_err(|e| format!("failed to re-identify context file {rel}: {e}"))?
                    != opened_handle
                || data.len() as u64 != opened_metadata.len()
            {
                return Err(format!(
                    "context file changed while it was being read: {rel}"
                ));
            }
            stats.total_bytes = stats.total_bytes.saturating_add(data.len() as u64);
            tar_append_file(tar, &rel, &data, data.len() as u64)?;
        } else {
            return Err(format!(
                "special files are not supported in build context: {rel}"
            ));
        }
    }
    if validated_context_directory_handle(root, dir)? != directory_handle {
        return Err(format!(
            "context directory changed during traversal: {}",
            dir.display()
        ));
    }
    Ok(())
}

fn validated_context_directory_handle(
    root: &Path,
    dir: &Path,
) -> Result<same_file::Handle, String> {
    let metadata = std::fs::symlink_metadata(dir)
        .map_err(|e| format!("failed to inspect context directory {}: {e}", dir.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(format!(
            "build context contains a directory symlink: {}",
            dir.display()
        ));
    }
    let canonical = std::fs::canonicalize(dir).map_err(|e| {
        format!(
            "failed to canonicalize context directory {}: {e}",
            dir.display()
        )
    })?;
    if !canonical.starts_with(root) {
        return Err(format!("context directory escaped root: {}", dir.display()));
    }
    same_file::Handle::from_path(dir).map_err(|e| {
        format!(
            "failed to identify context directory {}: {e}",
            dir.display()
        )
    })
}

#[cfg(unix)]
fn ensure_single_link_context_file(metadata: &std::fs::Metadata) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    if metadata.nlink() != 1 {
        return Err("hard-linked files are not supported in build context".to_string());
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_single_link_context_file(_metadata: &std::fs::Metadata) -> Result<(), String> {
    Ok(())
}

fn relative_tar_path(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "context path escaped root".to_string())?;
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("context path must be relative without parent components".to_string());
    }
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn tar_append_file(
    tar: &mut tar::Builder<Vec<u8>>,
    relative: &str,
    data: &[u8],
    size: u64,
) -> Result<(), String> {
    let mut header = tar::Header::new_gnu();
    header.set_size(size);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, relative, data)
        .map_err(|e| format!("failed to append {relative} to context tar: {e}"))
}

fn append_log_tail(tail: &mut String, chunk: &str) {
    tail.push_str(chunk);
    if tail.len() > MAX_BUILD_LOG_BYTES {
        let drop_bytes = tail.len() - MAX_BUILD_LOG_BYTES;
        let drop_at = tail
            .char_indices()
            .find_map(|(idx, _)| (idx >= drop_bytes).then_some(idx))
            .unwrap_or(drop_bytes);
        tail.drain(..drop_at);
    }
}

fn error(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        severity: "error",
        code,
        message: message.into(),
    }
}

fn diagnostics_to_json(diagnostics: &mut [Diagnostic]) -> Vec<Value> {
    diagnostics
        .iter()
        .map(|d| serde_json::json!({"severity": d.severity, "code": d.code, "message": d.message}))
        .collect()
}

fn string_field(input: &Value, field: &str) -> Option<String> {
    input
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn u16_field(input: &Value, field: &str) -> Option<u16> {
    input
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|n| u16::try_from(n).ok())
}

fn value_or_null(value: Option<String>) -> Value {
    value.map(Value::String).unwrap_or(Value::Null)
}

fn empty_collection(value: &Value) -> bool {
    value.is_null()
        || value.as_array().is_some_and(Vec::is_empty)
        || value.as_object().is_some_and(serde_json::Map::is_empty)
}

fn valid_image(image: &str) -> bool {
    image.len() <= 255
        && image.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'.' | b'/' | b':' | b'_' | b'-' | b'@')
        })
        && !image.starts_with('-')
        && !image.contains("..")
}

fn valid_label_value(value: &str) -> bool {
    value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/' | b':'))
}

fn sanitize_name(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    sanitized.trim_matches('-').chars().take(48).collect()
}

fn container_ref(input: &Value) -> Option<String> {
    string_field(input, "container_id")
        .or_else(|| string_field(input, "container_name"))
        .or_else(|| string_field(input, "container"))
}

fn managed_container_scope(input: &Value) -> Result<(String, String, String), String> {
    let installation_id = string_field(input, "installation_id")
        .filter(|value| InstallationId::parse(value.as_str()).is_ok())
        .ok_or_else(|| "installation_id is required and must be an opaque UUID".to_string())?;
    let route_id = string_field(input, "route_id")
        .filter(|value| valid_label_value(value))
        .ok_or_else(|| "route_id is required and must be label-safe".to_string())?;
    let port_lease_id = string_field(input, "port_lease_id")
        .filter(|value| valid_label_value(value))
        .ok_or_else(|| "port_lease_id is required and must be label-safe".to_string())?;
    Ok((installation_id, route_id, port_lease_id))
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

fn provenance(request: &InprocInvocation) -> Value {
    serde_json::json!({
        "package_id": request.provider_package_id,
        "capability_id": request.capability_id
    })
}

fn with_provenance(mut value: Value, request: &InprocInvocation) -> Value {
    if let Some(obj) = value.as_object_mut() {
        obj.insert("provenance".to_string(), provenance(request));
    }
    value
}

fn rejected_output(request: &InprocInvocation) -> Value {
    serde_json::json!({
        "kind": "docker_runtime_lab_rejected",
        "reason": "input contains raw-secret-like content; pass only host-owned metadata and secret_ref references",
        "docker_performed": false,
        "provenance": provenance(request)
    })
}

fn operation_rejected_output(request: &InprocInvocation, operation: &str, reason: &str) -> Value {
    serde_json::json!({
        "kind": "docker_runtime_lab_rejected",
        "operation": operation,
        "reason": reason,
        "docker_performed": false,
        "provenance": provenance(request)
    })
}

fn missing_container_ref_output(request: &InprocInvocation) -> Value {
    serde_json::json!({
        "kind": "docker_runtime_lab_rejected",
        "reason": "container_id or container_name is required",
        "docker_performed": false,
        "provenance": provenance(request)
    })
}

fn docker_error_output(operation: &str, error: String) -> Value {
    serde_json::json!({
        "kind": "docker_runtime_lab_error",
        "operation": operation,
        "docker_performed": false,
        "error": {
            "code": "docker_unavailable_or_failed",
            "message": redact_error_paths(&error)
        }
    })
}

fn redact_error_paths(input: &str) -> String {
    input
        .split_whitespace()
        .map(|token| {
            let windows_absolute = token.len() >= 3
                && token.as_bytes()[0].is_ascii_alphabetic()
                && token.as_bytes()[1] == b':'
                && matches!(token.as_bytes()[2], b'\\' | b'/');
            if token.starts_with('/') || windows_absolute {
                "[PATH_REDACTED]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn redact_log_text(input: &str) -> String {
    input
        .split('\n')
        .map(redact_log_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn redact_log_line(line: &str) -> String {
    let mut out = Vec::new();
    for token in line.split_whitespace() {
        let lower = token.to_lowercase();
        if lower.starts_with("token=")
            || lower.starts_with("secret=")
            || lower.starts_with("password=")
            || lower.starts_with("api_key=")
            || token.starts_with("sk-")
            || token.starts_with("Bearer")
            || safety::looks_like_raw_secret_value(token)
        {
            out.push("[REDACTED]".to_string());
        } else {
            out.push(token.to_string());
        }
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::sync::Mutex;

    use super::*;

    static DATA_DIR_ENV_LOCK: Mutex<()> = Mutex::new(());
    const INSTALLATION_ID: &str = "00000000-0000-4000-8000-000000000001";
    const WORKSPACE_ID: &str = "00000000-0000-4000-8000-000000000002";
    const CHANGE_SET_ID: &str = "chg-0123456789abcdef";

    struct DataDirEnv {
        previous: Option<OsString>,
    }

    impl DataDirEnv {
        fn set(path: &Path) -> Self {
            let previous = std::env::var_os("PLURORA_DATA_DIR");
            std::env::set_var("PLURORA_DATA_DIR", path);
            Self { previous }
        }
    }

    impl Drop for DataDirEnv {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var("PLURORA_DATA_DIR", value),
                None => std::env::remove_var("PLURORA_DATA_DIR"),
            }
        }
    }

    fn build_context_layout(data_dir: &Path, change_set_id: &str) -> (PathBuf, PathBuf) {
        let workspace = data_dir.join("workspaces").join(WORKSPACE_ID);
        let source = workspace.join("source");
        let scratch = workspace
            .join("development")
            .join(change_set_id)
            .join("workspace");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&scratch).unwrap();
        (source, scratch)
    }

    fn build_context_input(change_set_id: Option<&str>) -> Value {
        let mut input = serde_json::json!({
            "approved": true,
            "strategy": "dockerfile",
            "installation_id": INSTALLATION_ID,
            "workspace_id": WORKSPACE_ID,
            "build_id": "build-1",
            "dockerfile": "Dockerfile",
        });
        if let Some(change_set_id) = change_set_id {
            input["development_change_id"] = Value::String(change_set_id.to_string());
        }
        input
    }

    fn context_tar_entries(bytes: &[u8]) -> HashMap<String, Vec<u8>> {
        let mut archive = tar::Archive::new(bytes);
        archive
            .entries()
            .unwrap()
            .map(|entry| {
                let mut entry = entry.unwrap();
                let path = entry.path().unwrap().to_string_lossy().replace('\\', "/");
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).unwrap();
                (path, bytes)
            })
            .collect()
    }

    fn request(capability: &str, input: Value) -> InprocInvocation {
        InprocInvocation {
            capability_id: format!("{PACKAGE_ID}/{capability}"),
            provider_package_id: PACKAGE_ID.to_string(),
            session_id: None,
            input,
        }
    }

    #[test]
    fn docker_runtime_lab_validate_accepts_minimal_safe_spec() {
        let output = validate_spec(&request(
            "validate_spec",
            serde_json::json!({
                "image": "nginx:1.25-alpine",
                "installation_id": INSTALLATION_ID,
                "container_port": 80,
                "host_port": 18080,
                "route_id": "route-test",
                "port_lease_id": "lease-test"
            }),
        ))
        .unwrap();
        assert_eq!(output["valid"], true);
        assert_eq!(output["docker_performed"], false);
    }

    #[test]
    fn docker_runtime_lab_validate_blocks_privileged_host_network_mounts_env() {
        let output = validate_spec(&request(
            "validate_spec",
            serde_json::json!({
                "image": "nginx:latest",
                "installation_id": INSTALLATION_ID,
                "container_port": 80,
                "host_port": 18080,
                "route_id": "route-test",
                "port_lease_id": "lease-test",
                "privileged": true,
                "network_mode": "host",
                "binds": ["/tmp:/tmp"],
                "env": {"DEBUG": "1"}
            }),
        ))
        .unwrap();
        assert_eq!(output["valid"], false);
        let diagnostics = output["diagnostics"].as_array().unwrap();
        assert!(diagnostics
            .iter()
            .any(|d| d["code"] == "privileged_blocked"));
        assert!(diagnostics
            .iter()
            .any(|d| d["code"] == "network_mode_blocked"));
        assert!(diagnostics.iter().any(|d| d["code"] == "mounts_blocked"));
        assert!(diagnostics
            .iter()
            .any(|d| d["code"] == "env_or_secret_blocked"));
    }

    #[test]
    fn docker_runtime_lab_container_effects_require_approval_and_ownership_scope() {
        let unapproved = stop_container(&request(
            "stop_container",
            serde_json::json!({
                "container_id": "container-1",
                "installation_id": INSTALLATION_ID,
                "route_id": "route-1",
                "port_lease_id": "lease-1"
            }),
        ))
        .unwrap();
        assert_eq!(unapproved["kind"], "docker_runtime_lab_rejected");
        assert_eq!(unapproved["docker_performed"], false);

        let unscoped = status(&request(
            "status",
            serde_json::json!({ "container_id": "container-1" }),
        ))
        .unwrap();
        assert_eq!(unscoped["kind"], "docker_runtime_lab_rejected");
        assert_eq!(unscoped["docker_performed"], false);
    }

    #[test]
    fn docker_runtime_lab_plan_is_deterministic_and_no_docker() {
        let output = plan_container(&request(
            "plan_container",
            serde_json::json!({
                "image": "ghcr.io/example/app:sha-abc",
                "installation_id": INSTALLATION_ID,
                "container_port": 3000,
                "host_port": 13000,
                "route_id": "route-1",
                "port_lease_id": "lease-1"
            }),
        ))
        .unwrap();
        assert_eq!(output["kind"], "docker_runtime_lab_container_plan");
        assert_eq!(output["bind_host"], BIND_HOST);
        assert_eq!(output["requires_host_port_lease"], true);
        assert_eq!(output["requires_proxy_route"], true);
        assert_eq!(output["docker_performed"], false);
    }

    #[test]
    fn docker_runtime_lab_list_managed_shape_from_summary() {
        let container = ContainerSummary {
            id: Some("container-000001".to_string()),
            names: Some(vec!["/plurora-container-000001".to_string()]),
            labels: Some(HashMap::from([
                ("managed-by".to_string(), "plurora".to_string()),
                ("plurora.package_id".to_string(), PACKAGE_ID.to_string()),
                (
                    "plurora.installation_id".to_string(),
                    INSTALLATION_ID.to_string(),
                ),
                (
                    "plurora.route_id".to_string(),
                    "proxy-route-000001".to_string(),
                ),
                (
                    "plurora.port_lease_id".to_string(),
                    "port-lease-000001".to_string(),
                ),
                (
                    "plurora.deployment_operation_id".to_string(),
                    "dop-000001".to_string(),
                ),
            ])),
            state: Some(ContainerSummaryStateEnum::RUNNING),
            ports: Some(vec![bollard::models::PortSummary {
                ip: Some(BIND_HOST.to_string()),
                private_port: 3000,
                public_port: Some(39000),
                typ: None,
            }]),
            ..ContainerSummary::default()
        };
        let output = managed_container_json(&container).expect("managed container report");
        assert_eq!(output["container_id"], "container-000001");
        assert_eq!(output["container_name"], "plurora-container-000001");
        assert_eq!(output["route_id"], "proxy-route-000001");
        assert_eq!(output["port_lease_id"], "port-lease-000001");
        assert_eq!(output["operation_id"], "dop-000001");
        assert_eq!(output["running"], true);
        assert_eq!(output["host_port"], 39000);

        let mut target_owned = container;
        let labels = target_owned.labels.as_mut().expect("container labels");
        labels.remove("plurora.package_id");
        labels.insert(
            "plurora.target_driver".to_string(),
            "plurora-target-agent-v1".to_string(),
        );
        assert!(managed_container_json(&target_owned).is_none());
    }

    #[test]
    fn docker_runtime_lab_start_fail_closed_without_approval() {
        let output = start_container(&request(
            "start_container",
            serde_json::json!({
                "image": "nginx:latest",
                "container_port": 80,
                "host_port": 18080,
                "route_id": "route-test",
                "port_lease_id": "lease-test"
            }),
        ))
        .unwrap();
        assert_eq!(output["kind"], "docker_runtime_lab_rejected");
        assert_eq!(output["docker_performed"], false);
    }

    #[test]
    fn docker_runtime_lab_start_still_rejects_env_and_secrets() {
        for input in [
            serde_json::json!({
                "approved": true,
                "image": "nginx:latest",
                "container_port": 80,
                "host_port": 8080,
                "route_id": "route-1",
                "port_lease_id": "lease-1",
                "env": ["TOKEN=value"]
            }),
            serde_json::json!({
                "approved": true,
                "image": "nginx:latest",
                "container_port": 80,
                "host_port": 8080,
                "route_id": "route-1",
                "port_lease_id": "lease-1",
                "secrets": ["secret_ref:env:TOKEN"]
            }),
            serde_json::json!({
                "approved": true,
                "image": "nginx:latest",
                "container_port": 80,
                "host_port": 8080,
                "route_id": "route-1",
                "port_lease_id": "lease-1",
                "mounts": [{"source": "/tmp", "target": "/data", "type": "bind"}]
            }),
            serde_json::json!({
                "approved": true,
                "image": "nginx:latest",
                "container_port": 80,
                "host_port": 8080,
                "route_id": "route-1",
                "port_lease_id": "lease-1",
                "binds": ["/tmp:/data:ro"]
            }),
        ] {
            let output = start_container(&request("start_container", input)).unwrap();
            assert_eq!(output["kind"], "docker_runtime_lab_rejected");
            assert_eq!(output["docker_performed"], false);
        }
    }

    #[test]
    fn docker_runtime_lab_build_image_rejects_unsupported_strategy() {
        let output = build_image(&request(
            "build_image",
            serde_json::json!({
                "approved": true,
                "strategy": "compose",
                "installation_id": INSTALLATION_ID,
                "workspace_id": WORKSPACE_ID,
                "build_id": "build-1",
            }),
        ))
        .unwrap();
        assert_eq!(output["kind"], "docker_runtime_lab_rejected");
        assert_eq!(output["docker_performed"], false);
        assert!(output["reason"].as_str().unwrap().contains("unsupported"));
    }

    #[test]
    fn docker_runtime_lab_build_image_accepts_nixpacks_strategy_shape() {
        let _env_lock = DATA_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().to_path_buf();
        let previous_data_dir = std::env::var_os("PLURORA_DATA_DIR");
        std::env::set_var("PLURORA_DATA_DIR", &data_dir);
        let result = parse_build_image_request(&serde_json::json!({
            "approved": true,
            "strategy": "nixpacks",
            "installation_id": INSTALLATION_ID,
            "workspace_id": WORKSPACE_ID,
            "build_id": "build-1",
        }));
        match previous_data_dir {
            Some(value) => std::env::set_var("PLURORA_DATA_DIR", value),
            None => std::env::remove_var("PLURORA_DATA_DIR"),
        }
        let spec = result.unwrap();
        assert_eq!(spec.strategy, BuildStrategy::Nixpacks);
        assert_eq!(spec.dockerfile, "Dockerfile");
    }

    #[test]
    fn docker_runtime_lab_development_scratch_defaults_to_no_network() {
        let _env_lock = DATA_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().to_path_buf();
        let previous_data_dir = std::env::var_os("PLURORA_DATA_DIR");
        std::env::set_var("PLURORA_DATA_DIR", &data_dir);
        let change_set_id = CHANGE_SET_ID;
        let result = parse_build_image_request(&serde_json::json!({
            "approved": true,
            "strategy": "dockerfile",
            "installation_id": INSTALLATION_ID,
            "workspace_id": WORKSPACE_ID,
            "build_id": "build-1",
            "development_change_id": change_set_id
        }));
        match previous_data_dir {
            Some(value) => std::env::set_var("PLURORA_DATA_DIR", value),
            None => std::env::remove_var("PLURORA_DATA_DIR"),
        }
        let spec = result.unwrap();
        assert_eq!(spec.network_mode, BuildNetworkMode::None);
        assert_eq!(
            spec.context_scope,
            BuildContextScope::DevelopmentScratch {
                workspace_id: WorkspaceId::parse(WORKSPACE_ID).unwrap(),
                change_set_id: change_set_id.to_string()
            }
        );
    }

    #[test]
    fn docker_runtime_lab_development_scratch_rejects_nixpacks() {
        let _env_lock = DATA_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().to_path_buf();
        let previous_data_dir = std::env::var_os("PLURORA_DATA_DIR");
        std::env::set_var("PLURORA_DATA_DIR", &data_dir);
        let change_set_id = CHANGE_SET_ID;
        let result = parse_build_image_request(&serde_json::json!({
            "approved": true,
            "strategy": "nixpacks",
            "installation_id": INSTALLATION_ID,
            "workspace_id": WORKSPACE_ID,
            "build_id": "build-1",
            "development_change_id": change_set_id
        }));
        match previous_data_dir {
            Some(value) => std::env::set_var("PLURORA_DATA_DIR", value),
            None => std::env::remove_var("PLURORA_DATA_DIR"),
        }
        assert!(result
            .unwrap_err()
            .contains("only supports dockerfile strategy"));
    }

    #[test]
    fn docker_runtime_lab_development_build_context_uses_only_exact_scratch() {
        let _env_lock = DATA_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let _data_dir = DataDirEnv::set(temp.path());
        let (source, scratch) = build_context_layout(temp.path(), CHANGE_SET_ID);
        std::fs::write(source.join("source-only.txt"), "source").unwrap();
        std::fs::write(scratch.join("Dockerfile"), "FROM scratch\n").unwrap();
        std::fs::write(scratch.join("scratch-only.txt"), "scratch").unwrap();

        let prepared = prepare_docker_build_context(&build_context_input(Some(CHANGE_SET_ID)))
            .expect("development scratch context prepares without a source Dockerfile");
        let entries = context_tar_entries(&prepared.bytes);
        assert_eq!(entries.get("Dockerfile").unwrap(), b"FROM scratch\n");
        assert_eq!(entries.get("scratch-only.txt").unwrap(), b"scratch");
        assert!(!entries.contains_key("source-only.txt"));

        std::fs::remove_file(scratch.join("Dockerfile")).unwrap();
        std::fs::write(source.join("Dockerfile"), "FROM source\n").unwrap();
        assert!(prepare_docker_build_context(&build_context_input(Some(CHANGE_SET_ID))).is_err());
    }

    #[test]
    fn docker_runtime_lab_workspace_build_context_still_uses_source() {
        let _env_lock = DATA_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let _data_dir = DataDirEnv::set(temp.path());
        let (source, scratch) = build_context_layout(temp.path(), CHANGE_SET_ID);
        std::fs::write(source.join("Dockerfile"), "FROM source\n").unwrap();
        std::fs::write(source.join("source-only.txt"), "source").unwrap();
        std::fs::write(scratch.join("Dockerfile"), "FROM scratch\n").unwrap();
        std::fs::write(scratch.join("scratch-only.txt"), "scratch").unwrap();

        let prepared = prepare_docker_build_context(&build_context_input(None))
            .expect("workspace source context prepares");
        let entries = context_tar_entries(&prepared.bytes);
        assert_eq!(entries.get("Dockerfile").unwrap(), b"FROM source\n");
        assert_eq!(entries.get("source-only.txt").unwrap(), b"source");
        assert!(!entries.contains_key("scratch-only.txt"));
    }

    #[test]
    fn docker_runtime_lab_build_context_rejects_raw_wrong_and_missing_scopes() {
        let _env_lock = DATA_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let _data_dir = DataDirEnv::set(temp.path());
        let (source, scratch) = build_context_layout(temp.path(), CHANGE_SET_ID);
        std::fs::write(source.join("Dockerfile"), "FROM source\n").unwrap();
        std::fs::write(scratch.join("Dockerfile"), "FROM scratch\n").unwrap();

        let mut raw = build_context_input(Some(CHANGE_SET_ID));
        raw["context_dir"] = Value::String(scratch.to_string_lossy().into_owned());
        assert!(prepare_docker_build_context(&raw)
            .unwrap_err()
            .to_string()
            .contains("context_dir is not accepted"));

        let mut invalid_change = build_context_input(Some("chg-../../outside"));
        assert!(prepare_docker_build_context(&invalid_change).is_err());
        invalid_change["development_change_id"] = Value::String("chg-fedcba9876543210".into());
        assert!(prepare_docker_build_context(&invalid_change).is_err());

        let missing_workspace = WorkspaceId::parse("00000000-0000-4000-8000-000000000099").unwrap();
        let mut wrong_workspace = build_context_input(Some(CHANGE_SET_ID));
        wrong_workspace["workspace_id"] = Value::String(missing_workspace.to_string());
        assert!(prepare_docker_build_context(&wrong_workspace).is_err());

        let spec = parse_build_image_request(&build_context_input(Some(CHANGE_SET_ID))).unwrap();
        let mut mismatched = spec.clone();
        mismatched.workspace_id = missing_workspace;
        assert!(validate_build_context_scope(&mismatched)
            .unwrap_err()
            .contains("did not match workspace_id"));

        std::fs::remove_file(scratch.join("Dockerfile")).unwrap();
        std::fs::remove_dir(&scratch).unwrap();
        assert!(prepare_docker_build_context(&build_context_input(Some(CHANGE_SET_ID))).is_err());
    }

    #[test]
    fn docker_runtime_lab_build_context_identity_rejects_path_swap() {
        let _env_lock = DATA_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let _data_dir = DataDirEnv::set(temp.path());
        let (_, scratch) = build_context_layout(temp.path(), CHANGE_SET_ID);
        std::fs::write(scratch.join("Dockerfile"), "FROM original\n").unwrap();
        let spec = parse_build_image_request(&build_context_input(Some(CHANGE_SET_ID))).unwrap();
        let validated = validate_build_context_scope(&spec).unwrap();
        let parked = scratch.with_file_name("workspace-parked");
        match std::fs::rename(&scratch, &parked) {
            Ok(()) => {
                std::fs::create_dir(&scratch).unwrap();
                std::fs::write(scratch.join("Dockerfile"), "FROM replacement\n").unwrap();
                assert!(create_context_tar_at(&spec, &validated)
                    .unwrap_err()
                    .contains("changed after Host ownership validation"));
            }
            Err(_) => {
                create_context_tar_at(&spec, &validated)
                    .expect("a filesystem-blocked swap leaves the pinned context valid");
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn docker_runtime_lab_development_scope_rejects_intermediate_symlink() {
        use std::os::unix::fs::symlink;

        let _env_lock = DATA_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let _data_dir = DataDirEnv::set(temp.path());
        let (source, scratch) = build_context_layout(temp.path(), CHANGE_SET_ID);
        std::fs::write(source.join("Dockerfile"), "FROM source\n").unwrap();
        let development = scratch.parent().unwrap().parent().unwrap().to_path_buf();
        std::fs::remove_dir_all(&development).unwrap();
        let outside = temp.path().join("outside-development");
        let outside_scratch = outside.join(CHANGE_SET_ID).join("workspace");
        std::fs::create_dir_all(&outside_scratch).unwrap();
        std::fs::write(outside_scratch.join("Dockerfile"), "FROM outside\n").unwrap();
        symlink(&outside, &development).unwrap();

        assert!(prepare_docker_build_context(&build_context_input(Some(CHANGE_SET_ID))).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn docker_runtime_lab_development_scope_rejects_intermediate_reparse_point() {
        let _env_lock = DATA_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let _data_dir = DataDirEnv::set(temp.path());
        let (source, scratch) = build_context_layout(temp.path(), CHANGE_SET_ID);
        std::fs::write(source.join("Dockerfile"), "FROM source\n").unwrap();
        let development = scratch.parent().unwrap().parent().unwrap().to_path_buf();
        std::fs::remove_dir_all(&development).unwrap();
        let outside = temp.path().join("outside-development");
        let outside_scratch = outside.join(CHANGE_SET_ID).join("workspace");
        std::fs::create_dir_all(&outside_scratch).unwrap();
        std::fs::write(outside_scratch.join("Dockerfile"), "FROM outside\n").unwrap();
        let output = Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&development)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Windows junction test setup failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(prepare_docker_build_context(&build_context_input(Some(CHANGE_SET_ID))).is_err());
        std::fs::remove_dir(&development).unwrap();
    }

    #[test]
    fn docker_runtime_lab_nixpacks_command_shape_is_fixed_argv() {
        let context = PathBuf::from("/tmp/plurora/context");
        let out = PathBuf::from("/tmp/plurora/context/.plurora-nixpacks/build-1");
        let command = nixpacks_command("nixpacks", &context, &out);
        assert_eq!(command.get_program().to_string_lossy(), "nixpacks");
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            vec![
                "build".to_string(),
                "/tmp/plurora/context".to_string(),
                "--out".to_string(),
                "/tmp/plurora/context/.plurora-nixpacks/build-1".to_string()
            ]
        );
        assert!(validate_nixpacks_binary("nixpacks").is_ok());
        assert!(validate_nixpacks_binary("/bin/nixpacks").is_err());
        assert!(validate_nixpacks_binary("nix packs").is_err());
    }

    #[test]
    fn docker_runtime_lab_nixpacks_unavailable_fails_closed_before_docker() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = BuildImageSpec {
            strategy: BuildStrategy::Nixpacks,
            installation_id: INSTALLATION_ID.to_string(),
            workspace_id: WorkspaceId::parse(WORKSPACE_ID).unwrap(),
            build_id: "build-1".to_string(),
            context_scope: BuildContextScope::Workspace {
                workspace_id: WorkspaceId::parse(WORKSPACE_ID).unwrap(),
            },
            network_mode: BuildNetworkMode::Bridge,
            dockerfile: "Dockerfile".to_string(),
            source_commit: None,
            build_descriptor_hash: None,
            expected_context_digest: None,
            build_args: HashMap::new(),
            max_context_bytes: 1024,
            max_context_files: 10,
            build_timeout_secs: 1,
        };
        let validated_root = validated_context_root(tmp.path(), "test context").unwrap();
        let error = prepare_nixpacks_context_with_binary(
            &spec,
            &validated_root,
            "definitely-not-plurora-nixpacks",
        )
        .unwrap_err();
        assert!(error.contains("nixpacks unavailable") || error.contains("failed to start"));
    }

    #[test]
    fn docker_runtime_lab_build_image_rejects_build_secrets() {
        for input in [
            serde_json::json!({
                "approved": true,
                "installation_id": "00000000-0000-4000-8000-000000000001",
                "build_id": "build-1",
                "workspace_id": WORKSPACE_ID,
                "secrets": ["secret_ref:env:TOKEN"]
            }),
            serde_json::json!({
                "approved": true,
                "installation_id": "00000000-0000-4000-8000-000000000001",
                "build_id": "build-1",
                "workspace_id": WORKSPACE_ID,
                "build_args": {"TOKEN": "secret_ref:env:TOKEN"}
            }),
        ] {
            let output = build_image(&request("build_image", input)).unwrap();
            assert_eq!(output["kind"], "docker_runtime_lab_rejected");
            assert_eq!(output["docker_performed"], false);
        }
    }

    #[test]
    fn docker_runtime_lab_image_tag_sanitizes_installation_and_build() {
        assert_eq!(
            image_tag("My Installation/Alpha", "Build_001"),
            "plurora/my-installation-alpha:build_001"
        );
        assert_eq!(image_tag("***", "---"), "plurora/build:build");
    }

    #[test]
    fn docker_runtime_lab_dockerfile_path_policy() {
        assert!(validate_dockerfile_path("Dockerfile").is_ok());
        assert!(validate_dockerfile_path("docker/Dockerfile").is_ok());
        assert!(validate_dockerfile_path("/tmp/Dockerfile").is_err());
        assert!(validate_dockerfile_path("../Dockerfile").is_err());
        assert!(validate_dockerfile_path("docker/../Dockerfile").is_err());
    }

    #[test]
    fn docker_runtime_lab_context_tar_rejects_symlink_and_oversize() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Dockerfile"), "FROM scratch\n").unwrap();
        std::fs::write(tmp.path().join("app.txt"), "hello").unwrap();
        let spec = BuildImageSpec {
            strategy: BuildStrategy::Dockerfile,
            installation_id: INSTALLATION_ID.to_string(),
            workspace_id: WorkspaceId::parse(WORKSPACE_ID).unwrap(),
            build_id: "build-1".to_string(),
            context_scope: BuildContextScope::Workspace {
                workspace_id: WorkspaceId::parse(WORKSPACE_ID).unwrap(),
            },
            network_mode: BuildNetworkMode::Bridge,
            dockerfile: "Dockerfile".to_string(),
            source_commit: None,
            build_descriptor_hash: None,
            expected_context_digest: None,
            build_args: HashMap::new(),
            max_context_bytes: 1024,
            max_context_files: 10,
            build_timeout_secs: 1,
        };
        let tar = create_context_tar(&spec, tmp.path()).expect("context tar builds");
        assert_eq!(tar.files, 2);
        let digest = verified_context_digest(&spec, &tar).expect("context digest is computed");
        let mut mismatched = spec.clone();
        mismatched.expected_context_digest = Some(format!("sha256:{}", "0".repeat(64)));
        assert_ne!(digest, mismatched.expected_context_digest.clone().unwrap());
        assert!(verified_context_digest(&mismatched, &tar).is_err());
        assert!(tar.total_bytes > 0);

        let mut tiny = spec.clone();
        tiny.max_context_bytes = 1;
        assert!(create_context_tar(&tiny, tmp.path()).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink("app.txt", tmp.path().join("link.txt")).unwrap();
            assert!(create_context_tar(&spec, tmp.path()).is_err());
        }
    }

    #[test]
    fn docker_runtime_lab_context_tar_skips_default_heavy_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Dockerfile"), "FROM scratch\n").unwrap();
        std::fs::create_dir(tmp.path().join("node_modules")).unwrap();
        std::fs::write(tmp.path().join("node_modules/huge.js"), "ignored").unwrap();
        let spec = BuildImageSpec {
            strategy: BuildStrategy::Dockerfile,
            installation_id: INSTALLATION_ID.to_string(),
            workspace_id: WorkspaceId::parse(WORKSPACE_ID).unwrap(),
            build_id: "build-1".to_string(),
            context_scope: BuildContextScope::Workspace {
                workspace_id: WorkspaceId::parse(WORKSPACE_ID).unwrap(),
            },
            network_mode: BuildNetworkMode::Bridge,
            dockerfile: "Dockerfile".to_string(),
            source_commit: None,
            build_descriptor_hash: None,
            expected_context_digest: None,
            build_args: HashMap::new(),
            max_context_bytes: 1024,
            max_context_files: 10,
            build_timeout_secs: 1,
        };
        let tar = create_context_tar(&spec, tmp.path()).expect("context tar builds");
        assert_eq!(tar.files, 1);
    }

    #[test]
    fn docker_runtime_lab_redacts_logs() {
        let redacted = redact_log_text("token=abc password=hunter2 sk-test Bearer secret safe");
        assert!(redacted.contains("[REDACTED]"));
        assert!(!redacted.contains("hunter2"));
        assert!(redacted.contains("safe"));
    }

    #[tokio::test]
    #[ignore]
    async fn docker_runtime_lab_real_docker_smoke_env_gated() {
        if std::env::var("PLURORA_DOCKER_RUNTIME_LAB_SMOKE")
            .ok()
            .as_deref()
            != Some("1")
        {
            return;
        }
        let _ = docker().await.expect("docker daemon available");
    }
}
