//! Generate SDKs from docs/spec/v1/schemas/ for TS, Rust, OpenAPI.
//!
//! Outputs:
//!   sdk/typescript/contract-sdk/src/{types.ts, methods.ts, events.ts, index.ts}
//!   sdk/rust/plurora-contract-sdk/src/{types.rs, methods.rs, events.rs, lib.rs}
//!   sdk/openapi.yaml

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use heck::{ToLowerCamelCase, ToPascalCase, ToSnakeCase};
use schemars::schema::Schema;
use serde_json::{json, Map, Value};
use typify::{TypeSpace, TypeSpaceSettings};

const SCHEMA_DIR: &str = "docs/spec/v1/schemas";
const TS_DIR: &str = "sdk/typescript/contract-sdk/src";
const RUST_DIR: &str = "sdk/rust/plurora-contract-sdk/src";
const OPENAPI: &str = "sdk/openapi.yaml";
#[derive(Clone)]
struct MethodSpec {
    schema_id: String,
    id: String,
    function_ts: String,
    function_rs: String,
    params_ts: String,
    result_ts: String,
    params_rs: String,
    result_rs: String,
    params_schema: Value,
    result_schema: Value,
    openapi_aliases: BTreeMap<String, String>,
    contract: Value,
}

#[derive(Clone)]
struct EventSpec {
    kind: String,
    payload_alias: String,
    event_name: String,
    payload_ts: String,
    payload_rs: String,
}

struct TypeRegistry {
    schemas: BTreeMap<String, Value>,
}

impl TypeRegistry {
    fn new() -> Self {
        Self {
            schemas: BTreeMap::new(),
        }
    }

    fn register(&mut self, preferred: impl AsRef<str>, schema: &Value) -> String {
        let mut name = sanitize_type_name(preferred.as_ref());
        if name.is_empty() {
            name = "GeneratedType".to_string();
        }

        if let Some(existing) = self.schemas.get(&name) {
            if schemas_equivalent(existing, schema) {
                return name;
            }
        } else {
            self.schemas.insert(name.clone(), schema.clone());
            return name;
        }

        let base = name;
        let mut i = 2;
        loop {
            let candidate = format!("{base}{i}");
            match self.schemas.get(&candidate) {
                Some(existing) if schemas_equivalent(existing, schema) => return candidate,
                Some(_) => i += 1,
                None => {
                    self.schemas.insert(candidate.clone(), schema.clone());
                    return candidate;
                }
            }
        }
    }
}

fn main() -> Result<()> {
    fs::create_dir_all(TS_DIR)?;
    fs::create_dir_all(RUST_DIR)?;

    let schemas = read_schema_files(Path::new(SCHEMA_DIR))?;
    let mut registry = TypeRegistry::new();

    collect_top_level_types(&schemas, &mut registry);
    let methods = collect_methods(&schemas, &mut registry)?;
    validate_method_specs(&methods)?;
    let events = collect_events(&schemas, &mut registry)?;

    write_typescript(&registry, &methods, &events)?;
    write_rust(&registry, &methods, &events)?;
    write_openapi(&methods, &registry)?;

    Ok(())
}

fn read_schema_files(root: &Path) -> Result<BTreeMap<PathBuf, Value>> {
    let mut out = BTreeMap::new();
    visit_schema_files(root, &mut out)?;
    Ok(out)
}

fn visit_schema_files(dir: &Path, out: &mut BTreeMap<PathBuf, Value>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            visit_schema_files(&path, out)?;
        } else if path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|name| name.ends_with(".schema.json"))
        {
            let data = fs::read_to_string(&path)?;
            let value: Value = serde_json::from_str(&data)
                .with_context(|| format!("parsing {}", path.display()))?;
            out.insert(path, value);
        }
    }
    Ok(())
}

fn collect_top_level_types(schemas: &BTreeMap<PathBuf, Value>, registry: &mut TypeRegistry) {
    for (path, schema) in schemas {
        if path.components().any(|c| c.as_os_str() == "methods")
            || path.components().any(|c| c.as_os_str() == "events")
        {
            continue;
        }
        let name = schema_title(schema).unwrap_or_else(|| file_stem_type_name(path));
        registry.register(&name, schema);
        collect_defs(schema, registry);
    }
}

