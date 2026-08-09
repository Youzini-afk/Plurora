use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    is_secret_field_name, looks_like_raw_secret, ArtifactDescriptor, CapabilityDescriptor,
    CapabilityId, ContractMode, PackageEntry, PackageManifest, SecretRef, SurfaceContribution,
};

pub const COMPONENT_DESCRIPTOR_TYPE_URI: &str = "urn:plurora:component-descriptor:v1";
pub const COMPONENT_BEHAVIOR_TYPE_URI: &str = "urn:plurora:component-behavior:v1";
pub const PACKAGE_ENVELOPE_TYPE_URI: &str = "urn:plurora:package-envelope:v1";
pub const PACKAGE_MANIFEST_TYPE_URI: &str = "urn:plurora:package-manifest:v1";
pub const PACKAGE_ENTRY_TYPE_URI: &str = "urn:plurora:package-entry:v1";
pub const PACKAGED_PROTOCOL_TYPE_URI: &str = "urn:plurora:packaged-protocol:v1";
pub const PACKAGED_SURFACE_TYPE_URI: &str = "urn:plurora:packaged-surface:v1";
pub const SCHEMA_CONTRIBUTION_TYPE_URI: &str = "urn:plurora:schema-contribution:v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComponentTrustClass {
    SandboxedComponent,
    IsolatedProcess,
    RemoteBoundary,
    TrustedNative,
    StaticResource,
    ForeignCapsule,
}

