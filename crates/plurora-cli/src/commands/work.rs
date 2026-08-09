use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::{
    ambient_authority,
    fs::{Dir as CapabilityDir, OpenOptions as CapabilityOpenOptions},
};
use clap::{Args, Subcommand};
use plurora_core::{
    world_bundle_sha256_digest, ArtifactDescriptor, ComponentTrustClass, ContractMode,
    PackageEntry, PackageManifest,
};
use plurora_work::{
    parse_assembly_source, parse_work_source, project_package_manifest, resolve_assembly,
    ArtifactModel, AssemblyBinding, AssemblyNode, AssemblyNodeSource, AssemblyPortExposure,
    AssemblyRevision, BindingPhase, CanonicalArtifactObject, DiagnosticCode, DiagnosticReport,
    ModelError, ModelResult, NodeEvidence, NodeEvidenceKey, OperationalIntent, PortEndpoint,
    ResolverInput, ResolverOutput, RightsDeclaration, SourcePathRef, StateSlotDescriptor,
    TransparencyDeclaration, WorkDiagnostic, WorkEntrypoint, WorkEntrypointTarget, WorkRevision,
    MAX_SOURCE_DESCRIPTOR_BYTES,
};
use same_file::Handle;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

const CONTENT_BLOB_TYPE_URI: &str = "urn:plurora:content-blob:v1";
const CONTENT_DIRECTORY_TYPE_URI: &str = "urn:plurora:content-directory:v1";
const CONFIGURATION_TYPE_URI: &str = "urn:plurora:assembly-configuration:v1";
const STATE_SCHEMA_TYPE_URI: &str = "urn:plurora:state-schema:v1";

const INIT_WORK: &str = r#"schema: plurora.work-source.v1
work:
  id: {id}
  title: New Work
  assembly: assembly.yaml
"#;

const INIT_ASSEMBLY: &str = r#"schema: plurora.assembly-source.v1
assembly:
  id: {id}/main
  nodes: []
"#;

#[derive(Debug, Args)]
pub struct WorkArgs {
    #[command(subcommand)]
    pub command: WorkCommand,
}

#[derive(Debug, Subcommand)]
pub enum WorkCommand {
    /// Create a minimal, portable Work source tree.
    Init {
        path: PathBuf,
        #[arg(long, default_value = "example/work")]
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Materialize and validate a Work without writing runtime state.
    Check {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Materialize a Work and import its canonical authoring closure.
    Pack {
        path: PathBuf,
        #[arg(long)]
        data_dir: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Inspect a Work's canonical authoring graph without importing it.
    Inspect {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Serialize)]
struct InitReport {
    operation: &'static str,
    ok: bool,
    created: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub(crate) struct WorkReport {
    operation: &'static str,
    ok: bool,
    resolution_phase: BindingPhase,
    complete: bool,
    portable: bool,
    persisted: bool,
    pub(crate) work: ArtifactDescriptor,
    root_assembly: ArtifactDescriptor,
    root_lock: ArtifactDescriptor,
    closure: Vec<ArtifactDescriptor>,
    pub(crate) nodes: Vec<plurora_work::FlattenedNode>,
    pub(crate) exposed_ports: Vec<plurora_work::ResolvedExposedPort>,
    bindings: Vec<plurora_work::ResolvedBinding>,
    state_slots: Vec<plurora_work::ResolvedStateSlot>,
    diagnostics: DiagnosticReport,
}

#[derive(Debug, Serialize)]
struct ErrorReport {
    operation: &'static str,
    ok: bool,
    diagnostics: Vec<WorkDiagnostic>,
}

pub(crate) async fn run(args: WorkArgs) -> anyhow::Result<()> {
    match args.command {
        WorkCommand::Init { path, id, json } => run_init(path, id, json),
        WorkCommand::Check { path, json } => run_materialize("check", path, None, json).await,
        WorkCommand::Pack {
            path,
            data_dir,
            json,
        } => run_materialize("pack", path, data_dir, json).await,
        WorkCommand::Inspect { path, json } => run_materialize("inspect", path, None, json).await,
    }
}

fn run_init(path: PathBuf, id: String, json: bool) -> anyhow::Result<()> {
    let result = init_work(&path, &id);
    match result {
        Ok(created) => {
            let report = InitReport {
                operation: "init",
                ok: true,
                created,
            };
            print_report(&report, json, || {
                println!("Work source ready: work.yaml, assembly.yaml");
            })?;
            Ok(())
        }
        Err(error) => emit_error("init", &error, json),
    }
}

async fn run_materialize(
    operation: &'static str,
    path: PathBuf,
    data_dir: Option<PathBuf>,
    json: bool,
) -> anyhow::Result<()> {
    let result = materialize_work(&path);
    let mut materialized = match result {
        Ok(value) => value,
        Err(error) => return emit_error(operation, &error, json),
    };
    if !materialized.resolution.portable {
        let report = materialized.report(operation, false);
        print_report(&report, json, || print_human_report(&report))?;
        return Err(anyhow::anyhow!("port_unresolved"));
    }
    let persisted = operation == "pack";
    if persisted {
        let root = data_dir
            .map(|path| Ok(path.join("objects")))
            .unwrap_or_else(|| plurora_core::paths::data_dir().map(|path| path.join("objects")))
            .map_err(|_| anyhow::anyhow!("object_store_unavailable"))?;
        if let Err(error) = persist_objects(&root, materialized.objects.values()).await {
            return emit_error(operation, &error, json);
        }
    }
    let report = materialized.report(operation, persisted);
    print_report(&report, json, || print_human_report(&report))?;
    Ok(())
}

fn print_report<T: Serialize>(report: &T, json: bool, human: impl FnOnce()) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        human();
    }
    Ok(())
}

fn print_human_report(report: &WorkReport) {
    println!("Work: {}", report.work.digest);
    println!("Root Assembly: {}", report.root_assembly.digest);
    println!("Authoring lock: {}", report.root_lock.digest);
    println!("Closure: {} canonical objects", report.closure.len());
    println!(
        "Authoring resolution: portable={}, complete={}, persisted={}",
        report.portable, report.complete, report.persisted
    );
    for diagnostic in &report.diagnostics.diagnostics {
        println!(
            "{} [{}]: {}",
            diagnostic.severity, diagnostic.code, diagnostic.message
        );
    }
}

fn emit_error(operation: &'static str, error: &ModelError, json: bool) -> anyhow::Result<()> {
    let report = ErrorReport {
        operation,
        ok: false,
        diagnostics: vec![error.diagnostic()],
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        eprintln!("{}: {}", error.code, error.message);
    }
    Err(anyhow::anyhow!("{}", error.code.as_str()))
}

fn init_work(path: &Path, id: &str) -> ModelResult<Vec<&'static str>> {
    plurora_work::WorkId::parse(id.to_string())?;
    let path = absolute_path(path)?;
    ensure_real_directory(&path, true)?;
    let canonical_path = fs::canonicalize(&path).map_err(|_| io_error())?;
    let directory = CapabilityDir::open_ambient_dir(&canonical_path, ambient_authority())
        .map_err(|_| io_error())?;
    verify_capability_directory(&directory, &canonical_path)?;
    let work = INIT_WORK.replace("{id}", id);
    let assembly = INIT_ASSEMBLY.replace("{id}", id);
    preflight_idempotent(&directory, "work.yaml", work.as_bytes())?;
    preflight_idempotent(&directory, "assembly.yaml", assembly.as_bytes())?;
    let mut created = Vec::new();
    write_idempotent(
        &directory,
        "work.yaml",
        work.as_bytes(),
        "work.yaml",
        &mut created,
    )?;
    write_idempotent(
        &directory,
        "assembly.yaml",
        assembly.as_bytes(),
        "assembly.yaml",
        &mut created,
    )?;
    sync_capability_directory(&directory)?;
    verify_capability_directory(&directory, &canonical_path)?;
    materialize_work(&path)?;
    Ok(created)
}

fn preflight_idempotent(directory: &CapabilityDir, name: &str, expected: &[u8]) -> ModelResult<()> {
    match directory.symlink_metadata(name) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || read_capability_file(directory, name)? != expected
            {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "initialization would overwrite a different user file",
                ));
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(io_error()),
    }
}

