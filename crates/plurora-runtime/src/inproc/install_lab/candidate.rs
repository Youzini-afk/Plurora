use std::collections::BTreeMap;

use anyhow::Result;
use plurora_core::{
    canonical_json_bytes, world_bundle_sha256_digest, ArtifactDescriptor, ComponentTrustClass,
    ContractMode, PackageEntry,
};
use plurora_work::{
    normalize_foreign_capsule, normalize_package_manifest, project_package_manifest,
    resolve_assembly_at_phase, AcquisitionKind, AcquisitionRecord, ArtifactModel, AssemblyNode,
    AssemblyNodeSource, BindingPhase, CanonicalArtifactObject, ClaimStatus,
    ForeignCapsuleDescriptor, ForeignLaunchKind, ForeignLaunchRequirement, NodeEvidence,
    NodeEvidenceKey, NodeId, NormalizedWork, PackageProjection, ResolverInput, RightDisposition,
    RightsDeclaration, SourceVisibility, StateBindingKind, StateBindingRecord, StatePortability,
    TransparencyDeclaration, WorkId,
};
use serde::Serialize;
use serde_json::{json, Value};

use super::planner::ResolvedPackage;
use super::types::{SourceKind, WorkCandidate, WorkCandidateStatus};

const SOURCE_SNAPSHOT_TYPE_URI: &str = "urn:plurora:source-snapshot-evidence:v1";

pub(super) struct BuiltCandidate {
    pub(super) root_id: String,
    pub(super) work_candidate: WorkCandidate,
    pub(super) artifacts: Vec<CanonicalArtifactObject>,
    pub(super) acquisition: AcquisitionRecord,
    pub(super) state_bindings: Vec<StateBindingRecord>,
    pub(super) diagnostics: Vec<Value>,
}

pub(super) fn build_package_candidate(packages: &[ResolvedPackage]) -> Result<BuiltCandidate> {
    let root = packages
        .first()
        .ok_or_else(|| anyhow::anyhow!("package candidate requires a root package"))?;
    let projections = packages
        .iter()
        .map(|package| {
            project_package_manifest(&package.manifest, Vec::new()).map_err(anyhow::Error::from)
        })
        .collect::<Result<Vec<_>>>()?;
    let root_projection = projections
        .first()
        .ok_or_else(|| anyhow::anyhow!("root package projection is absent"))?;
    let explicit_foreign = root.manifest.entry.contract == ContractMode::None
        || matches!(&root.manifest.entry.kind, PackageEntry::Remote { .. });
    if explicit_foreign {
        return build_foreign_package_candidate(root, root_projection, &projections);
    }

    let normalized = normalize_package_manifest(&root.manifest, Vec::new())?;
    let (normalized, node_projections, dependency_diagnostics) =
        extend_package_assembly(normalized, packages, &projections)?;
    let resolved = resolve_normalized(&normalized, &node_projections)?;
    let mut artifacts = collect_projection_artifacts(&projections);
    artifacts.extend(normalized.artifacts.clone());
    artifacts.extend(resolved.lock_artifacts.clone());
    deduplicate_objects(&mut artifacts)?;
    let mut diagnostics = dependency_diagnostics;
    diagnostics.extend(resolver_diagnostics(&resolved)?);
    let work_descriptor = descriptor_for_model(&normalized.work)?;
    let lock_descriptor = resolved.root_lock_artifact.descriptor.clone();
    let closure = closure_descriptors(&artifacts);
    let state_bindings = state_bindings(&resolved);
    let work_candidate = WorkCandidate {
        source_kind: SourceKind::Package,
        status: WorkCandidateStatus::Installable,
        display_name: normalized.work.title.clone(),
        work_revision: Some(work_descriptor),
        assembly_lock: Some(lock_descriptor),
        closure,
        diagnostics: diagnostics.clone(),
    };
    Ok(BuiltCandidate {
        root_id: root.manifest.id.clone(),
        work_candidate,
        artifacts,
        acquisition: AcquisitionRecord {
            kind: AcquisitionKind::Package,
            source_ref: Some(root_projection.envelope.artifact.clone()),
            provenance_refs: Vec::new(),
            update_channel: None,
        },
        state_bindings,
        diagnostics,
    })
}

