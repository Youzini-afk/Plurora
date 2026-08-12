//! Foreign Work, Rights, transparency, and opaque-state vectors.
//!
//! Stateful backup and launch lifecycle tests live beside the production
//! Installation and Runtime implementations. These cases keep the portable
//! model and public wire boundary honest without publishing local coordinates.

use std::collections::BTreeMap;

use plurora_core::{ArtifactDescriptor, PackageId};
use plurora_runtime::{
    foreign_launch_secret_ref, rights_policy_outcome, ForeignEntitlementAdapter,
    ForeignLaunchBinding, ForeignLaunchTarget, PlatformMethod, RightsPolicyOutcome,
    FOREIGN_LAUNCH_BINDING_SCHEMA,
};
use plurora_work::{
    check_port_compatibility, normalize_foreign_capsule, AvailabilityPolicy, BindingPhase,
    ClaimStatus, DiagnosticCode, EffectClass, ForeignCapsuleDescriptor, ForeignLaunchKind,
    ForeignLaunchRequirement, InteractionModelId, NormalizedWorkKind, PortContract, PortDescriptor,
    PortId, PortMultiplicity, PortRole, RightDisposition, RightsDeclaration, RightsOperation,
    SourceVisibility, StatePortability, TransparencyDeclaration, TransportRequirements,
    WorkEntrypointTarget, WorkId, FOREIGN_CAPSULE_TYPE_URI, FOREIGN_DEDICATED_SERVER_ANNOTATION,
    FOREIGN_DEDICATED_SERVER_INTENT_URI, INTERACTION_ARTIFACT, RIGHTS_DECLARATION_TYPE_URI,
    TRANSPARENCY_DECLARATION_TYPE_URI,
};
use serde_json::{json, Value};

use crate::schema_export::{method_schema, method_schemas};

fn descriptor(artifact_type_uri: &str, digit: char) -> ArtifactDescriptor {
    ArtifactDescriptor {
        artifact_type_uri: artifact_type_uri.to_string(),
        media_type: "application/json".to_string(),
        digest: format!("sha256:{}", digit.to_string().repeat(64)),
        size_bytes: 2,
        references: Vec::new(),
        annotations: BTreeMap::new(),
    }
}

fn capsule(requirement: ForeignLaunchRequirement) -> ForeignCapsuleDescriptor {
    ForeignCapsuleDescriptor {
        capsule_id: "vendor/opaque-game".to_string(),
        launch_requirements: vec![requirement],
        protocol_ports: Vec::new(),
        state_slots: Vec::new(),
        rights: descriptor(RIGHTS_DECLARATION_TYPE_URI, 'a'),
        transparency: descriptor(TRANSPARENCY_DECLARATION_TYPE_URI, 'b'),
        annotations: BTreeMap::new(),
    }
}

fn rights(disposition: RightDisposition) -> RightsDeclaration {
    RightsDeclaration {
        license_expression: None,
        terms_uri: None,
        install: RightDisposition::Allowed,
        execute: RightDisposition::Allowed,
        backup: disposition,
        export_state: disposition,
        copy_across_hosts: disposition,
        redistribute_artifacts: disposition,
        modify: disposition,
        derive: disposition,
        modding: disposition,
        dedicated_server: disposition,
        entitlement_requirements: Vec::new(),
        evidence_refs: Vec::new(),
    }
}

fn protocol_port(role: PortRole, version: &str) -> PortDescriptor {
    PortDescriptor {
        port_id: PortId::parse("save").expect("fixed port id"),
        contract: PortContract {
            protocol_id: "game.save".to_string(),
            interface_id: "game.save/read".to_string(),
            version: version.to_string(),
            profiles: vec!["opaque-snapshot".to_string()],
        },
        interaction: InteractionModelId(INTERACTION_ARTIFACT.to_string()),
        role,
        transport: TransportRequirements::default(),
        annotations: BTreeMap::new(),
    }
}

