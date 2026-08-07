use super::*;
use crate::runtime::effects::{principal_identity, EffectReceiptRequest};
use crate::runtime::local_exec::ExecEffectContext;
use crate::DEFAULT_CONTRACT_PROFILE;
use chrono::Utc;
use plurora_core::{ArtifactDescriptor, EffectScope, EffectTerminalStatus};

const DEPLOYMENT_HUB_SESSION_ID: &str = "host_deployment_hub";

impl<S> Runtime<S>
where
    S: EventStore,
{
    fn ensure_target_authority(context: &ProtocolContext, target_id: &str) -> anyhow::Result<()> {
        if context.allows_host_resource("host", "target", target_id) {
            Ok(())
        } else {
            anyhow::bail!(
                "permission denied: authenticated authority does not include target '{}'",
                target_id
            )
        }
    }

    // --- Target ---

    pub(crate) async fn dispatch_target_list(
        &self,
        context: &ProtocolContext,
    ) -> anyhow::Result<Value> {
        let targets = self
            .config
            .target_registry
            .list()
            .await
            .into_iter()
            .filter(|target| context.allows_host_resource("host", "target", target.id.as_str()))
            .collect::<Vec<_>>();
        Ok(serde_json::to_value(targets)?)
    }

    pub(crate) async fn dispatch_target_status(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let target_id = required_str(params, "target_id", "host.target.status")?;
        Self::ensure_target_authority(context, &target_id)?;
        Ok(serde_json::to_value(
            self.config
                .target_registry
                .status(&target_id)
                .await
                .ok_or_else(|| anyhow::anyhow!("execution target '{target_id}' not found"))?,
        )?)
    }

    pub(crate) async fn dispatch_target_register(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let target: crate::runtime::ExecutionTarget = serde_json::from_value(params)?;
        Self::ensure_target_authority(context, target.id.as_str())?;
        anyhow::bail!("direct target mutation is disabled; use the Host target enrollment API")
    }

    pub(crate) async fn dispatch_target_unregister(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let target_id = required_str(params, "target_id", "host.target.unregister")?;
        Self::ensure_target_authority(context, &target_id)?;
        anyhow::bail!("direct target mutation is disabled; use the Host target revoke API")
    }

    // --- Exec ---

    pub(crate) async fn dispatch_exec_start(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: crate::runtime::LocalExecStartRequest = serde_json::from_value(params)?;
        Self::ensure_target_authority(context, &request.target_id)?;
        let effect_context = exec_effect_context(context, &request);
        self.append_deployment_hub_event(
            context,
            plurora_core::EVENT_EXEC_REQUEST,
            json!({
                "target_id": request.target_id,
                "program": request.command.program,
                "arg_count": request.command.args.len(),
                "port_names": request.port_names,
            }),
        )
        .await?;

        let executor = self.config.local_exec_executor.executor();
        let response = match executor.start(request).await {
            Ok(response) => response,
            Err(error) => {
                let status = crate::runtime::ExecStatus {
                    exec_id: None,
                    target_id: Some(effect_context.target_id.clone()),
                    kind: crate::runtime::ExecStatusKind::Failed,
                    exit_code: None,
                    message: None,
                    ready: false,
                };
                let receipt = self
                    .record_exec_effect(
                        &effect_context,
                        "exec.start",
                        &status,
                        EffectTerminalStatus::Failed,
                    )
                    .await?;
                self.append_deployment_hub_event(
                    context,
                    plurora_core::EVENT_EXEC_FAILED,
                    json!({
                        "exec_id": Value::Null,
                        "target_id": effect_context.target_id,
                        "status": status.kind,
                        "effect_kind": "exec.start",
                        "error_present": true,
                        "receipt": receipt,
                    }),
                )
                .await?;
                return Err(error);
            }
        };
        self.config
            .exec_registry
            .upsert_from_status(&response.status)
            .await;

        let terminal_status = exec_terminal_status(&response.status);
        if let Some(terminal_status) = terminal_status {
            if let Some(exec_id) = response.exec_id.as_ref() {
                self.finalize_exec_terminal_once(
                    context,
                    &effect_context,
                    exec_id,
                    "exec.start",
                    &response.status,
                    terminal_status,
                )
                .await?;
            } else {
                let receipt = self
                    .record_exec_effect(
                        &effect_context,
                        "exec.start",
                        &response.status,
                        terminal_status,
                    )
                    .await?;
                self.append_deployment_hub_event(
                    context,
                    exec_terminal_event_kind(terminal_status),
                    json!({
                        "exec_id": Value::Null,
                        "target_id": response.status.target_id,
                        "status": response.status.kind,
                        "ready": response.status.ready,
                        "effect_kind": "exec.start",
                        "error_present": response.error.is_some(),
                        "receipt": receipt,
                    }),
                )
                .await?;
            }
        } else if let Some(exec_id) = response.exec_id.as_ref() {
            self.config
                .exec_registry
                .record_effect_context(exec_id.clone(), effect_context.clone())
                .await;
            self.append_deployment_hub_event(
                context,
                plurora_core::EVENT_EXEC_STARTED,
                json!({
                    "exec_id": response.exec_id,
                    "target_id": response.status.target_id,
                    "status": response.status.kind,
                    "ready": response.status.ready,
                    "effect_kind": "exec.run",
                    "error_present": response.error.is_some(),
                    "effect_context": effect_context.clone(),
                }),
            )
            .await?;
            if executor.supports_terminal_monitoring() {
                self.spawn_exec_terminal_monitor(
                    context.clone(),
                    effect_context,
                    exec_id.clone(),
                    executor,
                );
            }
        }

        Ok(serde_json::to_value(response)?)
    }

    pub(crate) async fn dispatch_exec_stop(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: crate::runtime::LocalExecStopRequest = serde_json::from_value(params)?;
        if request.exec_id.trim().is_empty() {
            anyhow::bail!("host.exec.stop requires exec_id");
        }
        let exec_id = request.exec_id.clone();
        if let Some(status) = self.config.exec_registry.status(&exec_id).await {
            if let Some(target_id) = status.target_id.as_deref() {
                Self::ensure_target_authority(context, target_id)?;
            }
            if is_process_terminal(&status)
                && self
                    .config
                    .exec_registry
                    .terminal_receipt(&exec_id)
                    .await
                    .is_some()
            {
                return Ok(serde_json::to_value(
                    crate::runtime::LocalExecStopResponse {
                        exec_id,
                        status,
                        error: None,
                    },
                )?);
            }
        }
        let effect_context = self
            .config
            .exec_registry
            .effect_context(&exec_id)
            .await
            .unwrap_or_else(|| unknown_exec_effect_context(context, &exec_id));
        Self::ensure_target_authority(context, &effect_context.target_id)?;
        let response = match self
            .config
            .local_exec_executor
            .executor()
            .stop(request)
            .await
        {
            Ok(response) => response,
            Err(error) => {
                let status = crate::runtime::ExecStatus {
                    exec_id: Some(exec_id.clone()),
                    target_id: Some(effect_context.target_id.clone()),
                    kind: crate::runtime::ExecStatusKind::Failed,
                    exit_code: None,
                    message: None,
                    ready: false,
                };
                let receipt = self
                    .record_exec_effect(
                        &effect_context,
                        "exec.stop",
                        &status,
                        EffectTerminalStatus::Failed,
                    )
                    .await?;
                self.append_deployment_hub_event(
                    context,
                    plurora_core::EVENT_EXEC_FAILED,
                    json!({
                        "exec_id": exec_id,
                        "status": status.kind,
                        "effect_kind": "exec.stop",
                        "error_present": true,
                        "receipt": receipt,
                    }),
                )
                .await?;
                return Err(error);
            }
        };
        let terminal_status =
            exec_terminal_status(&response.status).unwrap_or(EffectTerminalStatus::Failed);
        if terminal_status == EffectTerminalStatus::Denied {
            let operation_key = format!("exec.stop:{exec_id}");
            if self
                .config
                .exec_registry
                .operation_receipt(&operation_key)
                .await
                .is_none()
            {
                let receipt = self
                    .record_exec_effect(
                        &effect_context,
                        "exec.stop",
                        &response.status,
                        terminal_status,
                    )
                    .await?;
                self.config
                    .exec_registry
                    .record_operation_receipt(operation_key, receipt.clone())
                    .await;
                self.append_deployment_hub_event(
                    context,
                    plurora_core::EVENT_EXEC_DENIED,
                    json!({
                        "exec_id": response.exec_id,
                        "status": response.status.kind,
                        "effect_kind": "exec.stop",
                        "error_present": response.error.is_some(),
                        "receipt": receipt,
                    }),
                )
                .await?;
            }
        } else {
            self.config
                .exec_registry
                .upsert_from_status(&response.status)
                .await;
            self.finalize_exec_terminal_once(
                context,
                &effect_context,
                &exec_id,
                "exec.run",
                &response.status,
                terminal_status,
            )
            .await?;
        }

        Ok(serde_json::to_value(response)?)
    }

    pub(crate) async fn dispatch_exec_status(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: crate::runtime::LocalExecStatusRequest = serde_json::from_value(params)?;
        if request.exec_id.trim().is_empty() {
            anyhow::bail!("host.exec.status requires exec_id");
        }
        let exec_id = request.exec_id.clone();
        if let Some(status) = self.config.exec_registry.status(&exec_id).await {
            if let Some(target_id) = status.target_id.as_deref() {
                Self::ensure_target_authority(context, target_id)?;
            }
            if is_process_terminal(&status)
                && self
                    .config
                    .exec_registry
                    .terminal_receipt(&exec_id)
                    .await
                    .is_some()
            {
                return Ok(serde_json::to_value(
                    crate::runtime::LocalExecStatusResponse {
                        status,
                        error: None,
                    },
                )?);
            }
        }
        let effect_context = self
            .config
            .exec_registry
            .effect_context(&exec_id)
            .await
            .unwrap_or_else(|| unknown_exec_effect_context(context, &exec_id));
        Self::ensure_target_authority(context, &effect_context.target_id)?;
        let response = match self
            .config
            .local_exec_executor
            .executor()
            .status(request)
            .await
        {
            Ok(response) => response,
            Err(error) => {
                let status = crate::runtime::ExecStatus {
                    exec_id: Some(exec_id.clone()),
                    target_id: Some(effect_context.target_id.clone()),
                    kind: crate::runtime::ExecStatusKind::Failed,
                    exit_code: None,
                    message: None,
                    ready: false,
                };
                let receipt = self
                    .record_exec_effect(
                        &effect_context,
                        "exec.status",
                        &status,
                        EffectTerminalStatus::Failed,
                    )
                    .await?;
                self.append_deployment_hub_event(
                    context,
                    plurora_core::EVENT_EXEC_FAILED,
                    json!({
                        "exec_id": exec_id,
                        "status": status.kind,
                        "effect_kind": "exec.status",
                        "error_present": true,
                        "receipt": receipt,
                    }),
                )
                .await?;
                return Err(error);
            }
        };
        if let Some(terminal_status) = exec_terminal_status(&response.status) {
            if terminal_status == EffectTerminalStatus::Denied {
                let operation_key = format!("exec.status:{exec_id}");
                if self
                    .config
                    .exec_registry
                    .operation_receipt(&operation_key)
                    .await
                    .is_none()
                {
                    let receipt = self
                        .record_exec_effect(
                            &effect_context,
                            "exec.status",
                            &response.status,
                            terminal_status,
                        )
                        .await?;
                    self.config
                        .exec_registry
                        .record_operation_receipt(operation_key, receipt.clone())
                        .await;
                    self.append_deployment_hub_event(
                        context,
                        plurora_core::EVENT_EXEC_DENIED,
                        json!({
                            "exec_id": exec_id,
                            "target_id": response.status.target_id,
                            "status": response.status.kind,
                            "effect_kind": "exec.status",
                            "error_present": response.error.is_some(),
                            "receipt": receipt,
                        }),
                    )
                    .await?;
                }
            } else {
                self.config
                    .exec_registry
                    .upsert_from_status(&response.status)
                    .await;
                self.finalize_exec_terminal_once(
                    context,
                    &effect_context,
                    &exec_id,
                    "exec.run",
                    &response.status,
                    terminal_status,
                )
                .await?;
            }
        } else {
            self.config
                .exec_registry
                .upsert_from_status(&response.status)
                .await;
        }
        Ok(serde_json::to_value(response)?)
    }

    pub(crate) async fn dispatch_exec_logs(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: crate::runtime::LocalExecLogsRequest = serde_json::from_value(params)?;
        if request.exec_id.trim().is_empty() {
            anyhow::bail!("host.exec.logs requires exec_id");
        }
        let target_id = if let Some(target_id) = self
            .config
            .exec_registry
            .status(&request.exec_id)
            .await
            .and_then(|status| status.target_id)
        {
            target_id
        } else {
            self.config
                .exec_registry
                .effect_context(&request.exec_id)
                .await
                .map(|effect| effect.target_id)
                .ok_or_else(|| {
                    anyhow::anyhow!("execution '{}' has no target context", request.exec_id)
                })?
        };
        Self::ensure_target_authority(context, &target_id)?;
        Ok(serde_json::to_value(
            self.config
                .local_exec_executor
                .executor()
                .logs(request)
                .await?,
        )?)
    }

    pub(crate) async fn dispatch_exec_list(
        &self,
        context: &ProtocolContext,
    ) -> anyhow::Result<Value> {
        let executions = self
            .config
            .exec_registry
            .list()
            .await
            .into_iter()
            .filter(|status| {
                status.target_id.as_deref().is_some_and(|target_id| {
                    context.allows_host_resource("host", "target", target_id)
                })
            })
            .collect();
        Ok(serde_json::to_value(
            crate::runtime::LocalExecListResponse { executions },
        )?)
    }

    async fn record_exec_effect(
        &self,
        context: &ExecEffectContext,
        effect_kind: &str,
        status: &crate::runtime::ExecStatus,
        terminal_status: EffectTerminalStatus,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let latency_ms = (Utc::now() - context.started_at).num_milliseconds().max(1) as u64;
        let mut request = EffectReceiptRequest::live(
            effect_kind,
            context.principal.clone(),
            json!({
                "kind": "local_exec_executor",
                "target_id": context.target_id,
            }),
            terminal_status,
            context.started_at,
            latency_ms,
            context.trace_id.clone(),
        );
        request.protocol_profiles = vec![DEFAULT_CONTRACT_PROFILE.to_string()];
        request.inputs = vec![json!({
            "target_id": context.target_id,
            "program": context.program,
            "arg_count": context.arg_count,
            "port_names": context.port_names,
            "lifecycle": context.lifecycle,
            "resource_limits": context.resource_limits,
        })];
        request.outputs = vec![json!({
            "exec_id": status.exec_id,
            "target_id": status.target_id,
            "status": status.kind,
            "exit_code": status.exit_code,
            "ready": status.ready,
        })];
        request.external_effects = status
            .exec_id
            .as_ref()
            .filter(|_| terminal_status != EffectTerminalStatus::Denied)
            .map(|exec_id| {
                json!({
                    "kind": "host_process",
                    "exec_id": exec_id,
                    "target_id": status.target_id,
                    "exit_code": status.exit_code,
                })
            })
            .into_iter()
            .collect();
        request.authority = Some(json!({
            "principal": context.principal,
            "target_id": context.target_id,
        }));
        request.policy_decision = Some(json!({
            "outcome": if terminal_status == EffectTerminalStatus::Denied {
                "denied"
            } else {
                "allowed"
            },
        }));
        request.scope = EffectScope {
            session_id: context.session_id.clone(),
            branch_id: None,
        };
        request.planned = json!({
            "target_id": context.target_id,
            "program": context.program,
            "arg_count": context.arg_count,
            "lifecycle": context.lifecycle,
            "resource_limits": context.resource_limits,
        });
        request.actual = json!({
            "exec_id": status.exec_id,
            "target_id": status.target_id,
            "status": terminal_status,
            "exec_status": status.kind,
            "exit_code": status.exit_code,
            "ready": status.ready,
        });
        self.record_effect_receipt(request).await
    }

    async fn finalize_exec_terminal_once(
        &self,
        protocol_context: &ProtocolContext,
        effect_context: &ExecEffectContext,
        exec_id: &str,
        effect_kind: &str,
        status: &crate::runtime::ExecStatus,
        terminal_status: EffectTerminalStatus,
    ) -> anyhow::Result<Option<ArtifactDescriptor>> {
        if !self
            .config
            .exec_registry
            .claim_terminal_receipt(exec_id)
            .await
        {
            return Ok(self.config.exec_registry.terminal_receipt(exec_id).await);
        }

        let result = async {
            let receipt = self
                .record_exec_effect(effect_context, effect_kind, status, terminal_status)
                .await?;
            self.append_deployment_hub_event(
                protocol_context,
                exec_terminal_event_kind(terminal_status),
                json!({
                    "exec_id": exec_id,
                    "target_id": status.target_id,
                    "status": status.kind,
                    "exit_code": status.exit_code,
                    "ready": status.ready,
                    "effect_kind": effect_kind,
                    "error_present": status.message.is_some(),
                    "receipt": receipt,
                }),
            )
            .await?;
            self.config.exec_registry.upsert_from_status(status).await;
            self.config
                .exec_registry
                .record_terminal_receipt(exec_id.to_string(), receipt.clone())
                .await;
            Ok::<ArtifactDescriptor, anyhow::Error>(receipt)
        }
        .await;

        match result {
            Ok(receipt) => Ok(Some(receipt)),
            Err(error) => {
                self.config
                    .exec_registry
                    .release_terminal_receipt_claim(exec_id)
                    .await;
                Err(error)
            }
        }
    }

    fn spawn_exec_terminal_monitor(
        &self,
        protocol_context: ProtocolContext,
        effect_context: ExecEffectContext,
        exec_id: String,
        executor: std::sync::Arc<dyn crate::runtime::LocalExecExecutor>,
    ) {
        let runtime = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                if runtime
                    .config
                    .exec_registry
                    .terminal_receipt(&exec_id)
                    .await
                    .is_some()
                {
                    break;
                }
                let response = match executor
                    .status(crate::runtime::LocalExecStatusRequest {
                        exec_id: exec_id.clone(),
                    })
                    .await
                {
                    Ok(response) => response,
                    Err(_) => continue,
                };
                if response.status.kind == crate::runtime::ExecStatusKind::Denied {
                    continue;
                }
                runtime
                    .config
                    .exec_registry
                    .upsert_from_status(&response.status)
                    .await;
                let Some(terminal_status) = exec_terminal_status(&response.status) else {
                    continue;
                };
                match runtime
                    .finalize_exec_terminal_once(
                        &protocol_context,
                        &effect_context,
                        &exec_id,
                        "exec.run",
                        &response.status,
                        terminal_status,
                    )
                    .await
                {
                    Ok(Some(_)) => break,
                    Ok(None) | Err(_) => continue,
                }
            }
        });
    }

    // --- Port ---

    pub(crate) async fn dispatch_port_lease(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: crate::runtime::PortLeaseRequest = serde_json::from_value(params)?;
        Self::ensure_target_authority(context, &request.target_id)?;
        let response = self.config.port_lease_registry.lease(request).await;
        self.append_deployment_hub_event(
            context,
            plurora_core::EVENT_PORT_LEASED,
            json!({
                "lease_id": response.lease.id,
                "target_id": response.lease.target_id,
                "port_name": response.lease.port_name,
                "host": response.lease.host,
                "port": response.lease.port,
                "protocol": response.lease.protocol,
                "bind": response.lease.bind,
                "status": response.lease.status,
            }),
        )
        .await?;
        Ok(serde_json::to_value(response)?)
    }

    pub(crate) async fn dispatch_port_release(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let lease_id = required_str(params, "lease_id", "host.port.release")?;
        let existing = self
            .config
            .port_lease_registry
            .status(&lease_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("port lease '{lease_id}' not found"))?;
        Self::ensure_target_authority(context, &existing.target_id)?;
        let lease = self
            .config
            .port_lease_registry
            .release(&lease_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("port lease '{lease_id}' not found"))?;
        self.append_deployment_hub_event(
            context,
            plurora_core::EVENT_PORT_RELEASED,
            json!({
                "lease_id": lease.id,
                "target_id": lease.target_id,
                "port_name": lease.port_name,
                "host": lease.host,
                "port": lease.port,
                "status": lease.status,
            }),
        )
        .await?;
        Ok(serde_json::to_value(lease)?)
    }

    pub(crate) async fn dispatch_port_status(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let lease_id = required_str(params, "lease_id", "host.port.status")?;
        let lease = self
            .config
            .port_lease_registry
            .status(&lease_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("port lease '{lease_id}' not found"))?;
        Self::ensure_target_authority(context, &lease.target_id)?;
        Ok(serde_json::to_value(lease)?)
    }

    pub(crate) async fn dispatch_port_list(
        &self,
        context: &ProtocolContext,
    ) -> anyhow::Result<Value> {
        let leases = self
            .config
            .port_lease_registry
            .list()
            .await
            .into_iter()
            .filter(|lease| context.allows_host_resource("host", "target", &lease.target_id))
            .collect::<Vec<_>>();
        Ok(serde_json::to_value(leases)?)
    }

    // --- Proxy ---

    pub(crate) async fn dispatch_proxy_register(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: crate::runtime::ProxyRouteRegisterRequest = serde_json::from_value(params)?;
        let lease_id = request.upstream.port_lease_id.clone();
        let lease = self.config.port_lease_registry.status(&lease_id).await;
        if !matches!(
            lease.as_ref().map(|lease| lease.status),
            Some(crate::runtime::PortLeaseStatusKind::Active)
        ) {
            self.append_deployment_hub_event(
                context,
                plurora_core::EVENT_PROXY_DENIED,
                json!({
                    "port_lease_id": lease_id,
                    "reason": "missing_or_inactive_port_lease",
                }),
            )
            .await?;
            anyhow::bail!("host.proxy.register requires an existing active port lease upstream");
        }
        let lease = lease.expect("active lease checked above");
        Self::ensure_target_authority(context, &lease.target_id)?;
        if request.upstream.port_name != lease.port_name {
            self.append_deployment_hub_event(
                context,
                plurora_core::EVENT_PROXY_DENIED,
                json!({
                    "port_lease_id": lease_id,
                    "requested_port_name": request.upstream.port_name,
                    "leased_port_name": lease.port_name,
                    "reason": "port_name_mismatch",
                }),
            )
            .await?;
            anyhow::bail!(
                "host.proxy.register upstream port_name must match the referenced port lease"
            );
        }

        let response = self.config.proxy_route_registry.register(request).await;
        self.append_deployment_hub_event(
            context,
            plurora_core::EVENT_PROXY_REGISTERED,
            json!({
                "route_id": response.route.id,
                "port_lease_id": response.route.upstream.port_lease_id,
                "port_name": response.route.upstream.port_name,
                "protocol": response.route.protocol,
                "access": response.route.access,
                "public_url": response.route.public_url,
                "iframe_url": response.route.iframe_url,
                "status": response.route.status,
                "ready": response.route.ready,
            }),
        )
        .await?;
        Ok(serde_json::to_value(response)?)
    }

    pub(crate) async fn dispatch_proxy_unregister(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let route_id = required_str(params, "route_id", "host.proxy.unregister")?;
        let existing = self
            .config
            .proxy_route_registry
            .status(&route_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("proxy route '{route_id}' not found"))?;
        let lease = self
            .config
            .port_lease_registry
            .status(&existing.upstream.port_lease_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("proxy route '{route_id}' has no active lease"))?;
        Self::ensure_target_authority(context, &lease.target_id)?;
        let route = self
            .config
            .proxy_route_registry
            .unregister(&route_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("proxy route '{route_id}' not found"))?;
        self.append_deployment_hub_event(
            context,
            plurora_core::EVENT_PROXY_UNREGISTERED,
            json!({
                "route_id": route.id,
                "port_lease_id": route.upstream.port_lease_id,
                "status": route.status,
            }),
        )
        .await?;
        Ok(serde_json::to_value(route)?)
    }

    pub(crate) async fn dispatch_proxy_status(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let route_id = required_str(params, "route_id", "host.proxy.status")?;
        let route = self
            .config
            .proxy_route_registry
            .status(&route_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("proxy route '{route_id}' not found"))?;
        let lease = self
            .config
            .port_lease_registry
            .status(&route.upstream.port_lease_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("proxy route '{route_id}' has no active lease"))?;
        Self::ensure_target_authority(context, &lease.target_id)?;
        Ok(serde_json::to_value(route)?)
    }

    pub(crate) async fn dispatch_proxy_list(
        &self,
        context: &ProtocolContext,
    ) -> anyhow::Result<Value> {
        let mut visible = Vec::new();
        for route in self.config.proxy_route_registry.list().await {
            let Some(lease) = self
                .config
                .port_lease_registry
                .status(&route.upstream.port_lease_id)
                .await
            else {
                continue;
            };
            if context.allows_host_resource("host", "target", &lease.target_id) {
                visible.push(route);
            }
        }
        Ok(serde_json::to_value(visible)?)
    }

    async fn append_deployment_hub_event(
        &self,
        context: &ProtocolContext,
        kind: &'static str,
        payload: Value,
    ) -> anyhow::Result<()> {
        let session_id = if let Some(session_id) = context.session_id.as_deref() {
            session_id.to_string()
        } else {
            self.ensure_deployment_hub_session().await?
        };
        self.append_platform_event(&session_id, kind, payload)
            .await?;
        Ok(())
    }

    async fn ensure_deployment_hub_session(&self) -> anyhow::Result<String> {
        {
            let sessions = self.sessions.read().await;
            if matches!(
                sessions
                    .get(DEPLOYMENT_HUB_SESSION_ID)
                    .map(|session| &session.status),
                Some(plurora_core::SessionStatus::Open)
            ) {
                return Ok(DEPLOYMENT_HUB_SESSION_ID.to_string());
            }
        }

        let now = chrono::Utc::now();
        let mut sessions = self.sessions.write().await;
        sessions.insert(
            DEPLOYMENT_HUB_SESSION_ID.to_string(),
            plurora_core::SessionRecord {
                id: DEPLOYMENT_HUB_SESSION_ID.to_string(),
                labels: vec!["platform_runtime".to_string(), "deployment_hub".to_string()],
                active_package_set: Vec::new(),
                principal_scope: None,
                status: plurora_core::SessionStatus::Open,
                created_at: now,
                updated_at: now,
                metadata: json!({"synthetic": true}),
            },
        );
        Ok(DEPLOYMENT_HUB_SESSION_ID.to_string())
    }
}

