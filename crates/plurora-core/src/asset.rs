use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ids::{AssetId, PackageId};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ArtifactDescriptor {
    pub artifact_type_uri: String,
    pub media_type: String,
    pub digest: String,
    pub size_bytes: u64,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub annotations: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssetRecord {
    pub id: AssetId,
    pub origin_package_id: PackageId,
    pub mime: String,
    pub hash: String,
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<ArtifactDescriptor>,
}