async fn open_without_protocols_is_still_a_capsule_and_paths_are_rejected() -> anyhow::Result<()> {
    let requirement = ForeignLaunchRequirement {
        launch_id: "play".to_string(),
        kind: ForeignLaunchKind::LocalExecutable,
        required_protocols: Vec::new(),
        annotations: BTreeMap::new(),
    };
    let normalized = normalize_foreign_capsule(
        WorkId::parse("vendor/opaque-game")?,
        "Opaque game".to_string(),
        String::new(),
        capsule(requirement.clone()),
    )?;
    anyhow::ensure!(normalized.kind == NormalizedWorkKind::ForeignCapsule);
    anyhow::ensure!(normalized.package.is_none() && normalized.port_catalog.is_empty());
    anyhow::ensure!(matches!(
        normalized.work.entrypoints[0].target,
        WorkEntrypointTarget::ForeignLaunch { .. }
    ));

    let mut unsafe_capsule = capsule(requirement);
    unsafe_capsule.annotations.insert(
        "executable".to_string(),
        Value::String(r"C:\Games\opaque.exe".to_string()),
    );
    anyhow::ensure!(unsafe_capsule.validate().unwrap_err().code == DiagnosticCode::RawPath);
    Ok(())
}

async fn closed_foreign_protocol_port_uses_the_ordinary_binding_contract() -> anyhow::Result<()> {
    let transparency = TransparencyDeclaration {
        source_visibility: SourceVisibility::Closed,
        source_refs: Vec::new(),
        reproducible_build_claim: ClaimStatus::Unknown,
        sbom_refs: Vec::new(),
        provenance_refs: Vec::new(),
        signature_refs: Vec::new(),
        telemetry_disclosures: Vec::new(),
        state_portability: StatePortability::OpaqueExportable,
        evidence_refs: Vec::new(),
    };
    transparency.validate()?;

    let export = protocol_port(
        PortRole::Export {
            multiplicity: PortMultiplicity { min: 0, max: None },
            effect_class: EffectClass::RecordedNondeterministic,
        },
        "1.2.0",
    );
    let import = protocol_port(
        PortRole::Import {
            multiplicity: PortMultiplicity {
                min: 1,
                max: Some(1),
            },
            latest_binding_phase: BindingPhase::Runtime,
            availability: AvailabilityPolicy::Required,
            accepted_effects: vec![EffectClass::RecordedNondeterministic],
        },
        "^1.0",
    );
    let mut value = capsule(ForeignLaunchRequirement {
        launch_id: "play".to_string(),
        kind: ForeignLaunchKind::RemoteService,
        required_protocols: vec!["game.save".to_string()],
        annotations: BTreeMap::new(),
    });
    value.protocol_ports.push(export.clone());
    value.validate()?;
    anyhow::ensure!(check_port_compatibility(&export, &import).is_ok());
    Ok(())
}

async fn absent_denied_and_entitled_rights_are_conservative() -> anyhow::Result<()> {
    for operation in [
        RightsOperation::Backup,
        RightsOperation::ExportState,
        RightsOperation::CopyAcrossHosts,
        RightsOperation::RedistributeArtifacts,
    ] {
        anyhow::ensure!(rights_policy_outcome(None, operation) == RightsPolicyOutcome::Unspecified);
        anyhow::ensure!(
            rights_policy_outcome(Some(&rights(RightDisposition::Denied)), operation)
                == RightsPolicyOutcome::Denied
        );
        anyhow::ensure!(
            rights_policy_outcome(
                Some(&rights(RightDisposition::RequiresEntitlement)),
                operation,
            ) == RightsPolicyOutcome::RequiresEntitlement
        );
    }
    anyhow::ensure!(
        rights_policy_outcome(None, RightsOperation::Install) == RightsPolicyOutcome::Allowed
            && rights_policy_outcome(None, RightsOperation::Execute)
                == RightsPolicyOutcome::Allowed
    );
    Ok(())
}