fn collect_methods(
    schemas: &BTreeMap<PathBuf, Value>,
    registry: &mut TypeRegistry,
) -> Result<Vec<MethodSpec>> {
    let mut methods = Vec::new();
    for (path, schema) in schemas {
        if !path.components().any(|c| c.as_os_str() == "methods") {
            continue;
        }
        let id = schema
            .pointer("/properties/method/const")
            .and_then(Value::as_str)
            .or_else(|| schema.get("title").and_then(Value::as_str))
            .ok_or_else(|| anyhow!("method schema {} has no method const", path.display()))?
            .to_string();
        let schema_id = id.clone();
        let contract = schema
            .get("x-plurora-contract")
            .ok_or_else(|| anyhow!("method schema {schema_id} has no x-plurora-contract"))?
            .clone();
        let defs = schema.get("$defs").or_else(|| schema.get("definitions"));
        let params_schema = defs
            .and_then(|d| d.get("Params"))
            .ok_or_else(|| anyhow!("method schema {schema_id} has no Params"))?
            .clone();
        let result_schema = defs
            .and_then(|d| d.get("Result"))
            .ok_or_else(|| anyhow!("method schema {schema_id} has no Result"))?
            .clone();

        collect_document_defs(schema, &["Params", "Result", "Errors"], registry);
        collect_defs(&params_schema, registry);
        collect_defs(&result_schema, registry);

        let base = method_base_name(&schema_id);
        let params_name = schema_title(&params_schema).unwrap_or_else(|| format!("{base}Params"));
        let result_name = schema_title(&result_schema).unwrap_or_else(|| format!("{base}Result"));
        let params_ts = registry.register(params_name, &params_schema);
        let result_ts = registry.register(result_name, &result_schema);
        let openapi_aliases =
            referenced_definition_aliases([&params_schema, &result_schema], schema, registry)?;
        let function_ts = method_function_ts(&schema_id);
        let function_rs = method_function_rs(&schema_id);
        methods.push(MethodSpec {
            schema_id,
            id: id.clone(),
            function_ts,
            function_rs,
            params_rs: params_ts.clone(),
            result_rs: result_ts.clone(),
            params_ts,
            result_ts,
            params_schema,
            result_schema,
            openapi_aliases,
            contract,
        });
    }
    methods.sort_by(|a, b| a.schema_id.cmp(&b.schema_id));
    Ok(methods)
}

fn validate_method_specs(methods: &[MethodSpec]) -> Result<()> {
    let mut exposed_ids = BTreeMap::new();
    let mut typescript_functions = BTreeMap::new();
    let mut rust_functions = BTreeMap::new();

    for method in methods {
        if method.id != method.schema_id {
            return Err(anyhow!(
                "method schema {} exposes a different wire id {}",
                method.schema_id,
                method.id
            ));
        }
        reserve_unique(
            &mut exposed_ids,
            &method.id,
            &method.schema_id,
            "RPC method id",
        )?;
        reserve_unique(
            &mut typescript_functions,
            &method.function_ts,
            &method.schema_id,
            "TypeScript method name / OpenAPI operationId",
        )?;
        reserve_unique(
            &mut rust_functions,
            &method.function_rs,
            &method.schema_id,
            "Rust method name",
        )?;
    }
    Ok(())
}

fn reserve_unique(
    registry: &mut BTreeMap<String, String>,
    value: &str,
    owner: &str,
    kind: &str,
) -> Result<()> {
    if value.is_empty() {
        return Err(anyhow!("method schema {owner} has an empty {kind}"));
    }
    if let Some(existing_owner) = registry.insert(value.to_string(), owner.to_string()) {
        return Err(anyhow!(
            "duplicate {kind} {value}: declared by {existing_owner} and {owner}"
        ));
    }
    Ok(())
}

fn collect_events(
    schemas: &BTreeMap<PathBuf, Value>,
    registry: &mut TypeRegistry,
) -> Result<Vec<EventSpec>> {
    let mut events = Vec::new();
    for (path, schema) in schemas {
        if !path.components().any(|c| c.as_os_str() == "events") {
            continue;
        }
        let kind = schema
            .pointer("/properties/kind/const")
            .and_then(Value::as_str)
            .or_else(|| schema.get("title").and_then(Value::as_str))
            .ok_or_else(|| anyhow!("event schema {} has no kind const", path.display()))?
            .to_string();
        let payload_schema = schema
            .get("$defs")
            .or_else(|| schema.get("definitions"))
            .and_then(|d| d.get("Payload"))
            .ok_or_else(|| anyhow!("event schema {kind} has no Payload"))?
            .clone();
        collect_document_defs(schema, &["Payload"], registry);
        collect_defs(&payload_schema, registry);
        let event_name = event_base_name(&kind);
        let payload_name =
            schema_title(&payload_schema).unwrap_or_else(|| format!("{event_name}Payload"));
        let payload_ts = registry.register(payload_name, &payload_schema);
        events.push(EventSpec {
            kind,
            payload_alias: format!("{event_name}Payload"),
            event_name: format!("{event_name}Event"),
            payload_rs: payload_ts.clone(),
            payload_ts,
        });
    }
    events.sort_by(|a, b| a.kind.cmp(&b.kind));
    Ok(events)
}

