use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Args;
use plurora_runtime::{InMemoryEventStore, Runtime, RuntimeConfig};

use super::host::load_host_profile;
use super::manifest::read_manifest;

#[derive(Args, Debug)]
pub struct AuditArgs {
    /// Package ID to audit, or a package directory containing manifest.yaml.
    #[arg(short, long)]
    pub package: String,

    /// Look back this many days (default: 7).
    #[arg(short, long, default_value = "7")]
    pub days: u32,

    /// Output format.
    #[arg(short, long, default_value = "human")]
    pub format: AuditFormat,
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum AuditFormat {
    Human,
    Json,
}

pub async fn run(args: AuditArgs) -> Result<()> {
    // v1 CLI is intentionally in-process/read-only: it loads the default
    // forge-alpha profile into an in-memory runtime, optionally loads the
    // requested package path, and audits that runtime's event store. For a
    // long-running host, call kernel.v1.audit.package over RPC instead.
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Arc::new(Runtime::new(store, RuntimeConfig::default()));
    let profile_path = PathBuf::from("profiles/forge-alpha.yaml");
    if profile_path.exists() {
        load_host_profile(runtime.clone(), profile_path).await?;
    }

    let package_id = resolve_package_arg(runtime.as_ref(), &args.package).await?;
    let until = chrono::Utc::now();
    let since = until - chrono::Duration::days(args.days as i64);
    let report = runtime.audit_package(&package_id, since, until).await?;

    match args.format {
        AuditFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        AuditFormat::Human => print_human(&report, args.days),
    }
    Ok(())
}

async fn resolve_package_arg(
    runtime: &Runtime<InMemoryEventStore>,
    package: &str,
) -> Result<String> {
    if runtime.package_status(&package.to_string()).await.is_some() {
        return Ok(package.to_string());
    }

    let path = PathBuf::from(package);
    let manifest_path = if path.is_dir() {
        for candidate in ["manifest.yaml", "manifest.yml", "manifest.json"] {
            let candidate_path = path.join(candidate);
            if candidate_path.exists() {
                return load_requested_manifest(runtime, candidate_path).await;
            }
        }
        anyhow::bail!(
            "package directory '{}' contains no manifest.yaml/yml/json",
            package
        );
    } else if path.exists() {
        path
    } else if package == "examples/echo" {
        PathBuf::from("examples/packages/echo-rust-inproc/manifest.yaml")
    } else {
        anyhow::bail!(
            "package '{}' is not loaded and is not a manifest path",
            package
        );
    };

    load_requested_manifest(runtime, manifest_path).await
}

async fn load_requested_manifest(
    runtime: &Runtime<InMemoryEventStore>,
    manifest_path: PathBuf,
) -> Result<String> {
    let manifest = read_manifest(manifest_path).await?;
    let package_id = manifest.id.clone();
    if runtime.package_status(&package_id).await.is_none() {
        runtime.load_package(manifest).await?;
    }
    Ok(package_id)
}

fn print_human(report: &plurora_runtime::PackageAuditReport, days: u32) {
    println!("Audit report: {}", report.package_id);
    println!(
        "Period: {} → {} ({} days)",
        report.since.date_naive(),
        report.until.date_naive(),
        days
    );
    println!();

    println!("Declared:");
    println!(
        "  capabilities.invoke: {}",
        list_or_none(&report.declared.capabilities_invoke)
    );
    println!(
        "  network.hosts: {}",
        list_or_none(&report.declared.network_hosts)
    );
    println!(
        "  secret_refs: {}",
        list_or_none(&report.declared.secret_refs)
    );
    println!(
        "  events: read={} append={}",
        report.declared.events_read, report.declared.events_append
    );
    println!(
        "  assets: read={} write={}",
        report.declared.assets_read, report.declared.assets_write
    );
    println!();

    println!("Used:");
    print_count_map(
        "  capabilities.invoked:",
        &report.used.capabilities_invoked,
        "invocations",
    );
    print_count_map(
        "  outbound by host:",
        &report.used.network_hosts_used,
        "requests",
    );
    print_count_map("  secret_refs:", &report.used.secret_refs_used, "uses");
    println!(
        "  events.append: {} events",
        report.used.events_append_count
    );
    println!("  events.read:    {} events", report.used.events_read_count);
    println!(
        "  assets.write:   {} writes",
        report.used.assets_write_count
    );
    println!("  assets.read:    {} reads", report.used.assets_read_count);
    println!();

    println!("Unused (consider removing from manifest):");
    println!(
        "  capabilities: {}",
        list_or_none(&report.unused.capabilities_unused)
    );
    println!(
        "  hosts:        {}",
        list_or_none(&report.unused.network_hosts_unused)
    );
    println!(
        "  secrets:      {}",
        list_or_none(&report.unused.secret_refs_unused)
    );
    println!();

    println!("Suggestions:");
    if report.suggestions.is_empty() {
        println!("  (none)");
    } else {
        for suggestion in &report.suggestions {
            println!("  - {}", suggestion.rationale);
        }
    }
}

fn list_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "(none)".to_string()
    } else {
        values.join(", ")
    }
}

fn print_count_map(label: &str, map: &std::collections::HashMap<String, u64>, unit: &str) {
    println!("{label}");
    if map.is_empty() {
        println!("    (none)");
        return;
    }
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    for (key, count) in entries {
        println!("    {:<32} {} {}", key, count, unit);
    }
}
