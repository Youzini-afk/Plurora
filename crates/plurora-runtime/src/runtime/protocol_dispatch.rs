use serde_json::{json, Value};

use super::Runtime;
use crate::{
    negotiate_contract, resolve_contract_method, ContractSelection, EventStore, KernelMethod,
    ProtocolContext, ProtocolPrincipal,
};

const HOST_AUTHORITY_AUDIT_SESSION: &str = "host_control_authority";
const HOST_AUTHORITY_AUDIT_EVENT: &str = "host/control/v1/authority.decision";
const HOST_AUTHORITY_AUDIT_WRITER: &str = "host/control-plane";

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub async fn call_protocol(
        &self,
        context: &ProtocolContext,
        method: &str,
        params: Value,
    ) -> Result<Value, crate::ProtocolError> {
        self.call_protocol_negotiated(context, method, params, None)
            .await
    }

    pub async fn call_protocol_negotiated(
        &self,
        context: &ProtocolContext,
        method: &str,
        params: Value,
        selection: Option<&ContractSelection>,
    ) -> Result<Value, crate::ProtocolError> {
        negotiate_contract(selection)?;
        let resolved = resolve_contract_method(method).map_err(|_| {
            crate::ProtocolError::invalid_request(format!(
                "protocol method '{}' is not a known contract method",
                method
            ))
        })?;
        let params = resolved.adapt_request(params)?;
        let result = self
            .dispatch_protocol_method(context, resolved.method, params)
            .await
            .map_err(crate::ProtocolError::from_anyhow)
            .and_then(|value| resolved.adapt_response(value));
        self.audit_host_authority_decision(
            context,
            resolved.method,
            resolved.contract.canonical_id.as_str(),
            resolved.requested_id(),
            &result,
        )
        .await;
        result
    }

    pub async fn call_subprocess_protocol(
        &self,
        context: &ProtocolContext,
        method: &str,
        params: Value,
    ) -> Result<Value, crate::ProtocolError> {
        self.call_subprocess_protocol_negotiated(context, method, params, None)
            .await
    }

    pub async fn call_subprocess_protocol_negotiated(
        &self,
        context: &ProtocolContext,
        method: &str,
        params: Value,
        selection: Option<&ContractSelection>,
    ) -> Result<Value, crate::ProtocolError> {
        negotiate_contract(selection)?;
        let resolved = resolve_contract_method(method).map_err(|_| {
            crate::ProtocolError::invalid_request(format!(
                "protocol method '{}' is not a known contract method",
                method
            ))
        })?;
        let kernel_method = resolved.method;
        let params = resolved.adapt_request(params)?;
        let gate = ensure_global_host_catalog_access(context, kernel_method).and_then(|()| {
            if is_deployment_hub_method(kernel_method) {
                ensure_deployment_hub_control_allowed(context, kernel_method)
            } else {
                Ok(())
            }
        });
        let result: anyhow::Result<Value> = if let Err(error) = gate {
            Err(error)
        } else {
            match kernel_method {
                KernelMethod::OutboundExecute => {
                    self.dispatch_outbound_execute(context, params).await
                }
                KernelMethod::OutboundStream => {
                    self.dispatch_outbound_stream(context, params).await
                }
                KernelMethod::OutboundWebSocketOpen => {
                    self.dispatch_outbound_websocket_open(context, params).await
                }
                KernelMethod::OutboundWebSocketSend => {
                    self.dispatch_outbound_websocket_send(context, &params)
                        .await
                }
                KernelMethod::OutboundWebSocketClose => {
                    self.dispatch_outbound_websocket_close(context, &params)
                        .await
                }
                KernelMethod::TargetList => self.dispatch_target_list(context).await,
                KernelMethod::TargetStatus => self.dispatch_target_status(context, &params).await,
                KernelMethod::TargetRegister => {
                    self.dispatch_target_register(context, params).await
                }
                KernelMethod::TargetUnregister => {
                    self.dispatch_target_unregister(context, &params).await
                }
                KernelMethod::ExecStart => self.dispatch_exec_start(context, params).await,
                KernelMethod::ExecStop => self.dispatch_exec_stop(context, params).await,
                KernelMethod::ExecStatus => self.dispatch_exec_status(context, params).await,
                KernelMethod::ExecLogs => self.dispatch_exec_logs(context, params).await,
                KernelMethod::ExecList => self.dispatch_exec_list(context).await,
                KernelMethod::PortLease => self.dispatch_port_lease(context, params).await,
                KernelMethod::PortRelease => self.dispatch_port_release(context, &params).await,
                KernelMethod::PortStatus => self.dispatch_port_status(context, &params).await,
                KernelMethod::PortList => self.dispatch_port_list(context).await,
                KernelMethod::ProxyRegister => self.dispatch_proxy_register(context, params).await,
                KernelMethod::ProxyUnregister => {
                    self.dispatch_proxy_unregister(context, &params).await
                }
                KernelMethod::ProxyStatus => self.dispatch_proxy_status(context, &params).await,
                KernelMethod::ProxyList => self.dispatch_proxy_list(context).await,
                KernelMethod::CapabilityCancel => self.dispatch_capability_cancel(&params).await,
                KernelMethod::HostInfo => {
                    serde_json::to_value(crate::host_info()).map_err(anyhow::Error::from)
                }
                KernelMethod::HostPing => Ok(json!({"ok": true})),
                KernelMethod::HostDiagnostics => Ok(self.host_diagnostics().await),
                KernelMethod::CapabilityDiscover => {
                    serde_json::to_value(self.discover_capabilities().await)
                        .map_err(anyhow::Error::from)
                }
                KernelMethod::CapabilityInvoke => match serde_json::from_value(params) {
                    Ok(request) => self
                        .invoke_capability_with_context(context, request)
                        .await
                        .and_then(|result| {
                            serde_json::to_value(result).map_err(anyhow::Error::from)
                        }),
                    Err(error) => Err(anyhow::Error::from(error)),
                },
                other => Err(anyhow::anyhow!(
                    "protocol method '{}' is not available over subprocess reverse stdio yet",
                    other
                )),
            }
        };
        let result = result
            .map_err(crate::ProtocolError::from_anyhow)
            .and_then(|value| resolved.adapt_response(value));
        self.audit_host_authority_decision(
            context,
            kernel_method,
            resolved.contract.canonical_id.as_str(),
            resolved.requested_id(),
            &result,
        )
        .await;
        result
    }

    async fn audit_host_authority_decision(
        &self,
        context: &ProtocolContext,
        method: KernelMethod,
        canonical_method: &str,
        requested_method: &str,
        result: &Result<Value, crate::ProtocolError>,
    ) {
        let Some(grant_id) = context.host_device_grant_id() else {
            return;
        };
        let authority = context.authority.as_ref();
        let operation_resources = context
            .host_operation
            .as_ref()
            .map(|operation| operation.resources.clone())
            .unwrap_or_default();
        let payload = json!({
            "principal": {"kind": "host_device", "grant_id": grant_id},
            "grant_id": grant_id,
            "delegation_chain": authority
                .map(|value| value.delegation_chain.clone())
                .unwrap_or_default(),
            "canonical_method": canonical_method,
            "requested_method": requested_method,
            "action": context
                .host_operation
                .as_ref()
                .map(|operation| operation.action.as_str())
                .unwrap_or_else(|| host_action_for_method(method)),
            "operation_resources": operation_resources,
            "granted_resources": authority
                .map(|value| value.resources.clone())
                .unwrap_or_default(),
            "decision": if result.is_ok() { "allow" } else { "deny" },
            "error_code": result.as_ref().err().map(|error| error.code.as_str()),
            "correlation_id": context.correlation_id,
            "parent_invocation_id": context.parent_invocation_id,
            "transport": context.transport,
        });
        if let Err(error) = self
            .store
            .append_with_sequence(
                HOST_AUTHORITY_AUDIT_SESSION.to_string(),
                HOST_AUTHORITY_AUDIT_WRITER.to_string(),
                HOST_AUTHORITY_AUDIT_EVENT.to_string(),
                1,
                payload,
                json!({"credential_material": "none"}),
            )
            .await
        {
            eprintln!("failed to append Host authority decision audit: {error}");
        }
    }

    pub(crate) async fn dispatch_protocol_method(
        &self,
        context: &ProtocolContext,
        kernel_method: KernelMethod,
        params: Value,
    ) -> anyhow::Result<Value> {
        ensure_global_host_catalog_access(context, kernel_method)?;
        if is_deployment_hub_method(kernel_method) {
            ensure_deployment_hub_control_allowed(context, kernel_method)?;
        }
        match kernel_method {
            // Host domain
            KernelMethod::HostInfo => Ok(serde_json::to_value(crate::host_info())?),
            KernelMethod::HostPing => Ok(json!({"ok": true})),
            KernelMethod::HostDiagnostics => Ok(self.host_diagnostics().await),

            // Surface domain
            KernelMethod::SurfaceResolveBundle => {
                self.dispatch_surface_resolve_bundle(context, &params).await
            }
            KernelMethod::SurfaceContributionList => {
                self.dispatch_surface_list(context, &params).await
            }
            KernelMethod::SurfaceContributionDescribe => {
                self.dispatch_surface_describe(context, &params).await
            }

            // Outbound domain
            KernelMethod::OutboundAudit => self.dispatch_outbound_audit(&params).await,
            KernelMethod::OutboundExecute => self.dispatch_outbound_execute(context, params).await,
            KernelMethod::OutboundStream => self.dispatch_outbound_stream(context, params).await,
            KernelMethod::OutboundWebSocketOpen => {
                self.dispatch_outbound_websocket_open(context, params).await
            }
            KernelMethod::OutboundWebSocketSend => {
                self.dispatch_outbound_websocket_send(context, &params)
                    .await
            }
            KernelMethod::OutboundWebSocketClose => {
                self.dispatch_outbound_websocket_close(context, &params)
                    .await
            }

            // Permission domain
            KernelMethod::PermissionGrant => self.dispatch_permission_grant(&params).await,
            KernelMethod::PermissionRevoke => self.dispatch_permission_revoke(&params).await,
            KernelMethod::PermissionList => self.dispatch_permission_list(&params).await,
            KernelMethod::PermissionAudit => self.dispatch_permission_audit().await,

            // Audit domain
            KernelMethod::AuditPackage => self.dispatch_audit_package(&params).await,

            // Proposal domain
            KernelMethod::ProposalCreate => self.dispatch_proposal_create(context, &params).await,
            KernelMethod::ProposalGet => self.dispatch_proposal_get(context, &params).await,
            KernelMethod::ProposalList => self.dispatch_proposal_list(context).await,
            KernelMethod::ProposalApprove => self.dispatch_proposal_approve(context, &params).await,
            KernelMethod::ProposalReject => self.dispatch_proposal_reject(context, &params).await,
            KernelMethod::ProposalApply => self.dispatch_proposal_apply(context, &params).await,

            // Session domain
            KernelMethod::SessionOpen => self.dispatch_session_open(context, params).await,
            KernelMethod::SessionClose => self.dispatch_session_close(context, &params).await,
            KernelMethod::SessionFork => self.dispatch_session_fork(context, &params).await,
            KernelMethod::SessionBranchList => {
                self.dispatch_session_branch_list(context, &params).await
            }
            KernelMethod::SessionGet => self.dispatch_session_get(context, &params).await,

            // Event domain
            KernelMethod::EventAppend => Ok(serde_json::to_value(
                self.append_event_with_context(context, serde_json::from_value(params)?)
                    .await?,
            )?),
            KernelMethod::EventList => self.dispatch_event_list(context, &params).await,

            // Package domain
            KernelMethod::PackageLoad => Ok(serde_json::to_value(
                self.load_package(serde_json::from_value(params)?).await?,
            )?),
            KernelMethod::PackageList => Ok(serde_json::to_value(self.list_packages().await)?),
            KernelMethod::PackageStatus => self.dispatch_package_status(&params).await,
            KernelMethod::PackageUnload => self.dispatch_package_unload(&params).await,
            KernelMethod::PackageRestart => self.dispatch_package_restart(&params).await,
            KernelMethod::PackageLogs => self.dispatch_package_logs(&params).await,

            // Project domain
            KernelMethod::ProjectList => self.dispatch_project_list(context, &params).await,
            KernelMethod::ProjectGet => self.dispatch_project_get(context, &params).await,
            KernelMethod::ProjectStart => self.dispatch_project_start(context, &params).await,
            KernelMethod::ProjectStop => self.dispatch_project_stop(context, &params).await,
            KernelMethod::ProjectStatus => self.dispatch_project_status(context, &params).await,

            // Deployment Hub Phase 1 primitives
            KernelMethod::TargetList => self.dispatch_target_list(context).await,
            KernelMethod::TargetStatus => self.dispatch_target_status(context, &params).await,
            KernelMethod::TargetRegister => self.dispatch_target_register(context, params).await,
            KernelMethod::TargetUnregister => {
                self.dispatch_target_unregister(context, &params).await
            }
            KernelMethod::ExecStart => self.dispatch_exec_start(context, params).await,
            KernelMethod::ExecStop => self.dispatch_exec_stop(context, params).await,
            KernelMethod::ExecStatus => self.dispatch_exec_status(context, params).await,
            KernelMethod::ExecLogs => self.dispatch_exec_logs(context, params).await,
            KernelMethod::ExecList => self.dispatch_exec_list(context).await,
            KernelMethod::PortLease => self.dispatch_port_lease(context, params).await,
            KernelMethod::PortRelease => self.dispatch_port_release(context, &params).await,
            KernelMethod::PortStatus => self.dispatch_port_status(context, &params).await,
            KernelMethod::PortList => self.dispatch_port_list(context).await,
            KernelMethod::ProxyRegister => self.dispatch_proxy_register(context, params).await,
            KernelMethod::ProxyUnregister => self.dispatch_proxy_unregister(context, &params).await,
            KernelMethod::ProxyStatus => self.dispatch_proxy_status(context, &params).await,
            KernelMethod::ProxyList => self.dispatch_proxy_list(context).await,

            // Capability domain
            KernelMethod::CapabilityDiscover => {
                Ok(serde_json::to_value(self.discover_capabilities().await)?)
            }
            KernelMethod::CapabilityInvoke => Ok(serde_json::to_value(
                self.invoke_capability_with_context(context, serde_json::from_value(params)?)
                    .await?,
            )?),
            KernelMethod::CapabilityHandleAttenuate => self.dispatch_cap_attenuate(&params).await,
            KernelMethod::CapabilityHandleRevoke => self.dispatch_cap_revoke(&params).await,
            KernelMethod::CapabilityHandleListFor => self.dispatch_cap_list_for(&params).await,
            KernelMethod::CapabilityStream => self.dispatch_capability_stream(&params).await,
            KernelMethod::CapabilityCancel => self.dispatch_capability_cancel(&params).await,

            // Extension / hook domain
            KernelMethod::ExtensionPointList => Ok(json!([
                "kernel/v1/event.before_append",
                "kernel/v1/event.after_append",
                "kernel/v1/capability.before_invoke",
                "kernel/v1/capability.after_invoke",
                "kernel/v1/package.loaded",
                "kernel/v1/package.unloaded"
            ])),
            KernelMethod::HookList => Ok(serde_json::to_value(
                self.extensions.list_all_hooks().await,
            )?),

            // Asset domain
            KernelMethod::AssetPut => Ok(serde_json::to_value(
                self.put_asset(serde_json::from_value(params)?).await?,
            )?),
            KernelMethod::AssetGet => self.dispatch_asset_get(&params).await,
            KernelMethod::AssetList => Ok(serde_json::to_value(self.list_assets().await)?),

            // Projection domain
            KernelMethod::ProjectionRegister => Ok(serde_json::to_value(
                self.projection_register(serde_json::from_value(params)?)
                    .await?,
            )?),
            KernelMethod::ProjectionRebuild => self.dispatch_projection_rebuild(&params).await,
            KernelMethod::ProjectionGet => self.dispatch_projection_get(&params).await,
            KernelMethod::ProjectionList => Ok(serde_json::to_value(self.projection_list().await)?),

            // Planned methods — no dispatch yet
            KernelMethod::SessionList
            | KernelMethod::EventSubscribe
            | KernelMethod::PackageDescribe
            | KernelMethod::CapabilityDescribe
            | KernelMethod::ExtensionPointDescribe
            | KernelMethod::HostPrincipal => {
                anyhow::bail!("protocol method '{}' is not yet implemented", kernel_method)
            }
        }
    }
}

