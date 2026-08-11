use std::fmt;
use std::str::FromStr;

use plurora_core::ProtocolDescriptor;
use schemars::{
    gen::SchemaGenerator,
    schema::{InstanceType, Metadata, Schema, SchemaObject, SingleOrVec},
    JsonSchema,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::installation_control::InstallationAuthorityRefresh;
use crate::run_control::RunAuthorityRefresh;
use crate::{
    contract_layers, contract_methods, contract_profiles, contract_versions, protocol_descriptors,
    resolve_contract_method, ContractLayerInfo, ContractMaturity, ContractMethod,
    ContractProfileInfo, ContractSelection, ContractVersionInfo, CONTRACT_REGISTRY_VERSION,
    DEFAULT_CONTRACT_PROFILE, PROTOCOL_COMMONS_REGISTRY_VERSION,
};
use crate::{PowerboxAuthorityRefresh, RealizationAuthorityRefresh};

// ---------------------------------------------------------------------------
// PlatformMethod is the single source of truth for public handler identity,
// implementation status, and streaming behavior. There is exactly one wire ID
// for each variant; the pre-release contract exposes no aliases.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformMethod {
    SessionOpen,
    SessionClose,
    SessionFork,
    SessionBranchList,
    SessionGet,
    SessionList,
    EventAppend,
    EventList,
    EventSubscribe,
    PackageLoad,
    PackageUnload,
    PackageRestart,
    PackageLogs,
    PackageList,
    PackageStatus,
    PackageDescribe,
    InstallationList,
    InstallationGet,
    InstallationCreate,
    InstallationUpdate,
    InstallationRemove,
    ExposureList,
    ExposureCreate,
    ExposureRevoke,
    BindingList,
    BindingCandidates,
    BindingSelect,
    BindingRevoke,
    RunList,
    RunGet,
    RunStart,
    RunStop,
    RunStatus,
    RealizationPlan,
    RealizationApply,
    RealizationGet,
    RealizationList,
    RealizationStop,
    RealizationRollback,
    RealizationReconcile,
    TargetList,
    TargetStatus,
    TargetRegister,
    TargetUnregister,
    ExecStart,
    ExecStop,
    ExecStatus,
    ExecLogs,
    ExecList,
    PortLease,
    PortRelease,
    PortStatus,
    PortList,
    ProxyRegister,
    ProxyUnregister,
    ProxyStatus,
    ProxyList,
    CapabilityDiscover,
    CapabilityDescribe,
    CapabilityInvoke,
    CapabilityHandleAttenuate,
    CapabilityHandleRevoke,
    CapabilityHandleListFor,
    CapabilityStream,
    CapabilityCancel,
    ExtensionPointList,
    ExtensionPointDescribe,
    HookList,
    AssetPut,
    AssetGet,
    AssetList,
    ProjectionRegister,
    ProjectionRebuild,
    ProjectionGet,
    ProjectionList,
    HostInfo,
    HostPing,
    HostDiagnostics,
    HostPrincipal,
    PermissionGrant,
    PermissionRevoke,
    PermissionList,
    PermissionAudit,
    AuditPackage,
    ProposalCreate,
    ProposalGet,
    ProposalList,
    ProposalApprove,
    ProposalReject,
    ProposalApply,
    SurfaceResolveBundle,
    SurfaceContributionList,
    SurfaceContributionDescribe,
    OutboundAudit,
    OutboundExecute,
    OutboundStream,
    OutboundWebSocketOpen,
    OutboundWebSocketSend,
    OutboundWebSocketClose,
}