fn collect_defs(schema: &Value, registry: &mut TypeRegistry) {
    match schema {
        Value::Object(map) => {
            for definitions_key in ["$defs", "definitions"] {
                if let Some(Value::Object(definitions)) = map.get(definitions_key) {
                    for (key, value) in definitions {
                        let name = schema_title(value).unwrap_or_else(|| key.to_pascal_case());
                        registry.register(name, value);
                        collect_defs(value, registry);
                    }
                }
            }
            for (key, value) in map {
                if key != "$defs" && key != "definitions" {
                    collect_defs(value, registry);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_defs(value, registry);
            }
        }
        _ => {}
    }
}

fn collect_document_defs(schema: &Value, excluded: &[&str], registry: &mut TypeRegistry) {
    let Some(definitions) = schema
        .get("$defs")
        .or_else(|| schema.get("definitions"))
        .and_then(Value::as_object)
    else {
        return;
    };
    for (key, value) in definitions {
        if excluded.contains(&key.as_str()) {
            continue;
        }
        let name = schema_title(value).unwrap_or_else(|| key.to_pascal_case());
        registry.register(name, value);
        collect_defs(value, registry);
    }
}

fn referenced_definition_aliases<'a>(
    schemas: impl IntoIterator<Item = &'a Value>,
    document: &Value,
    registry: &TypeRegistry,
) -> Result<BTreeMap<String, String>> {
    let mut referenced = BTreeSet::new();
    for schema in schemas {
        collect_local_ref_names(schema, &mut referenced);
    }
    let definitions = document
        .get("$defs")
        .or_else(|| document.get("definitions"))
        .and_then(Value::as_object);
    let mut aliases = BTreeMap::new();
    for local_name in referenced {
        if let Some(schema) = definitions.and_then(|definitions| definitions.get(&local_name)) {
            let preferred = schema_title(schema).unwrap_or_else(|| local_name.to_pascal_case());
            aliases.insert(
                local_name,
                registered_schema_name(registry, &preferred, schema)?,
            );
        } else {
            let candidate = sanitize_type_name(&local_name);
            if registry.schemas.contains_key(&candidate) {
                aliases.insert(local_name, candidate);
            }
        }
    }
    Ok(aliases)
}

fn collect_local_ref_names(value: &Value, names: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                if let Some(local) = reference
                    .strip_prefix("#/$defs/")
                    .or_else(|| reference.strip_prefix("#/definitions/"))
                {
                    if let Some(name) = local.split('/').next() {
                        names.insert(name.to_string());
                    }
                }
            }
            for child in map.values() {
                collect_local_ref_names(child, names);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_local_ref_names(child, names);
            }
        }
        _ => {}
    }
}

fn write_typescript(
    registry: &TypeRegistry,
    methods: &[MethodSpec],
    events: &[EventSpec],
) -> Result<()> {
    fs::write(Path::new(TS_DIR).join("types.ts"), emit_ts_types(registry))?;
    fs::write(
        Path::new(TS_DIR).join("methods.ts"),
        emit_ts_methods(methods),
    )?;
    fs::write(Path::new(TS_DIR).join("events.ts"), emit_ts_events(events))?;
    fs::write(
        Path::new(TS_DIR).join("index.ts"),
        "export * from \"./client\";\nexport * from \"./types\";\nexport * from \"./events\";\nexport * from \"./methods\";\n",
    )?;
    Ok(())
}

fn emit_ts_types(registry: &TypeRegistry) -> String {
    let mut out = generated_header("TypeScript types generated from docs/spec/v1/schemas/.");
    let ctx = TsContext { registry };
    for (name, schema) in &registry.schemas {
        out.push_str("\n");
        if let Some(description) = schema.get("description").and_then(Value::as_str) {
            out.push_str(&doc_comment(description));
        }
        if schema
            .get("properties")
            .and_then(Value::as_object)
            .is_some()
            && schema.get("oneOf").is_none()
            && schema.get("anyOf").is_none()
            && schema.get("allOf").is_none()
        {
            out.push_str(&format!("export interface {name} "));
            out.push_str(&ctx.object_body(schema, 0));
            out.push_str("\n");
        } else {
            out.push_str(&format!(
                "export type {name} = {};\n",
                ctx.type_expr(schema)
            ));
        }
    }
    out
}

fn emit_ts_methods(methods: &[MethodSpec]) -> String {
    let mut out =
        generated_header("TypeScript client methods generated from docs/spec/v1/schemas/methods/.");
    out.push_str("import type { PluroraClient } from \"./client\";\n");
    out.push_str("import type { ");
    let imports = methods
        .iter()
        .flat_map(|m| [m.params_ts.as_str(), m.result_ts.as_str()])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&imports);
    out.push_str(" } from \"./types\";\n\n");

    out.push_str("export interface PlatformMethods {\n");
    for method in methods {
        out.push_str(&format!(
            "  {}(params: {}): Promise<{}>;\n",
            method.function_ts, method.params_ts, method.result_ts
        ));
    }
    out.push_str("}\n\n");

    out.push_str(
        "declare module \"./client\" {\n  interface PluroraClient extends PlatformMethods {}\n}\n\n",
    );

    for method in methods {
        out.push_str(&format!(
            "export async function {}(\n  this: PluroraClient,\n  params: {},\n): Promise<{}> {{\n  return this.invoke(\"{}\", params) as Promise<{}>;\n}}\n\n",
            method.function_ts, method.params_ts, method.result_ts, method.id, method.result_ts
        ));
    }

    out.push_str(
        "export function attach<T extends PluroraClient>(client: T): T & PlatformMethods {\n",
    );
    for method in methods {
        out.push_str(&format!(
            "  (client as T & PlatformMethods).{} = {}.bind(client);\n",
            method.function_ts, method.function_ts
        ));
    }
    out.push_str("  return client as T & PlatformMethods;\n}\n");
    out
}

