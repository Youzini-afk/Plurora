use super::*;

impl<S> Runtime<S>
where
    S: EventStore,
{
    // --- Capability ---

    pub(crate) async fn dispatch_cap_attenuate(&self, params: &Value) -> anyhow::Result<Value> {
        let parent_handle: CapHandleId =
            serde_json::from_value(params.get("parent_handle").cloned().ok_or_else(|| {
                anyhow::anyhow!("authority.handle.attenuate requires parent_handle")
            })?)?;
        anyhow::ensure!(
            !self.run_bindings.is_binding_handle(parent_handle).await,
            "Run Binding authority cannot be attenuated outside the Binding broker"
        );
        let constraints = params.get("constraints").cloned().unwrap_or(Value::Null);
        let handle_id = self.handles.attenuate(parent_handle, constraints).await?;
        let handle = self
            .handles
            .lookup(handle_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("attenuated capability handle not found"))?;
        Ok(json!({ "handle": handle }))
    }

    pub(crate) async fn dispatch_cap_revoke(&self, params: &Value) -> anyhow::Result<Value> {
        let handle: CapHandleId = serde_json::from_value(
            params
                .get("handle")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("authority.handle.revoke requires handle"))?,
        )?;
        anyhow::ensure!(
            !self.run_bindings.is_binding_handle(handle).await,
            "Run Binding authority must be revoked through host.binding.revoke"
        );
        self.handles.revoke(handle).await?;
        Ok(json!({}))
    }

    pub(crate) async fn dispatch_cap_list_for(&self, params: &Value) -> anyhow::Result<Value> {
        let package_id: PackageId = params
            .get("package_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("authority.handle.list requires package_id"))?
            .to_string();
        Ok(json!({ "handles": self.handles.list_for(&package_id).await }))
    }

    pub(crate) async fn dispatch_capability_stream(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let handle = params
            .get("handle")
            .cloned()
            .map(serde_json::from_value)
            .transpose()?;
        let capability_id = params
            .get("capability_id")
            .and_then(Value::as_str)
            .map(String::from);
        let provider_package_id = params
            .get("provider_package_id")
            .and_then(Value::as_str)
            .map(String::from);
        let version = params
            .get("version")
            .and_then(Value::as_str)
            .map(String::from);
        let (caller_package_id, session_id) = match &context.principal {
            ProtocolPrincipal::Package { package_id } => (
                Some(package_id.clone()),
                context.session_id.clone().ok_or_else(|| {
                    anyhow::anyhow!(
                        "Package capability.stream requires a current invocation session"
                    )
                })?,
            ),
            _ => (
                None,
                context
                    .session_id
                    .clone()
                    .or_else(|| {
                        params
                            .get("session_id")
                            .and_then(Value::as_str)
                            .map(String::from)
                    })
                    .ok_or_else(|| anyhow::anyhow!("capability.stream requires session_id"))?,
            ),
        };
        let request = crate::CapabilityInvocationRequest {
            handle,
            capability_id,
            caller_package_id,
            provider_package_id,
            version,
            session_id: Some(session_id.clone()),
            input: Value::Null,
        };
        let mut prepared = self.prepare_capability_invocation(&request).await?;
        let mut metadata = params
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        if let Some(binding_permit) = prepared.binding_permit.as_ref() {
            let binding_metadata = binding_permit.stream_metadata();
            let object = metadata
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("capability.stream metadata must be an object"))?;
            object.extend(
                binding_metadata
                    .as_object()
                    .expect("broker stream metadata is an object")
                    .clone(),
            );
        }
        let binding_permit = prepared.binding_permit.take();
        let (frame, record) = self
            .stream_capability_start_prepared(
                &session_id,
                &prepared.provider,
                metadata,
                binding_permit,
            )
            .await?;
        if prepared.record_non_binding_invocation {
            self.handles
                .record_invocation(
                    prepared
                        .active_handle
                        .expect("non-binding prepared stream has a handle"),
                )
                .await?;
        }
        Ok(serde_json::json!({
            "frame": frame,
            "invocation": record,
        }))
    }

    pub(crate) async fn dispatch_capability_cancel(&self, params: &Value) -> anyhow::Result<Value> {
        let invocation_id = match params.get("invocation_id").and_then(Value::as_str) {
            Some(invocation_id) => invocation_id.to_string(),
            None => {
                let stream_id =
                    params
                        .get("stream_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            anyhow::anyhow!("capability.cancel requires invocation_id or stream_id")
                        })?;
                self.streams
                    .get_invocation_by_stream_id(stream_id)
                    .await
                    .ok_or_else(|| {
                        anyhow::anyhow!("capability.cancel stream_id '{}' not found", stream_id)
                    })?
                    .invocation_id
            }
        };
        let session_id = params
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("capability.cancel requires session_id"))?
            .to_string();
        let frame = self
            .stream_capability_cancel(&session_id, &invocation_id)
            .await?;
        if session_id.starts_with("platform_outbound_websocket_") {
            self.outbound_websocket_executor()
                .close(&frame.stream_id, 1001, Some("cancelled".to_string()))
                .await?;
        }
        Ok(serde_json::to_value(frame)?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{InMemoryEventStore, RuntimeConfig};

    #[tokio::test]
    async fn package_stream_requires_an_explicit_handle() {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        let session = runtime
            .open_session(OpenSessionRequest {
                labels: Vec::new(),
                active_package_set: vec!["example/caller".to_string()],
                metadata: json!({"kind": "run", "run_id": "run-test"}),
            })
            .await
            .unwrap();
        let mut context = ProtocolContext::package("example/caller", "test");
        context.session_id = Some(session.id.clone());
        let error = runtime
            .call_protocol(
                &context,
                "capability.stream",
                json!({
                    "session_id": session.id,
                    "capability_id": "example/provider/stream",
                }),
            )
            .await
            .unwrap_err();
        assert!(error.message.contains("explicit capability handle"));
    }
}