impl PlatformMethod {
    /// The only public wire identifier for this method.
    pub const fn id(&self) -> &'static str {
        match self {
            Self::SessionOpen => "context.open",
            Self::SessionClose => "context.close",
            Self::SessionFork => "context.fork",
            Self::SessionBranchList => "context.branch.list",
            Self::SessionGet => "context.get",
            Self::SessionList => "context.list",
            Self::EventAppend => "journal.append",
            Self::EventList => "journal.list",
            Self::EventSubscribe => "journal.subscribe",
            Self::PackageLoad => "host.package.load",
            Self::PackageUnload => "host.package.unload",
            Self::PackageRestart => "host.package.restart",
            Self::PackageLogs => "host.package.logs",
            Self::PackageList => "host.package.list",
            Self::PackageStatus => "host.package.status",
            Self::PackageDescribe => "host.package.describe",
            Self::InstallationList => "host.installation.list",
            Self::InstallationGet => "host.installation.get",
            Self::InstallationCreate => "host.installation.create",
            Self::InstallationUpdate => "host.installation.update",
            Self::InstallationRemove => "host.installation.remove",
            Self::ExposureList => "host.exposure.list",
            Self::ExposureCreate => "host.exposure.create",
            Self::ExposureRevoke => "host.exposure.revoke",
            Self::BindingList => "host.binding.list",
            Self::BindingCandidates => "host.binding.candidates",
            Self::BindingSelect => "host.binding.select",
            Self::BindingRevoke => "host.binding.revoke",
            Self::RunList => "host.run.list",
            Self::RunGet => "host.run.get",
            Self::RunStart => "host.run.start",
            Self::RunStop => "host.run.stop",
            Self::RunStatus => "host.run.status",
            Self::RealizationPlan => "host.realization.plan",
            Self::RealizationApply => "host.realization.apply",
            Self::RealizationGet => "host.realization.get",
            Self::RealizationList => "host.realization.list",
            Self::RealizationStop => "host.realization.stop",
            Self::RealizationRollback => "host.realization.rollback",
            Self::RealizationReconcile => "host.realization.reconcile",
            Self::TargetList => "host.target.list",
            Self::TargetStatus => "host.target.status",
            Self::TargetRegister => "host.target.register",
            Self::TargetUnregister => "host.target.unregister",
            Self::ExecStart => "host.exec.start",
            Self::ExecStop => "host.exec.stop",
            Self::ExecStatus => "host.exec.status",
            Self::ExecLogs => "host.exec.logs",
            Self::ExecList => "host.exec.list",
            Self::PortLease => "host.port.lease",
            Self::PortRelease => "host.port.release",
            Self::PortStatus => "host.port.status",
            Self::PortList => "host.port.list",
            Self::ProxyRegister => "host.proxy.register",
            Self::ProxyUnregister => "host.proxy.unregister",
            Self::ProxyStatus => "host.proxy.status",
            Self::ProxyList => "host.proxy.list",
            Self::CapabilityDiscover => "capability.discover",
            Self::CapabilityDescribe => "capability.describe",
            Self::CapabilityInvoke => "capability.invoke",
            Self::CapabilityHandleAttenuate => "authority.handle.attenuate",
            Self::CapabilityHandleRevoke => "authority.handle.revoke",
            Self::CapabilityHandleListFor => "authority.handle.list",
            Self::CapabilityStream => "capability.stream",
            Self::CapabilityCancel => "capability.cancel",
            Self::ExtensionPointList => "protocol.extension.list",
            Self::ExtensionPointDescribe => "protocol.extension.describe",
            Self::HookList => "protocol.hook.list",
            Self::AssetPut => "object.put",
            Self::AssetGet => "object.get",
            Self::AssetList => "object.list",
            Self::ProjectionRegister => "projection.register",
            Self::ProjectionRebuild => "projection.rebuild",
            Self::ProjectionGet => "projection.get",
            Self::ProjectionList => "projection.list",
            Self::HostInfo => "host.info",
            Self::HostPing => "host.ping",
            Self::HostDiagnostics => "host.diagnostics",
            Self::HostPrincipal => "identity.current",
            Self::PermissionGrant => "authority.grant.create",
            Self::PermissionRevoke => "authority.grant.revoke",
            Self::PermissionList => "authority.grant.list",
            Self::PermissionAudit => "authority.decision.list",
            Self::AuditPackage => "host.package.audit",
            Self::ProposalCreate => "change.proposal.create",
            Self::ProposalGet => "change.proposal.get",
            Self::ProposalList => "change.proposal.list",
            Self::ProposalApprove => "change.proposal.approve",
            Self::ProposalReject => "change.proposal.reject",
            Self::ProposalApply => "change.proposal.apply",
            Self::SurfaceResolveBundle => "host.surface.bundle.resolve",
            Self::SurfaceContributionList => "shell.contribution.list",
            Self::SurfaceContributionDescribe => "shell.contribution.describe",
            Self::OutboundAudit => "host.outbound.audit",
            Self::OutboundExecute => "host.outbound.execute",
            Self::OutboundStream => "host.outbound.stream",
            Self::OutboundWebSocketOpen => "host.outbound.websocket.open",
            Self::OutboundWebSocketSend => "host.outbound.websocket.send",
            Self::OutboundWebSocketClose => "host.outbound.websocket.close",
        }
    }

    /// Protocol implementation status for this method.
    pub const fn status(&self) -> MethodStatus {
        match self {
            Self::SessionOpen => MethodStatus::Implemented,
            Self::SessionClose => MethodStatus::Implemented,
            Self::SessionFork => MethodStatus::Partial,
            Self::SessionBranchList => MethodStatus::Partial,
            Self::SessionGet => MethodStatus::Partial,
            Self::SessionList => MethodStatus::Planned,
            Self::EventAppend => MethodStatus::Implemented,
            Self::EventList => MethodStatus::Partial,
            Self::EventSubscribe => MethodStatus::Planned,
            Self::PackageLoad => MethodStatus::Partial,
            Self::PackageUnload => MethodStatus::Partial,
            Self::PackageRestart => MethodStatus::Partial,
            Self::PackageLogs => MethodStatus::Partial,
            Self::PackageList => MethodStatus::Implemented,
            Self::PackageStatus => MethodStatus::Implemented,
            Self::PackageDescribe => MethodStatus::Planned,
            Self::InstallationList => MethodStatus::Implemented,
            Self::InstallationGet => MethodStatus::Implemented,
            Self::InstallationCreate => MethodStatus::Implemented,
            Self::InstallationUpdate => MethodStatus::Implemented,
            Self::InstallationRemove => MethodStatus::Implemented,
            Self::ExposureList
            | Self::ExposureCreate
            | Self::ExposureRevoke
            | Self::BindingList
            | Self::BindingCandidates
            | Self::BindingSelect
            | Self::BindingRevoke => MethodStatus::Implemented,
            Self::RunList | Self::RunGet | Self::RunStart | Self::RunStop | Self::RunStatus => {
                MethodStatus::Implemented
            }
            Self::RealizationPlan
            | Self::RealizationApply
            | Self::RealizationGet
            | Self::RealizationList
            | Self::RealizationStop
            | Self::RealizationRollback
            | Self::RealizationReconcile => MethodStatus::Implemented,
            Self::TargetList
            | Self::TargetStatus
            | Self::TargetRegister
            | Self::TargetUnregister
            | Self::ExecStart
            | Self::ExecStop
            | Self::ExecStatus
            | Self::ExecLogs
            | Self::ExecList
            | Self::PortLease
            | Self::PortRelease
            | Self::PortStatus
            | Self::PortList
            | Self::ProxyRegister
            | Self::ProxyUnregister
            | Self::ProxyStatus
            | Self::ProxyList => MethodStatus::Partial,
            Self::CapabilityDiscover => MethodStatus::Implemented,
            Self::CapabilityDescribe => MethodStatus::Planned,
            Self::CapabilityInvoke => MethodStatus::Partial,
            Self::CapabilityHandleAttenuate => MethodStatus::Partial,
            Self::CapabilityHandleRevoke => MethodStatus::Partial,
            Self::CapabilityHandleListFor => MethodStatus::Partial,
            Self::CapabilityStream => MethodStatus::Partial,
            Self::CapabilityCancel => MethodStatus::Partial,
            Self::ExtensionPointList => MethodStatus::Implemented,
            Self::ExtensionPointDescribe => MethodStatus::Planned,
            Self::HookList => MethodStatus::Partial,
            Self::AssetPut => MethodStatus::Partial,
            Self::AssetGet => MethodStatus::Partial,
            Self::AssetList => MethodStatus::Partial,
            Self::ProjectionRegister => MethodStatus::Partial,
            Self::ProjectionRebuild => MethodStatus::Partial,
            Self::ProjectionGet => MethodStatus::Partial,
            Self::ProjectionList => MethodStatus::Partial,
            Self::HostInfo => MethodStatus::Implemented,
            Self::HostPing => MethodStatus::Partial,
            Self::HostDiagnostics => MethodStatus::Partial,
            Self::HostPrincipal => MethodStatus::Planned,
            Self::PermissionGrant => MethodStatus::Partial,
            Self::PermissionRevoke => MethodStatus::Partial,
            Self::PermissionList => MethodStatus::Partial,
            Self::PermissionAudit => MethodStatus::Partial,
            Self::AuditPackage => MethodStatus::Partial,
            Self::ProposalCreate => MethodStatus::Partial,
            Self::ProposalGet => MethodStatus::Partial,
            Self::ProposalList => MethodStatus::Partial,
            Self::ProposalApprove => MethodStatus::Partial,
            Self::ProposalReject => MethodStatus::Partial,
            Self::ProposalApply => MethodStatus::Partial,
            Self::SurfaceResolveBundle => MethodStatus::Partial,
            Self::SurfaceContributionList => MethodStatus::Partial,
            Self::SurfaceContributionDescribe => MethodStatus::Partial,
            Self::OutboundAudit => MethodStatus::Partial,
            Self::OutboundExecute => MethodStatus::Partial,
            Self::OutboundStream => MethodStatus::Partial,
            Self::OutboundWebSocketOpen => MethodStatus::Partial,
            Self::OutboundWebSocketSend => MethodStatus::Partial,
            Self::OutboundWebSocketClose => MethodStatus::Partial,
        }
    }

    /// Whether this method returns a streaming response.
    pub const fn streaming(&self) -> bool {
        match self {
            Self::EventSubscribe
            | Self::CapabilityStream
            | Self::OutboundStream
            | Self::OutboundWebSocketOpen => true,
            _ => false,
        }
    }

    /// All known platform methods in registry order.
    pub const fn all() -> &'static [PlatformMethod] {
        &[
            Self::SessionOpen,
            Self::SessionClose,
            Self::SessionFork,
            Self::SessionBranchList,
            Self::SessionGet,
            Self::SessionList,
            Self::EventAppend,
            Self::EventList,
            Self::EventSubscribe,
            Self::PackageLoad,
            Self::PackageUnload,
            Self::PackageRestart,
            Self::PackageLogs,
            Self::PackageList,
            Self::PackageStatus,
            Self::PackageDescribe,
            Self::InstallationList,
            Self::InstallationGet,
            Self::InstallationCreate,
            Self::InstallationUpdate,
            Self::InstallationRemove,
            Self::ExposureList,
            Self::ExposureCreate,
            Self::ExposureRevoke,
            Self::BindingList,
            Self::BindingCandidates,
            Self::BindingSelect,
            Self::BindingRevoke,
            Self::RunList,
            Self::RunGet,
            Self::RunStart,
            Self::RunStop,
            Self::RunStatus,
            Self::RealizationPlan,
            Self::RealizationApply,
            Self::RealizationGet,
            Self::RealizationList,
            Self::RealizationStop,
            Self::RealizationRollback,
            Self::RealizationReconcile,
            Self::TargetList,
            Self::TargetStatus,
            Self::TargetRegister,
            Self::TargetUnregister,
            Self::ExecStart,
            Self::ExecStop,
            Self::ExecStatus,
            Self::ExecLogs,
            Self::ExecList,
            Self::PortLease,
            Self::PortRelease,
            Self::PortStatus,
            Self::PortList,
            Self::ProxyRegister,
            Self::ProxyUnregister,
            Self::ProxyStatus,
            Self::ProxyList,
            Self::CapabilityDiscover,
            Self::CapabilityDescribe,
            Self::CapabilityInvoke,
            Self::CapabilityHandleAttenuate,
            Self::CapabilityHandleRevoke,
            Self::CapabilityHandleListFor,
            Self::CapabilityStream,
            Self::CapabilityCancel,
            Self::ExtensionPointList,
            Self::ExtensionPointDescribe,
            Self::HookList,
            Self::AssetPut,
            Self::AssetGet,
            Self::AssetList,
            Self::ProjectionRegister,
            Self::ProjectionRebuild,
            Self::ProjectionGet,
            Self::ProjectionList,
            Self::HostInfo,
            Self::HostPing,
            Self::HostDiagnostics,
            Self::HostPrincipal,
            Self::PermissionGrant,
            Self::PermissionRevoke,
            Self::PermissionList,
            Self::PermissionAudit,
            Self::AuditPackage,
            Self::ProposalCreate,
            Self::ProposalGet,
            Self::ProposalList,
            Self::ProposalApprove,
            Self::ProposalReject,
            Self::ProposalApply,
            Self::SurfaceResolveBundle,
            Self::SurfaceContributionList,
            Self::SurfaceContributionDescribe,
            Self::OutboundAudit,
            Self::OutboundExecute,
            Self::OutboundStream,
            Self::OutboundWebSocketOpen,
            Self::OutboundWebSocketSend,
            Self::OutboundWebSocketClose,
        ]
    }

    /// Convert to the serialisable descriptor used in the public registry.
    pub fn to_protocol_method(&self) -> ProtocolMethod {
        ProtocolMethod {
            id: self.id(),
            streaming: self.streaming(),
            status: self.status(),
        }
    }

    /// Whether this method has a dispatch branch in the runtime
    /// (`dispatch_protocol_method`). Kept in sync with the dispatch match in
    /// `runtime.rs` — update both sides together.
    pub const fn is_dispatched(&self) -> bool {
        match self {
            // Implemented or Partial methods that have a dispatch arm
            Self::SessionOpen
            | Self::SessionClose
            | Self::SessionFork
            | Self::SessionBranchList
            | Self::SessionGet
            | Self::EventAppend
            | Self::EventList
            | Self::PackageLoad
            | Self::PackageUnload
            | Self::PackageRestart
            | Self::PackageLogs
            | Self::PackageList
            | Self::PackageStatus
            | Self::InstallationList
            | Self::InstallationGet
            | Self::InstallationCreate
            | Self::InstallationUpdate
            | Self::InstallationRemove
            | Self::ExposureList
            | Self::ExposureCreate
            | Self::ExposureRevoke
            | Self::BindingList
            | Self::BindingCandidates
            | Self::BindingSelect
            | Self::BindingRevoke
            | Self::RunList
            | Self::RunGet
            | Self::RunStart
            | Self::RunStop
            | Self::RunStatus
            | Self::RealizationPlan
            | Self::RealizationApply
            | Self::RealizationGet
            | Self::RealizationList
            | Self::RealizationStop
            | Self::RealizationRollback
            | Self::RealizationReconcile
            | Self::TargetList
            | Self::TargetStatus
            | Self::TargetRegister
            | Self::TargetUnregister
            | Self::ExecStart
            | Self::ExecStop
            | Self::ExecStatus
            | Self::ExecLogs
            | Self::ExecList
            | Self::PortLease
            | Self::PortRelease
            | Self::PortStatus
            | Self::PortList
            | Self::ProxyRegister
            | Self::ProxyUnregister
            | Self::ProxyStatus
            | Self::ProxyList
            | Self::CapabilityDiscover
            | Self::CapabilityInvoke
            | Self::CapabilityHandleAttenuate
            | Self::CapabilityHandleRevoke
            | Self::CapabilityHandleListFor
            | Self::CapabilityStream
            | Self::CapabilityCancel
            | Self::ExtensionPointList
            | Self::HookList
            | Self::AssetPut
            | Self::AssetGet
            | Self::AssetList
            | Self::ProjectionRegister
            | Self::ProjectionRebuild
            | Self::ProjectionGet
            | Self::ProjectionList
            | Self::HostInfo
            | Self::HostPing
            | Self::HostDiagnostics
            | Self::PermissionGrant
            | Self::PermissionRevoke
            | Self::PermissionList
            | Self::PermissionAudit
            | Self::AuditPackage
            | Self::ProposalCreate
            | Self::ProposalGet
            | Self::ProposalList
            | Self::ProposalApprove
            | Self::ProposalReject
            | Self::ProposalApply
            | Self::SurfaceResolveBundle
            | Self::SurfaceContributionList
            | Self::SurfaceContributionDescribe
            | Self::OutboundAudit
            | Self::OutboundExecute
            | Self::OutboundStream
            | Self::OutboundWebSocketOpen
            | Self::OutboundWebSocketSend
            | Self::OutboundWebSocketClose => true,
            // Planned methods with no dispatch yet
            Self::SessionList
            | Self::EventSubscribe
            | Self::PackageDescribe
            | Self::CapabilityDescribe
            | Self::ExtensionPointDescribe
            | Self::HostPrincipal => false,
        }
    }
}