fn emit_ts_events(events: &[EventSpec]) -> String {
    let mut out =
        generated_header("TypeScript event payloads generated from docs/spec/v1/schemas/events/.");
    out.push_str("import type { ");
    let imports = events
        .iter()
        .map(|e| e.payload_ts.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&imports);
    out.push_str(" } from \"./types\";\n\n");

    for event in events {
        out.push_str(&format!(
            "export interface {} {{\n  kind: {};\n  payload: {};\n}}\n\n",
            event.event_name,
            json_string(&event.kind),
            event.payload_ts
        ));
    }
    out.push_str("export interface PlatformEventPayloadMap {\n");
    for event in events {
        out.push_str(&format!(
            "  {}: {};\n",
            json_string(&event.kind),
            event.payload_ts
        ));
    }
    out.push_str("}\n\nexport type PlatformEvent =\n");
    for event in events {
        out.push_str(&format!("  | {}\n", event.event_name));
    }
    out.push_str(";\n");
    out
}

struct TsContext<'a> {
    registry: &'a TypeRegistry,
}

impl TsContext<'_> {
    fn type_expr(&self, schema: &Value) -> String {
        match schema {
            Value::Bool(true) => "unknown".to_string(),
            Value::Bool(false) => "never".to_string(),
            Value::Object(map) => self.type_expr_object(map),
            _ => "unknown".to_string(),
        }
    }

    fn type_expr_object(&self, map: &Map<String, Value>) -> String {
        if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
            return self.ref_name(reference);
        }
        if let Some(value) = map.get("const") {
            return literal_ts(value);
        }
        if let Some(values) = map.get("enum").and_then(Value::as_array) {
            return values
                .iter()
                .map(literal_ts)
                .collect::<Vec<_>>()
                .join(" | ");
        }
        if let Some(values) = map.get("oneOf").and_then(Value::as_array) {
            return union_ts(values.iter().map(|v| self.type_expr(v)).collect());
        }
        if let Some(values) = map.get("anyOf").and_then(Value::as_array) {
            return union_ts(values.iter().map(|v| self.type_expr(v)).collect());
        }
        if let Some(values) = map.get("allOf").and_then(Value::as_array) {
            return values
                .iter()
                .map(|v| self.type_expr(v))
                .collect::<Vec<_>>()
                .join(" & ");
        }
        if let Some(type_value) = map.get("type") {
            if let Some(types) = type_value.as_array() {
                return union_ts(
                    types
                        .iter()
                        .map(|t| self.type_for_json_type(t, map))
                        .collect(),
                );
            }
            return self.type_for_json_type(type_value, map);
        }
        if map.contains_key("properties") {
            return self.object_body(&Value::Object(map.clone()), 0);
        }
        "unknown".to_string()
    }

    fn type_for_json_type(&self, type_value: &Value, map: &Map<String, Value>) -> String {
        match type_value.as_str() {
            Some("string") => "string".to_string(),
            Some("integer" | "number") => "number".to_string(),
            Some("boolean") => "boolean".to_string(),
            Some("null") => "null".to_string(),
            Some("array") => {
                let item = map
                    .get("items")
                    .map(|v| self.type_expr(v))
                    .unwrap_or_else(|| "unknown".to_string());
                format!("Array<{item}>")
            }
            Some("object") => self.object_body(&Value::Object(map.clone()), 0),
            _ => "unknown".to_string(),
        }
    }

    fn object_body(&self, schema: &Value, indent: usize) -> String {
        let Some(map) = schema.as_object() else {
            return "Record<string, unknown>".to_string();
        };
        let properties = map.get("properties").and_then(Value::as_object);
        let additional = map.get("additionalProperties");
        if properties.is_none() {
            return match additional {
                Some(Value::Bool(false)) => "Record<string, never>".to_string(),
                Some(Value::Object(_)) | Some(Value::Bool(true)) => format!(
                    "Record<string, {}>",
                    additional
                        .map(|v| self.type_expr(v))
                        .unwrap_or_else(|| "unknown".to_string())
                ),
                _ => "Record<string, unknown>".to_string(),
            };
        }

        let required = map
            .get("required")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        let mut out = String::from("{\n");
        let prop_indent = " ".repeat(indent + 2);
        for (name, prop_schema) in properties.unwrap() {
            if let Some(description) = prop_schema.get("description").and_then(Value::as_str) {
                for line in doc_comment(description).lines() {
                    out.push_str(&prop_indent);
                    out.push_str(line);
                    out.push('\n');
                }
            }
            let optional = if required.contains(name.as_str()) {
                ""
            } else {
                "?"
            };
            out.push_str(&format!(
                "{}{}{}: {};\n",
                prop_indent,
                json_string(name),
                optional,
                self.type_expr(prop_schema)
            ));
        }
        if let Some(Value::Object(_)) = additional {
            out.push_str(&format!(
                "{}[key: string]: {};\n",
                prop_indent,
                self.type_expr(additional.unwrap())
            ));
        }
        out.push_str(&" ".repeat(indent));
        out.push('}');
        out
    }

    fn ref_name(&self, reference: &str) -> String {
        let name = reference
            .strip_prefix("#/$defs/")
            .or_else(|| reference.strip_prefix("#/definitions/"))
            .or_else(|| reference.strip_prefix("#/components/schemas/"))
            .unwrap_or(reference)
            .split('/')
            .last()
            .unwrap_or(reference)
            .to_pascal_case();
        if self.registry.schemas.contains_key(&name) {
            name
        } else {
            sanitize_type_name(&name)
        }
    }
}

