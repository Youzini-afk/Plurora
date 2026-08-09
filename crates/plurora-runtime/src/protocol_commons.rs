use std::collections::HashSet;
use std::sync::OnceLock;

use plurora_core::{
    NegotiatedProtocol, ProtocolAuthorityRequirement, ProtocolCompatibilityProfile,
    ProtocolConformanceVector, ProtocolDescriptor, ProtocolDocumentReference,
    ProtocolImplementationClaim, ProtocolMaturity, ProtocolSchemaKind, ProtocolSchemaReference,
    ProtocolSelection, PROTOCOL_DESCRIPTOR_TYPE_URI,
};
use serde_json::json;

use crate::ProtocolError;

pub const PROTOCOL_COMMONS_REGISTRY_VERSION: &str = "0.2.0";
pub const CHANGE_PROTOCOL_ID: &str = "plurora.change";
pub const CHANGE_PROTOCOL_VERSION: &str = "1.0.0";
pub const CHANGE_DEFAULT_PROFILE: &str = "plurora.change/default/v1";
pub const SHELL_PROTOCOL_ID: &str = "plurora.shell.default";
pub const SHELL_PROTOCOL_VERSION: &str = "1.0.0";
pub const SHELL_PROTOCOL_PROFILE: &str = "plurora.shell.default/v1";
pub const WORK_PROTOCOL_ID: &str = "plurora.work";
pub const WORK_PROTOCOL_VERSION: &str = "1.0.0";
pub const WORK_EXPERIMENTAL_PROFILE: &str = "plurora.work/experimental/v1";
pub const ASSEMBLY_PROTOCOL_ID: &str = "plurora.assembly";
pub const ASSEMBLY_PROTOCOL_VERSION: &str = "1.0.0";
pub const ASSEMBLY_EXPERIMENTAL_PROFILE: &str = "plurora.assembly/experimental/v1";
pub const WORLD_BUNDLE_PROTOCOL_ID: &str = "plurora.world.bundle";
pub const WORLD_BUNDLE_PROTOCOL_VERSION: &str = "1.0.0";
pub const WORLD_BUNDLE_EXPERIMENTAL_PROFILE: &str = "plurora.world.bundle/experimental/v1";

static PROTOCOL_DESCRIPTORS: OnceLock<Vec<ProtocolDescriptor>> = OnceLock::new();

pub fn protocol_descriptors() -> &'static [ProtocolDescriptor] {
    PROTOCOL_DESCRIPTORS
        .get_or_init(|| {
            let descriptors = vec![
                change_protocol_descriptor(),
                shell_protocol_descriptor(),
                work_protocol_descriptor(),
                assembly_protocol_descriptor(),
                world_bundle_protocol_descriptor(),
            ];
            validate_protocol_registry(&descriptors)
                .expect("built-in Protocol Commons descriptors must be valid");
            descriptors
        })
        .as_slice()
}

pub fn protocol_descriptor(protocol_id: &str) -> Option<&'static ProtocolDescriptor> {
    protocol_descriptors()
        .iter()
        .find(|descriptor| descriptor.protocol_id == protocol_id)
}

pub fn negotiate_protocols(
    selections: &[ProtocolSelection],
) -> Result<Vec<NegotiatedProtocol>, ProtocolError> {
    let mut seen_requested = HashSet::new();
    let mut seen_negotiated = HashSet::new();
    let mut negotiated = Vec::with_capacity(selections.len());
    for selection in selections {
        if !seen_requested.insert(selection.protocol_id.as_str()) {
            return Err(ProtocolError::invalid_request(format!(
                "protocol selection contains duplicate protocol id '{}'",
                selection.protocol_id
            )));
        }
        let result = negotiate_protocol(selection)?;
        if !seen_negotiated.insert(result.protocol_id.clone()) {
            return Err(ProtocolError::invalid_request(format!(
                "protocol selection resolves more than once to '{}'",
                result.protocol_id
            )));
        }
        negotiated.push(result);
    }
    Ok(negotiated)
}

