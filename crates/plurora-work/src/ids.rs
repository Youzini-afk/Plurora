use std::fmt;

use schemars::gen::SchemaGenerator;
use schemars::schema::{InstanceType, Schema, SchemaObject, SingleOrVec, StringValidation};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::diagnostic::{DiagnosticCode, ModelError, ModelResult};

fn invalid_id(label: &str) -> ModelError {
    ModelError::new(
        DiagnosticCode::InvalidId,
        format!("{label} does not satisfy the required identifier grammar"),
    )
}

pub(crate) fn validate_local_id(value: &str, label: &str) -> ModelResult<()> {
    if value.is_empty()
        || value.contains("..")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(invalid_id(label));
    }
    Ok(())
}

pub(crate) fn validate_namespaced_id(value: &str, label: &str) -> ModelResult<()> {
    let segments = value.split('/').collect::<Vec<_>>();
    if segments.len() < 2 {
        return Err(invalid_id(label));
    }
    for segment in segments {
        validate_local_id(segment, label)?;
    }
    Ok(())
}

macro_rules! logical_id {
    ($name:ident, $validator:ident, $label:literal, $pattern:literal) => {
        #[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl JsonSchema for $name {
            fn schema_name() -> String {
                stringify!($name).to_string()
            }

            fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
                logical_id_schema($pattern)
            }
        }

        impl $name {
            pub fn parse(value: impl Into<String>) -> ModelResult<Self> {
                let value = value.into();
                $validator(&value, $label)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl TryFrom<String> for $name {
            type Error = ModelError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::parse(value)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

fn logical_id_schema(pattern: &str) -> Schema {
    Schema::Object(SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::String))),
        string: Some(Box::new(StringValidation {
            pattern: Some(pattern.to_string()),
            ..StringValidation::default()
        })),
        ..SchemaObject::default()
    })
}

logical_id!(
    WorkId,
    validate_namespaced_id,
    "work id",
    r"^(?!.*\.\.)[A-Za-z0-9._-]+(?:/[A-Za-z0-9._-]+)+$"
);
logical_id!(
    AssemblyId,
    validate_namespaced_id,
    "assembly id",
    r"^(?!.*\.\.)[A-Za-z0-9._-]+(?:/[A-Za-z0-9._-]+)+$"
);
logical_id!(
    NodeId,
    validate_local_id,
    "node id",
    r"^(?!.*\.\.)[A-Za-z0-9._-]+$"
);
logical_id!(
    PortId,
    validate_local_id,
    "port id",
    r"^(?!.*\.\.)[A-Za-z0-9._-]+$"
);
logical_id!(
    StateSlotId,
    validate_local_id,
    "state slot id",
    r"^(?!.*\.\.)[A-Za-z0-9._-]+$"
);

fn uuid_schema(_generator: &mut SchemaGenerator) -> Schema {
    Schema::Object(SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::String))),
        format: Some("uuid".to_string()),
        ..SchemaObject::default()
    })
}

macro_rules! host_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl JsonSchema for $name {
            fn schema_name() -> String {
                stringify!($name).to_string()
            }

            fn json_schema(generator: &mut SchemaGenerator) -> Schema {
                uuid_schema(generator)
            }
        }

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4().to_string())
            }

            pub fn parse(value: impl Into<String>) -> ModelResult<Self> {
                let value = value.into();
                let parsed = Uuid::parse_str(&value).map_err(|_| invalid_id($label))?;
                Ok(Self(parsed.to_string()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

host_id!(InstallationId, "installation id");
host_id!(RunId, "run id");
host_id!(ExposureId, "exposure id");
host_id!(BindingId, "binding id");
host_id!(RealizationId, "realization id");
host_id!(WorkspaceId, "workspace id");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_ids_reject_paths_and_shell_tokens() {
        assert!(WorkId::parse("example/modular-simulation").is_ok());
        assert!(WorkId::parse("example/../escape").is_err());
        assert!(NodeId::parse("node.one-2").is_ok());
        assert!(NodeId::parse("node/one").is_err());
        assert!(PortId::parse("$(whoami)").is_err());
    }

    #[test]
    fn host_ids_are_opaque_uuid_values() {
        let first = InstallationId::new();
        let second = InstallationId::new();
        assert_ne!(first, second);
        assert!(Uuid::parse_str(first.as_str()).is_ok());
        assert!(InstallationId::parse("title-derived-id").is_err());
        assert_eq!(
            InstallationId::parse(first.as_str().to_ascii_uppercase())
                .unwrap()
                .as_str(),
            first.as_str()
        );
    }

    #[test]
    fn workspace_ids_are_opaque_uuid_values() {
        let workspace_id = WorkspaceId::new();
        assert!(Uuid::parse_str(workspace_id.as_str()).is_ok());
        assert_eq!(
            WorkspaceId::parse(workspace_id.as_str().to_ascii_uppercase())
                .unwrap()
                .as_str(),
            workspace_id.as_str()
        );
        assert!(WorkspaceId::parse("workspace-slug").is_err());
        assert!(WorkspaceId::parse("workspace/path").is_err());
    }

    #[test]
    fn logical_id_schema_carries_the_wire_grammar() {
        let schema = serde_json::to_value(schemars::schema_for!(WorkId)).unwrap();
        assert_eq!(
            schema.pointer("/pattern").and_then(|v| v.as_str()),
            Some(r"^(?!.*\.\.)[A-Za-z0-9._-]+(?:/[A-Za-z0-9._-]+)+$")
        );
    }
}