fn write_idempotent(
    directory: &CapabilityDir,
    name: &str,
    expected: &[u8],
    label: &'static str,
    created: &mut Vec<&'static str>,
) -> ModelResult<()> {
    let mut options = CapabilityOpenOptions::new();
    options.write(true).create_new(true);
    match directory.open_with(name, &options) {
        Ok(mut file) => {
            file.write_all(expected).map_err(|_| io_error())?;
            file.sync_all().map_err(|_| io_error())?;
            created.push(label);
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let actual = read_capability_file(directory, name)?;
            if actual == expected {
                Ok(())
            } else {
                Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "initialization would overwrite a different user file",
                ))
            }
        }
        Err(_) => Err(io_error()),
    }
}

fn read_capability_file(directory: &CapabilityDir, name: &str) -> ModelResult<Vec<u8>> {
    let metadata = directory.symlink_metadata(name).map_err(|_| io_error())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(raw_path());
    }
    let mut file = directory.open(name).map_err(|_| io_error())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|_| io_error())?;
    Ok(bytes)
}

#[derive(Debug)]
struct MaterializedWork {
    work_object: CanonicalArtifactObject,
    root_assembly: ArtifactDescriptor,
    resolution: ResolverOutput,
    objects: BTreeMap<String, MaterializedObject>,
}

#[derive(Debug)]
enum MaterializedObject {
    Memory(CanonicalArtifactObject),
    OpenFile {
        descriptor: ArtifactDescriptor,
        file: File,
    },
}

impl MaterializedObject {
    fn descriptor(&self) -> &ArtifactDescriptor {
        match self {
            Self::Memory(object) => &object.descriptor,
            Self::OpenFile { descriptor, .. } => descriptor,
        }
    }
}

impl MaterializedWork {
    fn report(&mut self, operation: &'static str, persisted: bool) -> WorkReport {
        let mut closure = self
            .objects
            .values()
            .map(|object| object.descriptor().clone())
            .collect::<Vec<_>>();
        closure.sort_by(|left, right| left.digest.cmp(&right.digest));
        WorkReport {
            operation,
            ok: self.resolution.portable,
            resolution_phase: BindingPhase::Authoring,
            complete: self.resolution.complete,
            portable: self.resolution.portable,
            persisted,
            work: self.work_object.descriptor.clone(),
            root_assembly: self.root_assembly.clone(),
            root_lock: self.resolution.root_lock_artifact.descriptor.clone(),
            closure,
            nodes: self.resolution.flattened_nodes.clone(),
            exposed_ports: self.resolution.exposed_ports.clone(),
            bindings: self.resolution.bindings.clone(),
            state_slots: self.resolution.state_slots.clone(),
            diagnostics: self.resolution.diagnostics.clone(),
        }
    }
}

struct MaterializeContext {
    root: PathBuf,
    root_handle: Handle,
    assemblies: BTreeMap<String, AssemblyRevision>,
    node_evidence: BTreeMap<NodeEvidenceKey, NodeEvidence>,
    objects: BTreeMap<String, MaterializedObject>,
    assembly_cache: BTreeMap<PathBuf, ArtifactDescriptor>,
    assembly_stack: BTreeSet<PathBuf>,
}

pub(crate) fn check_work_path(path: &Path) -> ModelResult<WorkReport> {
    let mut materialized = materialize_work(path)?;
    if !materialized.resolution.portable {
        return Err(ModelError::new(
            DiagnosticCode::PortUnresolved,
            "Work has unresolved authoring-time requirements",
        ));
    }
    Ok(materialized.report("check", false))
}

fn materialize_work(input: &Path) -> ModelResult<MaterializedWork> {
    let work_path = normalize_work_path(input)?;
    let root = work_path
        .parent()
        .ok_or_else(|| ModelError::new(DiagnosticCode::RawPath, "Work source root is invalid"))?;
    ensure_real_directory(root, false)?;
    let root_metadata = fs::symlink_metadata(root).map_err(|_| io_error())?;
    reject_link_like(&root_metadata)?;
    if !root_metadata.is_dir() {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "Work source root is not a directory",
        ));
    }
    let canonical_root = fs::canonicalize(root).map_err(|_| io_error())?;
    let root_handle = Handle::from_path(&canonical_root).map_err(|_| io_error())?;
    let work_bytes = safe_read_contained(
        &canonical_root,
        &canonical_root,
        Path::new("work.yaml"),
        true,
    )?;
    let source = parse_work_source(&work_bytes)?;
    let mut context = MaterializeContext {
        root: canonical_root,
        root_handle,
        assemblies: BTreeMap::new(),
        node_evidence: BTreeMap::new(),
        objects: BTreeMap::new(),
        assembly_cache: BTreeMap::new(),
        assembly_stack: BTreeSet::new(),
    };
    let source_root = context.root.clone();
    let root_assembly =
        materialize_assembly(&mut context, source_root.clone(), &source.work.assembly)?;

    let mut content_roots = Vec::new();
    for source_ref in &source.work.content {
        content_roots.push(materialize_content(&mut context, &source_root, source_ref)?);
    }
    content_roots.sort_by(|left, right| left.digest.cmp(&right.digest));
    content_roots.dedup_by(|left, right| left.digest == right.digest);

    let rights = source
        .work
        .rights
        .as_ref()
        .map(|reference| {
            materialize_typed::<RightsDeclaration>(&mut context, &source_root, reference)
        })
        .transpose()?;
    let transparency = source
        .work
        .transparency
        .as_ref()
        .map(|reference| {
            materialize_typed::<TransparencyDeclaration>(&mut context, &source_root, reference)
        })
        .transpose()?;
    let operational_intent = source
        .work
        .operational_intent
        .as_ref()
        .map(|reference| {
            materialize_typed::<OperationalIntent>(&mut context, &source_root, reference)
        })
        .transpose()?;

    let input = ResolverInput {
        root_assembly: root_assembly.clone(),
        assemblies: context.assemblies.clone(),
        node_evidence: context.node_evidence.clone(),
        content_roots: content_roots.clone(),
        protocol_profiles: Vec::new(),
    };
    let resolution = resolve_assembly(&input)?;
    for object in &resolution.lock_artifacts {
        insert_object(&mut context.objects, object.clone())?;
    }
    insert_object(&mut context.objects, resolution.root_lock_artifact.clone())?;

    let exposed_ids = resolution
        .exposed_ports
        .iter()
        .map(|port| &port.port_id)
        .collect::<BTreeSet<_>>();
    let entrypoints = source
        .work
        .entrypoints
        .iter()
        .map(|entrypoint| {
            let target = if let Some(port_id) = &entrypoint.port_id {
                if !exposed_ids.contains(port_id) {
                    return Err(ModelError::new(
                        DiagnosticCode::PortUnresolved,
                        "Work entrypoint references an unexposed Assembly Port",
                    ));
                }
                WorkEntrypointTarget::AssemblyPort {
                    port_id: port_id.clone(),
                }
            } else if let Some(surface_id) = &entrypoint.surface_id {
                WorkEntrypointTarget::Surface {
                    surface_id: surface_id.clone(),
                }
            } else {
                WorkEntrypointTarget::ForeignLaunch {
                    launch_id: entrypoint
                        .foreign_launch_id
                        .clone()
                        .expect("validated source target"),
                }
            };
            Ok(WorkEntrypoint {
                id: entrypoint.id.clone(),
                intent_uri: entrypoint.intent_uri.clone(),
                target,
                annotations: entrypoint.annotations.clone(),
            })
        })
        .collect::<ModelResult<Vec<_>>>()?;
    let work = WorkRevision {
        schema: WorkRevision::SCHEMA.to_string(),
        work_id: source.work.id,
        title: source.work.title,
        description: source.work.description,
        assembly: root_assembly.clone(),
        content_roots,
        entrypoints,
        rights,
        transparency,
        operational_intent,
        annotations: source.work.annotations,
    };
    let work_object = CanonicalArtifactObject::from_model(&work)?;
    insert_object(&mut context.objects, work_object.clone())?;
    verify_object_closure(&context.objects)?;
    let final_root = Handle::from_path(&context.root).map_err(|_| io_error())?;
    if final_root != context.root_handle {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "Work source root changed while it was being read",
        ));
    }
    Ok(MaterializedWork {
        work_object,
        root_assembly,
        resolution,
        objects: context.objects,
    })
}