pub fn validate_protocol_registry(descriptors: &[ProtocolDescriptor]) -> Result<(), String> {
    let mut protocol_ids = HashSet::new();
    let mut implementation_ids = HashSet::new();
    for descriptor in descriptors {
        if descriptor.descriptor_type_uri != PROTOCOL_DESCRIPTOR_TYPE_URI {
            return Err(format!(
                "protocol '{}' has unsupported descriptor type '{}'",
                descriptor.protocol_id, descriptor.descriptor_type_uri
            ));
        }
        if !protocol_ids.insert(descriptor.protocol_id.as_str()) {
            return Err(format!(
                "duplicate protocol id '{}'",
                descriptor.protocol_id
            ));
        }
        parse_major(&descriptor.version).ok_or_else(|| {
            format!(
                "protocol '{}' has invalid version '{}'",
                descriptor.protocol_id, descriptor.version
            )
        })?;

        let profiles = descriptor
            .compatibility_profiles
            .iter()
            .map(|profile| profile.id.as_str())
            .collect::<HashSet<_>>();
        if profiles.len() != descriptor.compatibility_profiles.len() {
            return Err(format!(
                "protocol '{}' contains duplicate compatibility profiles",
                descriptor.protocol_id
            ));
        }
        if profiles.is_empty() {
            return Err(format!(
                "protocol '{}' has no compatibility profile",
                descriptor.protocol_id
            ));
        }

        let vectors = descriptor
            .conformance_vectors
            .iter()
            .map(|vector| vector.id.as_str())
            .collect::<HashSet<_>>();
        if vectors.len() != descriptor.conformance_vectors.len() {
            return Err(format!(
                "protocol '{}' contains duplicate conformance vectors",
                descriptor.protocol_id
            ));
        }
        for vector in &descriptor.conformance_vectors {
            if vector
                .profiles
                .iter()
                .any(|profile| !profiles.contains(profile.as_str()))
            {
                return Err(format!(
                    "protocol vector '{}' references an unknown profile for '{}'",
                    vector.id, descriptor.protocol_id
                ));
            }
        }
        let mut migrations = HashSet::new();
        for migration in &descriptor.migrations {
            if migration.to_version != descriptor.version {
                return Err(format!(
                    "migration '{}' targets '{}' instead of protocol version '{}'",
                    migration.adapter_id, migration.to_version, descriptor.version
                ));
            }
            if !migrations.insert((
                migration.from_protocol_id.as_str(),
                migration.from_version.as_str(),
            )) {
                return Err(format!(
                    "protocol '{}' contains a duplicate migration source",
                    descriptor.protocol_id
                ));
            }
        }
        let required_vectors = descriptor
            .conformance_vectors
            .iter()
            .filter(|vector| vector.required)
            .map(|vector| vector.id.as_str())
            .collect::<HashSet<_>>();
        for implementation in &descriptor.conforming_implementations {
            if !implementation_ids.insert(implementation.implementation_id.as_str()) {
                return Err(format!(
                    "duplicate implementation id '{}'",
                    implementation.implementation_id
                ));
            }
            if implementation.version != descriptor.version {
                return Err(format!(
                    "implementation '{}' claims version '{}' for protocol '{}@{}'",
                    implementation.implementation_id,
                    implementation.version,
                    descriptor.protocol_id,
                    descriptor.version
                ));
            }
            if implementation
                .profiles
                .iter()
                .any(|profile| !profiles.contains(profile.as_str()))
            {
                return Err(format!(
                    "implementation '{}' claims an unknown profile for protocol '{}'",
                    implementation.implementation_id, descriptor.protocol_id
                ));
            }
            let implementation_vectors = implementation
                .conformance_vectors
                .iter()
                .map(String::as_str)
                .collect::<HashSet<_>>();
            if !required_vectors.is_subset(&implementation_vectors)
                || implementation_vectors
                    .iter()
                    .any(|vector| !vectors.contains(vector))
            {
                return Err(format!(
                    "implementation '{}' does not use the protocol-owned conformance vector set for '{}'",
                    implementation.implementation_id, descriptor.protocol_id
                ));
            }
        }
    }
    Ok(())
}

