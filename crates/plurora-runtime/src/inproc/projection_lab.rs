//! Handler for `official/projection-lab` capabilities.

use serde_json::Value;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/projection-lab";

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/diff") {
        Some(diff(request))
    } else {
        None
    }
}

fn diff(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "projection_diff",
        "before": request.input.get("before").cloned().unwrap_or(Value::Null),
        "after": request.input.get("after").cloned().unwrap_or(Value::Null),
        "projection_id": request.input.get("projection_id").cloned().unwrap_or(Value::Null),
    }))
}
