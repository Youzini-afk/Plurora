use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, ensure};
use plurora_core::{ArtifactDescriptor, PackageId};
use plurora_work::{
    canonical_json_bytes, validate_artifact_descriptor, ArtifactModel, ForeignCapsuleDescriptor,
    ForeignLaunchKind, ForeignLaunchRequirement, InstallationId, InstallationSecretPolicy,
    RightDisposition, RightsDeclaration, RightsOperation, TransparencyDeclaration, WorkRevision,
    FOREIGN_CAPSULE_TYPE_URI,
};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::process::{Child, Command};

use crate::{CapabilityInvocationRequest, EventStore, ObjectStore, Runtime};

pub const FOREIGN_LAUNCH_BINDING_SCHEMA: &str = "plurora.foreign-launch-binding.v1";

/// Installation-local launch data. The serialized value belongs in the
/// Installation secret store; it is never a Work artifact, Installation view,
/// event payload, receipt, or diagnostic.
#[derive(Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForeignLaunchBinding {
    pub schema: String,
    pub launch_id: String,
    pub target: ForeignLaunchTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entitlement: Option<ForeignEntitlementAdapter>,
}

impl fmt::Debug for ForeignLaunchBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ForeignLaunchBinding")
            .field("schema", &self.schema)
            .field("launch_id", &self.launch_id)
            .field("kind", &self.target.kind())
            .field(
                "entitlement",
                &self.entitlement.as_ref().map(|_| "configured"),
            )
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ForeignLaunchTarget {
    ExternalUri {
        uri: String,
    },
    LocalExecutable {
        executable: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        working_directory: Option<String>,
    },
    ManagedArtifact {
        artifact: ArtifactDescriptor,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
    },
    OciImage {
        image: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
    },
    RemoteService {
        endpoint: String,
    },
    EntitlementAdapter {
        adapter: ForeignEntitlementAdapter,
    },
}

impl fmt::Debug for ForeignLaunchTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ForeignLaunchTarget")
            .field("kind", &self.kind())
            .finish()
    }
}

impl ForeignLaunchTarget {
    pub fn kind(&self) -> ForeignLaunchKind {
        match self {
            Self::ExternalUri { .. } => ForeignLaunchKind::ExternalUri,
            Self::LocalExecutable { .. } => ForeignLaunchKind::LocalExecutable,
            Self::ManagedArtifact { .. } => ForeignLaunchKind::ManagedArtifact,
            Self::OciImage { .. } => ForeignLaunchKind::OciImage,
            Self::RemoteService { .. } => ForeignLaunchKind::RemoteService,
            Self::EntitlementAdapter { .. } => ForeignLaunchKind::EntitlementAdapter,
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        match self {
            Self::ExternalUri { uri } => validate_uri(uri, "external URI"),
            Self::LocalExecutable {
                executable,
                args,
                working_directory,
            } => {
                validate_local_text(executable, "local executable")?;
                if let Some(directory) = working_directory {
                    validate_local_text(directory, "working directory")?;
                }
                validate_args(args)
            }
            Self::ManagedArtifact { artifact, args } => {
                validate_artifact_descriptor(artifact)?;
                validate_args(args)
            }
            Self::OciImage { image, args } => {
                validate_oci_image(image)?;
                validate_args(args)
            }
            Self::RemoteService { endpoint } => validate_uri(endpoint, "remote service endpoint"),
            Self::EntitlementAdapter { adapter } => adapter.validate(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForeignEntitlementAdapter {
    pub package_id: PackageId,
    pub capability_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default)]
    pub input: Value,
}

impl fmt::Debug for ForeignEntitlementAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ForeignEntitlementAdapter")
            .field("package_id", &self.package_id)
            .field("capability_id", &self.capability_id)
            .field("version", &self.version)
            .field("input", &"redacted")
            .finish()
    }
}

impl ForeignEntitlementAdapter {
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            !self.package_id.trim().is_empty()
                && !self.capability_id.trim().is_empty()
                && self
                    .version
                    .as_ref()
                    .is_none_or(|version| !version.trim().is_empty()),
            "foreign entitlement adapter identity is invalid"
        );
        Ok(())
    }
}

