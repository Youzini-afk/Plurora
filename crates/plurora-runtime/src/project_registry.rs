//! In-memory registry of installed projects with state tracking.
//!
//! On host startup, scans `<data_dir>/projects/<id>/project.yaml` and loads
//! descriptors into the registry. State transitions (Installed → Starting →
//! Running → Stopping → Stopped → Failed) are tracked in memory.

use std::collections::HashMap;
use std::sync::RwLock;

use std::io::Read;

use anyhow::Context;
use plurora_core::project::{ProjectDescriptor, ProjectId, ProjectState};
use same_file::Handle;

#[derive(Default)]
pub struct ProjectRegistry {
    inner: RwLock<HashMap<ProjectId, ProjectEntry>>,
}

#[derive(Clone, Debug)]
pub struct ProjectEntry {
    pub descriptor: ProjectDescriptor,
    pub state: ProjectState,
}

impl ProjectRegistry {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Register a project (typically called when an installed project's
    /// project.yaml is read at host startup).
    pub fn register(&self, descriptor: ProjectDescriptor) -> anyhow::Result<()> {
        descriptor.validate()?;
        let mut guard = self.inner.write().expect("project registry poisoned");
        guard.insert(
            descriptor.project.id.clone(),
            ProjectEntry {
                descriptor,
                state: ProjectState::Installed,
            },
        );
        Ok(())
    }

    pub fn unregister(&self, id: &ProjectId) -> Option<ProjectEntry> {
        self.inner
            .write()
            .expect("project registry poisoned")
            .remove(id)
    }

    pub fn get(&self, id: &ProjectId) -> Option<ProjectEntry> {
        self.inner
            .read()
            .expect("project registry poisoned")
            .get(id)
            .cloned()
    }

    pub fn list(&self) -> Vec<ProjectEntry> {
        self.inner
            .read()
            .expect("project registry poisoned")
            .values()
            .cloned()
            .collect()
    }

    pub fn set_state(&self, id: &ProjectId, state: ProjectState) -> anyhow::Result<()> {
        let mut guard = self.inner.write().expect("project registry poisoned");
        let entry = guard
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("project '{}' not registered", id))?;
        entry.state = state;
        Ok(())
    }

    /// Discover and load all projects under <data_dir>/projects/.
    pub fn load_from_disk(&self) -> anyhow::Result<usize> {
        let projects_dir = plurora_core::paths::projects_dir()?;
        self.load_from_projects_dir(&projects_dir)
    }

    /// Discover and load all projects under an explicit projects directory.
    pub fn load_from_projects_dir(&self, projects_dir: &std::path::Path) -> anyhow::Result<usize> {
        let projects_metadata = match std::fs::symlink_metadata(projects_dir) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(error.into()),
        };
        anyhow::ensure!(
            projects_metadata.is_dir() && !projects_metadata.file_type().is_symlink(),
            "projects root must be a real directory, not a symlink: {}",
            projects_dir.display()
        );
        let projects_dir = std::fs::canonicalize(projects_dir).with_context(|| {
            format!(
                "failed to canonicalize projects directory {}",
                projects_dir.display()
            )
        })?;

