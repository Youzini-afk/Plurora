use chrono::{DateTime, Utc};
use schemars::{
    schema::{InstanceType, Metadata, Schema, SchemaObject, SingleOrVec},
    JsonSchema,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::ids::{CapabilityId, PackageId, SessionId};

/// Opaque kernel-minted handle ID. Unforgeable per-kernel-process.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct CapHandleId(pub u128);

impl Serialize for CapHandleId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for CapHandleId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = CapHandleId;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a u128 capability handle id encoded as a string or integer")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                value.parse::<u128>().map(CapHandleId).map_err(E::custom)
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(CapHandleId(value as u128))
            }

            fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(CapHandleId(value))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

impl JsonSchema for CapHandleId {
    fn schema_name() -> String {
        "CapHandleId".to_string()
    }

    fn json_schema(_gen: &mut schemars::r#gen::SchemaGenerator) -> Schema {
        let mut schema = SchemaObject::default();
        schema.instance_type = Some(SingleOrVec::Single(Box::new(InstanceType::String)));
        schema.format = Some("uint128".to_string());
        schema.metadata = Some(Box::new(Metadata::default()));
        Schema::Object(schema)
    }
}

impl CapHandleId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().as_u128())
    }
}

impl Default for CapHandleId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CapHandle {
    pub id: CapHandleId,
    pub cap_type: CapabilityId,
    pub cap_version: String,
    pub scope: HandleScope,
    #[schemars(schema_with = "json_value_schema")]
    pub constraints: serde_json::Value,
    pub lease: HandleLease,
    pub provenance: HandleProvenance,
    pub parent: Option<CapHandleId>,
    pub revoked: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct HandleScope {
    pub holder_package_id: PackageId,
    pub session_id: Option<SessionId>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct HandleLease {
    pub expires_at: Option<DateTime<Utc>>,
    pub max_invocations: Option<u32>,
    pub invocations_used: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct HandleProvenance {
    pub granted_at: DateTime<Utc>,
    /// "kernel" if minted by kernel itself.
    pub granted_by_package_id: PackageId,
    /// e.g. "auto_mint", "package_load", "attenuate", "delegate".
    pub via_method: String,
}

fn json_value_schema(_gen: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
    schemars::schema::Schema::Bool(true)
}