impl ForeignLaunchBinding {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.schema == FOREIGN_LAUNCH_BINDING_SCHEMA && !self.launch_id.trim().is_empty(),
            "foreign launch binding identity is invalid"
        );
        self.target.validate()?;
        if let Some(entitlement) = &self.entitlement {
            entitlement.validate()?;
        }
        Ok(())
    }

    pub fn validate_for(&self, requirement: &ForeignLaunchRequirement) -> anyhow::Result<()> {
        ensure!(
            self.launch_id == requirement.launch_id && self.target.kind() == requirement.kind,
            "foreign launch binding does not match the fixed launch requirement"
        );
        self.validate()
    }
}

fn validate_uri(value: &str, label: &str) -> anyhow::Result<()> {
    validate_local_text(value, label)?;
    let parsed = url::Url::parse(value).map_err(|_| anyhow!("{label} is invalid"))?;
    ensure!(!parsed.scheme().is_empty(), "{label} has no scheme");
    Ok(())
}

fn validate_args(args: &[String]) -> anyhow::Result<()> {
    ensure!(
        args.iter().all(|value| !value.contains('\0')),
        "foreign launch arguments contain a NUL byte"
    );
    Ok(())
}

fn validate_local_text(value: &str, label: &str) -> anyhow::Result<()> {
    ensure!(
        !value.trim().is_empty() && !value.contains('\0'),
        "{label} is invalid"
    );
    Ok(())
}

fn validate_oci_image(image: &str) -> anyhow::Result<()> {
    validate_local_text(image, "OCI image")?;
    ensure!(
        !image.starts_with('-'),
        "OCI image must not be interpreted as a Docker option"
    );
    Ok(())
}

pub fn foreign_launch_secret_name(launch_id: &str) -> String {
    let digest = Sha256::digest(launch_id.as_bytes());
    format!("foreign-launch-{digest:x}")
}

pub fn foreign_launch_secret_ref(launch_id: &str) -> String {
    format!(
        "secret_ref:installation:{}",
        foreign_launch_secret_name(launch_id)
    )
}

pub async fn load_rights_declaration(
    store: &dyn ObjectStore,
    work: &WorkRevision,
) -> anyhow::Result<Option<RightsDeclaration>> {
    match work.rights.as_ref() {
        Some(descriptor) => load_model(store, descriptor).await.map(Some),
        None => Ok(None),
    }
}

pub async fn load_work_revision(
    store: &dyn ObjectStore,
    descriptor: &ArtifactDescriptor,
) -> anyhow::Result<WorkRevision> {
    load_model(store, descriptor).await
}

pub async fn load_transparency_declaration(
    store: &dyn ObjectStore,
    work: &WorkRevision,
) -> anyhow::Result<Option<TransparencyDeclaration>> {
    match work.transparency.as_ref() {
        Some(descriptor) => load_model(store, descriptor).await.map(Some),
        None => Ok(None),
    }
}

pub async fn load_foreign_capsule(
    store: &dyn ObjectStore,
    work: &WorkRevision,
) -> anyhow::Result<Option<ForeignCapsuleDescriptor>> {
    let candidates = work
        .content_roots
        .iter()
        .filter(|descriptor| descriptor.artifact_type_uri == FOREIGN_CAPSULE_TYPE_URI)
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [] => Ok(None),
        [descriptor] => load_model(store, descriptor).await.map(Some),
        _ => Err(anyhow!(
            "foreign Work contains more than one capsule descriptor"
        )),
    }
}

