use std::collections::{BTreeMap, BTreeSet};

use plurora_core::*;
use plurora_runtime::*;
use schemars::{schema_for, JsonSchema};
use serde_json::{Map, Value};

use super::SCHEMA;

#[derive(JsonSchema)]
pub(crate) struct EmptyParams {}

#[derive(JsonSchema)]
pub(crate) struct ErrorShape {
    code: String,
    message: String,
    details: Value,
}

#[derive(JsonSchema)]
pub(crate) struct SessionCloseParams {
    session_id: String,
}
#[derive(JsonSchema)]
pub(crate) struct SessionGetParams {
    session_id: String,
}
#[derive(JsonSchema)]
pub(crate) struct SessionForkParams {
    parent_session_id: String,
    forked_from_sequence: u64,
    #[schemars(schema_with = "json_value_schema")]
    metadata: Value,
}
#[derive(JsonSchema)]
pub(crate) struct SessionBranchListParams {
    session_id: String,
}
#[derive(JsonSchema)]
pub(crate) struct PackageIdParams {
    package_id: String,
}
#[derive(JsonSchema)]
pub(crate) struct ProjectIdParams {
    project_id: plurora_core::project::ProjectId,
}
#[derive(JsonSchema)]
pub(crate) struct ProjectListParams {
    filter_state: Option<plurora_core::project::ProjectState>,
}
#[derive(JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StorageMeasurementStateSchema {
    Measured,
    Unknown,
}
#[derive(JsonSchema)]
pub(crate) struct ProjectStorageSummarySchema {
    data_bytes: Option<u64>,
    cache_bytes: Option<u64>,
    bundle_bytes: Option<u64>,
    log_bytes: Option<u64>,
    total_bytes: Option<u64>,
    measured_at: Option<chrono::DateTime<chrono::Utc>>,
    measurement_state: StorageMeasurementStateSchema,
}
#[derive(JsonSchema)]
pub(crate) struct ProjectListItemSchema {
    id: plurora_core::project::ProjectId,
    title: String,
    description: String,
    #[serde(rename = "type")]
    project_type: plurora_core::project::ProjectType,
    state: plurora_core::project::ProjectState,
    icon: Option<String>,
    entry_surface_id: Option<String>,
    storage_summary: Option<ProjectStorageSummarySchema>,
    #[schemars(schema_with = "string_schema")]
    running_session_id: Option<String>,
}
#[derive(JsonSchema)]
pub(crate) struct ProjectListResultSchema {
    projects: Vec<ProjectListItemSchema>,
}
#[derive(JsonSchema)]
pub(crate) struct ProjectStartResult {
    project_id: plurora_core::project::ProjectId,
    previous_state: plurora_core::project::ProjectState,
    new_state: plurora_core::project::ProjectState,
    session_id: String,
    already_running: bool,
}
#[derive(JsonSchema)]
pub(crate) struct ProjectStopResult {
    project_id: plurora_core::project::ProjectId,
    previous_state: plurora_core::project::ProjectState,
    new_state: plurora_core::project::ProjectState,
    #[schemars(schema_with = "string_schema")]
    session_id: Option<String>,
}
#[derive(JsonSchema)]
pub(crate) struct ProjectStatusResult {
    project_id: plurora_core::project::ProjectId,
    state: plurora_core::project::ProjectState,
    sessions_count: usize,
    secrets_count: usize,
    #[schemars(schema_with = "string_schema")]
    running_session_id: Option<String>,
    storage_summary: Option<ProjectStorageSummarySchema>,
}
#[derive(JsonSchema)]
pub(crate) struct TargetIdParams {
    target_id: String,
}
#[derive(JsonSchema)]
pub(crate) struct ExecIdParams {
    exec_id: String,
}
#[derive(JsonSchema)]
pub(crate) struct PortLeaseIdParams {
    lease_id: String,
}
#[derive(JsonSchema)]
pub(crate) struct ProxyRouteIdParams {
    route_id: String,
}
#[derive(JsonSchema)]
pub(crate) struct ProjectLifecyclePayloadSchema {
    project_id: plurora_core::project::ProjectId,
    title: String,
    #[serde(rename = "type")]
    project_type: plurora_core::project::ProjectType,
    previous_state: Option<plurora_core::project::ProjectState>,
    new_state: plurora_core::project::ProjectState,
}
#[derive(JsonSchema)]
pub(crate) struct AssetGetParams {
    asset_id: String,
}
#[derive(JsonSchema)]
pub(crate) struct ProjectionIdParams {
    projection_id: String,
}
#[derive(JsonSchema)]
pub(crate) struct ProposalIdParams {
    proposal_id: String,
}
#[derive(JsonSchema)]
pub(crate) struct ProposalDecisionParams {
    proposal_id: String,
    reason: Option<String>,
}
#[derive(JsonSchema)]
pub(crate) struct SurfaceListParams {
    slot: Option<String>,
}
#[derive(JsonSchema)]
pub(crate) struct SurfaceDescribeParams {
    surface_id: String,
}
#[derive(JsonSchema)]
pub(crate) struct SurfaceResolveBundleParams {
    surface_id: String,
}
#[derive(JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SurfaceBundleSourceSchema {
    InstalledProject,
    DevPath,
}
#[derive(JsonSchema)]
pub(crate) struct SurfaceResolveBundleResult {
    surface_id: String,
    bundle_url: String,
    export_name: String,
    stylesheets: Vec<String>,
    wrapper_class: Option<String>,
    project_id: Option<String>,
    source: SurfaceBundleSourceSchema,
}
#[derive(JsonSchema)]
pub(crate) struct PermissionGrantParams {
    principal: ProtocolPrincipal,
    permission: String,
    scope: Option<String>,
    reason: Option<String>,
}
#[derive(JsonSchema)]
pub(crate) struct PermissionRevokeParams {
    grant_id: String,
}
#[derive(JsonSchema)]
pub(crate) struct PermissionListParams {
    principal: Option<ProtocolPrincipal>,
}
#[derive(JsonSchema)]
pub(crate) struct OutboundAuditParams {
    package_id: String,
}
#[derive(JsonSchema)]
pub(crate) struct AuditPackageParams {
    package_id: PackageId,
    since: Option<chrono::DateTime<chrono::Utc>>,
    until: Option<chrono::DateTime<chrono::Utc>>,
}
#[derive(JsonSchema)]
pub(crate) struct CapAttenuateParams {
    parent_handle: CapHandleId,
    #[schemars(schema_with = "json_value_schema")]
    constraints: Value,
}
#[derive(JsonSchema)]
pub(crate) struct CapRevokeParams {
    handle: CapHandleId,
}
#[derive(JsonSchema)]
pub(crate) struct CapListForParams {
    package_id: PackageId,
}
#[derive(JsonSchema)]
pub(crate) struct CapHandleResult {
    handle: CapHandle,
}
#[derive(JsonSchema)]
pub(crate) struct CapHandlesResult {
    handles: Vec<CapHandle>,
}
#[derive(JsonSchema)]
pub(crate) struct CapabilityStreamParams {
    capability_id: String,
    provider_package_id: Option<String>,
    #[schemars(schema_with = "json_value_schema")]
    input: Value,
}
#[derive(JsonSchema)]
pub(crate) struct CapabilityCancelParams {
    stream_id: Option<String>,
    invocation_id: Option<String>,
    session_id: Option<String>,
}
#[derive(JsonSchema)]
pub(crate) struct OutboundExecuteParams {
    package_id: Option<String>,
    capability_id: String,
    destination_host: String,
    method: String,
    path: Option<String>,
    purpose: Option<String>,
    secret_refs: Vec<String>,
    timeout_ms: Option<u64>,
    #[schemars(schema_with = "json_value_schema")]
    metadata: Value,
    #[schemars(schema_with = "optional_json_value_schema")]
    body_shape: Option<Value>,
    secret_headers: Vec<OutboundSecretHeaderSpec>,
    static_headers: Vec<OutboundStaticHeader>,
}
#[derive(JsonSchema)]
pub(crate) struct OutboundStreamParams {
    package_id: Option<String>,
    capability_id: String,
    destination_host: String,
    method: String,
    path: Option<String>,
    purpose: Option<String>,
    secret_refs: Vec<String>,
    timeout_ms: Option<u64>,
    stream_format: Option<StreamFormat>,
    max_frame_bytes: Option<u64>,
    max_total_bytes: Option<u64>,
    max_duration_ms: Option<u64>,
    #[schemars(schema_with = "json_value_schema")]
    metadata: Value,
    #[schemars(schema_with = "optional_json_value_schema")]
    body_shape: Option<Value>,
    secret_headers: Vec<OutboundSecretHeaderSpec>,
    static_headers: Vec<OutboundStaticHeader>,
}
#[derive(JsonSchema)]
pub(crate) struct OutboundWebSocketSendParams {
    connection_id: String,
    kind: Option<FrameKind>,
    data: Option<String>,
    bytes: Option<Vec<u8>>,
    code: Option<u16>,
    reason: Option<String>,
}

