use std::collections::HashSet;
use std::fmt;
use std::sync::OnceLock;

use plurora_core::{NegotiatedProtocol, ProtocolSelection};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{negotiate_protocols, MethodStatus, PlatformMethod, ProtocolError};

pub const CONTRACT_REGISTRY_VERSION: &str = "0.1.0";
pub const CONTRACT_LAYER_VERSION: &str = "0.1.0";
pub const DEFAULT_CONTRACT_PROFILE: &str = "plurora.contract.default/v1";
pub const SHELL_DEFAULT_PROFILE: &str = "plurora.shell.default/v1";

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum ContractOwnerLayer {
    Substrate,
    Host,
    Protocol,
    Shell,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractMaturity {
    Experimental,
    Candidate,
    Stable,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ContractMethod {
    pub id: String,
    pub owner_layer: ContractOwnerLayer,
    pub maturity: ContractMaturity,
    pub request_schema: String,
    pub response_schema: String,
    pub introduced_in: String,
    pub implementation_status: MethodStatus,
    pub streaming: bool,
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) method: PlatformMethod,
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
    pub method: PlatformMethod,
    pub contract: &'static ContractMethod,
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

pub fn contract_methods() -> &'static [ContractMethod] {
    CONTRACT_METHODS
        .get_or_init(|| {
            PlatformMethod::all()
                .iter()
                .copied()
                .map(contract_descriptor)
                .collect()
        })
        .as_slice()
}

pub fn contract_method(method: PlatformMethod) -> &'static ContractMethod {
    contract_methods()
        .iter()
        .find(|descriptor| descriptor.method == method)
        .expect("every PlatformMethod must have a contract descriptor")
}

pub fn resolve_contract_method(id: &str) -> Result<ResolvedContractMethod, UnknownContractMethod> {
    contract_methods()
        .iter()
        .find(|contract| contract.id == id)
        .map(|contract| ResolvedContractMethod {
            method: contract.method,
            contract,
        })
        .ok_or_else(|| UnknownContractMethod { id: id.to_string() })
}

