use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use serde_json::json;

use crate::commands::install::{invoke_install_lab, load_install_runtime};
use crate::install::default_data_dir;

#[derive(Args, Debug)]
pub struct LockfileArgs {
    #[arg(short, long, default_value = "default")]
    pub profile: String,

    #[arg(long)]
    pub data_dir: Option<PathBuf>,

    /// Just verify lockfile matches store; exit non-zero on drift
    #[arg(long)]
    pub check: bool,
}

pub async fn run(args: LockfileArgs) -> Result<()> {
    let data_dir = args.data_dir.unwrap_or_else(default_data_dir);
    let runtime = load_install_runtime().await?;
    let result = invoke_install_lab(
        &runtime,
        "plurora/install-lab/check_lockfile",
        json!({
            "profile": args.profile,
            "data_dir": data_dir.display().to_string(),
        }),
    )
    .await?
    .output;
    let ok = result["ok"].as_bool().unwrap_or(false);
    if ok {
        println!("Lockfile OK");
    } else {
        println!("Lockfile drift detected:");
        println!("{}", serde_json::to_string_pretty(&result["drift"])?);
        if args.check {
            anyhow::bail!("lockfile drift detected");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Command};

    #[test]
    fn parses_lockfile_args() {
        let cli = Cli::try_parse_from([
            "plurora",
            "lockfile",
            "--profile",
            "dev",
            "--data-dir",
            "/tmp/plurora",
            "--check",
        ])
        .unwrap();
        match cli.command {
            Command::Lockfile(args) => {
                assert_eq!(args.profile, "dev");
                assert_eq!(args.data_dir, Some(PathBuf::from("/tmp/plurora")));
                assert!(args.check);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