async fn load_model<T>(
    store: &dyn ObjectStore,
    descriptor: &ArtifactDescriptor,
) -> anyhow::Result<T>
where
    T: ArtifactModel + DeserializeOwned + Serialize,
{
    validate_artifact_descriptor(descriptor)?;
    ensure!(
        descriptor.artifact_type_uri == T::ARTIFACT_TYPE_URI,
        "artifact has an unexpected model type"
    );
    let bytes = store
        .get(&descriptor.digest)
        .await
        .map_err(|_| anyhow!("declared artifact is unavailable or corrupt"))?;
    ensure!(
        descriptor.size_bytes == bytes.len() as u64,
        "declared artifact size differs from its descriptor"
    );
    let model: T =
        serde_json::from_slice(&bytes).map_err(|_| anyhow!("artifact JSON is invalid"))?;
    model
        .validate()
        .map_err(|_| anyhow!("artifact model is invalid"))?;
    ensure!(
        canonical_json_bytes(&model)? == bytes,
        "artifact JSON is not canonical"
    );
    Ok(model)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightsPolicyOutcome {
    Allowed,
    RequiresEntitlement,
    Denied,
    Unspecified,
}

pub fn rights_policy_outcome(
    rights: Option<&RightsDeclaration>,
    operation: RightsOperation,
) -> RightsPolicyOutcome {
    match rights.map(|value| value.disposition(operation)) {
        None if matches!(
            operation,
            RightsOperation::Install | RightsOperation::Execute
        ) =>
        {
            RightsPolicyOutcome::Allowed
        }
        None => RightsPolicyOutcome::Unspecified,
        Some(RightDisposition::Allowed) => RightsPolicyOutcome::Allowed,
        Some(RightDisposition::RequiresEntitlement) => RightsPolicyOutcome::RequiresEntitlement,
        Some(RightDisposition::Denied) => RightsPolicyOutcome::Denied,
        Some(RightDisposition::Unspecified) => RightsPolicyOutcome::Unspecified,
    }
}

#[derive(Debug)]
pub struct RightsPolicyError {
    operation: RightsOperation,
    outcome: RightsPolicyOutcome,
}

impl RightsPolicyError {
    pub fn new(operation: RightsOperation, outcome: RightsPolicyOutcome) -> Self {
        Self { operation, outcome }
    }

    pub fn operation(&self) -> RightsOperation {
        self.operation
    }

    pub fn outcome(&self) -> RightsPolicyOutcome {
        self.outcome
    }
}

impl fmt::Display for RightsPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "rights policy blocked {:?}: {:?}",
            self.operation, self.outcome
        )
    }
}

impl std::error::Error for RightsPolicyError {}

pub fn require_declared_right(
    rights: Option<&RightsDeclaration>,
    operation: RightsOperation,
) -> anyhow::Result<()> {
    let outcome = rights_policy_outcome(rights, operation);
    if outcome == RightsPolicyOutcome::Allowed {
        return Ok(());
    }
    Err(RightsPolicyError::new(operation, outcome).into())
}

pub async fn resolve_foreign_launch_binding<S>(
    runtime: &Runtime<S>,
    installation_id: &InstallationId,
    policy: &InstallationSecretPolicy,
    requirement: &ForeignLaunchRequirement,
) -> anyhow::Result<ForeignLaunchBinding>
where
    S: EventStore,
{
    let reference = foreign_launch_secret_ref(&requirement.launch_id);
    ensure!(
        policy
            .allowed_secret_refs
            .iter()
            .any(|candidate| candidate == &reference),
        "binding_unavailable: Installation does not allow the required foreign launch binding"
    );
    let value = runtime
        .resolve_secret_ref_for_installation(&reference, installation_id)
        .await
        .map_err(|_| anyhow!("binding_unavailable: foreign launch binding is unavailable"))?;
    let binding: ForeignLaunchBinding = serde_json::from_str(&value)
        .map_err(|_| anyhow!("binding_invalid: foreign launch binding is malformed"))?;
    binding
        .validate_for(requirement)
        .map_err(|_| anyhow!("binding_invalid: foreign launch binding does not match the Work"))?;
    Ok(binding)
}

pub struct ActiveForeignLaunch {
    child: Option<Child>,
    materialized: Option<tempfile::TempDir>,
}

impl fmt::Debug for ActiveForeignLaunch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveForeignLaunch")
            .field("has_child", &self.child.is_some())
            .field("has_materialized_artifact", &self.materialized.is_some())
            .finish()
    }
}

impl ActiveForeignLaunch {
    fn detached() -> Self {
        Self {
            child: None,
            materialized: None,
        }
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        if child
            .try_wait()
            .map_err(|_| anyhow!("foreign launch status is unavailable"))?
            .is_none()
        {
            child
                .kill()
                .await
                .map_err(|_| anyhow!("foreign launch could not be stopped"))?;
        }
        let _ = child.wait().await;
        self.child = None;
        self.materialized = None;
        Ok(())
    }
}

