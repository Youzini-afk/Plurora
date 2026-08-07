use super::*;

impl<S> Runtime<S>
where
    S: EventStore,
{
    // --- Permission ---

    pub(crate) async fn dispatch_permission_grant(&self, params: &Value) -> anyhow::Result<Value> {
        let principal = params
            .get("principal")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.permission.grant requires principal"))?;
        let principal: crate::ProtocolPrincipal = serde_json::from_value(principal)?;
        let permission = params
            .get("permission")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.permission.grant requires permission"))?
            .to_string();
        let scope = params
            .get("scope")
            .and_then(Value::as_str)
            .map(str::to_string);
        let reason = params
            .get("reason")
            .and_then(Value::as_str)
            .map(str::to_string);
        Ok(serde_json::to_value(
            self.grant_permission(principal, permission, scope, reason)
                .await?,
        )?)
    }

    pub(crate) async fn dispatch_permission_revoke(&self, params: &Value) -> anyhow::Result<Value> {
        let grant_id = params
            .get("grant_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.permission.revoke requires grant_id"))?;
        Ok(serde_json::to_value(
            self.revoke_permission(grant_id).await?,
        )?)
    }

    pub(crate) async fn dispatch_permission_list(&self, params: &Value) -> anyhow::Result<Value> {
        let principal = match params.get("principal") {
            Some(value) => Some(serde_json::from_value(value.clone())?),
            None => None,
        };
        Ok(serde_json::to_value(
            self.list_permission_grants(principal).await,
        )?)
    }

    pub(crate) async fn dispatch_permission_audit(&self) -> anyhow::Result<Value> {
        let events = self.store.list_kind_prefix("kernel/v1/permission").await?;
        Ok(serde_json::to_value(events)?)
    }
}
