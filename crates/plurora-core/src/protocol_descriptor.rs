use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_DESCRIPTOR_TYPE_URI: &str = "urn:plurora:protocol-descriptor:v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolMaturity {
    Experimental,
    Candidate,
    Stable,
    Deprecated,
    LegacyAdapter,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolSchemaKind {
    JsonSchema,
    WitWorld,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolSchemaReference {
    pub id: String,
    pub version: String,
    pub kind: ProtocolSchemaKind,
    pub uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolDocumentReference {
    pub uri: String,
    pub media_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolAuthorityRequirement {
    pub authority: String,
    pub scope: String,
    pub operations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolConformanceVector {
    pub id: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolCompatibilityProfile {
    pub id: String,
    pub version: String,
    pub maturity: ProtocolMaturity,
    pub description: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolMigrationKind {
    IdentityAdapter,
    SemanticAdapter,
    DataMigration,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolMigration {
    pub from_protocol_id: String,
    pub from_version: String,
    pub to_version: String,
    pub kind: ProtocolMigrationKind,
    pub adapter_id: String,
    pub lossless: bool,
    pub instructions: ProtocolDocumentReference,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolImplementationClaim {
    pub implementation_id: String,
    pub provider: String,
    pub version: String,
    pub profiles: Vec<String>,
    pub conformance_vectors: Vec<String>,
    #[serde(default)]
    pub test_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolDescriptor {
    pub descriptor_type_uri: String,
    pub protocol_id: String,
    pub version: String,
    pub maturity: ProtocolMaturity,
    pub schemas: Vec<ProtocolSchemaReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wit_worlds: Vec<ProtocolSchemaReference>,
    pub semantic_specification: ProtocolDocumentReference,
    pub lifecycle: ProtocolDocumentReference,
    pub error_model: ProtocolDocumentReference,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authority_requirements: Vec<ProtocolAuthorityRequirement>,
    pub conformance_vectors: Vec<ProtocolConformanceVector>,
    pub compatibility_profiles: Vec<ProtocolCompatibilityProfile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub migrations: Vec<ProtocolMigration>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conforming_implementations: Vec<ProtocolImplementationClaim>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolSelection {
    pub protocol_id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct NegotiatedProtocol {
    pub protocol_id: String,
    pub requested_version: String,
    pub negotiated_version: String,
    pub maturity: ProtocolMaturity,
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_id: Option<String>,
}