fn negotiate_protocol(selection: &ProtocolSelection) -> Result<NegotiatedProtocol, ProtocolError> {
    if let Some(descriptor) = protocol_descriptor(&selection.protocol_id) {
        if descriptor.version == selection.version {
            return negotiate_descriptor(descriptor, selection, None);
        }

        let requested_major = parse_major(&selection.version);
        let supported_major = parse_major(&descriptor.version);
        let adapter = descriptor.migrations.iter().find(|migration| {
            migration.from_protocol_id == selection.protocol_id
                && migration.from_version == selection.version
        });
        if let Some(adapter) = adapter {
            return negotiate_descriptor(descriptor, selection, Some(adapter.adapter_id.clone()));
        }
        let reason = if requested_major.is_some() && requested_major != supported_major {
            "protocol_major_mismatch"
        } else {
            "unsupported_protocol_version"
        };
        return Err(protocol_negotiation_error(
            reason,
            selection,
            json!({
                "supported_version": descriptor.version,
                "requested_major": requested_major,
                "supported_major": supported_major,
                "available_adapters": descriptor.migrations,
            }),
        ));
    }

    for descriptor in protocol_descriptors() {
        if let Some(adapter) = descriptor.migrations.iter().find(|migration| {
            migration.from_protocol_id == selection.protocol_id
                && migration.from_version == selection.version
        }) {
            return negotiate_descriptor(descriptor, selection, Some(adapter.adapter_id.clone()));
        }
    }

    Err(protocol_negotiation_error(
        "unknown_protocol",
        selection,
        json!({
            "supported_protocols": protocol_descriptors()
                .iter()
                .map(|descriptor| json!({
                    "protocol_id": descriptor.protocol_id,
                    "version": descriptor.version,
                }))
                .collect::<Vec<_>>(),
        }),
    ))
}

fn negotiate_descriptor(
    descriptor: &ProtocolDescriptor,
    selection: &ProtocolSelection,
    adapter_id: Option<String>,
) -> Result<NegotiatedProtocol, ProtocolError> {
    let profile = selection
        .profile
        .as_deref()
        .or_else(|| {
            descriptor
                .compatibility_profiles
                .first()
                .map(|profile| profile.id.as_str())
        })
        .ok_or_else(|| {
            protocol_negotiation_error(
                "protocol_has_no_profile",
                selection,
                json!({"protocol_id": descriptor.protocol_id}),
            )
        })?;
    if !descriptor
        .compatibility_profiles
        .iter()
        .any(|candidate| candidate.id == profile)
    {
        return Err(protocol_negotiation_error(
            "unsupported_protocol_profile",
            selection,
            json!({
                "protocol_id": descriptor.protocol_id,
                "requested_profile": profile,
                "supported_profiles": descriptor.compatibility_profiles,
            }),
        ));
    }

    Ok(NegotiatedProtocol {
        protocol_id: descriptor.protocol_id.clone(),
        requested_version: selection.version.clone(),
        negotiated_version: descriptor.version.clone(),
        maturity: descriptor.maturity,
        profile: profile.to_string(),
        adapter_id,
    })
}

fn protocol_negotiation_error(
    reason: &str,
    selection: &ProtocolSelection,
    details: serde_json::Value,
) -> ProtocolError {
    ProtocolError::new(
        "runtime/error/unsupported_protocol",
        format!("requested protocol cannot be satisfied: {reason}"),
        json!({
            "reason": reason,
            "selection": selection,
            "details": details,
        }),
    )
}

