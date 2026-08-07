use std::collections::HashSet;
use std::fmt;
use std::sync::OnceLock;

use plurora_core::{NegotiatedProtocol, ProtocolSelection};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{negotiate_protocols, KernelMethod, MethodStatus, ProtocolError};

pub const CONTRACT_REGISTRY_VERSION: &str = "0.5.0";
pub const CONTRACT_LAYER_VERSION: &str = "0.1.0";
pub const DEFAULT_CONTRACT_PROFILE: &str = "plurora.contract.default/v1";
pub const SHELL_DEFAULT_PROFILE: &str = "plurora.shell.default/v1";
pub const LEGACY_CONTRACT_PROFILE: &str = "kernel.v1";

const INITIAL_CANONICAL_REGISTRY_VERSION: &str = "0.1.0";
const OWNER_NAMESPACE_REGISTRY_VERSION: &str = "0.2.0";
const PHASE_NINE_DEPRECATED_IN: &str = "plurora.contract.registry@0.4.0";
const PHASE_NINE_SUPPORT_UNTIL: &str = "plurora.contract.registry@0.5.0";

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum ContractOwnerLayer {
    Substrate,
    Host,
    Protocol,
    Shell,
    CrossLayer,
    LegacyAdapter,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractMaturity {
    Experimental,
    Candidate,
    Stable,
    Deprecated,
    LegacyAdapter,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractAdapter {
    Identity,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ContractAlias {
    pub id: String,
    pub canonical_id: String,
    pub maturity: ContractMaturity,
    pub request_adapter: ContractAdapter,
    pub response_adapter: ContractAdapter,
    pub introduced_in: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_in: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ContractDiagnostic {
    pub code: String,
    pub severity: String,
    pub requested_id: String,
    pub canonical_id: String,
    pub maturity: ContractMaturity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_in: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ContractMethod {
    pub canonical_id: String,
    pub aliases: Vec<ContractAlias>,
    pub owner_layer: ContractOwnerLayer,
    pub maturity: ContractMaturity,
    pub request_schema: String,
    pub response_schema: String,
    pub request_adapter: ContractAdapter,
    pub response_adapter: ContractAdapter,
    pub introduced_in: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_in: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    pub implementation_status: MethodStatus,
    pub streaming: bool,
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) method: KernelMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ContractLayerInfo {
    pub id: ContractOwnerLayer,
    pub description: String,
    pub maturity: ContractMaturity,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ContractVersionInfo {
    pub layer: ContractOwnerLayer,
    pub version: String,
    pub maturity: ContractMaturity,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ContractVersionRequirement {
    pub layer: ContractOwnerLayer,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ContractProfileInfo {
    pub id: String,
    pub maturity: ContractMaturity,
    pub versions: Vec<ContractVersionRequirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ContractSelection {
    pub profile: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub versions: Vec<ContractVersionRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocols: Vec<ProtocolSelection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ContractNegotiation {
    pub profile: String,
    pub maturity: ContractMaturity,
    pub versions: Vec<ContractVersionRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocols: Vec<NegotiatedProtocol>,
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedContractMethod {
    pub method: KernelMethod,
    pub contract: &'static ContractMethod,
    pub alias: Option<&'static ContractAlias>,
}

impl ResolvedContractMethod {
    pub fn requested_id(&self) -> &str {
        self.alias
            .map_or(self.contract.canonical_id.as_str(), |alias| {
                alias.id.as_str()
            })
    }

    pub fn adapt_request(&self, value: Value) -> Result<Value, ProtocolError> {
        let adapter = self
            .alias
            .map_or(self.contract.request_adapter, |alias| alias.request_adapter);
        apply_adapter(adapter, value)
    }

    pub fn adapt_response(&self, value: Value) -> Result<Value, ProtocolError> {
        let adapter = self.alias.map_or(self.contract.response_adapter, |alias| {
            alias.response_adapter
        });
        apply_adapter(adapter, value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownContractMethod {
    id: String,
}

impl fmt::Display for UnknownContractMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown contract method: {}", self.id)
    }
}

impl std::error::Error for UnknownContractMethod {}

static CONTRACT_METHODS: OnceLock<Vec<ContractMethod>> = OnceLock::new();
static CONTRACT_ALIASES: OnceLock<Vec<ContractAlias>> = OnceLock::new();

pub fn contract_methods() -> &'static [ContractMethod] {
    CONTRACT_METHODS
        .get_or_init(|| {
            KernelMethod::all()
                .iter()
                .copied()
                .map(contract_descriptor)
                .collect()
        })
        .as_slice()
}

pub fn contract_aliases() -> &'static [ContractAlias] {
    CONTRACT_ALIASES
        .get_or_init(|| {
            contract_methods()
                .iter()
                .flat_map(|method| method.aliases.iter().cloned())
                .collect()
        })
        .as_slice()
}

pub fn contract_method(method: KernelMethod) -> &'static ContractMethod {
    contract_methods()
        .iter()
        .find(|descriptor| descriptor.method == method)
        .expect("every KernelMethod must have a contract descriptor")
}

pub fn resolve_contract_method(id: &str) -> Result<ResolvedContractMethod, UnknownContractMethod> {
    for contract in contract_methods() {
        if contract.canonical_id == id {
            return Ok(ResolvedContractMethod {
                method: contract.method,
                contract,
                alias: None,
            });
        }
        if let Some(alias) = contract.aliases.iter().find(|alias| alias.id == id) {
            return Ok(ResolvedContractMethod {
                method: contract.method,
                contract,
                alias: Some(alias),
            });
        }
    }
    Err(UnknownContractMethod { id: id.to_string() })
}

pub fn contract_diagnostics(id: &str) -> Vec<ContractDiagnostic> {
    let Ok(resolved) = resolve_contract_method(id) else {
        return Vec::new();
    };
    let Some(alias) = resolved.alias else {
        return Vec::new();
    };
    if alias.deprecated_in.is_none()
        || !matches!(
            alias.maturity,
            ContractMaturity::Deprecated | ContractMaturity::LegacyAdapter
        )
    {
        return Vec::new();
    }

    let replacement = alias
        .replacement
        .clone()
        .unwrap_or_else(|| alias.canonical_id.clone());
    let (code, message) = match alias.maturity {
        ContractMaturity::Deprecated => (
            "plurora.contract.alias.deprecated",
            format!(
                "contract alias '{}' is deprecated; migrate to '{}' before the support window closes at {}",
                alias.id,
                replacement,
                alias.support_until.as_deref().unwrap_or("an unspecified registry version")
            ),
        ),
        ContractMaturity::LegacyAdapter => (
            "plurora.contract.alias.legacy_adapter",
            format!(
                "contract alias '{}' is a legacy adapter; use '{}'; no new field semantics will be added",
                alias.id, replacement
            ),
        ),
        _ => unreachable!("non-deprecated aliases returned before diagnostic construction"),
    };

    vec![ContractDiagnostic {
        code: code.to_string(),
        severity: "warning".to_string(),
        requested_id: alias.id.clone(),
        canonical_id: alias.canonical_id.clone(),
        maturity: alias.maturity,
        message,
        deprecated_in: alias.deprecated_in.clone(),
        replacement: Some(replacement),
        support_until: alias.support_until.clone(),
    }]
}

pub fn contract_layers() -> Vec<ContractLayerInfo> {
    vec![
        ContractLayerInfo {
            id: ContractOwnerLayer::Substrate,
            description: "Identity, authority, objects, journal, invocation, streams, and receipts"
                .to_string(),
            maturity: ContractMaturity::Experimental,
        },
        ContractLayerInfo {
            id: ContractOwnerLayer::Host,
            description:
                "Host-local installation, execution, ports, proxies, secrets, and diagnostics"
                    .to_string(),
            maturity: ContractMaturity::Experimental,
        },
        ContractLayerInfo {
            id: ContractOwnerLayer::Protocol,
            description: "Shared semantic protocols and compatibility profiles".to_string(),
            maturity: ContractMaturity::Experimental,
        },
        ContractLayerInfo {
            id: ContractOwnerLayer::Shell,
            description: "Product and shell interaction profiles".to_string(),
            maturity: ContractMaturity::Experimental,
        },
        ContractLayerInfo {
            id: ContractOwnerLayer::CrossLayer,
            description: "Transitional methods that still combine multiple owners".to_string(),
            maturity: ContractMaturity::Experimental,
        },
        ContractLayerInfo {
            id: ContractOwnerLayer::LegacyAdapter,
            description: "Compatibility routes for the kernel.v1 operational contract".to_string(),
            maturity: ContractMaturity::LegacyAdapter,
        },
    ]
}

pub fn contract_versions() -> Vec<ContractVersionInfo> {
    contract_layers()
        .into_iter()
        .map(|layer| ContractVersionInfo {
            layer: layer.id,
            version: CONTRACT_LAYER_VERSION.to_string(),
            maturity: layer.maturity,
        })
        .collect()
}

pub fn contract_profiles() -> Vec<ContractProfileInfo> {
    vec![
        ContractProfileInfo {
            id: DEFAULT_CONTRACT_PROFILE.to_string(),
            maturity: ContractMaturity::Experimental,
            versions: [
                ContractOwnerLayer::Substrate,
                ContractOwnerLayer::Host,
                ContractOwnerLayer::Protocol,
                ContractOwnerLayer::Shell,
            ]
            .into_iter()
            .map(version_requirement)
            .collect(),
        },
        ContractProfileInfo {
            id: SHELL_DEFAULT_PROFILE.to_string(),
            maturity: ContractMaturity::Experimental,
            versions: [
                ContractOwnerLayer::Host,
                ContractOwnerLayer::Protocol,
                ContractOwnerLayer::Shell,
            ]
            .into_iter()
            .map(version_requirement)
            .collect(),
        },
        ContractProfileInfo {
            id: LEGACY_CONTRACT_PROFILE.to_string(),
            maturity: ContractMaturity::LegacyAdapter,
            versions: vec![version_requirement(ContractOwnerLayer::LegacyAdapter)],
        },
    ]
}

pub fn negotiate_contract(
    selection: Option<&ContractSelection>,
) -> Result<ContractNegotiation, ProtocolError> {
    let requested_profile = selection
        .map(|selection| selection.profile.as_str())
        .unwrap_or(LEGACY_CONTRACT_PROFILE);
    let profiles = contract_profiles();
    let Some(profile) = profiles
        .iter()
        .find(|profile| profile.id == requested_profile)
    else {
        return Err(unsupported_contract_error(
            "unknown_profile",
            selection,
            json!({
                "requested_profile": requested_profile,
                "supported_profiles": profiles.iter().map(|profile| profile.id.as_str()).collect::<Vec<_>>(),
            }),
        ));
    };

    if let Some(selection) = selection {
        let mut seen = HashSet::new();
        for requested in &selection.versions {
            if !seen.insert(requested.layer) {
                return Err(ProtocolError::invalid_request(format!(
                    "contract selection contains duplicate version requirement for {:?}",
                    requested.layer
                )));
            }
            let Some(supported) = profile
                .versions
                .iter()
                .find(|supported| supported.layer == requested.layer)
            else {
                return Err(unsupported_contract_error(
                    "layer_not_in_profile",
                    Some(selection),
                    json!({
                        "requested_layer": requested.layer,
                        "requested_version": requested.version,
                        "profile": profile.id,
                        "profile_versions": profile.versions,
                    }),
                ));
            };
            if supported.version != requested.version {
                return Err(unsupported_contract_error(
                    "unsupported_version",
                    Some(selection),
                    json!({
                        "requested_layer": requested.layer,
                        "requested_version": requested.version,
                        "supported_version": supported.version,
                        "profile": profile.id,
                    }),
                ));
            }
        }
    }

    let protocols = selection
        .map(|selection| negotiate_protocols(&selection.protocols))
        .transpose()?
        .unwrap_or_default();

    Ok(ContractNegotiation {
        profile: profile.id.clone(),
        maturity: profile.maturity,
        versions: profile.versions.clone(),
        protocols,
    })
}

impl KernelMethod {
    pub fn contract(&self) -> &'static ContractMethod {
        contract_method(*self)
    }

    pub fn canonical_id(&self) -> &'static str {
        self.contract().canonical_id.as_str()
    }
}

fn contract_descriptor(method: KernelMethod) -> ContractMethod {
    let legacy_id = method.id();
    let canonical_id = match method {
        KernelMethod::HostInfo => "host.info",
        KernelMethod::ProjectList => "host.project.list",
        KernelMethod::ProjectGet => "host.project.get",
        KernelMethod::ProjectStart => "host.project.start",
        KernelMethod::ProjectStop => "host.project.stop",
        KernelMethod::ProjectStatus => "host.project.status",
        KernelMethod::TargetList => "host.target.list",
        KernelMethod::TargetStatus => "host.target.status",
        KernelMethod::TargetRegister => "host.target.register",
        KernelMethod::TargetUnregister => "host.target.unregister",
        KernelMethod::ExecStart => "host.exec.start",
        KernelMethod::ExecStop => "host.exec.stop",
        KernelMethod::ExecStatus => "host.exec.status",
        KernelMethod::ExecLogs => "host.exec.logs",
        KernelMethod::ExecList => "host.exec.list",
        KernelMethod::PortLease => "host.port.lease",
        KernelMethod::PortRelease => "host.port.release",
        KernelMethod::PortStatus => "host.port.status",
        KernelMethod::PortList => "host.port.list",
        KernelMethod::ProxyRegister => "host.proxy.register",
        KernelMethod::ProxyUnregister => "host.proxy.unregister",
        KernelMethod::ProxyStatus => "host.proxy.status",
        KernelMethod::ProxyList => "host.proxy.list",
        KernelMethod::SurfaceResolveBundle => "host.surface.bundle.resolve",
        KernelMethod::SurfaceContributionList => "shell.contribution.list",
        KernelMethod::SurfaceContributionDescribe => "shell.contribution.describe",
        KernelMethod::ProposalCreate => "change.proposal.create",
        KernelMethod::ProposalGet => "change.proposal.get",
        KernelMethod::ProposalList => "change.proposal.list",
        KernelMethod::ProposalApprove => "change.proposal.approve",
        KernelMethod::ProposalReject => "change.proposal.reject",
        KernelMethod::ProposalApply => "change.proposal.apply",
        KernelMethod::ProjectionRegister => "projection.register",
        KernelMethod::ProjectionRebuild => "projection.rebuild",
        KernelMethod::ProjectionGet => "projection.get",
        KernelMethod::ProjectionList => "projection.list",
        _ => legacy_id,
    };
    let aliases = if canonical_id == legacy_id {
        Vec::new()
    } else {
        let phase_nine_lifecycle = is_phase_nine_lifecycle_alias(method);
        vec![ContractAlias {
            id: legacy_id.to_string(),
            canonical_id: canonical_id.to_string(),
            maturity: ContractMaturity::LegacyAdapter,
            request_adapter: ContractAdapter::Identity,
            response_adapter: ContractAdapter::Identity,
            introduced_in: "kernel.v1@0.1.0".to_string(),
            deprecated_in: phase_nine_lifecycle.then(|| PHASE_NINE_DEPRECATED_IN.to_string()),
            replacement: Some(canonical_id.to_string()),
            support_until: phase_nine_lifecycle.then(|| PHASE_NINE_SUPPORT_UNTIL.to_string()),
        }]
    };
    let schema = format!("https://plurora.dev/spec/v1/methods/{legacy_id}.schema.json");
    ContractMethod {
        canonical_id: canonical_id.to_string(),
        aliases,
        owner_layer: owner_layer(method),
        maturity: if is_phase_nine_lifecycle_alias(method) {
            ContractMaturity::Candidate
        } else {
            ContractMaturity::Experimental
        },
        request_schema: format!("{schema}#/$defs/Params"),
        response_schema: format!("{schema}#/$defs/Result"),
        request_adapter: ContractAdapter::Identity,
        response_adapter: ContractAdapter::Identity,
        introduced_in: canonical_introduced_in(method, canonical_id, legacy_id),
        deprecated_in: None,
        replacement: None,
        implementation_status: method.status(),
        streaming: method.streaming(),
        method,
    }
}

fn is_phase_nine_lifecycle_alias(method: KernelMethod) -> bool {
    matches!(method, KernelMethod::HostInfo | KernelMethod::TargetList)
}

fn owner_layer(method: KernelMethod) -> ContractOwnerLayer {
    match method {
        KernelMethod::SessionOpen
        | KernelMethod::SessionClose
        | KernelMethod::SessionFork
        | KernelMethod::SessionBranchList
        | KernelMethod::SessionGet
        | KernelMethod::SessionList
        | KernelMethod::EventAppend
        | KernelMethod::EventList
        | KernelMethod::EventSubscribe
        | KernelMethod::PackageUnload
        | KernelMethod::PackageRestart
        | KernelMethod::CapabilityDiscover
        | KernelMethod::CapabilityDescribe
        | KernelMethod::CapabilityInvoke
        | KernelMethod::CapabilityHandleAttenuate
        | KernelMethod::CapabilityHandleRevoke
        | KernelMethod::CapabilityHandleListFor
        | KernelMethod::CapabilityStream
        | KernelMethod::CapabilityCancel
        | KernelMethod::AssetPut
        | KernelMethod::AssetGet
        | KernelMethod::HostPrincipal
        | KernelMethod::PermissionGrant
        | KernelMethod::PermissionRevoke
        | KernelMethod::PermissionList
        | KernelMethod::PermissionAudit
        | KernelMethod::OutboundAudit => ContractOwnerLayer::Substrate,

        KernelMethod::PackageLogs
        | KernelMethod::ProjectList
        | KernelMethod::ProjectGet
        | KernelMethod::ProjectStart
        | KernelMethod::ProjectStop
        | KernelMethod::ProjectStatus
        | KernelMethod::TargetList
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
        | KernelMethod::AssetList
        | KernelMethod::HostInfo
        | KernelMethod::HostPing
        | KernelMethod::HostDiagnostics => ContractOwnerLayer::Host,

        KernelMethod::ExtensionPointList
        | KernelMethod::ExtensionPointDescribe
        | KernelMethod::HookList
        | KernelMethod::ProjectionRegister
        | KernelMethod::ProjectionRebuild
        | KernelMethod::ProjectionGet
        | KernelMethod::ProjectionList
        | KernelMethod::ProposalCreate
        | KernelMethod::ProposalGet
        | KernelMethod::ProposalList
        | KernelMethod::ProposalApprove
        | KernelMethod::ProposalReject
        | KernelMethod::ProposalApply => ContractOwnerLayer::Protocol,

        KernelMethod::SurfaceContributionList | KernelMethod::SurfaceContributionDescribe => {
            ContractOwnerLayer::Shell
        }

        KernelMethod::PackageLoad
        | KernelMethod::PackageList
        | KernelMethod::PackageStatus
        | KernelMethod::PackageDescribe
        | KernelMethod::AuditPackage
        | KernelMethod::OutboundExecute
        | KernelMethod::OutboundStream
        | KernelMethod::OutboundWebSocketOpen
        | KernelMethod::OutboundWebSocketSend
        | KernelMethod::OutboundWebSocketClose => ContractOwnerLayer::CrossLayer,

        KernelMethod::SurfaceResolveBundle => ContractOwnerLayer::Host,
    }
}

fn canonical_introduced_in(method: KernelMethod, canonical_id: &str, legacy_id: &str) -> String {
    if canonical_id == legacy_id {
        return "kernel.v1@0.1.0".to_string();
    }
    if matches!(
        method,
        KernelMethod::SurfaceContributionList | KernelMethod::SurfaceContributionDescribe
    ) {
        return format!("{SHELL_DEFAULT_PROFILE}@{CONTRACT_LAYER_VERSION}");
    }
    if matches!(method, KernelMethod::HostInfo | KernelMethod::TargetList) {
        return format!("{DEFAULT_CONTRACT_PROFILE}@{INITIAL_CANONICAL_REGISTRY_VERSION}");
    }
    format!("{DEFAULT_CONTRACT_PROFILE}@{OWNER_NAMESPACE_REGISTRY_VERSION}")
}

fn version_requirement(layer: ContractOwnerLayer) -> ContractVersionRequirement {
    ContractVersionRequirement {
        layer,
        version: CONTRACT_LAYER_VERSION.to_string(),
    }
}

fn apply_adapter(adapter: ContractAdapter, value: Value) -> Result<Value, ProtocolError> {
    match adapter {
        ContractAdapter::Identity => Ok(value),
    }
}

fn unsupported_contract_error(
    reason: &str,
    selection: Option<&ContractSelection>,
    details: Value,
) -> ProtocolError {
    ProtocolError::new(
        "kernel/v1/error/unsupported_contract",
        format!("requested contract cannot be satisfied: {reason}"),
        json!({
            "reason": reason,
            "selection": selection,
            "details": details,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_complete_and_ids_are_globally_unique() {
        assert_eq!(contract_methods().len(), KernelMethod::all().len());
        let mut ids = HashSet::new();
        for contract in contract_methods() {
            assert!(ids.insert(contract.canonical_id.as_str()));
            for alias in &contract.aliases {
                assert!(ids.insert(alias.id.as_str()));
                assert_eq!(alias.canonical_id, contract.canonical_id);
            }
        }
    }

    #[test]
    fn every_legacy_alias_resolves_to_its_canonical_handler() {
        for contract in contract_methods() {
            let canonical = resolve_contract_method(&contract.canonical_id).unwrap();
            assert_eq!(canonical.method, contract.method);
            assert!(canonical.alias.is_none());
            for alias in &contract.aliases {
                let legacy = resolve_contract_method(&alias.id).unwrap();
                assert_eq!(legacy.method, canonical.method);
                assert_eq!(legacy.contract.canonical_id, contract.canonical_id);
                assert_eq!(legacy.alias, Some(alias));
            }
        }
    }

    #[test]
    fn phase_three_namespaces_are_exact() {
        let expected = [
            (KernelMethod::HostInfo, "host.info"),
            (KernelMethod::ProjectList, "host.project.list"),
            (KernelMethod::ProjectGet, "host.project.get"),
            (KernelMethod::ProjectStart, "host.project.start"),
            (KernelMethod::ProjectStop, "host.project.stop"),
            (KernelMethod::ProjectStatus, "host.project.status"),
            (KernelMethod::TargetList, "host.target.list"),
            (KernelMethod::TargetStatus, "host.target.status"),
            (KernelMethod::TargetRegister, "host.target.register"),
            (KernelMethod::TargetUnregister, "host.target.unregister"),
            (KernelMethod::ExecStart, "host.exec.start"),
            (KernelMethod::ExecStop, "host.exec.stop"),
            (KernelMethod::ExecStatus, "host.exec.status"),
            (KernelMethod::ExecLogs, "host.exec.logs"),
            (KernelMethod::ExecList, "host.exec.list"),
            (KernelMethod::PortLease, "host.port.lease"),
            (KernelMethod::PortRelease, "host.port.release"),
            (KernelMethod::PortStatus, "host.port.status"),
            (KernelMethod::PortList, "host.port.list"),
            (KernelMethod::ProxyRegister, "host.proxy.register"),
            (KernelMethod::ProxyUnregister, "host.proxy.unregister"),
            (KernelMethod::ProxyStatus, "host.proxy.status"),
            (KernelMethod::ProxyList, "host.proxy.list"),
            (
                KernelMethod::SurfaceResolveBundle,
                "host.surface.bundle.resolve",
            ),
            (
                KernelMethod::SurfaceContributionList,
                "shell.contribution.list",
            ),
            (
                KernelMethod::SurfaceContributionDescribe,
                "shell.contribution.describe",
            ),
            (KernelMethod::ProposalCreate, "change.proposal.create"),
            (KernelMethod::ProposalGet, "change.proposal.get"),
            (KernelMethod::ProposalList, "change.proposal.list"),
            (KernelMethod::ProposalApprove, "change.proposal.approve"),
            (KernelMethod::ProposalReject, "change.proposal.reject"),
            (KernelMethod::ProposalApply, "change.proposal.apply"),
            (KernelMethod::ProjectionRegister, "projection.register"),
            (KernelMethod::ProjectionRebuild, "projection.rebuild"),
            (KernelMethod::ProjectionGet, "projection.get"),
            (KernelMethod::ProjectionList, "projection.list"),
        ];
        for (method, canonical_id) in expected {
            assert_eq!(contract_method(method).canonical_id, canonical_id);
        }
        assert_eq!(contract_aliases().len(), expected.len());
    }

    #[test]
    fn shell_default_profile_and_registry_history_are_advertised() {
        let shell_profile = contract_profiles()
            .into_iter()
            .find(|profile| profile.id == SHELL_DEFAULT_PROFILE)
            .expect("shell default profile must be advertised");
        assert_eq!(
            shell_profile
                .versions
                .iter()
                .map(|requirement| requirement.layer)
                .collect::<Vec<_>>(),
            vec![
                ContractOwnerLayer::Host,
                ContractOwnerLayer::Protocol,
                ContractOwnerLayer::Shell,
            ]
        );
        assert_eq!(
            contract_method(KernelMethod::HostInfo).introduced_in,
            "plurora.contract.default/v1@0.1.0"
        );
        assert_eq!(
            contract_method(KernelMethod::ProjectList).introduced_in,
            "plurora.contract.default/v1@0.2.0"
        );
        assert_eq!(
            contract_method(KernelMethod::SurfaceContributionList).introduced_in,
            "plurora.shell.default/v1@0.1.0"
        );
    }

    #[test]
    fn phase_nine_aliases_complete_the_legacy_adapter_transition() {
        assert_eq!(CONTRACT_REGISTRY_VERSION, "0.5.0");
        for method in [KernelMethod::HostInfo, KernelMethod::TargetList] {
            let contract = contract_method(method);
            assert_eq!(contract.maturity, ContractMaturity::Candidate);
            let alias = contract.aliases.first().expect("tracked legacy alias");
            assert_eq!(alias.maturity, ContractMaturity::LegacyAdapter);
            assert_eq!(
                alias.deprecated_in.as_deref(),
                Some(PHASE_NINE_DEPRECATED_IN)
            );
            assert_eq!(
                alias.replacement.as_deref(),
                Some(contract.canonical_id.as_str())
            );
            assert_eq!(
                alias.support_until.as_deref(),
                Some(PHASE_NINE_SUPPORT_UNTIL)
            );
        }
    }

    #[test]
    fn phase_nine_legacy_adapters_freeze_wire_field_semantics() {
        let request = json!({
            "known": "value",
            "future_unknown_field": { "must": "remain lossless" }
        });
        let response = json!({
            "result": true,
            "future_unknown_field": [1, 2, 3]
        });

        for method in [KernelMethod::HostInfo, KernelMethod::TargetList] {
            let contract = contract_method(method);
            let alias = contract.aliases.first().expect("tracked legacy alias");
            let canonical = resolve_contract_method(&contract.canonical_id).unwrap();
            let legacy = resolve_contract_method(&alias.id).unwrap();

            assert_eq!(alias.maturity, ContractMaturity::LegacyAdapter);
            assert_eq!(alias.request_adapter, ContractAdapter::Identity);
            assert_eq!(alias.response_adapter, ContractAdapter::Identity);
            assert_eq!(legacy.method, canonical.method);
            assert_eq!(
                legacy.contract.request_schema,
                canonical.contract.request_schema
            );
            assert_eq!(
                legacy.contract.response_schema,
                canonical.contract.response_schema
            );
            assert_eq!(canonical.adapt_request(request.clone()).unwrap(), request);
            assert_eq!(
                canonical.adapt_response(response.clone()).unwrap(),
                response
            );
            assert_eq!(legacy.adapt_request(request.clone()).unwrap(), request);
            assert_eq!(legacy.adapt_response(response.clone()).unwrap(), response);
        }
    }

    #[test]
    fn diagnostics_are_emitted_only_for_lifecycle_tracked_aliases() {
        assert!(contract_diagnostics("host.info").is_empty());
        assert!(contract_diagnostics("kernel.v1.project.list").is_empty());

        let diagnostics = contract_diagnostics("kernel.v1.host.info");
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code, "plurora.contract.alias.legacy_adapter");
        assert_eq!(diagnostic.severity, "warning");
        assert_eq!(diagnostic.requested_id, "kernel.v1.host.info");
        assert_eq!(diagnostic.canonical_id, "host.info");
        assert_eq!(diagnostic.maturity, ContractMaturity::LegacyAdapter);
        assert!(diagnostic.message.contains("no new field semantics"));
        assert_eq!(
            diagnostic.support_until.as_deref(),
            Some(PHASE_NINE_SUPPORT_UNTIL)
        );
    }

    #[test]
    fn negotiation_rejects_unknown_profile_and_version() {
        let unknown_profile = ContractSelection {
            profile: "missing/profile".to_string(),
            versions: Vec::new(),
            protocols: Vec::new(),
        };
        let error = negotiate_contract(Some(&unknown_profile)).unwrap_err();
        assert_eq!(error.code, "kernel/v1/error/unsupported_contract");

        let unsupported_version = ContractSelection {
            profile: DEFAULT_CONTRACT_PROFILE.to_string(),
            versions: vec![ContractVersionRequirement {
                layer: ContractOwnerLayer::Host,
                version: "999.0.0".to_string(),
            }],
            protocols: Vec::new(),
        };
        let error = negotiate_contract(Some(&unsupported_version)).unwrap_err();
        assert_eq!(error.code, "kernel/v1/error/unsupported_contract");
        assert_eq!(error.details["reason"], "unsupported_version");
    }

    #[test]
    fn negotiation_includes_explicit_protocol_profiles() {
        let selection = ContractSelection {
            profile: DEFAULT_CONTRACT_PROFILE.to_string(),
            versions: Vec::new(),
            protocols: vec![ProtocolSelection {
                protocol_id: crate::CHANGE_PROTOCOL_ID.to_string(),
                version: crate::CHANGE_PROTOCOL_VERSION.to_string(),
                profile: Some(crate::CHANGE_DEFAULT_PROFILE.to_string()),
            }],
        };
        let negotiation = negotiate_contract(Some(&selection)).unwrap();
        assert_eq!(negotiation.protocols.len(), 1);
        assert_eq!(
            negotiation.protocols[0].protocol_id,
            crate::CHANGE_PROTOCOL_ID
        );
    }
}
