use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::assembly::{
    MAX_ASSEMBLY_BINDINGS, MAX_ASSEMBLY_NODES, MAX_EXPOSED_PORTS, MAX_STATE_SLOTS,
};
use crate::canonical::validate_portable_model;
use crate::diagnostic::{DiagnosticCode, ModelError, ModelResult};
use crate::ids::{AssemblyId, NodeId, PortId, StateSlotId, WorkId};
use crate::port::{BindingPhase, PortDescriptor, PortDirection, TransportPolicy};
use crate::state::{BackupPolicy, StatePortability, StateScope};

pub const MAX_SOURCE_DESCRIPTOR_BYTES: usize = 1024 * 1024;
pub const WORK_SOURCE_SCHEMA: &str = "plurora.work-source.v1";
pub const ASSEMBLY_SOURCE_SCHEMA: &str = "plurora.assembly-source.v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, JsonSchema)]
#[schemars(transparent)]
pub struct SourcePathRef(String);

impl SourcePathRef {
    pub fn parse(value: impl Into<String>) -> ModelResult<Self> {
        let value = value.into();
        validate_source_path_ref(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn path(&self) -> &str {
        self.0
            .split_once('#')
            .map_or(self.0.as_str(), |(path, _)| path)
    }

    pub fn fragment(&self) -> Option<&str> {
        self.0.split_once('#').map(|(_, fragment)| fragment)
    }
}

impl fmt::Display for SourcePathRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for SourcePathRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SourcePathRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

fn validate_source_path_ref(value: &str) -> ModelResult<()> {
    if value.is_empty()
        || value.contains('\0')
        || value.contains('\\')
        || value.contains("://")
        || value.starts_with('/')
        || value.starts_with('~')
        || value.to_ascii_lowercase().starts_with("file:")
    {
        return Err(invalid_source_path());
    }
    let (path, fragment) = value
        .split_once('#')
        .map_or((value, None), |(path, fragment)| (path, Some(fragment)));
    if path.is_empty()
        || path.contains('#')
        || (path.as_bytes().len() >= 2
            && path.as_bytes()[0].is_ascii_alphabetic()
            && path.as_bytes()[1] == b':')
        || path
            .trim_end_matches('/')
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return Err(invalid_source_path());
    }
    if let Some(fragment) = fragment {
        if fragment.is_empty()
            || fragment.contains('#')
            || PortId::parse(fragment.to_string()).is_err()
        {
            return Err(invalid_source_path());
        }
    }
    Ok(())
}

fn invalid_source_path() -> ModelError {
    ModelError::new(
        DiagnosticCode::RawPath,
        "source reference is not a contained portable lexical path",
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkSourceDocument {
    pub schema: String,
    pub work: WorkSourceDescriptor,
}

impl WorkSourceDocument {
    pub fn validate(&self) -> ModelResult<()> {
        if self.schema != WORK_SOURCE_SCHEMA {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "work source uses an unsupported schema",
            ));
        }
        self.work.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkSourceDescriptor {
    pub id: WorkId,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub assembly: SourcePathRef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entrypoints: Vec<WorkSourceEntrypoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<SourcePathRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rights: Option<SourcePathRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparency: Option<SourcePathRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operational_intent: Option<SourcePathRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl WorkSourceDescriptor {
    fn validate(&self) -> ModelResult<()> {
        if self.title.trim().is_empty() {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "work source title must not be empty",
            ));
        }
        let mut entrypoints = BTreeSet::new();
        for entrypoint in &self.entrypoints {
            entrypoint.validate()?;
            if !entrypoints.insert(entrypoint.id.as_str()) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "work source contains a duplicate entrypoint id",
                ));
            }
        }
        let mut content = BTreeSet::new();
        for root in &self.content {
            if !content.insert(root.as_str()) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "work source contains a duplicate content reference",
                ));
            }
        }
        validate_portable_model(&self.annotations)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkSourceEntrypoint {
    pub id: String,
    pub intent_uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_id: Option<PortId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreign_launch_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl WorkSourceEntrypoint {
    fn validate(&self) -> ModelResult<()> {
        PortId::parse(self.id.clone())?;
        if self.intent_uri.trim().is_empty()
            || self.intent_uri.chars().any(char::is_whitespace)
            || self.intent_uri.chars().any(char::is_control)
        {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "work entrypoint intent is invalid",
            ));
        }
        let target_count = usize::from(self.port_id.is_some())
            + usize::from(self.surface_id.is_some())
            + usize::from(self.foreign_launch_id.is_some());
        if target_count != 1 {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "work entrypoint must declare exactly one target",
            ));
        }
        if self.surface_id.as_ref().is_some_and(|value| {
            value.trim().is_empty()
                || value.chars().any(char::is_whitespace)
                || value.chars().any(char::is_control)
        }) {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "work entrypoint surface id is invalid",
            ));
        }
        if let Some(launch_id) = &self.foreign_launch_id {
            PortId::parse(launch_id.clone())?;
        }
        validate_portable_model(&self.annotations)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AssemblySourceDocument {
    pub schema: String,
    pub assembly: AssemblySourceDescriptor,
}

