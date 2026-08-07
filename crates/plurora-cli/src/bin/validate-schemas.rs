use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use jsonschema::JSONSchema;
use serde_json::Value;

const TOP_LEVEL_SCHEMAS: &[&str] = &[
    "artifact-descriptor.schema.json",
    "capability-descriptor.schema.json",
    "capability-invocation-request.schema.json",
    "capability-invocation-result.schema.json",
    "change-set.schema.json",
    "commit.schema.json",
    "component-descriptor.schema.json",
    "composition-lock.schema.json",
    "contract-selection.schema.json",
    "effect-receipt.schema.json",
    "event-envelope.schema.json",
    "intent.schema.json",
    "manifest.schema.json",
    "package-envelope-descriptor.schema.json",
    "permission-set.schema.json",
    "policy-decision.schema.json",
    "protocol-context.schema.json",
    "protocol-descriptor.schema.json",
    "protocol-response.schema.json",
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
        method_count == plurora_runtime::KernelMethod::all().len(),
        "method schema count {method_count} does not match registry {}",
        plurora_runtime::KernelMethod::all().len()
    );
    anyhow::ensure!(
        event_count >= 30,
        "event schema count {event_count} is unexpectedly low"
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

    // Additive-only diff hook for CI. Without a previous checkout, we validate the
    // canonical artifacts and leave breaking-change detection to CI that provides
    // BASE_SCHEMA_DIR.
    if let Ok(base) = std::env::var("BASE_SCHEMA_DIR") {
        let base = Path::new(&base);
        if base.exists() {
            let mut old_files = Vec::new();
            collect_json(base, &mut old_files)?;
            for old_file in old_files {
                let rel = old_file.strip_prefix(base)?;
                let new_file = root.join(rel);
                anyhow::ensure!(
                    new_file.exists(),
                    "breaking schema removal: {}",
                    rel.display()
                );
                let old_schema: Value = serde_json::from_str(&fs::read_to_string(&old_file)?)?;
                let new_schema: Value = serde_json::from_str(&fs::read_to_string(&new_file)?)?;
                validate_additive_schema(&old_schema, &new_schema, "$", rel)?;
            }
        }
    }
    println!(
        "validated {} schemas (methods: {method_count}, events: {event_count})",
        files.len()
    );
    Ok(())
}

fn validate_additive_schema(
    old: &Value,
    new: &Value,
    pointer: &str,
    file: &Path,
) -> anyhow::Result<()> {
    if let Some(old_values) = pure_string_enum_values(old) {
        let new_values = pure_string_enum_values(new).ok_or_else(|| {
            anyhow::anyhow!(
                "breaking string enum shape change in {} at {pointer}",
                file.display()
            )
        })?;
        anyhow::ensure!(
            old_values.is_subset(&new_values),
            "breaking enum narrowing in {} at {pointer}",
            file.display()
        );
        return Ok(());
    }

    if let Some(old_types) = schema_string_set(old.get("type")) {
        let new_types = schema_string_set(new.get("type"));
        anyhow::ensure!(
            new_types
                .as_ref()
                .is_none_or(|types| old_types.is_subset(types)),
            "breaking schema type narrowing in {} at {pointer}",
            file.display()
        );
    } else {
        anyhow::ensure!(
            new.get("type").is_none(),
            "breaking schema type constraint added in {} at {pointer}",
            file.display()
        );
    }

    if let Some(old_enum) = old.get("enum").and_then(Value::as_array) {
        let new_enum = new.get("enum").and_then(Value::as_array).ok_or_else(|| {
            anyhow::anyhow!("breaking enum removal in {} at {pointer}", file.display())
        })?;
        anyhow::ensure!(
            old_enum.iter().all(|value| new_enum.contains(value)),
            "breaking enum narrowing in {} at {pointer}",
            file.display()
        );
    }
    if let Some(old_const) = old.get("const") {
        anyhow::ensure!(
            new.get("const") == Some(old_const),
            "breaking const change in {} at {pointer}",
            file.display()
        );
    } else {
        anyhow::ensure!(
            new.get("const").is_none(),
            "breaking const constraint added in {} at {pointer}",
            file.display()
        );
    }

    let old_required = schema_string_set(old.get("required")).unwrap_or_default();
    let new_required = schema_string_set(new.get("required")).unwrap_or_default();
    anyhow::ensure!(
        new_required.is_subset(&old_required),
        "breaking required field added in {} at {pointer}: {:?}",
        file.display(),
        new_required.difference(&old_required).collect::<Vec<_>>()
    );

    compare_schema_maps(old, new, "properties", pointer, file)?;
    compare_schema_maps(old, new, "$defs", pointer, file)?;
    compare_schema_child(old, new, "items", pointer, file)?;
    compare_schema_child(old, new, "contains", pointer, file)?;
    compare_additional_properties(old, new, pointer, file)?;

    compare_lower_bound(old, new, "minimum", pointer, file)?;
    compare_lower_bound(old, new, "exclusiveMinimum", pointer, file)?;
    compare_lower_bound(old, new, "minLength", pointer, file)?;
    compare_lower_bound(old, new, "minItems", pointer, file)?;
    compare_lower_bound(old, new, "minProperties", pointer, file)?;
    compare_upper_bound(old, new, "maximum", pointer, file)?;
    compare_upper_bound(old, new, "exclusiveMaximum", pointer, file)?;
    compare_upper_bound(old, new, "maxLength", pointer, file)?;
    compare_upper_bound(old, new, "maxItems", pointer, file)?;
    compare_upper_bound(old, new, "maxProperties", pointer, file)?;

    compare_constraint(old, new, "$ref", pointer, file)?;
    compare_constraint(old, new, "pattern", pointer, file)?;
    compare_constraint(old, new, "format", pointer, file)?;
    compare_constraint(old, new, "not", pointer, file)?;
    compare_schema_array(old, new, "allOf", false, pointer, file)?;
    compare_schema_array(old, new, "anyOf", true, pointer, file)?;
    compare_schema_array(old, new, "oneOf", false, pointer, file)?;
    Ok(())
}

