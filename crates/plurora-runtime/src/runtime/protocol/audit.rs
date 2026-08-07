use super::*;

impl<S> Runtime<S>
where
    S: EventStore,
{
    // --- Audit ---

    pub(crate) async fn dispatch_audit_package(&self, params: &Value) -> anyhow::Result<Value> {
        let request: crate::AuditPackageParams = serde_json::from_value(params.clone())?;
        let (since, until) = request.window();
        Ok(serde_json::to_value(
            self.audit_package(&request.package_id, since, until)
                .await?,
        )?)
    }
}
