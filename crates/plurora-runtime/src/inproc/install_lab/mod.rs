//! Handler for `official/install-lab` capabilities.
//!
//! Orchestrates package installation by composing git-tools-lab and
//! integrity-lab through normal capability dispatch.

use anyhow::Result;
use serde_json::Value;
use std::path::Path;

use super::InprocInvocation;

mod executor;
mod fs_copy;
mod gc;
mod intake;
mod layout;
mod planner;
mod project_kind;
mod source;
mod types;
mod update_check;
mod updater;

const PACKAGE_ID: &str = "official/install-lab";

pub use layout::StoreSchemaMigration;

pub fn ensure_store_schema(data_dir: &Path) -> Result<Option<StoreSchemaMigration>> {
    layout::ensure_store_schema(data_dir)
}

pub async fn try_handle(request: &mut InprocInvocation) -> Option<Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    match request.capability_id.as_str() {
        "install.resolve_plan" | "official/install-lab/resolve_plan" => {
            Some(planner::resolve_plan(std::mem::take(&mut request.input)).await)
        }
        "install.execute_plan" | "official/install-lab/execute_plan" => Some(
            executor::execute_plan(
                std::mem::take(&mut request.input),
                request.session_id.as_deref(),
            )
            .await,
        ),
        "install.detect_kind" | "official/install-lab/detect_kind" => {
            Some(project_kind::detect_kind(std::mem::take(&mut request.input)).await)
        }
        "install.prepare_external_intake" | "official/install-lab/prepare_external_intake" => {
            Some(intake::prepare_external_intake(std::mem::take(&mut request.input)).await)
        }
        "install.register_project" | "official/install-lab/register_project" => {
            Some(executor::register_project_capability(std::mem::take(&mut request.input)).await)
        }
        "install.uninstall" | "official/install-lab/uninstall" => Some(
            executor::uninstall(
                std::mem::take(&mut request.input),
                request.session_id.as_deref(),
            )
            .await,
        ),
        "install.list_installed" | "official/install-lab/list_installed" => {
            Some(executor::list_installed(std::mem::take(&mut request.input)).await)
        }
        "install.check_lockfile" | "official/install-lab/check_lockfile" => {
            Some(executor::check_lockfile(std::mem::take(&mut request.input)).await)
        }
        "install.check_for_updates" | "official/install-lab/check_for_updates" => {
            Some(update_check::check_for_updates(std::mem::take(&mut request.input)).await)
        }
        "install.update_project" | "official/install-lab/update_project" => Some(
            updater::update_project(
                std::mem::take(&mut request.input),
                request.session_id.as_deref(),
            )
            .await,
        ),
        _ => None,
    }
}
