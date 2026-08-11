use std::path::PathBuf;

use anyhow::{anyhow, ensure, Result};
use chrono::{DateTime, Utc};
use clap::{Args, Subcommand, ValueEnum};
use plurora_runtime::{
    DockerBuildBackendSelection, ManagedTargetBuildNetworkMode, OciImageBackendSelection,
    RealizationApplyRequest, RealizationApproval, RealizationBackendSelection,
    RealizationGetRequest, RealizationListRequest, RealizationMutationResult,
    RealizationPlanRequest, RealizationPlanResult, RealizationReconcileRequest,
    RealizationRollbackRequest, RealizationStopRequest,
};
use plurora_work::{RealizationRevision, RealizationStatus};

use super::host_connection;
use super::installation::{call_host_protocol, HostInstallationClient};

#[derive(Args, Debug)]
pub struct RealizationArgs {
    #[command(subcommand)]
    pub command: RealizationCommand,
    #[arg(long, global = true, env = "PLURORA_HOST_URL")]
    pub endpoint: Option<String>,
    #[arg(
        long,
        global = true,
        env = "PLURORA_HTTP_ACCESS_TOKEN",
        hide_env_values = true
    )]
    pub access_token: Option<String>,
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum RealizationCommand {
    /// List visible durable Realization revisions.
    List(RealizationListArgs),
    /// Show one Realization revision.
    Info(RealizationInfoArgs),
    /// Compile an effect-free plan from an exact Installation and Target snapshot.
    Plan(RealizationPlanArgs),
    /// Apply an exact persisted plan after explicit risk approval.
    Apply(RealizationApplyArgs),
    /// Stop one active Realization and release its resources.
    Stop(RealizationStopArgs),
    /// Apply a historic persisted plan as a new Realization and stop the current one.
    Rollback(RealizationRollbackArgs),
    /// Observe Target truth and reconcile one exact revision.
    Reconcile(RealizationReconcileArgs),
}

#[derive(Args, Debug)]
pub struct RealizationListArgs {
    #[arg(long)]
    pub installation_id: Option<String>,
    #[arg(long)]
    pub target_id: Option<String>,
}

