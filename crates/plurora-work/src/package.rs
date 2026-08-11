use std::collections::{BTreeMap, BTreeSet};

use plurora_core::{
    canonical_json_bytes as core_canonical_json_bytes, package_envelope_for_manifest,
    world_bundle_sha256_digest, ArtifactDescriptor, CapabilityDescriptor, ComponentArtifactPayload,
    ComponentDescriptor, ComponentTrustClass, PackageEntry, PackageEnvelopeDescriptor,
    PackageManifest, ProtocolProfilePin,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::assembly::{AssemblyNode, AssemblyNodeSource, AssemblyRevision};
use crate::canonical::{
    validate_artifact_descriptor, validate_canonical_model, validate_descriptor_type,
    validate_portable_model, ArtifactModel,
};
use crate::diagnostic::{DiagnosticCode, ModelError, ModelResult};
use crate::ids::{validate_local_id, AssemblyId, NodeId, PortId, WorkId};
use crate::port::{
    AvailabilityPolicy, BindingPhase, EffectClass, InteractionModelId, PortContract,
    PortDescriptor, PortEndpoint, PortMultiplicity, PortRole, TransportRequirements,
    INTERACTION_CAPABILITY_STREAM, INTERACTION_CAPABILITY_UNARY,
};
use crate::rights::{RIGHTS_DECLARATION_TYPE_URI, TRANSPARENCY_DECLARATION_TYPE_URI};
use crate::state::StateSlotDescriptor;
use crate::work::{WorkEntrypoint, WorkEntrypointTarget, WorkRevision};
use crate::FOREIGN_CAPSULE_TYPE_URI;

pub const SOURCE_REPOSITORY_CANDIDATE_TYPE_URI: &str = "urn:plurora:source-repository-candidate:v1";
pub const SOURCE_REPOSITORY_CANDIDATE_SCHEMA: &str = "plurora.source-repository-candidate.v1";
pub const CAPABILITY_PROTOCOL_ID: &str = "plurora.capability";
pub const FOREIGN_DEDICATED_SERVER_ANNOTATION: &str = "plurora.foreign/dedicated_server";
pub const FOREIGN_LAUNCH_KIND_ANNOTATION: &str = "plurora.foreign/launch_kind";
pub const FOREIGN_DEDICATED_SERVER_INTENT_URI: &str = "plurora.shell.default/dedicated-server";
pub const FOREIGN_PLAY_INTENT_URI: &str = "plurora.shell.default/play";

const CLAIM_STATUS_ANNOTATION: &str = "plurora.projection/claim_status";
const CAPABILITY_ID_ANNOTATION: &str = "plurora.projection/capability_id";
const SIDE_EFFECTS_ANNOTATION: &str = "plurora.projection/side_effects";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CanonicalArtifactObject {
    pub descriptor: ArtifactDescriptor,
    pub bytes: Vec<u8>,
}

impl CanonicalArtifactObject {
    pub fn new(descriptor: ArtifactDescriptor, bytes: Vec<u8>) -> ModelResult<Self> {
        validate_artifact_descriptor(&descriptor)?;
        if descriptor.digest != world_bundle_sha256_digest(&bytes)
            || descriptor.size_bytes != bytes.len() as u64
        {
            return Err(ModelError::new(
                DiagnosticCode::ArtifactDigestMismatch,
                "canonical artifact object bytes do not match their descriptor",
            ));
        }
        Ok(Self { descriptor, bytes })
    }

    pub fn from_model<T: ArtifactModel>(model: &T) -> ModelResult<Self> {
        let bytes = model.canonical_bytes()?;
        let descriptor = model.artifact_descriptor()?;
        Self::new(descriptor, bytes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PackageProjection {
    pub envelope: PackageEnvelopeDescriptor,
    pub component: ComponentDescriptor,
    pub component_artifact: ArtifactDescriptor,
    pub ports: Vec<PortDescriptor>,
    pub protocol_profiles: Vec<ProtocolProfilePin>,
    pub artifacts: Vec<CanonicalArtifactObject>,
}

impl PackageProjection {
    pub fn port_catalog(&self, node_id: &NodeId) -> BTreeMap<PortEndpoint, PortDescriptor> {
        self.ports
            .iter()
            .cloned()
            .map(|port| {
                (
                    PortEndpoint {
                        node_id: node_id.clone(),
                        port_id: port.port_id.clone(),
                    },
                    port,
                )
            })
            .collect()
    }
}

pub fn project_package_manifest(
    manifest: &PackageManifest,
    explicit_ports: Vec<PortDescriptor>,
) -> ModelResult<PackageProjection> {
    if manifest.schema_version != 1 {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            "package manifest uses an unsupported schema version",
        ));
    }
    manifest.validate_basic().map_err(|_| {
        ModelError::new(
            DiagnosticCode::WorkInvalid,
            "package manifest is invalid and cannot be projected",
        )
    })?;
    let envelope = package_envelope_for_manifest(manifest).map_err(|_| {
        ModelError::new(
            DiagnosticCode::WorkInvalid,
            "package envelope could not be derived from the manifest",
        )
    })?;
    let component = envelope.components.first().cloned().ok_or_else(|| {
        ModelError::new(
            DiagnosticCode::ArtifactMissing,
            "package envelope does not contain a component",
        )
    })?;
    if envelope.components.len() != 1 {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            "package projection requires the current single-component envelope shape",
        ));
    }
    validate_portable_model(&manifest.entry.kind)?;
    validate_portable_model(&component.annotations)?;
    validate_canonical_model(&component.annotations)?;
    let ports = if component.trust_class == ComponentTrustClass::ForeignCapsule {
        if !explicit_ports.is_empty() {
            return Err(ModelError::new(
                DiagnosticCode::PortIncompatible,
                "contract:none component cannot claim executable Ports",
            ));
        }
        Vec::new()
    } else {
        project_capability_ports(manifest, explicit_ports)?
    };
    let protocol_profiles = sorted_protocol_profiles(&component);
    let artifacts = package_artifact_objects(manifest, &envelope, &component)?;
    Ok(PackageProjection {
        component_artifact: component.artifact.clone(),
        envelope,
        component,
        ports,
        protocol_profiles,
        artifacts,
    })
}

