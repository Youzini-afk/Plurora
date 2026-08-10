use std::collections::BTreeSet;
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
    "installation-state-authority-evidence.schema.json",
    "installation-state-decision-receipt.schema.json",
    "installation-state-snapshot.schema.json",
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
    let expected_method_ids = plurora_runtime::PlatformMethod::all()
        .iter()
        .map(|method| method.id().to_string())
        .collect::<BTreeSet<_>>();
    let actual_method_ids = schema_constants(&root.join("methods"), "/properties/method/const")?;
    let expected_event_kinds = plurora_core::PLATFORM_EVENT_KINDS
        .iter()
        .map(|kind| (*kind).to_string())
        .collect::<BTreeSet<_>>();
    let actual_event_kinds = schema_constants(&root.join("events"), "/properties/kind/const")?;
    let expected_top_level = TOP_LEVEL_SCHEMAS
        .iter()
        .map(|name| (*name).to_string())
        .collect::<BTreeSet<_>>();
    let actual_top_level = fs::read_dir(root)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json"))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        expected_method_ids.len() == 80,
        "method registry must contain exactly 80 identities"
    );
    anyhow::ensure!(
        expected_event_kinds.len() == 58,
        "event registry must contain exactly 58 identities"
    );
    anyhow::ensure!(
        expected_top_level.len() == 39,
        "top-level registry must contain exactly 39 identities"
    );
    anyhow::ensure!(
        method_count == plurora_runtime::PlatformMethod::all().len(),
        "method schema count {method_count} does not match registry {}",
        plurora_runtime::PlatformMethod::all().len()
    );
    anyhow::ensure!(
        actual_method_ids == expected_method_ids,
        "method schema identities differ from the runtime registry"
    );
    anyhow::ensure!(
        event_count == plurora_core::PLATFORM_EVENT_KINDS.len(),
        "event schema count {event_count} does not match registry {}",
        plurora_core::PLATFORM_EVENT_KINDS.len()
    );
    anyhow::ensure!(
        actual_event_kinds == expected_event_kinds,
        "event schema identities differ from the core registry"
    );
    anyhow::ensure!(
        top_level_count == TOP_LEVEL_SCHEMAS.len(),
        "top-level schema count {top_level_count} does not match the canonical set"
    );
    anyhow::ensure!(
        actual_top_level == expected_top_level,
        "top-level schema identities differ from the canonical set"
    );
    let expected_total =
        expected_method_ids.len() + expected_event_kinds.len() + expected_top_level.len();
    anyhow::ensure!(
        expected_total == 177,
        "public contract registry must contain exactly 177 identities"
    );
    anyhow::ensure!(
        files.len() == expected_total,
        "public contract contains {} schemas, expected {expected_total}",
        files.len()
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

fn schema_constants(dir: &Path, pointer: &str) -> anyhow::Result<BTreeSet<String>> {
    let mut values = BTreeSet::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let schema: Value = serde_json::from_str(&fs::read_to_string(entry.path())?)?;
        let value = schema
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("{} is missing {pointer}", entry.path().display()))?;
        anyhow::ensure!(
            values.insert(value.to_string()),
            "duplicate schema identity {value}"
        );
    }
    Ok(values)
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
