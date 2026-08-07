use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Component, Path};
use std::time::Duration;

use anyhow::Context;
use bollard::body_full;
use bollard::models::{
    ContainerCreateBody, ContainerSummary, ContainerSummaryStateEnum, HostConfig, PortBinding,
    PortMap,
};
use bollard::query_parameters::{
    BuildImageOptionsBuilder, CreateContainerOptionsBuilder, CreateImageOptionsBuilder,
    ListContainersOptionsBuilder, RemoveContainerOptionsBuilder, StopContainerOptionsBuilder,
};
use bollard::Docker;
use bytes::Bytes;
use futures::StreamExt;
use serde::{Deserialize, Serialize, Serializer};
use sha2::{Digest, Sha256};
use thiserror::Error;

const BIND_HOST: &str = "127.0.0.1";
const DRIVER_ID: &str = "plurora-target-agent-v1";
const DOCKER_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DOCKER_EFFECT_TIMEOUT: Duration = Duration::from_secs(60);
const DOCKER_PULL_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const DOCKER_BUILD_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const READINESS_TIMEOUT: Duration = Duration::from_secs(30);
const READINESS_INTERVAL: Duration = Duration::from_millis(250);
const READINESS_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_BUILD_CONTEXT_BYTES: usize = 256 * 1024 * 1024;
const MAX_BUILD_CONTEXT_FILES: u64 = 25_000;

#[derive(Debug, Error)]
#[error("managed target deployment outcome is unknown after {stage}")]
pub struct ManagedTargetDeploymentOutcomeUnknown {
    stage: &'static str,
}

pub fn is_managed_target_deployment_outcome_unknown(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<ManagedTargetDeploymentOutcomeUnknown>()
            .is_some()
    })
}

fn outcome_unknown(stage: &'static str) -> anyhow::Error {
    ManagedTargetDeploymentOutcomeUnknown { stage }.into()
}

pub fn managed_target_deployment_outcome_unknown(stage: &'static str) -> anyhow::Error {
    outcome_unknown(stage)
}

mod docker_container_id {
    use super::*;