fn normalize_work_path(input: &Path) -> ModelResult<PathBuf> {
    let input = absolute_path(input)?;
    let metadata = fs::symlink_metadata(&input).map_err(|_| io_error())?;
    reject_link_like(&metadata)?;
    if metadata.is_dir() {
        Ok(input.join("work.yaml"))
    } else if metadata.is_file()
        && input.file_name().and_then(|value| value.to_str()) == Some("work.yaml")
    {
        Ok(input)
    } else {
        Err(ModelError::new(
            DiagnosticCode::RawPath,
            "Work input must be a source directory or an explicit work.yaml",
        ))
    }
}

fn absolute_path(path: &Path) -> ModelResult<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir().map_err(|_| io_error())?.join(path))
    }
}

fn ensure_real_directory(path: &Path, create_missing: bool) -> ModelResult<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        if parent != path {
            ensure_real_directory(parent, create_missing)?;
        }
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            reject_link_like(&metadata)?;
            if !metadata.is_dir() {
                return Err(ModelError::new(
                    DiagnosticCode::RawPath,
                    "Expected a real directory",
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && create_missing => {
            match fs::create_dir(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(io_error()),
            }
            let metadata = fs::symlink_metadata(path).map_err(|_| io_error())?;
            reject_link_like(&metadata)?;
            if !metadata.is_dir() {
                return Err(ModelError::new(
                    DiagnosticCode::RawPath,
                    "Created path is not a real directory",
                ));
            }
        }
        Err(_) => return Err(io_error()),
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        let canonical_parent = fs::canonicalize(parent).map_err(|_| io_error())?;
        let canonical = fs::canonicalize(path).map_err(|_| io_error())?;
        if !canonical.starts_with(&canonical_parent) {
            return Err(raw_path());
        }
    }
    Ok(())
}

fn materialize_assembly(
    context: &mut MaterializeContext,
    base: PathBuf,
    reference: &SourcePathRef,
) -> ModelResult<ArtifactDescriptor> {
    require_no_fragment(reference)?;
    let path = resolve_contained(&context.root, &base, Path::new(reference.path()))?;
    if let Some(cached) = context.assembly_cache.get(&path) {
        return Ok(cached.clone());
    }
    if !context.assembly_stack.insert(path.clone()) {
        return Err(ModelError::new(
            DiagnosticCode::AssemblyCycle,
            "Assembly source containment graph contains a cycle",
        ));
    }
    let bytes = safe_read_file(&context.root, &path, Some(MAX_SOURCE_DESCRIPTOR_BYTES))?;
    let source = parse_assembly_source(&bytes)?;
    let source_base = path.parent().unwrap_or(&context.root).to_path_buf();
    let mut nodes = Vec::new();
    let mut pending_evidence = Vec::new();
    for node in source.assembly.nodes {
        let configuration = node
            .configuration
            .as_ref()
            .map(|reference| {
                materialize_value(context, &source_base, reference, CONFIGURATION_TYPE_URI)
            })
            .transpose()?;
        let mut canonical_ports = Vec::new();
        let source = if let Some(component) = node.component {
            let bytes = safe_read_source_ref(context, &source_base, &component, true)?;
            let manifest: PackageManifest = serde_yaml::from_slice(&bytes).map_err(|_| {
                ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "Package Manifest source is malformed",
                )
            })?;
            if manifest.entry.contract == ContractMode::None
                || matches!(&manifest.entry.kind, PackageEntry::Remote { .. })
            {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "foreign Package source requires explicit Foreign Capsule normalization",
                ));
            }
            let projection = project_package_manifest(&manifest, node.ports)?;
            if projection.component.trust_class == ComponentTrustClass::ForeignCapsule {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "foreign Package source requires explicit Foreign Capsule normalization",
                ));
            }
            validate_component_fragment(&component, &projection.component.component_id)?;
            canonical_ports = projection.ports.clone();
            for object in projection.artifacts.clone() {
                insert_object(&mut context.objects, object)?;
            }
            pending_evidence.push((
                node.id.clone(),
                NodeEvidence {
                    component: projection.component.clone(),
                    ports: projection.ports,
                    provenance_refs: vec![projection.envelope.artifact],
                    artifact_refs: projection
                        .artifacts
                        .iter()
                        .map(|object| object.descriptor.clone())
                        .collect(),
                },
            ));
            AssemblyNodeSource::Component {
                component: projection.component_artifact,
            }
        } else {
            let assembly = node.assembly.expect("validated node source");
            AssemblyNodeSource::Assembly {
                assembly: materialize_assembly(context, source_base.clone(), &assembly)?,
            }
        };
        nodes.push(AssemblyNode {
            node_id: node.id,
            source,
            ports: canonical_ports,
            configuration,
            annotations: node.annotations,
        });
    }
    let bindings = source
        .assembly
        .bindings
        .into_iter()
        .map(|binding| AssemblyBinding {
            binding_id: binding.id,
            provider: PortEndpoint {
                node_id: binding.from.node_id,
                port_id: binding.from.port_id,
            },
            consumer: PortEndpoint {
                node_id: binding.to.node_id,
                port_id: binding.to.port_id,
            },
            phase: binding.phase,
            transport_policy: binding.transport,
            annotations: binding.annotations,
        })
        .collect();
    let exposed_ports = source
        .assembly
        .exposed_ports
        .into_iter()
        .map(|exposure| AssemblyPortExposure {
            port_id: exposure.id,
            direction: exposure.direction,
            target: PortEndpoint {
                node_id: exposure.target.node_id,
                port_id: exposure.target.port_id,
            },
            annotations: exposure.annotations,
        })
        .collect();
    let state_slots = source
        .assembly
        .state_slots
        .into_iter()
        .map(|slot| {
            let schema_ref = slot
                .schema_ref
                .as_ref()
                .map(|reference| {
                    materialize_value(context, &source_base, reference, STATE_SCHEMA_TYPE_URI)
                })
                .transpose()?;
            Ok(StateSlotDescriptor {
                state_slot_id: slot.id,
                owner_node_id: slot.owner,
                schema_ref,
                scope: slot.scope,
                portability: slot.portability,
                migration_port: slot.migration_port.map(|endpoint| PortEndpoint {
                    node_id: endpoint.node_id,
                    port_id: endpoint.port_id,
                }),
                backup_policy: slot.backup_policy,
                annotations: slot.annotations,
            })
        })
        .collect::<ModelResult<Vec<_>>>()?;
    let assembly = AssemblyRevision {
        schema: AssemblyRevision::SCHEMA.to_string(),
        assembly_id: source.assembly.id,
        nodes,
        bindings,
        exposed_ports,
        state_slots,
        annotations: source.assembly.annotations,
    };
    let object = CanonicalArtifactObject::from_model(&assembly)?;
    let descriptor = object.descriptor.clone();
    for (node_id, evidence) in pending_evidence {
        context.node_evidence.insert(
            NodeEvidenceKey {
                assembly_digest: descriptor.digest.clone(),
                node_id,
            },
            evidence,
        );
    }
    context
        .assemblies
        .insert(descriptor.digest.clone(), assembly);
    insert_object(&mut context.objects, object)?;
    context
        .assembly_cache
        .insert(path.clone(), descriptor.clone());
    context.assembly_stack.remove(&path);
    Ok(descriptor)
}

