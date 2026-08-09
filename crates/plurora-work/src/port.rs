use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canonical::{validate_portable_model, validate_portable_value};
use crate::diagnostic::{DiagnosticCode, ModelError, ModelResult};
use crate::ids::{NodeId, PortId};

pub const INTERACTION_CAPABILITY_UNARY: &str = "plurora.interaction.capability-unary/v1";
pub const INTERACTION_CAPABILITY_STREAM: &str = "plurora.interaction.capability-stream/v1";
pub const INTERACTION_EVENT_STREAM: &str = "plurora.interaction.event-stream/v1";
pub const INTERACTION_DUPLEX_STREAM: &str = "plurora.interaction.duplex-stream/v1";
pub const INTERACTION_ARTIFACT: &str = "plurora.interaction.artifact/v1";
pub const INTERACTION_SNAPSHOT: &str = "plurora.interaction.snapshot/v1";
pub const INTERACTION_ENDPOINT: &str = "plurora.interaction.endpoint/v1";

pub const KNOWN_INTERACTION_MODELS: &[&str] = &[
    INTERACTION_CAPABILITY_UNARY,
    INTERACTION_CAPABILITY_STREAM,
    INTERACTION_EVENT_STREAM,
    INTERACTION_DUPLEX_STREAM,
    INTERACTION_ARTIFACT,
    INTERACTION_SNAPSHOT,
    INTERACTION_ENDPOINT,
];

#[derive(
    Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[schemars(transparent)]
pub struct InteractionModelId(pub String);

impl InteractionModelId {
    pub fn is_known(&self) -> bool {
        KNOWN_INTERACTION_MODELS.contains(&self.0.as_str())
    }

