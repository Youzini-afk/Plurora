use super::*;

impl<S> Runtime<S>
where
    S: EventStore,
{
    // --- Asset ---

    pub(crate) async fn dispatch_asset_get(&self, params: &Value) -> anyhow::Result<Value> {
        let asset_id = params
            .get("asset_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.asset.get requires asset_id"))?;
        Ok(serde_json::to_value(self.get_asset(asset_id).await?)?)
    }

    // --- Projection ---

    pub(crate) async fn dispatch_projection_rebuild(
        &self,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let projection_id = params
            .get("projection_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                anyhow::anyhow!("kernel.v1.projection.rebuild requires projection_id")
            })?;
        Ok(serde_json::to_value(
            self.projection_rebuild(projection_id).await?,
        )?)
    }

    pub(crate) async fn dispatch_projection_get(&self, params: &Value) -> anyhow::Result<Value> {
        let projection_id = params
            .get("projection_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.projection.get requires projection_id"))?;
        Ok(serde_json::to_value(
            self.projection_get(projection_id).await?,
        )?)
    }
}