fn is_deployment_hub_method(method: KernelMethod) -> bool {
    matches!(
        method,
        KernelMethod::TargetList
            | KernelMethod::TargetStatus
            | KernelMethod::TargetRegister
            | KernelMethod::TargetUnregister
            | KernelMethod::ExecStart
            | KernelMethod::ExecStop
            | KernelMethod::ExecStatus
            | KernelMethod::ExecLogs
            | KernelMethod::ExecList
            | KernelMethod::PortLease
            | KernelMethod::PortRelease
            | KernelMethod::PortStatus
            | KernelMethod::PortList
            | KernelMethod::ProxyRegister
            | KernelMethod::ProxyUnregister
            | KernelMethod::ProxyStatus
            | KernelMethod::ProxyList
    )
}

fn ensure_global_host_catalog_access(
    context: &ProtocolContext,
    method: KernelMethod,
) -> anyhow::Result<()> {
    let global = matches!(
        method,
        KernelMethod::HostDiagnostics
            | KernelMethod::PackageLoad
            | KernelMethod::PackageList
            | KernelMethod::PackageStatus
            | KernelMethod::PackageUnload
            | KernelMethod::PackageRestart
            | KernelMethod::PackageLogs
            | KernelMethod::PackageDescribe
            | KernelMethod::CapabilityDiscover
            | KernelMethod::CapabilityDescribe
            | KernelMethod::ExtensionPointList
            | KernelMethod::ExtensionPointDescribe
            | KernelMethod::HookList
            | KernelMethod::AssetPut
            | KernelMethod::AssetGet
            | KernelMethod::AssetList
            | KernelMethod::ProjectionRegister
            | KernelMethod::ProjectionRebuild
            | KernelMethod::ProjectionGet
            | KernelMethod::ProjectionList
    );
    if !global {
        return Ok(());
    }
    let action = host_action_for_method(method);
    anyhow::ensure!(
        context.allows_host_action(action),
        "{} permission denied: authenticated authority lacks {action}",
        method
    );
    anyhow::ensure!(
        context.allows_all_host_resources("host", "project"),
        "{} permission denied: Host-global objects require all-project authority",
        method
    );
    Ok(())
}