pub fn project_capability_ports(
    manifest: &PackageManifest,
    explicit_ports: Vec<PortDescriptor>,
) -> ModelResult<Vec<PortDescriptor>> {
    let mut ports = BTreeMap::<PortId, PortDescriptor>::new();
    let mut provides = manifest.provides.iter().collect::<Vec<_>>();
    provides.sort_by(|left, right| (&left.id, &left.version).cmp(&(&right.id, &right.version)));
    for capability in provides {
        let port = export_capability_port(capability)?;
        if ports.insert(port.port_id.clone(), port).is_some() {
            return Err(projection_collision());
        }
    }
    let mut consumes = manifest.consumes.iter().collect::<Vec<_>>();
    consumes.sort_by(|left, right| (&left.id, &left.version).cmp(&(&right.id, &right.version)));
    for capability in consumes {
        let port_id = capability_port_id(&capability.id)?;
        let port = PortDescriptor {
            port_id: port_id.clone(),
            contract: PortContract {
                protocol_id: CAPABILITY_PROTOCOL_ID.to_string(),
                interface_id: capability.id.clone(),
                version: capability.version.clone(),
                profiles: Vec::new(),
            },
            interaction: InteractionModelId(INTERACTION_CAPABILITY_UNARY.to_string()),
            role: PortRole::Import {
                multiplicity: PortMultiplicity {
                    min: 1,
                    max: Some(1),
                },
                latest_binding_phase: BindingPhase::Installation,
                availability: AvailabilityPolicy::Required,
                accepted_effects: vec![EffectClass::ExternalEffecting],
            },
            transport: TransportRequirements::default(),
            annotations: projection_annotations(&capability.id, None),
        };
        port.validate()?;
        if ports.insert(port_id, port).is_some() {
            return Err(projection_collision());
        }
    }
    for port in explicit_ports {
        port.validate()?;
        if ports.insert(port.port_id.clone(), port).is_some() {
            return Err(projection_collision());
        }
    }
    Ok(ports.into_values().collect())
}

fn export_capability_port(capability: &CapabilityDescriptor) -> ModelResult<PortDescriptor> {
    let port_id = capability_port_id(&capability.id)?;
    let mut effects = capability.side_effects.clone();
    effects.sort();
    effects.dedup();
    let port = PortDescriptor {
        port_id,
        contract: PortContract {
            protocol_id: CAPABILITY_PROTOCOL_ID.to_string(),
            interface_id: capability.id.clone(),
            version: capability.version.clone(),
            profiles: Vec::new(),
        },
        interaction: InteractionModelId(
            if capability.streaming {
                INTERACTION_CAPABILITY_STREAM
            } else {
                INTERACTION_CAPABILITY_UNARY
            }
            .to_string(),
        ),
        role: PortRole::Export {
            multiplicity: PortMultiplicity { min: 0, max: None },
            // A legacy side-effect string list is not a machine-verifiable purity
            // guarantee. Explicit Ports are the only path to a narrower class.
            effect_class: EffectClass::ExternalEffecting,
        },
        transport: TransportRequirements::default(),
        annotations: projection_annotations(&capability.id, Some(effects)),
    };
    port.validate()?;
    Ok(port)
}

fn capability_port_id(capability_id: &str) -> ModelResult<PortId> {
    let segment = capability_id.rsplit('/').next().unwrap_or_default();
    PortId::parse(segment.to_string()).map_err(|_| {
        ModelError::new(
            DiagnosticCode::InvalidId,
            "capability projection cannot derive a valid local port id",
        )
    })
}

fn projection_annotations(
    capability_id: &str,
    side_effects: Option<Vec<String>>,
) -> BTreeMap<String, Value> {
    let mut annotations = BTreeMap::from([
        (
            CLAIM_STATUS_ANNOTATION.to_string(),
            Value::String("legacy_adapted".to_string()),
        ),
        (
            CAPABILITY_ID_ANNOTATION.to_string(),
            Value::String(capability_id.to_string()),
        ),
    ]);
    if let Some(side_effects) = side_effects {
        annotations.insert(
            SIDE_EFFECTS_ANNOTATION.to_string(),
            Value::Array(side_effects.into_iter().map(Value::String).collect()),
        );
    }
    annotations
}

fn projection_collision() -> ModelError {
    ModelError::new(
        DiagnosticCode::WorkInvalid,
        "capability and explicit Port projection contains a local port id collision",
    )
}

fn sorted_protocol_profiles(component: &ComponentDescriptor) -> Vec<ProtocolProfilePin> {
    let mut profiles = component
        .protocol_implementations
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
    profiles.sort_by(|left, right| {
        (&left.protocol_id, &left.version, &left.profile).cmp(&(
            &right.protocol_id,
            &right.version,
            &right.profile,
        ))
    });
    profiles.dedup_by(|left, right| left == right);
    profiles
}