impl fmt::Display for PlatformMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

impl FromStr for PlatformMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        resolve_contract_method(s)
            .map(|resolved| resolved.method)
            .map_err(|error| error.to_string())
    }
}

// ---------------------------------------------------------------------------
// Public protocol types (API-compatible)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolMethod {
    pub id: &'static str,
    pub streaming: bool,
    pub status: MethodStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MethodStatus {
    Implemented,
    Partial,
    Planned,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProtocolPrincipal {
    HostAdmin,
    HostDev,
    Package {
        package_id: String,
    },
    Human {
        user_id: String,
    },
    Assistant {
        assistant_id: String,
        delegated_user_id: Option<String>,
    },
    Anonymous,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolResourceSelector {
    pub owner: String,
    pub kind: String,
    /// An explicit null selects every resource of this owner/kind. Omitting the
    /// field is invalid. Resource matching is structural and exact; callers
    /// must never use string-prefix matching for authority decisions.
    #[schemars(required)]
    pub id: Option<String>,
}

#[derive(Default)]
enum ExplicitNullableResourceId {
    #[default]
    Missing,
    Present(Option<String>),
}

impl<'de> Deserialize<'de> for ExplicitNullableResourceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer).map(Self::Present)
    }
}

impl<'de> Deserialize<'de> for ProtocolResourceSelector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireSelector {
            owner: String,
            kind: String,
            #[serde(default)]
            id: ExplicitNullableResourceId,
        }

        let selector = WireSelector::deserialize(deserializer)?;
        let id = match selector.id {
            ExplicitNullableResourceId::Present(id) => id,
            ExplicitNullableResourceId::Missing => {
                return Err(serde::de::Error::missing_field("id"));
            }
        };
        Ok(Self {
            owner: selector.owner,
            kind: selector.kind,
            id,
        })
    }
}