fn materialize_value(
    context: &mut MaterializeContext,
    base: &Path,
    reference: &SourcePathRef,
    artifact_type_uri: &str,
) -> ModelResult<ArtifactDescriptor> {
    if reference.fragment().is_some() {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "This source reference does not support a fragment",
        ));
    }
    let bytes = safe_read_source_ref(context, base, reference, true)?;
    let value: Value = serde_yaml::from_slice(&bytes).map_err(|_| {
        ModelError::new(
            DiagnosticCode::WorkInvalid,
            "Referenced metadata source is malformed",
        )
    })?;
    plurora_work::validate_portable_value(&value)?;
    let canonical = plurora_work::canonical_json_bytes(&value)?;
    let object = object_for_bytes(artifact_type_uri, "application/json", canonical, Vec::new())?;
    let descriptor = object.descriptor.clone();
    insert_object(&mut context.objects, object)?;
    Ok(descriptor)
}

fn materialize_typed<T>(
    context: &mut MaterializeContext,
    base: &Path,
    reference: &SourcePathRef,
) -> ModelResult<ArtifactDescriptor>
where
    T: ArtifactModel + for<'de> serde::Deserialize<'de>,
{
    if reference.fragment().is_some() {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "This source reference does not support a fragment",
        ));
    }
    let bytes = safe_read_source_ref(context, base, reference, true)?;
    let value: T = serde_yaml::from_slice(&bytes).map_err(|_| {
        ModelError::new(
            DiagnosticCode::WorkInvalid,
            "Typed metadata source is malformed",
        )
    })?;
    let object = CanonicalArtifactObject::from_model(&value)?;
    let descriptor = object.descriptor.clone();
    insert_object(&mut context.objects, object)?;
    Ok(descriptor)
}

#[derive(Serialize)]
struct DirectoryEntry {
    path: String,
    artifact: ArtifactDescriptor,
}

#[derive(Serialize)]
struct DirectoryObject {
    schema: &'static str,
    entries: Vec<DirectoryEntry>,
}

fn materialize_content(
    context: &mut MaterializeContext,
    base: &Path,
    reference: &SourcePathRef,
) -> ModelResult<ArtifactDescriptor> {
    if reference.fragment().is_some() {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "Content source reference does not support a fragment",
        ));
    }
    let path = resolve_contained(&context.root, base, Path::new(reference.path()))?;
    let metadata = fs::symlink_metadata(&path).map_err(|_| io_error())?;
    reject_link_like(&metadata)?;
    if metadata.is_file() {
        let object = object_for_file(&context.root, &path)?;
        let descriptor = object.descriptor().clone();
        insert_materialized_object(&mut context.objects, object)?;
        return Ok(descriptor);
    }
    if !metadata.is_dir() {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "Content source is neither a regular file nor a directory",
        ));
    }
    let directory_handle = Handle::from_path(&path).map_err(|_| io_error())?;
    let mut files = Vec::new();
    collect_content_files(&context.root, &path, &path, &mut files)?;
    files.sort();
    let mut entries = Vec::new();
    for file in files {
        let object = object_for_file(&context.root, &file)?;
        let relative = file
            .strip_prefix(&path)
            .map_err(|_| io_error())?
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        entries.push(DirectoryEntry {
            path: relative,
            artifact: object.descriptor().clone(),
        });
        insert_materialized_object(&mut context.objects, object)?;
    }
    let final_handle = Handle::from_path(&path).map_err(|_| io_error())?;
    if final_handle != directory_handle {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "Content directory changed while it was being read",
        ));
    }
    let value = DirectoryObject {
        schema: "plurora.content-directory.v1",
        entries,
    };
    let references = value
        .entries
        .iter()
        .map(|entry| entry.artifact.digest.clone())
        .collect();
    let bytes = plurora_work::canonical_json_bytes(&value)?;
    let object = object_for_bytes(
        CONTENT_DIRECTORY_TYPE_URI,
        "application/json",
        bytes,
        references,
    )?;
    let descriptor = object.descriptor.clone();
    insert_object(&mut context.objects, object)?;
    Ok(descriptor)
}

fn collect_content_files(
    root: &Path,
    base: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> ModelResult<()> {
    let metadata = fs::symlink_metadata(directory).map_err(|_| io_error())?;
    reject_link_like(&metadata)?;
    if !metadata.is_dir() {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "Content traversal encountered a non-directory",
        ));
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|_| io_error())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| io_error())?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|_| io_error())?;
        reject_link_like(&metadata)?;
        let canonical = fs::canonicalize(&path).map_err(|_| io_error())?;
        if !canonical.starts_with(root) || !canonical.starts_with(base) {
            return Err(ModelError::new(
                DiagnosticCode::RawPath,
                "Content source escapes its declared root",
            ));
        }
        if metadata.is_dir() {
            collect_content_files(root, base, &canonical, output)?;
        } else if metadata.is_file() {
            output.push(canonical);
        } else {
            return Err(ModelError::new(
                DiagnosticCode::RawPath,
                "Content source contains a non-regular file",
            ));
        }
    }
    Ok(())
}

fn object_for_bytes(
    artifact_type_uri: &str,
    media_type: &str,
    bytes: Vec<u8>,
    mut references: Vec<String>,
) -> ModelResult<CanonicalArtifactObject> {
    references.sort();
    references.dedup();
    let descriptor = ArtifactDescriptor {
        artifact_type_uri: artifact_type_uri.to_string(),
        media_type: media_type.to_string(),
        digest: world_bundle_sha256_digest(&bytes),
        size_bytes: bytes.len() as u64,
        references,
        annotations: BTreeMap::new(),
    };
    CanonicalArtifactObject::new(descriptor, bytes)
}