async fn entitlement_is_an_ordinary_redacted_adapter_without_drm_api() -> anyhow::Result<()> {
    let credential = "credential-must-never-leak";
    let binding = ForeignLaunchBinding {
        schema: FOREIGN_LAUNCH_BINDING_SCHEMA.to_string(),
        launch_id: "play".to_string(),
        target: ForeignLaunchTarget::RemoteService {
            endpoint: "https://service.example.invalid/play".to_string(),
        },
        entitlement: Some(ForeignEntitlementAdapter {
            package_id: PackageId::from("adapter/entitlement"),
            capability_id: "store.entitlement/check".to_string(),
            version: Some("1.0.0".to_string()),
            input: json!({"credential": credential}),
        }),
    };
    binding.validate()?;
    let debug = format!("{binding:?}");
    anyhow::ensure!(!debug.contains(credential) && debug.contains("configured"));
    anyhow::ensure!(foreign_launch_secret_ref("play").starts_with("secret_ref:installation:"));
    anyhow::ensure!(PlatformMethod::all().iter().all(|method| {
        let id = method.id().to_ascii_lowercase();
        !id.contains("drm") && !id.contains("ownership")
    }));
    Ok(())
}

async fn dedicated_server_and_backup_are_public_but_coordinates_are_not() -> anyhow::Result<()> {
    let requirement = ForeignLaunchRequirement {
        launch_id: "server".to_string(),
        kind: ForeignLaunchKind::OciImage,
        required_protocols: Vec::new(),
        annotations: BTreeMap::from([(
            FOREIGN_DEDICATED_SERVER_ANNOTATION.to_string(),
            Value::Bool(true),
        )]),
    };
    let normalized = normalize_foreign_capsule(
        WorkId::parse("vendor/opaque-game")?,
        "Opaque server".to_string(),
        String::new(),
        capsule(requirement),
    )?;
    anyhow::ensure!(
        normalized.work.entrypoints[0].intent_uri == FOREIGN_DEDICATED_SERVER_INTENT_URI
    );

    let exported = method_schemas();
    let installation_wire = [
        PlatformMethod::InstallationGet,
        PlatformMethod::InstallationList,
        PlatformMethod::InstallationUpdate,
        PlatformMethod::AssetGet,
    ]
    .into_iter()
    .map(|method| {
        let (_, params, result) = exported
            .iter()
            .find(|(candidate, _, _)| *candidate == method)
            .expect("Installation method schema");
        serde_json::to_string(&method_schema(method, params.clone(), result.clone()))
            .expect("schema JSON")
    })
    .collect::<String>();
    for field in [
        "backup",
        "rights_declaration",
        "transparency_declaration",
        "installation_state_artifact",
    ] {
        anyhow::ensure!(
            installation_wire.contains(field),
            "public schema omitted {field}"
        );
    }
    for private in [
        "foreign_launch_binding",
        "secret_value",
        "working_directory",
        "raw_credential",
    ] {
        anyhow::ensure!(
            !installation_wire.contains(private),
            "public schema leaked {private}"
        );
    }
    anyhow::ensure!(FOREIGN_CAPSULE_TYPE_URI.starts_with("urn:plurora:"));
    Ok(())
}

pub(crate) fn foreign_work_cases() -> Vec<super::runner::ConformanceCase> {
    macro_rules! case {
        ($id:expr, [$($tag:expr),*], $func:path) => {
            super::registry::case($id, &[$($tag),*], || Box::pin($func()))
        };
    }
    vec![
        case!(
            "foreign_work.capsule_without_protocols_and_no_portable_path",
            ["foreign_work", "portable", "negative"],
            open_without_protocols_is_still_a_capsule_and_paths_are_rejected
        ),
        case!(
            "foreign_work.closed_protocol_ordinary_binding_contract",
            ["foreign_work", "binding", "protocol"],
            closed_foreign_protocol_port_uses_the_ordinary_binding_contract
        ),
        case!(
            "foreign_work.rights_conservative_copy_export",
            ["foreign_work", "rights", "negative"],
            absent_denied_and_entitled_rights_are_conservative
        ),
        case!(
            "foreign_work.entitlement_adapter_redacted_no_drm",
            ["foreign_work", "entitlement", "privacy", "negative"],
            entitlement_is_an_ordinary_redacted_adapter_without_drm_api
        ),
        case!(
            "foreign_work.dedicated_backup_public_no_coordinates",
            ["foreign_work", "backup", "dedicated_server", "contract"],
            dedicated_server_and_backup_are_public_but_coordinates_are_not
        ),
    ]
}
