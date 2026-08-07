//! Project descriptors and identity.
//!
//! A "project" is a runtime instance of an installed package set, typically
//! with its own state, settings, secrets, and Home card. Projects are runtime
//! concepts, not kernel concepts — the kernel does not interpret them.
//!
//! Three project types:
//! - `plurora_native`: declares itself via `project.yaml`, references Plurora packages
//! - `external_wrapped`: an external project wrapped by an adapter package
//! - `external_workspace`: an external project running in an agent workspace

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProjectDescriptor {
    /// Schema version. Currently always 1.
    pub schema_version: u32,

    /// The actual project content.
    pub project: ProjectInner,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ProjectInner {
    /// Project id. Format: <safe_slug>__<short_hash>.
    /// Filesystem-safe (no /, no .., no shell special chars). Stable across upgrades.
    pub id: ProjectId,

    /// Display title for Home card.
    pub title: String,

    /// Description for tooltip / detail view.
    #[serde(default)]
    pub description: String,

    /// Project type discriminator.
    #[serde(rename = "type")]
    pub project_type: ProjectType,

    /// Optional icon path (relative to project root for native, optional for external).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,

    /// Entry surface id for click-to-play. Required for native; optional for external.
    /// For external_workspace, may point to a workspace-lab provided surface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_surface_id: Option<String>,

    /// Package manifest paths used by this project.
    /// For plurora_native: typically packages/* paths
    /// For external_wrapped: the adapter package
    /// For external_workspace: empty or workspace tooling
    #[serde(default)]
    pub packages: Vec<String>,

    /// Optional packages that may be loaded if available.
    #[serde(default)]
    pub optional_packages: Vec<String>,

    /// Required surface ids (composition-style validation).
    #[serde(default)]
    pub required_surfaces: Vec<String>,

    /// Required capabilities (composition-style validation).
    #[serde(default)]
    pub required_capabilities: Vec<String>,

    /// Secret policy for this project.
    #[serde(default)]
    pub secret_policy: SecretPolicy,

    /// Project type-specific data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external: Option<ExternalProjectData>,

    /// Free-form metadata for forward compat.
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

/// Newtype for project id with format validation.
#[derive(
    Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
#[serde(transparent)]
pub struct ProjectId(String);

impl ProjectId {
    /// Maximum allowed length for a project id.
    pub const MAX_LEN: usize = 128;
    /// Minimum allowed length.
    pub const MIN_LEN: usize = 1;

    /// Construct a ProjectId, validating format.
    pub fn new(s: impl Into<String>) -> anyhow::Result<Self> {
        let id = s.into();
        Self::validate(&id)?;
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(s: &str) -> anyhow::Result<()> {
        if s.is_empty() || s.len() > Self::MAX_LEN {
            anyhow::bail!(
                "project id length must be 1..={} (got {})",
                Self::MAX_LEN,
                s.len()
            );
        }
        // Reject path traversal and shell special chars.
        for ch in s.chars() {
            if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.') {
                anyhow::bail!("project id contains invalid character: {:?}", ch);
            }
        }
        if s == "." || s == ".." || s.starts_with('.') {
            anyhow::bail!("project id must not start with . or be . or ..");
        }
        if s.contains("..") {
            anyhow::bail!("project id must not contain ..");
        }
        Ok(())
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectType {
    /// Native Plurora project: has project.yaml at repo root, manifest references Plurora packages.
    PluroraNative,
    /// External project wrapped by an adapter package.
    ExternalWrapped,
    /// External project running in an agent workspace (no wrapping).
    ExternalWorkspace,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SecretPolicy {
    /// If true, secret_ref:project:NAME falls back to platform store when not found in project.
    /// If false, fail-closed.
    /// Default: true (convenience).
    #[serde(default = "default_fallback_to_platform")]
    pub fallback_to_platform: bool,

    /// Names that MUST be configured at project scope (no platform fallback for these specifically).
    /// Useful for sensitive secrets that should never accidentally use a shared platform key.
    #[serde(default)]
    pub require_per_project: Vec<String>,
}

fn default_fallback_to_platform() -> bool {
    true
}

impl Default for SecretPolicy {
    fn default() -> Self {
        Self {
            fallback_to_platform: true,
            require_per_project: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExternalProjectData {
    /// Source URL or path the external project was installed from.
    pub source: String,

    /// Resolved commit SHA or version (for git sources).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,

    /// For external_wrapped: the path to the adapter package's manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_manifest: Option<String>,

    /// For external_workspace: path to the fetched project tree under the workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,

    /// Kind of source that produced the managed workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<ExternalSourceKind>,

    /// Ownership boundary for the workspace path. Managed workspaces may be
    /// archived or deleted with the project; linked local sources never may.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_ownership: Option<ExternalWorkspaceOwnership>,

    /// Content digest of the materialized workspace tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_digest: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalSourceKind {
    Local,
    Git,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalWorkspaceOwnership {
    Managed,
    LinkedLocal,
}

/// Runtime project state. Not serialized to project.yaml; tracked by registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectState {
    /// Installed but not running.
    Installed,
    /// Stopped after running.
    Stopped,
    /// Currently transitioning to running.
    Starting,
    /// Currently running and serving.
    Running,
    /// Currently transitioning to stopped.
    Stopping,
    /// Failed to start or crashed.
    Failed,
    /// Soft-deleted, awaiting cleanup.
    Archived,
}

impl ProjectDescriptor {
    /// Validate the descriptor for shape and content invariants.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.schema_version != 1 {
            anyhow::bail!(
                "unsupported project schema version: {}",
                self.schema_version
            );
        }
        ProjectId::validate(self.project.id.as_str())?;
        if self.project.title.trim().is_empty() {
            anyhow::bail!("project title must not be empty");
        }
        if self.project.title.len() > 256 {
            anyhow::bail!("project title too long (max 256)");
        }
        // type-specific checks
        match self.project.project_type {
            ProjectType::PluroraNative => {
                if self.project.packages.is_empty() {
                    anyhow::bail!("plurora_native project must declare at least one package");
                }
                if self.project.entry_surface_id.is_none() {
                    anyhow::bail!("plurora_native project must declare entry_surface_id");
                }
            }
            ProjectType::ExternalWrapped => {
                if let Some(ext) = &self.project.external {
                    if ext.adapter_manifest.is_none() {
                        anyhow::bail!("external_wrapped requires external.adapter_manifest");
                    }
                } else {
                    anyhow::bail!("external_wrapped requires external section");
                }
            }
            ProjectType::ExternalWorkspace => {
                if let Some(ext) = &self.project.external {
                    if ext.workspace_root.is_none() {
                        anyhow::bail!("external_workspace requires external.workspace_root");
                    }
                } else {
                    anyhow::bail!("external_workspace requires external section");
                }
            }
        }
        // secret policy: require_per_project entries must be valid names (alphanumeric + _ - .)
        for n in &self.project.secret_policy.require_per_project {
            if n.is_empty() || n.len() > 128 {
                anyhow::bail!("require_per_project entry has invalid length: {}", n);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_id_accepts_safe_chars() {
        ProjectId::new("simple").unwrap();
        ProjectId::new("with-dashes").unwrap();
        ProjectId::new("with_underscores").unwrap();
        ProjectId::new("with.dots").unwrap();
        ProjectId::new("youzini-afk__YdlTavern__a1b2c3d4").unwrap();
    }

    #[test]
    fn project_id_rejects_path_traversal() {
        assert!(ProjectId::new("..").is_err());
        assert!(ProjectId::new("../escape").is_err());
        assert!(ProjectId::new(".hidden").is_err());
        assert!(ProjectId::new("a..b").is_err());
    }

    #[test]
    fn project_id_rejects_special_chars() {
        assert!(ProjectId::new("with/slash").is_err());
        assert!(ProjectId::new("with space").is_err());
        assert!(ProjectId::new("with\\backslash").is_err());
        assert!(ProjectId::new("with$dollar").is_err());
        assert!(ProjectId::new("").is_err());
    }

    #[test]
    fn descriptor_yaml_roundtrip_native() {
        let yaml = r#"
schema_version: 1
project:
  id: ydltavern-test__a1b2c3d4
  title: YdlTavern Test
  description: Test project for round-trip
  type: plurora_native
  entry_surface_id: foo/bar/play
  packages:
    - packages/foo/manifest.yaml
  secret_policy:
    fallback_to_platform: true
    require_per_project: []
"#;
        let parsed: ProjectDescriptor = serde_yaml::from_str(yaml).unwrap();
        parsed.validate().unwrap();
        let serialized = serde_yaml::to_string(&parsed).unwrap();
        let reparsed: ProjectDescriptor = serde_yaml::from_str(&serialized).unwrap();
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn descriptor_validates_native_requires_packages() {
        let mut d = make_native_descriptor();
        d.project.packages.clear();
        assert!(d.validate().is_err());
    }

    #[test]
    fn descriptor_validates_native_requires_entry_surface() {
        let mut d = make_native_descriptor();
        d.project.entry_surface_id = None;
        assert!(d.validate().is_err());
    }

    #[test]
    fn descriptor_validates_external_wrapped_requires_adapter() {
        let mut d = make_external_wrapped_descriptor();
        d.project.external.as_mut().unwrap().adapter_manifest = None;
        assert!(d.validate().is_err());
    }

    #[test]
    fn descriptor_validates_external_workspace_requires_root() {
        let mut d = make_external_workspace_descriptor();
        d.project.external.as_mut().unwrap().workspace_root = None;
        assert!(d.validate().is_err());
    }

    #[test]
    fn secret_policy_default_falls_back_to_platform() {
        let p = SecretPolicy::default();
        assert!(p.fallback_to_platform);
    }

    fn make_native_descriptor() -> ProjectDescriptor {
        ProjectDescriptor {
            schema_version: 1,
            project: ProjectInner {
                id: ProjectId::new("test__abc123").unwrap(),
                title: "Test".into(),
                description: String::new(),
                project_type: ProjectType::PluroraNative,
                icon: None,
                entry_surface_id: Some("foo/bar/play".into()),
                packages: vec!["packages/foo/manifest.yaml".into()],
                optional_packages: Vec::new(),
                required_surfaces: Vec::new(),
                required_capabilities: Vec::new(),
                secret_policy: SecretPolicy::default(),
                external: None,
                metadata: BTreeMap::new(),
            },
        }
    }

    fn make_external_wrapped_descriptor() -> ProjectDescriptor {
        let mut d = make_native_descriptor();
        d.project.project_type = ProjectType::ExternalWrapped;
        d.project.external = Some(ExternalProjectData {
            source: "https://github.com/foo/bar".into(),
            source_ref: Some("v1.0.0".into()),
            adapter_manifest: Some("packages/adapter/manifest.yaml".into()),
            workspace_root: None,
            source_kind: Some(ExternalSourceKind::Git),
            workspace_ownership: Some(ExternalWorkspaceOwnership::Managed),
            source_digest: Some("sha256:adapter".into()),
        });
        d
    }

    fn make_external_workspace_descriptor() -> ProjectDescriptor {
        let mut d = make_native_descriptor();
        d.project.project_type = ProjectType::ExternalWorkspace;
        d.project.external = Some(ExternalProjectData {
            source: "https://github.com/foo/bar".into(),
            source_ref: Some("main".into()),
            adapter_manifest: None,
            workspace_root: Some("/tmp/ws".into()),
            source_kind: Some(ExternalSourceKind::Git),
            workspace_ownership: Some(ExternalWorkspaceOwnership::Managed),
            source_digest: Some("sha256:workspace".into()),
        });
        d
    }
}
