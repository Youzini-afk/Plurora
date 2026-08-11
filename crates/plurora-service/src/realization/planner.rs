use std::collections::BTreeMap;

use axum::body::Bytes;
use chrono::{TimeZone, Utc};
use plurora_core::ArtifactDescriptor;
use plurora_runtime::{
    DockerBuildBackendSelection, ExecutionTarget, ExecutionTargetCapability, ObjectStore,
    OciImageBackendSelection, RealizationBackendSelection,
};
use plurora_work::{
    canonical_json_bytes, BuildAction, PlannedWorkloadInput, ResourceCapacity,
    TargetCapabilityRecord, TargetInventorySnapshot, EXECUTION_CLASS_CAPABILITY_PREFIX,
};
use serde::Serialize;
use serde_json::json;

pub(super) const OCI_IMAGE_REFERENCE_TYPE_URI: &str =
    "urn:plurora:realization-backend:oci-image-reference:v1";
pub(super) const BACKEND_PARAMETERS_TYPE_URI: &str =
    "urn:plurora:realization-backend:parameters:v1";
pub(super) const BUILD_OUTPUT_REFERENCE_TYPE_URI: &str =
    "urn:plurora:realization-backend:build-output-reference:v1";
pub(super) const REALIZATION_APPROVAL_TYPE_URI: &str = "urn:plurora:realization-approval:v1";

pub(super) async fn prepare_backend_artifacts(
    object_store: &dyn ObjectStore,
    selection: &RealizationBackendSelection,
    node_id: &plurora_work::NodeId,
    target_id: &str,
) -> anyhow::Result<PlannedWorkloadInput> {
    match selection {
        RealizationBackendSelection::OciImage(selection) => validate_oci_selection(selection)?,
        RealizationBackendSelection::DockerBuild(selection) => validate_build_selection(selection)?,
    }
    let parameter_ref = put_json_artifact(
        object_store,
        BACKEND_PARAMETERS_TYPE_URI,
        selection,
        Vec::new(),
        BTreeMap::new(),
    )
    .await?;
    match selection {
        RealizationBackendSelection::OciImage(selection) => {
            let launch_artifact = put_json_artifact(
                object_store,
                OCI_IMAGE_REFERENCE_TYPE_URI,
                &json!({
                    "schema": "plurora.realization-backend.oci-image-reference.v1",
                    "image": selection.image,
                }),
                Vec::new(),
                BTreeMap::from([("image".to_string(), json!(selection.image))]),
            )
            .await?;
            Ok(PlannedWorkloadInput {
                workload_id: selection.workload_id.clone(),
                target_id: target_id.to_string(),
                execution_class: selection.execution_class.clone(),
                launch_artifact,
                parameter_ref: Some(parameter_ref),
                build_action: None,
            })
        }
        RealizationBackendSelection::DockerBuild(selection) => {
            object_store
                .verify(&selection.build_context_ref.digest)
                .await?;
            let action_id = format!("build-{}", selection.workload_id);
            let output_id = format!("image-{}", selection.workload_id);
            let launch_artifact = put_json_artifact(
                object_store,
                BUILD_OUTPUT_REFERENCE_TYPE_URI,
                &json!({
                    "schema": "plurora.realization-backend.build-output-reference.v1",
                    "build_action_id": action_id,
                    "output_id": output_id,
                }),
                vec![selection.build_context_ref.digest.clone()],
                BTreeMap::new(),
            )
            .await?;
            Ok(PlannedWorkloadInput {
                workload_id: selection.workload_id.clone(),
                target_id: target_id.to_string(),
                execution_class: selection.execution_class.clone(),
                launch_artifact,
                parameter_ref: Some(parameter_ref.clone()),
                build_action: Some(BuildAction {
                    action_id,
                    node_id: node_id.clone(),
                    builder_id: "plurora.builder/dockerfile.v1".to_string(),
                    input_refs: vec![selection.build_context_ref.clone()],
                    parameter_ref,
                    output_id,
                }),
            })
        }
    }
}

pub(super) async fn persist_target_inventory(
    object_store: &dyn ObjectStore,
    target: &ExecutionTarget,
) -> anyhow::Result<(ArtifactDescriptor, TargetInventorySnapshot)> {
    let snapshot = target_inventory(target);
    snapshot
        .validate()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let descriptor = put_json_artifact(
        object_store,
        plurora_work::TARGET_INVENTORY_TYPE_URI,
        &snapshot,
        Vec::new(),
        BTreeMap::from([("target_id".to_string(), json!(target.id))]),
    )
    .await?;
    Ok((descriptor, snapshot))
}

