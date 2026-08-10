use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, ensure, Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use plurora_core::ArtifactDescriptor;
use plurora_runtime::{
    exact_artifact_upload, AssetPutRequest, InstallationCreateRequest, InstallationGetRequest,
    InstallationListRequest, InstallationMutationResult, InstallationRemoveRequest,
    InstallationStateAction, InstallationStateSnapshot, InstallationUpdateRequest,
    InstallationView, ObjectPutResponse, ObjectPutScope, StateDisposition,
};
use plurora_work::{
    AcquisitionKind, AcquisitionRecord, InstallationId, InstallationSecretPolicy,
    InstallationStatus, WorkId, WorkRevision,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;

use super::work::{
    pack_work_for_installation, prepare_object_store_for_installation, read_packed_object,
    PackedInstallationWork,
};
use super::{host_access, host_connection};

const INSTALLATION_JOURNAL_FILE: &str = "installations.sqlite3";

#[derive(Args, Debug)]
pub struct InstallationArgs {
    #[command(subcommand)]
    pub command: InstallationCommand,

    /// Host data directory (default: $PLURORA_DATA_DIR or ~/.plurora).
    /// Used only to stage canonical Work objects for create/update; it never selects a journal.
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,

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
}

#[derive(Subcommand, Debug)]
pub enum InstallationCommand {
    /// List host-owned Installations.
    List(InstallationListArgs),
    /// Show one Installation record and its CAS revision.
    Info(InstallationInfoArgs),
    /// Materialize a Work source and create an Installation.
    Create(InstallationCreateArgs),
    /// Materialize a new Work source and update an Installation.
    Update(InstallationUpdateArgs),
    /// Remove an Installation with an explicit state disposition.
    Remove(InstallationRemoveArgs),
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum InstallationStatusArg {
    Resolving,
    Ready,
    Updating,
    Blocked,
    Failed,
    Removing,
    Removed,
}

impl From<InstallationStatusArg> for InstallationStatus {
    fn from(value: InstallationStatusArg) -> Self {
        match value {
            InstallationStatusArg::Resolving => Self::Resolving,
            InstallationStatusArg::Ready => Self::Ready,
            InstallationStatusArg::Updating => Self::Updating,
            InstallationStatusArg::Blocked => Self::Blocked,
            InstallationStatusArg::Failed => Self::Failed,
            InstallationStatusArg::Removing => Self::Removing,
            InstallationStatusArg::Removed => Self::Removed,
        }
    }
}

#[derive(Args, Debug)]
pub struct InstallationListArgs {
    #[arg(long)]
    pub status: Option<InstallationStatusArg>,
    #[arg(long, value_enum, default_value = "human")]
    pub format: OutputFormat,
}

#[derive(Args, Debug)]
pub struct InstallationInfoArgs {
    pub installation_id: String,
    #[arg(long, value_enum, default_value = "human")]
    pub format: OutputFormat,
}

#[derive(Args, Debug)]
pub struct InstallationCreateArgs {
    /// Work source directory, or its work.yaml file.
    pub source: PathBuf,
    /// Stable retry key. Reuse the same key only for the same mutation.
    #[arg(long)]
    pub idempotency_key: String,
    /// Override the Work title used as the Installation display name.
    #[arg(long)]
    pub display_name: Option<String>,
    #[arg(long, value_enum, default_value = "human")]
    pub format: OutputFormat,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum StateActionArg {
    Preserve,
    Reset,
    Replace,
}

#[derive(Args, Debug)]
pub struct InstallationUpdateArgs {
    pub installation_id: String,
    /// Candidate Work source directory, or its work.yaml file.
    pub source: PathBuf,
    /// Current CAS revision returned by `installation info`.
    #[arg(long)]
    pub expected_revision: u64,
    /// Stable retry key. Reuse the same key only for the same mutation.
    #[arg(long)]
    pub idempotency_key: String,
    #[arg(long, value_enum)]
    pub state_action: StateActionArg,
    /// Typed InstallationStateSnapshot JSON uploaded to the selected Host.
    #[arg(long)]
    pub replacement_snapshot: Option<PathBuf>,
    #[arg(long)]
    pub display_name: Option<String>,
    #[arg(long, value_enum, default_value = "human")]
    pub format: OutputFormat,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum StateDispositionArg {
    Keep,
    Delete,
}

#[derive(Args, Debug)]
pub struct InstallationRemoveArgs {
    pub installation_id: String,
    /// Current CAS revision returned by `installation info`.
    #[arg(long)]
    pub expected_revision: u64,
    /// State disposition is intentionally mandatory.
    #[arg(long, value_enum)]
    pub state: StateDispositionArg,
    /// Stable retry key. Reuse the same key only for the same mutation.
    #[arg(long)]
    pub idempotency_key: String,
    #[arg(long, value_enum, default_value = "human")]
    pub format: OutputFormat,
}

pub(crate) struct HostInstallationClient {
    pub(crate) endpoint: String,
    pub(crate) access_token: String,
}

pub async fn run(args: InstallationArgs) -> Result<()> {
    let connection = host_connection::resolve(args.endpoint.as_deref())?;
    let client = HostInstallationClient {
        endpoint: connection.endpoint,
        access_token: args.access_token.unwrap_or_default(),
    };
    run_with_client(&client, args.data_dir, args.command).await
}

async fn run_with_client(
    client: &HostInstallationClient,
    data_dir: Option<PathBuf>,
    command: InstallationCommand,
) -> Result<()> {
    match command {
        InstallationCommand::List(args) => run_list(client, args).await,
        InstallationCommand::Info(args) => run_info(client, args).await,
        InstallationCommand::Create(args) => {
            let object_root = prepare_installation_object_root(data_dir).await?;
            run_create(client, &object_root, args).await
        }
        InstallationCommand::Update(args) => {
            let object_root = prepare_installation_object_root(data_dir).await?;
            run_update(client, &object_root, args).await
        }
        InstallationCommand::Remove(args) => run_remove(client, args).await,
    }
}

async fn run_list(client: &HostInstallationClient, args: InstallationListArgs) -> Result<()> {
    let mut views: Vec<InstallationView> = call_host_protocol(
        client,
        "host.installation.list",
        InstallationListRequest {
            status: args.status.map(Into::into),
        },
    )
    .await?;
    views.sort_by(|left, right| {
        left.record
            .installation_id
            .cmp(&right.record.installation_id)
    });
    match args.format {
        OutputFormat::Json => print_json(&views),
        OutputFormat::Human => {
            println!("{:<36} {:<24} {:<12} REVISION", "ID", "NAME", "STATUS");
            for view in views {
                println!(
                    "{:<36} {:<24} {:<12} {}",
                    view.record.installation_id,
                    truncate(&view.record.display_name, 24),
                    status_label(view.record.status),
                    view.revision
                );
            }
            Ok(())
        }
    }
}

async fn run_info(client: &HostInstallationClient, args: InstallationInfoArgs) -> Result<()> {
    let installation_id = parse_installation_id(args.installation_id)?;
    let view: InstallationView = call_host_protocol(
        client,
        "host.installation.get",
        InstallationGetRequest { installation_id },
    )
    .await?;
    match args.format {
        OutputFormat::Json => print_json(&view),
        OutputFormat::Human => {
            print_view(&view);
            Ok(())
        }
    }
}

async fn run_create(
    client: &HostInstallationClient,
    object_root: &Path,
    args: InstallationCreateArgs,
) -> Result<()> {
    ensure_non_empty_key(&args.idempotency_key)?;
    let packed = pack_work(&args.source, object_root).await?;
    let work_id = packed_work_id(object_root, &packed)?;
    let display_name = args
        .display_name
        .unwrap_or_else(|| packed.display_name.clone());
    ensure!(
        !display_name.trim().is_empty(),
        "Installation display name must not be empty"
    );
    let scope = ObjectPutScope::InstallationCreate {
        work_id: work_id.clone(),
    };
    let object_count = upload_work_closure(client, object_root, &packed, &scope).await?;
    let request = InstallationCreateRequest {
        work_id,
        source: local_import(&packed.work_revision),
        work_revision: packed.work_revision,
        assembly_lock: packed.assembly_lock,
        display_name,
        state_bindings: Vec::new(),
        secret_policy: InstallationSecretPolicy::default(),
        idempotency_key: args.idempotency_key,
        authority: None,
    };
    let result: InstallationMutationResult =
        call_host_protocol(client, "host.installation.create", request).await?;
    print_mutation("created", &result, object_count, args.format)
}

async fn run_update(
    client: &HostInstallationClient,
    object_root: &Path,
    args: InstallationUpdateArgs,
) -> Result<()> {
    ensure_non_empty_key(&args.idempotency_key)?;
    let (state_action, replacement_upload) = state_action(&args)?;
    let installation_id = parse_installation_id(args.installation_id)?;
    let packed = pack_work(&args.source, object_root).await?;
    let work_id = packed_work_id(object_root, &packed)?;
    let scope = ObjectPutScope::InstallationUpdate {
        installation_id: installation_id.clone(),
        work_id,
    };
    let object_count = upload_work_closure(client, object_root, &packed, &scope).await?;
    if let Some((descriptor, bytes)) = replacement_upload {
        upload_exact_artifact(client, &descriptor, &bytes, &scope).await?;
    }
    let request = InstallationUpdateRequest {
        installation_id,
        expected_revision: args.expected_revision,
        source: Some(local_import(&packed.work_revision)),
        work_revision: packed.work_revision,
        assembly_lock: packed.assembly_lock,
        display_name: args.display_name,
        state_bindings: None,
        secret_policy: None,
        state_action,
        idempotency_key: args.idempotency_key,
        authority: None,
    };
    let result: InstallationMutationResult =
        call_host_protocol(client, "host.installation.update", request).await?;
    print_mutation("updated", &result, object_count, args.format)
}

async fn run_remove(client: &HostInstallationClient, args: InstallationRemoveArgs) -> Result<()> {
    ensure_non_empty_key(&args.idempotency_key)?;
    let request = InstallationRemoveRequest {
        installation_id: parse_installation_id(args.installation_id)?,
        expected_revision: args.expected_revision,
        state_disposition: match args.state {
            StateDispositionArg::Keep => StateDisposition::Keep,
            StateDispositionArg::Delete => StateDisposition::Delete,
        },
        idempotency_key: args.idempotency_key,
        authority: None,
    };
    let result: InstallationMutationResult =
        call_host_protocol(client, "host.installation.remove", request).await?;
    print_mutation("removed", &result, 0, args.format)
}

fn state_action(
    args: &InstallationUpdateArgs,
) -> Result<(
    InstallationStateAction,
    Option<(ArtifactDescriptor, Vec<u8>)>,
)> {
    match args.state_action {
        StateActionArg::Preserve => {
            ensure!(
                args.replacement_snapshot.is_none(),
                "state-action preserve does not accept --replacement-snapshot"
            );
            Ok((InstallationStateAction::Preserve, None))
        }
        StateActionArg::Reset => {
            ensure!(
                args.replacement_snapshot.is_none(),
                "state-action reset does not accept --replacement-snapshot"
            );
            Ok((InstallationStateAction::Reset, None))
        }
        StateActionArg::Replace => {
            let path = args
                .replacement_snapshot
                .as_deref()
                .ok_or_else(|| anyhow!("state-action replace requires --replacement-snapshot"))?;
            let bytes =
                fs::read(path).map_err(|_| anyhow!("read replacement state snapshot failed"))?;
            let snapshot: InstallationStateSnapshot = serde_json::from_slice(&bytes)
                .map_err(|_| anyhow!("replacement state snapshot JSON is malformed"))?;
            let canonical = snapshot
                .canonical_bytes()
                .map_err(|_| anyhow!("replacement state snapshot is invalid"))?;
            let descriptor = snapshot
                .artifact_descriptor()
                .map_err(|_| anyhow!("replacement state snapshot is invalid"))?;
            Ok((
                InstallationStateAction::Replace {
                    replacement_snapshot: descriptor.clone(),
                },
                Some((descriptor, canonical)),
            ))
        }
    }
}

async fn pack_work(path: &Path, object_root: &Path) -> Result<PackedInstallationWork> {
    pack_work_for_installation(path, object_root)
        .await
        .map_err(|error| anyhow!(error.to_string()))
}

async fn upload_work_closure(
    client: &HostInstallationClient,
    object_root: &Path,
    packed: &PackedInstallationWork,
    scope: &ObjectPutScope,
) -> Result<usize> {
    for descriptor in &packed.closure {
        let bytes = read_packed_object(object_root, descriptor)
            .map_err(|_| anyhow!("read staged object '{}' failed", descriptor.digest))?;
        upload_exact_artifact(client, descriptor, &bytes, scope).await?;
    }
    Ok(packed.closure.len())
}

async fn upload_exact_artifact(
    client: &HostInstallationClient,
    descriptor: &ArtifactDescriptor,
    bytes: &[u8],
    scope: &ObjectPutScope,
) -> Result<()> {
    let uploaded: ObjectPutResponse = call_host_protocol(
        client,
        "object.put",
        AssetPutRequest {
            origin_package_id: None,
            mime: descriptor.media_type.clone(),
            content: encode_hex(bytes),
            metadata: json!({}),
            artifact: Some(exact_artifact_upload(descriptor, scope.clone())),
        },
    )
    .await?;
    ensure!(
        uploaded.asset.is_none() && uploaded.descriptor == *descriptor,
        "Host object.put response did not preserve the uploaded artifact descriptor"
    );
    Ok(())
}

fn packed_work_id(object_root: &Path, packed: &PackedInstallationWork) -> Result<WorkId> {
    let bytes = read_packed_object(object_root, &packed.work_revision)
        .map_err(|_| anyhow!("read packed WorkRevision failed"))?;
    let revision: WorkRevision = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow!("packed WorkRevision JSON is malformed"))?;
    Ok(revision.work_id)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn local_import(work_revision: &ArtifactDescriptor) -> AcquisitionRecord {
    AcquisitionRecord {
        kind: AcquisitionKind::LocalImport,
        source_ref: Some(work_revision.clone()),
        provenance_refs: Vec::new(),
        update_channel: None,
    }
}

fn ensure_non_empty_key(key: &str) -> Result<()> {
    ensure!(
        !key.trim().is_empty(),
        "--idempotency-key must not be empty"
    );
    Ok(())
}

fn parse_installation_id(value: String) -> Result<InstallationId> {
    InstallationId::parse(value).map_err(|_| anyhow!("installation id must be a UUID"))
}

pub(crate) async fn call_host_protocol<Q, R>(
    client: &HostInstallationClient,
    method: &str,
    request: Q,
) -> Result<R>
where
    Q: Serialize,
    R: DeserializeOwned,
{
    let response = host_access::request(
        &client.endpoint,
        &client.access_token,
        reqwest::Method::POST,
        "/rpc",
        Some(json!({
            "id": "plurora-cli-installation",
            "method": method,
            "params": serde_json::to_value(request)?,
        })),
    )
    .await?;
    if let Some(error) = response.get("error") {
        // Error payloads are untrusted Host data. Keep the stable reason/code
        // useful to a CLI caller without echoing raw diagnostics, paths, or
        // credentials into terminal/JSON output.
        let code = error
            .get("code")
            .and_then(serde_json::Value::as_str)
            .filter(|code| code.starts_with("runtime/error/") || code.starts_with("host/"))
            .unwrap_or("unknown");
        return Err(anyhow!("Host RPC method '{method}' failed ({code})"));
    }
    let value = response
        .get("result")
        .cloned()
        .ok_or_else(|| anyhow!("Host RPC method '{method}' response is missing result"))?;
    serde_json::from_value(value)
        .with_context(|| format!("Host RPC method '{method}' returned an invalid response"))
}

async fn prepare_installation_object_root(data_dir: Option<PathBuf>) -> Result<PathBuf> {
    let requested_data_root = match data_dir {
        Some(data_root) => data_root,
        None => plurora_core::paths::data_dir()?,
    };
    let data_root = prepare_data_root(&requested_data_root)?;
    let object_root = prepare_owned_directory(&data_root, "objects")?;
    prepare_object_store_for_installation(&object_root)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    Ok(object_root)
}

pub(crate) fn prepare_installation_journal(runtime_root: &Path) -> Result<PathBuf> {
    let journal = runtime_root.join(INSTALLATION_JOURNAL_FILE);
    ensure_regular_file_or_absent(&journal)?;
    for suffix in ["-journal", "-wal", "-shm"] {
        ensure_regular_file_or_absent(
            &runtime_root.join(format!("{INSTALLATION_JOURNAL_FILE}{suffix}")),
        )?;
    }
    Ok(journal)
}

fn prepare_data_root(path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        fs::create_dir_all(path).context("create Installation data root failed")?;
    }
    ensure_real_directory(path, "Installation data root")?;
    fs::canonicalize(path).context("resolve Installation data root failed")
}

fn prepare_owned_directory(root: &Path, name: &str) -> Result<PathBuf> {
    let path = root.join(name);
    if !path.exists() {
        fs::create_dir(&path).context("create Installation-owned directory failed")?;
    }
    ensure_real_directory(&path, "Installation-owned directory")?;
    let canonical =
        fs::canonicalize(&path).context("resolve Installation-owned directory failed")?;
    ensure!(
        canonical.starts_with(root),
        "Installation-owned directory escapes the data root"
    );
    Ok(canonical)
}

fn ensure_real_directory(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).with_context(|| format!("inspect {label} failed"))?;
    ensure!(
        metadata.file_type().is_dir() && !metadata.file_type().is_symlink(),
        "{label} must be a real directory"
    );
    ensure!(
        !is_reparse_point(&metadata),
        "{label} must not be a reparse point"
    );
    Ok(())
}

fn ensure_regular_file_or_absent(path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(anyhow!("inspect Installation journal failed")),
    };
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "Installation journal must be a real file"
    );
    ensure!(
        !is_reparse_point(&metadata),
        "Installation journal must not be a reparse point"
    );
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn print_json(value: &impl Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn print_mutation(
    operation: &str,
    result: &InstallationMutationResult,
    object_count: usize,
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Json => print_json(result),
        OutputFormat::Human => {
            println!(
                "Installation {operation}: {}",
                result.installation.record.installation_id
            );
            println!("Revision: {}", result.installation.revision);
            println!("Idempotent replay: {}", result.idempotent);
            if object_count > 0 {
                println!("Canonical Work objects: {object_count}");
            }
            Ok(())
        }
    }
}

fn print_view(view: &InstallationView) {
    println!("Installation: {}", view.record.installation_id);
    println!("Name: {}", view.record.display_name);
    println!("Status: {}", status_label(view.record.status));
    println!("Revision: {}", view.revision);
    println!("Work: {}", view.record.work_revision.digest);
    println!("Assembly lock: {}", view.record.assembly_lock.digest);
}

fn status_label(status: InstallationStatus) -> &'static str {
    match status {
        InstallationStatus::Resolving => "resolving",
        InstallationStatus::Ready => "ready",
        InstallationStatus::Updating => "updating",
        InstallationStatus::Blocked => "blocked",
        InstallationStatus::Failed => "failed",
        InstallationStatus::Removing => "removing",
        InstallationStatus::Removed => "removed",
    }
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::routing::post;
    use axum::{Json, Router};
    use clap::Parser;
    use serde_json::Value;
    use tokio::sync::Mutex;

    use super::*;
    use crate::cli::{Cli, Command};

    #[test]
    fn parses_exact_installation_surface_and_required_mutation_flags() {
        let cli = Cli::try_parse_from([
            "plurora",
            "installation",
            "create",
            "fixture",
            "--idempotency-key",
            "create-1",
            "--format",
            "json",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Command::Installation(InstallationArgs {
                command: InstallationCommand::Create(_),
                ..
            })
        ));

        assert!(Cli::try_parse_from(["plurora", "installation", "create", "fixture"]).is_err());
        assert!(Cli::try_parse_from([
            "plurora",
            "installation",
            "remove",
            "18f0776d-d719-4483-ab95-90a2483819d4",
            "--expected-revision",
            "1",
            "--idempotency-key",
            "remove-1"
        ])
        .is_err());
    }

    #[test]
    fn retired_installation_aliases_do_not_parse() {
        for command in [
            "project",
            "install",
            "uninstall",
            "update",
            "list-installed",
            "lockfile",
        ] {
            assert!(
                Cli::try_parse_from(["plurora", command]).is_err(),
                "retired top-level command still parses: {command}"
            );
        }
    }

    #[test]
    fn update_state_actions_use_host_issued_decisions_and_typed_snapshot_inputs() {
        let temporary = tempfile::tempdir().unwrap();
        let replacement = temporary.path().join("replacement.json");
        let snapshot = InstallationStateSnapshot {
            schema: plurora_runtime::INSTALLATION_STATE_SNAPSHOT_SCHEMA.to_string(),
            entries: vec![plurora_runtime::InstallationStateSnapshotEntry {
                path: "save/profile.bin".to_string(),
                bytes: vec![0, 1, 255],
            }],
        };
        std::fs::write(&replacement, serde_json::to_vec_pretty(&snapshot).unwrap()).unwrap();

        let args = update_args(StateActionArg::Reset);
        assert!(matches!(
            state_action(&args).unwrap(),
            (InstallationStateAction::Reset, None)
        ));

        let mut args = update_args(StateActionArg::Replace);
        args.replacement_snapshot = Some(replacement);
        let (action, upload) = state_action(&args).unwrap();
        let (uploaded_descriptor, uploaded_bytes) = upload.expect("snapshot upload");
        assert!(matches!(action, InstallationStateAction::Replace { .. }));
        assert_eq!(uploaded_descriptor, snapshot.artifact_descriptor().unwrap());
        assert_eq!(uploaded_bytes, snapshot.canonical_bytes().unwrap());

        let missing = update_args(StateActionArg::Replace);
        assert!(state_action(&missing)
            .unwrap_err()
            .to_string()
            .contains("requires --replacement-snapshot"));

        let malformed = temporary.path().join("private-local-state-name.json");
        std::fs::write(&malformed, b"private raw state bytes").unwrap();
        let mut malformed_args = update_args(StateActionArg::Replace);
        malformed_args.replacement_snapshot = Some(malformed.clone());
        let error = state_action(&malformed_args).unwrap_err().to_string();
        assert!(error.contains("snapshot JSON is malformed"));
        assert!(!error.contains(malformed.to_string_lossy().as_ref()));
        assert!(!error.contains("private raw state bytes"));

        for retired_flag in ["--approval-ref", "--migration-receipt"] {
            assert!(Cli::try_parse_from([
                "plurora",
                "installation",
                "update",
                "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                "unused",
                "--expected-revision",
                "1",
                "--idempotency-key",
                "update-1",
                "--state-action",
                "reset",
                retired_flag,
                "forged.json"
            ])
            .is_err());
        }
    }

    #[tokio::test]
    async fn selected_host_list_and_info_never_open_or_recover_a_local_journal() -> Result<()> {
        let installation_id = "18f0776d-d719-4483-ab95-90a2483819d4";
        let descriptor = |artifact_type_uri: &str, byte: char| {
            json!({
                "artifact_type_uri": artifact_type_uri,
                "media_type": "application/json",
                "digest": format!("sha256:{}", byte.to_string().repeat(64)),
                "size_bytes": 1,
                "references": [],
                "annotations": {}
            })
        };
        let view = json!({
            "record": {
                "schema_version": 1,
                "installation_id": installation_id,
                "work_revision": descriptor(plurora_work::WORK_REVISION_TYPE_URI, 'a'),
                "assembly_lock": descriptor(plurora_work::ASSEMBLY_LOCK_TYPE_URI, 'b'),
                "display_name": "Remote Installation",
                "source": {
                    "kind": "local_import",
                    "provenance_refs": []
                },
                "state_bindings": [],
                "secret_policy": {
                    "allowed_secret_refs": [],
                    "allow_platform_fallback": false
                },
                "created_at": "2026-08-10T00:00:00Z",
                "updated_at": "2026-08-10T00:00:00Z",
                "status": "ready"
            },
            "work_summary": {
                "work_id": "tests/remote-installation",
                "title": "Remote Installation",
                "description": "",
                "content_roots": [],
                "entrypoints": [],
                "rights": null,
                "transparency": null,
                "operational_intent": null,
                "annotations": {}
            },
            "revision": 1
        });
        let calls = Arc::new(Mutex::new(Vec::<String>::new()));
        let server_calls = calls.clone();
        let app = Router::new().route(
            "/rpc",
            post(move |Json(request): Json<Value>| {
                let calls = server_calls.clone();
                let view = view.clone();
                async move {
                    let method = request["method"].as_str().unwrap_or_default().to_string();
                    calls.lock().await.push(method.clone());
                    let result = match method.as_str() {
                        "host.installation.list" => json!([view]),
                        "host.installation.remove" => json!({
                            "installation": view,
                            "diff": null,
                            "idempotent": false
                        }),
                        _ => view,
                    };
                    Json(json!({
                        "id": request["id"],
                        "result": result
                    }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let server = tokio::spawn(async move { axum::serve(listener, app).await });

        let temporary = tempfile::tempdir()?;
        let runtime_root = temporary.path().join("runtime");
        fs::create_dir(&runtime_root)?;
        let journal = runtime_root.join(INSTALLATION_JOURNAL_FILE);
        let pending_sentinel = b"pending journal must remain byte-for-byte unchanged";
        fs::write(&journal, pending_sentinel)?;
        let client = HostInstallationClient {
            endpoint: format!("http://{address}"),
            access_token: String::new(),
        };

        run_with_client(
            &client,
            Some(temporary.path().to_path_buf()),
            InstallationCommand::List(InstallationListArgs {
                status: None,
                format: OutputFormat::Json,
            }),
        )
        .await?;
        run_with_client(
            &client,
            Some(temporary.path().to_path_buf()),
            InstallationCommand::Info(InstallationInfoArgs {
                installation_id: installation_id.to_string(),
                format: OutputFormat::Json,
            }),
        )
        .await?;
        run_with_client(
            &client,
            Some(temporary.path().to_path_buf()),
            InstallationCommand::Remove(InstallationRemoveArgs {
                installation_id: installation_id.to_string(),
                expected_revision: 1,
                state: StateDispositionArg::Keep,
                idempotency_key: "remove-remote".to_string(),
                format: OutputFormat::Json,
            }),
        )
        .await?;

        assert_eq!(fs::read(journal)?, pending_sentinel);
        assert!(!temporary.path().join("objects").exists());
        assert_eq!(
            *calls.lock().await,
            [
                "host.installation.list".to_string(),
                "host.installation.get".to_string(),
                "host.installation.remove".to_string()
            ]
        );
        server.abort();
        Ok(())
    }

    #[tokio::test]
    async fn complete_closure_upload_is_ordered_and_contains_no_local_source_identity() -> Result<()>
    {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source-with-sensitive-local-name");
        write_upload_fixture(&source)?;
        let unrelated_secret = "RawSecretOutsidePortableClosure-123456789";
        fs::write(source.join("unreferenced-secret.txt"), unrelated_secret)?;
        let object_root = temporary.path().join("staging/objects");
        let packed = pack_work(&source, &object_root).await?;
        assert!(packed.closure.len() > 3);

        let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let server_requests = requests.clone();
        let app = Router::new().route(
            "/rpc",
            post(move |Json(request): Json<Value>| {
                let requests = server_requests.clone();
                async move {
                    requests.lock().await.push(request.clone());
                    let params = &request["params"];
                    let descriptor = params["artifact"]["descriptor"].clone();
                    assert_eq!(params["artifact"]["content_encoding"], "hex");
                    Json(json!({
                        "id": request["id"],
                        "result": {
                            "asset": null,
                            "descriptor": descriptor
                        }
                    }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        let client = HostInstallationClient {
            endpoint: format!("http://{address}"),
            access_token: String::new(),
        };

        let work_id = packed_work_id(&object_root, &packed)?;
        let scope = ObjectPutScope::InstallationCreate {
            work_id: work_id.clone(),
        };
        assert_eq!(
            upload_work_closure(&client, &object_root, &packed, &scope).await?,
            packed.closure.len()
        );
        let requests = requests.lock().await.clone();
        assert_eq!(requests.len(), packed.closure.len());
        assert!(requests
            .iter()
            .all(|request| request["method"] == "object.put"));
        assert!(requests.iter().all(|request| {
            request["params"]["artifact"]["scope"]
                == json!({"kind": "installation_create", "work_id": work_id})
        }));
        let uploaded_digests = requests
            .iter()
            .map(|request| {
                request["params"]["artifact"]["descriptor"]["digest"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<_>>();
        let expected_digests = packed
            .closure
            .iter()
            .map(|descriptor| descriptor.digest.clone())
            .collect::<Vec<_>>();
        assert_eq!(uploaded_digests, expected_digests);
        assert!(uploaded_digests.windows(2).all(|pair| pair[0] <= pair[1]));
        let wire = serde_json::to_string(&requests)?;
        assert!(!wire.contains(source.to_string_lossy().as_ref()));
        assert!(!wire.contains(unrelated_secret));
        server.abort();
        Ok(())
    }

    #[tokio::test]
    async fn failed_object_upload_never_submits_installation_mutation() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("work");
        write_upload_fixture(&source)?;
        let object_root = temporary.path().join("staging/objects");
        let calls = Arc::new(Mutex::new(Vec::<String>::new()));
        let server_calls = calls.clone();
        let app = Router::new().route(
            "/rpc",
            post(move |Json(request): Json<Value>| {
                let calls = server_calls.clone();
                async move {
                    let method = request["method"].as_str().unwrap().to_string();
                    calls.lock().await.push(method);
                    Json(json!({
                        "id": request["id"],
                        "error": {"code": "upload_failed", "message": "injected"}
                    }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        let client = HostInstallationClient {
            endpoint: format!("http://{address}"),
            access_token: String::new(),
        };

        let error = run_create(
            &client,
            &object_root,
            InstallationCreateArgs {
                source,
                idempotency_key: "failed-upload".to_string(),
                display_name: None,
                format: OutputFormat::Json,
            },
        )
        .await
        .expect_err("upload failure must stop before Installation creation");
        assert!(error.to_string().contains("object.put"));
        assert_eq!(*calls.lock().await, ["object.put".to_string()]);
        server.abort();
        Ok(())
    }

    fn write_upload_fixture(root: &Path) -> Result<()> {
        fs::create_dir_all(root.join("packages/component"))?;
        fs::create_dir_all(root.join("content"))?;
        fs::write(root.join("content/binary.bin"), [0_u8, 1, 0xff, 2])?;
        fs::write(
            root.join("packages/component/manifest.yaml"),
            r#"schema_version: 1
id: fixture/component
version: 1.0.0
entry:
  kind: rust_inproc
  crate_ref: fixture-component
  symbol: register
  abi_version: 1
provides: []
consumes: []
requires: []
contributes: {}
permissions: {}
sandbox_policy: {}
"#,
        )?;
        fs::write(
            root.join("work.yaml"),
            "schema: plurora.work-source.v1\nwork:\n  id: fixture/upload\n  title: Upload Fixture\n  assembly: assembly.yaml\n  content: [content]\n",
        )?;
        fs::write(
            root.join("assembly.yaml"),
            "schema: plurora.assembly-source.v1\nassembly:\n  id: fixture/upload-main\n  nodes:\n    - id: component\n      component: packages/component/manifest.yaml\n",
        )?;
        Ok(())
    }

    fn update_args(state_action: StateActionArg) -> InstallationUpdateArgs {
        InstallationUpdateArgs {
            installation_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_string(),
            source: PathBuf::from("unused"),
            expected_revision: 1,
            idempotency_key: "update-1".to_string(),
            state_action,
            replacement_snapshot: None,
            display_name: None,
            format: OutputFormat::Json,
        }
    }
}
