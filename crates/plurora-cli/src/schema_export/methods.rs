use plurora_core::*;
use plurora_runtime::*;
use serde_json::{json, Value};

use super::defs;
use super::defs::*;
use super::SCHEMA;

pub(crate) fn method_schema(method: PlatformMethod, params: Value, result: Value) -> Value {
    json!({
        "$schema": SCHEMA,
        "$id": format!("urn:plurora:schema:method:{}:v1", method.id()),
        "title": method.id(),
        "description": format!("Plurora public platform method {}.", method.id()),
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

pub(crate) fn method_schemas() -> Vec<(PlatformMethod, Value, Value)> {
    PlatformMethod::all()
        .iter()
        .map(|method| {
            let method = *method;
            let (params, result) = match method {
                PlatformMethod::SessionOpen => (
                    schema_value::<OpenSessionRequest>(),
                    schema_value::<SessionRecord>(),
                ),
            PlatformMethod::SessionClose => (
                schema_value::<SessionCloseParams>(),
                schema_value::<EventEnvelope>(),
            ),
            PlatformMethod::SessionGet => (
                schema_value::<SessionGetParams>(),
                schema_value::<SessionRecord>(),
            ),
            PlatformMethod::SessionFork => (
                schema_value::<SessionForkParams>(),
                schema_value::<BranchRecord>(),
            ),
            PlatformMethod::SessionBranchList => (
                schema_value::<SessionBranchListParams>(),
                json!({"type":"array","items":schema_value::<BranchRecord>()}),
            ),
            PlatformMethod::SessionList
            | PlatformMethod::EventSubscribe
            | PlatformMethod::PackageDescribe
            | PlatformMethod::CapabilityDescribe
            | PlatformMethod::ExtensionPointDescribe
            | PlatformMethod::HostPrincipal => {
                (schema_value::<EmptyParams>(), json!({"type":"null"}))
            }
            PlatformMethod::EventAppend => (
                schema_value::<AppendEventRequest>(),
                schema_value::<EventEnvelope>(),
            ),
            PlatformMethod::EventList => (
                schema_value::<EventListRequest>(),
                json!({"type":"array","items":schema_value::<EventEnvelope>()}),
            ),
            PlatformMethod::PackageLoad => (
                schema_value::<PackageManifest>(),
                schema_value::<PackageRecord>(),
            ),
            PlatformMethod::PackageUnload
            | PlatformMethod::PackageRestart
            | PlatformMethod::PackageLogs
            | PlatformMethod::PackageStatus => {
                let result = if method == PlatformMethod::PackageLogs {
                    json!({"type":"array","items":schema_value::<SubprocessLogLine>()})
                } else {
                    schema_value::<PackageRecord>()
                };
                (schema_value::<PackageIdParams>(), result)
            }
            PlatformMethod::PackageList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<PackageRecord>()}),
            ),
            PlatformMethod::ProjectList => (
                schema_value::<ProjectListParams>(),
                schema_value::<ProjectListResultSchema>(),
            ),
            PlatformMethod::ProjectGet => (
                schema_value::<ProjectIdParams>(),
                json!({"allOf":[schema_value::<plurora_core::project::ProjectDescriptor>()],"properties":{"state":schema_value::<plurora_core::project::ProjectState>(),"storage_summary":schema_value::<ProjectStorageSummarySchema>(),"running_session_id":{"type":"string","description":"Session id when project state is running; absent otherwise"}}}),
            ),
            PlatformMethod::ProjectStart => (
                schema_value::<ProjectIdParams>(),
                schema_value::<ProjectStartResult>(),
            ),
            PlatformMethod::ProjectStop => (
                schema_value::<ProjectIdParams>(),
                schema_value::<ProjectStopResult>(),
            ),
            PlatformMethod::ProjectStatus => (
                schema_value::<ProjectIdParams>(),
                schema_value::<ProjectStatusResult>(),
            ),
            PlatformMethod::TargetList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<ExecutionTarget>()}),
            ),
            PlatformMethod::TargetStatus => (
                schema_value::<TargetIdParams>(),
                schema_value::<ExecutionTarget>(),
            ),
            PlatformMethod::TargetRegister => (
                schema_value::<ExecutionTarget>(),
                schema_value::<ExecutionTarget>(),
            ),
            PlatformMethod::TargetUnregister => (
                schema_value::<TargetIdParams>(),
                schema_value::<ExecutionTarget>(),
            ),
            PlatformMethod::ExecStart => (
                schema_value::<LocalExecStartRequest>(),
                schema_value::<LocalExecStartResponse>(),
            ),
            PlatformMethod::ExecStop => (
                schema_value::<LocalExecStopRequest>(),
                schema_value::<LocalExecStopResponse>(),
            ),
            PlatformMethod::ExecStatus => (
                schema_value::<ExecIdParams>(),
                schema_value::<LocalExecStatusResponse>(),
            ),
            PlatformMethod::ExecLogs => (
                schema_value::<LocalExecLogsRequest>(),
                schema_value::<LocalExecLogsResponse>(),
            ),
            PlatformMethod::ExecList => (
                schema_value::<EmptyParams>(),
                schema_value::<LocalExecListResponse>(),
            ),
            PlatformMethod::PortLease => (
                schema_value::<PortLeaseRequest>(),
                schema_value::<PortLeaseResponse>(),
            ),
            PlatformMethod::PortRelease | PlatformMethod::PortStatus => (
                schema_value::<PortLeaseIdParams>(),
                schema_value::<PortLeaseRecord>(),
            ),
            PlatformMethod::PortList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<PortLeaseRecord>()}),
            ),
            PlatformMethod::ProxyRegister => (
                schema_value::<ProxyRouteRegisterRequest>(),
                schema_value::<ProxyRouteRegisterResponse>(),
            ),
            PlatformMethod::ProxyUnregister | PlatformMethod::ProxyStatus => (
                schema_value::<ProxyRouteIdParams>(),
                schema_value::<ProxyRouteRecord>(),
            ),
            PlatformMethod::ProxyList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<ProxyRouteRecord>()}),
            ),
            PlatformMethod::CapabilityDiscover => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<RegisteredCapability>()}),
            ),
            PlatformMethod::CapabilityInvoke => (
                schema_value::<CapabilityInvocationRequest>(),
                schema_value::<CapabilityInvocationResult>(),
            ),
            PlatformMethod::CapabilityHandleAttenuate => (
                schema_value::<CapAttenuateParams>(),
                schema_value::<CapHandleResult>(),
            ),
            PlatformMethod::CapabilityHandleRevoke => {
                (schema_value::<CapRevokeParams>(), json!({"type":"object"}))
            }
            PlatformMethod::CapabilityHandleListFor => (
                schema_value::<CapListForParams>(),
                schema_value::<CapHandlesResult>(),
            ),
            PlatformMethod::CapabilityStream => (
                schema_value::<CapabilityStreamParams>(),
                json!({"type":"object"}),
            ),
            PlatformMethod::CapabilityCancel => (
                schema_value::<CapabilityCancelParams>(),
                schema_value::<StreamFrameEnvelope>(),
            ),
            PlatformMethod::ExtensionPointList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":{"type":"string"}}),
            ),
            PlatformMethod::HookList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<RegisteredHook>()}),
            ),
            PlatformMethod::AssetPut => (
                schema_value::<AssetPutRequest>(),
                schema_value::<AssetRecord>(),
            ),
            PlatformMethod::AssetGet => (
                schema_value::<AssetGetParams>(),
                schema_value::<AssetGetResponse>(),
            ),
            PlatformMethod::AssetList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<AssetRecord>()}),
            ),
            PlatformMethod::ProjectionRegister => (
                schema_value::<ProjectionDefinition>(),
                schema_value::<ProjectionDefinition>(),
            ),
            PlatformMethod::ProjectionRebuild | PlatformMethod::ProjectionGet => (
                schema_value::<ProjectionIdParams>(),
                schema_value::<ProjectionDefinition>(),
            ),
            PlatformMethod::ProjectionList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<ProjectionDefinition>()}),
            ),
            PlatformMethod::HostInfo => (schema_value::<EmptyParams>(), schema_value::<HostInfo>()),
            PlatformMethod::HostPing => (
                schema_value::<EmptyParams>(),
                json!({"type":"object","required":["ok"],"properties":{"ok":{"const":true}}}),
            ),
            PlatformMethod::HostDiagnostics => {
                (schema_value::<EmptyParams>(), json!({"type":"object"}))
            }
            PlatformMethod::PermissionGrant => (
                schema_value::<PermissionGrantParams>(),
                schema_value::<PermissionGrantRecord>(),
            ),
            PlatformMethod::PermissionRevoke => (
                schema_value::<PermissionRevokeParams>(),
                schema_value::<PermissionGrantRecord>(),
            ),
            PlatformMethod::PermissionList => (
                schema_value::<PermissionListParams>(),
                json!({"type":"array","items":schema_value::<PermissionGrantRecord>()}),
            ),
            PlatformMethod::PermissionAudit => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<EventEnvelope>()}),
            ),
            PlatformMethod::AuditPackage => (
                schema_value::<defs::AuditPackageParams>(),
                schema_value::<PackageAuditReport>(),
            ),
            PlatformMethod::ProposalCreate => (
                schema_value::<ProposalRecord>(),
                schema_value::<ProposalRecord>(),
            ),
            PlatformMethod::ProposalGet | PlatformMethod::ProposalApply => (
                schema_value::<ProposalIdParams>(),
                schema_value::<ProposalRecord>(),
            ),
            PlatformMethod::ProposalList => (
                schema_value::<EmptyParams>(),
                json!({"type":"array","items":schema_value::<ProposalRecord>()}),
            ),
            PlatformMethod::ProposalApprove | PlatformMethod::ProposalReject => (
                schema_value::<ProposalDecisionParams>(),
                schema_value::<ProposalRecord>(),
            ),
            PlatformMethod::SurfaceContributionList => {
                (schema_value::<SurfaceListParams>(), json!({"type":"array"}))
            }
            PlatformMethod::SurfaceResolveBundle => (
                schema_value::<SurfaceResolveBundleParams>(),
                schema_value::<SurfaceResolveBundleResult>(),
            ),
            PlatformMethod::SurfaceContributionDescribe => (
                schema_value::<SurfaceDescribeParams>(),
                json!({"type":"object"}),
            ),
            PlatformMethod::OutboundAudit => (
                schema_value::<OutboundAuditParams>(),
                json!({"type":"array","items":schema_value::<OutboundAuditRecord>()}),
            ),
            PlatformMethod::OutboundExecute => (
                schema_value::<OutboundExecuteParams>(),
                schema_value::<OutboundExecutorResponse>(),
            ),
            PlatformMethod::OutboundStream => (
                schema_value::<OutboundStreamParams>(),
                schema_value::<OutboundStreamResponse>(),
            ),
            PlatformMethod::OutboundWebSocketOpen => (
                schema_value::<OutboundWebSocketOpenRequest>(),
                json!({"type":"object"}),
            ),
            PlatformMethod::OutboundWebSocketSend | PlatformMethod::OutboundWebSocketClose => (
                schema_value::<OutboundWebSocketSendParams>(),
                json!({"type":"object"}),
            ),
            };
            (method, params, result)
        })
        .collect()
}