fn parse_major(version: &str) -> Option<u64> {
    version
        .trim_start_matches('v')
        .split('.')
        .next()?
        .parse()
        .ok()
}

fn change_protocol_descriptor() -> ProtocolDescriptor {
    let vectors = vec![
        vector(
            "proposal.lifecycle_apply",
            "Intent through approved commit produces operation and terminal receipts.",
            CHANGE_DEFAULT_PROFILE,
        ),
        vector(
            "proposal.reject_and_apply_denied",
            "A denied policy decision cannot be applied.",
            CHANGE_DEFAULT_PROFILE,
        ),
        vector(
            "proposal.authority_is_enforced",
            "Approval, rejection, and apply enforce scoped authority.",
            CHANGE_DEFAULT_PROFILE,
        ),
        vector(
            "proposal.preflight_failure_is_structured",
            "Preflight failure is terminal, structured, and does not silently half-commit.",
            CHANGE_DEFAULT_PROFILE,
        ),
    ];
    let vector_ids: Vec<String> = vectors.iter().map(|vector| vector.id.clone()).collect();
    ProtocolDescriptor {
        descriptor_type_uri: PROTOCOL_DESCRIPTOR_TYPE_URI.to_string(),
        protocol_id: CHANGE_PROTOCOL_ID.to_string(),
        version: CHANGE_PROTOCOL_VERSION.to_string(),
        maturity: ProtocolMaturity::Experimental,
        schemas: [
            ("intent", "intent.schema.json"),
            ("change-set", "change-set.schema.json"),
            ("policy-decision", "policy-decision.schema.json"),
            ("commit", "commit.schema.json"),
            ("effect-receipt", "effect-receipt.schema.json"),
        ]
        .into_iter()
        .map(|(id, filename)| json_schema(id, filename))
        .collect(),
        wit_worlds: Vec::new(),
        semantic_specification: spec("CHANGE_WORKFLOW.en.md"),
        lifecycle: spec("CHANGE_WORKFLOW.en.md#apply-lifecycle"),
        error_model: spec("CHANGE_WORKFLOW.en.md#compatibility-boundary"),
        authority_requirements: vec![ProtocolAuthorityRequirement {
            authority: "change.proposal.*".to_string(),
            scope: "proposal and operation targets".to_string(),
            operations: vec![
                "create".to_string(),
                "approve".to_string(),
                "reject".to_string(),
                "apply".to_string(),
            ],
        }],
        conformance_vectors: vectors,
        compatibility_profiles: vec![profile(
            CHANGE_DEFAULT_PROFILE,
            CHANGE_PROTOCOL_VERSION,
            "Proposal-compatible Intent/ChangeSet/PolicyDecision/Commit workflow.",
        )],
        migrations: Vec::new(),
        conforming_implementations: vec![
            ProtocolImplementationClaim {
                implementation_id: "plurora.runtime.change-proposal".to_string(),
                provider: "plurora-runtime".to_string(),
                version: CHANGE_PROTOCOL_VERSION.to_string(),
                profiles: vec![CHANGE_DEFAULT_PROFILE.to_string()],
                conformance_vectors: vector_ids.clone(),
                test_only: false,
            },
            ProtocolImplementationClaim {
                implementation_id: "org.example.change-reference".to_string(),
                provider: "third-party-conformance-fixture".to_string(),
                version: CHANGE_PROTOCOL_VERSION.to_string(),
                profiles: vec![CHANGE_DEFAULT_PROFILE.to_string()],
                conformance_vectors: vector_ids,
                test_only: true,
            },
        ],
    }
}