impl ProtocolResourceSelector {
    pub fn matches(&self, owner: &str, kind: &str, id: &str) -> bool {
        self.owner == owner
            && self.kind == kind
            && self.id.as_deref().is_none_or(|selected| selected == id)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolAuthorityContext {
    pub grant_id: String,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub resources: Vec<ProtocolResourceSelector>,
    #[serde(default)]
    pub delegation_chain: Vec<String>,
    /// Trusted transport fact. It is never serialized or accepted from wire
    /// JSON, and is used only to mint a short-lived internal mutation sidecar.
    #[serde(skip)]
    #[schemars(skip)]
    verified_expires_at_ms: Option<i64>,
    /// Trusted service callback used to refresh a device grant immediately
    /// before a destructive Installation state effect. Never accepted on wire.
    #[serde(skip)]
    #[schemars(skip)]
    authority_refresh: Option<InstallationAuthorityRefresh>,
    /// Trusted service callback used to refresh an exact parent Installation
    /// grant before each Run journal or activation effect boundary.
    #[serde(skip)]
    #[schemars(skip)]
    run_authority_refresh: Option<RunAuthorityRefresh>,
    /// Trusted service callback used to refresh exact Exposure/Binding
    /// mutation authority immediately before each durable append or effect.
    #[serde(skip)]
    #[schemars(skip)]
    powerbox_authority_refresh: Option<PowerboxAuthorityRefresh>,
    /// Trusted service callback used to refresh exact Managed Realization
    /// authority immediately before each durable append or external effect.
    #[serde(skip)]
    #[schemars(skip)]
    realization_authority_refresh: Option<RealizationAuthorityRefresh>,
}

/// Request-specific Host operation facts established by a trusted transport
/// adapter after it has parsed and authorized the request. Unlike authority,
/// this is never populated from the request body itself.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolHostOperationContext {
    pub action: String,
    #[serde(default)]
    pub resources: Vec<ProtocolResourceSelector>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolContext {
    pub principal: ProtocolPrincipal,
    pub transport: String,
    /// Authenticated Host authority carried by transport adapters. This is
    /// additive so older callers remain source- and wire-compatible. Device
    /// calls use the existing anonymous V1 principal as a fail-closed sentinel:
    /// old runtimes ignore this field and deny the call, while new runtimes
    /// require the scoped authority below.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority: Option<ProtocolAuthorityContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_operation: Option<ProtocolHostOperationContext>,
    /// Optional kernel session id this call is operating under.
    /// Used by outbound dispatch to scope secret resolution to the
    /// session's Installation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    #[schemars(schema_with = "optional_uuid_schema")]
    pub correlation_id: Option<Uuid>,
    #[serde(default)]
    #[schemars(schema_with = "optional_uuid_schema")]
    pub parent_invocation_id: Option<Uuid>,
}

impl ProtocolContext {
    pub fn host_dev(transport: impl Into<String>) -> Self {
        Self {
            principal: ProtocolPrincipal::HostDev,
            transport: transport.into(),
            authority: None,
            host_operation: None,
            session_id: None,
            correlation_id: Some(Uuid::new_v4()),
            parent_invocation_id: None,
        }
    }

    pub fn host_admin(transport: impl Into<String>) -> Self {
        Self {
            principal: ProtocolPrincipal::HostAdmin,
            transport: transport.into(),
            authority: None,
            host_operation: None,
            session_id: None,
            correlation_id: Some(Uuid::new_v4()),
            parent_invocation_id: None,
        }
    }

    pub fn host_device(
        grant_id: impl Into<String>,
        actions: Vec<String>,
        resources: Vec<ProtocolResourceSelector>,
        delegation_chain: Vec<String>,
        transport: impl Into<String>,
    ) -> Self {
        Self {
            principal: ProtocolPrincipal::Anonymous,
            transport: transport.into(),
            authority: Some(ProtocolAuthorityContext {
                grant_id: grant_id.into(),
                actions,
                resources,
                delegation_chain,
                verified_expires_at_ms: None,
                authority_refresh: None,
                run_authority_refresh: None,
                powerbox_authority_refresh: None,
                realization_authority_refresh: None,
            }),
            host_operation: None,
            session_id: None,
            correlation_id: Some(Uuid::new_v4()),
            parent_invocation_id: None,
        }
    }

    pub fn package(package_id: impl Into<String>, transport: impl Into<String>) -> Self {
        Self {
            principal: ProtocolPrincipal::Package {
                package_id: package_id.into(),
            },
            transport: transport.into(),
            authority: None,
            host_operation: None,
            session_id: None,
            correlation_id: Some(Uuid::new_v4()),
            parent_invocation_id: None,
        }
    }

    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    pub fn with_host_operation(
        mut self,
        action: impl Into<String>,
        resources: Vec<ProtocolResourceSelector>,
    ) -> Self {
        self.host_operation = Some(ProtocolHostOperationContext {
            action: action.into(),
            resources,
        });
        self
    }

    pub fn with_verified_authority_expiry(mut self, expires_at_ms: Option<i64>) -> Self {
        if let Some(authority) = self.authority.as_mut() {
            authority.verified_expires_at_ms = expires_at_ms;
        }
        self
    }

    pub fn with_installation_authority_refresh(
        mut self,
        refresh: InstallationAuthorityRefresh,
    ) -> Self {
        if let Some(authority) = self.authority.as_mut() {
            authority.authority_refresh = Some(refresh);
        }
        self
    }

    pub(crate) fn installation_authority_refresh(&self) -> Option<InstallationAuthorityRefresh> {
        self.authority
            .as_ref()
            .and_then(|authority| authority.authority_refresh.clone())
    }

    pub fn with_run_authority_refresh(mut self, refresh: RunAuthorityRefresh) -> Self {
        if let Some(authority) = self.authority.as_mut() {
            authority.run_authority_refresh = Some(refresh);
        }
        self
    }

    pub(crate) fn run_authority_refresh(&self) -> Option<RunAuthorityRefresh> {
        self.authority
            .as_ref()
            .and_then(|authority| authority.run_authority_refresh.clone())
    }

    pub fn with_powerbox_authority_refresh(mut self, refresh: PowerboxAuthorityRefresh) -> Self {
        if let Some(authority) = self.authority.as_mut() {
            authority.powerbox_authority_refresh = Some(refresh);
        }
        self
    }

    pub(crate) fn powerbox_authority_refresh(&self) -> Option<PowerboxAuthorityRefresh> {
        self.authority
            .as_ref()
            .and_then(|authority| authority.powerbox_authority_refresh.clone())
    }

    pub fn with_realization_authority_refresh(
        mut self,
        refresh: RealizationAuthorityRefresh,
    ) -> Self {
        if let Some(authority) = self.authority.as_mut() {
            authority.realization_authority_refresh = Some(refresh);
        }
        self
    }

    pub(crate) fn realization_authority_refresh(&self) -> Option<RealizationAuthorityRefresh> {
        self.authority
            .as_ref()
            .and_then(|authority| authority.realization_authority_refresh.clone())
    }

    pub(crate) fn verified_authority_expiry_ms(&self) -> Option<i64> {
        self.host_device_authority()
            .and_then(|authority| authority.verified_expires_at_ms)
    }

    pub fn effective_correlation_id(&self) -> Uuid {
        self.correlation_id.unwrap_or_else(Uuid::new_v4)
    }

    fn host_device_authority(&self) -> Option<&ProtocolAuthorityContext> {
        match self.principal {
            ProtocolPrincipal::Anonymous => self.authority.as_ref(),
            _ => None,
        }
    }

    pub fn is_host_device(&self) -> bool {
        self.host_device_authority().is_some()
    }

    pub fn host_device_grant_id(&self) -> Option<&str> {
        self.host_device_authority()
            .map(|authority| authority.grant_id.as_str())
    }

    pub fn allows_host_action(&self, action: &str) -> bool {
        if let Some(authority) = self.host_device_authority() {
            return authority
                .verified_expires_at_ms
                .is_none_or(|expiry| expiry > chrono::Utc::now().timestamp_millis())
                && authority.actions.iter().any(|item| item == action);
        }
        matches!(
            self.principal,
            ProtocolPrincipal::HostAdmin | ProtocolPrincipal::HostDev
        )
    }

    pub fn allows_host_resource(&self, owner: &str, kind: &str, id: &str) -> bool {
        if let Some(authority) = self.host_device_authority() {
            return authority
                .resources
                .iter()
                .any(|selector| selector.matches(owner, kind, id));
        }
        matches!(
            self.principal,
            ProtocolPrincipal::HostAdmin | ProtocolPrincipal::HostDev
        )
    }

    pub fn allows_all_host_resources(&self, owner: &str, kind: &str) -> bool {
        if let Some(authority) = self.host_device_authority() {
            return authority.resources.iter().any(|selector| {
                selector.owner == owner && selector.kind == kind && selector.id.is_none()
            });
        }
        matches!(
            self.principal,
            ProtocolPrincipal::HostAdmin | ProtocolPrincipal::HostDev
        )
    }

    pub fn host_operation_is_authorized(&self) -> bool {
        let Some(operation) = self.host_operation.as_ref() else {
            return false;
        };
        self.allows_host_action(&operation.action)
            && !operation.resources.is_empty()
            && operation.resources.iter().all(|resource| {
                resource.id.as_deref().is_some_and(|id| {
                    self.allows_host_resource(&resource.owner, &resource.kind, id)
                })
            })
    }
}

fn optional_uuid_schema(_gen: &mut SchemaGenerator) -> Schema {
    let mut schema = SchemaObject::default();
    schema.instance_type = Some(SingleOrVec::Vec(vec![
        InstanceType::String,
        InstanceType::Null,
    ]));
    schema.format = Some("uuid".to_string());
    schema.metadata = Some(Box::new(Metadata::default()));
    Schema::Object(schema)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ProtocolRequest {
    pub id: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<ContractSelection>,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ProtocolResponse {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ProtocolError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ProtocolError {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Value,
}

/// Typed marker for a Runtime operation whose durable effect may have committed
/// even though its public acknowledgement could not be completed.
#[derive(Debug)]
pub struct RuntimeOutcomeUnknown {
    stage: &'static str,
}

impl fmt::Display for RuntimeOutcomeUnknown {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Runtime outcome is unknown after {}", self.stage)
    }
}

impl std::error::Error for RuntimeOutcomeUnknown {}

pub fn runtime_outcome_unknown(stage: &'static str) -> anyhow::Error {
    RuntimeOutcomeUnknown { stage }.into()
}

fn is_runtime_outcome_unknown(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.downcast_ref::<RuntimeOutcomeUnknown>().is_some())
}

fn rights_policy_error(error: &anyhow::Error) -> Option<&crate::RightsPolicyError> {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<crate::RightsPolicyError>())
}

impl ProtocolError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, details: Value) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details,
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new("runtime/error/invalid_request", message, Value::Null)
    }

    pub fn from_anyhow(error: anyhow::Error) -> Self {
        let message = error.to_string();
        let code = if is_runtime_outcome_unknown(&error) {
            "runtime/error/outcome_unknown"
        } else if rights_policy_error(&error).is_some_and(|policy| {
            policy.outcome() == crate::RightsPolicyOutcome::RequiresEntitlement
        }) {
            "runtime/error/entitlement_required"
        } else if rights_policy_error(&error).is_some() {
            "runtime/error/rights_denied"
        } else if message.contains("not allowed")
            || message.contains("permission")
            || message.contains("authority_denied")
        {
            "runtime/error/permission_denied"
        } else if message.contains("revision_conflict")
            || message.contains("idempotency_conflict")
            || message.contains("active_run_exists")
            || message.contains("changed concurrently")
        {
            "runtime/error/conflict"
        } else if message.contains("ambiguous") {
            "runtime/error/ambiguous_route"
        } else if message.contains("schema")
            || message.contains("required")
            || message.contains("does not match")
        {
            "runtime/error/schema_invalid"
        } else if message.contains("not loaded")
            || message.contains("not found")
            || message.contains("no provider")
        {
            "runtime/error/not_found"
        } else if message.contains("closed")
            || message.contains("not ready")
            || message.contains("cannot execute")
        {
            "runtime/error/package_state"
        } else {
            "runtime/error/internal"
        };
        Self::new(code, message, Value::Null)
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct HostInfo {
    pub protocol_version: &'static str,
    pub methods: &'static [ProtocolMethod],
    pub supported_transports: Vec<&'static str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_registry_version: Option<&'static str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_profile: Option<&'static str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<Vec<ContractLayerInfo>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versions: Option<Vec<ContractVersionInfo>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profiles: Option<Vec<ContractProfileInfo>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maturity: Option<ContractMaturity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_methods: Option<&'static [ContractMethod]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_commons_registry_version: Option<&'static str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocols: Option<&'static [ProtocolDescriptor]>,
}

pub const PLATFORM_PROTOCOL_VERSION: &str = "0.1.0";

// PLATFORM_METHODS is derived from PlatformMethod — the enum is the single source
// of truth. If a new method variant is added to PlatformMethod, a corresponding
// entry must appear here (tests enforce this).
pub const PLATFORM_METHODS: &[ProtocolMethod] = &[
    ProtocolMethod {
        id: "context.open",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "context.close",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "context.fork",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "context.branch.list",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "context.get",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "context.list",
        streaming: false,
        status: MethodStatus::Planned,
    },
    ProtocolMethod {
        id: "journal.append",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "journal.list",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "journal.subscribe",
        streaming: true,
        status: MethodStatus::Planned,
    },
    ProtocolMethod {
        id: "host.package.load",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.package.unload",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.package.restart",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.package.logs",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.package.list",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.package.status",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.package.describe",
        streaming: false,
        status: MethodStatus::Planned,
    },
    ProtocolMethod {
        id: "host.installation.list",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.installation.get",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.installation.create",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.installation.update",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.installation.remove",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.exposure.list",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.exposure.create",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.exposure.revoke",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.binding.list",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.binding.candidates",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.binding.select",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.binding.revoke",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.run.list",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.run.get",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.run.start",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.run.stop",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.run.status",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.realization.plan",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.realization.apply",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.realization.get",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.realization.list",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.realization.stop",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.realization.rollback",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.realization.reconcile",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.target.list",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.target.status",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.target.register",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.target.unregister",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.exec.start",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.exec.stop",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.exec.status",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.exec.logs",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.exec.list",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.port.lease",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.port.release",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.port.status",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.port.list",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.proxy.register",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.proxy.unregister",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.proxy.status",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.proxy.list",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "capability.discover",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "capability.describe",
        streaming: false,
        status: MethodStatus::Planned,
    },
    ProtocolMethod {
        id: "capability.invoke",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "authority.handle.attenuate",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "authority.handle.revoke",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "authority.handle.list",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "capability.stream",
        streaming: true,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "capability.cancel",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "protocol.extension.list",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "protocol.extension.describe",
        streaming: false,
        status: MethodStatus::Planned,
    },
    ProtocolMethod {
        id: "protocol.hook.list",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "object.put",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "object.get",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "object.list",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "projection.register",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "projection.rebuild",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "projection.get",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "projection.list",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.info",
        streaming: false,
        status: MethodStatus::Implemented,
    },
    ProtocolMethod {
        id: "host.ping",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.diagnostics",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "identity.current",
        streaming: false,
        status: MethodStatus::Planned,
    },
    ProtocolMethod {
        id: "authority.grant.create",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "authority.grant.revoke",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "authority.grant.list",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "authority.decision.list",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.package.audit",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "change.proposal.create",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "change.proposal.get",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "change.proposal.list",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "change.proposal.approve",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "change.proposal.reject",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "change.proposal.apply",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.surface.bundle.resolve",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "shell.contribution.list",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "shell.contribution.describe",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.outbound.audit",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.outbound.execute",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.outbound.stream",
        streaming: true,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.outbound.websocket.open",
        streaming: true,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.outbound.websocket.send",
        streaming: false,
        status: MethodStatus::Partial,
    },
    ProtocolMethod {
        id: "host.outbound.websocket.close",
        streaming: false,
        status: MethodStatus::Partial,
    },
];

pub fn method_ids() -> Vec<&'static str> {
    PLATFORM_METHODS.iter().map(|method| method.id).collect()
}

