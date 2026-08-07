use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use plurora_core::project::{ProjectDescriptor, ProjectId, ProjectState};
use plurora_runtime::{InMemoryEventStore, Runtime, RuntimeConfig};
use serde_json::json;

use crate::commands::install::OutputFormat;
use crate::install::default_data_dir;

#[derive(Args, Debug)]
pub struct ProjectArgs {
    #[command(subcommand)]
    pub command: ProjectCommand,

    /// Data directory (default: ~/.plurora or $PLURORA_DATA_DIR)
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum ProjectCommand {
    /// List installed projects
    List(ProjectListArgs),
    /// Show project details
    Info(ProjectInfoArgs),
    /// Show current state
    Status(ProjectStatusArgs),
    /// Start a project (transition to Running)
    Start(ProjectStartArgs),
    /// Stop a running project
    Stop(ProjectStopArgs),
}

#[derive(Args, Debug)]
pub struct ProjectListArgs {
    #[arg(long, default_value = "human")]
    pub format: OutputFormat,
}

#[derive(Args, Debug)]
pub struct ProjectInfoArgs {
    pub project_id: String,
    #[arg(long, default_value = "human")]
    pub format: OutputFormat,
}

#[derive(Args, Debug)]
pub struct ProjectStatusArgs {
    pub project_id: String,
    #[arg(long, default_value = "human")]
    pub format: OutputFormat,
}

#[derive(Args, Debug)]
pub struct ProjectStartArgs {
    pub project_id: String,
}

#[derive(Args, Debug)]
pub struct ProjectStopArgs {
    pub project_id: String,
}

pub async fn run(args: ProjectArgs) -> Result<()> {
    match args.command {
        ProjectCommand::List(a) => run_list(a, args.data_dir).await,
        ProjectCommand::Info(a) => run_info(a, args.data_dir).await,
        ProjectCommand::Status(a) => run_status(a, args.data_dir).await,
        ProjectCommand::Start(a) => run_start(a, args.data_dir).await,
        ProjectCommand::Stop(a) => run_stop(a, args.data_dir).await,
    }
}

pub async fn run_list(args: ProjectListArgs, data_dir: Option<PathBuf>) -> Result<()> {
    let runtime = project_runtime(data_dir)?;
    let mut entries = runtime.config().project_registry.list();
    entries.sort_by(|a, b| a.descriptor.project.id.cmp(&b.descriptor.project.id));
    match args.format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(
                &entries
                    .iter()
                    .map(|entry| json!({"descriptor": entry.descriptor, "state": entry.state}))
                    .collect::<Vec<_>>()
            )?
        ),
        OutputFormat::Human => {
            println!("{:<35} {:<20} {:<20} STATE", "ID", "TITLE", "TYPE");
            for entry in entries {
                println!(
                    "{:<35} {:<20} {:<20} {}",
                    entry.descriptor.project.id.as_str(),
                    truncate(&entry.descriptor.project.title, 20),
                    serde_yaml::to_value(&entry.descriptor.project.project_type)?
                        .as_str()
                        .unwrap_or("?"),
                    state_label(entry.state)
                );
            }
        }
    }
    Ok(())
}

pub async fn run_info(args: ProjectInfoArgs, data_dir: Option<PathBuf>) -> Result<()> {
    let (runtime, id) = runtime_and_id(data_dir, &args.project_id)?;
    let entry = runtime
        .config()
        .project_registry
        .get(&id)
        .with_context(|| format!("project '{}' not found", id))?;
    match args.format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&entry.descriptor)?),
        OutputFormat::Human => print_project_detail(&entry.descriptor, entry.state)?,
    }
    Ok(())
}

pub async fn run_status(args: ProjectStatusArgs, data_dir: Option<PathBuf>) -> Result<()> {
    let data_dir_for_path = data_dir.clone().unwrap_or_else(default_data_dir);
    let (runtime, id) = runtime_and_id(data_dir, &args.project_id)?;
    let entry = runtime
        .config()
        .project_registry
        .get(&id)
        .with_context(|| format!("project '{}' not found", id))?;
    match args.format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "project_id": id.as_str(),
                "title": entry.descriptor.project.title,
                "type": entry.descriptor.project.project_type,
                "state": entry.state,
                "path": project_dir_for(&id, Some(&data_dir_for_path))?.display().to_string(),
            }))?
        ),
        OutputFormat::Human => {
            print_status(&entry.descriptor, entry.state, Some(&data_dir_for_path))?
        }
    }
    Ok(())
}