fn shell_protocol_descriptor() -> ProtocolDescriptor {
    let vectors = vec![
        vector(
            "surface.contribution_list",
            "Contributions are discoverable through the public shell registry.",
            SHELL_PROTOCOL_PROFILE,
        ),
        vector(
            "surface.shell_descriptor_metadata_validation",
            "Structured shell descriptors enforce bounded metadata and package ownership.",
            SHELL_PROTOCOL_PROFILE,
        ),
    ];
    ProtocolDescriptor {
        descriptor_type_uri: PROTOCOL_DESCRIPTOR_TYPE_URI.to_string(),
        protocol_id: SHELL_PROTOCOL_ID.to_string(),
        version: SHELL_PROTOCOL_VERSION.to_string(),
        maturity: ProtocolMaturity::Experimental,
        schemas: vec![json_schema("manifest", "manifest.schema.json")],
        wit_worlds: Vec::new(),
        semantic_specification: guide("SURFACE_HOSTING.en.md"),
        lifecycle: guide("SURFACE_HOSTING.en.md#host-api"),
        error_model: guide("SURFACE_HOSTING.en.md#host-bridge"),
        authority_requirements: vec![ProtocolAuthorityRequirement {
            authority: "public protocol plus explicit surface bridge allowlist".to_string(),
            scope: "mounted surface instance and session".to_string(),
            operations: vec![
                "discover".to_string(),
                "resolve".to_string(),
                "bridge.call".to_string(),
            ],
        }],
        conformance_vectors: vectors.clone(),
        compatibility_profiles: vec![profile(
            SHELL_PROTOCOL_PROFILE,
            SHELL_PROTOCOL_VERSION,
            "Default structured contributions and sandboxed surface bridge vocabulary.",
        )],
        migrations: Vec::new(),
        conforming_implementations: vec![ProtocolImplementationClaim {
            implementation_id: "plurora.runtime.shell-default".to_string(),
            provider: "plurora-runtime".to_string(),
            version: SHELL_PROTOCOL_VERSION.to_string(),
            profiles: vec![SHELL_PROTOCOL_PROFILE.to_string()],
            conformance_vectors: vectors.iter().map(|vector| vector.id.clone()).collect(),
            test_only: false,
        }],
    }
}

fn work_protocol_descriptor() -> ProtocolDescriptor {
    let vectors = vec![
        vector(
            "work.canonical_digest_stable",
            "Equivalent Work metadata produces identical canonical bytes and SHA-256 identity.",
            WORK_EXPERIMENTAL_PROFILE,
        ),
        vector(
            "work.portable_identity_redlines",
            "Portable Work artifacts reject raw secrets and host-local filesystem paths.",
            WORK_EXPERIMENTAL_PROFILE,
        ),
        vector(
            "work.unknown_annotations_preserved",
            "Unknown annotations survive lossless read, validation, and transfer.",
            WORK_EXPERIMENTAL_PROFILE,
        ),
    ];
    ProtocolDescriptor {
        descriptor_type_uri: PROTOCOL_DESCRIPTOR_TYPE_URI.to_string(),
        protocol_id: WORK_PROTOCOL_ID.to_string(),
        version: WORK_PROTOCOL_VERSION.to_string(),
        maturity: ProtocolMaturity::Experimental,
        schemas: [
            ("work-revision", "work-revision.schema.json"),
            ("rights-declaration", "rights-declaration.schema.json"),
            (
                "transparency-declaration",
                "transparency-declaration.schema.json",
            ),
            ("operational-intent", "operational-intent.schema.json"),
        ]
        .into_iter()
        .map(|(id, filename)| json_schema(id, filename))
        .collect(),
        wit_worlds: Vec::new(),
        semantic_specification: protocol_commons_spec("work-experimental-profile"),
        lifecycle: protocol_commons_spec("work-artifact-lifecycle"),
        error_model: protocol_commons_spec("work-and-assembly-error-model"),
        authority_requirements: vec![ProtocolAuthorityRequirement {
            authority: "object.write".to_string(),
            scope: "explicit content-addressed artifact destination".to_string(),
            operations: vec!["persist".to_string()],
        }],
        conformance_vectors: vectors.clone(),
        compatibility_profiles: vec![profile(
            WORK_EXPERIMENTAL_PROFILE,
            WORK_PROTOCOL_VERSION,
            "Portable immutable Work definitions with explicit artifact references.",
        )],
        migrations: Vec::new(),
        conforming_implementations: vec![ProtocolImplementationClaim {
            implementation_id: "plurora.work.model".to_string(),
            provider: "plurora-work".to_string(),
            version: WORK_PROTOCOL_VERSION.to_string(),
            profiles: vec![WORK_EXPERIMENTAL_PROFILE.to_string()],
            conformance_vectors: vectors.iter().map(|vector| vector.id.clone()).collect(),
            test_only: false,
        }],
    }
}

