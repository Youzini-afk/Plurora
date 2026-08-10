use anyhow::{anyhow, ensure, Result};
use clap::{Args, Subcommand, ValueEnum};
use plurora_runtime::{
    RunGetRequest, RunListRequest, RunStartRequest, RunStartResult, RunStatusRequest,
    RunStatusView, RunStopRequest, RunView,
};
use plurora_work::RunStatus;
use serde_json::json;

use super::host_connection;
use super::installation::{call_host_protocol, HostInstallationClient};

/// Public CLI entrypoint for the Host-owned Run lifecycle.
#[derive(Args, Debug)]
pub struct RunArgs {
    #[command(subcommand)]
    pub command: RunCommand,

    /// Host origin. Falls back to the selected `host connection` profile, then loopback.
    #[arg(long, global = true, env = "PLURORA_HOST_URL")]
    pub endpoint: Option<String>,

    /// Host root or device access token. It is never persisted by this command.
    #[arg(
        long,
        global = true,
        env = "PLURORA_HTTP_ACCESS_TOKEN",
        hide_env_values = true
    )]
    pub access_token: Option<String>,

    /// Emit structured JSON instead of the human-readable table/detail view.
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum RunCommand {
    /// List Runs visible to the selected Host.
    List(RunListArgs),
    /// Show one Run record.
    Info(RunInfoArgs),
    /// Perform an effect-free Installation-level preflight/status check.
    Status(RunStatusArgs),
    /// Start a Run from an exact Installation revision and entrypoint.
    Start(RunStartArgs),
    /// Stop one Run from an exact Installation and Run revision.
    Stop(RunStopArgs),
}

#[derive(Args, Debug)]
pub struct RunListArgs {
    #[arg(long)]
    pub installation_id: Option<String>,
    #[arg(long, value_enum)]
    pub status: Option<RunStatusArg>,
}

#[derive(Args, Debug)]
pub struct RunInfoArgs {
    pub installation_id: String,
    pub run_id: String,
}

#[derive(Args, Debug)]
pub struct RunStatusArgs {
    pub installation_id: String,
    #[arg(long)]
    pub entrypoint_id: Option<String>,
}

#[derive(Args, Debug)]
pub struct RunStartArgs {
    pub installation_id: String,
    /// Current CAS revision returned by `installation info` or `run status`.
    #[arg(long)]
    pub revision: u64,
    #[arg(long)]
    pub entrypoint_id: String,
    /// Stable retry key. Reuse the same key only for the same mutation.
    #[arg(long)]
    pub idempotency_key: String,
}

