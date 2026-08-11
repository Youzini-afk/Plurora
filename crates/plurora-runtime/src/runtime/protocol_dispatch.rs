use serde_json::{json, Value};

use super::{AssetPutRequest, ObjectPutScope, Runtime};
use crate::{
    negotiate_contract, resolve_contract_method, ContractSelection, EventStore, PlatformMethod,
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
        let result = self
            .dispatch_protocol_method(context, resolved.method, params)
            .await
            .map_err(crate::ProtocolError::from_anyhow);
        self.audit_host_authority_decision(
            context,
            resolved.method,
            resolved.contract.id.as_str(),
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
        let platform_method = resolved.method;
        let gate = ensure_global_host_catalog_access(context, platform_method).and_then(|()| {
            if is_deployment_hub_method(platform_method) {
                ensure_deployment_hub_control_allowed(context, platform_method)
            } else {
                Ok(())
            }
        });
        let result: anyhow::Result<Value> = if let Err(error) = gate {
            Err(error)
        } else {
            match platform_method {
                PlatformMethod::OutboundExecute => {
                    self.dispatch_outbound_execute(context, params).await
                }
                PlatformMethod::OutboundStream => {
                    self.dispatch_outbound_stream(context, params).await
                }
                PlatformMethod::OutboundWebSocketOpen => {
                    self.dispatch_outbound_websocket_open(context, params).await
                }
                PlatformMethod::OutboundWebSocketSend => {
                    self.dispatch_outbound_websocket_send(context, &params)
                        .await
                }
                PlatformMethod::OutboundWebSocketClose => {
                    self.dispatch_outbound_websocket_close(context, &params)
                        .await
                }
                PlatformMethod::TargetList => self.dispatch_target_list(context).await,
                PlatformMethod::TargetStatus => self.dispatch_target_status(context, &params).await,
                PlatformMethod::TargetRegister => {
                    self.dispatch_target_register(context, params).await
                }
                PlatformMethod::TargetUnregister => {
                    self.dispatch_target_unregister(context, &params).await
                }
                PlatformMethod::ExecStart => self.dispatch_exec_start(context, params).await,
                PlatformMethod::ExecStop => self.dispatch_exec_stop(context, params).await,
                PlatformMethod::ExecStatus => self.dispatch_exec_status(context, params).await,
                PlatformMethod::ExecLogs => self.dispatch_exec_logs(context, params).await,
                PlatformMethod::ExecList => self.dispatch_exec_list(context).await,
                PlatformMethod::PortLease => self.dispatch_port_lease(context, params).await,
                PlatformMethod::PortRelease => self.dispatch_port_release(context, &params).await,
                PlatformMethod::PortStatus => self.dispatch_port_status(context, &params).await,
                PlatformMethod::PortList => self.dispatch_port_list(context).await,
                PlatformMethod::ProxyRegister => {
                    self.dispatch_proxy_register(context, params).await
                }
                PlatformMethod::ProxyUnregister => {
                    self.dispatch_proxy_unregister(context, &params).await
                }
                PlatformMethod::ProxyStatus => self.dispatch_proxy_status(context, &params).await,
                PlatformMethod::ProxyList => self.dispatch_proxy_list(context).await,
                PlatformMethod::CapabilityCancel => self.dispatch_capability_cancel(&params).await,
                PlatformMethod::HostInfo => {
                    serde_json::to_value(crate::host_info()).map_err(anyhow::Error::from)
                }
                PlatformMethod::HostPing => Ok(json!({"ok": true})),
                PlatformMethod::HostDiagnostics => Ok(self.host_diagnostics().await),
                PlatformMethod::CapabilityDiscover => {
                    serde_json::to_value(self.discover_capabilities().await)
                        .map_err(anyhow::Error::from)
                }
                PlatformMethod::CapabilityInvoke => match serde_json::from_value(params) {
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
        let result = result.map_err(crate::ProtocolError::from_anyhow);
        self.audit_host_authority_decision(
            context,
            platform_method,
            resolved.contract.id.as_str(),
            &result,
        )
        .await;
        result
    }

    async fn audit_host_authority_decision(
        &self,
        context: &ProtocolContext,
        method: PlatformMethod,
        method_id: &str,
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
            "method": method_id,
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
        platform_method: PlatformMethod,
        params: Value,
    ) -> anyhow::Result<Value> {
        ensure_global_host_catalog_access(context, platform_method)?;
        if is_deployment_hub_method(platform_method) {
            ensure_deployment_hub_control_allowed(context, platform_method)?;
        }
        match platform_method {
            // Host domain
            PlatformMethod::HostInfo => Ok(serde_json::to_value(crate::host_info())?),
            PlatformMethod::HostPing => Ok(json!({"ok": true})),
            PlatformMethod::HostDiagnostics => Ok(self.host_diagnostics().await),

            // Surface domain
            PlatformMethod::SurfaceResolveBundle => {
                self.dispatch_surface_resolve_bundle(context, &params).await
            }
            PlatformMethod::SurfaceContributionList => {
                self.dispatch_surface_list(context, &params).await
            }
            PlatformMethod::SurfaceContributionDescribe => {
                self.dispatch_surface_describe(context, &params).await
            }

            // Outbound domain
            PlatformMethod::OutboundAudit => self.dispatch_outbound_audit(&params).await,
            PlatformMethod::OutboundExecute => {
                self.dispatch_outbound_execute(context, params).await
            }
            PlatformMethod::OutboundStream => self.dispatch_outbound_stream(context, params).await,
            PlatformMethod::OutboundWebSocketOpen => {
                self.dispatch_outbound_websocket_open(context, params).await
            }
            PlatformMethod::OutboundWebSocketSend => {
                self.dispatch_outbound_websocket_send(context, &params)
                    .await
            }
            PlatformMethod::OutboundWebSocketClose => {
                self.dispatch_outbound_websocket_close(context, &params)
                    .await
            }

            // Permission domain
            PlatformMethod::PermissionGrant => self.dispatch_permission_grant(&params).await,
            PlatformMethod::PermissionRevoke => self.dispatch_permission_revoke(&params).await,
            PlatformMethod::PermissionList => self.dispatch_permission_list(&params).await,
            PlatformMethod::PermissionAudit => self.dispatch_permission_audit().await,

            // Audit domain
            PlatformMethod::AuditPackage => self.dispatch_audit_package(&params).await,

            // Proposal domain
            PlatformMethod::ProposalCreate => self.dispatch_proposal_create(context, &params).await,
            PlatformMethod::ProposalGet => self.dispatch_proposal_get(context, &params).await,
            PlatformMethod::ProposalList => self.dispatch_proposal_list(context).await,
            PlatformMethod::ProposalApprove => {
                self.dispatch_proposal_approve(context, &params).await
            }
            PlatformMethod::ProposalReject => self.dispatch_proposal_reject(context, &params).await,
            PlatformMethod::ProposalApply => self.dispatch_proposal_apply(context, &params).await,

            // Session domain
            PlatformMethod::SessionOpen => self.dispatch_session_open(context, params).await,
            PlatformMethod::SessionClose => self.dispatch_session_close(context, &params).await,
            PlatformMethod::SessionFork => self.dispatch_session_fork(context, &params).await,
            PlatformMethod::SessionBranchList => {
                self.dispatch_session_branch_list(context, &params).await
            }
            PlatformMethod::SessionGet => self.dispatch_session_get(context, &params).await,

            // Event domain
            PlatformMethod::EventAppend => Ok(serde_json::to_value(
                self.append_event_with_context(context, serde_json::from_value(params)?)
                    .await?,
            )?),
            PlatformMethod::EventList => self.dispatch_event_list(context, &params).await,

            // Package domain
            PlatformMethod::PackageLoad => Ok(serde_json::to_value(
                self.load_package(serde_json::from_value(params)?).await?,
            )?),
            PlatformMethod::PackageList => Ok(serde_json::to_value(self.list_packages().await)?),
            PlatformMethod::PackageStatus => self.dispatch_package_status(&params).await,
            PlatformMethod::PackageUnload => self.dispatch_package_unload(&params).await,
            PlatformMethod::PackageRestart => self.dispatch_package_restart(&params).await,
            PlatformMethod::PackageLogs => self.dispatch_package_logs(&params).await,

            // Installation domain
            PlatformMethod::InstallationList => {
                self.dispatch_installation_list(context, params).await
            }
            PlatformMethod::InstallationGet => {
                self.dispatch_installation_get(context, params).await
            }
            PlatformMethod::InstallationCreate => {
                self.dispatch_installation_create(context, params).await
            }
            PlatformMethod::InstallationUpdate => {
                self.dispatch_installation_update(context, params).await
            }
            PlatformMethod::InstallationRemove => {
                self.dispatch_installation_remove(context, params).await
            }

            // Powerbox exposure / binding domain
            PlatformMethod::ExposureList => self.dispatch_exposure_list(context, params).await,
            PlatformMethod::ExposureCreate => self.dispatch_exposure_create(context, params).await,
            PlatformMethod::ExposureRevoke => self.dispatch_exposure_revoke(context, params).await,
            PlatformMethod::BindingList => self.dispatch_binding_list(context, params).await,
            PlatformMethod::BindingCandidates => {
                self.dispatch_binding_candidates(context, params).await
            }
            PlatformMethod::BindingSelect => self.dispatch_binding_select(context, params).await,
            PlatformMethod::BindingRevoke => self.dispatch_binding_revoke(context, params).await,

            // Run domain
            PlatformMethod::RunList => self.dispatch_run_list(context, params).await,
            PlatformMethod::RunGet => self.dispatch_run_get(context, params).await,
            PlatformMethod::RunStart => self.dispatch_run_start(context, params).await,
            PlatformMethod::RunStop => self.dispatch_run_stop(context, params).await,
            PlatformMethod::RunStatus => self.dispatch_run_status(context, params).await,

            // Managed Realization domain
            PlatformMethod::RealizationPlan => {
                self.dispatch_realization_plan(context, params).await
            }
            PlatformMethod::RealizationApply => {
                self.dispatch_realization_apply(context, params).await
            }
            PlatformMethod::RealizationGet => self.dispatch_realization_get(context, params).await,
            PlatformMethod::RealizationList => {
                self.dispatch_realization_list(context, params).await
            }
            PlatformMethod::RealizationStop => {
                self.dispatch_realization_stop(context, params).await
            }
            PlatformMethod::RealizationRollback => {
                self.dispatch_realization_rollback(context, params).await
            }
            PlatformMethod::RealizationReconcile => {
                self.dispatch_realization_reconcile(context, params).await
            }

            // Deployment Hub Phase 1 primitives
            PlatformMethod::TargetList => self.dispatch_target_list(context).await,
            PlatformMethod::TargetStatus => self.dispatch_target_status(context, &params).await,
            PlatformMethod::TargetRegister => self.dispatch_target_register(context, params).await,
            PlatformMethod::TargetUnregister => {
                self.dispatch_target_unregister(context, &params).await
            }
            PlatformMethod::ExecStart => self.dispatch_exec_start(context, params).await,
            PlatformMethod::ExecStop => self.dispatch_exec_stop(context, params).await,
            PlatformMethod::ExecStatus => self.dispatch_exec_status(context, params).await,
            PlatformMethod::ExecLogs => self.dispatch_exec_logs(context, params).await,
            PlatformMethod::ExecList => self.dispatch_exec_list(context).await,
            PlatformMethod::PortLease => self.dispatch_port_lease(context, params).await,
            PlatformMethod::PortRelease => self.dispatch_port_release(context, &params).await,
            PlatformMethod::PortStatus => self.dispatch_port_status(context, &params).await,
            PlatformMethod::PortList => self.dispatch_port_list(context).await,
            PlatformMethod::ProxyRegister => self.dispatch_proxy_register(context, params).await,
            PlatformMethod::ProxyUnregister => {
                self.dispatch_proxy_unregister(context, &params).await
            }
            PlatformMethod::ProxyStatus => self.dispatch_proxy_status(context, &params).await,
            PlatformMethod::ProxyList => self.dispatch_proxy_list(context).await,

            // Capability domain
            PlatformMethod::CapabilityDiscover => {
                Ok(serde_json::to_value(self.discover_capabilities().await)?)
            }
            PlatformMethod::CapabilityInvoke => Ok(serde_json::to_value(
                self.invoke_capability_with_context(context, serde_json::from_value(params)?)
                    .await?,
            )?),
            PlatformMethod::CapabilityHandleAttenuate => self.dispatch_cap_attenuate(&params).await,
            PlatformMethod::CapabilityHandleRevoke => self.dispatch_cap_revoke(&params).await,
            PlatformMethod::CapabilityHandleListFor => self.dispatch_cap_list_for(&params).await,
            PlatformMethod::CapabilityStream => {
                self.dispatch_capability_stream(context, &params).await
            }
            PlatformMethod::CapabilityCancel => self.dispatch_capability_cancel(&params).await,

            // Extension / hook domain
            PlatformMethod::ExtensionPointList => Ok(json!([
                "journal/before_append",
                "journal/after_append",
                "capability/before_invoke",
                "capability/after_invoke",
                "host/package.loaded",
                "host/package.unloaded"
            ])),
            PlatformMethod::HookList => Ok(serde_json::to_value(
                self.extensions.list_all_hooks().await,
            )?),

            // Asset domain
            PlatformMethod::AssetPut => {
                let request: AssetPutRequest = serde_json::from_value(params)?;
                ensure_object_put_access(context, &request)?;
                Ok(serde_json::to_value(self.put_object(request).await?)?)
            }
            PlatformMethod::AssetGet => self.dispatch_asset_get(&params).await,
            PlatformMethod::AssetList => Ok(serde_json::to_value(self.list_assets().await)?),

            // Projection domain
            PlatformMethod::ProjectionRegister => Ok(serde_json::to_value(
                self.projection_register(serde_json::from_value(params)?)
                    .await?,
            )?),
            PlatformMethod::ProjectionRebuild => self.dispatch_projection_rebuild(&params).await,
            PlatformMethod::ProjectionGet => self.dispatch_projection_get(&params).await,
            PlatformMethod::ProjectionList => {
                Ok(serde_json::to_value(self.projection_list().await)?)
            }

            // Planned methods — no dispatch yet
            PlatformMethod::SessionList
            | PlatformMethod::EventSubscribe
            | PlatformMethod::PackageDescribe
            | PlatformMethod::CapabilityDescribe
            | PlatformMethod::ExtensionPointDescribe
            | PlatformMethod::HostPrincipal => {
                anyhow::bail!(
                    "protocol method '{}' is not yet implemented",
                    platform_method
                )
            }
        }
    }
}

fn is_deployment_hub_method(method: PlatformMethod) -> bool {
    matches!(
        method,
        PlatformMethod::TargetList
            | PlatformMethod::TargetStatus
            | PlatformMethod::TargetRegister
            | PlatformMethod::TargetUnregister
            | PlatformMethod::ExecStart
            | PlatformMethod::ExecStop
            | PlatformMethod::ExecStatus
            | PlatformMethod::ExecLogs
            | PlatformMethod::ExecList
            | PlatformMethod::PortLease
            | PlatformMethod::PortRelease
            | PlatformMethod::PortStatus
            | PlatformMethod::PortList
            | PlatformMethod::ProxyRegister
            | PlatformMethod::ProxyUnregister
            | PlatformMethod::ProxyStatus
            | PlatformMethod::ProxyList
    )
}

fn ensure_global_host_catalog_access(
    context: &ProtocolContext,
    method: PlatformMethod,
) -> anyhow::Result<()> {
    let global = matches!(
        method,
        PlatformMethod::HostDiagnostics
            | PlatformMethod::PackageLoad
            | PlatformMethod::PackageList
            | PlatformMethod::PackageStatus
            | PlatformMethod::PackageUnload
            | PlatformMethod::PackageRestart
            | PlatformMethod::PackageLogs
            | PlatformMethod::PackageDescribe
            | PlatformMethod::CapabilityDiscover
            | PlatformMethod::CapabilityDescribe
            | PlatformMethod::ExtensionPointList
            | PlatformMethod::ExtensionPointDescribe
            | PlatformMethod::HookList
            | PlatformMethod::AssetGet
            | PlatformMethod::AssetList
            | PlatformMethod::ProjectionRegister
            | PlatformMethod::ProjectionRebuild
            | PlatformMethod::ProjectionGet
            | PlatformMethod::ProjectionList
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
        context.allows_all_host_resources("host", "installation"),
        "{} permission denied: Host-global objects require all-installation authority",
        method
    );
    Ok(())
}

fn ensure_object_put_access(
    context: &ProtocolContext,
    request: &AssetPutRequest,
) -> anyhow::Result<()> {
    let Some(upload) = request.artifact.as_ref() else {
        anyhow::ensure!(
            context.allows_host_action("access_manage")
                && context.allows_all_host_resources("host", "installation"),
            "object.put permission denied: ordinary Assets require access_manage and all-installation authority"
        );
        return Ok(());
    };

    anyhow::ensure!(
        context.allows_host_action("installation.manage"),
        "object.put permission denied: exact artifacts require installation.manage"
    );
    match &upload.scope {
        ObjectPutScope::InstallationCreate { work_id } => anyhow::ensure!(
            context.allows_host_resource("host", "work", work_id.as_str()),
            "object.put permission denied: exact artifact Work authority does not match"
        ),
        ObjectPutScope::InstallationUpdate {
            installation_id,
            work_id,
        } => anyhow::ensure!(
            context.allows_host_resource("host", "work", work_id.as_str())
                && context.allows_host_resource(
                    "host",
                    "installation",
                    installation_id.as_str()
                ),
            "object.put permission denied: exact artifact Work or Installation authority does not match"
        ),
    }
    Ok(())
}

fn host_action_for_method(method: PlatformMethod) -> &'static str {
    match method {
        PlatformMethod::SessionOpen
        | PlatformMethod::SessionClose
        | PlatformMethod::SessionFork => "access_manage",
        PlatformMethod::RunStart | PlatformMethod::RunStop => "run",
        PlatformMethod::RealizationPlan => "realization.plan",
        PlatformMethod::RealizationApply
        | PlatformMethod::RealizationStop
        | PlatformMethod::RealizationRollback
        | PlatformMethod::RealizationReconcile => "realization.apply",
        PlatformMethod::ExposureCreate | PlatformMethod::ExposureRevoke => "exposure.manage",
        PlatformMethod::BindingSelect | PlatformMethod::BindingRevoke => "binding.manage",
        PlatformMethod::InstallationCreate
        | PlatformMethod::InstallationUpdate
        | PlatformMethod::InstallationRemove => "installation.manage",
        PlatformMethod::ProposalCreate => "develop_propose",
        PlatformMethod::ProposalApprove | PlatformMethod::ProposalReject => "develop_approve",
        PlatformMethod::ProposalApply => "develop_execute",
        PlatformMethod::TargetRegister
        | PlatformMethod::TargetUnregister
        | PlatformMethod::ExecStart
        | PlatformMethod::ExecStop
        | PlatformMethod::PortLease
        | PlatformMethod::PortRelease
        | PlatformMethod::ProxyRegister
        | PlatformMethod::ProxyUnregister => "access_manage",
        PlatformMethod::HostInfo
        | PlatformMethod::HostPing
        | PlatformMethod::HostDiagnostics
        | PlatformMethod::InstallationList
        | PlatformMethod::InstallationGet
        | PlatformMethod::ExposureList
        | PlatformMethod::BindingList
        | PlatformMethod::BindingCandidates
        | PlatformMethod::RunList
        | PlatformMethod::RunGet
        | PlatformMethod::RunStatus
        | PlatformMethod::RealizationGet
        | PlatformMethod::RealizationList
        | PlatformMethod::TargetList
        | PlatformMethod::TargetStatus
        | PlatformMethod::ExecStatus
        | PlatformMethod::ExecLogs
        | PlatformMethod::ExecList
        | PlatformMethod::PortStatus
        | PlatformMethod::PortList
        | PlatformMethod::ProxyStatus
        | PlatformMethod::ProxyList
        | PlatformMethod::SessionBranchList
        | PlatformMethod::SessionGet
        | PlatformMethod::SessionList
        | PlatformMethod::EventList
        | PlatformMethod::EventSubscribe
        | PlatformMethod::PackageLogs
        | PlatformMethod::PackageList
        | PlatformMethod::PackageStatus
        | PlatformMethod::PackageDescribe
        | PlatformMethod::CapabilityDiscover
        | PlatformMethod::CapabilityDescribe
        | PlatformMethod::ExtensionPointList
        | PlatformMethod::ExtensionPointDescribe
        | PlatformMethod::HookList
        | PlatformMethod::AssetGet
        | PlatformMethod::AssetList
        | PlatformMethod::ProjectionGet
        | PlatformMethod::ProjectionList
        | PlatformMethod::ProposalGet
        | PlatformMethod::ProposalList
        | PlatformMethod::SurfaceResolveBundle
        | PlatformMethod::SurfaceContributionList
        | PlatformMethod::SurfaceContributionDescribe => "observe",
        _ => "access_manage",
    }
}

fn ensure_deployment_hub_control_allowed(
    context: &ProtocolContext,
    method: PlatformMethod,
) -> anyhow::Result<()> {
    let action = if matches!(
        method,
        PlatformMethod::TargetList
            | PlatformMethod::TargetStatus
            | PlatformMethod::ExecStatus
            | PlatformMethod::ExecLogs
            | PlatformMethod::ExecList
            | PlatformMethod::PortStatus
            | PlatformMethod::PortList
            | PlatformMethod::ProxyStatus
            | PlatformMethod::ProxyList
    ) {
        "observe"
    } else {
        "access_manage"
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
