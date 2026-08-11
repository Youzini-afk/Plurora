use anyhow::{anyhow, ensure, Result};
use chrono::{DateTime, Utc};
use clap::{ArgGroup, Args, Subcommand, ValueEnum};
use plurora_runtime::{
    BindingCandidate, BindingCandidatesRequest, BindingCandidatesResult,
    BindingComponentDisclosure, BindingDecisionStatus, BindingInstallationDisclosure,
    BindingListRequest, BindingMutationResult, BindingRevokeRequest, BindingSelectRequest,
    BindingView, BindingWorkDisclosure, CapabilityPin, ExposureCreateRequest, ExposureListRequest,
    ExposureMutationResult, ExposureRevokeRequest, ExposureView, RunRevisionPin,
};
use plurora_work::{
    BindingId, BindingPhase, ExposureId, ExposureStatus, InstallationId, PortDescriptor, PortId,
    ResourceSelector, RunId, SelectedTransport,
};
use serde::Serialize;

use super::host_connection;
use super::installation::{call_host_protocol, HostInstallationClient};

#[derive(Args, Debug)]
pub struct ExposureArgs {
    #[command(subcommand)]
    pub command: ExposureCommand,
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
pub enum ExposureCommand {
    /// List visible public Exposures without changing Host state.
    List(ExposureListArgs),
    /// Create an Exposure from an exact provider Installation, Run, and Port revision.
    Create(ExposureCreateArgs),
    /// Revoke an exact Exposure revision.
    Revoke(ExposureRevokeArgs),
}

#[derive(Args, Debug)]
pub struct ExposureListArgs {
    #[arg(long)]
    pub installation_id: Option<String>,
    #[arg(long)]
    pub run_id: Option<String>,
    #[arg(long, value_enum)]
    pub status: Option<ExposureStatusArg>,
}

#[derive(Args, Debug)]
#[command(group(ArgGroup::new("expiry").required(true).multiple(false).args(["expires_at", "no_expiry"])))]
pub struct ExposureCreateArgs {
    pub installation_id: String,
    #[arg(long)]
    pub installation_revision: u64,
    #[arg(long)]
    pub run_id: String,
    #[arg(long)]
    pub run_revision: u64,
    #[arg(long)]
    pub export_port: String,
    /// Exact audience selector in KIND:ID form. Repeat for more audiences.
    #[arg(long, required = true)]
    pub audience: Vec<String>,
    #[arg(long)]
    pub expires_at: Option<String>,
    #[arg(long)]
    pub no_expiry: bool,
    #[arg(long)]
    pub idempotency_key: String,
}

#[derive(Args, Debug)]
pub struct ExposureRevokeArgs {
    pub installation_id: String,
    #[arg(long)]
    pub installation_revision: u64,
    #[arg(long)]
    pub run_id: String,
    #[arg(long)]
    pub run_revision: u64,
    #[arg(long)]
    pub export_port: String,
    #[arg(long)]
    pub exposure_id: String,
    #[arg(long)]
    pub exposure_revision: u64,
    #[arg(long)]
    pub idempotency_key: String,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ExposureStatusArg {
    Active,
    Expired,
    Revoked,
}

impl From<ExposureStatusArg> for ExposureStatus {
    fn from(value: ExposureStatusArg) -> Self {
        match value {
            ExposureStatusArg::Active => Self::Active,
            ExposureStatusArg::Expired => Self::Expired,
            ExposureStatusArg::Revoked => Self::Revoked,
        }
    }
}

#[derive(Args, Debug)]
pub struct BindingArgs {
    #[command(subcommand)]
    pub command: BindingCommand,
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
pub enum BindingCommand {
    /// List visible public Binding decisions without changing Host state.
    List(BindingListArgs),
    /// List eligible provider candidates; this never selects one.
    Candidates(BindingCandidatesArgs),
    /// Select one explicit Exposure revision and candidate digest.
    Select(BindingSelectArgs),
    /// Revoke one exact Binding revision.
    Revoke(BindingRevokeArgs),
}

#[derive(Args, Debug)]
pub struct BindingListArgs {
    #[arg(long)]
    pub consumer_installation_id: Option<String>,
    #[arg(long)]
    pub run_id: Option<String>,
    #[arg(long, value_enum)]
    pub status: Option<BindingStatusArg>,
}

#[derive(Args, Debug)]
pub struct BindingCandidatesArgs {
    pub consumer_installation_id: String,
    #[arg(long)]
    pub consumer_installation_revision: u64,
    #[arg(long)]
    pub import_port: String,
    #[arg(long, value_enum)]
    pub phase: PublicBindingPhaseArg,
    #[command(flatten)]
    pub run: ConsumerRunArgs,
    /// Exact preference selector in KIND:ID form. Preferences only affect ordering.
    #[arg(long)]
    pub preference: Vec<String>,
}

#[derive(Args, Debug)]
pub struct BindingSelectArgs {
    pub consumer_installation_id: String,
    #[arg(long)]
    pub consumer_installation_revision: u64,
    #[arg(long)]
    pub import_port: String,
    #[arg(long, value_enum)]
    pub phase: PublicBindingPhaseArg,
    #[command(flatten)]
    pub run: ConsumerRunArgs,
    #[arg(long)]
    pub exposure_id: String,
    #[arg(long)]
    pub exposure_revision: u64,
    #[arg(long)]
    pub provider_installation_id: String,
    #[arg(long)]
    pub provider_installation_revision: u64,
    #[arg(long)]
    pub candidate_digest: String,
    #[arg(long)]
    pub idempotency_key: String,
}

#[derive(Args, Debug)]
pub struct BindingRevokeArgs {
    pub consumer_installation_id: String,
    #[arg(long)]
    pub consumer_installation_revision: u64,
    #[arg(long)]
    pub import_port: String,
    #[arg(long, value_enum)]
    pub phase: PublicBindingPhaseArg,
    #[command(flatten)]
    pub run: ConsumerRunArgs,
    #[arg(long)]
    pub exposure_id: String,
    #[arg(long)]
    pub binding_id: String,
    #[arg(long)]
    pub binding_revision: u64,
    #[arg(long)]
    pub idempotency_key: String,
}

#[derive(Args, Debug, Default)]
pub struct ConsumerRunArgs {
    #[arg(long)]
    pub run_id: Option<String>,
    #[arg(long)]
    pub run_revision: Option<u64>,
    #[arg(long)]
    pub context_id: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum PublicBindingPhaseArg {
    Launch,
    Runtime,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum BindingStatusArg {
    Selected,
    Revoked,
    Expired,
}

impl From<BindingStatusArg> for BindingDecisionStatus {
    fn from(value: BindingStatusArg) -> Self {
        match value {
            BindingStatusArg::Selected => Self::Selected,
            BindingStatusArg::Revoked => Self::Revoked,
            BindingStatusArg::Expired => Self::Expired,
        }
    }
}

#[derive(Serialize)]
struct CandidateOutput {
    candidate_digest: String,
    phase: BindingPhase,
    exposure: ExposureView,
    consumer_port: PortDescriptor,
    provider_port: PortDescriptor,
    provider_work: BindingWorkDisclosure,
    provider_installation: BindingInstallationDisclosure,
    provider_component: BindingComponentDisclosure,
    capability: CapabilityPin,
    selected_transport: SelectedTransport,
    scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    effective_expires_at: Option<DateTime<Utc>>,
    next_step: String,
}

#[derive(Serialize)]
struct CandidatesOutput {
    candidates: Vec<CandidateOutput>,
    gaps: Vec<plurora_runtime::BindingGap>,
    selection_performed: bool,
}

pub async fn run_exposure(args: ExposureArgs) -> Result<()> {
    let client = client(args.endpoint.as_deref(), args.access_token)?;
    match args.command {
        ExposureCommand::List(command) => exposure_list(&client, command, args.json).await,
        ExposureCommand::Create(command) => exposure_create(&client, command, args.json).await,
        ExposureCommand::Revoke(command) => exposure_revoke(&client, command, args.json).await,
    }
}

pub async fn run_binding(args: BindingArgs) -> Result<()> {
    let client = client(args.endpoint.as_deref(), args.access_token)?;
    match args.command {
        BindingCommand::List(command) => binding_list(&client, command, args.json).await,
        BindingCommand::Candidates(command) => {
            binding_candidates(&client, command, args.json).await
        }
        BindingCommand::Select(command) => binding_select(&client, command, args.json).await,
        BindingCommand::Revoke(command) => binding_revoke(&client, command, args.json).await,
    }
}

fn client(endpoint: Option<&str>, access_token: Option<String>) -> Result<HostInstallationClient> {
    let connection = host_connection::resolve(endpoint)?;
    Ok(HostInstallationClient {
        endpoint: connection.endpoint,
        access_token: access_token.unwrap_or_default(),
    })
}

async fn exposure_list(
    client: &HostInstallationClient,
    args: ExposureListArgs,
    json: bool,
) -> Result<()> {
    let mut values: Vec<ExposureView> = call_host_protocol(
        client,
        "host.exposure.list",
        ExposureListRequest {
            installation_id: args
                .installation_id
                .map(parse_installation_id)
                .transpose()?,
            run_id: args.run_id.map(parse_run_id).transpose()?,
            status: args.status.map(Into::into),
        },
    )
    .await?;
    values.sort_by(|a, b| a.record.exposure_id.cmp(&b.record.exposure_id));
    if json {
        return print_json(&values);
    }
    println!(
        "{:<36} {:<36} {:<20} {:<10} REVISION",
        "EXPOSURE", "INSTALLATION", "EXPORT PORT", "STATUS"
    );
    for value in values {
        println!(
            "{:<36} {:<36} {:<20} {:<10} {}",
            value.record.exposure_id,
            value.record.installation_id,
            value.record.export_port,
            enum_label(&value.record.status),
            value.revision
        );
    }
    Ok(())
}

async fn exposure_create(
    client: &HostInstallationClient,
    args: ExposureCreateArgs,
    json: bool,
) -> Result<()> {
    require_revision(args.installation_revision, "--installation-revision")?;
    require_revision(args.run_revision, "--run-revision")?;
    require_key(&args.idempotency_key)?;
    let expires_at = args.expires_at.map(parse_timestamp).transpose()?;
    let result: ExposureMutationResult = call_host_protocol(
        client,
        "host.exposure.create",
        ExposureCreateRequest {
            installation_id: parse_installation_id(args.installation_id)?,
            expected_installation_revision: args.installation_revision,
            run_id: parse_run_id(args.run_id)?,
            expected_run_revision: args.run_revision,
            export_port: parse_port_id(args.export_port)?,
            audience: args
                .audience
                .into_iter()
                .map(parse_selector)
                .collect::<Result<_>>()?,
            expires_at,
            idempotency_key: args.idempotency_key,
            authority: None,
        },
    )
    .await?;
    print_mutation(
        "created",
        &result.exposure,
        result.idempotent,
        &result.affected_binding_ids,
        json,
    )
}

async fn exposure_revoke(
    client: &HostInstallationClient,
    args: ExposureRevokeArgs,
    json: bool,
) -> Result<()> {
    require_revision(args.installation_revision, "--installation-revision")?;
    require_revision(args.run_revision, "--run-revision")?;
    require_revision(args.exposure_revision, "--exposure-revision")?;
    require_key(&args.idempotency_key)?;
    let result: ExposureMutationResult = call_host_protocol(
        client,
        "host.exposure.revoke",
        ExposureRevokeRequest {
            installation_id: parse_installation_id(args.installation_id)?,
            expected_installation_revision: args.installation_revision,
            run_id: parse_run_id(args.run_id)?,
            expected_run_revision: args.run_revision,
            export_port: parse_port_id(args.export_port)?,
            exposure_id: parse_exposure_id(args.exposure_id)?,
            expected_exposure_revision: args.exposure_revision,
            idempotency_key: args.idempotency_key,
            authority: None,
        },
    )
    .await?;
    print_mutation(
        "revoked",
        &result.exposure,
        result.idempotent,
        &result.affected_binding_ids,
        json,
    )
}

async fn binding_list(
    client: &HostInstallationClient,
    args: BindingListArgs,
    json: bool,
) -> Result<()> {
    let mut values: Vec<BindingView> = call_host_protocol(
        client,
        "host.binding.list",
        BindingListRequest {
            consumer_installation_id: args
                .consumer_installation_id
                .map(parse_installation_id)
                .transpose()?,
            run_id: args.run_id.map(parse_run_id).transpose()?,
            status: args.status.map(Into::into),
            query: None,
        },
    )
    .await?;
    values.sort_by(|a, b| a.record.binding_id.cmp(&b.record.binding_id));
    if json {
        return print_json(&values);
    }
    println!(
        "{:<36} {:<36} {:<20} {:<10} REVISION",
        "BINDING", "CONSUMER", "IMPORT PORT", "STATUS"
    );
    for value in values {
        println!(
            "{:<36} {:<36} {:<20} {:<10} {}",
            value.record.binding_id,
            value.record.consumer.installation.installation_id,
            value.record.consumer.port.root_port,
            enum_label(&value.record.status),
            value.revision
        );
    }
    Ok(())
}

async fn binding_candidates(
    client: &HostInstallationClient,
    args: BindingCandidatesArgs,
    json: bool,
) -> Result<()> {
    require_revision(
        args.consumer_installation_revision,
        "--consumer-installation-revision",
    )?;
    let phase = args.phase;
    let result: BindingCandidatesResult = call_host_protocol(
        client,
        "host.binding.candidates",
        BindingCandidatesRequest {
            consumer_installation_id: parse_installation_id(args.consumer_installation_id)?,
            expected_consumer_installation_revision: args.consumer_installation_revision,
            phase: phase.into(),
            consumer_run: consumer_run(phase, args.run)?,
            import_port: parse_port_id(args.import_port)?,
            preferences: args
                .preference
                .into_iter()
                .map(parse_selector)
                .collect::<Result<_>>()?,
            query: None,
        },
    )
    .await?;
    let output = CandidatesOutput {
        candidates: result
            .candidates
            .into_iter()
            .map(candidate_output)
            .collect(),
        gaps: result.gaps,
        selection_performed: false,
    };
    if json {
        return print_json(&output);
    }
    println!(
        "{:<71} {:<24} {:<24} {:<22} {:<18}",
        "CANDIDATE DIGEST", "PROTOCOL", "INTERFACE", "INTERACTION", "EFFECT"
    );
    for candidate in &output.candidates {
        let contract = &candidate.provider_port.contract;
        println!(
            "{:<71} {:<24} {:<24} {:<22} {:<18}",
            candidate.candidate_digest,
            contract.protocol_id,
            contract.interface_id,
            candidate.provider_port.interaction.0,
            provider_effect(&candidate.provider_port.role),
        );
        println!(
            "  version={} profiles={} audience={} scope={} exposure_expires_at={} effective_expires_at={}",
            contract.version,
            json_value(&contract.profiles),
            json_value(&candidate.exposure.record.audience),
            candidate.scope,
            optional_timestamp(candidate.exposure.record.expires_at),
            optional_timestamp(candidate.effective_expires_at),
        );
        println!(
            "  origin=work:{} title={:?} installation:{} display={:?} source={} package:{} component:{}@{}",
            candidate.provider_work.work_id,
            candidate.provider_work.title,
            candidate.provider_installation.installation_id,
            candidate.provider_installation.display_name,
            enum_label(&candidate.provider_installation.source.kind),
            candidate.provider_component.package_id,
            candidate.provider_component.component_id,
            candidate.provider_component.version,
        );
        println!(
            "  trust={} claim={} boundaries={} evidence=component:{},behavior:{},protocols:{},provenance:{}",
            enum_label(&candidate.provider_component.trust_class),
            enum_label(&candidate.provider_component.claim_status),
            json_value(&candidate.provider_component.enforced_boundaries),
            candidate.provider_component.component_artifact.digest,
            candidate.provider_component.behavior.digest,
            candidate.provider_component.protocol_implementations.len(),
            candidate.provider_installation.source.provenance_refs.len(),
        );
        println!(
            "  transport={} consumer_requirements={} provider_requirements={} next_step={}",
            candidate.selected_transport.class_id,
            json_value(&candidate.consumer_port.transport),
            json_value(&candidate.provider_port.transport),
            candidate.next_step,
        );
    }
    for gap in &output.gaps {
        println!("gap {}: {}", gap.reason_code, gap.next_step);
    }
    println!("No Binding was selected. Use `plurora binding select` with an explicit Exposure and candidate digest.");
    Ok(())
}

async fn binding_select(
    client: &HostInstallationClient,
    args: BindingSelectArgs,
    json: bool,
) -> Result<()> {
    require_revision(
        args.consumer_installation_revision,
        "--consumer-installation-revision",
    )?;
    require_revision(args.exposure_revision, "--exposure-revision")?;
    require_revision(
        args.provider_installation_revision,
        "--provider-installation-revision",
    )?;
    require_key(&args.idempotency_key)?;
    ensure!(
        !args.candidate_digest.trim().is_empty(),
        "--candidate-digest must not be empty"
    );
    let result: BindingMutationResult = call_host_protocol(
        client,
        "host.binding.select",
        BindingSelectRequest {
            consumer_installation_id: parse_installation_id(args.consumer_installation_id)?,
            expected_consumer_installation_revision: args.consumer_installation_revision,
            phase: args.phase.into(),
            consumer_run: consumer_run(args.phase, args.run)?,
            import_port: parse_port_id(args.import_port)?,
            exposure_id: parse_exposure_id(args.exposure_id)?,
            expected_exposure_revision: args.exposure_revision,
            provider_installation_id: parse_installation_id(args.provider_installation_id)?,
            expected_provider_installation_revision: args.provider_installation_revision,
            candidate_digest: args.candidate_digest,
            idempotency_key: args.idempotency_key,
            query: None,
            authority: None,
        },
    )
    .await?;
    print_binding_mutation("selected", result, json)
}

async fn binding_revoke(
    client: &HostInstallationClient,
    args: BindingRevokeArgs,
    json: bool,
) -> Result<()> {
    require_revision(
        args.consumer_installation_revision,
        "--consumer-installation-revision",
    )?;
    require_revision(args.binding_revision, "--binding-revision")?;
    require_key(&args.idempotency_key)?;
    let result: BindingMutationResult = call_host_protocol(
        client,
        "host.binding.revoke",
        BindingRevokeRequest {
            consumer_installation_id: parse_installation_id(args.consumer_installation_id)?,
            expected_consumer_installation_revision: args.consumer_installation_revision,
            consumer_run: consumer_run(args.phase, args.run)?,
            import_port: parse_port_id(args.import_port)?,
            exposure_id: parse_exposure_id(args.exposure_id)?,
            binding_id: parse_binding_id(args.binding_id)?,
            expected_binding_revision: args.binding_revision,
            idempotency_key: args.idempotency_key,
            authority: None,
        },
    )
    .await?;
    print_binding_mutation("revoked", result, json)
}

fn candidate_output(candidate: BindingCandidate) -> CandidateOutput {
    let exposure_id = candidate.exposure.record.exposure_id.clone();
    let exposure_revision = candidate.exposure.revision;
    let scope = format!(
        "installation:{}/run:{}/port:{}/exposure:{}",
        candidate.provider.installation.installation_id,
        candidate
            .provider
            .run
            .as_ref()
            .map(|run| run.run_id.to_string())
            .unwrap_or_else(|| "none".to_string()),
        candidate.provider.port.root_port,
        exposure_id,
    );
    let next_step = format!(
        "plurora binding select --exposure-id {} --exposure-revision {} --candidate-digest {}",
        exposure_id, exposure_revision, candidate.candidate_digest
    );
    CandidateOutput {
        candidate_digest: candidate.candidate_digest,
        phase: candidate.phase,
        exposure: candidate.exposure,
        consumer_port: candidate.consumer_port,
        provider_port: candidate.provider_port,
        provider_work: candidate.provider_work,
        provider_installation: candidate.provider_installation,
        provider_component: candidate.provider_component,
        capability: candidate.capability,
        selected_transport: candidate.transport,
        scope,
        effective_expires_at: candidate.effective_expires_at,
        next_step,
    }
}

impl From<PublicBindingPhaseArg> for BindingPhase {
    fn from(value: PublicBindingPhaseArg) -> Self {
        match value {
            PublicBindingPhaseArg::Launch => Self::Launch,
            PublicBindingPhaseArg::Runtime => Self::Runtime,
        }
    }
}

fn provider_effect(role: &plurora_work::PortRole) -> String {
    match role {
        plurora_work::PortRole::Export { effect_class, .. } => enum_label(effect_class),
        plurora_work::PortRole::Import {
            accepted_effects, ..
        } => json_value(accepted_effects),
    }
}

fn optional_timestamp(value: Option<DateTime<Utc>>) -> String {
    value
        .map(|timestamp| timestamp.to_rfc3339())
        .unwrap_or_else(|| "none".to_string())
}

fn json_value<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<unserializable>".to_string())
}

fn consumer_run(
    phase: PublicBindingPhaseArg,
    args: ConsumerRunArgs,
) -> Result<Option<RunRevisionPin>> {
    match phase {
        PublicBindingPhaseArg::Launch => {
            ensure!(
                args.run_id.is_none() && args.run_revision.is_none() && args.context_id.is_none(),
                "launch phase does not accept --run-id, --run-revision, or --context-id"
            );
            Ok(None)
        }
        PublicBindingPhaseArg::Runtime => {
            let run_id = args
                .run_id
                .ok_or_else(|| anyhow!("runtime phase requires --run-id"))?;
            let revision = args
                .run_revision
                .ok_or_else(|| anyhow!("runtime phase requires --run-revision"))?;
            let context_id = args
                .context_id
                .ok_or_else(|| anyhow!("runtime phase requires --context-id"))?;
            require_revision(revision, "--run-revision")?;
            ensure!(
                !context_id.trim().is_empty(),
                "--context-id must not be empty"
            );
            Ok(Some(RunRevisionPin {
                run_id: parse_run_id(run_id)?,
                run_revision: revision,
                context_id,
            }))
        }
    }
}

fn print_mutation(
    action: &str,
    view: &ExposureView,
    idempotent: bool,
    affected: &[BindingId],
    json: bool,
) -> Result<()> {
    if json {
        return print_json(
            &serde_json::json!({"exposure": view, "affected_binding_ids": affected, "idempotent": idempotent}),
        );
    }
    println!(
        "Exposure {}: {} (revision {}, idempotent={})",
        action, view.record.exposure_id, view.revision, idempotent
    );
    if !affected.is_empty() {
        println!(
            "Affected Bindings: {}",
            affected
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}

fn print_binding_mutation(action: &str, result: BindingMutationResult, json: bool) -> Result<()> {
    if json {
        return print_json(&result);
    }
    println!(
        "Binding {}: {} (revision {}, idempotent={})",
        action, result.binding.record.binding_id, result.binding.revision, result.idempotent
    );
    if !result.affected_binding_ids.is_empty() {
        println!(
            "Affected Bindings: {}",
            result
                .affected_binding_ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}

fn parse_selector(value: String) -> Result<ResourceSelector> {
    let (kind, id) = value
        .split_once(':')
        .ok_or_else(|| anyhow!("audience/preference must use exact KIND:ID syntax"))?;
    ensure!(
        !kind.trim().is_empty() && !id.trim().is_empty() && id != "*",
        "audience/preference must be an exact non-empty KIND:ID selector"
    );
    Ok(ResourceSelector {
        kind: kind.to_string(),
        id: id.to_string(),
    })
}

fn parse_timestamp(value: String) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| anyhow!("--expires-at must be an RFC 3339 timestamp"))
}

fn parse_installation_id(value: String) -> Result<InstallationId> {
    InstallationId::parse(value).map_err(|_| anyhow!("installation id must be a UUID"))
}
fn parse_run_id(value: String) -> Result<RunId> {
    RunId::parse(value).map_err(|_| anyhow!("run id must be a UUID"))
}
fn parse_exposure_id(value: String) -> Result<ExposureId> {
    ExposureId::parse(value).map_err(|_| anyhow!("exposure id must be a UUID"))
}
fn parse_binding_id(value: String) -> Result<BindingId> {
    BindingId::parse(value).map_err(|_| anyhow!("binding id must be a UUID"))
}
fn parse_port_id(value: String) -> Result<PortId> {
    PortId::parse(value).map_err(|_| anyhow!("port id is invalid"))
}
fn require_revision(value: u64, flag: &str) -> Result<()> {
    ensure!(value > 0, "{flag} must be greater than zero");
    Ok(())
}
fn require_key(value: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty(),
        "--idempotency-key must not be empty"
    );
    Ok(())
}
fn enum_label<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| json_value(value))
}
fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    use crate::cli::{Cli, Command};

