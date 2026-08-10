use super::*;

impl<S> Runtime<S>
where
    S: EventStore,
{
    // --- Session ---

    pub(crate) async fn dispatch_session_open(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: OpenSessionRequest = serde_json::from_value(params)?;
        if context.is_host_device() {
            if !context.allows_host_action("run") {
                anyhow::bail!("context.open permission denied: Host device lacks run");
            }
            match request
                .metadata
                .get("installation_id")
                .and_then(Value::as_str)
            {
                Some(installation_id)
                    if context.allows_host_resource("host", "installation", installation_id) => {}
                Some(installation_id) => anyhow::bail!(
                    "context.open permission denied for installation '{}'",
                    installation_id
                ),
                None if context.allows_all_host_resources("host", "installation") => {}
                None => anyhow::bail!(
                    "installation-scoped Host devices must open sessions with metadata.installation_id"
                ),
            }
        }
        Ok(serde_json::to_value(self.open_session(request).await?)?)
    }

    pub(crate) async fn dispatch_session_close(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let session_id = params
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("context.close requires session_id"))?
            .to_string();
        if context.is_host_device() {
            self.ensure_host_session_access(context, "run", &session_id)
                .await?;
        }
        Ok(serde_json::to_value(self.close_session(session_id).await?)?)
    }

    pub(crate) async fn dispatch_session_get(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let session_id = params
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("context.get requires session_id"))?;
        if context.is_host_device() {
            self.ensure_host_session_access(context, "observe", session_id)
                .await?;
        }
        Ok(serde_json::to_value(
            self.get_session(session_id)
                .await
                .ok_or_else(|| anyhow::anyhow!("session '{session_id}' not found"))?,
        )?)
    }

    pub(crate) async fn dispatch_session_fork(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let parent_session_id = params
            .get("parent_session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("context.fork requires parent_session_id"))?
            .to_string();
        if context.is_host_device() {
            self.ensure_host_session_access(context, "run", &parent_session_id)
                .await?;
        }
        let forked_from_sequence = params
            .get("forked_from_sequence")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow::anyhow!("context.fork requires forked_from_sequence"))?;
        let metadata = params.get("metadata").cloned().unwrap_or_else(|| json!({}));
        Ok(serde_json::to_value(
            self.fork_session(parent_session_id, forked_from_sequence, metadata)
                .await?,
        )?)
    }

    pub(crate) async fn dispatch_session_branch_list(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let session_id = params
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("context.branch.list requires session_id"))?
            .to_string();
        if context.is_host_device() {
            self.ensure_host_session_access(context, "observe", &session_id)
                .await?;
        }
        Ok(serde_json::to_value(self.list_branches(&session_id).await)?)
    }

    // --- Event ---

    pub(crate) async fn dispatch_event_list(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let request: EventListRequest = serde_json::from_value(params.clone())?;
        Ok(serde_json::to_value(
            self.list_events_range_with_context(context, &request)
                .await?,
        )?)
    }
}