fn host_action_for_method(method: KernelMethod) -> &'static str {
    match method {
        KernelMethod::ProjectStart
        | KernelMethod::ProjectStop
        | KernelMethod::SessionOpen
        | KernelMethod::SessionClose
        | KernelMethod::SessionFork => "project_operate",
        KernelMethod::ProposalCreate => "develop_propose",
        KernelMethod::ProposalApprove | KernelMethod::ProposalReject => "develop_approve",
        KernelMethod::ProposalApply => "develop_execute",
        KernelMethod::TargetRegister
        | KernelMethod::TargetUnregister
        | KernelMethod::ExecStart
        | KernelMethod::ExecStop
        | KernelMethod::PortLease
        | KernelMethod::PortRelease
        | KernelMethod::ProxyRegister
        | KernelMethod::ProxyUnregister => "deploy",
        KernelMethod::HostInfo
        | KernelMethod::HostPing
        | KernelMethod::HostDiagnostics
        | KernelMethod::ProjectList
        | KernelMethod::ProjectGet
        | KernelMethod::ProjectStatus
        | KernelMethod::TargetList
        | KernelMethod::TargetStatus
        | KernelMethod::ExecStatus
        | KernelMethod::ExecLogs
        | KernelMethod::ExecList
        | KernelMethod::PortStatus
        | KernelMethod::PortList
        | KernelMethod::ProxyStatus
        | KernelMethod::ProxyList
        | KernelMethod::SessionBranchList
        | KernelMethod::SessionGet
        | KernelMethod::SessionList
        | KernelMethod::EventList
        | KernelMethod::EventSubscribe
        | KernelMethod::PackageLogs
        | KernelMethod::PackageList
        | KernelMethod::PackageStatus
        | KernelMethod::PackageDescribe
        | KernelMethod::CapabilityDiscover
        | KernelMethod::CapabilityDescribe
        | KernelMethod::ExtensionPointList
        | KernelMethod::ExtensionPointDescribe
        | KernelMethod::HookList
        | KernelMethod::AssetGet
        | KernelMethod::AssetList
        | KernelMethod::ProjectionGet
        | KernelMethod::ProjectionList
        | KernelMethod::ProposalGet
        | KernelMethod::ProposalList
        | KernelMethod::SurfaceResolveBundle
        | KernelMethod::SurfaceContributionList
        | KernelMethod::SurfaceContributionDescribe => "observe",
        _ => "access_manage",
    }
}

fn ensure_deployment_hub_control_allowed(
    context: &ProtocolContext,
    method: KernelMethod,
) -> anyhow::Result<()> {
    let action = if matches!(
        method,
        KernelMethod::TargetList
            | KernelMethod::TargetStatus
            | KernelMethod::ExecStatus
            | KernelMethod::ExecLogs
            | KernelMethod::ExecList
            | KernelMethod::PortStatus
            | KernelMethod::PortList
            | KernelMethod::ProxyStatus
            | KernelMethod::ProxyList
    ) {
        "observe"
    } else {
        "deploy"
    };
    if context.is_host_device() {
        anyhow::ensure!(
            context.allows_host_action(action),
            "permission denied: deployment hub method requires authenticated Host action '{action}'"
        );
        return Ok(());
    }
    anyhow::ensure!(
        matches!(
            context.principal,
            ProtocolPrincipal::HostAdmin | ProtocolPrincipal::HostDev
        ),
        "permission denied: deployment hub method requires authenticated Host action '{action}'"
    );
    Ok(())
}