pub async fn activate_foreign_launch<S>(
    runtime: &Runtime<S>,
    session_id: &str,
    binding: &ForeignLaunchBinding,
    rights: Option<&RightsDeclaration>,
) -> anyhow::Result<ActiveForeignLaunch>
where
    S: EventStore,
{
    let outcome = rights_policy_outcome(rights, RightsOperation::Execute);
    match outcome {
        RightsPolicyOutcome::Allowed => {}
        RightsPolicyOutcome::RequiresEntitlement
            if binding.entitlement.is_some()
                || matches!(
                    binding.target,
                    ForeignLaunchTarget::EntitlementAdapter { .. }
                ) => {}
        _ => return Err(RightsPolicyError::new(RightsOperation::Execute, outcome).into()),
    }

    if let Some(adapter) = &binding.entitlement {
        authorize_entitlement(runtime, session_id, adapter).await?;
    }
    activate_target(runtime, session_id, &binding.target).await
}

fn activate_target<'a, S>(
    runtime: &'a Runtime<S>,
    session_id: &'a str,
    target: &'a ForeignLaunchTarget,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = anyhow::Result<ActiveForeignLaunch>> + Send + 'a>,
>
where
    S: EventStore,
{
    Box::pin(async move {
        match target {
            ForeignLaunchTarget::ExternalUri { uri } => {
                let mut command = external_uri_command(uri)?;
                spawn_redacted(&mut command, None)
            }
            ForeignLaunchTarget::LocalExecutable {
                executable,
                args,
                working_directory,
            } => {
                let mut command = Command::new(executable);
                command.args(args);
                if let Some(directory) = working_directory {
                    command.current_dir(directory);
                }
                spawn_redacted(&mut command, None)
            }
            ForeignLaunchTarget::ManagedArtifact { artifact, args } => {
                let bytes = runtime
                    .object_store()
                    .get(&artifact.digest)
                    .await
                    .map_err(|_| {
                        anyhow!("artifact_missing: managed launch artifact is unavailable")
                    })?;
                ensure!(
                    bytes.len() as u64 == artifact.size_bytes,
                    "artifact_digest_mismatch: managed launch artifact size differs"
                );
                let directory = tempfile::Builder::new()
                    .prefix("plurora-managed-foreign-")
                    .tempdir()
                    .map_err(|_| anyhow!("managed launch staging failed"))?;
                let executable = managed_artifact_path(directory.path());
                std::fs::write(&executable, &bytes)
                    .map_err(|_| anyhow!("managed launch staging failed"))?;
                make_executable(&executable)?;
                let mut command = Command::new(&executable);
                command.args(args).current_dir(directory.path());
                spawn_redacted(&mut command, Some(directory))
            }
            ForeignLaunchTarget::OciImage { image, args } => {
                let mut command = Command::new("docker");
                command.arg("run").arg("--rm").arg(image).args(args);
                spawn_redacted(&mut command, None)
            }
            ForeignLaunchTarget::RemoteService { .. } => Ok(ActiveForeignLaunch::detached()),
            ForeignLaunchTarget::EntitlementAdapter { adapter } => {
                let output = authorize_entitlement(runtime, session_id, adapter).await?;
                let Some(launch) = output.get("launch") else {
                    return Ok(ActiveForeignLaunch::detached());
                };
                let launch: ForeignLaunchTarget =
                    serde_json::from_value(launch.clone()).map_err(|_| {
                        anyhow!("entitlement_denied: adapter returned an invalid launch decision")
                    })?;
                ensure!(
                    !matches!(launch, ForeignLaunchTarget::EntitlementAdapter { .. }),
                    "entitlement_denied: nested entitlement adapters are invalid"
                );
                launch.validate().map_err(|_| {
                    anyhow!("entitlement_denied: adapter returned an invalid launch decision")
                })?;
                activate_target(runtime, session_id, &launch).await
            }
        }
    })
}

async fn authorize_entitlement<S>(
    runtime: &Runtime<S>,
    session_id: &str,
    adapter: &ForeignEntitlementAdapter,
) -> anyhow::Result<Value>
where
    S: EventStore,
{
    adapter
        .validate()
        .map_err(|_| anyhow!("entitlement_denied: entitlement adapter is invalid"))?;
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(adapter.capability_id.clone()),
            caller_package_id: None,
            provider_package_id: Some(adapter.package_id.clone()),
            version: adapter.version.clone(),
            session_id: Some(session_id.to_string()),
            input: adapter.input.clone(),
        })
        .await
        .map_err(|_| anyhow!("entitlement_denied: entitlement adapter denied or is unavailable"))?;
    ensure!(
        result.output.get("allowed").and_then(Value::as_bool) == Some(true),
        "entitlement_denied: entitlement adapter denied or is unavailable"
    );
    Ok(result.output)
}