fn pure_string_enum_values(schema: &Value) -> Option<BTreeSet<String>> {
    let object = schema.as_object()?;
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "$schema"
                | "title"
                | "description"
                | "default"
                | "deprecated"
                | "examples"
                | "type"
                | "enum"
                | "oneOf"
        )
    }) {
        return None;
    }
    if let Some(values) = object.get("enum").and_then(Value::as_array) {
        let types = schema_string_set(object.get("type"))?;
        if types != BTreeSet::from(["string".to_string()]) {
            return None;
        }
        return values
            .iter()
            .map(|value| value.as_str().map(str::to_string))
            .collect();
    }
    let variants = object.get("oneOf")?.as_array()?;
    if variants.is_empty() {
        return None;
    }
    let mut values = BTreeSet::new();
    for variant in variants {
        values.extend(pure_string_enum_values(variant)?);
    }
    Some(values)
}

fn schema_string_set(value: Option<&Value>) -> Option<BTreeSet<String>> {
    match value? {
        Value::String(value) => Some(BTreeSet::from([value.clone()])),
        Value::Array(values) => Some(
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
        ),
        _ => None,
    }
}

fn compare_schema_maps(
    old: &Value,
    new: &Value,
    keyword: &str,
    pointer: &str,
    file: &Path,
) -> anyhow::Result<()> {
    let Some(old_map) = old.get(keyword).and_then(Value::as_object) else {
        return Ok(());
    };
    let new_map = new.get(keyword).and_then(Value::as_object).ok_or_else(|| {
        anyhow::anyhow!(
            "breaking {keyword} removal in {} at {pointer}",
            file.display()
        )
    })?;
    for (name, old_schema) in old_map {
        let new_schema = new_map.get(name).ok_or_else(|| {
            anyhow::anyhow!(
                "breaking {keyword} entry removal in {} at {pointer}/{keyword}/{name}",
                file.display()
            )
        })?;
        validate_additive_schema(
            old_schema,
            new_schema,
            &format!("{pointer}/{keyword}/{name}"),
            file,
        )?;
    }
    Ok(())
}

fn compare_schema_child(
    old: &Value,
    new: &Value,
    keyword: &str,
    pointer: &str,
    file: &Path,
) -> anyhow::Result<()> {
    match (old.get(keyword), new.get(keyword)) {
        (Some(old_schema), Some(new_schema)) => validate_additive_schema(
            old_schema,
            new_schema,
            &format!("{pointer}/{keyword}"),
            file,
        ),
        (Some(_), None) => Ok(()),
        (None, Some(_)) => anyhow::bail!(
            "breaking {keyword} constraint added in {} at {pointer}",
            file.display()
        ),
        (None, None) => Ok(()),
    }
}

