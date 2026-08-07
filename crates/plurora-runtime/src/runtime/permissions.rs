use chrono::Utc;
use plurora_core::{
    new_id, EventEnvelope, PackageId, SessionId, EVENT_PERMISSION_DENIED, EVENT_PERMISSION_GRANTED,
    EVENT_PERMISSION_REVOKED,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::Runtime;
use crate::{EventStore, ProtocolPrincipal};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PermissionGrantRecord {
    pub id: String,
    pub principal: ProtocolPrincipal,
    pub permission: String,
    #[serde(default)]
    pub scope: Option<String>,
    pub granted_at: chrono::DateTime<Utc>,
    #[serde(default)]
    pub revoked_at: Option<chrono::DateTime<Utc>>,
    #[serde(default)]
    pub reason: Option<String>,
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub async fn grant_permission(
        &self,
        principal: ProtocolPrincipal,
        permission: String,
        scope: Option<String>,
        reason: Option<String>,
    ) -> anyhow::Result<PermissionGrantRecord> {
        let record = PermissionGrantRecord {
            id: new_id("gr"),
            principal,
            permission,
            scope,
            granted_at: Utc::now(),
            revoked_at: None,
            reason,
        };
        self.grants
            .write()
            .await
            .insert(record.id.clone(), record.clone());
        self.append_platform_event(
            &format!("platform_permission_{}", record.id),
            EVENT_PERMISSION_GRANTED,
            serde_json::to_value(&record)?,
        )
        .await?;
        Ok(record)
    }

    pub async fn revoke_permission(&self, grant_id: &str) -> anyhow::Result<PermissionGrantRecord> {
        let mut grants = self.grants.write().await;
        let record = grants
            .get_mut(grant_id)
            .ok_or_else(|| anyhow::anyhow!("grant '{grant_id}' not found"))?;
        record.revoked_at = Some(Utc::now());
        let record = record.clone();
        drop(grants);
        self.append_platform_event(
            &format!("platform_permission_{}", record.id),
            EVENT_PERMISSION_REVOKED,
            serde_json::to_value(&record)?,
        )
        .await?;
        Ok(record)
    }

    pub async fn list_permission_grants(
        &self,
        principal: Option<ProtocolPrincipal>,
    ) -> Vec<PermissionGrantRecord> {
        let mut grants: Vec<_> = self
            .grants
            .read()
            .await
            .values()
            .filter(|grant| {
                principal
                    .as_ref()
                    .map(|principal| &grant.principal == principal)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        grants.sort_by(|a, b| a.granted_at.cmp(&b.granted_at));
        grants
    }

    pub async fn principal_has_grant(
        &self,
        principal: &ProtocolPrincipal,
        permission: &str,
        scope: Option<&str>,
    ) -> bool {
        if matches!(
            principal,
            ProtocolPrincipal::HostAdmin | ProtocolPrincipal::HostDev
        ) {
            return true;
        }
        self.grants.read().await.values().any(|grant| {
            grant.revoked_at.is_none()
                && &grant.principal == principal
                && grant.permission == permission
                && grant
                    .scope
                    .as_deref()
                    .map(|grant_scope| {
                        scope
                            .map(|scope| scope.starts_with(grant_scope))
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
        })
    }

    pub(crate) async fn audit_permission_denied(
        &self,
        session_id: &SessionId,
        package_id: &PackageId,
        operation: &str,
    ) -> anyhow::Result<EventEnvelope> {
        self.append_platform_event(
            session_id,
            EVENT_PERMISSION_DENIED,
            json!({
                "package_id": package_id,
                "operation": operation,
            }),
        )
        .await
    }
}