#[derive(Args, Debug)]
pub struct RunStopArgs {
    pub installation_id: String,
    pub run_id: String,
    /// Current Run CAS revision returned by `run info` or `run list`.
    #[arg(long)]
    pub revision: u64,
    /// Stable retry key. Reuse the same key only for the same mutation.
    #[arg(long)]
    pub idempotency_key: String,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum RunStatusArg {
    Starting,
    Running,
    Degraded,
    Stopping,
    Stopped,
    Failed,
    Interrupted,
}

impl From<RunStatusArg> for RunStatus {
    fn from(value: RunStatusArg) -> Self {
        match value {
            RunStatusArg::Starting => Self::Starting,
            RunStatusArg::Running => Self::Running,
            RunStatusArg::Degraded => Self::Degraded,
            RunStatusArg::Stopping => Self::Stopping,
            RunStatusArg::Stopped => Self::Stopped,
            RunStatusArg::Failed => Self::Failed,
            RunStatusArg::Interrupted => Self::Interrupted,
        }
    }
}

pub async fn run(args: RunArgs) -> Result<()> {
    let connection = host_connection::resolve(args.endpoint.as_deref())?;
    let client = HostInstallationClient {
        endpoint: connection.endpoint,
        access_token: args.access_token.unwrap_or_default(),
    };
    run_with_client(&client, args.command, args.json).await
}

async fn run_with_client(
    client: &HostInstallationClient,
    command: RunCommand,
    json_output: bool,
) -> Result<()> {
    match command {
        RunCommand::List(args) => run_list(client, args, json_output).await,
        RunCommand::Info(args) => run_info(client, args, json_output).await,
        RunCommand::Status(args) => run_status(client, args, json_output).await,
        RunCommand::Start(args) => run_start(client, args, json_output).await,
        RunCommand::Stop(args) => run_stop(client, args, json_output).await,
    }
}

async fn run_list(
    client: &HostInstallationClient,
    args: RunListArgs,
    json_output: bool,
) -> Result<()> {
    let installation_id = args
        .installation_id
        .map(parse_installation_id)
        .transpose()?;
    let status = args.status.map(Into::into);
    let mut runs: Vec<RunView> = call_host_protocol(
        client,
        "host.run.list",
        RunListRequest {
            installation_id,
            status,
        },
    )
    .await?;
    runs.sort_by(|left, right| {
        left.record
            .installation_id
            .cmp(&right.record.installation_id)
            .then_with(|| left.record.run_id.cmp(&right.record.run_id))
    });
    if json_output {
        print_json(&runs)
    } else {
        println!(
            "{:<36} {:<36} {:<12} {:<8} {:<20} HEALTH",
            "RUN ID", "INSTALLATION", "STATUS", "REVISION", "ENTRYPOINT"
        );
        for run in runs {
            println!(
                "{:<36} {:<36} {:<12} {:<8} {:<20} {}",
                run.record.run_id,
                run.record.installation_id,
                run_status_label(run.record.status),
                run.revision,
                truncate(&run.entrypoint_id, 20),
                health_label(run.record.health.status)
            );
        }
        Ok(())
    }
}

async fn run_info(
    client: &HostInstallationClient,
    args: RunInfoArgs,
    json_output: bool,
) -> Result<()> {
    let run: RunView = call_host_protocol(
        client,
        "host.run.get",
        RunGetRequest {
            installation_id: parse_installation_id(args.installation_id)?,
            run_id: parse_run_id(args.run_id)?,
        },
    )
    .await?;
    if json_output {
        print_json(&run)
    } else {
        print_run_human(&run)
    }
}

async fn run_status(
    client: &HostInstallationClient,
    args: RunStatusArgs,
    json_output: bool,
) -> Result<()> {
    let status: RunStatusView = call_host_protocol(
        client,
        "host.run.status",
        RunStatusRequest {
            installation_id: parse_installation_id(args.installation_id)?,
            entrypoint_id: args.entrypoint_id,
        },
    )
    .await?;
    if json_output {
        print_json(&status)
    } else {
        print_status_human(&status)
    }
}

async fn run_start(
    client: &HostInstallationClient,
    args: RunStartArgs,
    json_output: bool,
) -> Result<()> {
    ensure_non_empty_key(&args.idempotency_key)?;
    ensure!(
        !args.entrypoint_id.trim().is_empty(),
        "--entrypoint-id must not be empty"
    );
    let result: RunStartResult = call_host_protocol(
        client,
        "host.run.start",
        RunStartRequest {
            installation_id: parse_installation_id(args.installation_id)?,
            expected_installation_revision: args.revision,
            entrypoint_id: args.entrypoint_id,
            idempotency_key: args.idempotency_key,
            authority: None,
        },
    )
    .await?;

    if !result.gaps.is_empty() {
        // A structured preflight gap is an expected blocked outcome, not an
        // opaque transport error. Keep the result clearly marked blocked while
        // preserving the stable reason_code/node/port/next_step fields.
        if json_output {
            print_json(&json!({
                "status": "blocked",
                "run": result.run,
                "gaps": result.gaps,
                "idempotent": result.idempotent,
            }))
        } else {
            println!("Run start: blocked (preflight)");
            for gap in result.gaps {
                print_gap_human(&gap);
            }
            Ok(())
        }
    } else if json_output {
        print_json(&result)
    } else if let Some(run) = result.run.as_ref() {
        println!("Run start: started");
        println!("Idempotent replay: {}", result.idempotent);
        print_run_human(run)
    } else {
        Err(anyhow!(
            "Host returned a successful Run start without a Run or structured gaps"
        ))
    }
}

async fn run_stop(
    client: &HostInstallationClient,
    args: RunStopArgs,
    json_output: bool,
) -> Result<()> {
    ensure_non_empty_key(&args.idempotency_key)?;
    let result: plurora_runtime::RunMutationResult = call_host_protocol(
        client,
        "host.run.stop",
        RunStopRequest {
            installation_id: parse_installation_id(args.installation_id)?,
            run_id: parse_run_id(args.run_id)?,
            expected_revision: args.revision,
            idempotency_key: args.idempotency_key,
            authority: None,
        },
    )
    .await?;
    if json_output {
        print_json(&result)
    } else {
        println!("Run stop: stopped");
        println!("Idempotent replay: {}", result.idempotent);
        print_run_human(&result.run)
    }
}

fn print_run_human(run: &RunView) -> Result<()> {
    println!("Run: {}", run.record.run_id);
    println!("Installation: {}", run.record.installation_id);
    println!("Revision: {}", run.revision);
    println!("Installation revision: {}", run.installation_revision);
    println!("Entrypoint: {}", run.entrypoint_id);
    println!("Status: {}", run_status_label(run.record.status));
    println!("Health: {}", health_label(run.record.health.status));
    if let Some(reason) = run.record.health.reason_code.as_deref() {
        println!("Health reason: {reason}");
    }
    Ok(())
}

fn print_status_human(status: &RunStatusView) -> Result<()> {
    println!("Installation: {}", status.installation_id);
    println!("Installation revision: {}", status.installation_revision);
    println!("Work revision: {}", status.work_revision.digest);
    match status.active_run.as_ref() {
        Some(run) => print_run_human(run)?,
        None => {
            println!("Active run: none");
            println!("Health: none");
        }
    }
    if let Some(preflight) = status.preflight.as_ref() {
        println!("Entrypoint: {}", preflight.entrypoint_id);
        if preflight.gaps.is_empty() {
            println!("Preflight: ready");
        } else {
            println!("Preflight: blocked");
            for gap in &preflight.gaps {
                print_gap_human(gap);
            }
        }
    }
    Ok(())
}

fn print_gap_human(gap: &plurora_runtime::RunGap) {
    println!("Gap:");
    println!("  reason_code: {}", gap.reason_code);
    println!("  node: {}", gap.node_id.as_deref().unwrap_or("-"));
    println!("  port: {}", gap.port_id.as_deref().unwrap_or("-"));
    println!("  next_step: {}", gap.next_step);
}

fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn parse_installation_id(value: String) -> Result<plurora_work::InstallationId> {
    plurora_work::InstallationId::parse(value)
        .map_err(|_| anyhow!("installation id must be a UUID"))
}

fn parse_run_id(value: String) -> Result<plurora_work::RunId> {
    plurora_work::RunId::parse(value).map_err(|_| anyhow!("run id must be a UUID"))
}

fn ensure_non_empty_key(key: &str) -> Result<()> {
    ensure!(
        !key.trim().is_empty(),
        "--idempotency-key must not be empty"
    );
    Ok(())
}

fn truncate(value: &str, width: usize) -> String {
    let mut chars = value.chars();
    let value = chars.by_ref().take(width).collect::<String>();
    if chars.next().is_some() && width > 1 {
        format!("{}…", value.chars().take(width - 1).collect::<String>())
    } else {
        value
    }
}

fn run_status_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Starting => "starting",
        RunStatus::Running => "running",
        RunStatus::Degraded => "degraded",
        RunStatus::Stopping => "stopping",
        RunStatus::Stopped => "stopped",
        RunStatus::Failed => "failed",
        RunStatus::Interrupted => "interrupted",
    }
}