fn spawn_redacted(
    command: &mut Command,
    materialized: Option<tempfile::TempDir>,
) -> anyhow::Result<ActiveForeignLaunch> {
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let child = command
        .spawn()
        .map_err(|_| anyhow!("foreign_launch_failed: launch target could not start"))?;
    Ok(ActiveForeignLaunch {
        child: Some(child),
        materialized,
    })
}

fn external_uri_command(uri: &str) -> anyhow::Result<Command> {
    validate_uri(uri, "external URI")?;
    #[cfg(windows)]
    {
        let mut command = Command::new("rundll32.exe");
        command.arg("url.dll,FileProtocolHandler").arg(uri);
        Ok(command)
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg(uri);
        Ok(command)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut command = Command::new("xdg-open");
        command.arg(uri);
        Ok(command)
    }
}

fn managed_artifact_path(directory: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        directory.join("entry.exe")
    }
    #[cfg(not(windows))]
    {
        directory.join("entry")
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)
        .map_err(|_| anyhow!("managed launch staging failed"))?
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(path, permissions)
        .map_err(|_| anyhow!("managed launch staging failed"))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use super::*;

    fn requirement(kind: ForeignLaunchKind) -> ForeignLaunchRequirement {
        ForeignLaunchRequirement {
            launch_id: "play.local".to_string(),
            kind,
            required_protocols: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    fn rights(execute: RightDisposition) -> RightsDeclaration {
        RightsDeclaration {
            license_expression: None,
            terms_uri: None,
            install: RightDisposition::Allowed,
            execute,
            backup: RightDisposition::Allowed,
            export_state: RightDisposition::Allowed,
            copy_across_hosts: RightDisposition::Denied,
            redistribute_artifacts: RightDisposition::Denied,
            modify: RightDisposition::Denied,
            derive: RightDisposition::Denied,
            modding: RightDisposition::Denied,
            dedicated_server: RightDisposition::Denied,
            entitlement_requirements: Vec::new(),
            evidence_refs: Vec::new(),
        }
    }

    fn adapter(secret_marker: &str) -> ForeignEntitlementAdapter {
        ForeignEntitlementAdapter {
            package_id: "tests/entitlement".to_string(),
            capability_id: "tests/entitlement/check".to_string(),
            version: Some("1.0.0".to_string()),
            input: serde_json::json!({"credential": secret_marker}),
        }
    }

    #[test]
    fn local_coordinates_are_redacted_and_secret_names_are_stable() {
        let binding = ForeignLaunchBinding {
            schema: FOREIGN_LAUNCH_BINDING_SCHEMA.to_string(),
            launch_id: "play.local".to_string(),
            target: ForeignLaunchTarget::LocalExecutable {
                executable: "C:\\private\\game.exe".to_string(),
                args: vec!["--profile".to_string(), "private-user".to_string()],
                working_directory: Some("C:\\private".to_string()),
            },
            entitlement: None,
        };
        binding
            .validate_for(&requirement(ForeignLaunchKind::LocalExecutable))
            .unwrap();
        let debug = format!("{binding:?}");
        assert!(!debug.contains("game.exe"));
        assert!(!debug.contains("private-user"));
        assert_eq!(
            foreign_launch_secret_ref("play.local"),
            foreign_launch_secret_ref("play.local")
        );
        assert_ne!(
            foreign_launch_secret_ref("play.local"),
            foreign_launch_secret_ref("server")
        );
    }

    #[test]
    fn target_kind_must_match_the_portable_requirement() {
        let binding = ForeignLaunchBinding {
            schema: FOREIGN_LAUNCH_BINDING_SCHEMA.to_string(),
            launch_id: "play.local".to_string(),
            target: ForeignLaunchTarget::RemoteService {
                endpoint: "https://example.invalid/rpc".to_string(),
            },
            entitlement: None,
        };
        assert!(binding
            .validate_for(&requirement(ForeignLaunchKind::LocalExecutable))
            .is_err());
    }

    #[test]
    fn oci_image_binding_rejects_docker_option_injection() {
        let binding = ForeignLaunchBinding {
            schema: FOREIGN_LAUNCH_BINDING_SCHEMA.to_string(),
            launch_id: "play.local".to_string(),
            target: ForeignLaunchTarget::OciImage {
                image: "--privileged".to_string(),
                args: Vec::new(),
            },
            entitlement: None,
        };
        assert!(binding
            .validate_for(&requirement(ForeignLaunchKind::OciImage))
            .is_err());
    }

    #[test]
    fn rights_outcome_is_conservative_when_a_declaration_exists() {
        assert_eq!(
            rights_policy_outcome(
                Some(&rights(RightDisposition::Denied)),
                RightsOperation::Execute
            ),
            RightsPolicyOutcome::Denied
        );
        assert_eq!(
            rights_policy_outcome(
                Some(&rights(RightDisposition::Unspecified)),
                RightsOperation::Execute
            ),
            RightsPolicyOutcome::Unspecified
        );
        assert_eq!(
            rights_policy_outcome(None, RightsOperation::Execute),
            RightsPolicyOutcome::Allowed
        );
        assert_eq!(
            rights_policy_outcome(None, RightsOperation::CopyAcrossHosts),
            RightsPolicyOutcome::Unspecified
        );
        assert_eq!(
            rights_policy_outcome(None, RightsOperation::Backup),
            RightsPolicyOutcome::Unspecified
        );
    }

    #[test]
    fn all_host_local_launch_target_shapes_validate_without_debug_disclosure() {
        let artifact = ArtifactDescriptor {
            artifact_type_uri: "urn:plurora:foreign-managed-binary:v1".to_string(),
            media_type: "application/octet-stream".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let values = [
            ForeignLaunchTarget::ExternalUri {
                uri: "https://example.invalid/play?opaque=coordinate".to_string(),
            },
            ForeignLaunchTarget::LocalExecutable {
                executable: "C:\\private\\game.exe".to_string(),
                args: vec!["--opaque".to_string()],
                working_directory: Some("C:\\private".to_string()),
            },
            ForeignLaunchTarget::ManagedArtifact {
                artifact,
                args: Vec::new(),
            },
            ForeignLaunchTarget::OciImage {
                image: "registry.invalid/private/game@sha256:opaque".to_string(),
                args: Vec::new(),
            },
            ForeignLaunchTarget::RemoteService {
                endpoint: "https://service.invalid/private".to_string(),
            },
            ForeignLaunchTarget::EntitlementAdapter {
                adapter: adapter("opaque-adapter-input"),
            },
        ];
        for target in values {
            let kind = target.kind();
            let binding = ForeignLaunchBinding {
                schema: FOREIGN_LAUNCH_BINDING_SCHEMA.to_string(),
                launch_id: "play.local".to_string(),
                target,
                entitlement: None,
            };
            binding.validate_for(&requirement(kind)).unwrap();
            let debug = format!("{binding:?}");
            assert!(!debug.contains("private"));
            assert!(!debug.contains("opaque"));
        }
    }

    #[tokio::test]
    async fn entitlement_failure_redacts_adapter_input_and_has_no_platform_drm_path(
    ) -> anyhow::Result<()> {
        let marker = "credential-must-never-escape";
        let runtime = Runtime::new(
            Arc::new(crate::InMemoryEventStore::default()),
            crate::RuntimeConfig::default(),
        );
        let binding = ForeignLaunchBinding {
            schema: FOREIGN_LAUNCH_BINDING_SCHEMA.to_string(),
            launch_id: "play.local".to_string(),
            target: ForeignLaunchTarget::RemoteService {
                endpoint: "https://service.invalid/play".to_string(),
            },
            entitlement: Some(adapter(marker)),
        };
        let error = activate_foreign_launch(
            &runtime,
            "session-without-special-authority",
            &binding,
            Some(&rights(RightDisposition::RequiresEntitlement)),
        )
        .await
        .expect_err("an unavailable ordinary capability must deny entitlement");
        let rendered = error.to_string();
        assert!(rendered.contains("entitlement_denied"));
        assert!(!rendered.contains(marker));
        assert!(!rendered.contains("credential"));
        Ok(())
    }
}