impl Default for ComponentTrustClass {
    fn default() -> Self {
        Self::ForeignCapsule
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComponentClaimStatus {
    Declared,
    LegacyAdapted,
    ForeignCapsule,
}

impl Default for ComponentClaimStatus {
    fn default() -> Self {
        Self::ForeignCapsule
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ComponentBoundaryClaims {
    pub process_failure_isolation: bool,
    pub network_isolation: bool,
    pub filesystem_isolation: bool,
    pub resource_limits_enforced: bool,
    pub remote_identity: bool,
    pub tenancy_isolation: bool,
    pub revocation_enforced: bool,
    pub no_code_execution: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolImplementationDeclaration {
    pub protocol_id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conformance_vectors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ComponentDeclaration {
    pub id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_ids: Vec<CapabilityId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocol_implementations: Vec<ProtocolImplementationDeclaration>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content_roots: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surface_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PackagedProtocolDescriptor {
    pub implementation: ProtocolImplementationDeclaration,
    pub artifact: ArtifactDescriptor,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PackagedSurfaceDescriptor {
    pub surface_id: String,
    pub version: String,
    pub artifact: ArtifactDescriptor,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct NamedPackageArtifact {
    pub id: String,
    pub descriptor: ArtifactDescriptor,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ComponentDescriptor {
    pub component_id: String,
    pub version: String,
    pub artifact: ArtifactDescriptor,
    pub behavior: ArtifactDescriptor,
    pub entry_kind: String,
    pub trust_class: ComponentTrustClass,
    pub claim_status: ComponentClaimStatus,
    pub enforced_boundaries: ComponentBoundaryClaims,
    pub capability_ids: Vec<CapabilityId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocol_implementations: Vec<PackagedProtocolDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content_roots: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<PackagedSurfaceDescriptor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentArtifactPayload {
    pub component_id: String,
    pub version: String,
    pub behavior: ArtifactDescriptor,
    pub entry_kind: String,
    pub entry: PackageEntry,
    pub contract: ContractMode,
    pub trust_class: ComponentTrustClass,
    pub claim_status: ComponentClaimStatus,
    pub enforced_boundaries: ComponentBoundaryClaims,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocol_artifacts: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content_roots: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surface_artifacts: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

pub fn decode_component_artifact_payload(
    descriptor: &ArtifactDescriptor,
    bytes: &[u8],
) -> anyhow::Result<ComponentArtifactPayload> {
    anyhow::ensure!(
        descriptor.artifact_type_uri == COMPONENT_DESCRIPTOR_TYPE_URI
            && descriptor.media_type == "application/json",
        "component artifact descriptor has an unexpected type or media type"
    );
    let payload: ComponentArtifactPayload = serde_json::from_slice(bytes)?;
    let canonical = canonical_json_bytes(&payload)?;
    anyhow::ensure!(
        canonical == bytes,
        "component artifact JSON is not canonical"
    );
    anyhow::ensure!(
        descriptor.digest == sha256(bytes) && descriptor.size_bytes == bytes.len() as u64,
        "component artifact descriptor does not match its payload"
    );
    anyhow::ensure!(
        !payload.component_id.trim().is_empty()
            && !payload.version.trim().is_empty()
            && !payload.entry_kind.trim().is_empty(),
        "component artifact identity fields must not be empty"
    );
    anyhow::ensure!(
        payload.entry_kind == package_entry_kind(&payload.entry),
        "component artifact entry kind differs from its typed entry"
    );
    let expected_trust = component_trust_class_for(&payload.contract, &payload.entry);
    anyhow::ensure!(
        payload.trust_class == expected_trust,
        "component artifact trust class differs from its typed entry and contract"
    );
    anyhow::ensure!(
        payload.enforced_boundaries == boundary_claims(expected_trust),
        "component artifact boundary claims differ from its derived trust class"
    );
    if expected_trust == ComponentTrustClass::ForeignCapsule {
        anyhow::ensure!(
            payload.claim_status == ComponentClaimStatus::ForeignCapsule
                && payload.protocol_artifacts.is_empty(),
            "foreign component artifact contains executable contract claims"
        );
    } else {
        anyhow::ensure!(
            payload.claim_status != ComponentClaimStatus::ForeignCapsule,
            "contract component artifact uses the foreign claim status"
        );
    }
    validate_component_payload_value(&serde_json::to_value((
        &payload.entry,
        &payload.annotations,
    ))?)?;
    validate_component_payload_descriptor(&payload.behavior)?;
    anyhow::ensure!(
        payload.behavior.artifact_type_uri == COMPONENT_BEHAVIOR_TYPE_URI,
        "component artifact behavior has an unexpected type"
    );
    let mut required_references = BTreeSet::from([payload.behavior.digest.as_str()]);
    for referenced in &payload.protocol_artifacts {
        anyhow::ensure!(
            referenced.artifact_type_uri == PACKAGED_PROTOCOL_TYPE_URI
                && referenced.media_type == "application/json",
            "component protocol reference has an unexpected type or media type"
        );
        validate_component_payload_descriptor(referenced)?;
        anyhow::ensure!(
            required_references.insert(referenced.digest.as_str()),
            "component artifact payload contains a duplicate referenced digest"
        );
    }
    for referenced in &payload.content_roots {
        validate_component_payload_descriptor(referenced)?;
        anyhow::ensure!(
            required_references.insert(referenced.digest.as_str()),
            "component artifact payload contains a duplicate referenced digest"
        );
    }
    for referenced in &payload.surface_artifacts {
        anyhow::ensure!(
            referenced.artifact_type_uri == PACKAGED_SURFACE_TYPE_URI
                && referenced.media_type == "application/json",
            "component surface reference has an unexpected type or media type"
        );
        validate_component_payload_descriptor(referenced)?;
        anyhow::ensure!(
            required_references.insert(referenced.digest.as_str()),
            "component artifact payload contains a duplicate referenced digest"
        );
    }
    let mut descriptor_references = BTreeSet::new();
    for digest in &descriptor.references {
        validate_sha256(digest)?;
        anyhow::ensure!(
            descriptor_references.insert(digest.as_str()),
            "component artifact descriptor contains a duplicate reference"
        );
    }
    anyhow::ensure!(
        required_references == descriptor_references,
        "component artifact descriptor references differ from its payload"
    );
    anyhow::ensure!(
        descriptor
            .annotations
            .get("component_id")
            .and_then(Value::as_str)
            == Some(payload.component_id.as_str()),
        "component artifact descriptor identity annotation differs from its payload"
    );
    Ok(payload)
}

fn validate_component_payload_descriptor(descriptor: &ArtifactDescriptor) -> anyhow::Result<()> {
    anyhow::ensure!(
        !descriptor.artifact_type_uri.trim().is_empty() && !descriptor.media_type.trim().is_empty(),
        "component payload reference has an empty type or media type"
    );
    validate_sha256(&descriptor.digest)?;
    let mut references = BTreeSet::new();
    for reference in &descriptor.references {
        validate_sha256(reference)?;
        anyhow::ensure!(
            reference != &descriptor.digest && references.insert(reference.as_str()),
            "component payload reference contains a duplicate or self reference"
        );
    }
    validate_component_payload_value(&serde_json::to_value(&descriptor.annotations)?)?;
    Ok(())
}

fn validate_component_payload_value(value: &Value) -> anyhow::Result<()> {
    fn walk(value: &Value, field_name: Option<&str>) -> anyhow::Result<()> {
        match value {
            Value::Array(values) => {
                for value in values {
                    walk(value, field_name)?;
                }
            }
            Value::Object(object) => {
                for (key, value) in object {
                    anyhow::ensure!(
                        !looks_like_raw_secret(key) && !looks_like_host_path(key, None),
                        "component artifact contains a non-portable object key"
                    );
                    if secret_value_field(key) {
                        let valid = match value {
                            Value::Null => true,
                            Value::String(reference) => SecretRef::is_valid_ref(reference),
                            Value::Array(references) => references
                                .iter()
                                .all(|value| value.as_str().is_some_and(SecretRef::is_valid_ref)),
                            _ => false,
                        };
                        anyhow::ensure!(valid, "component artifact contains a raw secret field");
                    }
                    walk(value, Some(key))?;
                }
            }
            Value::String(value) => {
                anyhow::ensure!(
                    !looks_like_raw_secret(value),
                    "component artifact contains a value that looks like a raw secret"
                );
                anyhow::ensure!(
                    !looks_like_host_path(value, field_name),
                    "component artifact contains a host-local filesystem path"
                );
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
        Ok(())
    }
    walk(value, None)
}

fn secret_value_field(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    is_secret_field_name(key)
        && !lower.ends_with("_id")
        && !lower.ends_with("_ids")
        && !lower.ends_with("_policy")
        && !lower.ends_with("_requirements")
        && !lower.ends_with("_imports")
        && !lower.ends_with("_scope")
}

fn looks_like_host_path(value: &str, field_name: Option<&str>) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    let bytes = trimmed.as_bytes();
    let drive_absolute = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\');
    let lower = trimmed.to_ascii_lowercase();
    let is_json_pointer = field_name.is_some_and(|field| {
        let field = field.to_ascii_lowercase();
        (field.contains("json_pointer") || field.contains("json-pointer"))
            && trimmed.starts_with('/')
    });
    drive_absolute
        || (trimmed.starts_with('/') && !is_json_pointer)
        || trimmed.starts_with("\\\\")
        || trimmed.starts_with("~/")
        || trimmed.starts_with("~\\")
        || trimmed == ".."
        || trimmed.starts_with("../")
        || trimmed.starts_with("..\\")
        || trimmed.contains("/../")
        || trimmed.contains("\\..\\")
        || lower.starts_with("file:")
        || lower.starts_with("unix:")
}

fn package_entry_kind(entry: &PackageEntry) -> &'static str {
    match entry {
        PackageEntry::RustInproc { .. } => "rust_inproc",
        PackageEntry::Subprocess { .. } => "subprocess",
        PackageEntry::Wasm { .. } => "wasm",
        PackageEntry::Remote { .. } => "remote",
        PackageEntry::SurfaceBundle { .. } => "surface_bundle",
    }
}

fn component_trust_class_for(contract: &ContractMode, entry: &PackageEntry) -> ComponentTrustClass {
    if *contract == ContractMode::None {
        return ComponentTrustClass::ForeignCapsule;
    }
    match entry {
        PackageEntry::RustInproc { .. } => ComponentTrustClass::TrustedNative,
        PackageEntry::Subprocess { .. } => ComponentTrustClass::IsolatedProcess,
        PackageEntry::Wasm { .. } => ComponentTrustClass::SandboxedComponent,
        PackageEntry::Remote { .. } => ComponentTrustClass::RemoteBoundary,
        PackageEntry::SurfaceBundle { .. } => ComponentTrustClass::StaticResource,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PackageEnvelopeDescriptor {
    pub package_id: String,
    pub package_version: String,
    pub artifact: ArtifactDescriptor,
    pub manifest: ArtifactDescriptor,
    pub components: Vec<ComponentDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocols: Vec<PackagedProtocolDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content_roots: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<PackagedSurfaceDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<NamedPackageArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ComponentLockPin {
    pub component_id: String,
    pub digest: String,
    pub behavior_digest: String,
    pub trust_class: ComponentTrustClass,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProtocolProfilePin {
    pub protocol_id: String,
    pub version: String,
    pub profile: String,
}

pub(crate) fn validate_package_lock_pins(
    component_pins: &[ComponentLockPin],
    protocol_profile_pins: &[ProtocolProfilePin],
    content_roots: &[ArtifactDescriptor],
) -> anyhow::Result<()> {
    let mut components = BTreeSet::new();
    for component in component_pins {
        anyhow::ensure!(
            !component.component_id.trim().is_empty(),
            "component pin id must not be empty"
        );
        anyhow::ensure!(
            components.insert(component.component_id.as_str()),
            "duplicate component pin '{}'",
            component.component_id
        );
        validate_sha256(&component.digest)?;
        validate_sha256(&component.behavior_digest)?;
    }
    let mut profiles = BTreeSet::new();
    for profile in protocol_profile_pins {
        anyhow::ensure!(
            !profile.protocol_id.trim().is_empty()
                && !profile.version.trim().is_empty()
                && !profile.profile.trim().is_empty(),
            "protocol profile pin fields must not be empty"
        );
        anyhow::ensure!(
            profiles.insert((
                profile.protocol_id.as_str(),
                profile.version.as_str(),
                profile.profile.as_str(),
            )),
            "duplicate protocol profile pin '{}@{}:{}'",
            profile.protocol_id,
            profile.version,
            profile.profile
        );
    }
    let mut roots = BTreeSet::new();
    for root in content_roots {
        validate_sha256(&root.digest)?;
        anyhow::ensure!(
            roots.insert(root.digest.as_str()),
            "duplicate content root '{}'",
            root.digest
        );
    }
    Ok(())
}

impl ComponentLockPin {
    pub fn from_descriptor(descriptor: &ComponentDescriptor) -> Self {
        Self {
            component_id: descriptor.component_id.clone(),
            digest: descriptor.artifact.digest.clone(),
            behavior_digest: descriptor.behavior.digest.clone(),
            trust_class: descriptor.trust_class,
        }
    }
}

pub fn package_envelope_for_manifest(
    manifest: &PackageManifest,
) -> anyhow::Result<PackageEnvelopeDescriptor> {
    let manifest_artifact = json_artifact(
        PACKAGE_MANIFEST_TYPE_URI,
        manifest,
        Vec::new(),
        annotation("package_id", &manifest.id),
    )?;
    let components = component_descriptors_for_manifest(manifest)?;
    let protocols = unique_protocols(&components);
    let content_roots = unique_artifacts(
        components
            .iter()
            .flat_map(|component| component.content_roots.iter().cloned()),
    );
    let surfaces = unique_surfaces(
        components
            .iter()
            .flat_map(|component| component.surfaces.iter().cloned()),
    );
    let mut artifacts = vec![NamedPackageArtifact {
        id: "entry".to_string(),
        descriptor: json_artifact(
            PACKAGE_ENTRY_TYPE_URI,
            &manifest.entry,
            Vec::new(),
            BTreeMap::new(),
        )?,
    }];
    for schema in &manifest.contributes.schemas {
        artifacts.push(NamedPackageArtifact {
            id: schema.id.clone(),
            descriptor: json_artifact(
                SCHEMA_CONTRIBUTION_TYPE_URI,
                schema,
                Vec::new(),
                annotation("schema_id", &schema.id),
            )?,
        });
    }
    artifacts.sort_by(|a, b| a.id.cmp(&b.id));

    #[derive(Serialize)]
    struct EnvelopeMaterial<'a> {
        package_id: &'a str,
        package_version: &'a str,
        manifest_digest: &'a str,
        component_digests: Vec<&'a str>,
        protocol_digests: Vec<&'a str>,
        content_root_digests: Vec<&'a str>,
        surface_digests: Vec<&'a str>,
        artifact_digests: Vec<&'a str>,
    }
    let material = EnvelopeMaterial {
        package_id: &manifest.id,
        package_version: &manifest.version,
        manifest_digest: &manifest_artifact.digest,
        component_digests: components
            .iter()
            .map(|component| component.artifact.digest.as_str())
            .collect(),
        protocol_digests: protocols
            .iter()
            .map(|protocol| protocol.artifact.digest.as_str())
            .collect(),
        content_root_digests: content_roots
            .iter()
            .map(|root| root.digest.as_str())
            .collect(),
        surface_digests: surfaces
            .iter()
            .map(|surface| surface.artifact.digest.as_str())
            .collect(),
        artifact_digests: artifacts
            .iter()
            .map(|artifact| artifact.descriptor.digest.as_str())
            .collect(),
    };
    let references = std::iter::once(manifest_artifact.digest.clone())
        .chain(
            components
                .iter()
                .map(|component| component.artifact.digest.clone()),
        )
        .chain(
            protocols
                .iter()
                .map(|protocol| protocol.artifact.digest.clone()),
        )
        .chain(content_roots.iter().map(|root| root.digest.clone()))
        .chain(
            surfaces
                .iter()
                .map(|surface| surface.artifact.digest.clone()),
        )
        .chain(
            artifacts
                .iter()
                .map(|artifact| artifact.descriptor.digest.clone()),
        )
        .collect();
    let artifact = json_artifact(
        PACKAGE_ENVELOPE_TYPE_URI,
        &material,
        references,
        annotation("package_id", &manifest.id),
    )?;
    Ok(PackageEnvelopeDescriptor {
        package_id: manifest.id.clone(),
        package_version: manifest.version.clone(),
        artifact,
        manifest: manifest_artifact,
        components,
        protocols,
        content_roots,
        surfaces,
        artifacts,
    })
}

pub fn component_descriptors_for_manifest(
    manifest: &PackageManifest,
) -> anyhow::Result<Vec<ComponentDescriptor>> {
    let explicit = manifest.entry.component.is_some();
    let declaration = manifest
        .entry
        .component
        .clone()
        .unwrap_or_else(|| legacy_component_declaration(manifest));
    Ok(vec![build_component_descriptor(
        manifest,
        declaration,
        explicit,
    )?])
}

pub fn component_trust_class(manifest: &PackageManifest) -> ComponentTrustClass {
    component_trust_class_for(&manifest.entry.contract, &manifest.entry.kind)
}

pub fn protocol_profile_pins_for_envelope(
    envelope: &PackageEnvelopeDescriptor,
) -> Vec<ProtocolProfilePin> {
    let mut pins = envelope
        .protocols
        .iter()
        .flat_map(|protocol| {
            protocol
                .implementation
                .profiles
                .iter()
                .map(|profile| ProtocolProfilePin {
                    protocol_id: protocol.implementation.protocol_id.clone(),
                    version: protocol.implementation.version.clone(),
                    profile: profile.clone(),
                })
        })
        .collect::<Vec<_>>();
    pins.sort_by(|a, b| {
        (&a.protocol_id, &a.version, &a.profile).cmp(&(&b.protocol_id, &b.version, &b.profile))
    });
    pins.dedup_by(|a, b| {
        a.protocol_id == b.protocol_id && a.version == b.version && a.profile == b.profile
    });
    pins
}

fn build_component_descriptor(
    manifest: &PackageManifest,
    mut declaration: ComponentDeclaration,
    explicit: bool,
) -> anyhow::Result<ComponentDescriptor> {
    if declaration.capability_ids.is_empty() {
        declaration.capability_ids = manifest
            .provides
            .iter()
            .map(|capability| capability.id.clone())
            .collect();
    }
    if declaration.surface_ids.is_empty() {
        declaration.surface_ids = manifest
            .contributes
            .surfaces
            .iter()
            .map(|surface| surface.id.clone())
            .collect();
    }
    declaration.capability_ids.sort();
    declaration.capability_ids.dedup();
    declaration.surface_ids.sort();
    declaration.surface_ids.dedup();
    for implementation in &mut declaration.protocol_implementations {
        implementation.profiles.sort();
        implementation.profiles.dedup();
        implementation.conformance_vectors.sort();
        implementation.conformance_vectors.dedup();
    }
    declaration.protocol_implementations.sort_by(|a, b| {
        (
            &a.protocol_id,
            &a.version,
            &a.profiles,
            &a.conformance_vectors,
        )
            .cmp(&(
                &b.protocol_id,
                &b.version,
                &b.profiles,
                &b.conformance_vectors,
            ))
    });
    declaration.protocol_implementations.dedup();
    declaration.content_roots.sort_by(|a, b| {
        (&a.digest, &a.artifact_type_uri, &a.media_type).cmp(&(
            &b.digest,
            &b.artifact_type_uri,
            &b.media_type,
        ))
    });
    declaration
        .content_roots
        .dedup_by(|a, b| a.digest == b.digest);

    let trust_class = component_trust_class(manifest);
    if trust_class == ComponentTrustClass::ForeignCapsule {
        // `contract:none` is deliberately outside Protocol Commons. Even when
        // callers construct descriptors without first validating a manifest,
        // do not turn unverified protocol declarations into portable claims.
        declaration.protocol_implementations.clear();
    }

    let protocol_implementations = declaration
        .protocol_implementations
        .iter()
        .cloned()
        .map(|implementation| {
            let artifact = json_artifact(
                PACKAGED_PROTOCOL_TYPE_URI,
                &implementation,
                Vec::new(),
                annotation("protocol_id", &implementation.protocol_id),
            )?;
            Ok(PackagedProtocolDescriptor {
                implementation,
                artifact,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let surfaces = declaration
        .surface_ids
        .iter()
        .map(|surface_id| {
            let surface = manifest
                .contributes
                .surfaces
                .iter()
                .find(|surface| &surface.id == surface_id)
                .ok_or_else(|| anyhow::anyhow!("component surface '{surface_id}' not found"))?;
            packaged_surface(surface)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let claim_status = if trust_class == ComponentTrustClass::ForeignCapsule {
        ComponentClaimStatus::ForeignCapsule
    } else if explicit {
        ComponentClaimStatus::Declared
    } else {
        ComponentClaimStatus::LegacyAdapted
    };
    let enforced_boundaries = boundary_claims(trust_class);

    #[derive(Serialize)]
    struct CapabilityBehaviorClaim {
        id: CapabilityId,
        version: String,
        input_schema: Value,
        output_schema: Value,
        streaming: bool,
        side_effects: Vec<String>,
    }
    fn behavior_claim(capability: &CapabilityDescriptor) -> CapabilityBehaviorClaim {
        let mut side_effects = capability.side_effects.clone();
        side_effects.sort();
        side_effects.dedup();
        CapabilityBehaviorClaim {
            id: capability.id.clone(),
            version: capability.version.clone(),
            input_schema: capability.input_schema.clone(),
            output_schema: capability.output_schema.clone(),
            streaming: capability.streaming,
            side_effects,
        }
    }
    let claimed_capability_ids = declaration
        .capability_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut capability_claims = manifest
        .provides
        .iter()
        .filter(|capability| claimed_capability_ids.contains(capability.id.as_str()))
        .map(behavior_claim)
        .collect::<Vec<_>>();
    capability_claims.sort_by(|a, b| (&a.id, &a.version).cmp(&(&b.id, &b.version)));

    #[derive(Serialize)]
    struct BehaviorMaterial<'a> {
        component_id: &'a str,
        version: &'a str,
        capability_ids: &'a [CapabilityId],
        capability_claims: &'a [CapabilityBehaviorClaim],
        protocol_digests: Vec<&'a str>,
    }
    let behavior = json_artifact(
        COMPONENT_BEHAVIOR_TYPE_URI,
        &BehaviorMaterial {
            component_id: &declaration.id,
            version: &declaration.version,
            capability_ids: &declaration.capability_ids,
            capability_claims: &capability_claims,
            protocol_digests: protocol_implementations
                .iter()
                .map(|protocol| protocol.artifact.digest.as_str())
                .collect(),
        },
        protocol_implementations
            .iter()
            .map(|protocol| protocol.artifact.digest.clone())
            .collect(),
        annotation("component_id", &declaration.id),
    )?;

    let material = ComponentArtifactPayload {
        component_id: declaration.id.clone(),
        version: declaration.version.clone(),
        behavior: behavior.clone(),
        entry_kind: manifest.entry_kind().to_string(),
        entry: manifest.entry.kind.clone(),
        contract: manifest.entry.contract.clone(),
        trust_class,
        claim_status,
        enforced_boundaries: enforced_boundaries.clone(),
        protocol_artifacts: protocol_implementations
            .iter()
            .map(|protocol| protocol.artifact.clone())
            .collect(),
        content_roots: declaration.content_roots.clone(),
        surface_artifacts: surfaces
            .iter()
            .map(|surface| surface.artifact.clone())
            .collect(),
        annotations: declaration.annotations.clone(),
    };
    let references = std::iter::once(behavior.digest.clone())
        .chain(
            protocol_implementations
                .iter()
                .map(|protocol| protocol.artifact.digest.clone()),
        )
        .chain(
            declaration
                .content_roots
                .iter()
                .map(|root| root.digest.clone()),
        )
        .chain(
            surfaces
                .iter()
                .map(|surface| surface.artifact.digest.clone()),
        )
        .collect();
    let artifact = json_artifact(
        COMPONENT_DESCRIPTOR_TYPE_URI,
        &material,
        references,
        annotation("component_id", &declaration.id),
    )?;
    Ok(ComponentDescriptor {
        component_id: declaration.id,
        version: declaration.version,
        artifact,
        behavior,
        entry_kind: manifest.entry_kind().to_string(),
        trust_class,
        claim_status,
        enforced_boundaries,
        capability_ids: declaration.capability_ids,
        protocol_implementations,
        content_roots: declaration.content_roots,
        surfaces,
        annotations: declaration.annotations,
    })
}

fn legacy_component_declaration(manifest: &PackageManifest) -> ComponentDeclaration {
    let mut annotations = BTreeMap::new();
    annotations.insert("legacy_package_adapter".to_string(), Value::Bool(true));
    ComponentDeclaration {
        id: format!("{}/component/default", manifest.id),
        version: manifest.version.clone(),
        capability_ids: manifest
            .provides
            .iter()
            .map(|capability| capability.id.clone())
            .collect(),
        protocol_implementations: Vec::new(),
        content_roots: Vec::new(),
        surface_ids: manifest
            .contributes
            .surfaces
            .iter()
            .map(|surface| surface.id.clone())
            .collect(),
        annotations,
    }
}

fn boundary_claims(trust_class: ComponentTrustClass) -> ComponentBoundaryClaims {
    match trust_class {
        ComponentTrustClass::SandboxedComponent => ComponentBoundaryClaims {
            resource_limits_enforced: false,
            ..ComponentBoundaryClaims::default()
        },
        ComponentTrustClass::IsolatedProcess => ComponentBoundaryClaims {
            process_failure_isolation: true,
            ..ComponentBoundaryClaims::default()
        },
        ComponentTrustClass::RemoteBoundary => ComponentBoundaryClaims::default(),
        ComponentTrustClass::StaticResource => ComponentBoundaryClaims {
            no_code_execution: true,
            ..ComponentBoundaryClaims::default()
        },
        ComponentTrustClass::TrustedNative | ComponentTrustClass::ForeignCapsule => {
            ComponentBoundaryClaims::default()
        }
    }
}

fn packaged_surface(surface: &SurfaceContribution) -> anyhow::Result<PackagedSurfaceDescriptor> {
    Ok(PackagedSurfaceDescriptor {
        surface_id: surface.id.clone(),
        version: surface.version.clone(),
        artifact: json_artifact(
            PACKAGED_SURFACE_TYPE_URI,
            surface,
            Vec::new(),
            annotation("surface_id", &surface.id),
        )?,
    })
}

fn unique_protocols(components: &[ComponentDescriptor]) -> Vec<PackagedProtocolDescriptor> {
    let mut by_digest = BTreeMap::new();
    for protocol in components
        .iter()
        .flat_map(|component| component.protocol_implementations.iter())
    {
        by_digest
            .entry(protocol.artifact.digest.clone())
            .or_insert_with(|| protocol.clone());
    }
    by_digest.into_values().collect()
}

fn unique_artifacts(
    artifacts: impl IntoIterator<Item = ArtifactDescriptor>,
) -> Vec<ArtifactDescriptor> {
    let mut by_digest = BTreeMap::new();
    for artifact in artifacts {
        by_digest.entry(artifact.digest.clone()).or_insert(artifact);
    }
    by_digest.into_values().collect()
}

fn unique_surfaces(
    surfaces: impl IntoIterator<Item = PackagedSurfaceDescriptor>,
) -> Vec<PackagedSurfaceDescriptor> {
    let mut by_digest = BTreeMap::new();
    for surface in surfaces {
        by_digest
            .entry(surface.artifact.digest.clone())
            .or_insert(surface);
    }
    by_digest.into_values().collect()
}

fn json_artifact<T: Serialize>(
    artifact_type_uri: &str,
    value: &T,
    references: Vec<String>,
    annotations: BTreeMap<String, Value>,
) -> anyhow::Result<ArtifactDescriptor> {
    let bytes = canonical_json_bytes(value)?;
    Ok(ArtifactDescriptor {
        artifact_type_uri: artifact_type_uri.to_string(),
        media_type: "application/json".to_string(),
        digest: sha256(&bytes),
        size_bytes: bytes.len() as u64,
        references,
        annotations,
    })
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> anyhow::Result<Vec<u8>> {
    let mut value = serde_json::to_value(value)?;
    sort_json_value(&mut value);
    Ok(serde_json::to_vec(&value)?)
}

fn sort_json_value(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                sort_json_value(value);
            }
        }
        Value::Object(object) => {
            let mut sorted = BTreeMap::new();
            for (key, mut value) in std::mem::take(object) {
                sort_json_value(&mut value);
                sorted.insert(key, value);
            }
            object.extend(sorted);
        }
        _ => {}
    }
}

fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn validate_sha256(digest: &str) -> anyhow::Result<()> {
    let Some(hex) = digest.strip_prefix("sha256:") else {
        anyhow::bail!("digest '{digest}' must use sha256:");
    };
    anyhow::ensure!(
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "digest '{digest}' is not a complete SHA-256 value"
    );
    Ok(())
}

fn annotation(key: &str, value: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([(key.to_string(), Value::String(value.to_string()))])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapabilityDescriptor, EntryDescriptor, PackageContributions, PermissionSet, SandboxPolicy,
        SubprocessTransport,
    };

    fn manifest(package_id: &str, component: Option<ComponentDeclaration>) -> PackageManifest {
        PackageManifest {
            schema_version: 1,
            id: package_id.to_string(),
            version: "1.0.0".to_string(),
            display_name: None,
            description: None,
            author: None,
            license: None,
            entry: EntryDescriptor {
                kind: PackageEntry::Subprocess {
                    command: vec!["reference-component".to_string()],
                    transport: SubprocessTransport::JsonRpcStdio,
                },
                contract: ContractMode::V1,
                component,
            },
            provides: Vec::new(),
            consumes: Vec::new(),
            requires: Vec::new(),
            contributes: PackageContributions::default(),
            permissions: PermissionSet::default(),
            sandbox_policy: SandboxPolicy::default(),
        }
    }

    fn declaration() -> ComponentDeclaration {
        ComponentDeclaration {
            id: "org.example/reference-component".to_string(),
            version: "1.0.0".to_string(),
            capability_ids: Vec::new(),
            protocol_implementations: vec![ProtocolImplementationDeclaration {
                protocol_id: "plurora.change".to_string(),
                version: "1.0.0".to_string(),
                profiles: vec!["plurora.change/default/v1".to_string()],
                conformance_vectors: vec!["proposal.lifecycle_apply".to_string()],
            }],
            content_roots: Vec::new(),
            surface_ids: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    #[test]
    fn explicit_component_identity_is_independent_of_package_envelope() {
        let first = package_envelope_for_manifest(&manifest("vendor/one", Some(declaration())))
            .expect("first envelope");
        let second = package_envelope_for_manifest(&manifest("vendor/two", Some(declaration())))
            .expect("second envelope");
        assert_ne!(first.artifact.digest, second.artifact.digest);
        assert_eq!(first.components[0], second.components[0]);
        assert_eq!(
            first.components[0].behavior.digest,
            second.components[0].behavior.digest
        );
    }

    #[test]
    fn contract_none_is_always_a_foreign_capsule() {
        let mut manifest = manifest("vendor/foreign", Some(declaration()));
        manifest.entry.contract = ContractMode::None;
        let validation = manifest
            .validate_basic()
            .expect_err("foreign capsule protocol claims must be rejected");
        assert!(validation
            .to_string()
            .contains("cannot claim protocol conformance"));
        let envelope = package_envelope_for_manifest(&manifest).expect("foreign envelope");
        assert_eq!(
            envelope.components[0].trust_class,
            ComponentTrustClass::ForeignCapsule
        );
        assert_eq!(
            envelope.components[0].claim_status,
            ComponentClaimStatus::ForeignCapsule
        );
        assert_eq!(
            envelope.components[0].enforced_boundaries,
            ComponentBoundaryClaims::default()
        );
        assert!(envelope.components[0].protocol_implementations.is_empty());
        assert!(envelope.protocols.is_empty());
    }

    #[test]
    fn explicit_component_capabilities_must_match_package_provides() {
        let mut declaration = declaration();
        declaration.capability_ids = vec!["org.example/reference-component/echo".to_string()];
        let mut manifest = manifest("vendor/one", Some(declaration));
        manifest.provides = vec![CapabilityDescriptor {
            id: "org.example/reference-component/echo".to_string(),
            version: "1.0.0".to_string(),
            input_schema: Value::Null,
            output_schema: Value::Null,
            streaming: false,
            side_effects: Vec::new(),
            description: None,
        }];
        manifest.validate_basic().expect("matching capability set");

        manifest.provides[0].id = "vendor/one/other".to_string();
        let error = manifest
            .validate_basic()
            .expect_err("mismatched component capability set must fail");
        assert!(error.to_string().contains("must exactly match"));
    }

    #[test]
    fn capability_contract_changes_component_and_behavior_digests() {
        let mut declaration = declaration();
        declaration.protocol_implementations.clear();
        declaration.capability_ids = vec!["org.example/reference-component/echo".to_string()];
        let mut first = manifest("vendor/one", Some(declaration));
        first.provides = vec![CapabilityDescriptor {
            id: "org.example/reference-component/echo".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: Value::Null,
            streaming: false,
            side_effects: Vec::new(),
            description: Some("human-facing documentation".to_string()),
        }];
        let mut second = first.clone();
        second.provides[0].input_schema =
            serde_json::json!({"type": "object", "required": ["value"]});

        let first = package_envelope_for_manifest(&first).expect("first envelope");
        let second = package_envelope_for_manifest(&second).expect("second envelope");
        assert_ne!(
            first.components[0].behavior.digest,
            second.components[0].behavior.digest
        );
        assert_ne!(
            first.components[0].artifact.digest,
            second.components[0].artifact.digest
        );
    }

    #[test]
    fn component_payload_derives_trust_and_validates_typed_references() {
        let manifest = manifest("vendor/one", Some(declaration()));
        let component = package_envelope_for_manifest(&manifest).unwrap().components[0].clone();
        let payload = ComponentArtifactPayload {
            component_id: component.component_id.clone(),
            version: component.version.clone(),
            behavior: component.behavior.clone(),
            entry_kind: manifest.entry_kind().to_string(),
            entry: manifest.entry.kind.clone(),
            contract: manifest.entry.contract.clone(),
            trust_class: component.trust_class,
            claim_status: component.claim_status,
            enforced_boundaries: component.enforced_boundaries.clone(),
            protocol_artifacts: component
                .protocol_implementations
                .iter()
                .map(|protocol| protocol.artifact.clone())
                .collect(),
            content_roots: component.content_roots.clone(),
            surface_artifacts: component
                .surfaces
                .iter()
                .map(|surface| surface.artifact.clone())
                .collect(),
            annotations: component.annotations.clone(),
        };

        let encode = |payload: &ComponentArtifactPayload| {
            let bytes = canonical_json_bytes(payload).unwrap();
            let references = std::iter::once(payload.behavior.digest.clone())
                .chain(
                    payload
                        .protocol_artifacts
                        .iter()
                        .chain(&payload.content_roots)
                        .chain(&payload.surface_artifacts)
                        .map(|artifact| artifact.digest.clone()),
                )
                .collect();
            let descriptor = ArtifactDescriptor {
                artifact_type_uri: COMPONENT_DESCRIPTOR_TYPE_URI.to_string(),
                media_type: "application/json".to_string(),
                digest: sha256(&bytes),
                size_bytes: bytes.len() as u64,
                references,
                annotations: annotation("component_id", &payload.component_id),
            };
            (descriptor, bytes)
        };

        let (descriptor, bytes) = encode(&payload);
        decode_component_artifact_payload(&descriptor, &bytes).unwrap();

        let mut false_trust = payload.clone();
        false_trust.trust_class = ComponentTrustClass::SandboxedComponent;
        let (descriptor, bytes) = encode(&false_trust);
        assert!(decode_component_artifact_payload(&descriptor, &bytes).is_err());

        let mut wrong_protocol_type = payload.clone();
        wrong_protocol_type.protocol_artifacts[0].artifact_type_uri =
            PACKAGED_SURFACE_TYPE_URI.to_string();
        let (descriptor, bytes) = encode(&wrong_protocol_type);
        assert!(decode_component_artifact_payload(&descriptor, &bytes).is_err());

        let mut host_path = payload;
        host_path.entry = PackageEntry::Subprocess {
            command: vec![r"C:\Users\creator\run.exe".to_string()],
            transport: SubprocessTransport::JsonRpcStdio,
        };
        let (descriptor, bytes) = encode(&host_path);
        assert!(decode_component_artifact_payload(&descriptor, &bytes).is_err());
    }
}
