use plurora_core::*;
use plurora_runtime::*;
use serde_json::{json, Value};

use super::defs;
use super::defs::*;
use super::{BASE, SCHEMA};

pub(crate) fn method_schema(method: KernelMethod, params: Value, result: Value) -> Value {
    json!({
        "$schema": SCHEMA,
        "$id": format!("{BASE}/methods/{}.schema.json", method.id()),
        "title": method.id(),
        "description": format!("Plurora public kernel method {}.", method.id()),
        "x-plurora-contract": method.contract(),
        "type": "object",
        "additionalProperties": false,
        "required": ["method", "params"],
        "properties": {
            "method": { "const": method.id() },
            "params": { "$ref": "#/$defs/Params" },
            "result": { "$ref": "#/$defs/Result" },
            "errors": { "$ref": "#/$defs/Errors" }
        },
        "$defs": { "Params": params, "Result": result, "Errors": { "type": "array", "items": schema_value::<ErrorShape>() } }
    })
}

pub(crate) fn method_schemas() -> Vec<(KernelMethod, Value, Value)> {
    KernelMethod::all()
        .iter()
        .map(|method| {
            let method = *method;
            let (params, result) = match method {
                KernelMethod::SessionOpen => (
                    schema_value::<OpenSessionRequest>(),
                    schema_value::<KernelSession>(),
                ),
            KernelMethod::SessionClose => (
                schema_value::<SessionCloseParams>(),
                schema_value::<EventEnvelope>(),
            ),
            KernelMethod::SessionGet => (
                schema_value::<SessionGetParams>(),
                schema_value::<KernelSession>(),
            ),
            KernelMethod::SessionFork => (
                schema_value::<SessionForkParams>(),
                schema_value::<BranchRecord>(),
            ),
            KernelMethod::SessionBranchList => (
                schema_value::<SessionBranchListParams>(),
                json!({"type":"array","items":schema_value::<BranchRecord>()}),
            ),
            KernelMethod::SessionList
            | KernelMethod::EventSubscribe
            | KernelMethod::PackageDescribe
            | KernelMethod::CapabilityDescribe
            | KernelMethod::ExtensionPointDescribe
            | KernelMethod::HostPrincipal => {
                (schema_value::<EmptyParams>(), json!({"type":"null"}))
            }
            KernelMethod::EventAppend => (
                schema_value::<AppendEventRequest>(),
                schema_value::<EventEnvelope>(),
            ),
            KernelMethod::EventList => (
                schema_value::<EventListRequest>(),
                json!({"type":"array","items":schema_value::<EventEnvelope>()}),
            ),
            KernelMethod::PackageLoad => (
                schema_value::<PackageManifest>(),
                schema_value::<PackageRecord>(),
            ),
            KernelMethod::PackageUnload
            | KernelMethod::PackageRestart
            | KernelMethod::PackageLogs
            | KernelMethod::PackageStatus => {
                let result = if method == KernelMethod::PackageLogs {
                    json!({"type":"array","items":schema_value::<SubprocessLogLine>()})
                } else {
                    schema_value::<PackageRecord>()
                };
                (schema_value::<PackageIdParams>(), result)
            }
            KernelMethod::PackageList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<PackageRecord>()}),
            ),
            KernelMethod::ProjectList => (
                schema_value::<ProjectListParams>(),
                schema_value::<ProjectListResultSchema>(),
            ),
            KernelMethod::ProjectGet => (
                schema_value::<ProjectIdParams>(),
                json!({"allOf":[schema_value::<plurora_core::project::ProjectDescriptor>()],"properties":{"state":schema_value::<plurora_core::project::ProjectState>(),"storage_summary":schema_value::<ProjectStorageSummarySchema>(),"running_session_id":{"type":"string","description":"Session id when project state is running; absent otherwise"}}}),
            ),
            KernelMethod::ProjectStart => (
                schema_value::<ProjectIdParams>(),
                schema_value::<ProjectStartResult>(),
            ),
            KernelMethod::ProjectStop => (
                schema_value::<ProjectIdParams>(),
                schema_value::<ProjectStopResult>(),
            ),
            KernelMethod::ProjectStatus => (
                schema_value::<ProjectIdParams>(),
                schema_value::<ProjectStatusResult>(),
            ),
            KernelMethod::TargetList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<ExecutionTarget>()}),
            ),
            KernelMethod::TargetStatus => (
                schema_value::<TargetIdParams>(),
                schema_value::<ExecutionTarget>(),
            ),
            KernelMethod::TargetRegister => (
                schema_value::<ExecutionTarget>(),
                schema_value::<ExecutionTarget>(),
            ),
            KernelMethod::TargetUnregister => (
                schema_value::<TargetIdParams>(),
                schema_value::<ExecutionTarget>(),
            ),
            KernelMethod::ExecStart => (
                schema_value::<LocalExecStartRequest>(),
                schema_value::<LocalExecStartResponse>(),
            ),
            KernelMethod::ExecStop => (
                schema_value::<LocalExecStopRequest>(),
                schema_value::<LocalExecStopResponse>(),
            ),
            KernelMethod::ExecStatus => (
                schema_value::<ExecIdParams>(),
                schema_value::<LocalExecStatusResponse>(),
            ),
            KernelMethod::ExecLogs => (
                schema_value::<LocalExecLogsRequest>(),
                schema_value::<LocalExecLogsResponse>(),
            ),
            KernelMethod::ExecList => (
                schema_value::<EmptyParams>(),
                schema_value::<LocalExecListResponse>(),
            ),
            KernelMethod::PortLease => (
                schema_value::<PortLeaseRequest>(),
                schema_value::<PortLeaseResponse>(),
            ),
            KernelMethod::PortRelease | KernelMethod::PortStatus => (
                schema_value::<PortLeaseIdParams>(),
                schema_value::<PortLeaseRecord>(),
            ),
            KernelMethod::PortList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<PortLeaseRecord>()}),
            ),
            KernelMethod::ProxyRegister => (
                schema_value::<ProxyRouteRegisterRequest>(),
                schema_value::<ProxyRouteRegisterResponse>(),
            ),
            KernelMethod::ProxyUnregister | KernelMethod::ProxyStatus => (
                schema_value::<ProxyRouteIdParams>(),
                schema_value::<ProxyRouteRecord>(),
            ),
            KernelMethod::ProxyList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<ProxyRouteRecord>()}),
            ),
            KernelMethod::CapabilityDiscover => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<RegisteredCapability>()}),
            ),
            KernelMethod::CapabilityInvoke => (
                schema_value::<CapabilityInvocationRequest>(),
                schema_value::<CapabilityInvocationResult>(),
            ),
            KernelMethod::CapabilityHandleAttenuate => (
                schema_value::<CapAttenuateParams>(),
                schema_value::<CapHandleResult>(),
            ),
            KernelMethod::CapabilityHandleRevoke => {
                (schema_value::<CapRevokeParams>(), json!({"type":"object"}))
            }
            KernelMethod::CapabilityHandleListFor => (
                schema_value::<CapListForParams>(),
                schema_value::<CapHandlesResult>(),
            ),
            KernelMethod::CapabilityStream => (
                schema_value::<CapabilityStreamParams>(),
                json!({"type":"object"}),
            ),
            KernelMethod::CapabilityCancel => (
                schema_value::<CapabilityCancelParams>(),
                schema_value::<StreamFrameEnvelope>(),
            ),
            KernelMethod::ExtensionPointList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":{"type":"string"}}),
            ),
            KernelMethod::HookList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<RegisteredHook>()}),
            ),
            KernelMethod::AssetPut => (
                schema_value::<AssetPutRequest>(),
                schema_value::<AssetRecord>(),
            ),
            KernelMethod::AssetGet => (
                schema_value::<AssetGetParams>(),
                schema_value::<AssetGetResponse>(),
            ),
            KernelMethod::AssetList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<AssetRecord>()}),
            ),
            KernelMethod::ProjectionRegister => (
                schema_value::<ProjectionDefinition>(),
                schema_value::<ProjectionDefinition>(),
            ),
            KernelMethod::ProjectionRebuild | KernelMethod::ProjectionGet => (
                schema_value::<ProjectionIdParams>(),
                schema_value::<ProjectionDefinition>(),
            ),
            KernelMethod::ProjectionList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<ProjectionDefinition>()}),
            ),
            KernelMethod::HostInfo => (schema_value::<EmptyParams>(), schema_value::<HostInfo>()),
            KernelMethod::HostPing => (
                schema_value::<EmptyParams>(),
                json!({"type":"object","required":["ok"],"properties":{"ok":{"const":true}}}),
            ),
            KernelMethod::HostDiagnostics => {
                (schema_value::<EmptyParams>(), json!({"type":"object"}))
            }
            KernelMethod::PermissionGrant => (
                schema_value::<PermissionGrantParams>(),
                schema_value::<PermissionGrantRecord>(),
            ),
            KernelMethod::PermissionRevoke => (
                schema_value::<PermissionRevokeParams>(),
                schema_value::<PermissionGrantRecord>(),
            ),
            KernelMethod::PermissionList => (
                schema_value::<PermissionListParams>(),
                json!({"type":"array","items":schema_value::<PermissionGrantRecord>()}),
            ),
            KernelMethod::PermissionAudit => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<EventEnvelope>()}),
            ),
            KernelMethod::AuditPackage => (
                schema_value::<defs::AuditPackageParams>(),
                schema_value::<PackageAuditReport>(),
            ),
            KernelMethod::ProposalCreate => (
                schema_value::<ProposalRecord>(),
                schema_value::<ProposalRecord>(),
            ),
            KernelMethod::ProposalGet | KernelMethod::ProposalApply => (
                schema_value::<ProposalIdParams>(),
                schema_value::<ProposalRecord>(),
            ),
            KernelMethod::ProposalList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<ProposalRecord>()}),
            ),
            KernelMethod::ProposalApprove | KernelMethod::ProposalReject => (
                schema_value::<ProposalDecisionParams>(),
                schema_value::<ProposalRecord>(),
            ),
            KernelMethod::SurfaceContributionList => {
                (schema_value::<SurfaceListParams>(), json!({"type":"array"}))
            }
            KernelMethod::SurfaceResolveBundle => (
                schema_value::<SurfaceResolveBundleParams>(),
                schema_value::<SurfaceResolveBundleResult>(),
            ),
            KernelMethod::SurfaceContributionDescribe => (
                schema_value::<SurfaceDescribeParams>(),
                json!({"type":"object"}),
            ),
            KernelMethod::OutboundAudit => (
                schema_value::<OutboundAuditParams>(),
                json!({"type":"array","items":schema_value::<OutboundAuditRecord>()}),
            ),
            KernelMethod::OutboundExecute => (
                schema_value::<OutboundExecuteParams>(),
                schema_value::<OutboundExecutorResponse>(),
            ),
            KernelMethod::OutboundStream => (
                schema_value::<OutboundStreamParams>(),
                schema_value::<KernelOutboundStreamResponse>(),
            ),
            KernelMethod::OutboundWebSocketOpen => (
                schema_value::<OutboundWebSocketOpenRequest>(),
                json!({"type":"object"}),
            ),
            KernelMethod::OutboundWebSocketSend | KernelMethod::OutboundWebSocketClose => (
                schema_value::<OutboundWebSocketSendParams>(),
                json!({"type":"object"}),
            ),
            };
            (method, params, result)
        })
        .collect()
}