pub(crate) fn json_value_schema(
    _gen: &mut schemars::r#gen::SchemaGenerator,
) -> schemars::schema::Schema {
    schemars::schema::Schema::Bool(true)
}
pub(crate) fn optional_json_value_schema(
    _gen: &mut schemars::r#gen::SchemaGenerator,
) -> schemars::schema::Schema {
    schemars::schema::Schema::Bool(true)
}

pub(crate) fn string_schema(
    _gen: &mut schemars::r#gen::SchemaGenerator,
) -> schemars::schema::Schema {
    use schemars::schema::{InstanceType, Schema, SchemaObject, SingleOrVec};

    let mut schema = SchemaObject::default();
    schema.instance_type = Some(SingleOrVec::Single(Box::new(InstanceType::String)));
    Schema::Object(schema)
}

pub(crate) fn schema_value<T: JsonSchema>() -> Value {
    let schema = schema_for!(T);
    let mut value = serde_json::to_value(schema).expect("schema serializes");
    normalize_schema(&mut value);
    value
}

pub(crate) fn normalize_schema(value: &mut Value) {
    normalize_schema_with_key(None, value);
    hoist_nested_definitions(value);
}

fn normalize_schema_with_key(parent_key: Option<&str>, value: &mut Value) {
    match value {
        Value::Object(map) => {
            if map.get("$schema").and_then(Value::as_str)
                == Some("http://json-schema.org/draft-07/schema#")
            {
                map.insert("$schema".to_string(), Value::String(SCHEMA.to_string()));
            }
            if let Some(defs) = map.remove("definitions") {
                map.insert("$defs".to_string(), defs);
            }
            if matches!(parent_key, Some("created_at" | "timestamp"))
                && map.get("format").and_then(Value::as_str) == Some("date-time")
            {
                map.remove("default");
            }
            for (key, v) in map.iter_mut() {
                normalize_schema_with_key(Some(key.as_str()), v);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                normalize_schema_with_key(parent_key, v);
            }
        }
        Value::String(reference) if parent_key == Some("$ref") => {
            if let Some(rest) = reference.strip_prefix("#/definitions/") {
                *reference = format!("#/$defs/{rest}");
            }
        }
        _ => {}
    }
}