fn exec_effect_context(
    context: &ProtocolContext,
    request: &crate::runtime::LocalExecStartRequest,
) -> ExecEffectContext {
    ExecEffectContext {
        principal: principal_identity(&context.principal),
        target_id: request.target_id.clone(),
        program: request.command.program.clone(),
        arg_count: request.command.args.len(),
        port_names: request.port_names.clone(),
        lifecycle: request.lifecycle,
        resource_limits: request.resource_limits.clone(),
        session_id: context.session_id.clone(),
        started_at: Utc::now(),
        trace_id: context.effective_correlation_id().to_string(),
    }
}

fn unknown_exec_effect_context(context: &ProtocolContext, exec_id: &str) -> ExecEffectContext {
    ExecEffectContext {
        principal: principal_identity(&context.principal),
        target_id: "unknown".to_string(),
        program: "unknown".to_string(),
        arg_count: 0,
        port_names: Vec::new(),
        lifecycle: crate::runtime::ExecLifecyclePolicy::StopOnSessionClose,
        resource_limits: crate::runtime::ExecResourceLimits::default(),
        session_id: context.session_id.clone(),
        started_at: Utc::now(),
        trace_id: format!("{}:{exec_id}", context.effective_correlation_id()),
    }
}

fn exec_terminal_status(status: &crate::runtime::ExecStatus) -> Option<EffectTerminalStatus> {
    match status.kind {
        crate::runtime::ExecStatusKind::Pending | crate::runtime::ExecStatusKind::Running => None,
        crate::runtime::ExecStatusKind::Stopped => Some(EffectTerminalStatus::Cancelled),
        crate::runtime::ExecStatusKind::Exited if status.exit_code.unwrap_or(0) == 0 => {
            Some(EffectTerminalStatus::Succeeded)
        }
        crate::runtime::ExecStatusKind::Exited => Some(EffectTerminalStatus::Failed),
        crate::runtime::ExecStatusKind::Failed => {
            if status
                .message
                .as_deref()
                .is_some_and(|message| message.to_ascii_lowercase().contains("timeout"))
            {
                Some(EffectTerminalStatus::TimedOut)
            } else {
                Some(EffectTerminalStatus::Failed)
            }
        }
        crate::runtime::ExecStatusKind::Denied => Some(EffectTerminalStatus::Denied),
        crate::runtime::ExecStatusKind::Unknown => Some(EffectTerminalStatus::Failed),
    }
}

fn exec_terminal_event_kind(status: EffectTerminalStatus) -> &'static str {
    match status {
        EffectTerminalStatus::Succeeded => plurora_core::EVENT_EXEC_COMPLETED,
        EffectTerminalStatus::Denied => plurora_core::EVENT_EXEC_DENIED,
        EffectTerminalStatus::Cancelled => plurora_core::EVENT_EXEC_STOPPED,
        EffectTerminalStatus::Failed
        | EffectTerminalStatus::TimedOut
        | EffectTerminalStatus::Partial => plurora_core::EVENT_EXEC_FAILED,
    }
}

fn is_process_terminal(status: &crate::runtime::ExecStatus) -> bool {
    matches!(
        status.kind,
        crate::runtime::ExecStatusKind::Stopped
            | crate::runtime::ExecStatusKind::Exited
            | crate::runtime::ExecStatusKind::Failed
    )
}

fn required_str(params: &Value, field: &str, method: &str) -> anyhow::Result<String> {
    let value = params
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("{method} requires {field}"))?;
    if value.trim().is_empty() {
        anyhow::bail!("{method} requires {field}");
    }
    Ok(value.to_string())
}