        let mut count = 0;
        for entry in std::fs::read_dir(&projects_dir).with_context(|| {
            format!(
                "failed to read projects directory {}",
                projects_dir.display()
            )
        })? {
            let entry = entry.with_context(|| {
                format!(
                    "failed to read projects directory entry in {}",
                    projects_dir.display()
                )
            })?;
            let path = entry.path();
            // Skip .archived and other dot-dirs.
            let name = match path.file_name().and_then(|s| s.to_str()) {
                Some(n) => n,
                None => continue,
            };
            if name.starts_with('.') {
                continue;
            }
            let metadata = std::fs::symlink_metadata(&path).with_context(|| {
                format!("failed to inspect project directory {}", path.display())
            })?;
            if metadata.file_type().is_symlink() {
                anyhow::bail!(
                    "project directory must be a real directory, not a symlink: {}",
                    path.display()
                );
            }
            if !metadata.is_dir() {
                continue;
            }
            let project_dir = std::fs::canonicalize(&path).with_context(|| {
                format!(
                    "failed to canonicalize project directory {}",
                    path.display()
                )
            })?;
            anyhow::ensure!(
                project_dir.parent() == Some(projects_dir.as_path()),
                "project directory escaped the projects root: {}",
                project_dir.display()
            );

            let descriptor_path = project_dir.join("project.yaml");
            let descriptor_metadata = match std::fs::symlink_metadata(&descriptor_path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            anyhow::ensure!(
                descriptor_metadata.is_file() && !descriptor_metadata.file_type().is_symlink(),
                "project descriptor must be a real file, not a symlink: {}",
                descriptor_path.display()
            );

            let descriptor_file = std::fs::File::open(&descriptor_path).with_context(|| {
                format!(
                    "failed to open project descriptor {}",
                    descriptor_path.display()
                )
            })?;
            let opened_handle = Handle::from_file(descriptor_file.try_clone()?)?;
            let opened_metadata = descriptor_file.metadata()?;
            let current_metadata = std::fs::symlink_metadata(&descriptor_path)?;
            anyhow::ensure!(
                opened_metadata.is_file()
                    && current_metadata.is_file()
                    && !current_metadata.file_type().is_symlink()
                    && Handle::from_path(&descriptor_path)? == opened_handle,
                "project descriptor changed while it was being opened: {}",
                descriptor_path.display()
            );
            let mut yaml = String::new();
            descriptor_file
                .take(1024 * 1024 + 1)
                .read_to_string(&mut yaml)
                .with_context(|| {
                    format!(
                        "failed to read project descriptor {}",
                        descriptor_path.display()
                    )
                })?;
            anyhow::ensure!(
                yaml.len() <= 1024 * 1024,
                "project descriptor exceeds the 1 MiB limit: {}",
                descriptor_path.display()
            );
            let descriptor: ProjectDescriptor = serde_yaml::from_str(&yaml).map_err(|e| {
                anyhow::anyhow!("invalid project.yaml at {}: {e}", descriptor_path.display())
            })?;
            anyhow::ensure!(
                descriptor.project.id.as_str() == name,
                "project descriptor id '{}' does not match directory '{}'",
                descriptor.project.id,
                name
            );

            self.register(descriptor)?;
            count += 1;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plurora_core::project::{ProjectInner, ProjectType, SecretPolicy};

    fn descriptor(id: &str) -> ProjectDescriptor {
        ProjectDescriptor {
            schema_version: 1,
            project: ProjectInner {
                id: ProjectId::new(id).expect("valid project id"),
                title: id.to_string(),
                description: String::new(),
                project_type: ProjectType::ExternalWorkspace,
                icon: None,
                packages: Vec::new(),
                optional_packages: Vec::new(),
                required_surfaces: Vec::new(),
                required_capabilities: Vec::new(),
                entry_surface_id: None,
                external: Some(plurora_core::project::ExternalProjectData {
                    source: "https://example.invalid/repo.git".to_string(),
                    source_ref: None,
                    adapter_manifest: None,
                    workspace_root: Some("C:/managed/workspace".to_string()),
                    source_kind: None,
                    workspace_ownership: None,
                    source_digest: None,
                }),
                secret_policy: SecretPolicy::default(),
                metadata: std::collections::BTreeMap::new(),
            },
        }
    }

    #[test]
    fn project_directory_name_must_match_descriptor_id() -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        let project_dir = root.path().join("expected");
        std::fs::create_dir(&project_dir)?;
        std::fs::write(
            project_dir.join("project.yaml"),
            serde_yaml::to_string(&descriptor("different"))?,
        )?;

        let registry = ProjectRegistry::new();
        assert!(registry.load_from_projects_dir(root.path()).is_err());
        assert!(registry.list().is_empty());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn project_registry_rejects_symlinked_project_directory() -> anyhow::Result<()> {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        std::fs::write(
            outside.path().join("project.yaml"),
            serde_yaml::to_string(&descriptor("linked"))?,
        )?;
        symlink(outside.path(), root.path().join("linked"))?;

        let registry = ProjectRegistry::new();
        assert!(registry.load_from_projects_dir(root.path()).is_err());
        assert!(registry.list().is_empty());
        Ok(())
    }
}
