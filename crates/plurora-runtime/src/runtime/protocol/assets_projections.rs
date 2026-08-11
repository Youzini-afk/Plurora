use super::*;

impl<S> Runtime<S>
where
    S: EventStore,
{
    // --- Asset ---

    pub(crate) async fn dispatch_asset_get(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let request: crate::runtime::ObjectGetRequest = serde_json::from_value(params.clone())
            .map_err(|error| anyhow::anyhow!("object.get request is invalid: {error}"))?;
        crate::runtime::protocol_dispatch::ensure_object_get_access(context, &request)?;
        Ok(serde_json::to_value(self.get_object(request).await?)?)
    }

    // --- Projection ---

    pub(crate) async fn dispatch_projection_rebuild(
        &self,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let projection_id = params
            .get("projection_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("projection.rebuild requires projection_id"))?;
        Ok(serde_json::to_value(
            self.projection_rebuild(projection_id).await?,
        )?)
    }

    pub(crate) async fn dispatch_projection_get(&self, params: &Value) -> anyhow::Result<Value> {
        let projection_id = params
            .get("projection_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("projection.get requires projection_id"))?;
        Ok(serde_json::to_value(
            self.projection_get(projection_id).await?,
        )?)
    }
}