pub fn host_info() -> HostInfo {
    HostInfo {
        protocol_version: PLATFORM_PROTOCOL_VERSION,
        methods: PLATFORM_METHODS,
        supported_transports: vec!["in_process", "http_rpc", "host_stdio", "http_ad_hoc"],
        contract_registry_version: Some(CONTRACT_REGISTRY_VERSION),
        default_profile: Some(DEFAULT_CONTRACT_PROFILE),
        layers: Some(contract_layers()),
        versions: Some(contract_versions()),
        profiles: Some(contract_profiles()),
        maturity: Some(ContractMaturity::Candidate),
        contract_methods: Some(contract_methods()),
        protocol_commons_registry_version: Some(PROTOCOL_COMMONS_REGISTRY_VERSION),
        protocols: Some(protocol_descriptors()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_contains_no_content_methods() {
        for id in method_ids() {
            assert!(!id.contains("turn"));
            assert!(!id.contains("prompt"));
            assert!(!id.contains("model"));
            assert!(!id.contains("message"));
        }
    }

    #[test]
    fn protocol_registry_matches_alpha_contract_core() {
        let ids = method_ids();
        for expected in [
            "context.open",
            "context.list",
            "journal.subscribe",
            "host.package.describe",
            "capability.cancel",
            "object.put",
            "identity.current",
        ] {
            assert!(ids.contains(&expected), "missing {expected}");
        }
    }

    #[test]
    fn installation_and_run_methods_have_unique_public_identities() {
        let ids = method_ids();
        assert_eq!(ids.len(), 99);
        for expected in [
            "host.installation.list",
            "host.installation.get",
            "host.installation.create",
            "host.installation.update",
            "host.installation.remove",
            "host.exposure.list",
            "host.exposure.create",
            "host.exposure.revoke",
            "host.binding.list",
            "host.binding.candidates",
            "host.binding.select",
            "host.binding.revoke",
            "host.run.list",
            "host.run.get",
            "host.run.start",
            "host.run.stop",
            "host.run.status",
        ] {
            assert!(ids.contains(&expected), "missing {expected}");
        }
        let retired_owner = ["host", "project"].join(".");
        assert!(ids.iter().all(|id| !id.starts_with(&retired_owner)));
    }

    #[test]
    fn revision_and_idempotency_failures_are_public_conflicts() {
        for message in [
            "revision_conflict: installation revision is stale",
            "run_revision_conflict",
            "idempotency_conflict: key was reused",
            "active_run_exists: Installation already has an active Run",
        ] {
            assert_eq!(
                ProtocolError::from_anyhow(anyhow::anyhow!(message)).code,
                "runtime/error/conflict"
            );
        }
    }

    #[test]
    fn outcome_unknown_requires_the_typed_marker() {
        assert_eq!(
            ProtocolError::from_anyhow(runtime_outcome_unknown("public event relay")).code,
            "runtime/error/outcome_unknown"
        );
        assert_eq!(
            ProtocolError::from_anyhow(anyhow::anyhow!(
                "ordinary failure containing outcome_unknown text"
            ))
            .code,
            "runtime/error/internal"
        );
    }

    #[test]
    fn protocol_context_serializes_session_id_when_set() {
        let ctx = ProtocolContext {
            principal: ProtocolPrincipal::HostAdmin,
            transport: "http".into(),
            authority: None,
            host_operation: None,
            session_id: Some("session-abc".into()),
            correlation_id: None,
            parent_invocation_id: None,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("\"session_id\":\"session-abc\""));
    }

    #[test]
    fn protocol_context_omits_session_id_when_none() {
        let ctx = ProtocolContext {
            principal: ProtocolPrincipal::HostAdmin,
            transport: "http".into(),
            authority: None,
            host_operation: None,
            session_id: None,
            correlation_id: None,
            parent_invocation_id: None,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        assert!(!json.contains("session_id"));
    }

    #[test]
    fn protocol_context_deserializes_without_session_id() {
        let json = r#"{"principal":{"kind":"host_admin"},"transport":"http"}"#;
        let ctx: ProtocolContext = serde_json::from_str(json).unwrap();
        assert!(ctx.session_id.is_none());
        assert!(ctx.authority.is_none());
    }

    #[test]
    fn host_device_authority_matches_exact_structured_resources() {
        let context = ProtocolContext::host_device(
            "grant-1",
            vec!["observe".into()],
            vec![ProtocolResourceSelector {
                owner: "host".into(),
                kind: "installation".into(),
                id: Some("installation-a".into()),
            }],
            Vec::new(),
            "http",
        );
        assert_eq!(context.principal, ProtocolPrincipal::Anonymous);
        assert!(context.is_host_device());
        assert_eq!(context.host_device_grant_id(), Some("grant-1"));
        assert!(context.allows_host_action("observe"));
        assert!(!context.allows_host_action("realization.apply"));
        assert!(context.allows_host_resource("host", "installation", "installation-a"));
        assert!(!context.allows_host_resource("host", "installation", "installation-ab"));
    }

    #[test]
    fn resource_selector_requires_an_explicit_nullable_id() {
        let wildcard = ProtocolResourceSelector {
            owner: "host".into(),
            kind: "installation".into(),
            id: None,
        };
        assert_eq!(
            serde_json::to_value(&wildcard).unwrap(),
            serde_json::json!({
                "owner": "host",
                "kind": "installation",
                "id": null,
            })
        );

        let decoded_wildcard: ProtocolResourceSelector = serde_json::from_value(
            serde_json::json!({"owner": "host", "kind": "installation", "id": null}),
        )
        .unwrap();
        assert_eq!(decoded_wildcard, wildcard);
        assert!(decoded_wildcard.matches("host", "installation", "installation-a"));

        let missing_id = serde_json::from_value::<ProtocolResourceSelector>(serde_json::json!({
            "owner": "host",
            "kind": "installation",
        }))
        .unwrap_err();
        assert!(missing_id.to_string().contains("missing field `id`"));

        let exact: ProtocolResourceSelector = serde_json::from_value(serde_json::json!({
            "owner": "host",
            "kind": "installation",
            "id": "installation-a",
        }))
        .unwrap();
        assert!(exact.matches("host", "installation", "installation-a"));
        assert!(!exact.matches("host", "installation", "installation-b"));
    }

    #[test]
    fn resource_selector_schema_requires_id() {
        let schema = schemars::schema_for!(ProtocolResourceSelector);
        let required = &schema
            .schema
            .object
            .as_ref()
            .expect("resource selector schema must be an object")
            .required;
        assert!(required.contains("id"));
    }

    // --- PlatformMethod / registry alignment tests ---

    #[test]
    fn every_registry_id_parses_to_platform_method() {
        for method in PLATFORM_METHODS {
            let parsed: Result<PlatformMethod, String> = method.id.parse();
            assert!(
                parsed.is_ok(),
                "registry id '{}' does not parse to PlatformMethod",
                method.id
            );
        }
    }

    #[test]
    fn platform_method_all_covers_entire_registry() {
        let all_ids: Vec<&'static str> = PlatformMethod::all().iter().map(|m| m.id()).collect();
        let registry_ids: Vec<&'static str> = PLATFORM_METHODS.iter().map(|m| m.id).collect();
        assert_eq!(
            all_ids, registry_ids,
            "PlatformMethod::all() and PLATFORM_METHODS must have identical ordered identities"
        );
    }

    #[test]
    fn registry_matches_enum_metadata() {
        for method in PLATFORM_METHODS {
            let km: PlatformMethod = method.id.parse().unwrap();
            assert_eq!(method.id, km.id(), "id mismatch for {:?}", km);
            assert_eq!(
                method.streaming,
                km.streaming(),
                "streaming mismatch for {:?}",
                km
            );
            assert_eq!(method.status, km.status(), "status mismatch for {:?}", km);
        }
    }

    #[test]
    fn no_duplicate_ids_in_all() {
        let all = PlatformMethod::all();
        let ids: Vec<&'static str> = all.iter().map(|m| m.id()).collect();
        let unique: std::collections::HashSet<&'static str> = ids.iter().copied().collect();
        assert_eq!(
            ids.len(),
            unique.len(),
            "PlatformMethod::all() contains duplicate ids"
        );
    }

    #[test]
    fn session_close_is_implemented_and_dispatched() {
        let km = PlatformMethod::SessionClose;
        assert_eq!(km.id(), "context.close");
        assert_eq!(km.status(), MethodStatus::Implemented);
        assert!(km.is_dispatched(), "context.close must be dispatch-covered");
    }

    #[test]
    fn hook_list_status_matches_dispatch() {
        let km = PlatformMethod::HookList;
        assert_eq!(km.id(), "protocol.hook.list");
        // Was previously Planned, but dispatch exists → must be at least Partial
        assert!(
            matches!(km.status(), MethodStatus::Implemented | MethodStatus::Partial),
            "protocol.hook.list status must be Implemented or Partial since dispatch exists, got {:?}",
            km.status()
        );
        assert!(
            km.is_dispatched(),
            "protocol.hook.list must be dispatch-covered"
        );
    }

    #[test]
    fn implemented_or_partial_methods_must_be_dispatched() {
        for method in PLATFORM_METHODS {
            let km: PlatformMethod = method.id.parse().unwrap();
            if matches!(
                km.status(),
                MethodStatus::Implemented | MethodStatus::Partial
            ) {
                assert!(
                    km.is_dispatched(),
                    "{:?} ({}) is {:?} but has no dispatch — add dispatch or downgrade to Planned",
                    km,
                    km.id(),
                    km.status()
                );
            }
        }
    }

    #[test]
    fn dispatched_methods_must_not_be_planned() {
        for method in PLATFORM_METHODS {
            let km: PlatformMethod = method.id.parse().unwrap();
            if km.is_dispatched() {
                assert!(
                    !matches!(km.status(), MethodStatus::Planned),
                    "{:?} ({}) is dispatched but status is Planned — upgrade to at least Partial",
                    km,
                    km.id()
                );
            }
        }
    }

    #[test]
    fn display_roundtrips_through_fromstr() {
        for km in PlatformMethod::all() {
            let s = km.to_string();
            let parsed: PlatformMethod = s.parse().unwrap();
            assert_eq!(
                *km, parsed,
                "Display -> FromStr roundtrip failed for {:?}",
                km
            );
        }
    }
}