fn health_label(status: plurora_work::HealthStatus) -> &'static str {
    match status {
        plurora_work::HealthStatus::Unknown => "unknown",
        plurora_work::HealthStatus::Healthy => "healthy",
        plurora_work::HealthStatus::Degraded => "degraded",
        plurora_work::HealthStatus::Unhealthy => "unhealthy",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::routing::post;
    use axum::{Json, Router};
    use clap::Parser;
    use serde_json::{json, Value};
    use tokio::sync::Mutex;

    use super::*;
    use crate::cli::Cli;

    const INSTALLATION_ID: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const RUN_ID: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

    #[test]
    fn run_surface_parses_and_retired_project_start_stop_do_not() {
        for args in vec![
            vec!["plurora", "run", "list", "--json"],
            vec!["plurora", "run", "info", INSTALLATION_ID, RUN_ID, "--json"],
            vec![
                "plurora",
                "run",
                "status",
                INSTALLATION_ID,
                "--entrypoint-id",
                "play",
            ],
            vec![
                "plurora",
                "run",
                "start",
                INSTALLATION_ID,
                "--revision",
                "7",
                "--entrypoint-id",
                "play",
                "--idempotency-key",
                "start-1",
            ],
            vec![
                "plurora",
                "run",
                "stop",
                INSTALLATION_ID,
                RUN_ID,
                "--revision",
                "9",
                "--idempotency-key",
                "stop-1",
            ],
        ] {
            assert!(Cli::try_parse_from(args).is_ok());
        }
        for args in [
            vec!["plurora", "project", "start", INSTALLATION_ID],
            vec!["plurora", "project", "stop", INSTALLATION_ID, RUN_ID],
        ] {
            assert!(
                Cli::try_parse_from(args).is_err(),
                "retired Project start/stop command unexpectedly parsed"
            );
        }
    }

    #[tokio::test]
    async fn run_requests_use_exact_public_wire_ids_and_fields() -> Result<()> {
        let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let server_requests = requests.clone();
        let app = Router::new().route(
            "/rpc",
            post(move |Json(request): Json<Value>| {
                let requests = server_requests.clone();
                async move {
                    requests.lock().await.push(request.clone());
                    let method = request["method"].as_str().unwrap_or_default();
                    let result = match method {
                        "host.run.list" => json!([]),
                        "host.run.get" => fixture_run_view(),
                        "host.run.status" => json!({
                            "installation_id": INSTALLATION_ID,
                            "installation_revision": 7,
                            "work_revision": fixture_descriptor(),
                            "active_run": null,
                            "preflight": null
                        }),
                        "host.run.start" => json!({
                            "run": fixture_run_view(),
                            "gaps": [],
                            "idempotent": false
                        }),
                        "host.run.stop" => json!({
                            "run": fixture_run_view(),
                            "idempotent": false
                        }),
                        _ => json!({}),
                    };
                    Json(json!({"id": request["id"], "result": result}))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        let client = HostInstallationClient {
            endpoint: format!("http://{address}"),
            access_token: "token-that-must-not-be-printed".to_string(),
        };

        run_with_client(
            &client,
            RunCommand::List(RunListArgs {
                installation_id: None,
                status: None,
            }),
            true,
        )
        .await?;
        run_with_client(
            &client,
            RunCommand::Info(RunInfoArgs {
                installation_id: INSTALLATION_ID.to_string(),
                run_id: RUN_ID.to_string(),
            }),
            true,
        )
        .await?;
        run_with_client(
            &client,
            RunCommand::Status(RunStatusArgs {
                installation_id: INSTALLATION_ID.to_string(),
                entrypoint_id: Some("play".to_string()),
            }),
            true,
        )
        .await?;
        run_with_client(
            &client,
            RunCommand::Start(RunStartArgs {
                installation_id: INSTALLATION_ID.to_string(),
                revision: 7,
                entrypoint_id: "play".to_string(),
                idempotency_key: "start-1".to_string(),
            }),
            true,
        )
        .await?;
        run_with_client(
            &client,
            RunCommand::Stop(RunStopArgs {
                installation_id: INSTALLATION_ID.to_string(),
                run_id: RUN_ID.to_string(),
                revision: 9,
                idempotency_key: "stop-1".to_string(),
            }),
            true,
        )
        .await?;

        let requests = requests.lock().await;
        let params = requests
            .iter()
            .map(|request| request["params"].clone())
            .collect::<Vec<_>>();
        assert_eq!(
            params,
            vec![
                json!({}),
                json!({"installation_id": INSTALLATION_ID, "run_id": RUN_ID}),
                json!({"installation_id": INSTALLATION_ID, "entrypoint_id": "play"}),
                json!({
                    "installation_id": INSTALLATION_ID,
                    "expected_installation_revision": 7,
                    "entrypoint_id": "play",
                    "idempotency_key": "start-1"
                }),
                json!({
                    "installation_id": INSTALLATION_ID,
                    "run_id": RUN_ID,
                    "expected_revision": 9,
                    "idempotency_key": "stop-1"
                })
            ]
        );
        let wire = serde_json::to_string(&*requests)?;
        assert!(!wire.contains("token-that-must-not-be-printed"));
        assert!(!wire.contains("private-local-path"));
        server.abort();
        Ok(())
    }

    #[test]
    fn blocked_start_result_is_structured_and_has_no_mutation_fields() -> Result<()> {
        let result = RunStartResult {
            run: None,
            gaps: vec![plurora_runtime::RunGap::new(
                "target_unsatisfied",
                "create a RealizationPlan",
            )
            .for_node("server")
            .for_port("http")],
            idempotent: false,
        };
        let output = serde_json::to_string(&json!({
            "status": "blocked",
            "run": result.run,
            "gaps": result.gaps,
            "idempotent": result.idempotent,
        }))?;
        assert!(output.contains("target_unsatisfied"));
        assert!(output.contains("server"));
        assert!(output.contains("http"));
        assert!(output.contains("create a RealizationPlan"));
        assert!(!output.contains("idempotency_key"));
        Ok(())
    }

    fn fixture_descriptor() -> Value {
        json!({
            "artifact_type_uri": "urn:plurora:work-revision:v1",
            "media_type": "application/json",
            "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "size_bytes": 1,
            "references": [],
            "annotations": {}
        })
    }

    fn fixture_run_view() -> Value {
        json!({
            "record": {
                "run_id": RUN_ID,
                "installation_id": INSTALLATION_ID,
                "status": "running",
                "node_instances": [],
                "bindings": [],
                "started_at": "2026-08-10T00:00:00Z",
                "health": {"status": "healthy", "diagnostic_refs": []}
            },
            "revision": 9,
            "installation_revision": 7,
            "entrypoint_id": "play"
        })
    }
}
