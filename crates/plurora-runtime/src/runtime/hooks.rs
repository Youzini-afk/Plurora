use serde_json::Value;

use super::Runtime;
use crate::{validate_json_schema_subset, EventStore, ExtensionDispatchResult};

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub async fn dispatch_extension(
        &self,
        extension_point: &str,
        payload: Value,
    ) -> ExtensionDispatchResult {
        self.extensions.dispatch(extension_point, payload).await
    }

    pub(crate) async fn dispatch_extension_handlers(
        &self,
        extension_point: &str,
        payload: Value,
    ) -> ExtensionDispatchResult {
        let invoked = self.extensions.list_hooks(extension_point).await;
        let mut payload = payload;
        let mut vetoed_by = None;
        for hook in &invoked {
            match hook.subscription.handler.as_str() {
                "veto" => {
                    vetoed_by = Some(hook.subscriber_package_id.clone());
                    break;
                }
                "metadata_trace" => merge_metadata_patch(
                    &mut payload,
                    serde_json::json!({"hook_trace": hook.subscriber_package_id}),
                ),
                handler if handler.contains('/') => {
                    let handler_id = handler.to_string();
                    let provider = match self
                        .capabilities
                        .resolve(&handler_id, Some(&hook.subscriber_package_id), None)
                        .await
                    {
                        Ok(provider) => provider,
                        Err(_) => {
                            vetoed_by = Some(hook.subscriber_package_id.clone());
                            break;
                        }
                    };
                    if validate_json_schema_subset(&provider.descriptor.input_schema, &payload)
                        .is_err()
                    {
                        vetoed_by = Some(hook.subscriber_package_id.clone());
                        break;
                    }
                    let output = match self
                        .execute_registered_capability(
                            &provider,
                            &handler_id,
                            None,
                            payload.clone(),
                        )
                        .await
                    {
                        Ok(output) => output,
                        Err(_) => {
                            vetoed_by = Some(hook.subscriber_package_id.clone());
                            break;
                        }
                    };
                    if output.get("decision").and_then(Value::as_str) == Some("veto") {
                        vetoed_by = Some(hook.subscriber_package_id.clone());
                        break;
                    }
                    if let Some(patch) = output.get("metadata_patch") {
                        merge_metadata_patch(&mut payload, patch.clone());
                    }
                }
                _ => {}
            }
        }
        ExtensionDispatchResult {
            extension_point: extension_point.to_string(),
            invoked,
            vetoed_by,
            payload,
        }
    }
}

fn merge_metadata_patch(payload: &mut Value, patch: Value) {
    let Some(patch) = patch.as_object() else {
        return;
    };
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    let metadata = object
        .entry("metadata")
        .or_insert_with(|| Value::Object(Default::default()));
    let Some(metadata) = metadata.as_object_mut() else {
        return;
    };
    for (key, value) in patch {
        metadata.insert(key.clone(), value.clone());
    }
}