fn package_artifact_objects(
    manifest: &PackageManifest,
    envelope: &PackageEnvelopeDescriptor,
    component: &ComponentDescriptor,
) -> ModelResult<Vec<CanonicalArtifactObject>> {
    #[derive(Serialize)]
    struct CapabilityBehaviorClaim<'a> {
        id: &'a str,
        version: &'a str,
        input_schema: &'a Value,
        output_schema: &'a Value,
        streaming: bool,
        side_effects: Vec<String>,
    }
    #[derive(Serialize)]
    struct BehaviorMaterial<'a> {
        component_id: &'a str,
        version: &'a str,
        capability_ids: &'a [String],
        capability_claims: Vec<CapabilityBehaviorClaim<'a>>,
        protocol_digests: Vec<&'a str>,
    }
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

    let mut objects = Vec::new();
    objects.push(core_object(
        envelope.manifest.clone(),
        core_bytes(manifest)?,
    )?);
    let entry = envelope
        .artifacts
        .iter()
        .find(|artifact| artifact.id == "entry")
        .ok_or_else(|| {
            ModelError::new(
                DiagnosticCode::ArtifactMissing,
                "package projection is missing its entry artifact",
            )
        })?;
    objects.push(core_object(
        entry.descriptor.clone(),
        core_bytes(&manifest.entry)?,
    )?);
    for schema in &manifest.contributes.schemas {
        let descriptor = envelope
            .artifacts
            .iter()
            .find(|artifact| artifact.id == schema.id)
            .map(|artifact| artifact.descriptor.clone())
            .ok_or_else(|| {
                ModelError::new(
                    DiagnosticCode::ArtifactMissing,
                    "package projection is missing a schema artifact",
                )
            })?;
        objects.push(core_object(descriptor, core_bytes(schema)?)?);
    }
    for protocol in &component.protocol_implementations {
        objects.push(core_object(
            protocol.artifact.clone(),
            core_bytes(&protocol.implementation)?,
        )?);
    }
    for surface in &component.surfaces {
        let source = manifest
            .contributes
            .surfaces
            .iter()
            .find(|candidate| candidate.id == surface.surface_id)
            .ok_or_else(|| {
                ModelError::new(
                    DiagnosticCode::ArtifactMissing,
                    "package projection is missing a surface source",
                )
            })?;
        objects.push(core_object(surface.artifact.clone(), core_bytes(source)?)?);
    }

    let claimed = component
        .capability_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut capability_claims = manifest
        .provides
        .iter()
        .filter(|capability| claimed.contains(capability.id.as_str()))
        .collect::<Vec<_>>();
    capability_claims
        .sort_by(|left, right| (&left.id, &left.version).cmp(&(&right.id, &right.version)));
    let capability_claims = capability_claims
        .into_iter()
        .map(|capability| {
            let mut side_effects = capability.side_effects.clone();
            side_effects.sort();
            side_effects.dedup();
            CapabilityBehaviorClaim {
                id: &capability.id,
                version: &capability.version,
                input_schema: &capability.input_schema,
                output_schema: &capability.output_schema,
                streaming: capability.streaming,
                side_effects,
            }
        })
        .collect();
    let behavior = BehaviorMaterial {
        component_id: &component.component_id,
        version: &component.version,
        capability_ids: &component.capability_ids,
        capability_claims,
        protocol_digests: component
            .protocol_implementations
            .iter()
            .map(|protocol| protocol.artifact.digest.as_str())
            .collect(),
    };
    objects.push(core_object(
        component.behavior.clone(),
        core_bytes(&behavior)?,
    )?);
    let component_material = ComponentArtifactPayload {
        component_id: component.component_id.clone(),
        version: component.version.clone(),
        behavior: component.behavior.clone(),
        entry_kind: component.entry_kind.clone(),
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
    objects.push(core_object(
        component.artifact.clone(),
        core_bytes(&component_material)?,
    )?);
    let envelope_material = EnvelopeMaterial {
        package_id: &envelope.package_id,
        package_version: &envelope.package_version,
        manifest_digest: &envelope.manifest.digest,
        component_digests: envelope
            .components
            .iter()
            .map(|component| component.artifact.digest.as_str())
            .collect(),
        protocol_digests: envelope
            .protocols
            .iter()
            .map(|protocol| protocol.artifact.digest.as_str())
            .collect(),
        content_root_digests: envelope
            .content_roots
            .iter()
            .map(|root| root.digest.as_str())
            .collect(),
        surface_digests: envelope
            .surfaces
            .iter()
            .map(|surface| surface.artifact.digest.as_str())
            .collect(),
        artifact_digests: envelope
            .artifacts
            .iter()
            .map(|artifact| artifact.descriptor.digest.as_str())
            .collect(),
    };
    objects.push(core_object(
        envelope.artifact.clone(),
        core_bytes(&envelope_material)?,
    )?);
    objects.sort_by(|left, right| left.descriptor.digest.cmp(&right.descriptor.digest));
    objects.dedup_by(|left, right| left.descriptor.digest == right.descriptor.digest);
    Ok(objects)
}

fn core_object(
    descriptor: ArtifactDescriptor,
    bytes: Vec<u8>,
) -> ModelResult<CanonicalArtifactObject> {
    CanonicalArtifactObject::new(descriptor, bytes)
}

fn core_bytes<T: Serialize>(value: &T) -> ModelResult<Vec<u8>> {
    core_canonical_json_bytes(value).map_err(|_| {
        ModelError::new(
            DiagnosticCode::WorkInvalid,
            "package artifact could not be serialized canonically",
        )
    })
}

