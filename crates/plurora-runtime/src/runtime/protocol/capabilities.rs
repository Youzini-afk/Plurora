use super::*;

impl<S> Runtime<S>
where
    S: EventStore,
{
    // --- Capability ---

    pub(crate) async fn dispatch_cap_attenuate(&self, params: &Value) -> anyhow::Result<Value> {
        let parent_handle: CapHandleId =
            serde_json::from_value(params.get("parent_handle").cloned().ok_or_else(|| {
                anyhow::anyhow!("kernel.v1.cap.attenuate requires parent_handle")
            })?)?;
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
                .ok_or_else(|| anyhow::anyhow!("kernel.v1.cap.revoke requires handle"))?,
        )?;
        self.handles.revoke(handle).await?;
        Ok(json!({}))
    }

    pub(crate) async fn dispatch_cap_list_for(&self, params: &Value) -> anyhow::Result<Value> {
        let package_id: PackageId = params
            .get("package_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.cap.list_for requires package_id"))?
            .to_string();
        Ok(json!({ "handles": self.handles.list_for(&package_id).await }))
    }

    pub(crate) async fn dispatch_capability_stream(&self, params: &Value) -> anyhow::Result<Value> {
        let (capability_id, handle_version) = if let Some(handle_value) = params.get("handle") {
            let handle_id: CapHandleId = serde_json::from_value(handle_value.clone())?;
            let handle = self
                .handles
                .lookup(handle_id)
                .await
                .ok_or_else(|| anyhow::anyhow!("capability handle not found"))?;
            if handle.revoked {
                anyhow::bail!("capability handle is revoked");
            }
            if let Some(expires_at) = handle.lease.expires_at {
                if expires_at <= chrono::Utc::now() {
                    anyhow::bail!("capability handle lease expired");
                }
            }
            if let Some(max_invocations) = handle.lease.max_invocations {
                if handle.lease.invocations_used >= max_invocations {
                    anyhow::bail!("capability handle lease exhausted");
                }
            }
            let version = if handle.cap_version == "1" {
                None
            } else {
                Some(handle.cap_version)
            };
            (handle.cap_type, version)
        } else {
            (
                params
                    .get("capability_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "kernel.v1.capability.stream requires capability_id or handle"
                        )
                    })?
                    .to_string(),
                None,
            )
        };
        let session_id = params
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.capability.stream requires session_id"))?
            .to_string();
        let provider_package_id: Option<String> = params
            .get("provider_package_id")
            .and_then(Value::as_str)
            .map(String::from);
        let version: Option<String> = handle_version.or_else(|| {
            params
                .get("version")
                .and_then(Value::as_str)
                .map(String::from)
        });
        let metadata = params
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let (frame, record) = self
            .stream_capability_start(
                &session_id,
                &capability_id,
                provider_package_id.as_ref().map(|x| x.as_str()),
                version.as_ref().map(|s| s.as_str()),
                metadata,
            )
            .await?;
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
                            anyhow::anyhow!(
                                "kernel.v1.capability.cancel requires invocation_id or stream_id"
                            )
                        })?;
                self.streams
                    .get_invocation_by_stream_id(stream_id)
                    .await
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "kernel.v1.capability.cancel stream_id '{}' not found",
                            stream_id
                        )
                    })?
                    .invocation_id
            }
        };
        let session_id = params
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.capability.cancel requires session_id"))?
            .to_string();
        let frame = self
            .stream_capability_cancel(&session_id, &invocation_id)
            .await?;
        if session_id.starts_with("kernel_outbound_websocket_") {
            self.outbound_websocket_executor()
                .close(&frame.stream_id, 1001, Some("cancelled".to_string()))
                .await?;
        }
        Ok(serde_json::to_value(frame)?)
    }
}