fn assembly_protocol_descriptor() -> ProtocolDescriptor {
    let vectors = vec![
        vector(
            "assembly.containment_acyclic",
            "Nested Assembly containment is complete, digest-bound, bounded, and acyclic.",
            ASSEMBLY_EXPERIMENTAL_PROFILE,
        ),
        vector(
            "assembly.port_contract_valid",
            "Bindings validate role, protocol, interface, version, profile, interaction, effect, transport, and cardinality.",
            ASSEMBLY_EXPERIMENTAL_PROFILE,
        ),
        vector(
            "assembly.state_slot_valid",
            "Portable state declares a schema and durable replacement requires migration or explicit reset.",
            ASSEMBLY_EXPERIMENTAL_PROFILE,
        ),
    ];
    ProtocolDescriptor {
        descriptor_type_uri: PROTOCOL_DESCRIPTOR_TYPE_URI.to_string(),
        protocol_id: ASSEMBLY_PROTOCOL_ID.to_string(),
        version: ASSEMBLY_PROTOCOL_VERSION.to_string(),
        maturity: ProtocolMaturity::Experimental,
        schemas: [
            ("port-descriptor", "port-descriptor.schema.json"),
            ("assembly-revision", "assembly-revision.schema.json"),
            (
                "state-slot-descriptor",
                "state-slot-descriptor.schema.json",
            ),
            ("assembly-lock", "assembly-lock.schema.json"),
        ]
        .into_iter()
        .map(|(id, filename)| json_schema(id, filename))
        .collect(),
        wit_worlds: Vec::new(),
        semantic_specification: protocol_commons_spec("assembly-experimental-profile"),
        lifecycle: protocol_commons_spec("assembly-artifact-lifecycle"),
        error_model: protocol_commons_spec("work-and-assembly-error-model"),
        authority_requirements: vec![ProtocolAuthorityRequirement {
            authority: "object.write".to_string(),
            scope: "explicit content-addressed artifact destination".to_string(),
            operations: vec!["persist".to_string()],
        }],
        conformance_vectors: vectors.clone(),
        compatibility_profiles: vec![profile(
            ASSEMBLY_EXPERIMENTAL_PROFILE,
            ASSEMBLY_PROTOCOL_VERSION,
            "Recursive Component and nested Assembly graphs with typed Ports, Bindings, and State Slots.",
        )],
        migrations: Vec::new(),
        conforming_implementations: vec![ProtocolImplementationClaim {
            implementation_id: "plurora.assembly.model".to_string(),
            provider: "plurora-work".to_string(),
            version: ASSEMBLY_PROTOCOL_VERSION.to_string(),
            profiles: vec![ASSEMBLY_EXPERIMENTAL_PROFILE.to_string()],
            conformance_vectors: vectors.iter().map(|vector| vector.id.clone()).collect(),
            test_only: false,
        }],
    }
}