fn sort_artifacts(artifacts: &mut Vec<ArtifactDescriptor>) {
    artifacts.sort_by(|left, right| {
        (&left.digest, &left.artifact_type_uri, &left.media_type).cmp(&(
            &right.digest,
            &right.artifact_type_uri,
            &right.media_type,
        ))
    });
    artifacts.dedup_by(|left, right| left.digest == right.digest);
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NormalizedWorkKind {
    Package,
    ForeignCapsule,
    ContentOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct NormalizedWork {
    pub kind: NormalizedWorkKind,
    pub work: WorkRevision,
    pub assembly: AssemblyRevision,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub port_catalog: Vec<(PortEndpoint, PortDescriptor)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<CanonicalArtifactObject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<PackageProjection>,
}

impl NormalizedWork {
    pub fn validate(&self) -> ModelResult<()> {
        self.assembly.validate()?;
        self.work.validate()?;
        if self.work.assembly != self.assembly.artifact_descriptor()? {
            return Err(ModelError::new(
                DiagnosticCode::ArtifactDigestMismatch,
                "normalized Work does not reference its canonical Assembly",
            ));
        }
        let catalog = self
            .port_catalog
            .iter()
            .cloned()
            .collect::<BTreeMap<_, _>>();
        self.assembly.validate_ports(&catalog)?;
        let mut digests = BTreeSet::new();
        for artifact in &self.artifacts {
            CanonicalArtifactObject::new(artifact.descriptor.clone(), artifact.bytes.clone())?;
            if !digests.insert(artifact.descriptor.digest.as_str()) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "normalized Work contains a duplicate canonical artifact object",
                ));
            }
        }
        Ok(())
    }
}

pub fn normalize_package_manifest(
    manifest: &PackageManifest,
    explicit_ports: Vec<PortDescriptor>,
) -> ModelResult<NormalizedWork> {
    if matches!(&manifest.entry.kind, PackageEntry::Remote { .. }) {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            "remote Package entry requires explicit Foreign Capsule normalization",
        ));
    }
    let package = project_package_manifest(manifest, explicit_ports)?;
    if package.component.trust_class == ComponentTrustClass::ForeignCapsule {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            "contract:none package requires explicit Foreign Capsule normalization",
        ));
    }
    let work_id = WorkId::parse(manifest.id.clone())?;
    let assembly_id = AssemblyId::parse(format!("{}/assembly", manifest.id))?;
    let node_id = NodeId::parse("main")?;
    let assembly = AssemblyRevision {
        schema: AssemblyRevision::SCHEMA.to_string(),
        assembly_id,
        nodes: vec![AssemblyNode {
            node_id: node_id.clone(),
            source: AssemblyNodeSource::Component {
                component: package.component.artifact.clone(),
            },
            ports: package.ports.clone(),
            configuration: None,
            annotations: BTreeMap::from([(
                "plurora.normalization/component_id".to_string(),
                Value::String(package.component.component_id.clone()),
            )]),
        }],
        bindings: Vec::new(),
        exposed_ports: Vec::new(),
        state_slots: Vec::new(),
        annotations: BTreeMap::new(),
    };
    let assembly_object = CanonicalArtifactObject::from_model(&assembly)?;
    let work = WorkRevision {
        schema: WorkRevision::SCHEMA.to_string(),
        work_id,
        title: manifest
            .display_name
            .clone()
            .unwrap_or_else(|| manifest.id.clone()),
        description: manifest.description.clone().unwrap_or_default(),
        assembly: assembly_object.descriptor.clone(),
        content_roots: package.component.content_roots.clone(),
        entrypoints: Vec::new(),
        rights: None,
        transparency: None,
        operational_intent: None,
        annotations: BTreeMap::from([(
            "plurora.normalization/package_envelope".to_string(),
            Value::String(package.envelope.artifact.digest.clone()),
        )]),
    };
    let work_object = CanonicalArtifactObject::from_model(&work)?;
    let mut artifacts = package.artifacts.clone();
    artifacts.extend([assembly_object, work_object]);
    artifacts.sort_by(|left, right| left.descriptor.digest.cmp(&right.descriptor.digest));
    artifacts.dedup_by(|left, right| left.descriptor.digest == right.descriptor.digest);
    let port_catalog = package
        .port_catalog(&node_id)
        .into_iter()
        .collect::<Vec<_>>();
    let normalized = NormalizedWork {
        kind: NormalizedWorkKind::Package,
        work,
        assembly,
        port_catalog,
        artifacts,
        package: Some(package),
    };
    normalized.validate()?;
    Ok(normalized)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForeignLaunchKind {
    ExternalUri,
    LocalExecutable,
    ManagedArtifact,
    OciImage,
    RemoteService,
    EntitlementAdapter,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ForeignLaunchRequirement {
    pub launch_id: String,
    pub kind: ForeignLaunchKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_protocols: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ForeignCapsuleDescriptor {
    pub capsule_id: String,
    pub launch_requirements: Vec<ForeignLaunchRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocol_ports: Vec<PortDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_slots: Vec<StateSlotDescriptor>,
    pub rights: ArtifactDescriptor,
    pub transparency: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl ForeignCapsuleDescriptor {
    pub fn validate(&self) -> ModelResult<()> {
        WorkId::parse(self.capsule_id.clone())?;
        validate_descriptor_type(&self.rights, RIGHTS_DECLARATION_TYPE_URI)?;
        validate_descriptor_type(&self.transparency, TRANSPARENCY_DECLARATION_TYPE_URI)?;
        if self.launch_requirements.is_empty() {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "foreign capsule must declare at least one abstract launch requirement",
            ));
        }
        let mut launches = BTreeSet::new();
        for requirement in &self.launch_requirements {
            validate_local_id(&requirement.launch_id, "foreign launch id")?;
            if !launches.insert(requirement.launch_id.as_str())
                || requirement
                    .required_protocols
                    .iter()
                    .any(|protocol| protocol.trim().is_empty())
            {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "foreign capsule contains an invalid or duplicate launch requirement",
                ));
            }
            if contains_foreign_coordinate_field(&requirement.annotations) {
                return Err(ModelError::new(
                    DiagnosticCode::RawPath,
                    "foreign launch coordinates belong in an Installation-local binding",
                ));
            }
        }
        if contains_foreign_coordinate_field(&self.annotations) {
            return Err(ModelError::new(
                DiagnosticCode::RawPath,
                "foreign launch coordinates belong in an Installation-local binding",
            ));
        }
        let mut ports = BTreeSet::new();
        for port in &self.protocol_ports {
            port.validate()?;
            if !ports.insert(&port.port_id) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "foreign capsule contains a duplicate protocol port",
                ));
            }
        }
        for slot in &self.state_slots {
            slot.validate()?;
        }
        validate_portable_model(self)
    }
}