fn object_for_file(root: &Path, path: &Path) -> ModelResult<MaterializedObject> {
    let before = fs::symlink_metadata(path).map_err(|_| io_error())?;
    reject_link_like(&before)?;
    if !before.is_file() {
        return Err(raw_path());
    }
    let mut file = File::open(path).map_err(|_| io_error())?;
    let verification_file = file.try_clone().map_err(|_| io_error())?;
    let opened = Handle::from_file(verification_file.try_clone().map_err(|_| io_error())?)
        .map_err(|_| io_error())?;
    let opened_metadata = verification_file.metadata().map_err(|_| io_error())?;
    let canonical_root = fs::canonicalize(root).map_err(|_| io_error())?;
    let opened_path = fs::canonicalize(path).map_err(|_| io_error())?;
    if canonical_root != root
        || !opened_path.starts_with(&canonical_root)
        || Handle::from_path(path).map_err(|_| io_error())? != opened
    {
        return Err(raw_path());
    }
    let (digest, size_bytes) = digest_reader(&mut file)?;
    let after = fs::symlink_metadata(path).map_err(|_| io_error())?;
    reject_link_like(&after)?;
    if Handle::from_path(path).map_err(|_| io_error())? != opened
        || !same_file_metadata(&opened_metadata, &after)
        || size_bytes != after.len()
    {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "Content source changed while it was being hashed",
        ));
    }
    file.seek(SeekFrom::Start(0)).map_err(|_| io_error())?;
    Ok(MaterializedObject::OpenFile {
        descriptor: ArtifactDescriptor {
            artifact_type_uri: CONTENT_BLOB_TYPE_URI.to_string(),
            media_type: "application/octet-stream".to_string(),
            digest,
            size_bytes,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        },
        file,
    })
}

fn insert_object(
    objects: &mut BTreeMap<String, MaterializedObject>,
    object: CanonicalArtifactObject,
) -> ModelResult<()> {
    insert_materialized_object(objects, MaterializedObject::Memory(object))
}

fn insert_materialized_object(
    objects: &mut BTreeMap<String, MaterializedObject>,
    object: MaterializedObject,
) -> ModelResult<()> {
    let descriptor = object.descriptor().clone();
    if let Some(existing) = objects.get(&descriptor.digest) {
        if existing.descriptor() != &descriptor {
            return Err(ModelError::new(
                DiagnosticCode::ArtifactDigestMismatch,
                "Canonical closure contains inconsistent descriptors for one digest",
            ));
        }
        return Ok(());
    }
    objects.insert(descriptor.digest, object);
    Ok(())
}

fn verify_object_closure(objects: &BTreeMap<String, MaterializedObject>) -> ModelResult<()> {
    for object in objects.values() {
        verify_materialized_object(object)?;
        for reference in &object.descriptor().references {
            if !objects.contains_key(reference) {
                return Err(ModelError::new(
                    DiagnosticCode::ArtifactMissing,
                    "Canonical Work closure is missing a referenced artifact",
                ));
            }
        }
    }
    Ok(())
}

fn verify_materialized_object(object: &MaterializedObject) -> ModelResult<()> {
    match object {
        MaterializedObject::Memory(object) => {
            CanonicalArtifactObject::new(object.descriptor.clone(), object.bytes.clone())?;
        }
        MaterializedObject::OpenFile { descriptor, file } => {
            let mut source = file.try_clone().map_err(|_| io_error())?;
            source.seek(SeekFrom::Start(0)).map_err(|_| io_error())?;
            let (digest, size_bytes) = digest_reader(&mut source)?;
            if digest != descriptor.digest || size_bytes != descriptor.size_bytes {
                return Err(ModelError::new(
                    DiagnosticCode::ArtifactDigestMismatch,
                    "Open content artifact changed after it was materialized",
                ));
            }
        }
    }
    Ok(())
}

async fn persist_objects<'a>(
    root: &Path,
    objects: impl Iterator<Item = &'a MaterializedObject>,
) -> ModelResult<()> {
    let root = absolute_path(root)?;
    ensure_real_directory(&root, true)?;
    let canonical_root = fs::canonicalize(&root).map_err(|_| io_error())?;
    let root_dir = CapabilityDir::open_ambient_dir(&canonical_root, ambient_authority())
        .map_err(|_| io_error())?;
    verify_capability_directory(&root_dir, &canonical_root)?;
    match root_dir.symlink_metadata("sha256") {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(raw_path());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            root_dir.create_dir("sha256").map_err(|_| io_error())?;
        }
        Err(_) => return Err(io_error()),
    }
    let algorithm_dir = root_dir.open_dir("sha256").map_err(|_| raw_path())?;
    let canonical_algorithm = canonical_root.join("sha256");
    verify_capability_directory(&algorithm_dir, &canonical_algorithm)?;

    for object in objects {
        let descriptor = object.descriptor();
        let hex = descriptor
            .digest
            .strip_prefix("sha256:")
            .ok_or_else(|| object_store_error("ObjectStore artifact digest is invalid"))?;
        match algorithm_dir.symlink_metadata(hex) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(raw_path());
                }
                verify_capability_object(&algorithm_dir, &canonical_algorithm, hex, object)?;
                continue;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(io_error()),
        }
        let temporary = format!(".tmp-{}", uuid::Uuid::new_v4());
        let mut options = CapabilityOpenOptions::new();
        options.write(true).create_new(true);
        let write_result = (|| {
            let mut file = algorithm_dir
                .open_with(&temporary, &options)
                .map_err(|_| object_store_error("ObjectStore temporary object creation failed"))?;
            write_materialized_object(object, &mut file)?;
            file.flush()
                .map_err(|_| object_store_error("ObjectStore object flush failed"))?;
            file.sync_all()
                .map_err(|_| object_store_error("ObjectStore object sync failed"))?;
            drop(file);
            algorithm_dir
                .rename(&temporary, &algorithm_dir, hex)
                .map_err(|_| object_store_error("ObjectStore atomic object publish failed"))?;
            sync_capability_directory(&algorithm_dir)?;
            verify_capability_object(&algorithm_dir, &canonical_algorithm, hex, object)
        })();
        if write_result.is_err() {
            let _ = algorithm_dir.remove_file(&temporary);
        }
        write_result?;
    }
    Ok(())
}

fn verify_capability_directory(directory: &CapabilityDir, expected: &Path) -> ModelResult<()> {
    let opened = Handle::from_file(
        directory
            .try_clone()
            .map_err(|_| io_error())?
            .into_std_file(),
    )
    .map_err(|_| io_error())?;
    let current = Handle::from_path(expected).map_err(|_| io_error())?;
    let canonical = fs::canonicalize(expected).map_err(|_| io_error())?;
    if opened != current || canonical != expected {
        return Err(raw_path());
    }
    Ok(())
}

fn verify_capability_object(
    directory: &CapabilityDir,
    directory_path: &Path,
    name: &str,
    expected: &MaterializedObject,
) -> ModelResult<()> {
    let metadata = directory.symlink_metadata(name).map_err(|_| io_error())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(raw_path());
    }
    let object_path = directory_path.join(name);
    let path_metadata = fs::symlink_metadata(&object_path).map_err(|_| io_error())?;
    reject_link_like(&path_metadata)?;
    if !path_metadata.is_file() {
        return Err(raw_path());
    }
    let mut file = open_capability_object_nofollow(directory, name)?;
    let opened_handle =
        Handle::from_file(file.try_clone().map_err(|_| io_error())?).map_err(|_| io_error())?;
    let opened_metadata = file.metadata().map_err(|_| io_error())?;
    verify_opened_capability_object_identity(
        directory,
        &object_path,
        name,
        &file,
        &opened_handle,
        &opened_metadata,
    )?;
    let (digest, size_bytes) = digest_reader(&mut file)?;
    verify_opened_capability_object_identity(
        directory,
        &object_path,
        name,
        &file,
        &opened_handle,
        &opened_metadata,
    )?;
    if size_bytes != expected.descriptor().size_bytes || digest != expected.descriptor().digest {
        return Err(object_store_error(
            "ObjectStore integrity verification failed",
        ));
    }
    Ok(())
}