fn compare_additional_properties(
    old: &Value,
    new: &Value,
    pointer: &str,
    file: &Path,
) -> anyhow::Result<()> {
    let old_value = old.get("additionalProperties");
    let new_value = new.get("additionalProperties");
    if old_value != Some(&Value::Bool(false)) && new_value == Some(&Value::Bool(false)) {
        anyhow::bail!(
            "breaking additionalProperties restriction in {} at {pointer}",
            file.display()
        );
    }
    if let (Some(old_schema @ Value::Object(_)), Some(new_schema @ Value::Object(_))) =
        (old_value, new_value)
    {
        validate_additive_schema(
            old_schema,
            new_schema,
            &format!("{pointer}/additionalProperties"),
            file,
        )?;
    }
    Ok(())
}

fn compare_lower_bound(
    old: &Value,
    new: &Value,
    keyword: &str,
    pointer: &str,
    file: &Path,
) -> anyhow::Result<()> {
    let old_value = old.get(keyword).and_then(Value::as_f64);
    let new_value = new.get(keyword).and_then(Value::as_f64);
    anyhow::ensure!(
        match (old_value, new_value) {
            (Some(old), Some(new)) => new <= old,
            (Some(_), None) | (None, None) => true,
            (None, Some(_)) => false,
        },
        "breaking {keyword} tightening in {} at {pointer}",
        file.display()
    );
    Ok(())
}

fn compare_upper_bound(
    old: &Value,
    new: &Value,
    keyword: &str,
    pointer: &str,
    file: &Path,
) -> anyhow::Result<()> {
    let old_value = old.get(keyword).and_then(Value::as_f64);
    let new_value = new.get(keyword).and_then(Value::as_f64);
    anyhow::ensure!(
        match (old_value, new_value) {
            (Some(old), Some(new)) => new >= old,
            (Some(_), None) | (None, None) => true,
            (None, Some(_)) => false,
        },
        "breaking {keyword} tightening in {} at {pointer}",
        file.display()
    );
    Ok(())
}

fn compare_constraint(
    old: &Value,
    new: &Value,
    keyword: &str,
    pointer: &str,
    file: &Path,
) -> anyhow::Result<()> {
    match (old.get(keyword), new.get(keyword)) {
        (Some(old), Some(new)) => anyhow::ensure!(
            old == new,
            "breaking {keyword} change in {} at {pointer}",
            file.display()
        ),
        (Some(_), None) | (None, None) => {}
        (None, Some(_)) => anyhow::bail!(
            "breaking {keyword} constraint added in {} at {pointer}",
            file.display()
        ),
    }
    Ok(())
}

fn compare_schema_array(
    old: &Value,
    new: &Value,
    keyword: &str,
    allow_additions: bool,
    pointer: &str,
    file: &Path,
) -> anyhow::Result<()> {
    let Some(old_items) = old.get(keyword).and_then(Value::as_array) else {
        anyhow::ensure!(
            new.get(keyword).is_none(),
            "breaking {keyword} constraint added in {} at {pointer}",
            file.display()
        );
        return Ok(());
    };
    let new_items = new.get(keyword).and_then(Value::as_array).ok_or_else(|| {
        anyhow::anyhow!(
            "breaking {keyword} removal in {} at {pointer}",
            file.display()
        )
    })?;
    anyhow::ensure!(
        if allow_additions {
            new_items.len() >= old_items.len()
        } else {
            new_items.len() == old_items.len()
        },
        "breaking {keyword} shape change in {} at {pointer}",
        file.display()
    );
    for (index, old_schema) in old_items.iter().enumerate() {
        validate_additive_schema(
            old_schema,
            &new_items[index],
            &format!("{pointer}/{keyword}/{index}"),
            file,
        )?;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_equivalent_one_of_to_flat_string_enum() -> anyhow::Result<()> {
        let old = serde_json::json!({
            "oneOf": [
                { "type": "string", "enum": ["available", "offline"] },
                {
                    "description": "legacy value",
                    "type": "string",
                    "enum": ["unavailable"]
                }
            ]
        });
        let new = serde_json::json!({
            "type": "string",
            "enum": ["available", "offline", "unavailable"]
        });
        validate_additive_schema(&old, &new, "$/$defs/status", Path::new("target.json"))
    }

    #[test]
    fn rejects_value_loss_while_flattening_string_enum() {
        let old = serde_json::json!({
            "oneOf": [
                { "type": "string", "enum": ["available", "offline"] },
                { "type": "string", "enum": ["unavailable"] }
            ]
        });
        let new = serde_json::json!({
            "type": "string",
            "enum": ["available", "offline"]
        });
        assert!(
            validate_additive_schema(&old, &new, "$/$defs/status", Path::new("target.json"))
                .is_err()
        );
    }
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
