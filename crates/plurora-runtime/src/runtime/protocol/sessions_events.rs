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
            anyhow::ensure!(
                context.allows_host_action("access_manage"),
                "context.open permission denied: Host device lacks access_manage"
            );
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
        let session = self
            .get_session(&session_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("session '{session_id}' not found"))?;
        anyhow::ensure!(
            session.metadata.get("kind").and_then(Value::as_str) != Some("run"),
            "Host-owned Run contexts can only be closed through host.run.stop"
        );
        if context.is_host_device() {
            self.ensure_host_session_access(context, "access_manage", &session_id)
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
        let parent = self
            .get_session(&parent_session_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("session '{parent_session_id}' not found"))?;
        anyhow::ensure!(
            parent.metadata.get("kind").and_then(Value::as_str) != Some("run"),
            "Host-owned Run contexts cannot be forked"
        );
        if context.is_host_device() {
            self.ensure_host_session_access(context, "access_manage", &parent_session_id)
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use crate::{InMemoryEventStore, OpenSessionRequest, ProtocolContext, Runtime, RuntimeConfig};

    #[tokio::test]
    async fn public_context_methods_cannot_take_over_host_owned_run_contexts() -> anyhow::Result<()>
    {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        let session = runtime
            .open_session(OpenSessionRequest {
                metadata: json!({"kind": "run"}),
                ..OpenSessionRequest::default()
            })
            .await?;
        let context = ProtocolContext::host_dev("run-context-test");
        assert!(runtime
            .call_protocol(&context, "context.close", json!({"session_id": session.id}),)
            .await
            .is_err());
        assert!(runtime
            .call_protocol(
                &context,
                "context.fork",
                json!({
                    "parent_session_id": session.id,
                    "forked_from_sequence": 0,
                    "metadata": {},
                }),
            )
            .await
            .is_err());
        assert!(runtime.get_session(&session.id).await.is_some());
        runtime.close_session(session.id).await?;
        Ok(())
    }
}