fn contains_foreign_coordinate_field(annotations: &BTreeMap<String, Value>) -> bool {
    annotations
        .iter()
        .any(|(key, value)| is_foreign_coordinate_field(key) || value_has_coordinate_field(value))
}

fn value_has_coordinate_field(value: &Value) -> bool {
    match value {
        Value::Array(values) => values.iter().any(value_has_coordinate_field),
        Value::Object(values) => values.iter().any(|(key, value)| {
            is_foreign_coordinate_field(key) || value_has_coordinate_field(value)
        }),
        _ => false,
    }
}

fn is_foreign_coordinate_field(key: &str) -> bool {
    let field = key
        .rsplit(['/', ':'])
        .next()
        .unwrap_or(key)
        .replace('-', "_")
        .to_ascii_lowercase();
    matches!(
        field.as_str(),
        "location" | "uri" | "url" | "path" | "executable" | "endpoint" | "image" | "oci_image"
    )
}

impl ArtifactModel for ForeignCapsuleDescriptor {
    const ARTIFACT_TYPE_URI: &'static str = FOREIGN_CAPSULE_TYPE_URI;

    fn validate(&self) -> ModelResult<()> {
        ForeignCapsuleDescriptor::validate(self)
    }

    fn referenced_artifacts(&self) -> Vec<&ArtifactDescriptor> {
        std::iter::once(&self.rights)
            .chain(std::iter::once(&self.transparency))
            .chain(
                self.state_slots
                    .iter()
                    .filter_map(|slot| slot.schema_ref.as_ref()),
            )
            .collect()
    }
}

pub fn normalize_foreign_capsule(
    work_id: WorkId,
    title: String,
    description: String,
    mut capsule: ForeignCapsuleDescriptor,
) -> ModelResult<NormalizedWork> {
    if capsule.capsule_id != work_id.as_str() || title.trim().is_empty() {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            "foreign capsule identity or title is invalid",
        ));
    }
    capsule
        .launch_requirements
        .sort_by(|left, right| left.launch_id.cmp(&right.launch_id));
    capsule
        .protocol_ports
        .sort_by(|left, right| left.port_id.cmp(&right.port_id));
    capsule
        .state_slots
        .sort_by(|left, right| left.state_slot_id.cmp(&right.state_slot_id));
    capsule.validate()?;
    let assembly = empty_assembly(&work_id, "foreign")?;
    let assembly_object = CanonicalArtifactObject::from_model(&assembly)?;
    let capsule_object = CanonicalArtifactObject::from_model(&capsule)?;
    let entrypoints = capsule
        .launch_requirements
        .iter()
        .map(|requirement| WorkEntrypoint {
            id: requirement.launch_id.clone(),
            intent_uri: if foreign_launch_is_dedicated_server(requirement) {
                FOREIGN_DEDICATED_SERVER_INTENT_URI.to_string()
            } else {
                FOREIGN_PLAY_INTENT_URI.to_string()
            },
            target: WorkEntrypointTarget::ForeignLaunch {
                launch_id: requirement.launch_id.clone(),
            },
            annotations: BTreeMap::from([(
                FOREIGN_LAUNCH_KIND_ANNOTATION.to_string(),
                Value::String(foreign_launch_kind_name(requirement.kind).to_string()),
            )]),
        })
        .collect();
    let work = WorkRevision {
        schema: WorkRevision::SCHEMA.to_string(),
        work_id,
        title,
        description,
        assembly: assembly_object.descriptor.clone(),
        content_roots: vec![capsule_object.descriptor.clone()],
        entrypoints,
        rights: Some(capsule.rights.clone()),
        transparency: Some(capsule.transparency.clone()),
        operational_intent: None,
        annotations: BTreeMap::from([(
            "plurora.normalization/kind".to_string(),
            Value::String("foreign_capsule".to_string()),
        )]),
    };
    let work_object = CanonicalArtifactObject::from_model(&work)?;
    let mut artifacts = vec![assembly_object, capsule_object, work_object];
    artifacts.sort_by(|left, right| left.descriptor.digest.cmp(&right.descriptor.digest));
    let normalized = NormalizedWork {
        kind: NormalizedWorkKind::ForeignCapsule,
        work,
        assembly,
        port_catalog: Vec::new(),
        artifacts,
        package: None,
    };
    normalized.validate()?;
    Ok(normalized)
}

pub fn foreign_launch_is_dedicated_server(requirement: &ForeignLaunchRequirement) -> bool {
    requirement
        .annotations
        .get(FOREIGN_DEDICATED_SERVER_ANNOTATION)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub fn foreign_launch_kind_name(kind: ForeignLaunchKind) -> &'static str {
    match kind {
        ForeignLaunchKind::ExternalUri => "external_uri",
        ForeignLaunchKind::LocalExecutable => "local_executable",
        ForeignLaunchKind::ManagedArtifact => "managed_artifact",
        ForeignLaunchKind::OciImage => "oci_image",
        ForeignLaunchKind::RemoteService => "remote_service",
        ForeignLaunchKind::EntitlementAdapter => "entitlement_adapter",
    }
}