fn build_foreign_package_candidate(
    root: &ResolvedPackage,
    root_projection: &PackageProjection,
    projections: &[PackageProjection],
) -> Result<BuiltCandidate> {
    let (rights, transparency, declaration_objects) = conservative_foreign_declarations()?;
    let work_id = WorkId::parse(root.manifest.id.clone())?;
    let title = root
        .manifest
        .display_name
        .clone()
        .unwrap_or_else(|| root.manifest.id.clone());
    let launch_kind = if matches!(&root.manifest.entry.kind, PackageEntry::Remote { .. }) {
        ForeignLaunchKind::RemoteService
    } else {
        ForeignLaunchKind::ManagedArtifact
    };
    let normalized = normalize_foreign_capsule(
        work_id,
        title,
        root.manifest.description.clone().unwrap_or_default(),
        ForeignCapsuleDescriptor {
            capsule_id: root.manifest.id.clone(),
            launch_requirements: vec![ForeignLaunchRequirement {
                launch_id: "run".to_string(),
                kind: launch_kind,
                required_protocols: Vec::new(),
                annotations: BTreeMap::new(),
            }],
            protocol_ports: Vec::new(),
            state_slots: Vec::new(),
            rights,
            transparency,
            annotations: BTreeMap::new(),
        },
    )?;
    let resolved = resolve_normalized(&normalized, &[])?;
    let mut artifacts = collect_projection_artifacts(projections);
    artifacts.extend(declaration_objects);
    artifacts.extend(normalized.artifacts.clone());
    artifacts.extend(resolved.lock_artifacts.clone());
    deduplicate_objects(&mut artifacts)?;
    finish_foreign_candidate(
        root.manifest.id.clone(),
        normalized,
        resolved,
        artifacts,
        AcquisitionRecord {
            kind: AcquisitionKind::Package,
            source_ref: Some(root_projection.envelope.artifact.clone()),
            provenance_refs: Vec::new(),
            update_channel: None,
        },
    )
}

pub(super) fn build_foreign_candidate(
    source_digest: &str,
    display_name: String,
) -> Result<BuiltCandidate> {
    let digest = source_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow::anyhow!("foreign source digest is not SHA-256"))?;
    anyhow::ensure!(
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "foreign source digest is not SHA-256"
    );
    let root_id = format!("foreign/{digest}");
    let evidence = source_snapshot_object(source_digest)?;
    let (rights, transparency, declaration_objects) = conservative_foreign_declarations()?;
    let normalized = normalize_foreign_capsule(
        WorkId::parse(root_id.clone())?,
        if display_name.trim().is_empty() {
            "Foreign source".to_string()
        } else {
            display_name
        },
        "Foreign source capsule requiring an explicit Host-local binding.".to_string(),
        ForeignCapsuleDescriptor {
            capsule_id: root_id.clone(),
            launch_requirements: vec![ForeignLaunchRequirement {
                launch_id: "run".to_string(),
                kind: ForeignLaunchKind::ManagedArtifact,
                required_protocols: Vec::new(),
                annotations: BTreeMap::new(),
            }],
            protocol_ports: Vec::new(),
            state_slots: Vec::new(),
            rights,
            transparency,
            annotations: BTreeMap::new(),
        },
    )?;
    let resolved = resolve_normalized(&normalized, &[])?;
    let mut artifacts = vec![evidence.clone()];
    artifacts.extend(declaration_objects);
    artifacts.extend(normalized.artifacts.clone());
    artifacts.extend(resolved.lock_artifacts.clone());
    deduplicate_objects(&mut artifacts)?;
    finish_foreign_candidate(
        root_id,
        normalized,
        resolved,
        artifacts,
        AcquisitionRecord {
            kind: AcquisitionKind::LocalImport,
            source_ref: Some(evidence.descriptor),
            provenance_refs: Vec::new(),
            update_channel: None,
        },
    )
}

