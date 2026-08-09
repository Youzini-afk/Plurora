use std::collections::BTreeSet;

use plurora_core::ArtifactDescriptor;
use schemars::JsonSchema;
use semver::VersionReq;
use serde::{Deserialize, Serialize};

use crate::canonical::{validate_artifact_descriptor, validate_portable_model, ArtifactModel};
use crate::diagnostic::{DiagnosticCode, ModelError, ModelResult};
use crate::state::StatePortability;

pub const RIGHTS_DECLARATION_TYPE_URI: &str = "urn:plurora:rights-declaration:v1";
pub const TRANSPARENCY_DECLARATION_TYPE_URI: &str = "urn:plurora:transparency-declaration:v1";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolRequirement {
    pub protocol_id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<String>,
}

impl ProtocolRequirement {
    pub fn validate(&self) -> ModelResult<()> {
        if self.protocol_id.trim().is_empty()
            || self.protocol_id.chars().any(char::is_whitespace)
            || VersionReq::parse(&self.version).is_err()
        {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "protocol requirement has an invalid id or version requirement",
            ));
        }
        let profiles = self.profiles.iter().collect::<BTreeSet<_>>();
        if profiles.len() != self.profiles.len()
            || self
                .profiles
                .iter()
                .any(|profile| profile.trim().is_empty())
        {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "protocol requirement contains an invalid or duplicate profile",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RightDisposition {
    Allowed,
    Denied,
    RequiresEntitlement,
    Unspecified,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RightsDeclaration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_expression: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terms_uri: Option<String>,
    pub install: RightDisposition,
    pub execute: RightDisposition,
    pub backup: RightDisposition,
    pub export_state: RightDisposition,
    pub copy_across_hosts: RightDisposition,
    pub redistribute_artifacts: RightDisposition,
    pub modify: RightDisposition,
    pub derive: RightDisposition,
    pub modding: RightDisposition,
    pub dedicated_server: RightDisposition,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entitlement_requirements: Vec<ProtocolRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<ArtifactDescriptor>,
}

impl RightsDeclaration {
    pub fn validate(&self) -> ModelResult<()> {
        if self
            .license_expression
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
            || self
                .terms_uri
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
        {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "rights declaration contains an empty license or terms URI",
            ));
        }
        for requirement in &self.entitlement_requirements {
            requirement.validate()?;
        }
        validate_unique_artifacts(&self.evidence_refs)?;
        validate_portable_model(self)
    }
}

impl ArtifactModel for RightsDeclaration {
    const ARTIFACT_TYPE_URI: &'static str = RIGHTS_DECLARATION_TYPE_URI;

    fn validate(&self) -> ModelResult<()> {
        RightsDeclaration::validate(self)
    }

    fn referenced_artifacts(&self) -> Vec<&ArtifactDescriptor> {
        self.evidence_refs.iter().collect()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceVisibility {
    Open,
    SourceAvailable,
    Closed,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStatus {
    Claimed,
    Verified,
    Refuted,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct TransparencyDeclaration {
    pub source_visibility: SourceVisibility,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<ArtifactDescriptor>,
    pub reproducible_build_claim: ClaimStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sbom_refs: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance_refs: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signature_refs: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telemetry_disclosures: Vec<String>,
    pub state_portability: StatePortability,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<ArtifactDescriptor>,
}

impl TransparencyDeclaration {
    pub fn validate(&self) -> ModelResult<()> {
        let references = self.all_references();
        validate_unique_artifacts(&references.into_iter().cloned().collect::<Vec<_>>())?;
        if self
            .telemetry_disclosures
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "transparency declaration contains an empty telemetry disclosure",
            ));
        }
        validate_portable_model(self)
    }

    fn all_references(&self) -> Vec<&ArtifactDescriptor> {
        self.source_refs
            .iter()
            .chain(&self.sbom_refs)
            .chain(&self.provenance_refs)
            .chain(&self.signature_refs)
            .chain(&self.evidence_refs)
            .collect()
    }
}

impl ArtifactModel for TransparencyDeclaration {
    const ARTIFACT_TYPE_URI: &'static str = TRANSPARENCY_DECLARATION_TYPE_URI;

    fn validate(&self) -> ModelResult<()> {
        TransparencyDeclaration::validate(self)
    }

    fn referenced_artifacts(&self) -> Vec<&ArtifactDescriptor> {
        self.all_references()
    }
}

fn validate_unique_artifacts(references: &[ArtifactDescriptor]) -> ModelResult<()> {
    let mut digests = BTreeSet::new();
    for reference in references {
        validate_artifact_descriptor(reference)?;
        if !digests.insert(reference.digest.as_str()) {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "declaration contains a duplicate evidence reference",
            ));
        }
    }
    Ok(())
}
