use std::collections::BTreeMap;
use std::sync::{Arc, Weak};
use std::time::Duration;

use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use plurora_runtime::{
    EventStore, PortLeaseRequest, PortProtocol, ProxyProtocol, ProxyRouteRegisterRequest,
    ProxyRouteUpstream, RealizationAuthoritySubject, RealizationBackendSelection,
    RealizationEffectKind, RealizationEffectReceipt, RealizationMutationAuthority, Runtime,
};
use plurora_work::{RealizationPlan, RealizationRevision, RealizationStatus, RealizedResource};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::partial_execution_error;
use super::{RealizationExecution, RealizationExecutionDriver, RealizationObservation};
use crate::target_agent::{submit_host_operation, wait_for_host_operation};
use crate::{
    AppState, BuildDeployJobRegistry, DevelopmentRegistry, HostAccessRegistry,
    InstallationRegistry, TargetAgentRegistry,
};
use crate::{
    CreateTargetOperationRequest, DeclarativeVerifierDescriptor, TargetDeploymentDescriptor,
    TargetDeploymentRef, TargetOperationReceiptStatus, TargetOperationRecord, TargetOperationSpec,
    TargetOperationStatusKind,
};

#[derive(Debug, Clone)]
pub struct RealizationBackendReconcileSummary {
    pub target_workloads_projected: usize,
    pub runtime: plurora_runtime::DeploymentReconcileSummary,
}

pub async fn reconcile_realization_backends<S>(
    state: &AppState<S>,
) -> anyhow::Result<RealizationBackendReconcileSummary>
where
    S: EventStore,
{
    let target_workloads_projected =
        crate::target_agent::reconcile_target_deployment_control_plane(state).await?;
    let runtime = state.runtime.reconcile_deployment().await?;
    Ok(RealizationBackendReconcileSummary {
        target_workloads_projected,
        runtime,
    })
}

pub struct ServiceRealizationExecutor<S>
where
    S: EventStore,
{
    runtime: Weak<Runtime<S>>,
    build_jobs: Arc<BuildDeployJobRegistry>,
    development: Arc<DevelopmentRegistry>,
    host_access: Arc<HostAccessRegistry>,
    installations: Arc<InstallationRegistry>,
    target_agents: Arc<TargetAgentRegistry>,
}