fn world_bundle_protocol_descriptor() -> ProtocolDescriptor {
    let vectors = vec![
        vector(
            "world_bundle.reference_closure",
            "Every referenced object is present and digest verified.",
            WORLD_BUNDLE_EXPERIMENTAL_PROFILE,
        ),
        vector(
            "world_bundle.cross_host_import",
            "A fresh host imports the same objects, lineage, envelopes, and receipts.",
            WORLD_BUNDLE_EXPERIMENTAL_PROFILE,
        ),
        vector(
            "world_bundle.offline_replay",
            "Historical replay succeeds with providers and outbound executors disabled.",
            WORLD_BUNDLE_EXPERIMENTAL_PROFILE,
        ),
        vector(
            "world_bundle.reexecution_branch",
            "A different implementation re-executes on a new causal branch.",
            WORLD_BUNDLE_EXPERIMENTAL_PROFILE,
        ),
        vector(
            "world_bundle.shell_independence",
            "A headless client reads the same world without Web-shell state.",
            WORLD_BUNDLE_EXPERIMENTAL_PROFILE,
        ),
    ];
    ProtocolDescriptor {
        descriptor_type_uri: PROTOCOL_DESCRIPTOR_TYPE_URI.to_string(),
        protocol_id: WORLD_BUNDLE_PROTOCOL_ID.to_string(),
        version: WORLD_BUNDLE_PROTOCOL_VERSION.to_string(),
        maturity: ProtocolMaturity::Experimental,
        schemas: [
            ("artifact-descriptor", "artifact-descriptor.schema.json"),
            ("event-envelope", "event-envelope.schema.json"),
            ("effect-receipt", "effect-receipt.schema.json"),
            ("world-bundle", "world-bundle.schema.json"),
            ("world-head", "world-head.schema.json"),
            ("world-journal-range", "world-journal-range.schema.json"),
        ]
        .into_iter()
        .map(|(id, filename)| json_schema(id, filename))
        .collect(),
        wit_worlds: Vec::new(),
        semantic_specification: protocol_commons_spec("world-bundle-experimental-profile"),
        lifecycle: protocol_commons_spec("world-bundle-lifecycle"),
        error_model: protocol_commons_spec("world-bundle-error-model"),
        authority_requirements: vec![ProtocolAuthorityRequirement {
            authority: "journal.read + object.read + explicit import authority".to_string(),
            scope: "selected world head and complete reference closure".to_string(),
            operations: vec![
                "export".to_string(),
                "audit".to_string(),
                "import".to_string(),
                "historical_replay".to_string(),
                "reexecute_on_branch".to_string(),
            ],
        }],
        conformance_vectors: vectors.clone(),
        compatibility_profiles: vec![profile(
            WORLD_BUNDLE_EXPERIMENTAL_PROFILE,
            WORLD_BUNDLE_PROTOCOL_VERSION,
            "Portable object, journal, receipt, lineage, policy, and assembly closure.",
        )],
        migrations: Vec::new(),
        conforming_implementations: vec![ProtocolImplementationClaim {
            implementation_id: "plurora.runtime.world-bundle".to_string(),
            provider: "plurora-runtime".to_string(),
            version: WORLD_BUNDLE_PROTOCOL_VERSION.to_string(),
            profiles: vec![WORLD_BUNDLE_EXPERIMENTAL_PROFILE.to_string()],
            conformance_vectors: vectors.iter().map(|vector| vector.id.clone()).collect(),
            test_only: false,
        }],
    }
}

fn vector(id: &str, description: &str, profile: &str) -> ProtocolConformanceVector {
    ProtocolConformanceVector {
        id: id.to_string(),
        description: description.to_string(),
        profiles: vec![profile.to_string()],
        required: true,
    }
}

fn profile(id: &str, version: &str, description: &str) -> ProtocolCompatibilityProfile {
    ProtocolCompatibilityProfile {
        id: id.to_string(),
        version: version.to_string(),
        maturity: ProtocolMaturity::Experimental,
        description: description.to_string(),
    }
}

fn json_schema(id: &str, filename: &str) -> ProtocolSchemaReference {
    ProtocolSchemaReference {
        id: id.to_string(),
        version: "1".to_string(),
        kind: ProtocolSchemaKind::JsonSchema,
        uri: format!("https://plurora.dev/spec/v1/schemas/{filename}"),
    }
}