#[derive(Args, Debug)]
pub struct RealizationInfoArgs {
    pub installation_id: String,
    pub realization_id: String,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum RealizationBackendArg {
    OciImage,
    DockerBuild,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum BuildNetworkArg {
    None,
    Bridge,
}

#[derive(Args, Debug)]
pub struct RealizationPlanArgs {
    pub installation_id: String,
    #[arg(long)]
    pub installation_revision: u64,
    #[arg(long)]
    pub target_id: String,
    #[arg(long)]
    pub workload_id: String,
    #[arg(long)]
    pub execution_class: String,
    #[arg(long, value_enum)]
    pub backend: RealizationBackendArg,
    /// Content-addressed OCI reference (`name@sha256:...`) for `oci-image`.
    #[arg(long)]
    pub image: Option<String>,
    /// JSON ArtifactDescriptor for an immutable build-context tar.
    #[arg(long)]
    pub build_context_ref: Option<PathBuf>,
    #[arg(long)]
    pub workspace_id: Option<String>,
    #[arg(long)]
    pub dockerfile: Option<String>,
    #[arg(long, value_enum)]
    pub network_mode: Option<BuildNetworkArg>,
    #[arg(long)]
    pub source_tree_digest: Option<String>,
    #[arg(long)]
    pub build_descriptor_hash: Option<String>,
    #[arg(long)]
    pub container_port: u16,
    #[arg(long)]
    pub port_name: String,
    #[arg(long)]
    pub route_id: String,
    #[arg(long)]
    pub public: bool,
    #[arg(long)]
    pub health_path: Option<String>,
    #[arg(long)]
    pub pull_if_missing: bool,
    #[arg(long)]
    pub idempotency_key: String,
}

#[derive(Args, Debug)]
pub struct RealizationApplyArgs {
    pub installation_id: String,
    pub realization_id: String,
    #[arg(long)]
    pub target_id: String,
    #[arg(long)]
    pub revision: u64,
    /// JSON ArtifactDescriptor returned by `realization plan`.
    #[arg(long)]
    pub plan_ref: PathBuf,
    /// Explicitly approve this exact plan and every listed `--accept-risk` value.
    #[arg(long)]
    pub approve: bool,
    #[arg(long = "accept-risk", required = true)]
    pub accepted_risks: Vec<String>,
    #[arg(long)]
    pub approval_expires_at: Option<String>,
    #[arg(long)]
    pub idempotency_key: String,
}

#[derive(Args, Debug)]
pub struct RealizationStopArgs {
    pub installation_id: String,
    pub realization_id: String,
    #[arg(long)]
    pub target_id: String,
    #[arg(long)]
    pub revision: u64,
    #[arg(long)]
    pub idempotency_key: String,
}

#[derive(Args, Debug)]
pub struct RealizationRollbackArgs {
    pub installation_id: String,
    pub realization_id: String,
    #[arg(long)]
    pub target_id: String,
    #[arg(long)]
    pub revision: u64,
    #[arg(long)]
    pub rollback_to_realization_id: String,
    #[arg(long)]
    pub plan_digest: String,
    #[arg(long)]
    pub approve: bool,
    #[arg(long = "accept-risk", required = true)]
    pub accepted_risks: Vec<String>,
    #[arg(long)]
    pub approval_expires_at: Option<String>,
    #[arg(long)]
    pub idempotency_key: String,
}

#[derive(Args, Debug)]
pub struct RealizationReconcileArgs {
    pub installation_id: String,
    pub realization_id: String,
    #[arg(long)]
    pub target_id: String,
    #[arg(long)]
    pub revision: u64,
    #[arg(long)]
    pub idempotency_key: String,
}

pub async fn run(args: RealizationArgs) -> Result<()> {
    let connection = host_connection::resolve(args.endpoint.as_deref())?;
    let client = HostInstallationClient {
        endpoint: connection.endpoint,
        access_token: args.access_token.unwrap_or_default(),
    };
    match args.command {
        RealizationCommand::List(command) => list(&client, command, args.json).await,
        RealizationCommand::Info(command) => info(&client, command, args.json).await,
        RealizationCommand::Plan(command) => plan(&client, command, args.json).await,
        RealizationCommand::Apply(command) => apply(&client, command, args.json).await,
        RealizationCommand::Stop(command) => stop(&client, command, args.json).await,
        RealizationCommand::Rollback(command) => rollback(&client, command, args.json).await,
        RealizationCommand::Reconcile(command) => reconcile(&client, command, args.json).await,
    }
}

async fn list(
    client: &HostInstallationClient,
    args: RealizationListArgs,
    json_output: bool,
) -> Result<()> {
    let values: Vec<RealizationRevision> = call_host_protocol(
        client,
        "host.realization.list",
        RealizationListRequest {
            installation_id: args.installation_id.map(parse_installation).transpose()?,
            target_id: args.target_id,
        },
    )
    .await?;
    if json_output {
        print_json(&values)
    } else {
        println!(
            "{:<36} {:<36} {:<18} {:<8} PLAN",
            "REALIZATION", "INSTALLATION", "STATUS", "REV"
        );
        for value in values {
            println!(
                "{:<36} {:<36} {:<18} {:<8} {}",
                value.realization_id,
                value.installation_id,
                status_label(value.status),
                value.revision,
                value.plan_ref.digest
            );
        }
        Ok(())
    }
}

async fn info(
    client: &HostInstallationClient,
    args: RealizationInfoArgs,
    json_output: bool,
) -> Result<()> {
    let value: RealizationRevision = call_host_protocol(
        client,
        "host.realization.get",
        RealizationGetRequest {
            installation_id: parse_installation(args.installation_id)?,
            realization_id: parse_realization(args.realization_id)?,
        },
    )
    .await?;
    if json_output {
        print_json(&value)
    } else {
        print_human(&value)
    }
}

async fn plan(
    client: &HostInstallationClient,
    args: RealizationPlanArgs,
    json_output: bool,
) -> Result<()> {
    nonempty_key(&args.idempotency_key)?;
    let common_access = if args.public {
        plurora_runtime::ProxyRouteAccess::Public
    } else {
        plurora_runtime::ProxyRouteAccess::HostAuthenticated
    };
    let backend = match args.backend {
        RealizationBackendArg::OciImage => {
            ensure!(
                args.build_context_ref.is_none(),
                "--build-context-ref is only valid for docker-build"
            );
            RealizationBackendSelection::OciImage(OciImageBackendSelection {
                workload_id: args.workload_id,
                execution_class: args.execution_class,
                image: required(args.image, "--image")?,
                container_port: args.container_port,
                port_name: args.port_name,
                route_id: args.route_id,
                route_access: common_access,
                health_path: args.health_path,
                pull_if_missing: args.pull_if_missing,
            })
        }
        RealizationBackendArg::DockerBuild => {
            ensure!(args.image.is_none(), "--image is only valid for oci-image");
            RealizationBackendSelection::DockerBuild(DockerBuildBackendSelection {
                workload_id: args.workload_id,
                execution_class: args.execution_class,
                workspace_id: plurora_work::WorkspaceId::parse(required(
                    args.workspace_id,
                    "--workspace-id",
                )?)
                .map_err(|_| anyhow!("workspace id must be a UUID"))?,
                build_context_ref: super::work::read_artifact_descriptor(&required(
                    args.build_context_ref,
                    "--build-context-ref",
                )?)
                .map_err(|error| anyhow!(error.to_string()))?,
                dockerfile: required(args.dockerfile, "--dockerfile")?,
                network_mode: match args.network_mode.unwrap_or(BuildNetworkArg::None) {
                    BuildNetworkArg::None => ManagedTargetBuildNetworkMode::None,
                    BuildNetworkArg::Bridge => ManagedTargetBuildNetworkMode::Bridge,
                },
                source_tree_digest: required(args.source_tree_digest, "--source-tree-digest")?,
                build_descriptor_hash: required(
                    args.build_descriptor_hash,
                    "--build-descriptor-hash",
                )?,
                container_port: args.container_port,
                port_name: args.port_name,
                route_id: args.route_id,
                route_access: common_access,
                health_path: args.health_path,
            })
        }
    };
    let result: RealizationPlanResult = call_host_protocol(
        client,
        "host.realization.plan",
        RealizationPlanRequest {
            installation_id: parse_installation(args.installation_id)?,
            expected_installation_revision: args.installation_revision,
            target_id: args.target_id,
            backends: vec![backend],
            idempotency_key: args.idempotency_key,
        },
    )
    .await?;
    if json_output {
        print_json(&result)
    } else if result.gaps.is_empty() {
        println!("Realization plan: ready");
        println!("Idempotent replay: {}", result.replayed);
        if let Some(realization) = result.realization.as_ref() {
            print_human(realization)?;
        }
        if let Some(plan_ref) = result.plan_ref.as_ref() {
            println!("Plan digest: {}", plan_ref.digest);
            println!("Plan descriptor: {}", serde_json::to_string(plan_ref)?);
        }
        if let Some(plan) = result.plan.as_ref() {
            println!("Risks requiring approval: {}", plan.risk_summary.join(", "));
        }
        Ok(())
    } else {
        println!("Realization plan: blocked");
        for gap in result.gaps {
            println!("  {}: {}", gap.reason_code, gap.next_step);
        }
        Ok(())
    }
}

async fn apply(
    client: &HostInstallationClient,
    args: RealizationApplyArgs,
    json_output: bool,
) -> Result<()> {
    ensure!(
        args.approve,
        "--approve is required for Realization effects"
    );
    nonempty_key(&args.idempotency_key)?;
    let plan_ref = super::work::read_artifact_descriptor(&args.plan_ref)
        .map_err(|error| anyhow!(error.to_string()))?;
    let accepted_risks = args.accepted_risks.clone();
    let request = RealizationApplyRequest {
        installation_id: parse_installation(args.installation_id)?,
        target_id: args.target_id,
        realization_id: parse_realization(args.realization_id)?,
        expected_revision: args.revision,
        approval: approval(
            plan_ref.digest.clone(),
            accepted_risks,
            args.approval_expires_at.as_deref(),
        )?,
        plan_ref,
        idempotency_key: args.idempotency_key,
    };
    mutation(client, "host.realization.apply", request, json_output).await
}

async fn stop(
    client: &HostInstallationClient,
    args: RealizationStopArgs,
    json_output: bool,
) -> Result<()> {
    nonempty_key(&args.idempotency_key)?;
    mutation(
        client,
        "host.realization.stop",
        RealizationStopRequest {
            installation_id: parse_installation(args.installation_id)?,
            target_id: args.target_id,
            realization_id: parse_realization(args.realization_id)?,
            expected_revision: args.revision,
            idempotency_key: args.idempotency_key,
        },
        json_output,
    )
    .await
}

async fn rollback(
    client: &HostInstallationClient,
    args: RealizationRollbackArgs,
    json_output: bool,
) -> Result<()> {
    ensure!(args.approve, "--approve is required for rollback effects");
    nonempty_key(&args.idempotency_key)?;
    let approval = approval(
        args.plan_digest,
        args.accepted_risks,
        args.approval_expires_at.as_deref(),
    )?;
    mutation(
        client,
        "host.realization.rollback",
        RealizationRollbackRequest {
            installation_id: parse_installation(args.installation_id)?,
            target_id: args.target_id,
            realization_id: parse_realization(args.realization_id)?,
            expected_revision: args.revision,
            rollback_to_realization_id: parse_realization(args.rollback_to_realization_id)?,
            approval,
            idempotency_key: args.idempotency_key,
        },
        json_output,
    )
    .await
}

async fn reconcile(
    client: &HostInstallationClient,
    args: RealizationReconcileArgs,
    json_output: bool,
) -> Result<()> {
    nonempty_key(&args.idempotency_key)?;
    mutation(
        client,
        "host.realization.reconcile",
        RealizationReconcileRequest {
            installation_id: parse_installation(args.installation_id)?,
            target_id: args.target_id,
            realization_id: parse_realization(args.realization_id)?,
            expected_revision: args.revision,
            idempotency_key: args.idempotency_key,
        },
        json_output,
    )
    .await
}

async fn mutation<T: serde::Serialize>(
    client: &HostInstallationClient,
    method: &str,
    request: T,
    json_output: bool,
) -> Result<()> {
    let result: RealizationMutationResult = call_host_protocol(client, method, request).await?;
    if json_output {
        print_json(&result)
    } else {
        println!(
            "Realization mutation: {}",
            status_label(result.realization.status)
        );
        println!("Idempotent replay: {}", result.replayed);
        print_human(&result.realization)?;
        for gap in result.gaps {
            println!("  {}: {}", gap.reason_code, gap.next_step);
        }
        Ok(())
    }
}

fn approval(
    plan_digest: String,
    accepted_risks: Vec<String>,
    expires_at: Option<&str>,
) -> Result<RealizationApproval> {
    Ok(RealizationApproval {
        plan_digest,
        decision: "approved".to_string(),
        accepted_risks,
        decided_at: Utc::now(),
        expires_at: expires_at.map(parse_time).transpose()?,
    })
}

fn parse_time(value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| anyhow!("approval expiry must be RFC3339"))
}

fn print_human(value: &RealizationRevision) -> Result<()> {
    println!("Realization: {}", value.realization_id);
    println!("Installation: {}", value.installation_id);
    println!("Revision: {}", value.revision);
    println!("Status: {}", status_label(value.status));
    println!("Plan: {}", value.plan_ref.digest);
    println!("Resources: {}", value.actual_resources.len());
    println!("Receipts: {}", value.receipts.len());
    if let Some(reason) = value.health.reason_code.as_deref() {
        println!("Reason: {reason}");
    }
    Ok(())
}

fn status_label(status: RealizationStatus) -> &'static str {
    match status {
        RealizationStatus::Planned => "planned",
        RealizationStatus::Applying => "applying",
        RealizationStatus::Active => "active",
        RealizationStatus::Degraded => "degraded",
        RealizationStatus::Stopping => "stopping",
        RealizationStatus::Stopped => "stopped",
        RealizationStatus::Failed => "failed",
        RealizationStatus::OutcomeUnknown => "outcome_unknown",
        RealizationStatus::RecoveryRequired => "recovery_required",
    }
}

fn parse_installation(value: String) -> Result<plurora_work::InstallationId> {
    plurora_work::InstallationId::parse(value)
        .map_err(|_| anyhow!("installation id must be a UUID"))
}

fn parse_realization(value: String) -> Result<plurora_work::RealizationId> {
    plurora_work::RealizationId::parse(value).map_err(|_| anyhow!("realization id must be a UUID"))
}

fn nonempty_key(value: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty(),
        "--idempotency-key must not be empty"
    );
    Ok(())
}

fn required<T>(value: Option<T>, name: &str) -> Result<T> {
    value.ok_or_else(|| anyhow!("{name} is required for the selected backend"))
}

fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