fn open_capability_object_nofollow(directory: &CapabilityDir, name: &str) -> ModelResult<File> {
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    directory
        .open_with(name, &options)
        .map(|file| file.into_std())
        .map_err(|_| raw_path())
}

fn verify_opened_capability_object_identity(
    directory: &CapabilityDir,
    object_path: &Path,
    name: &str,
    file: &File,
    opened_handle: &Handle,
    initial_metadata: &fs::Metadata,
) -> ModelResult<()> {
    let directory_path = object_path.parent().ok_or_else(io_error)?;
    verify_capability_directory(directory, directory_path)?;
    let capability_metadata = directory.symlink_metadata(name).map_err(|_| io_error())?;
    if capability_metadata.file_type().is_symlink() || !capability_metadata.is_file() {
        return Err(raw_path());
    }
    let current_metadata = fs::symlink_metadata(object_path).map_err(|_| io_error())?;
    reject_link_like(&current_metadata)?;
    if !current_metadata.is_file() {
        return Err(raw_path());
    }
    let opened_metadata = file.metadata().map_err(|_| io_error())?;
    let current_handle = Handle::from_path(object_path).map_err(|_| io_error())?;
    if current_handle != *opened_handle
        || !same_file_metadata(initial_metadata, &opened_metadata)
        || !same_file_metadata(&opened_metadata, &current_metadata)
    {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "ObjectStore entry changed while it was being verified",
        ));
    }
    Ok(())
}

fn write_materialized_object(
    object: &MaterializedObject,
    destination: &mut cap_std::fs::File,
) -> ModelResult<()> {
    match object {
        MaterializedObject::Memory(object) => destination
            .write_all(&object.bytes)
            .map_err(|_| object_store_error("ObjectStore object write failed")),
        MaterializedObject::OpenFile { descriptor, file } => {
            let mut source = file.try_clone().map_err(|_| io_error())?;
            source.seek(SeekFrom::Start(0)).map_err(|_| io_error())?;
            let mut hasher = Sha256::new();
            let mut size_bytes = 0_u64;
            let mut buffer = vec![0_u8; 64 * 1024];
            loop {
                let read = source.read(&mut buffer).map_err(|_| io_error())?;
                if read == 0 {
                    break;
                }
                destination
                    .write_all(&buffer[..read])
                    .map_err(|_| object_store_error("ObjectStore object write failed"))?;
                hasher.update(&buffer[..read]);
                size_bytes = size_bytes.saturating_add(read as u64);
            }
            let digest = format!("sha256:{:x}", hasher.finalize());
            if digest != descriptor.digest || size_bytes != descriptor.size_bytes {
                return Err(ModelError::new(
                    DiagnosticCode::ArtifactDigestMismatch,
                    "Open content artifact changed before ObjectStore import",
                ));
            }
            Ok(())
        }
    }
}

fn digest_reader(reader: &mut impl Read) -> ModelResult<(String, u64)> {
    let mut hasher = Sha256::new();
    let mut size_bytes = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(|_| io_error())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size_bytes = size_bytes.saturating_add(read as u64);
    }
    Ok((format!("sha256:{:x}", hasher.finalize()), size_bytes))
}

#[cfg(unix)]
fn sync_capability_directory(directory: &CapabilityDir) -> ModelResult<()> {
    directory
        .try_clone()
        .map_err(|_| io_error())?
        .into_std_file()
        .sync_all()
        .map_err(|_| io_error())
}

#[cfg(not(unix))]
fn sync_capability_directory(_directory: &CapabilityDir) -> ModelResult<()> {
    Ok(())
}

fn object_store_error(message: &'static str) -> ModelError {
    ModelError::new(DiagnosticCode::ArtifactDigestMismatch, message)
}

fn safe_read_source_ref(
    context: &MaterializeContext,
    base: &Path,
    reference: &SourcePathRef,
    bounded_yaml: bool,
) -> ModelResult<Vec<u8>> {
    let path = resolve_contained(&context.root, base, Path::new(reference.path()))?;
    safe_read_file(
        &context.root,
        &path,
        bounded_yaml.then_some(MAX_SOURCE_DESCRIPTOR_BYTES),
    )
}

fn safe_read_contained(
    root: &Path,
    base: &Path,
    relative: &Path,
    bounded_yaml: bool,
) -> ModelResult<Vec<u8>> {
    let path = resolve_contained(root, base, relative)?;
    safe_read_file(
        root,
        &path,
        bounded_yaml.then_some(MAX_SOURCE_DESCRIPTOR_BYTES),
    )
}

fn resolve_contained(root: &Path, base: &Path, relative: &Path) -> ModelResult<PathBuf> {
    if relative.is_absolute() {
        return Err(raw_path());
    }
    let mut current = base.to_path_buf();
    let mut directory_handles = Vec::new();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(raw_path());
        };
        current.push(segment);
        let metadata = fs::symlink_metadata(&current).map_err(|_| io_error())?;
        reject_link_like(&metadata)?;
        let canonical = fs::canonicalize(&current).map_err(|_| io_error())?;
        if !canonical.starts_with(root) {
            return Err(raw_path());
        }
        if metadata.is_dir() {
            directory_handles.push((
                canonical.clone(),
                Handle::from_path(&canonical).map_err(|_| io_error())?,
            ));
        }
        current = canonical;
    }
    for (directory, expected) in directory_handles {
        if Handle::from_path(directory).map_err(|_| io_error())? != expected {
            return Err(ModelError::new(
                DiagnosticCode::RawPath,
                "Source directory changed while a contained reference was being opened",
            ));
        }
    }
    Ok(current)
}

fn safe_read_file(
    containment_root: &Path,
    path: &Path,
    max_bytes: Option<usize>,
) -> ModelResult<Vec<u8>> {
    let before = fs::symlink_metadata(path).map_err(|_| io_error())?;
    reject_link_like(&before)?;
    if !before.is_file() {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "Referenced source is not a regular file",
        ));
    }
    let file = File::open(path).map_err(|_| io_error())?;
    let verification_file = file.try_clone().map_err(|_| io_error())?;
    let opened = Handle::from_file(verification_file.try_clone().map_err(|_| io_error())?)
        .map_err(|_| io_error())?;
    let opened_metadata = verification_file.metadata().map_err(|_| io_error())?;
    let opened_path = fs::canonicalize(path).map_err(|_| io_error())?;
    let canonical_root = fs::canonicalize(containment_root).map_err(|_| io_error())?;
    if canonical_root != containment_root || !opened_path.starts_with(&canonical_root) {
        return Err(raw_path());
    }
    let current = Handle::from_path(path).map_err(|_| io_error())?;
    if opened != current {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "Referenced source changed while it was being opened",
        ));
    }
    let mut bytes = Vec::new();
    if let Some(max) = max_bytes {
        file.take((max as u64) + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| io_error())?;
        if bytes.len() > max {
            return Err(ModelError::new(
                DiagnosticCode::ArtifactBudgetExceeded,
                "YAML source descriptor exceeds the 1 MiB implementation budget",
            ));
        }
    } else {
        let mut file = file;
        file.read_to_end(&mut bytes).map_err(|_| io_error())?;
    }
    let after = fs::symlink_metadata(path).map_err(|_| io_error())?;
    reject_link_like(&after)?;
    let final_handle = Handle::from_path(path).map_err(|_| io_error())?;
    let final_metadata = verification_file.metadata().map_err(|_| io_error())?;
    if opened != final_handle
        || !same_file_metadata(&opened_metadata, &final_metadata)
        || !same_file_metadata(&opened_metadata, &after)
        || final_metadata.len() != bytes.len() as u64
    {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "Referenced source changed while it was being read",
        ));
    }
    Ok(bytes)
}