fn hoist_nested_definitions(value: &mut Value) {
    let Value::Object(root) = value else {
        return;
    };
    let root_entries = root
        .remove("$defs")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let mut state = DefinitionHoistState::default();

    for (name, schema) in &root_entries {
        state.reserve_exact(name, schema, &root_entries);
    }
    for (name, mut schema) in root_entries {
        process_definition_scope(&mut schema, &BTreeMap::new(), &mut state);
        state.definitions.insert(name.clone(), schema);
        state.materialized.insert(name);
    }
    for child in root.values_mut() {
        process_definition_scope(child, &BTreeMap::new(), &mut state);
    }
    if !state.definitions.is_empty() {
        root.insert("$defs".to_string(), Value::Object(state.definitions));
    }
}

#[derive(Default)]
struct DefinitionHoistState {
    definitions: Map<String, Value>,
    origins: BTreeMap<String, Value>,
    materialized: BTreeSet<String>,
}

impl DefinitionHoistState {
    fn reserve_exact(&mut self, name: &str, schema: &Value, scope: &Map<String, Value>) {
        self.origins
            .insert(name.to_string(), definition_origin(schema, scope));
    }

    fn reserve(&mut self, preferred: &str, schema: &Value, scope: &Map<String, Value>) -> String {
        let origin = definition_origin(schema, scope);
        let mut candidate = preferred.to_string();
        let mut suffix = 2;
        loop {
            match self.origins.get(&candidate) {
                Some(existing) if schemas_equivalent(existing, &origin) => return candidate,
                Some(_) => {
                    candidate = format!("{preferred}{suffix}");
                    suffix += 1;
                }
                None => {
                    self.origins.insert(candidate.clone(), origin.clone());
                    return candidate;
                }
            }
        }
    }
}

fn process_definition_scope(
    value: &mut Value,
    inherited: &BTreeMap<String, String>,
    state: &mut DefinitionHoistState,
) {
    match value {
        Value::Object(map) => {
            let local_entries = map
                .remove("$defs")
                .and_then(|value| value.as_object().cloned())
                .unwrap_or_default();
            let mut scope = inherited.clone();
            let mut reservations = Vec::new();
            for (local_name, schema) in &local_entries {
                let global_name = state.reserve(local_name, schema, &local_entries);
                scope.insert(local_name.clone(), global_name.clone());
                reservations.push((local_name.clone(), global_name));
            }
            for ((_, mut schema), (_, global_name)) in
                local_entries.into_iter().zip(reservations.iter())
            {
                if !state.materialized.contains(global_name) {
                    process_definition_scope(&mut schema, &scope, state);
                    state.definitions.insert(global_name.clone(), schema);
                    state.materialized.insert(global_name.clone());
                }
            }
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                rewrite_scoped_reference(reference, &scope);
            }
            for child in map.values_mut() {
                process_definition_scope(child, &scope, state);
            }
        }
        Value::Array(values) => {
            for child in values {
                process_definition_scope(child, inherited, state);
            }
        }
        _ => {}
    }
}

