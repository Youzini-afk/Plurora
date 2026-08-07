#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;

use plurora_core::*;
use plurora_runtime::*;

mod defs;
mod events;
mod methods;
mod write;

pub(crate) const SCHEMA: &str = "https://json-schema.org/draft/2020-12/schema";

use defs::schema_value;
use events::{event_schema, event_schemas};
use methods::method_schemas;
use write::{filename, write_json, write_method};

pub fn export_all() -> anyhow::Result<()> {
    let out = PathBuf::from("docs/spec/v1/schemas");
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
        out.join("composition-lock.schema.json"),
        &schema_value::<CompositionLock>(),
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