impl AssemblySourceDocument {
    pub fn validate(&self) -> ModelResult<()> {
        if self.schema != ASSEMBLY_SOURCE_SCHEMA {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "assembly source uses an unsupported schema",
            ));
        }
        self.assembly.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AssemblySourceDescriptor {
    pub id: AssemblyId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<AssemblyNodeSourceDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<AssemblyBindingSourceDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exposed_ports: Vec<AssemblyExposureSourceDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_slots: Vec<StateSlotSourceDescriptor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl AssemblySourceDescriptor {
    fn validate(&self) -> ModelResult<()> {
        if self.nodes.len() > MAX_ASSEMBLY_NODES
            || self.bindings.len() > MAX_ASSEMBLY_BINDINGS
            || self.exposed_ports.len() > MAX_EXPOSED_PORTS
            || self.state_slots.len() > MAX_STATE_SLOTS
        {
            return Err(ModelError::new(
                DiagnosticCode::WorkTooComplex,
                "assembly source exceeds an implementation complexity budget",
            ));
        }
        let mut nodes = BTreeSet::new();
        for node in &self.nodes {
            node.validate()?;
            if !nodes.insert(&node.id) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "assembly source contains a duplicate node id",
                ));
            }
        }
        let mut bindings = BTreeSet::new();
        for binding in &self.bindings {
            binding.validate()?;
            if !bindings.insert(binding.id.as_str())
                || !nodes.contains(&binding.from.node_id)
                || !nodes.contains(&binding.to.node_id)
            {
                return Err(ModelError::new(
                    DiagnosticCode::PortUnresolved,
                    "assembly source binding identity or node reference is invalid",
                ));
            }
        }
        let mut exposures = BTreeSet::new();
        for exposure in &self.exposed_ports {
            if !exposures.insert(&exposure.id) || !nodes.contains(&exposure.target.node_id) {
                return Err(ModelError::new(
                    DiagnosticCode::PortUnresolved,
                    "assembly source exposure identity or node reference is invalid",
                ));
            }
        }
        let mut slots = BTreeSet::new();
        for slot in &self.state_slots {
            slot.validate()?;
            if !slots.insert(&slot.id)
                || !nodes.contains(&slot.owner)
                || slot
                    .migration_port
                    .as_ref()
                    .is_some_and(|endpoint| !nodes.contains(&endpoint.node_id))
            {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "assembly source state slot identity or node reference is invalid",
                ));
            }
        }
        validate_portable_model(&self.annotations)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AssemblyNodeSourceDescriptor {
    pub id: NodeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<SourcePathRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<SourcePathRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<PortDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<SourcePathRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl AssemblyNodeSourceDescriptor {
    fn validate(&self) -> ModelResult<()> {
        if self.component.is_some() == self.assembly.is_some() {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "assembly node must reference exactly one component or nested assembly",
            ));
        }
        if self.assembly.is_some() && !self.ports.is_empty() {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "nested Assembly nodes expose Ports through the nested source, not an inline catalog",
            ));
        }
        let mut ports = BTreeSet::new();
        for port in &self.ports {
            port.validate()?;
            if !ports.insert(&port.port_id) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "assembly node contains a duplicate explicit port id",
                ));
            }
        }
        validate_portable_model(&self.annotations)
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct SourcePortEndpoint {
    pub node_id: NodeId,
    pub port_id: PortId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AssemblyBindingSourceDescriptor {
    pub id: String,
    pub from: SourcePortEndpoint,
    pub to: SourcePortEndpoint,
    #[serde(default = "default_binding_phase")]
    pub phase: BindingPhase,
    #[serde(default)]
    pub transport: TransportPolicy,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl AssemblyBindingSourceDescriptor {
    fn validate(&self) -> ModelResult<()> {
        PortId::parse(self.id.clone())?;
        self.transport.validate()?;
        validate_portable_model(&self.annotations)
    }
}

fn default_binding_phase() -> BindingPhase {
    BindingPhase::Authoring
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AssemblyExposureSourceDescriptor {
    pub id: PortId,
    pub direction: PortDirection,
    pub target: SourcePortEndpoint,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StateSlotSourceDescriptor {
    pub id: StateSlotId,
    pub owner: NodeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_ref: Option<SourcePathRef>,
    pub scope: StateScope,
    pub portability: StatePortability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_port: Option<SourcePortEndpoint>,
    pub backup_policy: BackupPolicy,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl StateSlotSourceDescriptor {
    fn validate(&self) -> ModelResult<()> {
        if self.portability == StatePortability::Portable && self.schema_ref.is_none() {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "portable source state slot must reference a schema",
            ));
        }
        validate_portable_model(&self.annotations)
    }
}

pub fn parse_work_source(bytes: &[u8]) -> ModelResult<WorkSourceDocument> {
    let document: WorkSourceDocument = parse_yaml_source(bytes)?;
    document.validate()?;
    Ok(document)
}

pub fn parse_assembly_source(bytes: &[u8]) -> ModelResult<AssemblySourceDocument> {
    let document: AssemblySourceDocument = parse_yaml_source(bytes)?;
    document.validate()?;
    Ok(document)
}

fn parse_yaml_source<T>(bytes: &[u8]) -> ModelResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    if bytes.len() > MAX_SOURCE_DESCRIPTOR_BYTES {
        return Err(ModelError::new(
            DiagnosticCode::ArtifactBudgetExceeded,
            "YAML source descriptor exceeds the 1 MiB implementation budget",
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| {
        ModelError::new(
            DiagnosticCode::WorkInvalid,
            "YAML source descriptor is not valid UTF-8",
        )
    })?;
    serde_yaml::from_str(text).map_err(|_| {
        ModelError::new(
            DiagnosticCode::WorkInvalid,
            "YAML source descriptor is malformed",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: &str = r#"schema: plurora.work-source.v1
work:
  id: example/modular-simulation
  title: Modular Simulation
  description: A composable simulation.
  assembly: assembly.yaml
  entrypoints:
    - id: play
      intent_uri: plurora.shell.default/play
      surface_id: example/simulation-ui/play
  content: [content/]
  rights: rights.yaml
  operational_intent: operation.yaml
  annotations:
    thirdparty.example/value: {enabled: true}
"#;

    const ASSEMBLY: &str = r#"schema: plurora.assembly-source.v1
assembly:
  id: example/modular-simulation-main
  nodes:
    - id: simulation
      component: packages/simulation/manifest.yaml#main
    - id: save
      component: packages/save/manifest.yaml#main
  bindings:
    - id: save-binding
      from: {node_id: save, port_id: save-export}
      to: {node_id: simulation, port_id: save-import}
  exposed_ports:
    - id: play
      direction: export
      target: {node_id: simulation, port_id: play}
  state_slots:
    - id: state
      owner: simulation
      schema_ref: schemas/state.json
      scope: user
      portability: portable
      backup_policy: required
  annotations:
    thirdparty.example/value: [one, two]
"#;

    #[test]
    fn valid_documents_parse_and_annotations_round_trip() {
        let work = parse_work_source(WORK.as_bytes()).unwrap();
        assert_eq!(work.work.content[0].as_str(), "content/");
        let assembly = parse_assembly_source(ASSEMBLY.as_bytes()).unwrap();
        assert_eq!(assembly.assembly.bindings[0].from.node_id.as_str(), "save");
        let yaml = serde_yaml::to_string(&assembly).unwrap();
        let decoded = parse_assembly_source(yaml.as_bytes()).unwrap();
        assert_eq!(decoded.assembly.annotations, assembly.assembly.annotations);
    }

    #[test]
    fn malformed_utf8_yaml_and_wrong_schema_are_rejected_without_input_echo() {
        assert_eq!(
            parse_work_source(&[0xff]).unwrap_err().code,
            DiagnosticCode::WorkInvalid
        );
        assert_eq!(
            parse_work_source(b"schema: [").unwrap_err().code,
            DiagnosticCode::WorkInvalid
        );
        let wrong = WORK.replace(WORK_SOURCE_SCHEMA, "plurora.work-source.v2");
        assert_eq!(
            parse_work_source(wrong.as_bytes()).unwrap_err().code,
            DiagnosticCode::WorkInvalid
        );
    }

    #[test]
    fn one_mib_is_accepted_and_one_byte_more_is_rejected() {
        let mut exact = WORK.as_bytes().to_vec();
        exact.extend(std::iter::repeat_n(
            b' ',
            MAX_SOURCE_DESCRIPTOR_BYTES - exact.len(),
        ));
        parse_work_source(&exact).unwrap();
        exact.push(b' ');
        assert_eq!(
            parse_work_source(&exact).unwrap_err().code,
            DiagnosticCode::ArtifactBudgetExceeded
        );
    }

    #[test]
    fn path_references_reject_escape_absolute_prefix_backslash_and_nul() {
        for path in [
            "../assembly.yaml",
            "/assembly.yaml",
            "C:assembly.yaml",
            "dir\\assembly.yaml",
            "dir/../assembly.yaml",
            "dir/./assembly.yaml",
            "dir//assembly.yaml",
            "file:assembly.yaml",
            "https://example.invalid/assembly.yaml",
            "bad\0path",
        ] {
            let yaml = WORK.replace("assembly.yaml", path);
            assert_eq!(
                parse_work_source(yaml.as_bytes()).unwrap_err().code,
                DiagnosticCode::WorkInvalid,
                "{path}"
            );
        }
    }

    #[test]
    fn explicit_node_ports_reject_duplicate_ids() {
        let yaml = ASSEMBLY.replace(
            "component: packages/simulation/manifest.yaml#main",
            r#"component: packages/simulation/manifest.yaml#main
      ports:
        - &port
          port_id: play
          contract: {protocol_id: game.play, interface_id: start, version: 1.0.0}
          interaction: plurora.interaction.capability-unary/v1
          role:
            kind: export
            multiplicity: {min: 0, max: 1}
            effect_class: external_effecting
        - *port"#,
        );
        assert_eq!(
            parse_assembly_source(yaml.as_bytes()).unwrap_err().code,
            DiagnosticCode::WorkInvalid
        );
    }

    #[test]
    fn endpoint_objects_preserve_dots_in_both_local_ids() {
        let yaml = ASSEMBLY
            .replace(
                "node_id: save, port_id: save-export",
                "node_id: save.main, port_id: save.export",
            )
            .replace("id: save\n", "id: save.main\n");
        let document = parse_assembly_source(yaml.as_bytes()).unwrap();
        assert_eq!(
            document.assembly.bindings[0].from.node_id.as_str(),
            "save.main"
        );
        assert_eq!(
            document.assembly.bindings[0].from.port_id.as_str(),
            "save.export"
        );
        let encoded = serde_yaml::to_string(&document).unwrap();
        let decoded = parse_assembly_source(encoded.as_bytes()).unwrap();
        assert_eq!(decoded, document);
    }
}
