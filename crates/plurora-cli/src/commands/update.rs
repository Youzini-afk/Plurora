use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use serde_json::json;

use crate::commands::install::{invoke_install_lab, load_install_runtime};
use crate::install::default_data_dir;

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Specific package to update (default: all installed packages)
    pub package_id: Option<String>,

    /// Specific project to update.
    #[arg(long, value_name = "PROJECT_ID")]
    pub project_id: Option<String>,

    /// Explicitly update all installed packages (default when no target is provided).
    #[arg(long)]
    pub all: bool,

    /// Re-run the update transaction even when sources appear current.
    #[arg(long)]
    pub force: bool,

    #[arg(short, long, default_value = "default")]
    pub profile: String,

    #[arg(long)]
    pub data_dir: Option<PathBuf>,

    /// Only check for updates, don't install
    #[arg(long)]
    pub check_only: bool,
}

pub async fn run(args: UpdateArgs) -> Result<()> {
    anyhow::ensure!(
        !(args.all && args.package_id.is_some()),
        "--all cannot be combined with a package id"
    );
    anyhow::ensure!(
        !(args.all && args.project_id.is_some()),
        "--all cannot be combined with --project-id"
    );
    anyhow::ensure!(
        !(args.package_id.is_some() && args.project_id.is_some()),
        "provide either a package id or --project-id, not both"
    );

    let data_dir = args.data_dir.unwrap_or_else(default_data_dir);
    let runtime = load_install_runtime().await?;
    let mut input = json!({
        "profile": args.profile,
        "data_dir": data_dir.display().to_string(),
    });
    if let Some(package_id) = args.package_id.as_deref() {
        input["package_id"] = json!(package_id);
    }
    if let Some(project_id) = args.project_id.as_deref() {
        input["project_id"] = json!(project_id);
    }

    if args.check_only {
        let checked = invoke_install_lab(
            &runtime,
            "plurora/install-lab/check_for_updates",
            input.clone(),
        )
        .await?
        .output;
        let results = checked["results"].as_array().cloned().unwrap_or_default();
        if results.is_empty() {
            if let Some(package_id) = args.package_id.as_deref() {
                anyhow::bail!("package '{package_id}' is not installed");
            }
            if let Some(project_id) = args.project_id.as_deref() {
                anyhow::bail!(
                    "project '{project_id}' is not installed or has no updateable packages"
                );
            }
            println!("No installed packages.");
            return Ok(());
        }
        for result in results {
            print_update_check_result(&result);
        }
        return Ok(());
    }

    input["force"] = json!(args.force);
    let output = invoke_install_lab(&runtime, "plurora/install-lab/update_project", input)
        .await?
        .output;
    print_update_project_result(&output);

    if output["status"] == json!("current")
        && output["check"]["results"]
            .as_array()
            .is_some_and(Vec::is_empty)
    {
        if let Some(package_id) = args.package_id.as_deref() {
            anyhow::bail!("package '{package_id}' is not installed");
        }
        if let Some(project_id) = args.project_id.as_deref() {
            anyhow::bail!("project '{project_id}' is not installed or has no updateable packages");
        }
        println!("No installed packages.");
    }
    Ok(())
}

fn print_update_project_result(output: &serde_json::Value) {
    if let Some(results) = output["check"]["results"].as_array() {
        for result in results {
            print_update_check_result(result);
        }
    }
    match output["status"].as_str().unwrap_or("unknown") {
        "updated" => {
            let count = output["updated_packages"]
                .as_array()
                .map(Vec::len)
                .unwrap_or(0);
            println!("Updated {count} packages.");
            if output["store_gc"]["ok"] == json!(false) {
                let warning = output["store_gc"]["warning"]
                    .as_str()
                    .unwrap_or("unknown error");
                eprintln!("Warning: store GC failed after update: {warning}");
            }
        }
        "current" => println!("Updated 0 packages."),
        "not_applicable" => {
            let reason = output["reason"].as_str().unwrap_or("not applicable");
            println!("Skipped update: {reason}");
        }
        other => println!("Update finished with status: {other}"),
    }
}

fn print_update_check_result(result: &serde_json::Value) {
    let id = result["package_id"]
        .as_str()
        .or_else(|| result["id"].as_str())
        .unwrap_or("?");
    let status = result["status"].as_str().unwrap_or("unknown");
    let reason = result["reason"].as_str().unwrap_or("");
    match status {
        "current" => println!("{id}: up to date"),
        "update_available" => {
            let current = result["current_commit"]
                .as_str()
                .or_else(|| result["current_tree_hash"].as_str())
                .unwrap_or("(unknown)");
            let available = result["upstream_commit"]
                .as_str()
                .or_else(|| result["available_tree_hash"].as_str())
                .unwrap_or("(unknown)");
            println!("{id}: update available {current} -> {available}");
        }
        "repair_required" => println!("{id}: repair required ({reason})"),
        "not_applicable" => println!("{id}: skipped ({reason})"),
        "check_failed" => println!("{id}: check failed ({reason})"),
        other => println!("{id}: {other} ({reason})"),
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};

    #[test]
    fn parses_update_args() {
        let cli = Cli::try_parse_from([
            "plurora",
            "update",
            "fixture/pkg-local",
            "--profile",
            "dev",
            "--data-dir",
            "/tmp/plurora",
            "--check-only",
        ])
        .unwrap();
        match cli.command {
            Command::Update(args) => {
                assert_eq!(args.package_id.as_deref(), Some("fixture/pkg-local"));
                assert_eq!(args.profile, "dev");
                assert_eq!(args.data_dir, Some(PathBuf::from("/tmp/plurora")));
                assert!(args.check_only);
                assert!(!args.force);
                assert!(!args.all);
                assert_eq!(args.project_id, None);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_update_project_all_force_args() {
        let cli = Cli::try_parse_from([
            "plurora",
            "update",
            "--project-id",
            "fixture-project__abc12345",
            "--all",
            "--force",
        ])
        .unwrap();
        match cli.command {
            Command::Update(args) => {
                assert_eq!(args.package_id, None);
                assert_eq!(
                    args.project_id.as_deref(),
                    Some("fixture-project__abc12345")
                );
                assert!(args.all);
                assert!(args.force);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[tokio::test]
    async fn update_args_runtime_validation_rejects_ambiguous_targets() {
        let args = UpdateArgs {
            package_id: Some("fixture/pkg".to_string()),
            project_id: Some("fixture-project__abc12345".to_string()),
            all: false,
            force: false,
            profile: "default".to_string(),
            data_dir: None,
            check_only: true,
        };
        let err = run(args).await.expect_err("ambiguous target rejected");
        assert!(err
            .to_string()
            .contains("either a package id or --project-id"));
    }
}
