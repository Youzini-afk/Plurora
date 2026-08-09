use std::collections::{BTreeMap, BTreeSet};

use plurora_core::ArtifactDescriptor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::assembly::ASSEMBLY_REVISION_TYPE_URI;
use crate::canonical::{
    fixed_string_schema, validate_artifact_descriptor, validate_descriptor_type,
    validate_portable_model, ArtifactModel,
};
use crate::diagnostic::{DiagnosticCode, ModelError, ModelResult};
use crate::ids::{validate_local_id, PortId, WorkId};
use crate::operational::OPERATIONAL_INTENT_TYPE_URI;
use crate::rights::{RIGHTS_DECLARATION_TYPE_URI, TRANSPARENCY_DECLARATION_TYPE_URI};

pub const WORK_REVISION_TYPE_URI: &str = "urn:plurora:work-revision:v1";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkEntrypointTarget {
    AssemblyPort { port_id: PortId },
    Surface { surface_id: String },
    ForeignLaunch { launch_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorkEntrypoint {
    pub id: String,
    pub intent_uri: String,
    pub target: WorkEntrypointTarget,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

impl WorkEntrypoint {
    pub fn validate(&self) -> ModelResult<()> {
        validate_local_id(&self.id, "work entrypoint id")?;
        validate_open_identifier(&self.intent_uri, "work entrypoint intent")?;
        match &self.target {
            WorkEntrypointTarget::AssemblyPort { .. } => {}
            WorkEntrypointTarget::Surface { surface_id } => {
                validate_open_identifier(surface_id, "surface id")?
            }
            WorkEntrypointTarget::ForeignLaunch { launch_id } => {
                validate_local_id(launch_id, "foreign launch id")?
            }
        }
        validate_portable_model(&self.annotations)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorkRevision {
    #[schemars(schema_with = "work_revision_schema")]
    pub schema: String,
    pub work_id: WorkId,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub assembly: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content_roots: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entrypoints: Vec<WorkEntrypoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rights: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparency: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operational_intent: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, Value>,
}

fn work_revision_schema(
    _generator: &mut schemars::gen::SchemaGenerator,
) -> schemars::schema::Schema {
    fixed_string_schema(WorkRevision::SCHEMA)
}

impl WorkRevision {
    pub const SCHEMA: &'static str = "plurora.work-revision.v1";

    pub fn validate(&self) -> ModelResult<()> {
        if self.schema != Self::SCHEMA || self.title.trim().is_empty() {
            return Err(ModelError::new(
                DiagnosticCode::WorkInvalid,
                "work revision schema or title is invalid",
            ));
        }
        validate_descriptor_type(&self.assembly, ASSEMBLY_REVISION_TYPE_URI)?;
        let mut roots = BTreeSet::new();
        for root in &self.content_roots {
            validate_artifact_descriptor(root)?;
            if !roots.insert(root.digest.as_str()) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "work revision contains a duplicate content root",
                ));
            }
        }
        let mut entrypoints = BTreeSet::new();
        for entrypoint in &self.entrypoints {
            entrypoint.validate()?;
            if !entrypoints.insert(entrypoint.id.as_str()) {
                return Err(ModelError::new(
                    DiagnosticCode::WorkInvalid,
                    "work revision contains a duplicate entrypoint id",
                ));
            }
        }
        if let Some(rights) = &self.rights {
            validate_descriptor_type(rights, RIGHTS_DECLARATION_TYPE_URI)?;
        }
        if let Some(transparency) = &self.transparency {
            validate_descriptor_type(transparency, TRANSPARENCY_DECLARATION_TYPE_URI)?;
        }
        if let Some(intent) = &self.operational_intent {
            validate_descriptor_type(intent, OPERATIONAL_INTENT_TYPE_URI)?;
        }
        validate_portable_model(self)
    }
}

impl ArtifactModel for WorkRevision {
    const ARTIFACT_TYPE_URI: &'static str = WORK_REVISION_TYPE_URI;

    fn validate(&self) -> ModelResult<()> {
        WorkRevision::validate(self)
    }

    fn referenced_artifacts(&self) -> Vec<&ArtifactDescriptor> {
        let mut references = vec![&self.assembly];
        references.extend(&self.content_roots);
        references.extend(self.rights.iter());
        references.extend(self.transparency.iter());
        references.extend(self.operational_intent.iter());
        references
    }

    fn descriptor_annotations(&self) -> BTreeMap<String, Value> {
        BTreeMap::from([(
            "work_id".to_string(),
            Value::String(self.work_id.to_string()),
        )])
    }
}

fn validate_open_identifier(value: &str, label: &str) -> ModelResult<()> {
    if value.trim().is_empty()
        || value.chars().any(char::is_whitespace)
        || value.chars().any(char::is_control)
    {
        return Err(ModelError::new(
            DiagnosticCode::WorkInvalid,
            format!("{label} is empty or contains whitespace/control characters"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical::ArtifactModel;

    fn descriptor(kind: &str, byte: char) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: kind.to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", byte.to_string().repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    fn work() -> WorkRevision {
        WorkRevision {
            schema: WorkRevision::SCHEMA.to_string(),
            work_id: WorkId::parse("example/simulation").unwrap(),
            title: "Simulation".to_string(),
            description: "Composable rules".to_string(),
            assembly: descriptor(ASSEMBLY_REVISION_TYPE_URI, 'a'),
            content_roots: vec![descriptor("urn:example:content:v1", 'b')],
            entrypoints: vec![WorkEntrypoint {
                id: "play".to_string(),
                intent_uri: "plurora.shell.default/play".to_string(),
                target: WorkEntrypointTarget::AssemblyPort {
                    port_id: PortId::parse("play").unwrap(),
                },
                annotations: BTreeMap::new(),
            }],
            rights: None,
            transparency: None,
            operational_intent: None,
            annotations: BTreeMap::new(),
        }
    }

    #[test]
    fn work_digest_is_reproducible_and_annotations_round_trip() {
        let mut first = work();
        first.annotations.insert("z".to_string(), Value::Bool(true));
        first
            .annotations
            .insert("a".to_string(), serde_json::json!({"nested": 1}));
        let second: WorkRevision =
            serde_json::from_value(serde_json::to_value(&first).unwrap()).unwrap();
        assert_eq!(first.annotations, second.annotations);
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }

    #[test]
    fn work_rejects_raw_paths_in_unknown_annotations() {
        let mut value = work();
        value.annotations.insert(
            "thirdparty.example/source".to_string(),
            Value::String("/home/creator/work".to_string()),
        );
        assert_eq!(value.validate().unwrap_err().code, DiagnosticCode::RawPath);
    }

    #[test]
    fn work_digest_rejects_nondeterministic_annotation_fields() {
        let mut value = work();
        value.annotations.insert(
            "generated_at".to_string(),
            Value::String("2026-08-09T00:00:00Z".to_string()),
        );
        assert_eq!(
            value.digest().unwrap_err().code,
            DiagnosticCode::WorkInvalid
        );
    }

    #[test]
    fn work_schema_exposes_the_exact_discriminator() {
        let schema = serde_json::to_value(schemars::schema_for!(WorkRevision)).unwrap();
        assert_eq!(
            schema.pointer("/properties/schema/const"),
            Some(&serde_json::json!(WorkRevision::SCHEMA))
        );
    }
}
