use std::collections::{HashMap, HashSet};
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
    ListContainersOptionsBuilder, ListImagesOptionsBuilder, RemoveContainerOptionsBuilder,
    RemoveImageOptionsBuilder, StopContainerOptionsBuilder,
};
use bollard::Docker;
use bytes::Bytes;
use futures::StreamExt;
use plurora_work::{InstallationId, WorkspaceId};
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

#[async_trait::async_trait]
pub trait ManagedTargetEffectGuard: Send + Sync {
    async fn ensure_current(&self) -> anyhow::Result<()>;
}

async fn ensure_effect_guard(
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
) -> anyhow::Result<()> {
    guard.ensure_current().await
}

async fn ensure_effect_guard_after(
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
    stage: &'static str,
) -> anyhow::Result<()> {
    guard
        .ensure_current()
        .await
        .map_err(|_| outcome_unknown(stage))
}

async fn guarded_docker_call<T, F>(
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
    stage: &'static str,
    call: F,
) -> anyhow::Result<T>
where
    F: std::future::Future<Output = T>,
{
    ensure_effect_guard(guard).await?;
    let result = call.await;
    ensure_effect_guard_after(guard, stage).await?;
    Ok(result)
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
    pub installation_id: InstallationId,
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManagedTargetImageDisposition {
    RetainForDeployment,
    RemoveAfterVerification,
}

impl ManagedTargetImageDisposition {
    fn as_str(self) -> &'static str {
        match self {
            Self::RetainForDeployment => "retain_for_deployment",
            Self::RemoveAfterVerification => "remove_after_verification",
        }
    }
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
    pub installation_id: InstallationId,
    pub workspace_id: WorkspaceId,
    pub build_id: String,
    pub dockerfile: String,
    pub network_mode: ManagedTargetBuildNetworkMode,
    pub disposition: ManagedTargetImageDisposition,
    pub source_tree_digest: String,
    pub build_descriptor_hash: String,
    pub context_digest: String,
    pub context_tar: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ManagedTargetImageBuildReceipt {
    pub target_id: String,
    pub image: String,
    pub image_id: String,
    pub installation_id: InstallationId,
    pub workspace_id: WorkspaceId,
    pub build_id: String,
    pub dockerfile: String,
    pub network_mode: ManagedTargetBuildNetworkMode,
    pub disposition: ManagedTargetImageDisposition,
    pub context_digest: String,
    pub source_tree_digest: String,
    pub build_descriptor_hash: String,
    pub image_removed: bool,
    pub image_retained: bool,
}

trait ManagedTargetImageLabelSource {
    fn target_id(&self) -> &str;
    fn installation_id(&self) -> &InstallationId;
    fn workspace_id(&self) -> &WorkspaceId;
    fn build_id(&self) -> &str;
    fn dockerfile(&self) -> &str;
    fn network_mode(&self) -> ManagedTargetBuildNetworkMode;
    fn disposition(&self) -> ManagedTargetImageDisposition;
    fn context_digest(&self) -> &str;
    fn source_tree_digest(&self) -> &str;
    fn build_descriptor_hash(&self) -> &str;
}

impl ManagedTargetImageLabelSource for ManagedTargetImageBuild {
    fn target_id(&self) -> &str {
        &self.target_id
    }

    fn installation_id(&self) -> &InstallationId {
        &self.installation_id
    }

    fn workspace_id(&self) -> &WorkspaceId {
        &self.workspace_id
    }

    fn build_id(&self) -> &str {
        &self.build_id
    }

    fn dockerfile(&self) -> &str {
        &self.dockerfile
    }

    fn network_mode(&self) -> ManagedTargetBuildNetworkMode {
        self.network_mode
    }

    fn disposition(&self) -> ManagedTargetImageDisposition {
        self.disposition
    }

    fn context_digest(&self) -> &str {
        &self.context_digest
    }

    fn source_tree_digest(&self) -> &str {
        &self.source_tree_digest
    }

    fn build_descriptor_hash(&self) -> &str {
        &self.build_descriptor_hash
    }
}

impl ManagedTargetImageLabelSource for ManagedTargetImageBuildReceipt {
    fn target_id(&self) -> &str {
        &self.target_id
    }

    fn installation_id(&self) -> &InstallationId {
        &self.installation_id
    }

    fn workspace_id(&self) -> &WorkspaceId {
        &self.workspace_id
    }

    fn build_id(&self) -> &str {
        &self.build_id
    }

    fn dockerfile(&self) -> &str {
        &self.dockerfile
    }

    fn network_mode(&self) -> ManagedTargetBuildNetworkMode {
        self.network_mode
    }

    fn disposition(&self) -> ManagedTargetImageDisposition {
        self.disposition
    }

    fn context_digest(&self) -> &str {
        &self.context_digest
    }

    fn source_tree_digest(&self) -> &str {
        &self.source_tree_digest
    }

    fn build_descriptor_hash(&self) -> &str {
        &self.build_descriptor_hash
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedTargetDeploymentRef {
    pub target_id: String,
    pub installation_id: InstallationId,
    pub deployment_id: String,
    pub route_id: String,
    pub port_lease_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManagedTargetDeploymentObservation {
    #[serde(skip_serializing)]
    pub target_id: String,
    #[serde(skip_serializing)]
    pub installation_id: InstallationId,
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
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
) -> anyhow::Result<ManagedTargetImageBuildReceipt> {
    validate_image_build_request(&request)?;
    validate_build_context_tar(&request.context_tar, &request.dockerfile)?;
    anyhow::ensure!(
        crate::sha256_digest(&request.context_tar) == request.context_digest,
        "managed target build context digest did not match"
    );

    let docker = guarded_docker(guard, "docker image build runtime fence confirmation").await?;
    build_managed_target_image_with_backend(
        &DockerManagedTargetImageBackend { docker: &docker },
        request,
        guard,
    )
    .await
}

async fn build_managed_target_image_with_backend<B>(
    backend: &B,
    mut request: ManagedTargetImageBuild,
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
) -> anyhow::Result<ManagedTargetImageBuildReceipt>
where
    B: ManagedTargetImageBackend + ?Sized,
{
    validate_image_build_request(&request)?;
    validate_build_context_tar(&request.context_tar, &request.dockerfile)?;
    anyhow::ensure!(
        crate::sha256_digest(&request.context_tar) == request.context_digest,
        "managed target build context digest did not match"
    );
    let image = target_image_tag(request.installation_id.as_str(), &request.build_id);
    let labels = managed_image_labels(&request);
    let context_tar = std::mem::take(&mut request.context_tar);
    let build_result = guarded_docker_call(
        guard,
        "docker image build fence confirmation",
        backend.build_image(&request, &image, &labels, context_tar),
    )
    .await?;
    if let Err(error) = build_result {
        let (certainty, error) = match error {
            ManagedTargetImageBuildError::DaemonTerminal(error) => (
                ManagedTargetImageBuildFailureCertainty::DaemonTerminal,
                error,
            ),
            ManagedTargetImageBuildError::Ambiguous(error) => {
                (ManagedTargetImageBuildFailureCertainty::Ambiguous, error)
            }
        };
        return handle_failed_remove_after_verification_build(
            backend,
            &request,
            &image,
            None,
            guard,
            certainty,
            error.context("managed target Docker build failed"),
        )
        .await;
    }

    ensure_effect_guard(guard).await?;
    let inspected = backend.inspect_image(&image).await;
    ensure_effect_guard_after(guard, "docker image build inspection fence confirmation").await?;
    let inspected = match inspected {
        Ok(Some(inspected)) => inspected,
        Ok(None) => {
            return handle_failed_remove_after_verification_build(
                backend,
                &request,
                &image,
                None,
                guard,
                ManagedTargetImageBuildFailureCertainty::PostBuildVerification,
                anyhow::anyhow!("managed target built image disappeared before inspection"),
            )
            .await;
        }
        Err(error) => {
            return handle_failed_remove_after_verification_build(
                backend,
                &request,
                &image,
                None,
                guard,
                ManagedTargetImageBuildFailureCertainty::PostBuildVerification,
                error.context("managed target built image inspect failed"),
            )
            .await;
        }
    };
    let image_id = inspected.id;
    let validation = (|| {
        anyhow::ensure!(
            is_sha256_digest(&image_id),
            "managed target built image has no content-addressable id"
        );
        anyhow::ensure!(
            image_has_exact_provenance_labels(&inspected.labels, &labels),
            "managed target built image provenance label mismatch"
        );
        Ok::<_, anyhow::Error>(())
    })();
    if let Err(error) = validation {
        return handle_failed_remove_after_verification_build(
            backend,
            &request,
            &image,
            Some(image_id),
            guard,
            ManagedTargetImageBuildFailureCertainty::PostBuildVerification,
            error,
        )
        .await;
    }
    let receipt = managed_image_build_receipt(request, image, image_id);
    if let Err(error) = validate_image_build_receipt_state(&receipt, false) {
        return handle_failed_remove_after_verification_build(
            backend,
            &receipt,
            &receipt.image,
            Some(receipt.image_id.clone()),
            guard,
            ManagedTargetImageBuildFailureCertainty::PostBuildVerification,
            error,
        )
        .await;
    }
    ensure_effect_guard_after(guard, "docker image build receipt fence confirmation").await?;
    Ok(receipt)
}

async fn handle_failed_remove_after_verification_build<B, S>(
    backend: &B,
    source: &S,
    image: &str,
    image_id: Option<String>,
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
    certainty: ManagedTargetImageBuildFailureCertainty,
    error: anyhow::Error,
) -> anyhow::Result<ManagedTargetImageBuildReceipt>
where
    B: ManagedTargetImageBackend + ?Sized,
    S: ManagedTargetImageLabelSource + ?Sized,
{
    if source.disposition() == ManagedTargetImageDisposition::RetainForDeployment {
        return match certainty {
            ManagedTargetImageBuildFailureCertainty::DaemonTerminal => Err(error),
            ManagedTargetImageBuildFailureCertainty::Ambiguous
            | ManagedTargetImageBuildFailureCertainty::PostBuildVerification => {
                Err(outcome_unknown("docker image build verification"))
            }
        };
    }
    cleanup_managed_target_images(backend, source, image, image_id.as_deref(), guard).await?;
    match certainty {
        ManagedTargetImageBuildFailureCertainty::Ambiguous => {
            Err(outcome_unknown("docker image build"))
        }
        ManagedTargetImageBuildFailureCertainty::DaemonTerminal
        | ManagedTargetImageBuildFailureCertainty::PostBuildVerification => Err(error),
    }
}

fn managed_image_build_receipt(
    request: ManagedTargetImageBuild,
    image: String,
    image_id: String,
) -> ManagedTargetImageBuildReceipt {
    ManagedTargetImageBuildReceipt {
        target_id: request.target_id,
        image,
        image_id,
        installation_id: request.installation_id,
        workspace_id: request.workspace_id,
        build_id: request.build_id,
        dockerfile: request.dockerfile,
        network_mode: request.network_mode,
        disposition: request.disposition,
        context_digest: request.context_digest,
        source_tree_digest: request.source_tree_digest,
        build_descriptor_hash: request.build_descriptor_hash,
        image_removed: false,
        image_retained: true,
    }
}

pub async fn finalize_managed_target_image_build(
    receipt: ManagedTargetImageBuildReceipt,
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
) -> anyhow::Result<ManagedTargetImageBuildReceipt> {
    match receipt.disposition {
        ManagedTargetImageDisposition::RetainForDeployment => {
            validate_image_build_receipt_state(&receipt, false)?;
            ensure_effect_guard(guard).await?;
            Ok(receipt)
        }
        ManagedTargetImageDisposition::RemoveAfterVerification => {
            remove_managed_target_image(receipt, guard).await
        }
    }
}

pub async fn remove_managed_target_image(
    mut receipt: ManagedTargetImageBuildReceipt,
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
) -> anyhow::Result<ManagedTargetImageBuildReceipt> {
    validate_image_build_receipt_state(&receipt, false)?;
    anyhow::ensure!(
        receipt.disposition == ManagedTargetImageDisposition::RemoveAfterVerification,
        "managed target image disposition does not authorize removal"
    );
    ensure_effect_guard(guard).await?;
    let docker = docker().await;
    ensure_effect_guard_after(guard, "docker image removal runtime fence confirmation").await?;
    let docker = docker.map_err(|_| outcome_unknown("docker image removal lookup"))?;
    cleanup_managed_target_images(
        &DockerManagedTargetImageBackend { docker: &docker },
        &receipt,
        &receipt.image,
        Some(&receipt.image_id),
        guard,
    )
    .await?;
    receipt.image_removed = true;
    receipt.image_retained = false;
    validate_image_build_receipt_state(&receipt, true)?;
    ensure_effect_guard_after(guard, "docker image final receipt fence confirmation").await?;
    Ok(receipt)
}

pub async fn wait_for_managed_target_deployment_readiness(
    deployment: &ManagedTargetDeploymentObservation,
    health_path: Option<&str>,
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
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
        ensure_effect_guard(guard).await?;
        let probe = probe_managed_target_deployment(deployment.host_port, health_path).await;
        ensure_effect_guard_after(guard, "deployment readiness fence confirmation").await?;
        match probe {
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
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
) -> anyhow::Result<ManagedTargetDeploymentObservation> {
    validate_apply_request(request)?;
    let docker = guarded_docker(guard, "docker deployment runtime fence confirmation").await?;
    apply_managed_target_deployment_with_backend(
        &DockerManagedTargetDeploymentBackend { docker: &docker },
        request,
        guard,
    )
    .await
}

async fn apply_managed_target_deployment_with_backend<B>(
    backend: &B,
    request: &ManagedTargetDeploymentApply,
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
) -> anyhow::Result<ManagedTargetDeploymentObservation>
where
    B: ManagedTargetDeploymentBackend + ?Sized,
{
    if let Some(observation) = guarded_find(
        backend,
        &request.reference(),
        guard,
        "docker deployment lookup fence confirmation",
    )
    .await?
    {
        anyhow::ensure!(
            observation.owner_operation_id == request.operation_id
                && observation.port_name == request.port_name
                && observation.image == request.image
                && observation.container_port == request.container_port,
            "existing managed deployment conflicts with the requested operation"
        );
        if !observation.running {
            ensure_effect_guard(guard).await?;
            let started = backend.start_container(&observation.container_id).await;
            ensure_effect_guard_after(guard, "docker container start fence confirmation").await?;
            if started.is_err() {
                if let Ok(Some(observation)) = guarded_find(
                    backend,
                    &request.reference(),
                    guard,
                    "docker container start recovery lookup fence confirmation",
                )
                .await
                {
                    if observation.running {
                        validate_requested_host_port(request, &observation)?;
                        ensure_effect_guard_after(
                            guard,
                            "docker container start receipt fence confirmation",
                        )
                        .await?;
                        return Ok(observation);
                    }
                }
                return Err(outcome_unknown("docker container start"));
            }
            let observation = guarded_find(
                backend,
                &request.reference(),
                guard,
                "docker container start verification lookup fence confirmation",
            )
            .await
            .map_err(|_| outcome_unknown("docker container start verification"))?;
            let observation = observation
                .ok_or_else(|| outcome_unknown("docker container start verification"))?;
            validate_requested_host_port(request, &observation)?;
            ensure_effect_guard_after(guard, "docker container start receipt fence confirmation")
                .await?;
            return Ok(observation);
        }
        validate_requested_host_port(request, &observation)?;
        ensure_effect_guard(guard).await?;
        return Ok(observation);
    }

    if request.pull_if_missing {
        ensure_effect_guard(guard).await?;
        let pulled = backend.pull_image(&request.image).await;
        ensure_effect_guard_after(guard, "docker image pull fence confirmation").await?;
        pulled.map_err(|_| outcome_unknown("docker image pull"))?;
    }
    ensure_effect_guard(guard).await?;
    let image_id = backend.inspect_image_id(&request.image).await;
    ensure_effect_guard_after(guard, "docker image inspect fence confirmation").await?;
    let image_id = image_id?;

    ensure_effect_guard(guard).await?;
    let created = backend.create_container(request, &image_id).await;
    ensure_effect_guard_after(guard, "docker container create fence confirmation").await?;
    let created_id = match created {
        Ok(created_id) => created_id,
        Err(_) => {
            if let Ok(Some(observation)) = guarded_find(
                backend,
                &request.reference(),
                guard,
                "docker container create recovery lookup fence confirmation",
            )
            .await
            {
                if observation.running && observation.owner_operation_id == request.operation_id {
                    validate_requested_host_port(request, &observation)?;
                    ensure_effect_guard_after(
                        guard,
                        "docker container create receipt fence confirmation",
                    )
                    .await?;
                    return Ok(observation);
                }
            }
            return Err(outcome_unknown("docker container create"));
        }
    };
    ensure_effect_guard(guard).await?;
    let started = backend.start_container(&created_id).await;
    ensure_effect_guard_after(guard, "docker container start fence confirmation").await?;
    if started.is_err() {
        if let Ok(Some(observation)) = guarded_find(
            backend,
            &request.reference(),
            guard,
            "docker container start recovery lookup fence confirmation",
        )
        .await
        {
            if observation.running {
                validate_requested_host_port(request, &observation)?;
                ensure_effect_guard_after(
                    guard,
                    "docker container start receipt fence confirmation",
                )
                .await?;
                return Ok(observation);
            }
        }
        return Err(outcome_unknown("docker container start"));
    }
    let observation = guarded_find(
        backend,
        &request.reference(),
        guard,
        "docker deployment verification lookup fence confirmation",
    )
    .await
    .map_err(|_| outcome_unknown("docker container start verification"))?;
    let observation =
        observation.ok_or_else(|| outcome_unknown("docker container start verification"))?;
    validate_requested_host_port(request, &observation)?;
    ensure_effect_guard_after(guard, "docker deployment receipt fence confirmation").await?;
    Ok(observation)
}

pub async fn observe_managed_target_deployment(
    reference: &ManagedTargetDeploymentRef,
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
) -> anyhow::Result<Option<ManagedTargetDeploymentObservation>> {
    validate_reference(reference)?;
    let docker = guarded_docker(guard, "docker observation runtime fence confirmation").await?;
    observe_managed_target_deployment_with_backend(
        &DockerManagedTargetDeploymentBackend { docker: &docker },
        reference,
        guard,
    )
    .await
}

async fn observe_managed_target_deployment_with_backend<B>(
    backend: &B,
    reference: &ManagedTargetDeploymentRef,
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
) -> anyhow::Result<Option<ManagedTargetDeploymentObservation>>
where
    B: ManagedTargetDeploymentBackend + ?Sized,
{
    guarded_find(
        backend,
        reference,
        guard,
        "docker observation lookup fence confirmation",
    )
    .await
}

pub async fn drain_managed_target_deployment(
    reference: &ManagedTargetDeploymentRef,
    grace_seconds: u16,
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
) -> anyhow::Result<ManagedTargetDeploymentDrainReceipt> {
    validate_reference(reference)?;
    anyhow::ensure!(grace_seconds <= 300, "deployment grace period is too large");
    let docker = guarded_docker(guard, "docker drain runtime fence confirmation").await?;
    drain_managed_target_deployment_with_backend(
        &DockerManagedTargetDeploymentBackend { docker: &docker },
        reference,
        grace_seconds,
        guard,
    )
    .await
}

async fn drain_managed_target_deployment_with_backend<B>(
    backend: &B,
    reference: &ManagedTargetDeploymentRef,
    grace_seconds: u16,
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
) -> anyhow::Result<ManagedTargetDeploymentDrainReceipt>
where
    B: ManagedTargetDeploymentBackend + ?Sized,
{
    let Some(observation) = guarded_find(
        backend,
        reference,
        guard,
        "docker drain lookup fence confirmation",
    )
    .await?
    else {
        ensure_effect_guard(guard).await?;
        return Ok(ManagedTargetDeploymentDrainReceipt {
            deployment: None,
            stopped: true,
            grace_seconds,
            container_retained: false,
        });
    };
    if observation.running {
        ensure_effect_guard(guard).await?;
        let stopped = backend
            .stop_container(&observation.container_id, grace_seconds)
            .await;
        ensure_effect_guard_after(guard, "docker container drain fence confirmation").await?;
        if stopped.is_err() {
            match guarded_find(
                backend,
                reference,
                guard,
                "docker drain recovery lookup fence confirmation",
            )
            .await
            {
                Ok(Some(current)) if !current.running => {}
                _ => return Err(outcome_unknown("docker container drain")),
            }
        }
    }
    let after = match guarded_find(
        backend,
        reference,
        guard,
        "docker drain verification lookup fence confirmation",
    )
    .await
    {
        Ok(Some(current)) if !current.running => current,
        _ => return Err(outcome_unknown("docker container drain verification")),
    };
    ensure_effect_guard_after(guard, "docker container drain receipt fence confirmation").await?;
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
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
) -> anyhow::Result<ManagedTargetDeploymentStopReceipt> {
    validate_reference(reference)?;
    anyhow::ensure!(grace_seconds <= 300, "deployment grace period is too large");
    let docker = guarded_docker(guard, "docker stop runtime fence confirmation").await?;
    stop_managed_target_deployment_with_backend(
        &DockerManagedTargetDeploymentBackend { docker: &docker },
        reference,
        grace_seconds,
        force_remove,
        guard,
    )
    .await
}

async fn stop_managed_target_deployment_with_backend<B>(
    backend: &B,
    reference: &ManagedTargetDeploymentRef,
    grace_seconds: u16,
    force_remove: bool,
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
) -> anyhow::Result<ManagedTargetDeploymentStopReceipt>
where
    B: ManagedTargetDeploymentBackend + ?Sized,
{
    let Some(observation) = guarded_find(
        backend,
        reference,
        guard,
        "docker stop lookup fence confirmation",
    )
    .await?
    else {
        ensure_effect_guard(guard).await?;
        return Ok(ManagedTargetDeploymentStopReceipt {
            stopped: true,
            removed: true,
            force_remove,
            grace_seconds,
        });
    };
    if observation.running {
        ensure_effect_guard(guard).await?;
        let stop = backend
            .stop_container(&observation.container_id, grace_seconds)
            .await;
        ensure_effect_guard_after(guard, "docker container stop fence confirmation").await?;
        match stop {
            Ok(()) => {}
            Err(_) if force_remove => {}
            Err(_) => match guarded_find(
                backend,
                reference,
                guard,
                "docker stop recovery lookup fence confirmation",
            )
            .await
            {
                Ok(Some(current)) if !current.running => {}
                _ => return Err(outcome_unknown("docker container stop")),
            },
        }
    }
    ensure_effect_guard(guard).await?;
    let removed = backend
        .remove_container(&observation.container_id, force_remove)
        .await;
    ensure_effect_guard_after(guard, "docker container removal fence confirmation").await?;
    if removed.is_err() {
        match guarded_find(
            backend,
            reference,
            guard,
            "docker removal recovery lookup fence confirmation",
        )
        .await
        {
            Ok(None) => {}
            _ => return Err(outcome_unknown("docker container removal")),
        }
    }
    ensure_effect_guard_after(guard, "docker container stop receipt fence confirmation").await?;
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
            installation_id: self.installation_id.clone(),
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

async fn guarded_docker(
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
    stage: &'static str,
) -> anyhow::Result<Docker> {
    guarded_docker_call(guard, stage, docker()).await?
}

#[async_trait::async_trait]
trait ManagedTargetDeploymentBackend: Send + Sync {
    async fn find(
        &self,
        reference: &ManagedTargetDeploymentRef,
    ) -> anyhow::Result<Option<ManagedTargetDeploymentObservation>>;

    async fn pull_image(&self, image: &str) -> anyhow::Result<()>;

    async fn inspect_image_id(&self, image: &str) -> anyhow::Result<String>;

    async fn create_container(
        &self,
        request: &ManagedTargetDeploymentApply,
        image_id: &str,
    ) -> anyhow::Result<String>;

    async fn start_container(&self, container_id: &str) -> anyhow::Result<()>;

    async fn stop_container(&self, container_id: &str, grace_seconds: u16) -> anyhow::Result<()>;

    async fn remove_container(&self, container_id: &str, force_remove: bool) -> anyhow::Result<()>;
}

async fn guarded_find<B>(
    backend: &B,
    reference: &ManagedTargetDeploymentRef,
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
    stage: &'static str,
) -> anyhow::Result<Option<ManagedTargetDeploymentObservation>>
where
    B: ManagedTargetDeploymentBackend + ?Sized,
{
    guarded_docker_call(guard, stage, backend.find(reference)).await?
}

struct DockerManagedTargetDeploymentBackend<'a> {
    docker: &'a Docker,
}

#[async_trait::async_trait]
impl ManagedTargetDeploymentBackend for DockerManagedTargetDeploymentBackend<'_> {
    async fn find(
        &self,
        reference: &ManagedTargetDeploymentRef,
    ) -> anyhow::Result<Option<ManagedTargetDeploymentObservation>> {
        Ok(find_target_deployment(self.docker, reference)
            .await?
            .map(|(_, observation)| observation))
    }

    async fn pull_image(&self, image: &str) -> anyhow::Result<()> {
        let options = CreateImageOptionsBuilder::default()
            .from_image(image)
            .build();
        tokio::time::timeout(DOCKER_PULL_TIMEOUT, async {
            let mut stream = self.docker.create_image(Some(options), None, None);
            while let Some(item) = stream.next().await {
                item.context("docker image pull failed")?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("docker image pull timed out")??;
        Ok(())
    }

    async fn inspect_image_id(&self, image: &str) -> anyhow::Result<String> {
        let inspected_image =
            tokio::time::timeout(DOCKER_EFFECT_TIMEOUT, self.docker.inspect_image(image))
                .await
                .context("docker image inspect timed out")??;
        inspected_image
            .id
            .context("docker image has no content-addressable id")
    }

    async fn create_container(
        &self,
        request: &ManagedTargetDeploymentApply,
        image_id: &str,
    ) -> anyhow::Result<String> {
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
        let config = ContainerCreateBody {
            image: Some(image_id.to_string()),
            labels: Some(deployment_labels(request)),
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
            request.installation_id.as_str(),
            &request.deployment_id,
        );
        let options = CreateContainerOptionsBuilder::default()
            .name(&container_name)
            .build();
        tokio::time::timeout(
            DOCKER_EFFECT_TIMEOUT,
            self.docker.create_container(Some(options), config),
        )
        .await
        .context("docker container create timed out")?
        .map(|created| created.id)
        .map_err(Into::into)
    }

    async fn start_container(&self, container_id: &str) -> anyhow::Result<()> {
        tokio::time::timeout(
            DOCKER_EFFECT_TIMEOUT,
            self.docker.start_container(container_id, None),
        )
        .await
        .context("docker container start timed out")??;
        Ok(())
    }

    async fn stop_container(&self, container_id: &str, grace_seconds: u16) -> anyhow::Result<()> {
        let options = StopContainerOptionsBuilder::default()
            .t(i32::from(grace_seconds))
            .build();
        tokio::time::timeout(
            Duration::from_secs(u64::from(grace_seconds).saturating_add(30)),
            self.docker.stop_container(container_id, Some(options)),
        )
        .await
        .context("docker container stop timed out")??;
        Ok(())
    }

    async fn remove_container(&self, container_id: &str, force_remove: bool) -> anyhow::Result<()> {
        let options = RemoveContainerOptionsBuilder::default()
            .force(force_remove)
            .v(false)
            .build();
        tokio::time::timeout(
            DOCKER_EFFECT_TIMEOUT,
            self.docker.remove_container(container_id, Some(options)),
        )
        .await
        .context("docker container removal timed out")??;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManagedTargetImageInspection {
    id: String,
    labels: HashMap<String, String>,
}

#[derive(Debug)]
enum ManagedTargetImageBuildError {
    DaemonTerminal(anyhow::Error),
    Ambiguous(anyhow::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManagedTargetImageBuildFailureCertainty {
    DaemonTerminal,
    Ambiguous,
    PostBuildVerification,
}

#[async_trait::async_trait]
trait ManagedTargetImageBackend: Send + Sync {
    async fn build_image(
        &self,
        request: &ManagedTargetImageBuild,
        image: &str,
        labels: &HashMap<String, String>,
        context_tar: Vec<u8>,
    ) -> Result<(), ManagedTargetImageBuildError>;

    async fn list_images(
        &self,
        exact_labels: &HashMap<String, String>,
    ) -> anyhow::Result<Vec<String>>;

    async fn inspect_image(
        &self,
        reference: &str,
    ) -> anyhow::Result<Option<ManagedTargetImageInspection>>;

    async fn remove_image(&self, image_id: &str) -> anyhow::Result<()>;
}

struct DockerManagedTargetImageBackend<'a> {
    docker: &'a Docker,
}

#[async_trait::async_trait]
impl ManagedTargetImageBackend for DockerManagedTargetImageBackend<'_> {
    async fn build_image(
        &self,
        request: &ManagedTargetImageBuild,
        image: &str,
        labels: &HashMap<String, String>,
        context_tar: Vec<u8>,
    ) -> Result<(), ManagedTargetImageBuildError> {
        let options = BuildImageOptionsBuilder::default()
            .dockerfile(&request.dockerfile)
            .t(image)
            .q(false)
            .rm(true)
            .forcerm(true)
            .memory(1024 * 1024 * 1024)
            .cpuquota(100_000)
            .networkmode(request.network_mode.as_str())
            .labels(labels)
            .build();
        let build = async {
            let mut stream =
                self.docker
                    .build_image(options, None, Some(body_full(Bytes::from(context_tar))));
            while let Some(item) = stream.next().await {
                let item = item.map_err(|error| {
                    ManagedTargetImageBuildError::Ambiguous(
                        anyhow::Error::from(error)
                            .context("managed target Docker build stream failed"),
                    )
                })?;
                if item.error_detail.is_some() {
                    return Err(ManagedTargetImageBuildError::DaemonTerminal(
                        anyhow::anyhow!("managed target Docker daemon rejected the build"),
                    ));
                }
            }
            Ok(())
        };
        match tokio::time::timeout(DOCKER_BUILD_TIMEOUT, build).await {
            Ok(result) => result,
            Err(error) => Err(ManagedTargetImageBuildError::Ambiguous(
                anyhow::Error::from(error).context("managed target Docker build timed out"),
            )),
        }
    }

    async fn list_images(
        &self,
        exact_labels: &HashMap<String, String>,
    ) -> anyhow::Result<Vec<String>> {
        let filters = HashMap::from([(
            "label".to_string(),
            exact_labels
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>(),
        )]);
        let options = ListImagesOptionsBuilder::default()
            .all(true)
            .filters(&filters)
            .build();
        let images = tokio::time::timeout(
            DOCKER_EFFECT_TIMEOUT,
            self.docker.list_images(Some(options)),
        )
        .await
        .context("docker managed image list timed out")??;
        Ok(images.into_iter().map(|image| image.id).collect())
    }

    async fn inspect_image(
        &self,
        reference: &str,
    ) -> anyhow::Result<Option<ManagedTargetImageInspection>> {
        match tokio::time::timeout(DOCKER_EFFECT_TIMEOUT, self.docker.inspect_image(reference))
            .await
        {
            Ok(Ok(inspected)) => {
                let id = inspected
                    .id
                    .context("managed target built image has no content-addressable id")?;
                let labels = inspected
                    .config
                    .and_then(|config| config.labels)
                    .unwrap_or_default();
                Ok(Some(ManagedTargetImageInspection { id, labels }))
            }
            Ok(Err(error)) if docker_not_found_error(&error) => Ok(None),
            Ok(Err(error)) => Err(error.into()),
            Err(error) => Err(error.into()),
        }
    }

    async fn remove_image(&self, image_id: &str) -> anyhow::Result<()> {
        let options = RemoveImageOptionsBuilder::default()
            .force(true)
            .noprune(true)
            .build();
        match tokio::time::timeout(
            DOCKER_EFFECT_TIMEOUT,
            self.docker.remove_image(image_id, Some(options), None),
        )
        .await
        {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) if docker_not_found_error(&error) => Ok(()),
            Ok(Err(error)) => Err(error.into()),
            Err(error) => Err(error.into()),
        }
    }
}

fn managed_image_labels<S>(source: &S) -> HashMap<String, String>
where
    S: ManagedTargetImageLabelSource + ?Sized,
{
    HashMap::from([
        ("managed-by".to_string(), "plurora".to_string()),
        ("plurora.target_driver".to_string(), DRIVER_ID.to_string()),
        (
            "plurora.target_id".to_string(),
            source.target_id().to_string(),
        ),
        (
            "plurora.installation_id".to_string(),
            source.installation_id().to_string(),
        ),
        (
            "plurora.workspace_id".to_string(),
            source.workspace_id().to_string(),
        ),
        (
            "plurora.build_id".to_string(),
            source.build_id().to_string(),
        ),
        (
            "plurora.dockerfile".to_string(),
            source.dockerfile().to_string(),
        ),
        (
            "plurora.build_network_mode".to_string(),
            source.network_mode().as_str().to_string(),
        ),
        (
            "plurora.image_disposition".to_string(),
            source.disposition().as_str().to_string(),
        ),
        (
            "plurora.source_tree_digest".to_string(),
            source.source_tree_digest().to_string(),
        ),
        (
            "plurora.build_context_digest".to_string(),
            source.context_digest().to_string(),
        ),
        (
            "plurora.build_descriptor_hash".to_string(),
            source.build_descriptor_hash().to_string(),
        ),
    ])
}

#[cfg(test)]
fn validate_built_image_ownership_labels(
    receipt: &ManagedTargetImageBuildReceipt,
    actual_image_id: &str,
    actual_labels: &HashMap<String, String>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        actual_image_id == receipt.image_id,
        "managed target built image content identity changed before removal"
    );
    anyhow::ensure!(
        image_has_exact_provenance_labels(actual_labels, &managed_image_labels(receipt)),
        "managed target built image provenance label mismatch before removal"
    );
    Ok(())
}

fn image_has_exact_provenance_labels(
    actual: &HashMap<String, String>,
    expected: &HashMap<String, String>,
) -> bool {
    expected
        .iter()
        .all(|(name, value)| actual.get(name) == Some(value))
}

async fn cleanup_managed_target_images<B>(
    backend: &B,
    source: &(impl ManagedTargetImageLabelSource + ?Sized),
    image: &str,
    known_image_id: Option<&str>,
    guard: &(dyn ManagedTargetEffectGuard + Send + Sync),
) -> anyhow::Result<()>
where
    B: ManagedTargetImageBackend + ?Sized,
{
    let expected_labels = managed_image_labels(source);
    let listed_ids = guarded_docker_call(
        guard,
        "docker image cleanup discovery fence confirmation",
        backend.list_images(&expected_labels),
    )
    .await?;
    let mut listed_ids =
        listed_ids.map_err(|_| outcome_unknown("docker image cleanup discovery"))?;
    listed_ids.sort();
    listed_ids.dedup();

    // A list result which cannot immediately be inspected is not evidence of
    // absence. Fail closed instead of polling a stale daemon view indefinitely.
    for image_id in &listed_ids {
        let inspected = guarded_docker_call(
            guard,
            "docker image cleanup discovery inspection fence confirmation",
            backend.inspect_image(image_id),
        )
        .await?;
        let Some(inspected) =
            inspected.map_err(|_| outcome_unknown("docker image cleanup discovery inspection"))?
        else {
            return Err(outcome_unknown("docker image cleanup discovery inspection"));
        };
        if inspected.id != *image_id
            || !is_sha256_digest(image_id)
            || !image_has_exact_provenance_labels(&inspected.labels, &expected_labels)
        {
            return Err(outcome_unknown("docker image cleanup discovery inspection"));
        }
    }

    let listed_ids = listed_ids.into_iter().collect::<HashSet<_>>();
    let mut references = listed_ids.iter().cloned().collect::<Vec<_>>();
    references.push(image.to_string());
    if let Some(image_id) = known_image_id {
        references.push(image_id.to_string());
    }
    references.sort();
    references.dedup();
    let mut removal_ids = HashSet::new();
    for reference in references {
        let inspected = guarded_docker_call(
            guard,
            "docker image cleanup inspection fence confirmation",
            backend.inspect_image(&reference),
        )
        .await?;
        let Some(inspected) =
            inspected.map_err(|_| outcome_unknown("docker image cleanup inspection"))?
        else {
            if listed_ids.contains(&reference) {
                return Err(outcome_unknown("docker image cleanup inspection"));
            }
            continue;
        };
        if is_sha256_digest(&reference) && inspected.id != reference {
            return Err(outcome_unknown("docker image cleanup inspection"));
        }
        if image_has_exact_provenance_labels(&inspected.labels, &expected_labels) {
            if !is_sha256_digest(&inspected.id) {
                return Err(outcome_unknown("docker image cleanup inspection"));
            }
            removal_ids.insert(inspected.id);
        } else if reference == image || known_image_id == Some(reference.as_str()) {
            anyhow::bail!(
                "managed target image cleanup reference no longer has the receipt provenance"
            );
        } else {
            return Err(outcome_unknown("docker image cleanup inspection"));
        }
    }

    let mut removal_ids = removal_ids.into_iter().collect::<Vec<_>>();
    removal_ids.sort();
    for image_id in &removal_ids {
        let inspected = guarded_docker_call(
            guard,
            "docker image cleanup ownership reinspection fence confirmation",
            backend.inspect_image(image_id),
        )
        .await?;
        let Some(inspected) = inspected
            .map_err(|_| outcome_unknown("docker image cleanup ownership reinspection"))?
        else {
            continue;
        };
        if inspected.id != *image_id
            || !image_has_exact_provenance_labels(&inspected.labels, &expected_labels)
        {
            return Err(outcome_unknown(
                "docker image cleanup ownership reinspection",
            ));
        }
        let removal = guarded_docker_call(
            guard,
            "docker image removal fence confirmation",
            backend.remove_image(image_id),
        )
        .await?;
        removal.map_err(|_| outcome_unknown("docker image removal"))?;
    }

    let image_confirmation = guarded_docker_call(
        guard,
        "docker image removal confirmation fence confirmation",
        backend.inspect_image(image),
    )
    .await?;
    match image_confirmation.map_err(|_| outcome_unknown("docker image removal confirmation"))? {
        None => {}
        Some(inspected)
            if image_has_exact_provenance_labels(&inspected.labels, &expected_labels) =>
        {
            return Err(outcome_unknown("docker image removal confirmation"));
        }
        Some(_) => {
            anyhow::bail!(
                "managed target deterministic image tag is occupied by an external image"
            );
        }
    }

    for image_id in &removal_ids {
        let confirmation = guarded_docker_call(
            guard,
            "docker image id confirmation fence confirmation",
            backend.inspect_image(image_id),
        )
        .await?;
        if confirmation
            .map_err(|_| outcome_unknown("docker image removal confirmation"))?
            .is_some()
        {
            return Err(outcome_unknown("docker image removal confirmation"));
        }
    }

    let residual_ids = guarded_docker_call(
        guard,
        "docker image cleanup confirmation fence confirmation",
        backend.list_images(&expected_labels),
    )
    .await?;
    let mut residual_ids =
        residual_ids.map_err(|_| outcome_unknown("docker image cleanup confirmation"))?;
    residual_ids.sort();
    residual_ids.dedup();
    for image_id in &residual_ids {
        let inspected = guarded_docker_call(
            guard,
            "docker image cleanup final inspection fence confirmation",
            backend.inspect_image(image_id),
        )
        .await?;
        let Some(inspected) =
            inspected.map_err(|_| outcome_unknown("docker image cleanup confirmation"))?
        else {
            return Err(outcome_unknown("docker image cleanup confirmation"));
        };
        if inspected.id != *image_id
            || !image_has_exact_provenance_labels(&inspected.labels, &expected_labels)
        {
            return Err(outcome_unknown("docker image cleanup confirmation"));
        }
    }
    if residual_ids.is_empty() {
        Ok(())
    } else {
        Err(outcome_unknown("docker image cleanup confirmation"))
    }
}

fn validate_image_build_receipt_state(
    receipt: &ManagedTargetImageBuildReceipt,
    final_state: bool,
) -> anyhow::Result<()> {
    for (name, value) in [
        ("target_id", receipt.target_id.as_str()),
        ("installation_id", receipt.installation_id.as_str()),
        ("workspace_id", receipt.workspace_id.as_str()),
        ("build_id", receipt.build_id.as_str()),
    ] {
        validate_label_value(name, value)?;
    }
    validate_relative_path("dockerfile", &receipt.dockerfile)?;
    anyhow::ensure!(
        is_sha256_digest(&receipt.image_id)
            && is_sha256_digest(&receipt.context_digest)
            && is_sha256_digest(&receipt.source_tree_digest)
            && is_sha256_digest(&receipt.build_descriptor_hash),
        "managed target image build receipt contains an invalid digest"
    );
    anyhow::ensure!(
        valid_image_reference(&receipt.image),
        "managed target image build receipt contains an invalid image reference"
    );
    anyhow::ensure!(
        receipt.image == target_image_tag(receipt.installation_id.as_str(), &receipt.build_id),
        "managed target image build receipt contains the wrong deterministic image reference"
    );
    if final_state {
        let disposition_matches = match receipt.disposition {
            ManagedTargetImageDisposition::RetainForDeployment => {
                !receipt.image_removed && receipt.image_retained
            }
            ManagedTargetImageDisposition::RemoveAfterVerification => {
                receipt.image_removed && !receipt.image_retained
            }
        };
        anyhow::ensure!(
            disposition_matches,
            "managed target image build receipt does not prove its disposition"
        );
    } else {
        anyhow::ensure!(
            !receipt.image_removed && receipt.image_retained,
            "managed target image build receipt is not an unfinished retained image"
        );
    }
    Ok(())
}

pub fn validate_managed_target_image_build_receipt(
    receipt: &ManagedTargetImageBuildReceipt,
) -> anyhow::Result<()> {
    validate_image_build_receipt_state(receipt, true)
}

fn docker_not_found_error(error: &bollard::errors::Error) -> bool {
    matches!(
        error,
        bollard::errors::Error::DockerResponseServerError {
            status_code: 404,
            ..
        }
    )
}

fn validate_image_build_request(request: &ManagedTargetImageBuild) -> anyhow::Result<()> {
    for (name, value) in [
        ("target_id", request.target_id.as_str()),
        ("installation_id", request.installation_id.as_str()),
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

fn target_image_tag(installation_id: &str, build_id: &str) -> String {
    format!(
        "plurora/{}:{}",
        sanitize_image_component(installation_id, 80),
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
        ("installation_id", reference.installation_id.as_str()),
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
        (
            "plurora.installation_id".to_string(),
            request.installation_id.to_string(),
        ),
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

fn deployment_container_name(
    target_id: &str,
    installation_id: &str,
    deployment_id: &str,
) -> String {
    let digest =
        Sha256::digest(format!("{target_id}\0{installation_id}\0{deployment_id}").as_bytes());
    format!("plurora-target-{}", &format!("{digest:x}")[..24])
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
            format!("plurora.installation_id={}", reference.installation_id),
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
        installation_id: reference.installation_id.clone(),
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
        (
            "plurora.installation_id",
            reference.installation_id.as_str(),
        ),
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

    const INSTALLATION_ID: &str = "11111111-1111-4111-8111-111111111111";
    const WORKSPACE_ID: &str = "22222222-2222-4222-8222-222222222222";

    #[derive(Debug, Default)]
    struct TestEffectGuard;

    #[async_trait::async_trait]
    impl ManagedTargetEffectGuard for TestEffectGuard {
        async fn ensure_current(&self) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[derive(Debug, Clone, Default)]
    struct SwitchEffectGuard {
        current: std::sync::Arc<std::sync::atomic::AtomicBool>,
        checks: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl SwitchEffectGuard {
        fn new_current() -> Self {
            Self {
                current: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
                checks: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }
        }

        fn revoke(&self) {
            self.current
                .store(false, std::sync::atomic::Ordering::Release);
        }

        fn checks(&self) -> usize {
            self.checks.load(std::sync::atomic::Ordering::Acquire)
        }
    }

    #[async_trait::async_trait]
    impl ManagedTargetEffectGuard for SwitchEffectGuard {
        async fn ensure_current(&self) -> anyhow::Result<()> {
            self.checks
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            anyhow::ensure!(
                self.current.load(std::sync::atomic::Ordering::Acquire),
                "injected executor fence loss"
            );
            Ok(())
        }
    }

    #[derive(Debug)]
    struct FakeManagedTargetDeploymentState {
        observation: Option<ManagedTargetDeploymentObservation>,
        actions: Vec<String>,
        revoke_after: HashSet<String>,
    }

    struct FakeManagedTargetDeploymentBackend {
        state: std::sync::Mutex<FakeManagedTargetDeploymentState>,
        guard: SwitchEffectGuard,
    }

    impl FakeManagedTargetDeploymentBackend {
        fn new(
            observation: Option<ManagedTargetDeploymentObservation>,
            revoke_after: impl IntoIterator<Item = &'static str>,
        ) -> Self {
            Self {
                state: std::sync::Mutex::new(FakeManagedTargetDeploymentState {
                    observation,
                    actions: Vec::new(),
                    revoke_after: revoke_after.into_iter().map(str::to_string).collect(),
                }),
                guard: SwitchEffectGuard::new_current(),
            }
        }

        fn action(&self, action: &str) {
            let revoke = {
                let mut state = self.state.lock().unwrap();
                state.actions.push(action.to_string());
                state.revoke_after.contains(action)
            };
            if revoke {
                self.guard.revoke();
            }
        }

        fn actions(&self) -> Vec<String> {
            self.state.lock().unwrap().actions.clone()
        }
    }

    #[async_trait::async_trait]
    impl ManagedTargetDeploymentBackend for FakeManagedTargetDeploymentBackend {
        async fn find(
            &self,
            _reference: &ManagedTargetDeploymentRef,
        ) -> anyhow::Result<Option<ManagedTargetDeploymentObservation>> {
            self.action("find");
            Ok(self.state.lock().unwrap().observation.clone())
        }

        async fn pull_image(&self, _image: &str) -> anyhow::Result<()> {
            self.action("pull");
            Ok(())
        }

        async fn inspect_image_id(&self, _image: &str) -> anyhow::Result<String> {
            self.action("inspect_image");
            Ok(format!("sha256:{}", "a".repeat(64)))
        }

        async fn create_container(
            &self,
            request: &ManagedTargetDeploymentApply,
            _image_id: &str,
        ) -> anyhow::Result<String> {
            self.action("create");
            let container_id = "container-1".to_string();
            self.state.lock().unwrap().observation =
                Some(deployment_observation(request, &container_id, false));
            Ok(container_id)
        }

        async fn start_container(&self, _container_id: &str) -> anyhow::Result<()> {
            self.action("start");
            if let Some(observation) = self.state.lock().unwrap().observation.as_mut() {
                observation.running = true;
                observation.state = "running".to_string();
            }
            Ok(())
        }

        async fn stop_container(
            &self,
            _container_id: &str,
            _grace_seconds: u16,
        ) -> anyhow::Result<()> {
            self.action("stop");
            if let Some(observation) = self.state.lock().unwrap().observation.as_mut() {
                observation.running = false;
                observation.state = "exited".to_string();
            }
            Ok(())
        }

        async fn remove_container(
            &self,
            _container_id: &str,
            _force_remove: bool,
        ) -> anyhow::Result<()> {
            self.action("remove");
            self.state.lock().unwrap().observation = None;
            Ok(())
        }
    }

    fn deployment_request(pull_if_missing: bool) -> ManagedTargetDeploymentApply {
        ManagedTargetDeploymentApply {
            target_id: "target-1".to_string(),
            installation_id: InstallationId::parse(INSTALLATION_ID).unwrap(),
            deployment_id: "deployment-1".to_string(),
            route_id: "route-1".to_string(),
            port_lease_id: "lease-1".to_string(),
            port_name: "http".to_string(),
            image: "registry.example/app:latest".to_string(),
            container_port: 8080,
            requested_host_port: Some(49_152),
            pull_if_missing,
            operation_id: "operation-1".to_string(),
        }
    }

    fn deployment_observation(
        request: &ManagedTargetDeploymentApply,
        container_id: &str,
        running: bool,
    ) -> ManagedTargetDeploymentObservation {
        ManagedTargetDeploymentObservation {
            target_id: request.target_id.clone(),
            installation_id: request.installation_id.clone(),
            deployment_id: request.deployment_id.clone(),
            route_id: request.route_id.clone(),
            port_lease_id: request.port_lease_id.clone(),
            port_name: request.port_name.clone(),
            container_id: container_id.to_string(),
            container_name: "plurora-target-test".to_string(),
            image: request.image.clone(),
            image_id: Some(format!("sha256:{}", "a".repeat(64))),
            container_port: request.container_port,
            host_port: request.requested_host_port.unwrap(),
            bind_host: BIND_HOST.to_string(),
            running,
            state: if running { "running" } else { "created" }.to_string(),
            owner_operation_id: request.operation_id.clone(),
        }
    }

    #[tokio::test]
    async fn deployment_effect_fence_stops_after_pull_create_and_start() {
        for (revoke_after, expected_actions) in [
            ("pull", vec!["find", "pull"]),
            ("create", vec!["find", "inspect_image", "create"]),
            ("start", vec!["find", "inspect_image", "create", "start"]),
        ] {
            let request = deployment_request(revoke_after == "pull");
            let backend = FakeManagedTargetDeploymentBackend::new(None, [revoke_after]);
            let error =
                apply_managed_target_deployment_with_backend(&backend, &request, &backend.guard)
                    .await
                    .unwrap_err();
            assert!(is_managed_target_deployment_outcome_unknown(&error));
            assert_eq!(backend.actions(), expected_actions);
        }
    }

    #[tokio::test]
    async fn docker_connect_and_initial_find_are_guarded_before_the_next_effect() {
        let connect_guard = SwitchEffectGuard::new_current();
        let next_effects = std::sync::atomic::AtomicUsize::new(0);
        let result = guarded_docker_call(
            &connect_guard,
            "fake docker connect fence confirmation",
            async {
                connect_guard.revoke();
                Ok::<_, anyhow::Error>(())
            },
        )
        .await;
        if result.is_ok() {
            next_effects.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        }
        let error = result.unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));
        assert_eq!(connect_guard.checks(), 2);
        assert_eq!(next_effects.load(std::sync::atomic::Ordering::Acquire), 0);

        let request = deployment_request(false);
        let backend = FakeManagedTargetDeploymentBackend::new(None, ["find"]);
        let error =
            apply_managed_target_deployment_with_backend(&backend, &request, &backend.guard)
                .await
                .unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));
        assert_eq!(backend.actions(), vec!["find"]);
        assert_eq!(backend.guard.checks(), 2);
    }

    #[tokio::test]
    async fn deployment_observation_guards_the_find_on_both_sides() {
        let request = deployment_request(false);
        let observation = deployment_observation(&request, "container-1", true);
        let backend = FakeManagedTargetDeploymentBackend::new(Some(observation.clone()), []);
        assert_eq!(
            observe_managed_target_deployment_with_backend(
                &backend,
                &request.reference(),
                &backend.guard,
            )
            .await
            .unwrap(),
            Some(observation)
        );
        assert_eq!(backend.actions(), vec!["find"]);
        assert_eq!(backend.guard.checks(), 2);

        let revoked = FakeManagedTargetDeploymentBackend::new(None, ["find"]);
        let error = observe_managed_target_deployment_with_backend(
            &revoked,
            &request.reference(),
            &revoked.guard,
        )
        .await
        .unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));
        assert_eq!(revoked.actions(), vec!["find"]);
        assert_eq!(revoked.guard.checks(), 2);
    }

    #[tokio::test]
    async fn successful_apply_and_stop_guard_every_find_and_mutation() {
        let request = deployment_request(false);
        let apply_backend = FakeManagedTargetDeploymentBackend::new(None, []);
        let applied = apply_managed_target_deployment_with_backend(
            &apply_backend,
            &request,
            &apply_backend.guard,
        )
        .await
        .unwrap();
        assert!(applied.running);
        assert_eq!(
            apply_backend.actions(),
            vec!["find", "inspect_image", "create", "start", "find"]
        );
        assert_eq!(apply_backend.guard.checks(), 11);

        let stop_backend = FakeManagedTargetDeploymentBackend::new(Some(applied), []);
        let stopped = stop_managed_target_deployment_with_backend(
            &stop_backend,
            &request.reference(),
            0,
            true,
            &stop_backend.guard,
        )
        .await
        .unwrap();
        assert!(stopped.stopped && stopped.removed);
        assert_eq!(stop_backend.actions(), vec!["find", "stop", "remove"]);
        assert_eq!(stop_backend.guard.checks(), 7);
    }

    #[tokio::test]
    async fn deployment_effect_fence_stops_between_stop_and_remove() {
        let request = deployment_request(false);
        let backend = FakeManagedTargetDeploymentBackend::new(
            Some(deployment_observation(&request, "container-1", true)),
            ["stop"],
        );
        let error = stop_managed_target_deployment_with_backend(
            &backend,
            &request.reference(),
            0,
            true,
            &backend.guard,
        )
        .await
        .unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));
        assert_eq!(backend.actions(), vec!["find", "stop"]);

        let backend = FakeManagedTargetDeploymentBackend::new(
            Some(deployment_observation(&request, "container-1", false)),
            ["remove"],
        );
        let error = stop_managed_target_deployment_with_backend(
            &backend,
            &request.reference(),
            0,
            true,
            &backend.guard,
        )
        .await
        .unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));
        assert_eq!(backend.actions(), vec!["find", "remove"]);
    }

    #[test]
    fn container_identity_is_deterministic_and_not_caller_controlled() {
        let first = deployment_container_name("target-1", INSTALLATION_ID, "deployment-1");
        assert_eq!(
            first,
            deployment_container_name("target-1", INSTALLATION_ID, "deployment-1")
        );
        assert_ne!(
            first,
            deployment_container_name("target-2", INSTALLATION_ID, "deployment-1")
        );
        assert_ne!(
            first,
            deployment_container_name(
                "target-1",
                "33333333-3333-4333-8333-333333333333",
                "deployment-1"
            )
        );
        assert!(first.starts_with("plurora-target-"));
        assert_eq!(first.len(), "plurora-target-".len() + 24);
    }

    #[test]
    fn apply_validation_rejects_address_and_command_shaped_images() {
        let mut request = ManagedTargetDeploymentApply {
            target_id: "target-1".to_string(),
            installation_id: InstallationId::parse(INSTALLATION_ID).unwrap(),
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
            installation_id: InstallationId::parse(INSTALLATION_ID).unwrap(),
            workspace_id: WorkspaceId::parse(WORKSPACE_ID).unwrap(),
            build_id: "build-1".to_string(),
            dockerfile: "Dockerfile".to_string(),
            network_mode: ManagedTargetBuildNetworkMode::None,
            disposition: ManagedTargetImageDisposition::RemoveAfterVerification,
            source_tree_digest: format!("sha256:{}", "a".repeat(64)),
            build_descriptor_hash: format!("sha256:{}", "b".repeat(64)),
            context_digest: crate::sha256_digest(&bytes),
            context_tar: bytes,
        };
        validate_image_build_request(&request).unwrap();
        request.context_digest = format!("sha256:{}", "c".repeat(64));
        let error = build_managed_target_image(request, &TestEffectGuard)
            .await
            .unwrap_err();
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
            let installation_id = InstallationId::new();
            let workspace_id = WorkspaceId::new();
            let build_id = format!("build-{target_suffix}-{suffix}");
            let build = finalize_managed_target_image_build(
                build_managed_target_image(
                    ManagedTargetImageBuild {
                        target_id: target_id.to_string(),
                        installation_id: installation_id.clone(),
                        workspace_id: workspace_id.clone(),
                        build_id: build_id.clone(),
                        dockerfile: "Dockerfile".to_string(),
                        network_mode: ManagedTargetBuildNetworkMode::None,
                        disposition: ManagedTargetImageDisposition::RetainForDeployment,
                        source_tree_digest: format!("sha256:{}", "a".repeat(64)),
                        build_descriptor_hash: format!("sha256:{}", "b".repeat(64)),
                        context_digest: context_digest.clone(),
                        context_tar: context_tar.clone(),
                    },
                    &TestEffectGuard,
                )
                .await?,
                &TestEffectGuard,
            )
            .await?;
            let deployment_id = format!("deployment-{target_suffix}-{suffix}");
            let route_id = format!("route-{target_suffix}-{suffix}");
            let lease_id = format!("lease-{target_suffix}-{suffix}");
            let operation_id = format!("operation-{target_suffix}-{suffix}");
            let applied = apply_managed_target_deployment(
                &ManagedTargetDeploymentApply {
                    target_id: target_id.to_string(),
                    installation_id: installation_id.clone(),
                    deployment_id: deployment_id.clone(),
                    route_id: route_id.clone(),
                    port_lease_id: lease_id.clone(),
                    port_name: "http".to_string(),
                    image: build.image_id.clone(),
                    container_port: 80,
                    requested_host_port: None,
                    pull_if_missing: false,
                    operation_id,
                },
                &TestEffectGuard,
            )
            .await?;
            let preview_result = async {
                wait_for_managed_target_deployment_readiness(&applied, Some("/"), &TestEffectGuard)
                    .await?;
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
                    installation_id,
                    deployment_id,
                    route_id,
                    port_lease_id: lease_id,
                },
                0,
                true,
                &TestEffectGuard,
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
            let removed_build = finalize_managed_target_image_build(
                build_managed_target_image(
                    ManagedTargetImageBuild {
                        target_id: target_id.to_string(),
                        installation_id: InstallationId::new(),
                        workspace_id: WorkspaceId::new(),
                        build_id: format!("verify-remove-{target_suffix}-{suffix}"),
                        dockerfile: "Dockerfile".to_string(),
                        network_mode: ManagedTargetBuildNetworkMode::None,
                        disposition: ManagedTargetImageDisposition::RemoveAfterVerification,
                        source_tree_digest: format!("sha256:{}", "c".repeat(64)),
                        build_descriptor_hash: format!("sha256:{}", "d".repeat(64)),
                        context_digest: context_digest.clone(),
                        context_tar: context_tar.clone(),
                    },
                    &TestEffectGuard,
                )
                .await?,
                &TestEffectGuard,
            )
            .await?;
            anyhow::ensure!(
                removed_build.image_removed && !removed_build.image_retained,
                "verification image removal was not confirmed"
            );
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
            installation_id: InstallationId::parse(INSTALLATION_ID).unwrap(),
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
        assert!(value.get("installation_id").is_none());
        assert!(!crate::scan_effect_value_for_raw_secrets(&value, "receipt.output").has_findings());
    }

    fn image_build_receipt(
        disposition: ManagedTargetImageDisposition,
    ) -> ManagedTargetImageBuildReceipt {
        let installation_id = InstallationId::parse(INSTALLATION_ID).unwrap();
        ManagedTargetImageBuildReceipt {
            target_id: "target-1".to_string(),
            image: target_image_tag(installation_id.as_str(), "build-1"),
            image_id: format!("sha256:{}", "1".repeat(64)),
            installation_id,
            workspace_id: WorkspaceId::parse(WORKSPACE_ID).unwrap(),
            build_id: "build-1".to_string(),
            dockerfile: "Dockerfile".to_string(),
            network_mode: ManagedTargetBuildNetworkMode::None,
            disposition,
            context_digest: format!("sha256:{}", "2".repeat(64)),
            source_tree_digest: format!("sha256:{}", "3".repeat(64)),
            build_descriptor_hash: format!("sha256:{}", "4".repeat(64)),
            image_removed: false,
            image_retained: true,
        }
    }

    fn image_build_request(disposition: ManagedTargetImageDisposition) -> ManagedTargetImageBuild {
        let mut archive = tar::Builder::new(Vec::new());
        let dockerfile = b"FROM scratch\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(dockerfile.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive
            .append_data(&mut header, "Dockerfile", dockerfile.as_slice())
            .unwrap();
        let context_tar = archive.into_inner().unwrap();
        ManagedTargetImageBuild {
            target_id: "target-1".to_string(),
            installation_id: InstallationId::parse(INSTALLATION_ID).unwrap(),
            workspace_id: WorkspaceId::parse(WORKSPACE_ID).unwrap(),
            build_id: "build-1".to_string(),
            dockerfile: "Dockerfile".to_string(),
            network_mode: ManagedTargetBuildNetworkMode::None,
            disposition,
            source_tree_digest: format!("sha256:{}", "3".repeat(64)),
            build_descriptor_hash: format!("sha256:{}", "4".repeat(64)),
            context_digest: crate::sha256_digest(&context_tar),
            context_tar,
        }
    }

    #[derive(Debug, Clone)]
    struct FakeManagedTargetImage {
        inspection: ManagedTargetImageInspection,
        tags: HashSet<String>,
    }

    #[derive(Debug, Default)]
    struct FakeManagedTargetImageState {
        images: HashMap<String, FakeManagedTargetImage>,
        removed_ids: Vec<String>,
        list_calls: usize,
        fail_list_calls: HashSet<usize>,
        inspect_calls: usize,
        fail_inspect_calls: HashSet<usize>,
        fail_remove_once: HashSet<String>,
        persistent_stale_list_ids: HashSet<String>,
        build_image_id: Option<String>,
        build_failure: FakeManagedTargetImageBuildFailure,
        revoke_after_remove: Option<(usize, SwitchEffectGuard)>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    enum FakeManagedTargetImageBuildFailure {
        #[default]
        None,
        DaemonTerminal,
        Ambiguous,
    }

    #[derive(Debug, Default)]
    struct FakeManagedTargetImageBackend {
        state: std::sync::Mutex<FakeManagedTargetImageState>,
    }

    impl FakeManagedTargetImageBackend {
        fn insert_image(
            &self,
            id: String,
            tags: impl IntoIterator<Item = String>,
            labels: HashMap<String, String>,
        ) {
            self.state.lock().unwrap().images.insert(
                id.clone(),
                FakeManagedTargetImage {
                    inspection: ManagedTargetImageInspection { id, labels },
                    tags: tags.into_iter().collect(),
                },
            );
        }

        fn fail_list_call(&self, call: usize) {
            self.state.lock().unwrap().fail_list_calls.insert(call);
        }

        fn fail_remove_once(&self, image_id: String) {
            self.state.lock().unwrap().fail_remove_once.insert(image_id);
        }

        fn fail_inspect_call(&self, call: usize) {
            self.state.lock().unwrap().fail_inspect_calls.insert(call);
        }

        fn configure_build(
            &self,
            image_id: Option<String>,
            failure: FakeManagedTargetImageBuildFailure,
        ) {
            let mut state = self.state.lock().unwrap();
            state.build_image_id = image_id;
            state.build_failure = failure;
        }

        fn revoke_after_remove(&self, removal: usize, guard: SwitchEffectGuard) {
            self.state.lock().unwrap().revoke_after_remove = Some((removal, guard));
        }

        fn persist_stale_list_id(&self, image_id: String) {
            self.state
                .lock()
                .unwrap()
                .persistent_stale_list_ids
                .insert(image_id);
        }

        fn contains_image(&self, image_id: &str) -> bool {
            self.state.lock().unwrap().images.contains_key(image_id)
        }

        fn removed_ids(&self) -> Vec<String> {
            self.state.lock().unwrap().removed_ids.clone()
        }

        fn list_calls(&self) -> usize {
            self.state.lock().unwrap().list_calls
        }
    }

    #[async_trait::async_trait]
    impl ManagedTargetImageBackend for FakeManagedTargetImageBackend {
        async fn build_image(
            &self,
            _request: &ManagedTargetImageBuild,
            image: &str,
            labels: &HashMap<String, String>,
            _context_tar: Vec<u8>,
        ) -> Result<(), ManagedTargetImageBuildError> {
            let mut state = self.state.lock().unwrap();
            if let Some(id) = state.build_image_id.clone() {
                state.images.insert(
                    id.clone(),
                    FakeManagedTargetImage {
                        inspection: ManagedTargetImageInspection {
                            id,
                            labels: labels.clone(),
                        },
                        tags: HashSet::from([image.to_string()]),
                    },
                );
            }
            match state.build_failure {
                FakeManagedTargetImageBuildFailure::None => Ok(()),
                FakeManagedTargetImageBuildFailure::DaemonTerminal => {
                    Err(ManagedTargetImageBuildError::DaemonTerminal(
                        anyhow::anyhow!("injected terminal image build failure"),
                    ))
                }
                FakeManagedTargetImageBuildFailure::Ambiguous => {
                    Err(ManagedTargetImageBuildError::Ambiguous(anyhow::anyhow!(
                        "injected ambiguous image build failure"
                    )))
                }
            }
        }

        async fn list_images(
            &self,
            exact_labels: &HashMap<String, String>,
        ) -> anyhow::Result<Vec<String>> {
            let mut state = self.state.lock().unwrap();
            state.list_calls += 1;
            let call = state.list_calls;
            if state.fail_list_calls.remove(&call) {
                anyhow::bail!("injected image list failure");
            }
            let mut ids = state
                .images
                .values()
                .filter(|image| {
                    image_has_exact_provenance_labels(&image.inspection.labels, exact_labels)
                })
                .map(|image| image.inspection.id.clone())
                .collect::<Vec<_>>();
            ids.extend(state.persistent_stale_list_ids.iter().cloned());
            Ok(ids)
        }

        async fn inspect_image(
            &self,
            reference: &str,
        ) -> anyhow::Result<Option<ManagedTargetImageInspection>> {
            let mut state = self.state.lock().unwrap();
            state.inspect_calls += 1;
            let call = state.inspect_calls;
            if state.fail_inspect_calls.remove(&call) {
                anyhow::bail!("injected image inspection failure");
            }
            Ok(state
                .images
                .get(reference)
                .or_else(|| {
                    state
                        .images
                        .values()
                        .find(|image| image.tags.contains(reference))
                })
                .map(|image| image.inspection.clone()))
        }

        async fn remove_image(&self, image_id: &str) -> anyhow::Result<()> {
            let mut state = self.state.lock().unwrap();
            if state.fail_remove_once.remove(image_id) {
                anyhow::bail!("injected image removal failure");
            }
            if state.images.remove(image_id).is_some() {
                state.removed_ids.push(image_id.to_string());
            }
            let revoke = state
                .revoke_after_remove
                .as_ref()
                .filter(|(removal, _)| *removal == state.removed_ids.len())
                .map(|(_, guard)| guard.clone());
            drop(state);
            if let Some(guard) = revoke {
                guard.revoke();
            }
            Ok(())
        }
    }

    #[test]
    fn image_disposition_receipt_requires_exact_labels_and_final_state() {
        let retained = image_build_receipt(ManagedTargetImageDisposition::RetainForDeployment);
        validate_managed_target_image_build_receipt(&retained).unwrap();
        let labels = managed_image_labels(&retained);
        validate_built_image_ownership_labels(&retained, &retained.image_id, &labels).unwrap();

        let mut wrong_target = labels;
        wrong_target.insert("plurora.target_id".to_string(), "target-2".to_string());
        let error =
            validate_built_image_ownership_labels(&retained, &retained.image_id, &wrong_target)
                .unwrap_err();
        assert!(!is_managed_target_deployment_outcome_unknown(&error));

        let mut removed =
            image_build_receipt(ManagedTargetImageDisposition::RemoveAfterVerification);
        assert!(validate_managed_target_image_build_receipt(&removed).is_err());
        removed.image_removed = true;
        removed.image_retained = false;
        validate_managed_target_image_build_receipt(&removed).unwrap();
    }

    #[test]
    fn build_request_and_receipt_use_the_same_complete_managed_image_labels() {
        let installation_id = InstallationId::parse(INSTALLATION_ID).unwrap();
        let request = ManagedTargetImageBuild {
            target_id: "target-1".to_string(),
            installation_id: installation_id.clone(),
            workspace_id: WorkspaceId::parse(WORKSPACE_ID).unwrap(),
            build_id: "build-1".to_string(),
            dockerfile: "docker/Dockerfile".to_string(),
            network_mode: ManagedTargetBuildNetworkMode::Bridge,
            disposition: ManagedTargetImageDisposition::RemoveAfterVerification,
            source_tree_digest: format!("sha256:{}", "3".repeat(64)),
            build_descriptor_hash: format!("sha256:{}", "4".repeat(64)),
            context_digest: format!("sha256:{}", "2".repeat(64)),
            context_tar: Vec::new(),
        };
        let build_labels = managed_image_labels(&request);
        let receipt = managed_image_build_receipt(
            request,
            target_image_tag(installation_id.as_str(), "build-1"),
            format!("sha256:{}", "1".repeat(64)),
        );

        assert_eq!(build_labels, managed_image_labels(&receipt));
        assert_eq!(build_labels.len(), 12);
    }

    #[tokio::test]
    async fn image_cleanup_removes_receipt_and_rebuilt_ids_but_preserves_near_match() {
        let receipt = image_build_receipt(ManagedTargetImageDisposition::RemoveAfterVerification);
        let labels = managed_image_labels(&receipt);
        let first_id = receipt.image_id.clone();
        let rebuilt_id = format!("sha256:{}", "5".repeat(64));
        let external_id = format!("sha256:{}", "6".repeat(64));
        let backend = FakeManagedTargetImageBackend::default();
        backend.insert_image(first_id.clone(), [], labels.clone());
        backend.insert_image(rebuilt_id.clone(), [receipt.image.clone()], labels.clone());
        let mut near_match = labels;
        near_match.remove("plurora.build_context_digest");
        backend.insert_image(
            external_id.clone(),
            ["external/near-match:latest".to_string()],
            near_match,
        );

        cleanup_managed_target_images(
            &backend,
            &receipt,
            &receipt.image,
            Some(&receipt.image_id),
            &TestEffectGuard,
        )
        .await
        .unwrap();

        assert!(!backend.contains_image(&first_id));
        assert!(!backend.contains_image(&rebuilt_id));
        assert!(backend.contains_image(&external_id));
        let removed = backend.removed_ids();
        assert!(removed.contains(&first_id));
        assert!(removed.contains(&rebuilt_id));
        assert!(!removed.contains(&external_id));
    }

    #[tokio::test]
    async fn image_cleanup_retries_partial_and_after_remove_before_receipt_windows() {
        let receipt = image_build_receipt(ManagedTargetImageDisposition::RemoveAfterVerification);
        let labels = managed_image_labels(&receipt);
        let first_id = receipt.image_id.clone();
        let rebuilt_id = format!("sha256:{}", "5".repeat(64));
        let backend = FakeManagedTargetImageBackend::default();
        backend.insert_image(first_id.clone(), [], labels.clone());
        backend.insert_image(rebuilt_id.clone(), [receipt.image.clone()], labels);
        backend.fail_remove_once(rebuilt_id.clone());

        let error = cleanup_managed_target_images(
            &backend,
            &receipt,
            &receipt.image,
            Some(&receipt.image_id),
            &TestEffectGuard,
        )
        .await
        .unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));
        assert!(!backend.contains_image(&first_id));
        assert!(backend.contains_image(&rebuilt_id));

        cleanup_managed_target_images(
            &backend,
            &receipt,
            &receipt.image,
            Some(&receipt.image_id),
            &TestEffectGuard,
        )
        .await
        .unwrap();
        assert!(!backend.contains_image(&rebuilt_id));

        cleanup_managed_target_images(
            &backend,
            &receipt,
            &receipt.image,
            Some(&receipt.image_id),
            &TestEffectGuard,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn image_cleanup_fails_closed_on_a_persistently_stale_list_entry() {
        let receipt = image_build_receipt(ManagedTargetImageDisposition::RemoveAfterVerification);
        let stale_id = format!("sha256:{}", "7".repeat(64));
        let backend = FakeManagedTargetImageBackend::default();
        backend.persist_stale_list_id(stale_id);

        let error = cleanup_managed_target_images(
            &backend,
            &receipt,
            &receipt.image,
            Some(&receipt.image_id),
            &TestEffectGuard,
        )
        .await
        .unwrap_err();

        assert!(is_managed_target_deployment_outcome_unknown(&error));
        assert_eq!(backend.list_calls(), 1);
        assert!(backend.removed_ids().is_empty());
    }

    #[tokio::test]
    async fn image_cleanup_query_and_recheck_failures_are_outcome_unknown() {
        let receipt = image_build_receipt(ManagedTargetImageDisposition::RemoveAfterVerification);
        let query_failure = FakeManagedTargetImageBackend::default();
        query_failure.fail_list_call(1);
        let error = cleanup_managed_target_images(
            &query_failure,
            &receipt,
            &receipt.image,
            Some(&receipt.image_id),
            &TestEffectGuard,
        )
        .await
        .unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));

        let recheck_failure = FakeManagedTargetImageBackend::default();
        recheck_failure.fail_list_call(2);
        let error = cleanup_managed_target_images(
            &recheck_failure,
            &receipt,
            &receipt.image,
            Some(&receipt.image_id),
            &TestEffectGuard,
        )
        .await
        .unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));
    }

    #[tokio::test]
    async fn remove_after_verification_cleans_images_after_build_and_inspect_failures() {
        let image_id = format!("sha256:{}", "8".repeat(64));

        let inspect_failure = FakeManagedTargetImageBackend::default();
        inspect_failure.configure_build(
            Some(image_id.clone()),
            FakeManagedTargetImageBuildFailure::None,
        );
        inspect_failure.fail_inspect_call(1);
        let error = build_managed_target_image_with_backend(
            &inspect_failure,
            image_build_request(ManagedTargetImageDisposition::RemoveAfterVerification),
            &TestEffectGuard,
        )
        .await
        .unwrap_err();
        assert!(!is_managed_target_deployment_outcome_unknown(&error));
        assert!(!inspect_failure.contains_image(&image_id));
        assert_eq!(inspect_failure.removed_ids(), vec![image_id.clone()]);

        let ambiguous_build = FakeManagedTargetImageBackend::default();
        ambiguous_build.configure_build(
            Some(image_id.clone()),
            FakeManagedTargetImageBuildFailure::Ambiguous,
        );
        let error = build_managed_target_image_with_backend(
            &ambiguous_build,
            image_build_request(ManagedTargetImageDisposition::RemoveAfterVerification),
            &TestEffectGuard,
        )
        .await
        .unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));
        assert!(!ambiguous_build.contains_image(&image_id));
        assert_eq!(ambiguous_build.removed_ids(), vec![image_id.clone()]);

        let retained = FakeManagedTargetImageBackend::default();
        retained.configure_build(
            Some(image_id.clone()),
            FakeManagedTargetImageBuildFailure::None,
        );
        retained.fail_inspect_call(1);
        let error = build_managed_target_image_with_backend(
            &retained,
            image_build_request(ManagedTargetImageDisposition::RetainForDeployment),
            &TestEffectGuard,
        )
        .await
        .unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));
        assert!(retained.contains_image(&image_id));
        assert!(retained.removed_ids().is_empty());
    }

    #[tokio::test]
    async fn build_failure_certainty_controls_cleanup_and_terminal_status() {
        let empty_ambiguous = FakeManagedTargetImageBackend::default();
        empty_ambiguous.configure_build(None, FakeManagedTargetImageBuildFailure::Ambiguous);
        let error = build_managed_target_image_with_backend(
            &empty_ambiguous,
            image_build_request(ManagedTargetImageDisposition::RemoveAfterVerification),
            &TestEffectGuard,
        )
        .await
        .unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));
        assert!(empty_ambiguous.removed_ids().is_empty());
        assert_eq!(empty_ambiguous.list_calls(), 2);

        let empty_terminal = FakeManagedTargetImageBackend::default();
        empty_terminal.configure_build(None, FakeManagedTargetImageBuildFailure::DaemonTerminal);
        let error = build_managed_target_image_with_backend(
            &empty_terminal,
            image_build_request(ManagedTargetImageDisposition::RemoveAfterVerification),
            &TestEffectGuard,
        )
        .await
        .unwrap_err();
        assert!(!is_managed_target_deployment_outcome_unknown(&error));
        assert!(empty_terminal.removed_ids().is_empty());
        assert_eq!(empty_terminal.list_calls(), 2);

        let cleanup_unknown = FakeManagedTargetImageBackend::default();
        cleanup_unknown.configure_build(None, FakeManagedTargetImageBuildFailure::Ambiguous);
        cleanup_unknown.fail_list_call(1);
        let error = build_managed_target_image_with_backend(
            &cleanup_unknown,
            image_build_request(ManagedTargetImageDisposition::RemoveAfterVerification),
            &TestEffectGuard,
        )
        .await
        .unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));

        let retained_id = format!("sha256:{}", "a".repeat(64));
        for failure in [
            FakeManagedTargetImageBuildFailure::Ambiguous,
            FakeManagedTargetImageBuildFailure::DaemonTerminal,
        ] {
            let retained = FakeManagedTargetImageBackend::default();
            retained.configure_build(Some(retained_id.clone()), failure);
            let error = build_managed_target_image_with_backend(
                &retained,
                image_build_request(ManagedTargetImageDisposition::RetainForDeployment),
                &TestEffectGuard,
            )
            .await
            .unwrap_err();
            assert_eq!(
                is_managed_target_deployment_outcome_unknown(&error),
                failure == FakeManagedTargetImageBuildFailure::Ambiguous
            );
            assert!(retained.contains_image(&retained_id));
            assert!(retained.removed_ids().is_empty());
            assert_eq!(retained.list_calls(), 0);
        }
    }

    #[tokio::test]
    async fn cleanup_inspection_remove_and_final_confirmation_failures_are_outcome_unknown() {
        let receipt = image_build_receipt(ManagedTargetImageDisposition::RemoveAfterVerification);
        let labels = managed_image_labels(&receipt);

        let inspect_failure = FakeManagedTargetImageBackend::default();
        inspect_failure.insert_image(receipt.image_id.clone(), [], labels.clone());
        inspect_failure.fail_inspect_call(1);
        let inspect_guard = SwitchEffectGuard::new_current();
        let error = cleanup_managed_target_images(
            &inspect_failure,
            &receipt,
            &receipt.image,
            Some(&receipt.image_id),
            &inspect_guard,
        )
        .await
        .unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));
        assert_eq!(inspect_guard.checks(), 4);

        let remove_failure = FakeManagedTargetImageBackend::default();
        remove_failure.insert_image(receipt.image_id.clone(), [], labels.clone());
        remove_failure.fail_remove_once(receipt.image_id.clone());
        let error = cleanup_managed_target_images(
            &remove_failure,
            &receipt,
            &receipt.image,
            Some(&receipt.image_id),
            &TestEffectGuard,
        )
        .await
        .unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));

        let final_confirmation_failure = FakeManagedTargetImageBackend::default();
        final_confirmation_failure.insert_image(receipt.image_id.clone(), [], labels);
        final_confirmation_failure.fail_inspect_call(5);
        let error = cleanup_managed_target_images(
            &final_confirmation_failure,
            &receipt,
            &receipt.image,
            Some(&receipt.image_id),
            &TestEffectGuard,
        )
        .await
        .unwrap_err();
        assert!(is_managed_target_deployment_outcome_unknown(&error));
    }

    #[tokio::test]
    async fn image_cleanup_effect_fence_stops_between_exact_image_deletes() {
        let receipt = image_build_receipt(ManagedTargetImageDisposition::RemoveAfterVerification);
        let labels = managed_image_labels(&receipt);
        let first_id = receipt.image_id.clone();
        let second_id = format!("sha256:{}", "9".repeat(64));
        let guard = SwitchEffectGuard::new_current();
        let backend = FakeManagedTargetImageBackend::default();
        backend.insert_image(first_id.clone(), [], labels.clone());
        backend.insert_image(second_id.clone(), [], labels);
        backend.revoke_after_remove(1, guard.clone());

        let error = cleanup_managed_target_images(
            &backend,
            &receipt,
            &receipt.image,
            Some(&receipt.image_id),
            &guard,
        )
        .await
        .unwrap_err();

        assert!(is_managed_target_deployment_outcome_unknown(&error));
        assert_eq!(backend.removed_ids().len(), 1);
        assert_eq!(
            usize::from(backend.contains_image(&first_id))
                + usize::from(backend.contains_image(&second_id)),
            1
        );
    }

    #[test]
    fn docker_not_found_detection_uses_the_structured_status() {
        let not_found = bollard::errors::Error::DockerResponseServerError {
            status_code: 404,
            message: "not found".to_string(),
        };
        assert!(docker_not_found_error(&not_found));
        let unavailable = bollard::errors::Error::DockerResponseServerError {
            status_code: 503,
            message: "not found".to_string(),
        };
        assert!(!docker_not_found_error(&unavailable));
    }
}