pub fn normalize_content_only(
    work_id: WorkId,
    title: String,
    description: String,
    mut content_roots: Vec<ArtifactDescriptor>,
    mut entrypoints: Vec<WorkEntrypoint>,
    annotations: BTreeMap<String, Value>,
) -> ModelResult<NormalizedWork> {
    if title.trim().is_empty() || content_roots.is_empty() {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            "content-only Work requires a title and at least one content root",
        ));
    }
    for root in &content_roots {
        validate_artifact_descriptor(root)?;
    }
    sort_artifacts(&mut content_roots);
    entrypoints.sort_by(|left, right| left.id.cmp(&right.id));
    let assembly = empty_assembly(&work_id, "content")?;
    let assembly_object = CanonicalArtifactObject::from_model(&assembly)?;
    let work = WorkRevision {
        schema: WorkRevision::SCHEMA.to_string(),
        work_id,
        title,
        description,
        assembly: assembly_object.descriptor.clone(),
        content_roots,
        entrypoints,
        rights: None,
        transparency: None,
        operational_intent: None,
        annotations,
    };
    let work_object = CanonicalArtifactObject::from_model(&work)?;
    let mut artifacts = vec![assembly_object, work_object];
    artifacts.sort_by(|left, right| left.descriptor.digest.cmp(&right.descriptor.digest));
    let normalized = NormalizedWork {
        kind: NormalizedWorkKind::ContentOnly,
        work,
        assembly,
        port_catalog: Vec::new(),
        artifacts,
        package: None,
    };
    normalized.validate()?;
    Ok(normalized)
}