fn write_rust(registry: &TypeRegistry, methods: &[MethodSpec], events: &[EventSpec]) -> Result<()> {
    let mut settings = TypeSpaceSettings::default();
    settings
        .with_derive("PartialEq".to_string())
        .with_attr("#[allow(clippy::large_enum_variant)]".to_string());
    let mut type_space = TypeSpace::new(&settings);
    let type_defs = registry
        .schemas
        .iter()
        .map(|(name, schema)| {
            Ok((
                name.clone(),
                schema_for_shared_rust_types(name, schema, registry)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    type_space
        .add_ref_types(type_defs)
        .map_err(|e| anyhow!("typify failed for shared schema registry: {e}"))?;

    let mut types = generated_rust_header("Rust types generated from docs/spec/v1/schemas/.");
    types.push_str("#![allow(clippy::large_enum_variant)]\n#![allow(clippy::derive_partial_eq_without_eq)]\n#![allow(clippy::module_name_repetitions)]\n\n");
    types.push_str(&format_rust_tokens(type_space.to_stream().to_string())?);
    fs::write(Path::new(RUST_DIR).join("types.rs"), types)?;
    fs::write(
        Path::new(RUST_DIR).join("methods.rs"),
        emit_rust_methods(methods)?,
    )?;
    fs::write(
        Path::new(RUST_DIR).join("events.rs"),
        emit_rust_events(events)?,
    )?;
    fs::write(
        Path::new(RUST_DIR).join("lib.rs"),
        "pub mod client;\npub mod events;\npub mod methods;\npub mod types;\n\npub use client::{PluroraClient, PluroraTransport};\npub use events::*;\npub use types::*;\n",
    )?;
    rustfmt_generated_sources(&["types.rs", "methods.rs", "events.rs", "lib.rs"])?;
    Ok(())
}

fn rustfmt_generated_sources(files: &[&str]) -> Result<()> {
    let rustfmt = std::env::var_os("RUSTFMT").unwrap_or_else(|| "rustfmt".into());
    let paths = files
        .iter()
        .map(|file| Path::new(RUST_DIR).join(file))
        .collect::<Vec<_>>();
    let status = Command::new(&rustfmt)
        .arg("--edition=2021")
        .arg("--config-path")
        .arg("rustfmt.toml")
        .args(&paths)
        .status()
        .with_context(|| {
            format!(
                "running {} on generated Rust SDK",
                rustfmt.to_string_lossy()
            )
        })?;
    if !status.success() {
        anyhow::bail!("rustfmt failed for generated Rust SDK with status {status}");
    }
    Ok(())
}

fn schema_for_shared_rust_types(
    name: &str,
    schema: &Value,
    registry: &TypeRegistry,
) -> Result<Schema> {
    let aliases = definition_aliases(schema, registry)?;
    let mut value = schema.clone();
    rewrite_local_definition_refs(&mut value, &aliases, registry)?;
    strip_definition_blocks(&mut value);
    strip_nested_schema_titles(&mut value, true);
    normalize_root_discriminators_for_typify(&mut value);
    if let Some(map) = value.as_object_mut() {
        map.insert("title".to_string(), Value::String(name.to_string()));
    }
    normalize_for_typify(&mut value);
    serde_json::from_value(value).with_context(|| format!("building shared schema for {name}"))
}

fn definition_aliases(schema: &Value, registry: &TypeRegistry) -> Result<BTreeMap<String, String>> {
    let mut aliases = BTreeMap::new();
    collect_definition_aliases(schema, registry, &mut aliases)?;
    Ok(aliases)
}

fn collect_definition_aliases(
    value: &Value,
    registry: &TypeRegistry,
    aliases: &mut BTreeMap<String, String>,
) -> Result<()> {
    match value {
        Value::Object(map) => {
            for key in ["$defs", "definitions"] {
                if let Some(Value::Object(definitions)) = map.get(key) {
                    for (local_name, schema) in definitions {
                        let preferred =
                            schema_title(schema).unwrap_or_else(|| local_name.to_pascal_case());
                        let canonical = registered_schema_name(registry, &preferred, schema)?;
                        if let Some(existing) =
                            aliases.insert(local_name.clone(), canonical.clone())
                        {
                            if existing != canonical {
                                anyhow::bail!(
                                    "definition {local_name} resolves to both {existing} and {canonical}"
                                );
                            }
                        }
                        collect_definition_aliases(schema, registry, aliases)?;
                    }
                }
            }
            for (key, child) in map {
                if key != "$defs" && key != "definitions" {
                    collect_definition_aliases(child, registry, aliases)?;
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_definition_aliases(value, registry, aliases)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn registered_schema_name(
    registry: &TypeRegistry,
    preferred: &str,
    schema: &Value,
) -> Result<String> {
    let base = sanitize_type_name(preferred);
    let mut candidate = base.clone();
    let mut index = 2;
    loop {
        match registry.schemas.get(&candidate) {
            Some(existing) if schemas_equivalent(existing, schema) => return Ok(candidate),
            Some(_) => {
                candidate = format!("{base}{index}");
                index += 1;
            }
            None => anyhow::bail!(
                "definition {preferred} has no canonical entry in the shared type registry"
            ),
        }
    }
}

fn rewrite_local_definition_refs(
    value: &mut Value,
    aliases: &BTreeMap<String, String>,
    registry: &TypeRegistry,
) -> Result<()> {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                let local = reference
                    .strip_prefix("#/$defs/")
                    .or_else(|| reference.strip_prefix("#/definitions/"));
                if let Some(local) = local {
                    let (local_name, suffix) = local
                        .split_once('/')
                        .map_or((local, ""), |(name, suffix)| (name, suffix));
                    let canonical = aliases
                        .get(local_name)
                        .cloned()
                        .or_else(|| {
                            let candidate = sanitize_type_name(local_name);
                            registry
                                .schemas
                                .contains_key(&candidate)
                                .then_some(candidate)
                        })
                        .ok_or_else(|| anyhow!("unresolved local schema reference {reference}"))?;
                    *reference = if suffix.is_empty() {
                        format!("#/definitions/{canonical}")
                    } else {
                        format!("#/definitions/{canonical}/{suffix}")
                    };
                }
            }
            for child in map.values_mut() {
                rewrite_local_definition_refs(child, aliases, registry)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                rewrite_local_definition_refs(value, aliases, registry)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn strip_definition_blocks(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("$defs");
            map.remove("definitions");
            for child in map.values_mut() {
                strip_definition_blocks(child);
            }
        }
        Value::Array(values) => {
            for value in values {
                strip_definition_blocks(value);
            }
        }
        _ => {}
    }
}

fn strip_nested_schema_titles(value: &mut Value, is_root: bool) {
    match value {
        Value::Object(map) => {
            if !is_root {
                map.remove("title");
                map.remove("$id");
                map.remove("$schema");
            }
            for (keyword, child) in map.iter_mut() {
                if matches!(
                    keyword.as_str(),
                    "properties" | "patternProperties" | "dependentSchemas"
                ) {
                    if let Value::Object(schemas) = child {
                        for schema in schemas.values_mut() {
                            strip_nested_schema_titles(schema, false);
                        }
                    }
                } else {
                    strip_nested_schema_titles(child, false);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                strip_nested_schema_titles(value, false);
            }
        }
        _ => {}
    }
}

fn normalize_for_typify(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(defs) = map.remove("$defs") {
                map.insert("definitions".to_string(), defs);
            }
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                if let Some(rest) = reference.strip_prefix("#/$defs/") {
                    *reference = format!("#/definitions/{rest}");
                }
            }
            for value in map.values_mut() {
                normalize_for_typify(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                normalize_for_typify(value);
            }
        }
        _ => {}
    }
}

fn emit_rust_methods(methods: &[MethodSpec]) -> Result<String> {
    let header =
        generated_rust_header("Rust client methods generated from docs/spec/v1/schemas/methods/.");
    let mut out = String::new();
    out.push_str(
        "use anyhow::Result;\n\nuse crate::client::PluroraClient;\nuse crate::types::*;\n\n",
    );
    out.push_str("impl PluroraClient {\n");
    for method in methods {
        out.push_str(&format!(
            "    pub async fn {}(&self, params: {}) -> Result<{}> {{\n        let raw = self.invoke(\"{}\", serde_json::to_value(params)?).await?;\n        Ok(serde_json::from_value(raw)?)\n    }}\n\n",
            method.function_rs, method.params_rs, method.result_rs, method.id
        ));
    }
    out.push_str("}\n");
    Ok(format!("{}{}", header, format_rust(out)?))
}

fn emit_rust_events(events: &[EventSpec]) -> Result<String> {
    let header = generated_rust_header(
        "Rust event payload aliases generated from docs/spec/v1/schemas/events/.",
    );
    let mut out = String::new();
    out.push_str("use crate::types::*;\n\n");
    for event in events {
        out.push_str(&format!(
            "pub const {}: &str = \"{}\";\n",
            event.kind.to_shouty_snake_case(),
            event.kind
        ));
        if event.payload_alias != event.payload_rs {
            out.push_str(&format!(
                "pub type {} = {};\n",
                event.payload_alias, event.payload_rs
            ));
        }
        out.push('\n');
    }
    Ok(format!("{}{}", header, format_rust(out)?))
}

fn write_openapi(methods: &[MethodSpec], registry: &TypeRegistry) -> Result<()> {
    let mut paths = Map::new();
    for method in methods {
        let operation_id = method.function_ts.clone();
        let mut request_schema = json_rpc_request_schema(&method.id, &method.params_schema);
        let mut response_schema = json_rpc_response_schema(&method.result_schema);
        let implementation_status = method
            .contract
            .get("implementation_status")
            .cloned()
            .ok_or_else(|| anyhow!("method {} contract has no implementation_status", method.id))?;
        rewrite_local_definition_refs(&mut request_schema, &method.openapi_aliases, registry)?;
        rewrite_local_definition_refs(&mut response_schema, &method.openapi_aliases, registry)?;
        strip_definition_blocks(&mut request_schema);
        strip_definition_blocks(&mut response_schema);
        normalize_openapi_refs(&mut request_schema);
        normalize_openapi_refs(&mut response_schema);
        paths.insert(
            format!("/rpc/{}", method.id),
            json!({
                "post": {
                    "operationId": operation_id,
                    "summary": format!("Invoke {}", method.id),
                    "x-plurora-contract": method.contract,
                    "x-plurora-implementation-status": implementation_status,
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": request_schema
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "JSON-RPC response envelope",
                            "content": {
                                "application/json": {
                                    "schema": response_schema
                                }
                            }
                        }
                    }
                }
            }),
        );
    }

    let mut components = Map::new();
    for (name, schema) in &registry.schemas {
        let aliases = definition_aliases(schema, registry)?;
        let mut component = schema.clone();
        rewrite_local_definition_refs(&mut component, &aliases, registry)?;
        strip_definition_blocks(&mut component);
        normalize_openapi_refs(&mut component);
        components.insert(name.clone(), component);
    }

    let openapi = json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Plurora Public Contract API",
            "version": "1.0.0",
            "description": "Generated from docs/spec/v1/schemas/. JSON-RPC methods are exposed as typed /rpc/{method} operations for code generators."
        },
        "paths": paths,
        "components": { "schemas": components }
    });
    validate_local_document_refs(&openapi, &openapi, "generated OpenAPI")?;
    fs::write(OPENAPI, serde_yaml::to_string(&openapi)?)?;
    Ok(())
}

fn validate_local_document_refs(value: &Value, root: &Value, label: &str) -> Result<()> {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                if let Some(pointer) = reference.strip_prefix('#') {
                    if !pointer.is_empty() && root.pointer(pointer).is_none() {
                        anyhow::bail!("{label} contains unresolved local reference {reference}");
                    }
                }
            }
            for child in map.values() {
                validate_local_document_refs(child, root, label)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                validate_local_document_refs(child, root, label)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn normalize_openapi_refs(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(defs) = map.remove("definitions") {
                map.entry("$defs".to_string()).or_insert(defs);
            }
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                if let Some(rest) = reference.strip_prefix("#/definitions/") {
                    *reference = format!("#/components/schemas/{rest}");
                } else if let Some(rest) = reference.strip_prefix("#/$defs/") {
                    *reference = format!("#/components/schemas/{rest}");
                }
            }
            for value in map.values_mut() {
                normalize_openapi_refs(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                normalize_openapi_refs(value);
            }
        }
        _ => {}
    }
}

fn json_rpc_request_schema(method: &str, params: &Value) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["jsonrpc", "id", "method", "params"],
        "properties": {
            "jsonrpc": { "const": "2.0" },
            "id": { "oneOf": [{"type":"string"}, {"type":"integer"}, {"type":"null"}] },
            "method": { "const": method },
            "params": params,
            "contract": { "$ref": "#/components/schemas/ContractSelection" }
        }
    })
}

