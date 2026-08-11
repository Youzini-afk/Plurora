#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;

use plurora_core::*;
use plurora_runtime::*;
use plurora_work::*;

mod defs;
mod events;
mod methods;
mod write;

pub(crate) const SCHEMA: &str = "https://json-schema.org/draft/2020-12/schema";

use defs::schema_value;
pub(crate) use events::{event_schema, event_schemas};
pub(crate) use methods::{method_schema, method_schemas};
use write::{filename, write_json, write_method};

pub fn export_all() -> anyhow::Result<()> {
    let out = PathBuf::from("docs/spec/v1/schemas");
    if out.exists() {
        for entry in fs::read_dir(&out)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && entry
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".schema.json")
            {
                fs::remove_file(entry.path())?;
            }
        }
    }
    if out.join("methods").exists() {
        fs::remove_dir_all(out.join("methods"))?;
    }
    if out.join("events").exists() {
        fs::remove_dir_all(out.join("events"))?;
    }
    fs::create_dir_all(out.join("methods"))?;
    fs::create_dir_all(out.join("events"))?;

    write_json(
        out.join("manifest.schema.json"),
        &schema_value::<PackageManifest>(),
    )?;
    write_json(
        out.join("capability-descriptor.schema.json"),
        &schema_value::<CapabilityDescriptor>(),
    )?;
    write_json(
        out.join("permission-set.schema.json"),
        &schema_value::<PermissionSet>(),
    )?;
    write_json(
        out.join("event-envelope.schema.json"),
        &schema_value::<EventEnvelope>(),
    )?;
    write_json(
        out.join("protocol-context.schema.json"),
        &schema_value::<ProtocolContext>(),
    )?;
    write_json(
        out.join("protocol-response.schema.json"),
        &schema_value::<ProtocolResponse>(),
    )?;
    write_json(
        out.join("protocol-descriptor.schema.json"),
        &schema_value::<ProtocolDescriptor>(),
    )?;
    write_json(
        out.join("contract-selection.schema.json"),
        &schema_value::<ContractSelection>(),
    )?;
    write_json(
        out.join("capability-invocation-request.schema.json"),
        &schema_value::<CapabilityInvocationRequest>(),
    )?;
    write_json(
        out.join("capability-invocation-result.schema.json"),
        &schema_value::<CapabilityInvocationResult>(),
    )?;
    write_json(
        out.join("artifact-descriptor.schema.json"),
        &schema_value::<ArtifactDescriptor>(),
    )?;
    write_json(
        out.join("component-descriptor.schema.json"),
        &schema_value::<ComponentDescriptor>(),
    )?;
    write_json(
        out.join("package-envelope-descriptor.schema.json"),
        &schema_value::<PackageEnvelopeDescriptor>(),
    )?;
    write_json(
        out.join("work-revision.schema.json"),
        &schema_value::<WorkRevision>(),
    )?;
    write_json(
        out.join("port-descriptor.schema.json"),
        &schema_value::<PortDescriptor>(),
    )?;
    write_json(
        out.join("assembly-revision.schema.json"),
        &schema_value::<AssemblyRevision>(),
    )?;
    write_json(
        out.join("state-slot-descriptor.schema.json"),
        &schema_value::<StateSlotDescriptor>(),
    )?;
    write_json(
        out.join("assembly-lock.schema.json"),
        &schema_value::<AssemblyLock>(),
    )?;
    write_json(
        out.join("rights-declaration.schema.json"),
        &schema_value::<RightsDeclaration>(),
    )?;
    write_json(
        out.join("transparency-declaration.schema.json"),
        &schema_value::<TransparencyDeclaration>(),
    )?;
    write_json(
        out.join("installation-record.schema.json"),
        &schema_value::<InstallationRecord>(),
    )?;
    write_json(
        out.join("installation-state-snapshot.schema.json"),
        &schema_value::<InstallationStateSnapshot>(),
    )?;
    write_json(
        out.join("installation-state-decision-receipt.schema.json"),
        &schema_value::<InstallationStateDecisionReceipt>(),
    )?;
    write_json(
        out.join("installation-state-authority-evidence.schema.json"),
        &schema_value::<InstallationStateAuthorityEvidence>(),
    )?;
    write_json(
        out.join("run-record.schema.json"),
        &schema_value::<RunRecord>(),
    )?;
    write_json(
        out.join("exposure-record.schema.json"),
        &schema_value::<ExposureRecord>(),
    )?;
    write_json(
        out.join("active-binding-record.schema.json"),
        &schema_value::<ActiveBindingRecord>(),
    )?;
    write_json(
        out.join("operational-intent.schema.json"),
        &schema_value::<OperationalIntent>(),
    )?;
    write_json(
        out.join("target-inventory.schema.json"),
        &schema_value::<TargetInventorySnapshot>(),
    )?;
    write_json(
        out.join("realization-plan.schema.json"),
        &schema_value::<RealizationPlan>(),
    )?;
    write_json(
        out.join("realization-revision.schema.json"),
        &schema_value::<RealizationRevision>(),
    )?;
    write_json(
        out.join("world-bundle.schema.json"),
        &schema_value::<WorldBundleArchive>(),
    )?;
    write_json(
        out.join("world-head.schema.json"),
        &schema_value::<WorldHead>(),
    )?;
    write_json(
        out.join("world-journal-range.schema.json"),
        &schema_value::<WorldJournalRange>(),
    )?;
    write_json(
        out.join("effect-receipt.schema.json"),
        &schema_value::<EffectReceipt>(),
    )?;
    write_json(out.join("intent.schema.json"), &schema_value::<Intent>())?;
    write_json(
        out.join("change-set.schema.json"),
        &schema_value::<ChangeSet>(),
    )?;
    write_json(
        out.join("policy-decision.schema.json"),
        &schema_value::<PolicyDecision>(),
    )?;
    write_json(
        out.join("commit.schema.json"),
        &schema_value::<ChangeCommit>(),
    )?;

    for (method, params, result) in method_schemas() {
        write_method(&out, method, params, result)?;
    }

    for (kind, payload) in event_schemas() {
        write_json(
            out.join("events")
                .join(format!("{}.schema.json", filename(kind))),
            &event_schema(kind, payload),
        )?;
    }

    Ok(())
}