fn empty_assembly(work_id: &WorkId, suffix: &str) -> ModelResult<AssemblyRevision> {
    Ok(AssemblyRevision {
        schema: AssemblyRevision::SCHEMA.to_string(),
        assembly_id: AssemblyId::parse(format!("{}/{suffix}", work_id.as_str()))?,
        nodes: Vec::new(),
        bindings: Vec::new(),
        exposed_ports: Vec::new(),
        state_slots: Vec::new(),
        annotations: BTreeMap::new(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SourceRepositoryCandidate {
    pub schema: String,
    pub candidate_id: String,
    pub source_snapshot: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inspection_refs: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub build_graph_candidates: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub work_candidates: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operational_intent_candidates: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl SourceRepositoryCandidate {
    pub fn new(
        candidate_id: String,
        source_snapshot: ArtifactDescriptor,
        mut inspection_refs: Vec<ArtifactDescriptor>,
        mut build_graph_candidates: Vec<ArtifactDescriptor>,
        mut work_candidates: Vec<ArtifactDescriptor>,
        mut operational_intent_candidates: Vec<ArtifactDescriptor>,
        annotations: BTreeMap<String, Value>,
    ) -> ModelResult<Self> {
        sort_artifacts(&mut inspection_refs);
        sort_artifacts(&mut build_graph_candidates);
        sort_artifacts(&mut work_candidates);
        sort_artifacts(&mut operational_intent_candidates);
        let candidate = Self {
            schema: SOURCE_REPOSITORY_CANDIDATE_SCHEMA.to_string(),
            candidate_id,
            source_snapshot,
            inspection_refs,
            build_graph_candidates,
            work_candidates,
            operational_intent_candidates,
            annotations,
        };
        candidate.validate()?;
        Ok(candidate)
    }

    pub fn validate(&self) -> ModelResult<()> {
        if self.schema != SOURCE_REPOSITORY_CANDIDATE_SCHEMA {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "source repository candidate uses an unsupported schema",
            ));
        }
        validate_local_id(&self.candidate_id, "source repository candidate id")?;
        for reference in self.referenced_artifacts() {
            validate_artifact_descriptor(reference)?;
        }
        validate_portable_model(self)
    }
}

impl ArtifactModel for SourceRepositoryCandidate {
    const ARTIFACT_TYPE_URI: &'static str = SOURCE_REPOSITORY_CANDIDATE_TYPE_URI;

    fn validate(&self) -> ModelResult<()> {
        SourceRepositoryCandidate::validate(self)
    }

    fn referenced_artifacts(&self) -> Vec<&ArtifactDescriptor> {
        std::iter::once(&self.source_snapshot)
            .chain(&self.inspection_refs)
            .chain(&self.build_graph_candidates)
            .chain(&self.work_candidates)
            .chain(&self.operational_intent_candidates)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plurora_core::{
        CapabilityDescriptor, CapabilityRequirement, EntryDescriptor, PackageContributions,
        PackageEntry, PermissionSet, SandboxPolicy, COMPONENT_BEHAVIOR_TYPE_URI,
    };

    fn manifest() -> PackageManifest {
        PackageManifest {
            schema_version: 1,
            id: "example/package".to_string(),
            version: "1.0.0".to_string(),
            display_name: Some("Example Package".to_string()),
            description: Some("Example".to_string()),
            author: None,
            license: None,
            entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                crate_ref: "example-package".to_string(),
                symbol: "register".to_string(),
                abi_version: 1,
            }),
            provides: vec![
                CapabilityDescriptor {
                    id: "example/package/unary".to_string(),
                    version: "1.0.0".to_string(),
                    input_schema: Value::Null,
                    output_schema: Value::Null,
                    streaming: false,
                    side_effects: Vec::new(),
                    description: None,
                },
                CapabilityDescriptor {
                    id: "example/package/stream".to_string(),
                    version: "1.0.0".to_string(),
                    input_schema: Value::Null,
                    output_schema: Value::Null,
                    streaming: true,
                    side_effects: vec!["network".to_string()],
                    description: None,
                },
            ],
            consumes: vec![CapabilityRequirement {
                id: "example/provider/input".to_string(),
                version: "^1.0".to_string(),
            }],
            requires: Vec::new(),
            contributes: PackageContributions::default(),
            permissions: PermissionSet::default(),
            sandbox_policy: SandboxPolicy::default(),
        }
    }

    fn descriptor(kind: &str, byte: char) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: kind.to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", byte.to_string().repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    #[test]
    fn capability_projection_is_conservative_and_streaming_is_visible() {
        let ports = project_capability_ports(&manifest(), Vec::new()).unwrap();
        let unary = ports
            .iter()
            .find(|port| port.port_id.as_str() == "unary")
            .unwrap();
        let stream = ports
            .iter()
            .find(|port| port.port_id.as_str() == "stream")
            .unwrap();
        let input = ports
            .iter()
            .find(|port| port.port_id.as_str() == "input")
            .unwrap();
        assert_eq!(unary.contract.protocol_id, CAPABILITY_PROTOCOL_ID);
        assert_eq!(unary.contract.interface_id, "example/package/unary");
        assert_eq!(unary.interaction.0, INTERACTION_CAPABILITY_UNARY);
        assert_eq!(stream.interaction.0, INTERACTION_CAPABILITY_STREAM);
        assert!(matches!(
            unary.role,
            PortRole::Export {
                effect_class: EffectClass::ExternalEffecting,
                ..
            }
        ));
        assert_eq!(
            unary.annotations[CLAIM_STATUS_ANNOTATION],
            Value::String("legacy_adapted".to_string())
        );
        assert_eq!(
            stream.annotations[SIDE_EFFECTS_ANNOTATION],
            serde_json::json!(["network"])
        );
        assert!(matches!(
            &input.role,
            PortRole::Import {
                multiplicity: PortMultiplicity {
                    min: 1,
                    max: Some(1)
                },
                latest_binding_phase: BindingPhase::Installation,
                availability: AvailabilityPolicy::Required,
                accepted_effects
            } if accepted_effects == &vec![EffectClass::ExternalEffecting]
        ));
        assert!(input.transport.allowed_classes.is_empty());
    }

    #[test]
    fn projected_local_port_collisions_fail_structurally() {
        let mut value = manifest();
        value.provides[1].id = "other/namespace/unary".to_string();
        assert_eq!(
            project_capability_ports(&value, Vec::new())
                .unwrap_err()
                .code,
            DiagnosticCode::WorkInvalid
        );

        let explicit = PortDescriptor {
            port_id: PortId::parse("custom").unwrap(),
            contract: PortContract {
                protocol_id: "example.precise".to_string(),
                interface_id: "custom".to_string(),
                version: "1.0.0".to_string(),
                profiles: Vec::new(),
            },
            interaction: InteractionModelId(crate::INTERACTION_ARTIFACT.to_string()),
            role: PortRole::Export {
                multiplicity: PortMultiplicity {
                    min: 0,
                    max: Some(1),
                },
                effect_class: EffectClass::Pure,
            },
            transport: TransportRequirements::default(),
            annotations: BTreeMap::new(),
        };
        let projected = project_capability_ports(&manifest(), vec![explicit.clone()]).unwrap();
        assert!(projected.iter().any(|port| {
            port.port_id.as_str() == "custom"
                && matches!(
                    port.role,
                    PortRole::Export {
                        effect_class: EffectClass::Pure,
                        ..
                    }
                )
        }));
        let mut duplicate = explicit;
        duplicate.port_id = PortId::parse("unary").unwrap();
        assert_eq!(
            project_capability_ports(&manifest(), vec![duplicate])
                .unwrap_err()
                .code,
            DiagnosticCode::WorkInvalid
        );
    }

    #[test]
    fn package_normalization_preserves_component_identity_and_artifact_bytes() {
        let normalized = normalize_package_manifest(&manifest(), Vec::new()).unwrap();
        let package = normalized.package.as_ref().unwrap();
        assert_eq!(
            normalized.assembly.nodes[0].source.artifact(),
            &package.component.artifact
        );
        assert_eq!(normalized.work.work_id.as_str(), "example/package");
        assert!(normalized.artifacts.iter().all(|object| {
            object.descriptor.digest == world_bundle_sha256_digest(&object.bytes)
                && object.descriptor.size_bytes == object.bytes.len() as u64
        }));
        assert!(normalized
            .artifacts
            .iter()
            .any(|object| object.descriptor.artifact_type_uri == COMPONENT_BEHAVIOR_TYPE_URI));
        let available = normalized
            .artifacts
            .iter()
            .map(|object| object.descriptor.digest.as_str())
            .chain(
                normalized
                    .work
                    .content_roots
                    .iter()
                    .map(|root| root.digest.as_str()),
            )
            .collect::<BTreeSet<_>>();
        assert!(normalized.artifacts.iter().all(|object| object
            .descriptor
            .references
            .iter()
            .all(|reference| available.contains(reference.as_str()))));
    }

    #[test]
    fn port_semantics_change_normalized_assembly_and_work_digests() {
        let port = |version: &str| PortDescriptor {
            port_id: PortId::parse("custom").unwrap(),
            contract: PortContract {
                protocol_id: "example.precise".to_string(),
                interface_id: "custom".to_string(),
                version: version.to_string(),
                profiles: Vec::new(),
            },
            interaction: InteractionModelId(crate::INTERACTION_ARTIFACT.to_string()),
            role: PortRole::Export {
                multiplicity: PortMultiplicity {
                    min: 0,
                    max: Some(1),
                },
                effect_class: EffectClass::Pure,
            },
            transport: TransportRequirements::default(),
            annotations: BTreeMap::new(),
        };
        let first = normalize_package_manifest(&manifest(), vec![port("1.0.0")]).unwrap();
        let second = normalize_package_manifest(&manifest(), vec![port("2.0.0")]).unwrap();
        assert_ne!(
            first.assembly.digest().unwrap(),
            second.assembly.digest().unwrap()
        );
        assert_ne!(first.work.digest().unwrap(), second.work.digest().unwrap());
    }

    #[test]
    fn contract_none_package_requires_explicit_foreign_capsule_normalization() {
        let mut value = manifest();
        value.entry.contract = plurora_core::ContractMode::None;
        let projection = project_package_manifest(&value, Vec::new()).unwrap();
        assert_eq!(
            projection.component.trust_class,
            ComponentTrustClass::ForeignCapsule
        );
        assert!(projection.ports.is_empty());
        assert_eq!(
            normalize_package_manifest(&value, Vec::new())
                .unwrap_err()
                .code,
            DiagnosticCode::WorkInvalid
        );
    }

    #[test]
    fn package_work_normalization_rejects_host_paths_and_remote_coordinates() {
        let mut local = manifest();
        local.entry.kind = PackageEntry::Subprocess {
            command: vec!["C:\\host\\tool.exe".to_string()],
            transport: plurora_core::SubprocessTransport::JsonRpcStdio,
        };
        assert_eq!(
            normalize_package_manifest(&local, Vec::new())
                .unwrap_err()
                .code,
            DiagnosticCode::RawPath
        );

        let mut remote = manifest();
        remote.entry.kind = PackageEntry::Remote {
            endpoint: "https://service.example/rpc".to_string(),
            auth: plurora_core::RemoteAuth {
                scheme: "none".to_string(),
                config: Value::Null,
            },
        };
        assert_eq!(
            normalize_package_manifest(&remote, Vec::new())
                .unwrap_err()
                .code,
            DiagnosticCode::WorkInvalid
        );
    }

    #[test]
    fn package_projection_rejects_unknown_manifest_schema_versions() {
        let mut value = manifest();
        value.schema_version = 2;
        assert_eq!(
            project_package_manifest(&value, Vec::new())
                .unwrap_err()
                .code,
            DiagnosticCode::WorkInvalid
        );
    }

    #[test]
    fn foreign_content_and_repository_sources_have_distinct_normalized_shapes() {
        let rights = descriptor(RIGHTS_DECLARATION_TYPE_URI, 'a');
        let transparency = descriptor(TRANSPARENCY_DECLARATION_TYPE_URI, 'b');
        let foreign = normalize_foreign_capsule(
            WorkId::parse("example/foreign").unwrap(),
            "Foreign".to_string(),
            String::new(),
            ForeignCapsuleDescriptor {
                capsule_id: "example/foreign".to_string(),
                launch_requirements: vec![ForeignLaunchRequirement {
                    launch_id: "play".to_string(),
                    kind: ForeignLaunchKind::LocalExecutable,
                    required_protocols: Vec::new(),
                    annotations: BTreeMap::new(),
                }],
                protocol_ports: Vec::new(),
                state_slots: Vec::new(),
                rights,
                transparency,
                annotations: BTreeMap::new(),
            },
        )
        .unwrap();
        assert!(foreign.assembly.nodes.is_empty());
        assert!(matches!(
            foreign.work.entrypoints[0].target,
            WorkEntrypointTarget::ForeignLaunch { .. }
        ));
        assert_eq!(
            foreign.work.entrypoints[0].annotations[FOREIGN_LAUNCH_KIND_ANNOTATION],
            Value::String("local_executable".to_string())
        );

        let content = descriptor("urn:example:content:v1", 'c');
        let content_only = normalize_content_only(
            WorkId::parse("example/content").unwrap(),
            "Content".to_string(),
            String::new(),
            vec![content.clone()],
            vec![WorkEntrypoint {
                id: "inspect".to_string(),
                intent_uri: "plurora.shell.default/inspect".to_string(),
                target: WorkEntrypointTarget::Surface {
                    surface_id: "example/content/inspect".to_string(),
                },
                annotations: BTreeMap::new(),
            }],
            BTreeMap::new(),
        )
        .unwrap();
        assert!(content_only.assembly.nodes.is_empty());
        assert_eq!(content_only.work.content_roots, vec![content.clone()]);

        let repository = SourceRepositoryCandidate::new(
            "candidate".to_string(),
            content,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            BTreeMap::new(),
        )
        .unwrap();
        assert!(repository.work_candidates.is_empty());
        assert_eq!(
            repository.artifact_descriptor().unwrap().artifact_type_uri,
            SOURCE_REPOSITORY_CANDIDATE_TYPE_URI
        );
    }

    #[test]
    fn portable_foreign_capsule_rejects_concrete_local_location() {
        let capsule = ForeignCapsuleDescriptor {
            capsule_id: "example/foreign".to_string(),
            launch_requirements: vec![ForeignLaunchRequirement {
                launch_id: "play".to_string(),
                kind: ForeignLaunchKind::LocalExecutable,
                required_protocols: Vec::new(),
                annotations: BTreeMap::from([(
                    "location".to_string(),
                    Value::String("C:\\Games\\game.exe".to_string()),
                )]),
            }],
            protocol_ports: Vec::new(),
            state_slots: Vec::new(),
            rights: descriptor(RIGHTS_DECLARATION_TYPE_URI, 'a'),
            transparency: descriptor(TRANSPARENCY_DECLARATION_TYPE_URI, 'b'),
            annotations: BTreeMap::new(),
        };
        assert_eq!(
            capsule.validate().unwrap_err().code,
            DiagnosticCode::RawPath
        );

        let mut remote = capsule;
        remote.launch_requirements[0].annotations = BTreeMap::from([(
            "uri".to_string(),
            Value::String("https://service.example/run".to_string()),
        )]);
        assert_eq!(remote.validate().unwrap_err().code, DiagnosticCode::RawPath);

        let mut nested = remote;
        nested.launch_requirements[0].annotations.clear();
        nested.annotations = BTreeMap::from([(
            "thirdparty.example/metadata".to_string(),
            serde_json::json!({"endpoint": "opaque-service-coordinate"}),
        )]);
        assert_eq!(nested.validate().unwrap_err().code, DiagnosticCode::RawPath);
    }
}