pub fn contract_layers() -> Vec<ContractLayerInfo> {
    vec![
        ContractLayerInfo {
            id: ContractOwnerLayer::Substrate,
            description: "Identity, authority, objects, journal, invocation, streams, and receipts"
                .to_string(),
            maturity: ContractMaturity::Candidate,
        },
        ContractLayerInfo {
            id: ContractOwnerLayer::Host,
            description:
                "Host-local installation, execution, ports, proxies, secrets, deployment, and diagnostics"
                    .to_string(),
            maturity: ContractMaturity::Candidate,
        },
        ContractLayerInfo {
            id: ContractOwnerLayer::Protocol,
            description: "Shared semantic protocols, changes, projections, and extension contracts"
                .to_string(),
            maturity: ContractMaturity::Experimental,
        },
        ContractLayerInfo {
            id: ContractOwnerLayer::Shell,
            description: "Replaceable interaction and presentation profiles".to_string(),
            maturity: ContractMaturity::Experimental,
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
            maturity: ContractMaturity::Candidate,
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
    ]
}

pub fn negotiate_contract(
    selection: Option<&ContractSelection>,
) -> Result<ContractNegotiation, ProtocolError> {
    let requested_profile = selection
        .map(|selection| selection.profile.as_str())
        .unwrap_or(DEFAULT_CONTRACT_PROFILE);
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

impl PlatformMethod {
    pub fn contract(&self) -> &'static ContractMethod {
        contract_method(*self)
    }
}

fn contract_descriptor(method: PlatformMethod) -> ContractMethod {
    let id = method.id();
    let schema = format!("urn:plurora:schema:method:{id}:v1");
    let owner_layer = owner_layer(id);
    let maturity = match method.status() {
        MethodStatus::Implemented => ContractMaturity::Candidate,
        MethodStatus::Partial | MethodStatus::Planned => ContractMaturity::Experimental,
    };
    let profile = if owner_layer == ContractOwnerLayer::Shell {
        SHELL_DEFAULT_PROFILE
    } else {
        DEFAULT_CONTRACT_PROFILE
    };

    ContractMethod {
        id: id.to_string(),
        owner_layer,
        maturity,
        request_schema: format!("{schema}#/$defs/Params"),
        response_schema: format!("{schema}#/$defs/Result"),
        introduced_in: format!("{profile}@{CONTRACT_REGISTRY_VERSION}"),
        implementation_status: method.status(),
        streaming: method.streaming(),
        method,
    }
}

fn owner_layer(id: &str) -> ContractOwnerLayer {
    let owner = id.split('.').next().unwrap_or_default();
    match owner {
        "context" | "journal" | "capability" | "authority" | "object" | "identity" => {
            ContractOwnerLayer::Substrate
        }
        "host" => ContractOwnerLayer::Host,
        "protocol" | "change" | "projection" => ContractOwnerLayer::Protocol,
        "shell" => ContractOwnerLayer::Shell,
        _ => panic!("method {id} has no declared contract owner"),
    }
}

fn version_requirement(layer: ContractOwnerLayer) -> ContractVersionRequirement {
    ContractVersionRequirement {
        layer,
        version: CONTRACT_LAYER_VERSION.to_string(),
    }
}

fn unsupported_contract_error(
    reason: &str,
    selection: Option<&ContractSelection>,
    details: Value,
) -> ProtocolError {
    ProtocolError::new(
        "protocol/error/unsupported_contract",
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
        assert_eq!(contract_methods().len(), PlatformMethod::all().len());
        let mut ids = HashSet::new();
        for contract in contract_methods() {
            assert!(ids.insert(contract.id.as_str()));
            assert_eq!(contract.id, contract.method.id());
            assert!(contract
                .request_schema
                .starts_with("urn:plurora:schema:method:"));
            assert!(contract
                .response_schema
                .starts_with("urn:plurora:schema:method:"));
        }
    }

    #[test]
    fn resolver_accepts_only_the_single_registered_id() {
        for contract in contract_methods() {
            let resolved = resolve_contract_method(&contract.id).unwrap();
            assert_eq!(resolved.method, contract.method);
            assert_eq!(resolved.contract.id, contract.id);
        }
        assert!(resolve_contract_method("platform.host.info").is_err());
        assert!(resolve_contract_method("plurora.host.info").is_err());
    }

    #[test]
    fn profiles_have_no_legacy_layer() {
        assert_eq!(CONTRACT_REGISTRY_VERSION, "0.1.0");
        assert_eq!(contract_profiles().len(), 2);
        assert!(contract_profiles()
            .iter()
            .all(|profile| !profile.id.contains("legacy")));
        assert!(contract_layers().iter().all(|layer| matches!(
            layer.id,
            ContractOwnerLayer::Substrate
                | ContractOwnerLayer::Host
                | ContractOwnerLayer::Protocol
                | ContractOwnerLayer::Shell
        )));
    }

    #[test]
    fn omitted_selection_uses_the_default_profile() {
        let negotiated = negotiate_contract(None).unwrap();
        assert_eq!(negotiated.profile, DEFAULT_CONTRACT_PROFILE);
    }

    #[test]
    fn negotiation_rejects_unknown_profile_and_version() {
        let unknown_profile = ContractSelection {
            profile: "missing/profile".to_string(),
            versions: Vec::new(),
            protocols: Vec::new(),
        };
        let error = negotiate_contract(Some(&unknown_profile)).unwrap_err();
        assert_eq!(error.code, "protocol/error/unsupported_contract");

        let unsupported_version = ContractSelection {
            profile: DEFAULT_CONTRACT_PROFILE.to_string(),
            versions: vec![ContractVersionRequirement {
                layer: ContractOwnerLayer::Host,
                version: "999.0.0".to_string(),
            }],
            protocols: Vec::new(),
        };
        let error = negotiate_contract(Some(&unsupported_version)).unwrap_err();
        assert_eq!(error.code, "protocol/error/unsupported_contract");
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
        let negotiated = negotiate_contract(Some(&selection)).unwrap();
        assert_eq!(negotiated.protocols.len(), 1);
        assert_eq!(
            negotiated.protocols[0].protocol_id,
            crate::CHANGE_PROTOCOL_ID
        );
    }
}