fn definition_origin(schema: &Value, scope: &Map<String, Value>) -> Value {
    let mut dependencies = Map::new();
    let mut visited = BTreeSet::new();
    collect_definition_dependencies(schema, scope, &mut visited, &mut dependencies);
    let mut origin = Value::Object(Map::from_iter([
        ("schema".to_string(), schema.clone()),
        ("dependencies".to_string(), Value::Object(dependencies)),
    ]));
    strip_schema_metadata(&mut origin);
    origin
}

fn collect_definition_dependencies(
    value: &Value,
    scope: &Map<String, Value>,
    visited: &mut BTreeSet<String>,
    dependencies: &mut Map<String, Value>,
) {
    let mut referenced = BTreeSet::new();
    collect_scoped_reference_names(value, &mut referenced);
    for name in referenced {
        if !visited.insert(name.clone()) {
            continue;
        }
        if let Some(schema) = scope.get(&name) {
            dependencies.insert(name, schema.clone());
            collect_definition_dependencies(schema, scope, visited, dependencies);
        }
    }
}

fn collect_scoped_reference_names(value: &Value, names: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                if let Some(rest) = reference.strip_prefix("#/$defs/") {
                    if let Some(name) = rest.split('/').next() {
                        names.insert(name.to_string());
                    }
                }
            }
            for child in map.values() {
                collect_scoped_reference_names(child, names);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_scoped_reference_names(child, names);
            }
        }
        _ => {}
    }
}

fn rewrite_scoped_reference(reference: &mut String, scope: &BTreeMap<String, String>) {
    let Some(rest) = reference.strip_prefix("#/$defs/") else {
        return;
    };
    let (local_name, suffix) = rest
        .split_once('/')
        .map_or((rest, None), |(name, suffix)| (name, Some(suffix)));
    let Some(global_name) = scope.get(local_name) else {
        return;
    };
    *reference = match suffix {
        Some(suffix) => format!("#/$defs/{global_name}/{suffix}"),
        None => format!("#/$defs/{global_name}"),
    };
}

fn schemas_equivalent(left: &Value, right: &Value) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    strip_schema_metadata(&mut left);
    strip_schema_metadata(&mut right);
    left == right
}

fn strip_schema_metadata(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("title");
            map.remove("description");
            map.remove("$schema");
            map.remove("$id");
            for child in map.values_mut() {
                strip_schema_metadata(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                strip_schema_metadata(child);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::normalize_schema;

    #[test]
    fn normalize_hoists_nested_definitions_and_rebases_refs() {
        let mut schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "$defs": {
                "Result": {
                    "type": "object",
                    "definitions": {
                        "Artifact": { "type": "string" }
                    },
                    "properties": {
                        "artifact": { "$ref": "#/definitions/Artifact" }
                    }
                }
            },
            "properties": {
                "result": { "$ref": "#/$defs/Result" }
            }
        });

        normalize_schema(&mut schema);

        assert_eq!(
            schema.pointer("/$defs/Result/properties/artifact/$ref"),
            Some(&json!("#/$defs/Artifact"))
        );
        assert_eq!(
            schema.pointer("/$defs/Artifact/type"),
            Some(&json!("string"))
        );
        assert!(schema.pointer("/$defs/Result/$defs").is_none());
    }

    #[test]
    fn normalize_keeps_contextually_different_definition_closures_distinct() {
        let scoped = |bar_type: &str| {
            json!({
                "type": "object",
                "$defs": {
                    "Foo": { "$ref": "#/$defs/Bar" },
                    "Bar": { "type": bar_type }
                },
                "properties": { "foo": { "$ref": "#/$defs/Foo" } }
            })
        };
        let mut schema = json!({
            "$defs": {
                "ResultA": scoped("string"),
                "ResultB": scoped("integer")
            }
        });

        normalize_schema(&mut schema);

        let left = schema
            .pointer("/$defs/ResultA/properties/foo/$ref")
            .and_then(|value| value.as_str())
            .unwrap();
        let right = schema
            .pointer("/$defs/ResultB/properties/foo/$ref")
            .and_then(|value| value.as_str())
            .unwrap();
        assert_ne!(left, right);
        assert!(schema.pointer("/$defs/Foo").is_some());
        assert!(schema.pointer("/$defs/Foo2").is_some());
    }
}
