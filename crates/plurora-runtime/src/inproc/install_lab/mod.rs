//! Handler for `plurora/install-lab` capabilities.
//!
//! Install Lab detects and resolves sources, persists immutable Work/Assembly
//! candidate objects, and leaves Installation authority to the public Host API.

use anyhow::Result;
use serde_json::Value;

use super::InprocInvocation;

mod candidate;
mod detection;
mod executor;
mod fs_copy;
mod intake;
mod layout;
mod planner;
mod source;
mod types;

const PACKAGE_ID: &str = "plurora/install-lab";

pub async fn try_handle(request: &mut InprocInvocation) -> Option<Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    match request.capability_id.as_str() {
        "plurora/install-lab/detect_source" => {
            Some(detection::detect_source(std::mem::take(&mut request.input)).await)
        }
        "plurora/install-lab/resolve_plan" => {
            Some(planner::resolve_plan(std::mem::take(&mut request.input)).await)
        }
        "plurora/install-lab/execute_plan" => {
            Some(executor::execute_plan(std::mem::take(&mut request.input)).await)
        }
        "plurora/install-lab/prepare_external_intake" => {
            Some(intake::prepare_external_intake(std::mem::take(&mut request.input)).await)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn retired_capability_ids_are_unhandled() {
        for capability_id in [
            "plurora/install-lab/detect_kind",
            "install.detect_kind",
            "plurora/install-lab/register_project",
            "plurora/install-lab/uninstall",
            "plurora/install-lab/list_installed",
            "plurora/install-lab/check_lockfile",
            "plurora/install-lab/check_for_updates",
            "plurora/install-lab/update_project",
        ] {
            let mut request = InprocInvocation {
                provider_package_id: PACKAGE_ID.to_string(),
                capability_id: capability_id.to_string(),
                session_id: None,
                input: serde_json::json!({}),
            };
            assert!(try_handle(&mut request).await.is_none(), "{capability_id}");
        }
    }
}
