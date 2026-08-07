use super::*;

impl<S> Runtime<S>
where
    S: EventStore,
{
    // --- Package ---

    pub(crate) async fn dispatch_package_status(&self, params: &Value) -> anyhow::Result<Value> {
        let package_id = params
            .get("package_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.package.status requires package_id"))?
            .to_string();
        Ok(serde_json::to_value(
            self.package_status(&package_id)
                .await
                .ok_or_else(|| anyhow::anyhow!("package '{package_id}' is not loaded"))?,
        )?)
    }

    pub(crate) async fn dispatch_package_unload(&self, params: &Value) -> anyhow::Result<Value> {
        let package_id = params
            .get("package_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.package.unload requires package_id"))?
            .to_string();
        Ok(serde_json::to_value(
            self.unload_package(&package_id).await?,
        )?)
    }

    pub(crate) async fn dispatch_package_restart(&self, params: &Value) -> anyhow::Result<Value> {
        let package_id = params
            .get("package_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.package.restart requires package_id"))?
            .to_string();
        Ok(serde_json::to_value(
            self.restart_package(&package_id).await?,
        )?)
    }

    pub(crate) async fn dispatch_package_logs(&self, params: &Value) -> anyhow::Result<Value> {
        let package_id = params
            .get("package_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.package.logs requires package_id"))?
            .to_string();
        Ok(serde_json::to_value(self.package_logs(&package_id).await)?)
    }
}