pub async fn run_start(args: ProjectStartArgs, data_dir: Option<PathBuf>) -> Result<()> {
    let (runtime, id) = runtime_and_id(data_dir, &args.project_id)?;
    runtime
        .config()
        .project_registry
        .set_state(&id, ProjectState::Starting)?;
    runtime
        .config()
        .project_registry
        .set_state(&id, ProjectState::Running)?;
    println!("Project {} started.", id);
    Ok(())
}

pub async fn run_stop(args: ProjectStopArgs, data_dir: Option<PathBuf>) -> Result<()> {
    let (runtime, id) = runtime_and_id(data_dir, &args.project_id)?;
    runtime
        .config()
        .project_registry
        .set_state(&id, ProjectState::Stopping)?;
    runtime
        .config()
        .project_registry
        .set_state(&id, ProjectState::Stopped)?;
    println!("Project {} stopped.", id);
    Ok(())
}

fn project_runtime(data_dir: Option<PathBuf>) -> Result<Runtime<InMemoryEventStore>> {
    let data_dir = data_dir.unwrap_or_else(default_data_dir);
    let _guard = DataDirOverride::set(data_dir);
    plurora_core::paths::ensure_initialized()?;
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());
    runtime.config().project_registry.load_from_disk()?;
    Ok(runtime)
}

fn runtime_and_id(
    data_dir: Option<PathBuf>,
    raw_id: &str,
) -> Result<(Runtime<InMemoryEventStore>, ProjectId)> {
    let runtime = project_runtime(data_dir)?;
    let id = ProjectId::new(raw_id)?;
    Ok((runtime, id))
}

fn print_project_detail(descriptor: &ProjectDescriptor, state: ProjectState) -> Result<()> {
    print_status(descriptor, state, None)?;
    println!("Description: {}", descriptor.project.description);
    println!("Packages:    {}", descriptor.project.packages.join(", "));
    Ok(())
}

fn print_status(
    descriptor: &ProjectDescriptor,
    state: ProjectState,
    data_dir: Option<&Path>,
) -> Result<()> {
    let path = project_dir_for(&descriptor.project.id, data_dir)?;
    println!(
        "Project: {} ({})",
        descriptor.project.title, descriptor.project.id
    );
    println!(
        "Type:    {}",
        serde_yaml::to_value(&descriptor.project.project_type)?
            .as_str()
            .unwrap_or("?")
    );
    println!("State:   {}", state_label(state));
    println!("Sessions: 0 active");
    println!("Secrets:  0 project-scoped, 0 platform-only");
    println!("Path:    {}/", path.display());
    Ok(())
}

fn project_dir_for(id: &ProjectId, data_dir: Option<&Path>) -> Result<PathBuf> {
    if let Some(data_dir) = data_dir {
        Ok(data_dir.join("projects").join(id.as_str()))
    } else {
        plurora_core::paths::project_dir(id)
    }
}

fn state_label(state: ProjectState) -> &'static str {
    match state {
        ProjectState::Installed => "Installed",
        ProjectState::Stopped => "Stopped",
        ProjectState::Starting => "Starting",
        ProjectState::Running => "Running",
        ProjectState::Stopping => "Stopping",
        ProjectState::Failed => "Failed",
        ProjectState::Archived => "Archived",
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        value
            .chars()
            .take(max.saturating_sub(1))
            .collect::<String>()
            + "…"
    }
}

struct DataDirOverride {
    previous: Option<String>,
}

impl DataDirOverride {
    fn set(data_dir: PathBuf) -> Self {
        let previous = std::env::var("PLURORA_DATA_DIR").ok();
        std::env::set_var("PLURORA_DATA_DIR", data_dir.display().to_string());
        Self { previous }
    }
}

impl Drop for DataDirOverride {
    fn drop(&mut self) {
        match &self.previous {
            Some(previous) => std::env::set_var("PLURORA_DATA_DIR", previous),
            None => std::env::remove_var("PLURORA_DATA_DIR"),
        }
    }
}