    #[test]
    fn launch_and_runtime_pins_are_explicit_and_disjoint() {
        assert!(
            consumer_run(PublicBindingPhaseArg::Launch, ConsumerRunArgs::default())
                .unwrap()
                .is_none()
        );
        assert!(consumer_run(
            PublicBindingPhaseArg::Launch,
            ConsumerRunArgs {
                run_id: Some(RunId::new().to_string()),
                ..Default::default()
            }
        )
        .is_err());
        assert!(consumer_run(PublicBindingPhaseArg::Runtime, ConsumerRunArgs::default()).is_err());
        let pin = consumer_run(
            PublicBindingPhaseArg::Runtime,
            ConsumerRunArgs {
                run_id: Some(RunId::new().to_string()),
                run_revision: Some(2),
                context_id: Some("ctx".into()),
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(pin.run_revision, 2);
    }

    #[test]
    fn selectors_are_exact_and_expiry_is_explicit() {
        assert_eq!(
            parse_selector("installation:abc".into()).unwrap().kind,
            "installation"
        );
        assert!(parse_selector("installation:*".into()).is_err());
        assert!(parse_selector("broken".into()).is_err());
        assert!(parse_timestamp("2026-08-11T12:00:00Z".into()).is_ok());
    }

    #[test]
    fn all_seven_public_powerbox_commands_parse_without_retired_aliases() {
        const INSTALLATION: &str = "11111111-1111-4111-8111-111111111111";
        const PROVIDER: &str = "22222222-2222-4222-8222-222222222222";
        const RUN: &str = "33333333-3333-4333-8333-333333333333";
        const EXPOSURE: &str = "44444444-4444-4444-8444-444444444444";
        const BINDING: &str = "55555555-5555-4555-8555-555555555555";
        const DIGEST: &str =
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        for args in [
            vec!["plurora", "exposure", "list", "--json"],
            vec![
                "plurora",
                "exposure",
                "create",
                INSTALLATION,
                "--installation-revision",
                "1",
                "--run-id",
                RUN,
                "--run-revision",
                "2",
                "--export-port",
                "save",
                "--audience",
                "installation:11111111-1111-4111-8111-111111111111",
                "--no-expiry",
                "--idempotency-key",
                "create-1",
            ],
            vec![
                "plurora",
                "exposure",
                "revoke",
                INSTALLATION,
                "--installation-revision",
                "1",
                "--run-id",
                RUN,
                "--run-revision",
                "2",
                "--export-port",
                "save",
                "--exposure-id",
                EXPOSURE,
                "--exposure-revision",
                "3",
                "--idempotency-key",
                "revoke-1",
            ],
            vec!["plurora", "binding", "list", "--json"],
            vec![
                "plurora",
                "binding",
                "candidates",
                INSTALLATION,
                "--consumer-installation-revision",
                "1",
                "--import-port",
                "save",
                "--phase",
                "launch",
            ],
            vec![
                "plurora",
                "binding",
                "select",
                INSTALLATION,
                "--consumer-installation-revision",
                "1",
                "--import-port",
                "save",
                "--phase",
                "runtime",
                "--run-id",
                RUN,
                "--run-revision",
                "2",
                "--context-id",
                "ctx-1",
                "--exposure-id",
                EXPOSURE,
                "--exposure-revision",
                "3",
                "--provider-installation-id",
                PROVIDER,
                "--provider-installation-revision",
                "4",
                "--candidate-digest",
                DIGEST,
                "--idempotency-key",
                "select-1",
            ],
            vec![
                "plurora",
                "binding",
                "revoke",
                INSTALLATION,
                "--consumer-installation-revision",
                "1",
                "--import-port",
                "save",
                "--phase",
                "launch",
                "--exposure-id",
                EXPOSURE,
                "--binding-id",
                BINDING,
                "--binding-revision",
                "5",
                "--idempotency-key",
                "binding-revoke-1",
            ],
        ] {
            let parsed = Cli::try_parse_from(args).expect("public Powerbox command must parse");
            assert!(matches!(
                parsed.command,
                Command::Exposure(_) | Command::Binding(_)
            ));
        }

        assert!(Cli::try_parse_from(["plurora", "project", "binding", "list"]).is_err());
        assert!(Cli::try_parse_from([
            "plurora",
            "binding",
            "candidates",
            INSTALLATION,
            "--consumer-installation-revision",
            "1",
            "--import-port",
            "save",
            "--phase",
            "authoring",
        ])
        .is_err());
    }
}
