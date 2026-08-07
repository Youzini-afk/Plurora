use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use serde_json::{json, Value};

use crate::commands::install::{
    invoke_install_lab, join_or_none, load_install_runtime, OutputFormat,
};
use crate::install::default_data_dir;

#[derive(Args, Debug)]
pub struct ListInstalledArgs {
    #[arg(short, long, default_value = "default")]
    pub profile: String,

    #[arg(long)]
    pub data_dir: Option<PathBuf>,

    #[arg(long, default_value = "human")]
    pub format: OutputFormat,
}

pub async fn run(args: ListInstalledArgs) -> Result<()> {
    let data_dir = args.data_dir.unwrap_or_else(default_data_dir);
    let runtime = load_install_runtime().await?;
    let result = invoke_install_lab(
        &runtime,
        "official/install-lab/list_installed",
        json!({
            "profile": args.profile,
            "data_dir": data_dir.display().to_string(),
        }),
    )
    .await?
    .output;

    match args.format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&result)?),
        OutputFormat::Human => print_list_human(&args.profile, &result),
    }
    Ok(())
}

fn print_list_human(profile: &str, result: &Value) {
    println!("Profile: {profile}");
    println!("Installed packages:");
    let packages = result["packages"].as_array().cloned().unwrap_or_default();
    if packages.is_empty() {
        println!("  (none)");
        return;
    }
    for pkg in packages {
        let id = pkg["id"].as_str().unwrap_or("(unknown)");
        let version = pkg["version"].as_str().unwrap_or("(unknown)");
        let store = pkg["store_path"].as_str().unwrap_or("(unknown)");
        println!("  {id} @ {version}    ({store})");
        let grants = grant_summary(&pkg);
        println!("    granted: {grants}");
    }
}

fn grant_summary(pkg: &Value) -> String {
    let mut parts = Vec::new();
    let caps = join_or_none(pkg["granted_capabilities"].as_array());
    if caps != "(none)" {
        parts.push(caps);
    }
    let network = join_or_none(pkg["granted_network"].as_array());
    if network != "(none)" {
        parts.push(format!("network={network}"));
    }
    let secrets = join_or_none(pkg["granted_secrets"].as_array());
    if secrets != "(none)" {
        parts.push(format!("secrets={secrets}"));
    }
    if parts.is_empty() {
        "(none)".to_string()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};

    #[test]
    fn parses_list_installed_args() {
        let cli = Cli::try_parse_from([
            "plurora",
            "list-installed",
            "--profile",
            "dev",
            "--data-dir",
            "/tmp/plurora",
            "--format",
            "json",
        ])
        .unwrap();
        match cli.command {
            Command::ListInstalled(args) => {
                assert_eq!(args.profile, "dev");
                assert_eq!(args.data_dir, Some(PathBuf::from("/tmp/plurora")));
                assert_eq!(args.format, OutputFormat::Json);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