fn finish_foreign_candidate(
    root_id: String,
    normalized: NormalizedWork,
    resolved: plurora_work::ResolverOutput,
    artifacts: Vec<CanonicalArtifactObject>,
    acquisition: AcquisitionRecord,
) -> Result<BuiltCandidate> {
    let mut diagnostics = resolver_diagnostics(&resolved)?;
    diagnostics.push(json!({
        "code": "foreign_binding_required",
        "message": "Foreign Capsule is not runnable until the Host records an explicit local binding",
    }));
    let work_descriptor = descriptor_for_model(&normalized.work)?;
    let lock_descriptor = resolved.root_lock_artifact.descriptor.clone();
    let state_bindings = state_bindings(&resolved);
    let work_candidate = WorkCandidate {
        source_kind: SourceKind::Foreign,
        status: WorkCandidateStatus::ForeignBindingRequired,
        display_name: normalized.work.title.clone(),
        work_revision: Some(work_descriptor),
        assembly_lock: Some(lock_descriptor),
        closure: closure_descriptors(&artifacts),
        diagnostics: diagnostics.clone(),
    };
    Ok(BuiltCandidate {
        root_id,
        work_candidate,
        artifacts,
        acquisition,
        state_bindings,
        diagnostics,
    })
}

fn extend_package_assembly(
    mut normalized: NormalizedWork,
    packages: &[ResolvedPackage],
    projections: &[PackageProjection],
) -> Result<(NormalizedWork, Vec<(NodeId, PackageProjection)>, Vec<Value>)> {
    let root_projection = projections
        .first()
        .ok_or_else(|| anyhow::anyhow!("root package projection is absent"))?
        .clone();
    let mut node_projections = vec![(NodeId::parse("main")?, root_projection)];
    let mut diagnostics = Vec::new();
    for (index, (package, projection)) in packages.iter().zip(projections).enumerate().skip(1) {
        let explicit_foreign = package.manifest.entry.contract == ContractMode::None
            || matches!(&package.manifest.entry.kind, PackageEntry::Remote { .. })
            || projection.component.trust_class == ComponentTrustClass::ForeignCapsule;
        if explicit_foreign {
            diagnostics.push(json!({
                "code": "foreign_dependency_requires_binding",
                "package_id": package.manifest.id,
                "message": "Package dependency is a Foreign Capsule and was not silently componentized",
            }));
            continue;
        }
        let node_id = NodeId::parse(format!("dependency-{index}"))?;
        normalized.assembly.nodes.push(AssemblyNode {
            node_id: node_id.clone(),
            source: AssemblyNodeSource::Component {
                component: projection.component.artifact.clone(),
            },
            ports: projection.ports.clone(),
            configuration: None,
            annotations: BTreeMap::from([(
                "plurora.normalization/component_id".to_string(),
                Value::String(projection.component.component_id.clone()),
            )]),
        });
        normalized
            .port_catalog
            .extend(projection.port_catalog(&node_id));
        node_projections.push((node_id, projection.clone()));
    }
    normalized
        .assembly
        .nodes
        .sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let assembly_object = CanonicalArtifactObject::from_model(&normalized.assembly)?;
    normalized.work.assembly = assembly_object.descriptor.clone();
    let mut content_roots = projections
        .iter()
        .flat_map(|projection| projection.component.content_roots.clone())
        .collect::<Vec<_>>();
    sort_descriptors(&mut content_roots);
    normalized.work.content_roots = content_roots;
    let work_object = CanonicalArtifactObject::from_model(&normalized.work)?;
    normalized.artifacts.retain(|object| {
        object.descriptor.artifact_type_uri != plurora_work::ASSEMBLY_REVISION_TYPE_URI
            && object.descriptor.artifact_type_uri != plurora_work::WORK_REVISION_TYPE_URI
    });
    normalized.artifacts.extend([assembly_object, work_object]);
    deduplicate_objects(&mut normalized.artifacts)?;
    normalized.validate()?;
    Ok((normalized, node_projections, diagnostics))
}

