use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type EventId = String;
pub type SessionId = String;
pub type PackageId = String;
pub type CapabilityId = String;
pub type ExtensionPointId = String;
pub type HookId = String;
pub type AssetId = String;
pub type InvocationId = String;
pub type PrincipalId = String;

pub fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdRef {
    pub id: String,
}