fn same_file_metadata(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    before.len() == after.len()
        && match (before.modified(), after.modified()) {
            (Ok(before), Ok(after)) => before == after,
            _ => true,
        }
}

fn require_no_fragment(reference: &SourcePathRef) -> ModelResult<()> {
    if reference.fragment().is_some() {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "Nested Assembly source references do not support fragments",
        ));
    }
    Ok(())
}

fn validate_component_fragment(reference: &SourcePathRef, component_id: &str) -> ModelResult<()> {
    let Some(fragment) = reference.fragment() else {
        return Ok(());
    };
    if component_id.rsplit('/').next() != Some(fragment) {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            "Package source fragment does not match its Component identity",
        ));
    }
    Ok(())
}

fn reject_link_like(metadata: &fs::Metadata) -> ModelResult<()> {
    if metadata.file_type().is_symlink() || is_reparse_point(metadata) {
        return Err(ModelError::new(
            DiagnosticCode::RawPath,
            "Work paths must not traverse symlinks or junctions",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn raw_path() -> ModelError {
    ModelError::new(
        DiagnosticCode::RawPath,
        "Source reference is not contained by the Work source root",
    )
}

fn io_error() -> ModelError {
    ModelError::new(
        DiagnosticCode::ArtifactMissing,
        "A required Work source could not be opened safely",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use plurora_runtime::{FilesystemObjectStore, ObjectStore};
    use tempfile::TempDir;

    const MANIFEST: &str = r#"schema_version: 1
id: example/component
version: 1.0.0
entry:
  kind: rust_inproc
  crate_ref: example-component
  symbol: register
  abi_version: 1
provides: []
consumes: []
requires: []
contributes: {}
permissions: {}
sandbox_policy: {}
"#;

    fn write_nested_fixture(root: &Path) {
        fs::create_dir_all(root.join("packages/component")).unwrap();
        fs::write(root.join("packages/component/manifest.yaml"), MANIFEST).unwrap();
        fs::write(
            root.join("work.yaml"),
            r#"schema: plurora.work-source.v1
work:
  id: example/nested
  title: Nested
  assembly: assembly.yaml
  entrypoints:
    - id: play
      intent_uri: plurora.shell.default/play
      port_id: play
"#,
        )
        .unwrap();
        fs::write(
            root.join("assembly.yaml"),
            r#"schema: plurora.assembly-source.v1
assembly:
  id: example/root
  nodes:
    - id: child
      assembly: child.yaml
  exposed_ports:
    - id: play
      direction: export
      target: {node_id: child, port_id: play}
"#,
        )
        .unwrap();
        fs::write(
            root.join("child.yaml"),
            r#"schema: plurora.assembly-source.v1
assembly:
  id: example/child
  nodes:
    - id: component
      component: packages/component/manifest.yaml
      ports:
        - port_id: play
          contract:
            protocol_id: example.play
            interface_id: play
            version: 1.0.0
          interaction: plurora.interaction.capability-unary/v1
          role:
            kind: export
            multiplicity: {min: 0, max: 1}
            effect_class: external_effecting
  exposed_ports:
    - id: play
      direction: export
      target: {node_id: component, port_id: play}
"#,
        )
        .unwrap();
    }

    #[test]
    fn init_is_idempotent_and_refuses_different_user_files() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("work");
        assert_eq!(init_work(&root, "example/new").unwrap().len(), 2);
        assert!(init_work(&root, "example/new").unwrap().is_empty());
        fs::write(root.join("work.yaml"), "user data").unwrap();
        assert_eq!(
            init_work(&root, "example/new").unwrap_err().code,
            DiagnosticCode::WorkInvalid
        );
    }

    #[tokio::test]
    async fn nested_exposure_materializes_deterministically_and_persists_full_closure() {
        let temp = TempDir::new().unwrap();
        write_nested_fixture(temp.path());
        let first = materialize_work(temp.path()).unwrap();
        let second = materialize_work(&temp.path().join("work.yaml")).unwrap();
        assert_eq!(first.work_object.descriptor, second.work_object.descriptor);
        assert_eq!(first.resolution.exposed_ports[0].port_id.as_str(), "play");
        assert_eq!(first.resolution.exposed_ports[0].exposure_chain.len(), 2);
        let object_root = temp.path().join("store");
        persist_objects(&object_root, first.objects.values())
            .await
            .unwrap();
        let store = FilesystemObjectStore::new(&object_root);
        for object in first.objects.values() {
            assert!(store.has(&object.descriptor().digest).await.unwrap());
            assert_eq!(
                store
                    .verify(&object.descriptor().digest)
                    .await
                    .unwrap()
                    .size_bytes,
                object.descriptor().size_bytes
            );
        }
        assert!(!temp.path().join("profiles").exists());
        assert!(!temp.path().join("installations").exists());
        assert!(!temp.path().join("runs").exists());
    }

    #[tokio::test]
    async fn large_content_is_hashed_and_imported_from_an_open_file_stream() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("work.yaml"),
            "schema: plurora.work-source.v1\nwork:\n  id: example/content\n  title: Content\n  assembly: assembly.yaml\n  content: [large.bin]\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("assembly.yaml"),
            "schema: plurora.assembly-source.v1\nassembly:\n  id: example/content-assembly\n  nodes: []\n",
        )
        .unwrap();
        fs::write(temp.path().join("large.bin"), vec![0x5a; 8 * 1024 * 1024]).unwrap();
        let materialized = materialize_work(temp.path()).unwrap();
        let content = materialized
            .objects
            .values()
            .find(|object| object.descriptor().artifact_type_uri == CONTENT_BLOB_TYPE_URI)
            .unwrap();
        assert!(matches!(content, MaterializedObject::OpenFile { .. }));
        let object_root = temp.path().join("store");
        persist_objects(&object_root, materialized.objects.values())
            .await
            .unwrap();
        let store = FilesystemObjectStore::new(&object_root);
        assert_eq!(
            store
                .verify(&content.descriptor().digest)
                .await
                .unwrap()
                .size_bytes,
            8 * 1024 * 1024
        );
    }

    #[test]
    fn traversal_and_oversized_yaml_are_rejected_without_input_echo() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("work.yaml"),
            "schema: plurora.work-source.v1\nwork:\n  id: example/bad\n  title: Bad\n  assembly: ../secret.yaml\n",
        )
        .unwrap();
        let error = materialize_work(temp.path()).unwrap_err();
        assert_eq!(error.code, DiagnosticCode::WorkInvalid);
        assert!(!error
            .to_string()
            .contains(temp.path().to_string_lossy().as_ref()));
        assert!(!error.to_string().contains("secret.yaml"));

        let mut source = b"schema: plurora.work-source.v1\nwork:\n  id: example/big\n  title: Big\n  assembly: assembly.yaml\n".to_vec();
        source.resize(MAX_SOURCE_DESCRIPTOR_BYTES + 1, b' ');
        fs::write(temp.path().join("work.yaml"), source).unwrap();
        assert_eq!(
            materialize_work(temp.path()).unwrap_err().code,
            DiagnosticCode::ArtifactBudgetExceeded
        );
    }

    #[test]
    fn content_directory_metadata_uses_the_four_mib_canonical_budget() {
        let artifact = ArtifactDescriptor {
            artifact_type_uri: CONTENT_BLOB_TYPE_URI.to_string(),
            media_type: "application/octet-stream".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let value = DirectoryObject {
            schema: "plurora.content-directory.v1",
            entries: (0..25_000)
                .map(|index| DirectoryEntry {
                    path: format!("entry-{index:05}-{}.bin", "x".repeat(96)),
                    artifact: artifact.clone(),
                })
                .collect(),
        };
        assert_eq!(
            plurora_work::canonical_json_bytes(&value).unwrap_err().code,
            DiagnosticCode::ArtifactBudgetExceeded
        );
    }

    #[test]
    fn malformed_json_diagnostic_does_not_echo_secret_or_path() {
        let temp = TempDir::new().unwrap();
        let secret = "RawSecretExample1234567890abcdef";
        fs::write(temp.path().join("work.yaml"), format!("bad: {secret}")).unwrap();
        let error = materialize_work(temp.path()).unwrap_err();
        let output = serde_json::to_string(&ErrorReport {
            operation: "check",
            ok: false,
            diagnostics: vec![error.diagnostic()],
        })
        .unwrap();
        assert!(!output.contains(secret));
        assert!(!output.contains(temp.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn foreign_package_manifest_is_not_silently_treated_as_a_component_node() {
        let temp = TempDir::new().unwrap();
        write_nested_fixture(temp.path());
        let foreign = MANIFEST.replace(
            "  kind: rust_inproc\n",
            "  kind: rust_inproc\n  contract: none\n",
        );
        fs::write(
            temp.path().join("packages/component/manifest.yaml"),
            foreign,
        )
        .unwrap();
        assert_eq!(
            materialize_work(temp.path()).unwrap_err().code,
            DiagnosticCode::WorkInvalid
        );
    }

    #[cfg(unix)]
    #[test]
    fn source_and_intermediate_symlinks_are_rejected() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        write_nested_fixture(temp.path());
        let outside = TempDir::new().unwrap();
        fs::write(outside.path().join("assembly.yaml"), "outside").unwrap();
        fs::remove_file(temp.path().join("assembly.yaml")).unwrap();
        symlink(
            outside.path().join("assembly.yaml"),
            temp.path().join("assembly.yaml"),
        )
        .unwrap();
        assert_eq!(
            materialize_work(temp.path()).unwrap_err().code,
            DiagnosticCode::RawPath
        );

        fs::remove_file(temp.path().join("assembly.yaml")).unwrap();
        fs::write(
            temp.path().join("assembly.yaml"),
            "schema: plurora.assembly-source.v1\nassembly:\n  id: example/root\n  nodes:\n    - id: c\n      component: linked/component/manifest.yaml\n",
        )
        .unwrap();
        symlink(temp.path().join("packages"), temp.path().join("linked")).unwrap();
        assert_eq!(
            materialize_work(temp.path()).unwrap_err().code,
            DiagnosticCode::RawPath
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn object_store_algorithm_symlink_is_rejected() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let root = temp.path().join("store");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("sha256")).unwrap();
        let object = object_for_bytes(
            "urn:example:test-object:v1",
            "application/octet-stream",
            b"test".to_vec(),
            Vec::new(),
        )
        .unwrap();
        let object = MaterializedObject::Memory(object);
        assert_eq!(
            persist_objects(&root, std::iter::once(&object))
                .await
                .unwrap_err()
                .code,
            DiagnosticCode::RawPath
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn existing_object_symlink_is_rejected_even_when_target_content_matches() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let root = temp.path().join("store");
        let algorithm = root.join("sha256");
        let outside = temp.path().join("outside-object");
        fs::create_dir_all(&algorithm).unwrap();
        let object = object_for_bytes(
            "urn:example:test-object:v1",
            "application/octet-stream",
            b"matching object".to_vec(),
            Vec::new(),
        )
        .unwrap();
        fs::write(&outside, &object.bytes).unwrap();
        let hex = object.descriptor.digest.strip_prefix("sha256:").unwrap();
        symlink(&outside, algorithm.join(hex)).unwrap();
        let object = MaterializedObject::Memory(object);

        assert_eq!(
            persist_objects(&root, std::iter::once(&object))
                .await
                .unwrap_err()
                .code,
            DiagnosticCode::RawPath
        );
    }

    #[cfg(unix)]
    #[test]
    fn terminal_symlink_open_is_atomic_nofollow() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("target"), b"matching object").unwrap();
        symlink("target", temp.path().join("object")).unwrap();
        let directory = CapabilityDir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();

        assert_eq!(
            open_capability_object_nofollow(&directory, "object")
                .unwrap_err()
                .code,
            DiagnosticCode::RawPath
        );
    }

    #[test]
    fn opened_object_identity_rejects_same_content_entry_replacement() {
        let temp = TempDir::new().unwrap();
        let algorithm = temp.path().join("sha256");
        fs::create_dir_all(&algorithm).unwrap();
        let object_path = algorithm.join("object");
        fs::write(&object_path, b"same content").unwrap();
        let directory = CapabilityDir::open_ambient_dir(&algorithm, ambient_authority()).unwrap();
        let file = directory.open("object").unwrap().into_std();
        let opened_handle = Handle::from_file(file.try_clone().unwrap()).unwrap();
        let opened_metadata = file.metadata().unwrap();

        let replacement = algorithm.join("replacement");
        fs::write(&replacement, b"same content").unwrap();
        fs::remove_file(&object_path).unwrap();
        fs::rename(&replacement, &object_path).unwrap();

        assert_eq!(
            verify_opened_capability_object_identity(
                &directory,
                &object_path,
                "object",
                &file,
                &opened_handle,
                &opened_metadata,
            )
            .unwrap_err()
            .code,
            DiagnosticCode::RawPath
        );
    }

    #[tokio::test]
    async fn normal_existing_object_is_verified_idempotently() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("store");
        let object = object_for_bytes(
            "urn:example:test-object:v1",
            "application/octet-stream",
            b"existing object".to_vec(),
            Vec::new(),
        )
        .unwrap();
        let object = MaterializedObject::Memory(object);

        persist_objects(&root, std::iter::once(&object))
            .await
            .unwrap();
        persist_objects(&root, std::iter::once(&object))
            .await
            .unwrap();

        let hex = object.descriptor().digest.strip_prefix("sha256:").unwrap();
        assert_eq!(
            fs::read(root.join("sha256").join(hex)).unwrap(),
            b"existing object"
        );
    }

    #[cfg(windows)]
    #[test]
    fn intermediate_junction_is_rejected() {
        let temp = TempDir::new().unwrap();
        write_nested_fixture(temp.path());
        fs::write(
            temp.path().join("assembly.yaml"),
            "schema: plurora.assembly-source.v1\nassembly:\n  id: example/root\n  nodes:\n    - id: c\n      component: linked/component/manifest.yaml\n",
        )
        .unwrap();
        let link = temp.path().join("linked");
        let target = temp.path().join("packages");
        let status = std::process::Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&link)
            .arg(&target)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "test junction could not be created");
        assert_eq!(
            materialize_work(temp.path()).unwrap_err().code,
            DiagnosticCode::RawPath
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn object_store_algorithm_junction_is_rejected() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("store");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let link = root.join("sha256");
        let status = std::process::Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "test junction could not be created");
        let object = object_for_bytes(
            "urn:example:test-object:v1",
            "application/octet-stream",
            b"test".to_vec(),
            Vec::new(),
        )
        .unwrap();
        let object = MaterializedObject::Memory(object);
        assert_eq!(
            persist_objects(&root, std::iter::once(&object))
                .await
                .unwrap_err()
                .code,
            DiagnosticCode::RawPath
        );
    }
}