fn resolve_normalized(
    normalized: &NormalizedWork,
    node_projections: &[(NodeId, PackageProjection)],
) -> Result<plurora_work::ResolverOutput> {
    let root_assembly = descriptor_for_model(&normalized.assembly)?;
    let node_evidence = node_projections
        .iter()
        .map(|(node_id, projection)| {
            (
                NodeEvidenceKey {
                    assembly_digest: root_assembly.digest.clone(),
                    node_id: node_id.clone(),
                },
                NodeEvidence {
                    component: projection.component.clone(),
                    ports: projection.ports.clone(),
                    provenance_refs: Vec::new(),
                    artifact_refs: projection
                        .artifacts
                        .iter()
                        .map(|object| object.descriptor.clone())
                        .collect(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let protocol_profiles = node_projections
        .iter()
        .flat_map(|(_, projection)| projection.protocol_profiles.clone())
        .collect::<Vec<_>>();
    Ok(resolve_assembly_at_phase(
        &ResolverInput {
            root_assembly: root_assembly.clone(),
            assemblies: BTreeMap::from([(
                root_assembly.digest.clone(),
                normalized.assembly.clone(),
            )]),
            node_evidence,
            content_roots: normalized.work.content_roots.clone(),
            protocol_profiles,
        },
        BindingPhase::Installation,
    )?)
}

fn conservative_foreign_declarations() -> Result<(
    ArtifactDescriptor,
    ArtifactDescriptor,
    Vec<CanonicalArtifactObject>,
)> {
    let unspecified = RightDisposition::Unspecified;
    let rights = RightsDeclaration {
        license_expression: None,
        terms_uri: None,
        install: unspecified,
        execute: unspecified,
        backup: unspecified,
        export_state: unspecified,
        copy_across_hosts: unspecified,
        redistribute_artifacts: unspecified,
        modify: unspecified,
        derive: unspecified,
        modding: unspecified,
        dedicated_server: unspecified,
        entitlement_requirements: Vec::new(),
        evidence_refs: Vec::new(),
    };
    let transparency = TransparencyDeclaration {
        source_visibility: SourceVisibility::Unknown,
        source_refs: Vec::new(),
        reproducible_build_claim: ClaimStatus::Unknown,
        sbom_refs: Vec::new(),
        provenance_refs: Vec::new(),
        signature_refs: Vec::new(),
        telemetry_disclosures: Vec::new(),
        state_portability: StatePortability::HostBound,
        evidence_refs: Vec::new(),
    };
    let rights = CanonicalArtifactObject::from_model(&rights)?;
    let transparency = CanonicalArtifactObject::from_model(&transparency)?;
    Ok((
        rights.descriptor.clone(),
        transparency.descriptor.clone(),
        vec![rights, transparency],
    ))
}

#[derive(Serialize)]
struct SourceSnapshotEvidence<'a> {
    schema: &'static str,
    source_digest: &'a str,
}

fn source_snapshot_object(source_digest: &str) -> Result<CanonicalArtifactObject> {
    let bytes = canonical_json_bytes(&SourceSnapshotEvidence {
        schema: "plurora.source-snapshot-evidence.v1",
        source_digest,
    })?;
    let descriptor = ArtifactDescriptor {
        artifact_type_uri: SOURCE_SNAPSHOT_TYPE_URI.to_string(),
        media_type: "application/json".to_string(),
        digest: world_bundle_sha256_digest(&bytes),
        size_bytes: bytes.len() as u64,
        references: Vec::new(),
        annotations: BTreeMap::new(),
    };
    Ok(CanonicalArtifactObject::new(descriptor, bytes)?)
}

fn collect_projection_artifacts(projections: &[PackageProjection]) -> Vec<CanonicalArtifactObject> {
    projections
        .iter()
        .flat_map(|projection| projection.artifacts.clone())
        .collect()
}

fn descriptor_for_model<T: ArtifactModel>(model: &T) -> Result<ArtifactDescriptor> {
    Ok(model.artifact_descriptor()?)
}

fn resolver_diagnostics(output: &plurora_work::ResolverOutput) -> Result<Vec<Value>> {
    let mut diagnostics = output
        .diagnostics
        .diagnostics
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    if output.diagnostics.omitted_count > 0 {
        diagnostics.push(json!({
            "code": "diagnostics_truncated",
            "omitted_count": output.diagnostics.omitted_count,
        }));
    }
    Ok(diagnostics)
}

fn state_bindings(output: &plurora_work::ResolverOutput) -> Vec<StateBindingRecord> {
    output
        .state_slots
        .iter()
        .map(|slot| StateBindingRecord {
            state_slot_id: slot.state_slot_id.clone(),
            kind: StateBindingKind::HostManaged,
            binding_id: format!("state-{}", slot.state_slot_id),
            provider_ref: None,
        })
        .collect()
}

fn closure_descriptors(objects: &[CanonicalArtifactObject]) -> Vec<ArtifactDescriptor> {
    let mut descriptors = objects
        .iter()
        .map(|object| object.descriptor.clone())
        .collect::<Vec<_>>();
    sort_descriptors(&mut descriptors);
    descriptors
}

fn sort_descriptors(descriptors: &mut Vec<ArtifactDescriptor>) {
    descriptors.sort_by(|left, right| left.digest.cmp(&right.digest));
    descriptors.dedup_by(|left, right| left.digest == right.digest);
}

fn deduplicate_objects(objects: &mut Vec<CanonicalArtifactObject>) -> Result<()> {
    objects.sort_by(|left, right| left.descriptor.digest.cmp(&right.descriptor.digest));
    let mut seen = BTreeMap::<String, (ArtifactDescriptor, Vec<u8>)>::new();
    for object in objects.iter() {
        CanonicalArtifactObject::new(object.descriptor.clone(), object.bytes.clone())?;
        if let Some((descriptor, bytes)) = seen.get(&object.descriptor.digest) {
            anyhow::ensure!(
                descriptor == &object.descriptor && bytes == &object.bytes,
                "canonical candidate closure contains inconsistent duplicate objects"
            );
        } else {
            seen.insert(
                object.descriptor.digest.clone(),
                (object.descriptor.clone(), object.bytes.clone()),
            );
        }
    }
    objects.dedup_by(|left, right| left.descriptor.digest == right.descriptor.digest);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FilesystemObjectStore, ObjectStore};
    use plurora_core::{
        EntryDescriptor, PackageContributions, PackageManifest, PermissionSet, RemoteAuth,
        SandboxPolicy,
    };
    use plurora_work::{AssemblyLock, ASSEMBLY_LOCK_TYPE_URI, WORK_REVISION_TYPE_URI};

    use crate::inproc::install_lab::types::{
        PlannedPackage, PlannedPackageSourceKind, PlannedPermissions,
    };

    fn package_manifest() -> PackageManifest {
        PackageManifest {
            schema_version: 1,
            id: "example/installable".to_string(),
            version: "1.0.0".to_string(),
            display_name: Some("Installable".to_string()),
            description: Some("Candidate fixture".to_string()),
            author: None,
            license: None,
            entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                crate_ref: "example-installable".to_string(),
                symbol: "register".to_string(),
                abi_version: 1,
            }),
            provides: Vec::new(),
            consumes: Vec::new(),
            requires: Vec::new(),
            contributes: PackageContributions::default(),
            permissions: PermissionSet::default(),
            sandbox_policy: SandboxPolicy::default(),
        }
    }

    fn resolved_package(manifest: PackageManifest) -> ResolvedPackage {
        ResolvedPackage {
            planned: PlannedPackage {
                id: manifest.id.clone(),
                version: manifest.version.clone(),
                source_kind: PlannedPackageSourceKind::Local,
                manifest_hash: format!("sha256:{}", "a".repeat(64)),
                tree_hash: format!("sha256:{}", "b".repeat(64)),
                commit_sha: None,
                package_envelope_digest: None,
                component_pins: Vec::new(),
                protocol_profile_pins: Vec::new(),
                content_roots: Vec::new(),
                signed: false,
                signed_by: None,
                permissions: PlannedPermissions::default(),
                requires: Vec::new(),
                conformance: None,
            },
            manifest,
        }
    }

    #[tokio::test]
    async fn package_candidate_preserves_component_identity_and_persists_complete_closure(
    ) -> Result<()> {
        let manifest = package_manifest();
        let projection = project_package_manifest(&manifest, Vec::new())?;
        let built = build_package_candidate(&[resolved_package(manifest)])?;

        assert_eq!(
            built.work_candidate.status,
            WorkCandidateStatus::Installable
        );
        assert_eq!(
            built
                .work_candidate
                .work_revision
                .as_ref()
                .unwrap()
                .artifact_type_uri,
            WORK_REVISION_TYPE_URI
        );
        assert_eq!(
            built
                .work_candidate
                .assembly_lock
                .as_ref()
                .unwrap()
                .artifact_type_uri,
            ASSEMBLY_LOCK_TYPE_URI
        );
        let lock_descriptor = built.work_candidate.assembly_lock.as_ref().unwrap();
        let lock_object = built
            .artifacts
            .iter()
            .find(|object| object.descriptor.digest == lock_descriptor.digest)
            .unwrap();
        let lock: AssemblyLock = serde_json::from_slice(&lock_object.bytes)?;
        assert_eq!(lock.nodes.len(), 1);
        assert_eq!(lock.nodes[0].artifact, projection.component.artifact);

        let temporary = tempfile::tempdir()?;
        let data_dir = temporary.path().to_string_lossy();
        super::super::executor::persist_candidate_objects(Some(data_dir.as_ref()), &built).await?;
        let store = FilesystemObjectStore::new(temporary.path().join("objects"));
        for object in &built.artifacts {
            let verified = store.verify(&object.descriptor.digest).await?;
            assert_eq!(verified.size_bytes, object.descriptor.size_bytes);
        }
        for retired in ["store", "profiles", "projects", "installations", "runs"] {
            assert!(!temporary.path().join(retired).exists(), "{retired}");
        }
        Ok(())
    }

    #[test]
    fn contract_none_and_remote_packages_require_foreign_binding() -> Result<()> {
        let mut contract_none = package_manifest();
        contract_none.entry = EntryDescriptor::contract_none(PackageEntry::RustInproc {
            crate_ref: "example-installable".to_string(),
            symbol: "register".to_string(),
            abi_version: 1,
        });
        let candidate = build_package_candidate(&[resolved_package(contract_none)])?;
        assert_eq!(
            candidate.work_candidate.status,
            WorkCandidateStatus::ForeignBindingRequired
        );
        assert_eq!(candidate.work_candidate.source_kind, SourceKind::Foreign);

        let mut remote = package_manifest();
        remote.entry = EntryDescriptor::v1(PackageEntry::Remote {
            endpoint: "https://service.example/rpc".to_string(),
            auth: RemoteAuth {
                scheme: "none".to_string(),
                config: Value::Null,
            },
        });
        let candidate = build_package_candidate(&[resolved_package(remote)])?;
        assert_eq!(
            candidate.work_candidate.status,
            WorkCandidateStatus::ForeignBindingRequired
        );
        assert_eq!(candidate.work_candidate.source_kind, SourceKind::Foreign);
        Ok(())
    }
}