fn json_rpc_response_schema(result: &Value) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["jsonrpc", "id"],
        "properties": {
            "jsonrpc": { "const": "2.0" },
            "id": { "oneOf": [{"type":"string"}, {"type":"integer"}, {"type":"null"}] },
            "result": result,
            "error": {
                "type": "object",
                "required": ["code", "message"],
                "properties": {
                    "code": { "type": "integer" },
                    "message": { "type": "string" },
                    "data": true
                }
            }
        }
    })
}

fn schema_title(schema: &Value) -> Option<String> {
    schema
        .get("title")
        .and_then(Value::as_str)
        .map(sanitize_type_name)
}

fn file_stem_type_name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("GeneratedType")
        .trim_end_matches(".schema.json")
        .to_pascal_case()
}

fn sanitize_type_name(name: &str) -> String {
    let pascal = name.replace(['.', '/', '-'], "_").to_pascal_case();
    if pascal.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("T{pascal}")
    } else {
        pascal
    }
}

fn method_base_name(id: &str) -> String {
    sanitize_type_name(id)
}

fn event_base_name(kind: &str) -> String {
    sanitize_type_name(kind)
}

fn method_function_ts(id: &str) -> String {
    id.replace(['.', '-', '/'], "_").to_lower_camel_case()
}

fn method_function_rs(id: &str) -> String {
    id.replace(['.', '-', '/'], "_").to_snake_case()
}