pub(super) fn target_inventory(target: &ExecutionTarget) -> TargetInventorySnapshot {
    let mut capabilities = target
        .capabilities
        .iter()
        .map(|capability| TargetCapabilityRecord {
            capability_id: execution_target_capability_id(*capability).to_string(),
            version: "1".to_string(),
            properties: BTreeMap::new(),
            available: true,
            evidence_refs: Vec::new(),
        })
        .collect::<Vec<_>>();
    if target
        .capabilities
        .contains(&ExecutionTargetCapability::Deployment)
    {
        for execution_class in ["oci-container.v1", "docker-build.v1"] {
            capabilities.push(TargetCapabilityRecord {
                capability_id: format!("{EXECUTION_CLASS_CAPABILITY_PREFIX}{execution_class}"),
                version: "1".to_string(),
                properties: BTreeMap::new(),
                available: true,
                evidence_refs: Vec::new(),
            });
        }
    }
    capabilities.sort_by(|left, right| {
        left.capability_id
            .cmp(&right.capability_id)
            .then_with(|| left.version.cmp(&right.version))
    });
    TargetInventorySnapshot {
        target_id: target.id.clone(),
        observed_at: Utc
            .timestamp_millis_opt(
                target
                    .last_seen_at_ms
                    .or(target.enrolled_at_ms)
                    .unwrap_or_default(),
            )
            .single()
            .unwrap_or_else(|| {
                Utc.timestamp_millis_opt(0)
                    .single()
                    .expect("Unix epoch is a valid UTC timestamp")
            }),
        capabilities,
        // The current Target projection reports usage, not an advertised hard
        // capacity. Zero means "unspecified" in the portable inventory model;
        // inventing a ceiling from current usage would reject valid plans.
        capacity: ResourceCapacity::default(),
        labels: target.labels.clone(),
        trust_zone: target.labels.get("trust_zone").cloned().unwrap_or_else(|| {
            match target.reachability {
                plurora_runtime::ExecutionTargetReachability::LocalHost => "host-local".to_string(),
                _ => "managed-remote".to_string(),
            }
        }),
        topology: Vec::new(),
    }
}

fn execution_target_capability_id(capability: ExecutionTargetCapability) -> &'static str {
    match capability {
        ExecutionTargetCapability::LocalExec => "plurora.target/local-exec",
        ExecutionTargetCapability::PortLease => "plurora.target/port-lease",
        ExecutionTargetCapability::HttpProxyUpstream => "plurora.target/http-proxy-upstream",
        ExecutionTargetCapability::WebsocketProxyUpstream => {
            "plurora.target/websocket-proxy-upstream"
        }
        ExecutionTargetCapability::ArtifactTransfer => "plurora.target/artifact-transfer",
        ExecutionTargetCapability::DeclarativeVerifier => "plurora.target/declarative-verifier",
        ExecutionTargetCapability::HealthProbe => "plurora.target/health-probe",
        ExecutionTargetCapability::Deployment => "plurora.target/managed-workload",
        ExecutionTargetCapability::AuthenticatedTunnel => "plurora.target/authenticated-tunnel",
    }
}

fn validate_oci_selection(selection: &OciImageBackendSelection) -> anyhow::Result<()> {
    validate_common(
        &selection.workload_id,
        &selection.execution_class,
        selection.container_port,
        &selection.port_name,
        &selection.route_id,
        selection.health_path.as_deref(),
    )?;
    let Some((name, digest)) = selection.image.rsplit_once('@') else {
        anyhow::bail!("plan_stale: OCI image must use a content digest");
    };
    anyhow::ensure!(
        !name.is_empty()
            && digest.len() == 71
            && digest.starts_with("sha256:")
            && digest[7..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "artifact_digest_mismatch: OCI image digest is invalid"
    );
    Ok(())
}

fn validate_build_selection(selection: &DockerBuildBackendSelection) -> anyhow::Result<()> {
    validate_common(
        &selection.workload_id,
        &selection.execution_class,
        selection.container_port,
        &selection.port_name,
        &selection.route_id,
        selection.health_path.as_deref(),
    )?;
    validate_sha256(&selection.build_context_ref.digest)?;
    validate_sha256(&selection.source_tree_digest)?;
    validate_sha256(&selection.build_descriptor_hash)?;
    anyhow::ensure!(
        !selection.dockerfile.is_empty()
            && !selection.dockerfile.starts_with(['/', '\\'])
            && !selection
                .dockerfile
                .split(['/', '\\'])
                .any(|part| part == ".."),
        "work_invalid: Dockerfile path must be relative"
    );
    Ok(())
}

fn validate_sha256(value: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        value.len() == 71
            && value.starts_with("sha256:")
            && value[7..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "artifact_digest_mismatch: SHA-256 digest is invalid"
    );
    Ok(())
}

fn validate_common(
    workload_id: &str,
    execution_class: &str,
    container_port: u16,
    port_name: &str,
    route_id: &str,
    health_path: Option<&str>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !workload_id.trim().is_empty()
            && !execution_class.trim().is_empty()
            && container_port > 0
            && valid_token(port_name)
            && valid_token(route_id),
        "work_invalid: backend workload, execution class, port, or route is invalid"
    );
    if let Some(path) = health_path {
        anyhow::ensure!(
            path.starts_with('/') && !path.starts_with("//") && !path.contains(['\r', '\n']),
            "work_invalid: health path is invalid"
        );
    }
    Ok(())
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._".contains(&byte))
}

pub(super) async fn put_json_artifact<T>(
    object_store: &dyn ObjectStore,
    artifact_type_uri: &str,
    value: &T,
    references: Vec<String>,
    annotations: BTreeMap<String, serde_json::Value>,
) -> anyhow::Result<ArtifactDescriptor>
where
    T: Serialize,
{
    let bytes = canonical_json_bytes(value).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let info = object_store.put(Bytes::from(bytes)).await?;
    Ok(ArtifactDescriptor {
        artifact_type_uri: artifact_type_uri.to_string(),
        media_type: plurora_work::CANONICAL_JSON_MEDIA_TYPE.to_string(),
        digest: info.digest,
        size_bytes: info.size_bytes,
        references,
        annotations,
    })
}