impl<S> ServiceRealizationExecutor<S>
where
    S: EventStore,
{
    pub fn new(state: &AppState<S>) -> Self {
        Self {
            runtime: Arc::downgrade(&state.runtime),
            build_jobs: state.build_jobs.clone(),
            development: state.development.clone(),
            host_access: state.host_access.clone(),
            installations: state.installations.clone(),
            target_agents: state.target_agents.clone(),
        }
    }

    fn state(&self) -> anyhow::Result<AppState<S>> {
        Ok(AppState {
            runtime: self
                .runtime
                .upgrade()
                .ok_or_else(|| anyhow!("recovery_required: Runtime is unavailable"))?,
            static_dir: None,
            access_token: None,
            app_base_domain: None,
            build_jobs: self.build_jobs.clone(),
            development: self.development.clone(),
            host_access: self.host_access.clone(),
            installations: self.installations.clone(),
            target_agents: self.target_agents.clone(),
        })
    }

    async fn run_operation(
        &self,
        state: &AppState<S>,
        target_id: &str,
        installation_id: &plurora_work::InstallationId,
        spec: TargetOperationSpec,
        idempotency_key: String,
        authority: &RealizationMutationAuthority,
        subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<TargetOperationRecord> {
        authority.refresh_current_for(subject).await?;
        let operation = submit_host_operation(
            state,
            target_id,
            CreateTargetOperationRequest {
                installation_id: installation_id.clone(),
                spec,
                idempotency_key: Some(idempotency_key),
                expires_in_seconds: Some(120),
            },
        )
        .await
        .map_err(|_| anyhow!("target_unsatisfied: Target rejected the typed operation"))?;
        let operation = if operation.status.is_terminal() {
            operation
        } else {
            wait_for_host_operation(
                state,
                target_id,
                &operation.operation_id,
                Duration::from_secs(120),
            )
            .await?
        };
        if authority.refresh_current_for(subject).await.is_err() {
            return Err(plurora_runtime::managed_target_deployment_outcome_unknown(
                "Realization authority changed during target effect",
            ));
        }
        ensure_operation_succeeded(&operation)?;
        Ok(operation)
    }

    async fn prepare_route(
        &self,
        state: &AppState<S>,
        target_id: &str,
        port_name: &str,
        route_id: &str,
        route_access: plurora_runtime::ProxyRouteAccess,
        authority: &RealizationMutationAuthority,
        subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<TargetDeploymentRef> {
        authority.refresh_current_for(subject).await?;
        let lease = state
            .runtime
            .config()
            .port_lease_registry
            .lease(PortLeaseRequest {
                target_id: target_id.to_string(),
                port_name: port_name.to_string(),
                protocol: PortProtocol::Tcp,
                requested_port: None,
            })
            .await
            .lease;
        if authority.refresh_current_for(subject).await.is_err() {
            state
                .runtime
                .config()
                .port_lease_registry
                .release(&lease.id)
                .await;
            return Err(anyhow!(
                "authority_denied: Realization authority changed before route registration"
            ));
        }
        let route = state
            .runtime
            .config()
            .proxy_route_registry
            .register(ProxyRouteRegisterRequest {
                route_id: Some(route_id.to_string()),
                upstream: ProxyRouteUpstream {
                    port_lease_id: lease.id.clone(),
                    port_name: port_name.to_string(),
                },
                protocol: ProxyProtocol::Http,
                access: route_access,
            })
            .await
            .route;
        Ok(TargetDeploymentRef {
            deployment_id: route_id.to_string(),
            route_id: route.id,
            port_lease_id: lease.id,
        })
    }

    async fn cleanup_route(&self, state: &AppState<S>, reference: &TargetDeploymentRef) {
        state
            .runtime
            .config()
            .proxy_route_registry
            .unregister(&reference.route_id)
            .await;
        state
            .runtime
            .config()
            .port_lease_registry
            .release(&reference.port_lease_id)
            .await;
    }

    async fn image_for_backend(
        &self,
        state: &AppState<S>,
        realization: &RealizationRevision,
        target_id: &str,
        backend: &RealizationBackendSelection,
        authority: &RealizationMutationAuthority,
        subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<(String, Option<RealizationEffectReceipt>)> {
        match backend {
            RealizationBackendSelection::OciImage(selection) => Ok((selection.image.clone(), None)),
            RealizationBackendSelection::DockerBuild(selection) => {
                let operation = self
                    .run_operation(
                        state,
                        target_id,
                        &realization.installation_id,
                        TargetOperationSpec::VerifierRun {
                            verifier: DeclarativeVerifierDescriptor::DockerBuild {
                                digest: selection.build_context_ref.digest.clone(),
                                expected_size_bytes: Some(selection.build_context_ref.size_bytes),
                                dockerfile: selection.dockerfile.clone(),
                                network_mode: selection.network_mode,
                                disposition: plurora_runtime::ManagedTargetImageDisposition::RetainForDeployment,
                                build_id: realization_build_id(
                                    &realization.realization_id,
                                    &selection.workload_id,
                                ),
                                workspace_id: selection.workspace_id.clone(),
                                source_tree_digest: selection.source_tree_digest.clone(),
                                build_descriptor_hash: selection.build_descriptor_hash.clone(),
                            },
                        },
                        format!("realization:{}:build:{}", realization.realization_id, selection.workload_id),
                        authority,
                        subject,
                    )
                    .await?;
                let receipt = effect_receipt(
                    realization,
                    target_id,
                    RealizationEffectKind::Build,
                    &operation,
                )?;
                let Some(image) = operation
                    .receipt
                    .as_ref()
                    .and_then(|receipt| receipt.output.get("image"))
                    .and_then(Value::as_str)
                else {
                    return Err(partial_execution_error(
                        "recovery_required",
                        Vec::new(),
                        vec![receipt],
                    ));
                };
                if !image.contains("@sha256:") {
                    return Err(partial_execution_error(
                        "recovery_required",
                        Vec::new(),
                        vec![receipt],
                    ));
                }
                Ok((image.to_string(), Some(receipt)))
            }
        }
    }

    async fn cleanup_failed_launch(
        &self,
        state: &AppState<S>,
        realization: &RealizationRevision,
        target_id: &str,
        workload_id: &str,
        reference: &TargetDeploymentRef,
        authority: &RealizationMutationAuthority,
        subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<RealizationEffectReceipt> {
        let stopped = self
            .run_operation(
                state,
                target_id,
                &realization.installation_id,
                TargetOperationSpec::DeploymentStop {
                    deployment: reference.clone(),
                    grace_seconds: 0,
                    force_remove: true,
                },
                format!(
                    "realization:{}:cleanup:{workload_id}",
                    realization.realization_id
                ),
                authority,
                subject,
            )
            .await;
        self.cleanup_route(state, reference).await;
        let operation = stopped?;
        effect_receipt(
            realization,
            target_id,
            RealizationEffectKind::Stop,
            &operation,
        )
    }
}

#[async_trait]
impl<S> RealizationExecutionDriver for ServiceRealizationExecutor<S>
where
    S: EventStore,
{
    async fn apply(
        &self,
        realization: &RealizationRevision,
        target_id: &str,
        _plan: &RealizationPlan,
        backends: &[RealizationBackendSelection],
        authority: &RealizationMutationAuthority,
        subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<RealizationExecution> {
        let state = self.state()?;
        let mut resources = Vec::new();
        let mut receipts = Vec::new();
        for backend in backends {
            let (workload_id, port_name, route_id, route_access, container_port, health_path, pull) =
                match backend {
                    RealizationBackendSelection::OciImage(selection) => (
                        selection.workload_id.as_str(),
                        selection.port_name.as_str(),
                        selection.route_id.as_str(),
                        selection.route_access,
                        selection.container_port,
                        selection.health_path.clone(),
                        selection.pull_if_missing,
                    ),
                    RealizationBackendSelection::DockerBuild(selection) => (
                        selection.workload_id.as_str(),
                        selection.port_name.as_str(),
                        selection.route_id.as_str(),
                        selection.route_access,
                        selection.container_port,
                        selection.health_path.clone(),
                        false,
                    ),
                };
            let (image, build_receipt) = self
                .image_for_backend(&state, realization, target_id, backend, authority, subject)
                .await?;
            if let Some(build_receipt) = build_receipt {
                receipts.push(build_receipt);
            }
            let reference = match self
                .prepare_route(
                    &state,
                    target_id,
                    port_name,
                    route_id,
                    route_access,
                    authority,
                    subject,
                )
                .await
            {
                Ok(reference) => reference,
                Err(_error) if !receipts.is_empty() => {
                    return Err(partial_execution_error(
                        "recovery_required",
                        Vec::new(),
                        receipts,
                    ));
                }
                Err(error) => return Err(error),
            };
            let deployment = TargetDeploymentDescriptor {
                deployment: reference.clone(),
                port_name: port_name.to_string(),
                image: image.clone(),
                container_port,
                requested_host_port: None,
                pull_if_missing: pull,
                health_path,
            };
            let applied = self
                .run_operation(
                    &state,
                    target_id,
                    &realization.installation_id,
                    TargetOperationSpec::DeploymentApply { deployment },
                    format!(
                        "realization:{}:launch:{workload_id}",
                        realization.realization_id
                    ),
                    authority,
                    &subject,
                )
                .await;
            let operation = match applied {
                Ok(operation) => operation,
                Err(error) => {
                    let cleanup = self
                        .cleanup_failed_launch(
                            &state,
                            realization,
                            target_id,
                            workload_id,
                            &reference,
                            authority,
                            subject,
                        )
                        .await;
                    if let Ok(receipt) = cleanup {
                        receipts.push(receipt);
                    } else {
                        return Err(partial_execution_error(
                            "outcome_unknown",
                            vec![provisional_resource(
                                workload_id,
                                target_id,
                                &reference,
                                &image,
                            )],
                            receipts,
                        ));
                    }
                    return if receipts.is_empty() {
                        Err(error)
                    } else {
                        Err(partial_execution_error(
                            "recovery_required",
                            Vec::new(),
                            receipts,
                        ))
                    };
                }
            };
            let launch_receipt = match effect_receipt(
                realization,
                target_id,
                RealizationEffectKind::Launch,
                &operation,
            ) {
                Ok(receipt) => receipt,
                Err(_) => {
                    let cleanup = self
                        .cleanup_failed_launch(
                            &state,
                            realization,
                            target_id,
                            workload_id,
                            &reference,
                            authority,
                            subject,
                        )
                        .await;
                    if let Ok(receipt) = cleanup {
                        receipts.push(receipt);
                        return Err(partial_execution_error(
                            "recovery_required",
                            Vec::new(),
                            receipts,
                        ));
                    }
                    return Err(partial_execution_error(
                        "outcome_unknown",
                        vec![provisional_resource(
                            workload_id,
                            target_id,
                            &reference,
                            &image,
                        )],
                        receipts,
                    ));
                }
            };
            receipts.push(launch_receipt);
            let route = state
                .runtime
                .config()
                .proxy_route_registry
                .status(&reference.route_id)
                .await
                .ok_or_else(|| anyhow!("recovery_required: active route disappeared"));
            let lease = state
                .runtime
                .config()
                .port_lease_registry
                .status(&reference.port_lease_id)
                .await
                .ok_or_else(|| anyhow!("recovery_required: active port lease disappeared"));
            let lease = match (route, lease) {
                (Ok(_route), Ok(lease)) => lease,
                _ => {
                    let cleanup = self
                        .cleanup_failed_launch(
                            &state,
                            realization,
                            target_id,
                            workload_id,
                            &reference,
                            authority,
                            subject,
                        )
                        .await;
                    if let Ok(receipt) = cleanup {
                        receipts.push(receipt);
                        return Err(partial_execution_error(
                            "recovery_required",
                            Vec::new(),
                            receipts,
                        ));
                    }
                    return Err(partial_execution_error(
                        "outcome_unknown",
                        vec![provisional_resource(
                            workload_id,
                            target_id,
                            &reference,
                            &image,
                        )],
                        receipts,
                    ));
                }
            };
            resources.push(active_resource(
                workload_id,
                target_id,
                &operation.operation_id,
                &reference,
                &image,
                lease.port,
            ));
        }
        Ok(RealizationExecution {
            resources,
            receipts,
        })
    }

    async fn stop(
        &self,
        realization: &RealizationRevision,
        target_id: &str,
        authority: &RealizationMutationAuthority,
        subject: &RealizationAuthoritySubject,
    ) -> anyhow::Result<Vec<RealizationEffectReceipt>> {
        let state = self.state()?;
        let mut receipts = Vec::new();
        for resource in &realization.actual_resources {
            let reference = resource_ref(resource)?;
            let operation = self
                .run_operation(
                    &state,
                    target_id,
                    &realization.installation_id,
                    TargetOperationSpec::DeploymentStop {
                        deployment: reference.clone(),
                        grace_seconds: 10,
                        force_remove: true,
                    },
                    format!(
                        "realization:{}:stop:{}",
                        realization.realization_id, resource.resource_id
                    ),
                    authority,
                    subject,
                )
                .await?;
            self.cleanup_route(&state, &reference).await;
            receipts.push(effect_receipt(
                realization,
                target_id,
                RealizationEffectKind::Stop,
                &operation,
            )?);
        }
        Ok(receipts)
    }

    async fn observe(
        &self,
        realization: &RealizationRevision,
        target_id: &str,
    ) -> anyhow::Result<RealizationObservation> {
        let state = self.state()?;
        let mut all_active = true;
        let mut any_unknown = false;
        let mut receipts = Vec::new();
        for resource in &realization.actual_resources {
            let reference = resource_ref(resource)?;
            // Observe is deliberately effect-free. Target Agent operation authority
            // is Host-owned and does not grant mutation or retry a launch.
            let operation = submit_host_operation(
                &state,
                target_id,
                CreateTargetOperationRequest {
                    installation_id: realization.installation_id.clone(),
                    spec: TargetOperationSpec::DeploymentObserve {
                        deployment: reference,
                    },
                    idempotency_key: Some(format!(
                        "realization:{}:observe:{}:{}",
                        realization.realization_id, resource.resource_id, realization.revision
                    )),
                    expires_in_seconds: Some(120),
                },
            )
            .await
            .map_err(|_| anyhow!("target_unsatisfied: Target observation was rejected"))?;
            let operation = if operation.status.is_terminal() {
                operation
            } else {
                wait_for_host_operation(
                    &state,
                    target_id,
                    &operation.operation_id,
                    Duration::from_secs(120),
                )
                .await?
            };
            match operation.status {
                TargetOperationStatusKind::Succeeded => {}
                TargetOperationStatusKind::OutcomeUnknown => any_unknown = true,
                _ => all_active = false,
            }
            receipts.push(effect_receipt(
                realization,
                target_id,
                RealizationEffectKind::Reconcile,
                &operation,
            )?);
        }
        let (status, reason) = if any_unknown {
            (
                RealizationStatus::OutcomeUnknown,
                Some("outcome_unknown".to_string()),
            )
        } else if all_active && !realization.actual_resources.is_empty() {
            (RealizationStatus::Active, None)
        } else if realization.status == RealizationStatus::Stopping {
            (
                RealizationStatus::RecoveryRequired,
                Some("recovery_required".to_string()),
            )
        } else {
            (
                RealizationStatus::Failed,
                Some("target_unsatisfied".to_string()),
            )
        };
        Ok(RealizationObservation {
            status,
            resources: realization.actual_resources.clone(),
            receipts,
            reason_code: reason,
        })
    }
}

fn provisional_resource(
    workload_id: &str,
    target_id: &str,
    reference: &TargetDeploymentRef,
    image: &str,
) -> RealizedResource {
    RealizedResource {
        resource_id: workload_id.to_string(),
        resource_type: "managed_workload".to_string(),
        target_id: target_id.to_string(),
        backend_id: reference.deployment_id.clone(),
        properties: BTreeMap::from([
            ("deployment_id".to_string(), json!(reference.deployment_id)),
            ("route_id".to_string(), json!(reference.route_id)),
            ("port_lease_id".to_string(), json!(reference.port_lease_id)),
            ("image".to_string(), json!(image)),
        ]),
        receipt_ref: None,
    }
}

fn active_resource(
    workload_id: &str,
    target_id: &str,
    operation_id: &str,
    reference: &TargetDeploymentRef,
    image: &str,
    host_port: u16,
) -> RealizedResource {
    let mut resource = provisional_resource(workload_id, target_id, reference, image);
    resource.backend_id = operation_id.to_string();
    resource
        .properties
        .insert("host_port".to_string(), json!(host_port));
    resource
}

fn ensure_operation_succeeded(operation: &TargetOperationRecord) -> anyhow::Result<()> {
    match operation.status {
        TargetOperationStatusKind::Succeeded => Ok(()),
        TargetOperationStatusKind::OutcomeUnknown => Err(
            plurora_runtime::managed_target_deployment_outcome_unknown("Target operation"),
        ),
        _ => Err(anyhow!("target_unsatisfied: Target operation failed")),
    }
}

fn effect_receipt(
    realization: &RealizationRevision,
    target_id: &str,
    effect: RealizationEffectKind,
    operation: &TargetOperationRecord,
) -> anyhow::Result<RealizationEffectReceipt> {
    let receipt = operation
        .receipt
        .as_ref()
        .ok_or_else(|| anyhow!("recovery_required: Target operation has no receipt"))?;
    let status = match receipt.status {
        TargetOperationReceiptStatus::Succeeded => "succeeded",
        TargetOperationReceiptStatus::Failed => "failed",
        TargetOperationReceiptStatus::Cancelled => "cancelled",
        TargetOperationReceiptStatus::OutcomeUnknown => "outcome_unknown",
    };
    Ok(RealizationEffectReceipt {
        realization_id: realization.realization_id.clone(),
        action_id: operation.operation_id.clone(),
        target_id: target_id.to_string(),
        effect,
        request_digest: operation.authority.request_digest.clone(),
        status: status.to_string(),
        started_at: Utc
            .timestamp_millis_opt(operation.created_at_ms)
            .single()
            .ok_or_else(|| anyhow!("Target operation start timestamp is invalid"))?,
        finished_at: Utc
            .timestamp_millis_opt(receipt.completed_at_ms)
            .single()
            .ok_or_else(|| anyhow!("Target operation receipt timestamp is invalid"))?,
        observed_resources: Vec::new(),
        diagnostic_ref: None,
    })
}

fn resource_ref(resource: &RealizedResource) -> anyhow::Result<TargetDeploymentRef> {
    let string = |key: &str| {
        resource
            .properties
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| anyhow!("recovery_required: realized resource is missing {key}"))
    };
    Ok(TargetDeploymentRef {
        deployment_id: string("deployment_id")?,
        route_id: string("route_id")?,
        port_lease_id: string("port_lease_id")?,
    })
}

fn realization_build_id(realization_id: &plurora_work::RealizationId, workload_id: &str) -> String {
    let digest = Sha256::digest(format!("{realization_id}:{workload_id}").as_bytes());
    format!("realization-{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_resource_projection_is_public_record_safe() -> anyhow::Result<()> {
        let resource = active_resource(
            "web",
            "local",
            "operation-1",
            &TargetDeploymentRef {
                deployment_id: "deployment-1".to_string(),
                route_id: "acceptance-route".to_string(),
                port_lease_id: "lease-1".to_string(),
            },
            &format!("registry.example/nginx@sha256:{}", "a".repeat(64)),
            32_768,
        );

        plurora_work::validate_portable_model(&resource)?;
        assert_eq!(resource.properties["route_id"], json!("acceptance-route"));
        assert_eq!(resource.properties["host_port"], json!(32_768));
        assert!(!resource.properties.contains_key("public_url"));
        Ok(())
    }
}