    pub fn serialize<S>(container_id: &String, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("docker:{container_id}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedTargetDeploymentApply {
    pub target_id: String,
    pub project_id: String,
    pub deployment_id: String,
    pub route_id: String,
    pub port_lease_id: String,
    pub port_name: String,
    pub image: String,
    pub container_port: u16,
    pub requested_host_port: Option<u16>,
    pub pull_if_missing: bool,
    pub operation_id: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManagedTargetBuildNetworkMode {
    None,
    Bridge,
}

impl ManagedTargetBuildNetworkMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bridge => "bridge",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedTargetImageBuild {
    pub target_id: String,
    pub project_id: String,
    pub build_id: String,
    pub dockerfile: String,
    pub network_mode: ManagedTargetBuildNetworkMode,
    pub source_tree_digest: String,
    pub build_descriptor_hash: String,
    pub context_digest: String,
    pub context_tar: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManagedTargetImageBuildReceipt {
    pub image: String,
    pub image_id: String,
    pub build_id: String,
    pub context_digest: String,
    pub source_tree_digest: String,
    pub build_descriptor_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedTargetDeploymentRef {
    pub target_id: String,
    pub project_id: String,
    pub deployment_id: String,
    pub route_id: String,
    pub port_lease_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManagedTargetDeploymentObservation {
    #[serde(skip_serializing)]
    pub target_id: String,
    #[serde(skip_serializing)]
    pub project_id: String,
    #[serde(skip_serializing)]
    pub deployment_id: String,
    #[serde(skip_serializing)]
    pub route_id: String,
    #[serde(skip_serializing)]
    pub port_lease_id: String,
    #[serde(skip_serializing)]
    pub port_name: String,
    #[serde(serialize_with = "docker_container_id::serialize")]
    pub container_id: String,
    pub container_name: String,
    #[serde(skip_serializing)]
    pub image: String,
    pub image_id: Option<String>,
    pub container_port: u16,
    pub host_port: u16,
    pub bind_host: String,
    pub running: bool,
    pub state: String,
    #[serde(skip_serializing)]
    pub owner_operation_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManagedTargetDeploymentDrainReceipt {
    pub deployment: Option<ManagedTargetDeploymentObservation>,
    pub stopped: bool,
    pub grace_seconds: u16,
    pub container_retained: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManagedTargetDeploymentStopReceipt {
    pub stopped: bool,
    pub removed: bool,
    pub force_remove: bool,
    pub grace_seconds: u16,
}

pub async fn validate_managed_target_deployment_runtime() -> anyhow::Result<()> {
    docker().await.map(|_| ())
}

pub async fn build_managed_target_image(
    request: ManagedTargetImageBuild,
) -> anyhow::Result<ManagedTargetImageBuildReceipt> {
    validate_image_build_request(&request)?;
    validate_build_context_tar(&request.context_tar, &request.dockerfile)?;
    anyhow::ensure!(
        crate::sha256_digest(&request.context_tar) == request.context_digest,
        "managed target build context digest did not match"
    );

    let docker = docker().await?;
    let image = target_image_tag(&request.project_id, &request.build_id);
    let labels = HashMap::from([
        ("managed-by".to_string(), "plurora".to_string()),
        ("plurora.target_driver".to_string(), DRIVER_ID.to_string()),
        ("plurora.target_id".to_string(), request.target_id.clone()),
        ("plurora.project_id".to_string(), request.project_id.clone()),
        ("plurora.build_id".to_string(), request.build_id.clone()),
        (
            "plurora.source_tree_digest".to_string(),
            request.source_tree_digest.clone(),
        ),
        (
            "plurora.build_context_digest".to_string(),
            request.context_digest.clone(),
        ),
        (
            "plurora.build_descriptor_hash".to_string(),
            request.build_descriptor_hash.clone(),
        ),
    ]);
    let options = BuildImageOptionsBuilder::default()
        .dockerfile(&request.dockerfile)
        .t(&image)
        .q(false)
        .rm(true)
        .forcerm(true)
        .memory(1024 * 1024 * 1024)
        .cpuquota(100_000)
        .networkmode(request.network_mode.as_str())
        .labels(&labels)
        .build();
    let build = async {
        let mut stream = docker.build_image(
            options,
            None,
            Some(body_full(Bytes::from(request.context_tar))),
        );
        while let Some(item) = stream.next().await {
            let item = item.context("managed target Docker build failed")?;
            anyhow::ensure!(
                item.error_detail
                    .and_then(|detail| detail.message)
                    .is_none(),
                "managed target Docker build failed"
            );
        }
        Ok::<_, anyhow::Error>(())
    };
    tokio::time::timeout(DOCKER_BUILD_TIMEOUT, build)
        .await
        .context("managed target Docker build timed out")??;
    let inspected = docker
        .inspect_image(&image)
        .await
        .context("managed target built image inspect failed")?;
    let image_id = inspected
        .id
        .context("managed target built image has no content-addressable id")?;
    let actual_labels = inspected
        .config
        .and_then(|config| config.labels)
        .context("managed target built image has no provenance labels")?;
    for (name, expected) in &labels {
        anyhow::ensure!(
            actual_labels.get(name) == Some(expected),
            "managed target built image provenance label mismatch"
        );
    }
    Ok(ManagedTargetImageBuildReceipt {
        image,
        image_id,
        build_id: request.build_id,
        context_digest: request.context_digest,
        source_tree_digest: request.source_tree_digest,
        build_descriptor_hash: request.build_descriptor_hash,
    })
}

pub async fn wait_for_managed_target_deployment_readiness(
    deployment: &ManagedTargetDeploymentObservation,
    health_path: Option<&str>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        deployment.running && deployment.bind_host == BIND_HOST && deployment.host_port > 0,
        "managed target deployment is not a running loopback candidate"
    );
    if let Some(path) = health_path {
        validate_health_path(path)?;
    }
    let deadline = tokio::time::Instant::now() + READINESS_TIMEOUT;
    loop {
        match probe_managed_target_deployment(deployment.host_port, health_path).await {
            Ok(()) => return Ok(()),
            Err(error) if tokio::time::Instant::now() >= deadline => {
                return Err(error.context("managed target readiness deadline expired"));
            }
            Err(_) => tokio::time::sleep(READINESS_INTERVAL).await,
        }
    }
}

pub async fn apply_managed_target_deployment(
    request: &ManagedTargetDeploymentApply,
) -> anyhow::Result<ManagedTargetDeploymentObservation> {
    validate_apply_request(request)?;
    let docker = docker().await?;
    if let Some(container) = find_target_container(&docker, &request.reference()).await? {
        let labels = validated_ownership_labels(&request.reference(), &container)?;
        anyhow::ensure!(
            labels.get("plurora.deployment_operation_id") == Some(&request.operation_id)
                && labels.get("plurora.port_name") == Some(&request.port_name)
                && labels.get("plurora.image_ref") == Some(&request.image)
                && labels.get("plurora.container_port")
                    == Some(&request.container_port.to_string()),
            "existing managed deployment conflicts with the requested operation"
        );
        if !matches!(container.state, Some(ContainerSummaryStateEnum::RUNNING)) {
            let container_id = container
                .id
                .as_deref()
                .context("managed deployment has no container id")?;
            let started = tokio::time::timeout(
                DOCKER_EFFECT_TIMEOUT,
                docker.start_container(container_id, None),
            )
            .await;
            if !matches!(started, Ok(Ok(()))) {
                if let Ok(observation) =
                    find_required_target_deployment(&docker, &request.reference()).await
                {
                    if observation.running {
                        validate_requested_host_port(request, &observation)?;
                        return Ok(observation);
                    }
                }
                return Err(outcome_unknown("docker container start"));
            }
            let observation = find_required_target_deployment(&docker, &request.reference())
                .await
                .map_err(|_| outcome_unknown("docker container start verification"))?;
            validate_requested_host_port(request, &observation)?;
            return Ok(observation);
        }
        let observation = find_required_target_deployment(&docker, &request.reference()).await?;
        validate_requested_host_port(request, &observation)?;
        return Ok(observation);
    }

    if request.pull_if_missing {
        let options = CreateImageOptionsBuilder::default()
            .from_image(&request.image)
            .build();
        tokio::time::timeout(DOCKER_PULL_TIMEOUT, async {
            let mut stream = docker.create_image(Some(options), None, None);
            while let Some(item) = stream.next().await {
                item.context("docker image pull failed")?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("docker image pull timed out")??;
    }
    let inspected_image =
        tokio::time::timeout(DOCKER_EFFECT_TIMEOUT, docker.inspect_image(&request.image))
            .await
            .context("docker image inspect timed out")??;
    let image_id = inspected_image
        .id
        .context("docker image has no content-addressable id")?;

    let container_port_key = format!("{}/tcp", request.container_port);
    let mut port_bindings: PortMap = HashMap::new();
    port_bindings.insert(
        container_port_key.clone(),
        Some(vec![PortBinding {
            host_ip: Some(BIND_HOST.to_string()),
            host_port: Some(
                request
                    .requested_host_port
                    .map(|port| port.to_string())
                    .unwrap_or_default(),
            ),
        }]),
    );
    let labels = deployment_labels(request);
    let config = ContainerCreateBody {
        image: Some(image_id),
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
    let container_name = deployment_container_name(
        &request.target_id,
        &request.project_id,
        &request.deployment_id,
    );
    let options = CreateContainerOptionsBuilder::default()
        .name(&container_name)
        .build();
    let created = match tokio::time::timeout(
        DOCKER_EFFECT_TIMEOUT,
        docker.create_container(Some(options), config),
    )
    .await
    {
        Ok(Ok(created)) => created,
        Ok(Err(_)) | Err(_) => {
            if let Ok(observation) =
                find_required_target_deployment(&docker, &request.reference()).await
            {
                if observation.running && observation.owner_operation_id == request.operation_id {
                    validate_requested_host_port(request, &observation)?;
                    return Ok(observation);
                }
            }
            return Err(outcome_unknown("docker container create"));
        }
    };
    let started = tokio::time::timeout(
        DOCKER_EFFECT_TIMEOUT,
        docker.start_container(&created.id, None),
    )
    .await;
    if !matches!(started, Ok(Ok(()))) {
        if let Ok(observation) =
            find_required_target_deployment(&docker, &request.reference()).await
        {
            if observation.running {
                validate_requested_host_port(request, &observation)?;
                return Ok(observation);
            }
        }
        return Err(outcome_unknown("docker container start"));
    }
    let observation = find_required_target_deployment(&docker, &request.reference())
        .await
        .map_err(|_| outcome_unknown("docker container start verification"))?;
    validate_requested_host_port(request, &observation)?;
    Ok(observation)
}

pub async fn observe_managed_target_deployment(
    reference: &ManagedTargetDeploymentRef,
) -> anyhow::Result<Option<ManagedTargetDeploymentObservation>> {
    validate_reference(reference)?;
    let docker = docker().await?;
    Ok(find_target_deployment(&docker, reference)
        .await?
        .map(|(_, observation)| observation))
}

pub async fn drain_managed_target_deployment(
    reference: &ManagedTargetDeploymentRef,
    grace_seconds: u16,
) -> anyhow::Result<ManagedTargetDeploymentDrainReceipt> {
    validate_reference(reference)?;
    anyhow::ensure!(grace_seconds <= 300, "deployment grace period is too large");
    let docker = docker().await?;
    let Some((_, observation)) = find_target_deployment(&docker, reference).await? else {
        return Ok(ManagedTargetDeploymentDrainReceipt {
            deployment: None,
            stopped: true,
            grace_seconds,
            container_retained: false,
        });
    };
    if observation.running {
        let options = StopContainerOptionsBuilder::default()
            .t(i32::from(grace_seconds))
            .build();
        let stopped = tokio::time::timeout(
            Duration::from_secs(u64::from(grace_seconds).saturating_add(30)),
            docker.stop_container(&observation.container_id, Some(options)),
        )
        .await;
        if !matches!(stopped, Ok(Ok(()))) {
            match find_target_deployment(&docker, reference).await {
                Ok(Some((_, current))) if !current.running => {}
                _ => return Err(outcome_unknown("docker container drain")),
            }
        }
    }
    let after = match find_target_deployment(&docker, reference).await {
        Ok(Some((_, current))) if !current.running => current,
        _ => return Err(outcome_unknown("docker container drain verification")),
    };
    Ok(ManagedTargetDeploymentDrainReceipt {
        deployment: Some(after),
        stopped: true,
        grace_seconds,
        container_retained: true,
    })
}

pub async fn stop_managed_target_deployment(
    reference: &ManagedTargetDeploymentRef,
    grace_seconds: u16,
    force_remove: bool,
) -> anyhow::Result<ManagedTargetDeploymentStopReceipt> {
    validate_reference(reference)?;
    anyhow::ensure!(grace_seconds <= 300, "deployment grace period is too large");
    let docker = docker().await?;
    let Some((_, observation)) = find_target_deployment(&docker, reference).await? else {
        return Ok(ManagedTargetDeploymentStopReceipt {
            stopped: true,
            removed: true,
            force_remove,
            grace_seconds,
        });
    };
    if observation.running {
        let options = StopContainerOptionsBuilder::default()
            .t(i32::from(grace_seconds))
            .build();
        let stop = tokio::time::timeout(
            Duration::from_secs(u64::from(grace_seconds).saturating_add(30)),
            docker.stop_container(&observation.container_id, Some(options)),
        )
        .await;
        match stop {
            Ok(Ok(())) => {}
            Ok(Err(_)) | Err(_) if force_remove => {}
            Ok(Err(_)) | Err(_) => match find_target_deployment(&docker, reference).await {
                Ok(Some((_, current))) if !current.running => {}
                _ => return Err(outcome_unknown("docker container stop")),
            },
        }
    }
    let options = RemoveContainerOptionsBuilder::default()
        .force(force_remove)
        .v(false)
        .build();
    let removed = tokio::time::timeout(
        DOCKER_EFFECT_TIMEOUT,
        docker.remove_container(&observation.container_id, Some(options)),
    )
    .await;
    if !matches!(removed, Ok(Ok(()))) {
        match find_target_container(&docker, reference).await {
            Ok(None) => {}
            _ => return Err(outcome_unknown("docker container removal")),
        }
    }
    Ok(ManagedTargetDeploymentStopReceipt {
        stopped: true,
        removed: true,
        force_remove,
        grace_seconds,
    })
}

pub async fn count_managed_target_deployments(target_id: &str) -> anyhow::Result<u64> {
    validate_label_value("target_id", target_id)?;
    let docker = docker().await?;
    let filters = HashMap::from([(
        "label".to_string(),
        vec![
            format!("plurora.target_driver={DRIVER_ID}"),
            format!("plurora.target_id={target_id}"),
        ],
    )]);
    let options = ListContainersOptionsBuilder::default()
        .all(true)
        .filters(&filters)
        .build();
    let containers =
        tokio::time::timeout(DOCKER_EFFECT_TIMEOUT, docker.list_containers(Some(options)))
            .await
            .context("docker managed deployment list timed out")??;
    Ok(u64::try_from(containers.len()).unwrap_or(u64::MAX))
}

pub async fn open_managed_target_tunnel_stream(
    target_id: &str,
    route_id: &str,
    port_lease_id: &str,
    port_name: &str,
    host_port: u16,
) -> anyhow::Result<tokio::net::TcpStream> {
    for (name, value) in [
        ("target_id", target_id),
        ("route_id", route_id),
        ("port_lease_id", port_lease_id),
        ("port_name", port_name),
    ] {
        validate_label_value(name, value)?;
    }
    anyhow::ensure!(host_port > 0, "target tunnel port must be non-zero");
    let docker = docker().await?;
    let filters = HashMap::from([(
        "label".to_string(),
        vec![
            format!("plurora.target_driver={DRIVER_ID}"),
            format!("plurora.target_id={target_id}"),
            format!("plurora.route_id={route_id}"),
            format!("plurora.port_lease_id={port_lease_id}"),
            format!("plurora.port_name={port_name}"),
        ],
    )]);
    let options = ListContainersOptionsBuilder::default()
        .all(true)
        .filters(&filters)
        .build();
    let containers =
        tokio::time::timeout(DOCKER_EFFECT_TIMEOUT, docker.list_containers(Some(options)))
            .await
            .context("docker target tunnel lookup timed out")??;
    anyhow::ensure!(
        containers.len() == 1,
        "target tunnel lease does not resolve to exactly one managed deployment"
    );
    let container = &containers[0];
    let labels = container
        .labels
        .as_ref()
        .context("target tunnel deployment has no ownership labels")?;
    for (key, expected) in [
        ("managed-by", "plurora"),
        ("plurora.target_driver", DRIVER_ID),
        ("plurora.target_id", target_id),
        ("plurora.route_id", route_id),
        ("plurora.port_lease_id", port_lease_id),
        ("plurora.port_name", port_name),
    ] {
        anyhow::ensure!(
            labels.get(key).map(String::as_str) == Some(expected),
            "target tunnel deployment ownership label mismatch"
        );
    }
    anyhow::ensure!(
        matches!(container.state, Some(ContainerSummaryStateEnum::RUNNING)),
        "target tunnel deployment is not running"
    );
    let container_port = labels
        .get("plurora.container_port")
        .context("target tunnel deployment has no container port label")?
        .parse::<u16>()?;
    let matching_port = container
        .ports
        .as_ref()
        .into_iter()
        .flatten()
        .find(|port| port.private_port == container_port && port.public_port == Some(host_port))
        .context("target tunnel port is not published by the managed deployment")?;
    anyhow::ensure!(
        matching_port.ip.as_deref() == Some(BIND_HOST),
        "target tunnel port is not loopback-only"
    );
    tokio::time::timeout(
        DOCKER_CONNECT_TIMEOUT,
        tokio::net::TcpStream::connect((BIND_HOST, host_port)),
    )
    .await
    .context("target tunnel loopback connect timed out")?
    .context("target tunnel loopback connect failed")
}

impl ManagedTargetDeploymentApply {
    fn reference(&self) -> ManagedTargetDeploymentRef {
        ManagedTargetDeploymentRef {
            target_id: self.target_id.clone(),
            project_id: self.project_id.clone(),
            deployment_id: self.deployment_id.clone(),
            route_id: self.route_id.clone(),
            port_lease_id: self.port_lease_id.clone(),
        }
    }
}

async fn docker() -> anyhow::Result<Docker> {
    let docker = Docker::connect_with_local_defaults()
        .or_else(|_| Docker::connect_with_defaults())
        .context("docker connection unavailable")?;
    tokio::time::timeout(DOCKER_CONNECT_TIMEOUT, docker.ping())
        .await
        .context("docker ping timed out")??;
    Ok(docker)
}

fn validate_image_build_request(request: &ManagedTargetImageBuild) -> anyhow::Result<()> {
    for (name, value) in [
        ("target_id", request.target_id.as_str()),
        ("project_id", request.project_id.as_str()),
        ("build_id", request.build_id.as_str()),
    ] {
        validate_label_value(name, value)?;
    }
    validate_relative_path("dockerfile", &request.dockerfile)?;
    for (name, value) in [
        ("source_tree_digest", request.source_tree_digest.as_str()),
        (
            "build_descriptor_hash",
            request.build_descriptor_hash.as_str(),
        ),
        ("context_digest", request.context_digest.as_str()),
    ] {
        anyhow::ensure!(is_sha256_digest(value), "managed target {name} is invalid");
    }
    anyhow::ensure!(
        !request.context_tar.is_empty() && request.context_tar.len() <= MAX_BUILD_CONTEXT_BYTES,
        "managed target build context size is invalid"
    );
    Ok(())
}

fn validate_build_context_tar(bytes: &[u8], dockerfile: &str) -> anyhow::Result<()> {
    let mut archive = tar::Archive::new(Cursor::new(bytes));
    let mut files = 0u64;
    let mut dockerfile_present = false;
    for entry in archive
        .entries()
        .context("managed target build context is not a tar archive")?
    {
        let entry = entry.context("managed target build context entry is invalid")?;
        let kind = entry.header().entry_type();
        anyhow::ensure!(
            kind.is_file() || kind.is_dir(),
            "managed target build context contains a non-file entry"
        );
        let path = entry
            .path()
            .context("managed target build context path is invalid")?;
        anyhow::ensure!(
            !path.is_absolute()
                && path
                    .components()
                    .all(|component| matches!(component, Component::Normal(_))),
            "managed target build context path escaped its root"
        );
        if kind.is_file() {
            files = files.saturating_add(1);
            anyhow::ensure!(
                files <= MAX_BUILD_CONTEXT_FILES,
                "managed target build context file limit exceeded"
            );
            if path == Path::new(dockerfile) {
                dockerfile_present = true;
            }
        }
    }
    anyhow::ensure!(
        dockerfile_present,
        "managed target build context does not contain its Dockerfile"
    );
    Ok(())
}

fn validate_relative_path(name: &str, value: &str) -> anyhow::Result<()> {
    let path = Path::new(value);
    anyhow::ensure!(
        !value.is_empty()
            && value.len() <= 255
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "managed target {name} is invalid"
    );
    Ok(())
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn target_image_tag(project_id: &str, build_id: &str) -> String {
    format!(
        "plurora/{}:{}",
        sanitize_image_component(project_id, 80),
        sanitize_image_component(build_id, 120)
    )
}

fn sanitize_image_component(value: &str, max_len: usize) -> String {
    let mut output = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .take(max_len)
        .collect::<String>();
    while output.contains("--") {
        output = output.replace("--", "-");
    }
    let output = output.trim_matches(['.', '-']).to_string();
    if output.is_empty() {
        "build".to_string()
    } else {
        output
    }
}

fn validate_health_path(path: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        path.starts_with('/')
            && path.len() <= 256
            && !path.contains(['\r', '\n'])
            && !path.starts_with("//"),
        "managed target health path is invalid"
    );
    Ok(())
}

async fn probe_managed_target_deployment(
    port: u16,
    health_path: Option<&str>,
) -> anyhow::Result<()> {
    tokio::time::timeout(
        READINESS_CONNECT_TIMEOUT,
        tokio::net::TcpStream::connect((BIND_HOST, port)),
    )
    .await
    .context("managed target TCP readiness probe timed out")?
    .context("managed target TCP readiness probe failed")?;
    let Some(path) = health_path else {
        return Ok(());
    };
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(READINESS_CONNECT_TIMEOUT)
        .build()?;
    let status = client
        .get(format!("http://{BIND_HOST}:{port}{path}"))
        .send()
        .await
        .context("managed target HTTP readiness probe failed")?
        .status();
    anyhow::ensure!(
        status.is_success() || status.is_redirection() || status.is_client_error(),
        "managed target HTTP readiness probe returned {status}"
    );
    Ok(())
}

fn validate_apply_request(request: &ManagedTargetDeploymentApply) -> anyhow::Result<()> {
    validate_reference(&request.reference())?;
    validate_label_value("port_name", &request.port_name)?;
    validate_label_value("operation_id", &request.operation_id)?;
    anyhow::ensure!(
        request.container_port > 0,
        "container port must be non-zero"
    );
    anyhow::ensure!(
        request.requested_host_port.is_none_or(|port| port > 0),
        "requested host port must be non-zero"
    );
    anyhow::ensure!(
        valid_image_reference(&request.image),
        "deployment image reference is invalid"
    );
    Ok(())
}

fn validate_requested_host_port(
    request: &ManagedTargetDeploymentApply,
    observation: &ManagedTargetDeploymentObservation,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        request
            .requested_host_port
            .is_none_or(|port| observation.host_port == port),
        "managed deployment actual port conflicts with the request"
    );
    Ok(())
}

fn valid_image_reference(image: &str) -> bool {
    if image.is_empty()
        || image.len() > 512
        || image.contains("://")
        || crate::scan_effect_value_for_raw_secrets(
            &serde_json::json!({ "image": image }),
            "deployment",
        )
        .has_findings()
        || !image
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/@+-".contains(&byte))
    {
        return false;
    }
    let Some((name, digest)) = image.rsplit_once('@') else {
        return true;
    };
    !name.is_empty()
        && !name.contains('@')
        && digest.len() == 71
        && digest.starts_with("sha256:")
        && digest[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_reference(reference: &ManagedTargetDeploymentRef) -> anyhow::Result<()> {
    for (name, value) in [
        ("target_id", reference.target_id.as_str()),
        ("project_id", reference.project_id.as_str()),
        ("deployment_id", reference.deployment_id.as_str()),
        ("route_id", reference.route_id.as_str()),
        ("port_lease_id", reference.port_lease_id.as_str()),
    ] {
        validate_label_value(name, value)?;
    }
    Ok(())
}

fn validate_label_value(name: &str, value: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !value.is_empty()
            && value.len() <= 256
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-._:/".contains(&byte)),
        "deployment {name} is invalid"
    );
    Ok(())
}

fn deployment_labels(request: &ManagedTargetDeploymentApply) -> HashMap<String, String> {
    HashMap::from([
        ("managed-by".to_string(), "plurora".to_string()),
        ("plurora.target_driver".to_string(), DRIVER_ID.to_string()),
        ("plurora.target_id".to_string(), request.target_id.clone()),
        ("plurora.project_id".to_string(), request.project_id.clone()),
        (
            "plurora.deployment_id".to_string(),
            request.deployment_id.clone(),
        ),
        ("plurora.route_id".to_string(), request.route_id.clone()),
        (
            "plurora.port_lease_id".to_string(),
            request.port_lease_id.clone(),
        ),
        ("plurora.port_name".to_string(), request.port_name.clone()),
        ("plurora.image_ref".to_string(), request.image.clone()),
        (
            "plurora.container_port".to_string(),
            request.container_port.to_string(),
        ),
        (
            "plurora.deployment_operation_id".to_string(),
            request.operation_id.clone(),
        ),
    ])
}

fn deployment_container_name(target_id: &str, project_id: &str, deployment_id: &str) -> String {
    let digest = Sha256::digest(format!("{target_id}\0{project_id}\0{deployment_id}").as_bytes());
    format!("plurora-target-{}", &format!("{digest:x}")[..24])
}

async fn find_required_target_deployment(
    docker: &Docker,
    reference: &ManagedTargetDeploymentRef,
) -> anyhow::Result<ManagedTargetDeploymentObservation> {
    find_target_deployment(docker, reference)
        .await?
        .map(|(_, observation)| observation)
        .context("managed deployment disappeared after Docker effect")
}

async fn find_target_deployment(
    docker: &Docker,
    reference: &ManagedTargetDeploymentRef,
) -> anyhow::Result<Option<(ContainerSummary, ManagedTargetDeploymentObservation)>> {
    find_target_container(docker, reference)
        .await?
        .map(|container| {
            let observation = observation_from_summary(reference, &container)?;
            Ok((container, observation))
        })
        .transpose()
}

async fn find_target_container(
    docker: &Docker,
    reference: &ManagedTargetDeploymentRef,
) -> anyhow::Result<Option<ContainerSummary>> {
    validate_reference(reference)?;
    let filters = HashMap::from([(
        "label".to_string(),
        vec![
            format!("plurora.target_driver={DRIVER_ID}"),
            format!("plurora.target_id={}", reference.target_id),
            format!("plurora.project_id={}", reference.project_id),
            format!("plurora.deployment_id={}", reference.deployment_id),
        ],
    )]);
    let options = ListContainersOptionsBuilder::default()
        .all(true)
        .filters(&filters)
        .build();
    let containers =
        tokio::time::timeout(DOCKER_EFFECT_TIMEOUT, docker.list_containers(Some(options)))
            .await
            .context("docker deployment lookup timed out")??;
    anyhow::ensure!(
        containers.len() <= 1,
        "multiple containers claim one target deployment identity"
    );
    let container = containers.into_iter().next();
    if let Some(container) = &container {
        validated_ownership_labels(reference, container)?;
    }
    Ok(container)
}

fn observation_from_summary(
    reference: &ManagedTargetDeploymentRef,
    container: &ContainerSummary,
) -> anyhow::Result<ManagedTargetDeploymentObservation> {
    let labels = validated_ownership_labels(reference, container)?;
    let labels = &labels;
    let container_port = labels
        .get("plurora.container_port")
        .context("managed deployment has no container port label")?
        .parse::<u16>()?;
    let port = container
        .ports
        .as_ref()
        .into_iter()
        .flatten()
        .find(|port| port.private_port == container_port)
        .context("managed deployment has no published port")?;
    let host_port = port
        .public_port
        .context("managed deployment has no actual host port")?;
    anyhow::ensure!(host_port > 0, "managed deployment actual host port is zero");
    let bind_host = port.ip.clone().unwrap_or_default();
    anyhow::ensure!(
        bind_host == BIND_HOST,
        "managed deployment port is not loopback-only"
    );
    Ok(ManagedTargetDeploymentObservation {
        target_id: reference.target_id.clone(),
        project_id: reference.project_id.clone(),
        deployment_id: reference.deployment_id.clone(),
        route_id: reference.route_id.clone(),
        port_lease_id: reference.port_lease_id.clone(),
        port_name: labels
            .get("plurora.port_name")
            .context("managed deployment has no port name label")?
            .clone(),
        container_id: container
            .id
            .clone()
            .context("managed deployment has no container id")?,
        container_name: container
            .names
            .as_ref()
            .and_then(|names| names.first())
            .map(|name| name.trim_start_matches('/').to_string())
            .context("managed deployment has no container name")?,
        image: labels
            .get("plurora.image_ref")
            .context("managed deployment has no image reference label")?
            .clone(),
        image_id: container.image_id.clone(),
        container_port,
        host_port,
        bind_host,
        running: matches!(container.state, Some(ContainerSummaryStateEnum::RUNNING)),
        state: container
            .state
            .map(|state| state.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        owner_operation_id: labels
            .get("plurora.deployment_operation_id")
            .context("managed deployment has no operation label")?
            .clone(),
    })
}

fn validated_ownership_labels<'a>(
    reference: &ManagedTargetDeploymentRef,
    container: &'a ContainerSummary,
) -> anyhow::Result<&'a HashMap<String, String>> {
    let labels = container
        .labels
        .as_ref()
        .context("managed deployment has no ownership labels")?;
    for (key, expected) in [
        ("managed-by", "plurora"),
        ("plurora.target_driver", DRIVER_ID),
        ("plurora.target_id", reference.target_id.as_str()),
        ("plurora.project_id", reference.project_id.as_str()),
        ("plurora.deployment_id", reference.deployment_id.as_str()),
        ("plurora.route_id", reference.route_id.as_str()),
        ("plurora.port_lease_id", reference.port_lease_id.as_str()),
    ] {
        anyhow::ensure!(
            labels.get(key).map(String::as_str) == Some(expected),
            "managed deployment ownership label mismatch"
        );
    }
    Ok(labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_identity_is_deterministic_and_not_caller_controlled() {
        let first = deployment_container_name("target-1", "project-1", "deployment-1");
        assert_eq!(
            first,
            deployment_container_name("target-1", "project-1", "deployment-1")
        );
        assert_ne!(
            first,
            deployment_container_name("target-2", "project-1", "deployment-1")
        );
        assert_ne!(
            first,
            deployment_container_name("target-1", "project-2", "deployment-1")
        );
        assert!(first.starts_with("plurora-target-"));
        assert_eq!(first.len(), "plurora-target-".len() + 24);
    }

    #[test]
    fn apply_validation_rejects_address_and_command_shaped_images() {
        let mut request = ManagedTargetDeploymentApply {
            target_id: "target-1".to_string(),
            project_id: "project-1".to_string(),
            deployment_id: "deployment-1".to_string(),
            route_id: "route-1".to_string(),
            port_lease_id: "lease-1".to_string(),
            port_name: "http".to_string(),
            image: format!("registry.example/app@sha256:{}", "a".repeat(64)),
            container_port: 8080,
            requested_host_port: None,
            pull_if_missing: false,
            operation_id: "operation-1".to_string(),
        };
        validate_apply_request(&request).unwrap();
        request.requested_host_port = Some(0);
        assert!(validate_apply_request(&request).is_err());
        request.requested_host_port = None;
        request.image = "https://registry.example/app".to_string();
        assert!(validate_apply_request(&request).is_err());
        request.image = "app; whoami".to_string();
        assert!(validate_apply_request(&request).is_err());
        request.image = "user@registry.example/app".to_string();
        assert!(validate_apply_request(&request).is_err());
    }

    #[test]
    fn ambiguous_effect_errors_are_distinguishable() {
        let error = outcome_unknown("test effect");
        assert!(is_managed_target_deployment_outcome_unknown(&error));
        assert!(!is_managed_target_deployment_outcome_unknown(
            &anyhow::anyhow!("known failure")
        ));
    }

    #[tokio::test]
    async fn verified_build_context_rejects_non_file_entries_and_wrong_digest() {
        let mut tar = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(13);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "Dockerfile", b"FROM scratch\n".as_slice())
            .unwrap();
        let bytes = tar.into_inner().unwrap();
        validate_build_context_tar(&bytes, "Dockerfile").unwrap();

        let mut invalid_tar = tar::Builder::new(Vec::new());
        let mut link = tar::Header::new_gnu();
        link.set_entry_type(tar::EntryType::Symlink);
        link.set_link_name("Dockerfile").unwrap();
        link.set_size(0);
        link.set_mode(0o777);
        link.set_cksum();
        invalid_tar
            .append_data(&mut link, "Dockerfile.link", std::io::empty())
            .unwrap();
        let invalid_bytes = invalid_tar.into_inner().unwrap();
        assert!(validate_build_context_tar(&invalid_bytes, "Dockerfile").is_err());

        let mut request = ManagedTargetImageBuild {
            target_id: "target-1".to_string(),
            project_id: "project-1".to_string(),
            build_id: "build-1".to_string(),
            dockerfile: "Dockerfile".to_string(),
            network_mode: ManagedTargetBuildNetworkMode::None,
            source_tree_digest: format!("sha256:{}", "a".repeat(64)),
            build_descriptor_hash: format!("sha256:{}", "b".repeat(64)),
            context_digest: crate::sha256_digest(&bytes),
            context_tar: bytes,
        };
        validate_image_build_request(&request).unwrap();
        request.context_digest = format!("sha256:{}", "c".repeat(64));
        let error = build_managed_target_image(request).await.unwrap_err();
        assert!(error.to_string().contains("context digest did not match"));
        assert!(validate_health_path("/ready").is_ok());
        assert!(validate_health_path("//remote.example/").is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn verified_build_context_deploys_on_local_and_agent_targets_smoke() -> anyhow::Result<()>
    {
        if std::env::var("PLURORA_TARGET_DEPLOYMENT_SMOKE")
            .ok()
            .as_deref()
            != Some("1")
        {
            return Ok(());
        }
        let dockerfile = b"FROM nginx:1.27-alpine\n";
        let mut archive = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(dockerfile.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive.append_data(&mut header, "Dockerfile", dockerfile.as_slice())?;
        let context_tar = archive.into_inner()?;
        let context_digest = crate::sha256_digest(&context_tar);
        let suffix = uuid::Uuid::new_v4().simple().to_string();

        for target_id in ["local", "agent-smoke"] {
            let target_suffix = target_id.replace('-', "");
            let project_id = format!("smoke-{target_suffix}-{suffix}");
            let build_id = format!("build-{target_suffix}-{suffix}");
            let build = build_managed_target_image(ManagedTargetImageBuild {
                target_id: target_id.to_string(),
                project_id: project_id.clone(),
                build_id: build_id.clone(),
                dockerfile: "Dockerfile".to_string(),
                network_mode: ManagedTargetBuildNetworkMode::None,
                source_tree_digest: format!("sha256:{}", "a".repeat(64)),
                build_descriptor_hash: format!("sha256:{}", "b".repeat(64)),
                context_digest: context_digest.clone(),
                context_tar: context_tar.clone(),
            })
            .await?;
            let deployment_id = format!("deployment-{target_suffix}-{suffix}");
            let route_id = format!("route-{target_suffix}-{suffix}");
            let lease_id = format!("lease-{target_suffix}-{suffix}");
            let operation_id = format!("operation-{target_suffix}-{suffix}");
            let applied = apply_managed_target_deployment(&ManagedTargetDeploymentApply {
                target_id: target_id.to_string(),
                project_id: project_id.clone(),
                deployment_id: deployment_id.clone(),
                route_id: route_id.clone(),
                port_lease_id: lease_id.clone(),
                port_name: "http".to_string(),
                image: build.image_id.clone(),
                container_port: 80,
                requested_host_port: None,
                pull_if_missing: false,
                operation_id,
            })
            .await?;
            let preview_result = async {
                wait_for_managed_target_deployment_readiness(&applied, Some("/")).await?;
                anyhow::ensure!(
                    applied.image_id.as_deref() == Some(build.image_id.as_str()),
                    "smoke deployment did not use the verified built image"
                );
                Ok::<_, anyhow::Error>(())
            }
            .await;
            let stopped = stop_managed_target_deployment(
                &ManagedTargetDeploymentRef {
                    target_id: target_id.to_string(),
                    project_id,
                    deployment_id,
                    route_id,
                    port_lease_id: lease_id,
                },
                0,
                true,
            )
            .await;
            let remove_options = bollard::query_parameters::RemoveImageOptionsBuilder::default()
                .force(true)
                .noprune(false)
                .build();
            let removed = docker()
                .await?
                .remove_image(&build.image, Some(remove_options), None)
                .await;
            preview_result?;
            stopped?;
            removed?;
            anyhow::ensure!(
                count_managed_target_deployments(target_id).await? == 0,
                "smoke deployment cleanup was incomplete"
            );
        }
        Ok(())
    }

    #[test]
    fn receipt_encodes_container_identity_as_a_typed_non_secret_reference() {
        let observation = ManagedTargetDeploymentObservation {
            target_id: "target-1".to_string(),
            project_id: "project-1".to_string(),
            deployment_id: "deployment-1".to_string(),
            route_id: "route-1".to_string(),
            port_lease_id: "lease-1".to_string(),
            port_name: "http".to_string(),
            container_id: "a".repeat(64),
            container_name: "plurora-target-test".to_string(),
            image: "registry.example/app:latest".to_string(),
            image_id: Some(format!("sha256:{}", "b".repeat(64))),
            container_port: 8080,
            host_port: 49152,
            bind_host: BIND_HOST.to_string(),
            running: true,
            state: "running".to_string(),
            owner_operation_id: "operation-1".to_string(),
        };
        let value = serde_json::to_value(&observation).unwrap();
        assert_eq!(value["container_id"], format!("docker:{}", "a".repeat(64)));
        assert!(value.get("target_id").is_none());
        assert!(value.get("project_id").is_none());
        assert!(!crate::scan_effect_value_for_raw_secrets(&value, "receipt.output").has_findings());
    }
}
