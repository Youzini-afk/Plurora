use std::fs;
use std::path::Path;

use jsonschema::JSONSchema;
use serde_json::Value;

const TOP_LEVEL_SCHEMAS: &[&str] = &[
    "active-binding-record.schema.json",
    "artifact-descriptor.schema.json",
    "assembly-lock.schema.json",
    "assembly-revision.schema.json",
    "capability-descriptor.schema.json",
    "capability-invocation-request.schema.json",
    "capability-invocation-result.schema.json",
    "change-set.schema.json",
    "commit.schema.json",
    "component-descriptor.schema.json",
    "contract-selection.schema.json",
    "effect-receipt.schema.json",
    "event-envelope.schema.json",
    "exposure-record.schema.json",
    "installation-record.schema.json",
    "intent.schema.json",
    "manifest.schema.json",
    "package-envelope-descriptor.schema.json",
    "permission-set.schema.json",
    "policy-decision.schema.json",
    "port-descriptor.schema.json",
    "protocol-context.schema.json",
    "protocol-descriptor.schema.json",
    "protocol-response.schema.json",
    "realization-plan.schema.json",
    "realization-revision.schema.json",
    "rights-declaration.schema.json",
    "run-record.schema.json",
    "state-slot-descriptor.schema.json",
    "target-inventory.schema.json",
    "transparency-declaration.schema.json",
    "operational-intent.schema.json",
    "work-revision.schema.json",
    "world-bundle.schema.json",
    "world-head.schema.json",
    "world-journal-range.schema.json",
];

fn main() -> anyhow::Result<()> {
    let root = Path::new("docs/spec/v1/schemas");
    anyhow::ensure!(
        root.exists(),
        "schema directory missing; run cargo run -p plurora-cli --bin export-schemas"
    );
    let mut files = Vec::new();
    collect_json(root, &mut files)?;
    anyhow::ensure!(!files.is_empty(), "no schema files found");
    for file in &files {
        let text = fs::read_to_string(file)?;
        let schema: Value = serde_json::from_str(&text)?;
        let dialect = schema
            .get("$schema")
            .and_then(Value::as_str)
            .unwrap_or_default();
        anyhow::ensure!(
            dialect == "https://json-schema.org/draft/2020-12/schema",
            "{} is not JSON Schema 2020-12",
            file.display()
        );
        validate_local_refs(&schema, &schema, file)?;
        JSONSchema::compile(&schema).map_err(|error| {
            anyhow::anyhow!("{} failed to compile as schema: {error}", file.display())
        })?;
    }

    let method_count = fs::read_dir(root.join("methods"))?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .count();
    let event_count = fs::read_dir(root.join("events"))?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .count();
    let top_level_count = fs::read_dir(root)?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .count();
    anyhow::ensure!(
        method_count == plurora_runtime::PlatformMethod::all().len(),
        "method schema count {method_count} does not match registry {}",
        plurora_runtime::PlatformMethod::all().len()
    );
    anyhow::ensure!(
        event_count == plurora_core::PLATFORM_EVENT_KINDS.len(),
        "event schema count {event_count} does not match registry {}",
        plurora_core::PLATFORM_EVENT_KINDS.len()
    );
    anyhow::ensure!(
        top_level_count == TOP_LEVEL_SCHEMAS.len(),
        "top-level schema count {top_level_count} does not match the canonical set"
    );
    for schema in TOP_LEVEL_SCHEMAS {
        anyhow::ensure!(
            root.join(schema).exists(),
            "top-level schema '{schema}' is missing"
        );
    }

    println!(
        "validated {} schemas (methods: {method_count}, events: {event_count})",
        files.len()
    );
    Ok(())
}

fn validate_local_refs(value: &Value, root: &Value, file: &Path) -> anyhow::Result<()> {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                if let Some(pointer) = reference.strip_prefix('#') {
                    anyhow::ensure!(
                        pointer.is_empty() || root.pointer(pointer).is_some(),
                        "{} contains unresolved local schema reference {}",
                        file.display(),
                        reference
                    );
                }
            }
            for child in map.values() {
                validate_local_refs(child, root, file)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                validate_local_refs(child, root, file)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn collect_json(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_json(&path, out)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
            out.push(path);
        }
    }
    Ok(())
}