fn spec(path: &str) -> ProtocolDocumentReference {
    ProtocolDocumentReference {
        uri: format!("https://plurora.dev/spec/{path}"),
        media_type: "text/markdown".to_string(),
        digest: None,
    }
}

fn guide(path: &str) -> ProtocolDocumentReference {
    ProtocolDocumentReference {
        uri: format!("https://plurora.dev/guides/{path}"),
        media_type: "text/markdown".to_string(),
        digest: None,
    }
}

fn protocol_commons_spec(anchor: &str) -> ProtocolDocumentReference {
    ProtocolDocumentReference {
        uri: format!("https://plurora.dev/spec/PROTOCOL_COMMONS.en.md#{anchor}"),
        media_type: "text/markdown".to_string(),
        digest: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_the_five_explicit_protocols() {
        assert_eq!(
            protocol_descriptors()
                .iter()
                .map(|descriptor| descriptor.protocol_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                CHANGE_PROTOCOL_ID,
                SHELL_PROTOCOL_ID,
                WORK_PROTOCOL_ID,
                ASSEMBLY_PROTOCOL_ID,
                WORLD_BUNDLE_PROTOCOL_ID,
            ]
        );
    }

    #[test]
    fn work_and_assembly_remain_experimental_and_claim_their_vectors() {
        for id in [WORK_PROTOCOL_ID, ASSEMBLY_PROTOCOL_ID] {
            let descriptor = protocol_descriptor(id).unwrap();
            assert_eq!(descriptor.maturity, ProtocolMaturity::Experimental);
            assert_eq!(descriptor.compatibility_profiles.len(), 1);
            assert_eq!(
                descriptor.compatibility_profiles[0].maturity,
                ProtocolMaturity::Experimental
            );
            assert_eq!(descriptor.conforming_implementations.len(), 1);
            let required = descriptor
                .conformance_vectors
                .iter()
                .map(|vector| vector.id.as_str())
                .collect::<HashSet<_>>();
            let claimed = descriptor.conforming_implementations[0]
                .conformance_vectors
                .iter()
                .map(String::as_str)
                .collect::<HashSet<_>>();
            assert_eq!(claimed, required);
        }
    }

    #[test]
    fn first_party_and_third_party_change_implementations_share_vectors() {
        let descriptor = protocol_descriptor(CHANGE_PROTOCOL_ID).unwrap();
        let expected = descriptor
            .conformance_vectors
            .iter()
            .filter(|vector| vector.required)
            .map(|vector| vector.id.as_str())
            .collect::<HashSet<_>>();
        assert_eq!(descriptor.conforming_implementations.len(), 2);
        for implementation in &descriptor.conforming_implementations {
            assert_eq!(
                implementation
                    .conformance_vectors
                    .iter()
                    .map(String::as_str)
                    .collect::<HashSet<_>>(),
                expected
            );
        }
    }

    #[test]
    fn major_mismatch_is_rejected_without_an_adapter() {
        let error = negotiate_protocols(&[ProtocolSelection {
            protocol_id: CHANGE_PROTOCOL_ID.to_string(),
            version: "2.0.0".to_string(),
            profile: None,
        }])
        .unwrap_err();
        assert_eq!(error.code, "runtime/error/unsupported_protocol");
        assert_eq!(error.details["reason"], "protocol_major_mismatch");
    }

    #[test]
    fn removed_protocol_ids_do_not_resolve() {
        let error = negotiate_protocols(&[ProtocolSelection {
            protocol_id: "platform.proposal".to_string(),
            version: "1.0.0".to_string(),
            profile: Some(CHANGE_DEFAULT_PROFILE.to_string()),
        }])
        .unwrap_err();
        assert_eq!(error.code, "runtime/error/unsupported_protocol");
        assert_eq!(error.details["reason"], "unknown_protocol");
    }
}