fn schemas_equivalent(a: &Value, b: &Value) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    strip_metadata(&mut a);
    strip_metadata(&mut b);
    a == b
}

fn strip_metadata(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("title");
            map.remove("description");
            map.remove("$schema");
            map.remove("$id");
            map.remove("$defs");
            map.remove("definitions");
            for value in map.values_mut() {
                strip_metadata(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                strip_metadata(value);
            }
        }
        _ => {}
    }
}

fn union_ts(types: Vec<String>) -> String {
    let unique = types.into_iter().collect::<BTreeSet<_>>();
    if unique.is_empty() {
        "unknown".to_string()
    } else {
        unique.into_iter().collect::<Vec<_>>().join(" | ")
    }
}

fn literal_ts(value: &Value) -> String {
    match value {
        Value::String(s) => json_string(s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => "unknown".to_string(),
    }
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serializes")
}

fn doc_comment(text: &str) -> String {
    let mut out = String::new();
    out.push_str("/**\n");
    for line in text.lines() {
        let line = line.trim();
        out.push_str(if line.is_empty() { " *" } else { " * " });
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(" */\n");
    out
}

fn generated_header(message: &str) -> String {
    format!("// @generated by cargo run -p plurora-cli --bin generate-sdks\n// {message}\n")
}

fn generated_rust_header(message: &str) -> String {
    format!("// @generated by cargo run -p plurora-cli --bin generate-sdks\n// {message}\n")
}

fn format_rust_tokens(tokens: String) -> Result<String> {
    format_rust(tokens)
}

fn format_rust(source: String) -> Result<String> {
    let file = syn::parse_file(&source).with_context(|| {
        let preview: String = source.chars().take(4000).collect();
        format!("generated Rust did not parse. Preview:\n{preview}")
    })?;
    Ok(prettyplease::unparse(&file))
}

trait ShoutySnake {
    fn to_shouty_snake_case(&self) -> String;
}

impl ShoutySnake for str {
    fn to_shouty_snake_case(&self) -> String {
        self.replace(['/', '.', '-'], "_")
            .to_snake_case()
            .to_uppercase()
    }
}

fn normalize_root_discriminators_for_typify(value: &mut Value) {
    let Some(properties) = value
        .as_object_mut()
        .and_then(|root| root.get_mut("properties"))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    for field in ["schema", "schema_version"] {
        let Some(schema) = properties.get_mut(field).and_then(Value::as_object_mut) else {
            continue;
        };
        if let Some(constant) = schema.remove("const") {
            schema.entry("enum").or_insert_with(|| json!([constant]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_title_metadata_is_removed_without_deleting_a_title_property() {
        let mut schema = json!({
            "title": "Example",
            "type": "object",
            "required": ["title"],
            "properties": {
                "title": {
                    "title": "Human-readable title",
                    "type": "string"
                }
            }
        });

        strip_nested_schema_titles(&mut schema, true);

        assert_eq!(
            schema.pointer("/properties/title/type"),
            Some(&json!("string"))
        );
        assert!(schema.pointer("/properties/title/title").is_none());
    }

    #[test]
    fn root_discriminators_become_singleton_enums_for_typify() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "schema": {"type": "string", "const": "example.v1"},
                "schema_version": {"type": "integer", "const": 1},
                "nested": {
                    "type": "object",
                    "properties": {
                        "schema": {"type": "string", "const": "nested.v1"}
                    }
                }
            }
        });

        normalize_root_discriminators_for_typify(&mut schema);

        assert_eq!(
            schema.pointer("/properties/schema/enum"),
            Some(&json!(["example.v1"]))
        );
        assert_eq!(
            schema.pointer("/properties/schema_version/enum"),
            Some(&json!([1]))
        );
        assert!(schema.pointer("/properties/schema/const").is_none());
        assert_eq!(
            schema.pointer("/properties/nested/properties/schema/const"),
            Some(&json!("nested.v1"))
        );
    }
}