    pub fn validate(&self) -> ModelResult<()> {
        validate_protocol_text(&self.0, "interaction model")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PortContract {
    pub protocol_id: String,
    pub interface_id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<String>,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum BindingPhase {
    Authoring,
    Installation,
    Launch,
    Runtime,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityPolicy {
    Required,
    DegradedWithout,
    Optional,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum EffectClass {
    Pure,
    DeterministicStateful,
    RecordedNondeterministic,
    ExternalEffecting,
    RealtimeBestEffort,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PortMultiplicity {
    pub min: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u16>,
}

impl PortMultiplicity {
    pub fn validate(&self) -> ModelResult<()> {
        if self.max.is_some_and(|max| self.min > max) {
            return Err(ModelError::new(
                DiagnosticCode::PortIncompatible,
                "port multiplicity has an invalid minimum/maximum range",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PortRole {
    Import {
        multiplicity: PortMultiplicity,
        latest_binding_phase: BindingPhase,
        availability: AvailabilityPolicy,
        accepted_effects: Vec<EffectClass>,
    },
    Export {
        multiplicity: PortMultiplicity,
        effect_class: EffectClass,
    },
}

impl PortRole {
    pub fn multiplicity(&self) -> &PortMultiplicity {
        match self {
            Self::Import { multiplicity, .. } | Self::Export { multiplicity, .. } => multiplicity,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct TransportRequirements {
    #[serde(default)]
    pub same_process: bool,
    #[serde(default)]
    pub local_only: bool,
    #[serde(default)]
    pub ordered: bool,
    #[serde(default)]
    pub reliable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_latency_class: Option<String>,
    #[serde(default)]
    pub large_payload: bool,
    #[serde(default)]
    pub shared_memory_allowed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_classes: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, Value>,
}

impl TransportRequirements {
    pub fn validate(&self) -> ModelResult<()> {
        if let Some(latency) = &self.max_latency_class {
            validate_protocol_text(latency, "latency class")?;
        }
        let mut classes = BTreeSet::new();
        for class in &self.allowed_classes {
            validate_protocol_text(class, "transport class")?;
            if !classes.insert(class) {
                return Err(ModelError::new(
                    DiagnosticCode::PortIncompatible,
                    "transport requirements contain a duplicate class",
                ));
            }
        }
        for (key, value) in &self.extensions {
            validate_protocol_text(key, "transport extension")?;
            validate_portable_value(value)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct TransportPolicy {
    #[serde(default)]
    pub requirements: TransportRequirements,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferred_classes: Vec<String>,
}

impl TransportPolicy {
    pub fn validate(&self) -> ModelResult<()> {
        self.requirements.validate()?;
        for class in &self.preferred_classes {
            validate_protocol_text(class, "preferred transport class")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PortDescriptor {
    pub port_id: PortId,
    pub contract: PortContract,
    pub interaction: InteractionModelId,
    pub role: PortRole,
    #[serde(default)]
    pub transport: TransportRequirements,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl PortDescriptor {
    pub fn validate(&self) -> ModelResult<()> {
        validate_protocol_text(&self.contract.protocol_id, "port protocol id")?;
        validate_protocol_text(&self.contract.interface_id, "port interface id")?;
        self.interaction.validate()?;
        self.role.multiplicity().validate()?;
        match &self.role {
            PortRole::Import {
                accepted_effects, ..
            } => {
                VersionReq::parse(&self.contract.version).map_err(|_| {
                    ModelError::new(
                        DiagnosticCode::PortIncompatible,
                        "import port version is not a semantic-version requirement",
                    )
                })?;
                if accepted_effects.is_empty()
                    || accepted_effects.iter().collect::<BTreeSet<_>>().len()
                        != accepted_effects.len()
                {
                    return Err(ModelError::new(
                        DiagnosticCode::PortIncompatible,
                        "import port must declare a unique non-empty accepted effect set",
                    ));
                }
            }
            PortRole::Export { .. } => {
                Version::parse(&self.contract.version).map_err(|_| {
                    ModelError::new(
                        DiagnosticCode::PortIncompatible,
                        "export port version is not an exact semantic version",
                    )
                })?;
            }
        }
        let mut profiles = BTreeSet::new();
        for profile in &self.contract.profiles {
            validate_protocol_text(profile, "port profile")?;
            if !profiles.insert(profile) {
                return Err(ModelError::new(
                    DiagnosticCode::PortIncompatible,
                    "port contract contains a duplicate profile",
                ));
            }
        }
        self.transport.validate()?;
        validate_portable_model(self)
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct PortEndpoint {
    pub node_id: NodeId,
    pub port_id: PortId,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PortDirection {
    Import,
    Export,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SelectedTransport {
    pub class_id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, Value>,
}

impl SelectedTransport {
    pub fn validate(&self) -> ModelResult<()> {
        validate_protocol_text(&self.class_id, "selected transport")?;
        validate_portable_model(self)
    }
}

pub fn check_transport_policy_compatibility(
    provider: &TransportRequirements,
    consumer: &TransportRequirements,
    policy: &TransportPolicy,
) -> ModelResult<()> {
    policy.validate()?;
    check_transport_requirement_sets(&[provider, consumer, &policy.requirements])
}

fn check_transport_requirement_sets(requirements: &[&TransportRequirements]) -> ModelResult<()> {
    for requirement in requirements {
        requirement.validate()?;
        if !requirement.extensions.is_empty() {
            return Err(ModelError::new(
                DiagnosticCode::PortIncompatible,
                "binding uses a transport extension without a declared implementation",
            ));
        }
    }

    let mut candidates: Option<BTreeSet<&str>> = None;
    for requirement in requirements {
        if requirement.allowed_classes.is_empty() {
            continue;
        }
        let allowed = requirement
            .allowed_classes
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        candidates = Some(match candidates {
            None => allowed,
            Some(existing) => existing.intersection(&allowed).copied().collect(),
        });
    }
    if candidates.is_some_and(|values| values.is_empty()) {
        return Err(ModelError::new(
            DiagnosticCode::PortIncompatible,
            "port and binding transport constraints have no common class",
        ));
    }
    Ok(())
}

pub fn check_port_compatibility(
    provider: &PortDescriptor,
    consumer: &PortDescriptor,
) -> ModelResult<()> {
    provider.validate()?;
    consumer.validate()?;
    let PortRole::Export {
        effect_class,
        multiplicity: _,
    } = provider.role
    else {
        return Err(ModelError::new(
            DiagnosticCode::PortIncompatible,
            "binding provider is not an export port",
        ));
    };
    let PortRole::Import {
        ref accepted_effects,
        ..
    } = consumer.role
    else {
        return Err(ModelError::new(
            DiagnosticCode::PortIncompatible,
            "binding consumer is not an import port",
        ));
    };
    check_transport_requirement_sets(&[&provider.transport, &consumer.transport])?;
    if !provider.interaction.is_known() || !consumer.interaction.is_known() {
        return Err(ModelError::new(
            DiagnosticCode::UnsupportedInteraction,
            "binding uses an interaction model without an implementation or adapter",
        ));
    }
    let export_version = Version::parse(&provider.contract.version).map_err(|_| {
        ModelError::new(
            DiagnosticCode::PortIncompatible,
            "provider version is not an exact semantic version",
        )
    })?;
    let import_requirement = VersionReq::parse(&consumer.contract.version).map_err(|_| {
        ModelError::new(
            DiagnosticCode::PortIncompatible,
            "consumer version is not a semantic-version requirement",
        )
    })?;
    let provider_profiles = provider
        .contract
        .profiles
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let consumer_profiles = consumer
        .contract
        .profiles
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if provider.contract.protocol_id != consumer.contract.protocol_id
        || provider.contract.interface_id != consumer.contract.interface_id
        || provider.interaction != consumer.interaction
        || !import_requirement.matches(&export_version)
        || !consumer_profiles.is_subset(&provider_profiles)
        || !accepted_effects.contains(&effect_class)
    {
        return Err(ModelError::new(
            DiagnosticCode::PortIncompatible,
            "provider and consumer port contracts are incompatible",
        ));
    }
    Ok(())
}

fn validate_protocol_text(value: &str, label: &str) -> ModelResult<()> {
    if value.trim().is_empty()
        || value.chars().any(char::is_whitespace)
        || value.chars().any(char::is_control)
    {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            format!("{label} is empty or contains whitespace/control characters"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(role: PortRole, version: &str) -> PortDescriptor {
        PortDescriptor {
            port_id: PortId::parse("save").unwrap(),
            contract: PortContract {
                protocol_id: "game.save".to_string(),
                interface_id: "store".to_string(),
                version: version.to_string(),
                profiles: vec!["game.save/default/v1".to_string()],
            },
            interaction: InteractionModelId(INTERACTION_CAPABILITY_UNARY.to_string()),
            role,
            transport: TransportRequirements::default(),
            annotations: BTreeMap::new(),
        }
    }

    #[test]
    fn compatible_ports_require_version_profile_interaction_and_effect_match() {
        let provider = port(
            PortRole::Export {
                multiplicity: PortMultiplicity {
                    min: 0,
                    max: Some(8),
                },
                effect_class: EffectClass::DeterministicStateful,
            },
            "1.2.0",
        );
        let consumer = port(
            PortRole::Import {
                multiplicity: PortMultiplicity {
                    min: 1,
                    max: Some(1),
                },
                latest_binding_phase: BindingPhase::Installation,
                availability: AvailabilityPolicy::Required,
                accepted_effects: vec![EffectClass::DeterministicStateful],
            },
            "^1.0",
        );
        check_port_compatibility(&provider, &consumer).unwrap();
        PortMultiplicity {
            min: 0,
            max: Some(0),
        }
        .validate()
        .unwrap();
    }

    #[test]
    fn incompatible_port_version_is_rejected() {
        let provider = port(
            PortRole::Export {
                multiplicity: PortMultiplicity {
                    min: 0,
                    max: Some(1),
                },
                effect_class: EffectClass::Pure,
            },
            "1.2.0",
        );
        let consumer = port(
            PortRole::Import {
                multiplicity: PortMultiplicity {
                    min: 1,
                    max: Some(1),
                },
                latest_binding_phase: BindingPhase::Authoring,
                availability: AvailabilityPolicy::Required,
                accepted_effects: vec![EffectClass::Pure],
            },
            "^2.0",
        );
        assert_eq!(
            check_port_compatibility(&provider, &consumer)
                .unwrap_err()
                .code,
            DiagnosticCode::PortIncompatible
        );
    }

    #[test]
    fn unknown_interactions_round_trip_but_cannot_bind() {
        let mut provider = port(
            PortRole::Export {
                multiplicity: PortMultiplicity {
                    min: 0,
                    max: Some(1),
                },
                effect_class: EffectClass::Pure,
            },
            "1.0.0",
        );
        let mut consumer = port(
            PortRole::Import {
                multiplicity: PortMultiplicity {
                    min: 1,
                    max: Some(1),
                },
                latest_binding_phase: BindingPhase::Authoring,
                availability: AvailabilityPolicy::Required,
                accepted_effects: vec![EffectClass::Pure],
            },
            "^1.0",
        );
        provider.interaction = InteractionModelId("example.interaction/custom/v9".to_string());
        consumer.interaction = provider.interaction.clone();
        provider.validate().unwrap();
        let round_trip: PortDescriptor =
            serde_json::from_value(serde_json::to_value(&provider).unwrap()).unwrap();
        assert_eq!(round_trip.interaction, provider.interaction);
        assert_eq!(
            check_port_compatibility(&provider, &consumer)
                .unwrap_err()
                .code,
            DiagnosticCode::UnsupportedInteraction
        );
    }

    #[test]
    fn unknown_transport_extensions_round_trip_but_cannot_bind() {
        let mut provider = port(
            PortRole::Export {
                multiplicity: PortMultiplicity {
                    min: 0,
                    max: Some(1),
                },
                effect_class: EffectClass::Pure,
            },
            "1.0.0",
        );
        let consumer = port(
            PortRole::Import {
                multiplicity: PortMultiplicity {
                    min: 1,
                    max: Some(1),
                },
                latest_binding_phase: BindingPhase::Authoring,
                availability: AvailabilityPolicy::Required,
                accepted_effects: vec![EffectClass::Pure],
            },
            "^1.0",
        );
        provider.transport.extensions.insert(
            "example.transport/zero-copy/v1".to_string(),
            serde_json::json!({"required": true}),
        );
        provider.validate().unwrap();
        assert_eq!(
            check_port_compatibility(&provider, &consumer)
                .unwrap_err()
                .code,
            DiagnosticCode::PortIncompatible
        );
    }

    #[test]
    fn binding_transport_policy_must_intersect_both_ports() {
        let provider = TransportRequirements {
            allowed_classes: vec!["inproc".to_string()],
            ..TransportRequirements::default()
        };
        let consumer = provider.clone();
        let policy = TransportPolicy {
            requirements: TransportRequirements {
                allowed_classes: vec!["tcp".to_string()],
                ..TransportRequirements::default()
            },
            preferred_classes: Vec::new(),
        };
        assert_eq!(
            check_transport_policy_compatibility(&provider, &consumer, &policy)
                .unwrap_err()
                .code,
            DiagnosticCode::PortIncompatible
        );
    }
}
