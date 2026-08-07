use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::Context;
use fs2::FileExt as _;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::host_access::normalize_host_endpoint;

const LOCAL_HOST_ENDPOINT: &str = "http://127.0.0.1:8787";
const STATE_FILE: &str = "client-connections.json";
const MAX_CONNECTIONS: usize = 32;
const MAX_CONTEXTS: usize = 64;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct HostConnectionProfile {
    endpoint: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
struct HostProjectTargetContext {
    project_id: String,
    target_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct HostConnectionState {
    version: u32,
    #[serde(default)]
    active: Option<String>,
    #[serde(default)]
    profiles: BTreeMap<String, HostConnectionProfile>,
    #[serde(default)]
    contexts: BTreeMap<String, HostProjectTargetContext>,
}

impl Default for HostConnectionState {
    fn default() -> Self {
        Self {
            version: 1,
            active: None,
            profiles: BTreeMap::new(),
            contexts: BTreeMap::new(),
        }
    }
}

impl HostConnectionState {
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.version == 1,
            "Host connection state version is unsupported"
        );
        anyhow::ensure!(
            self.profiles.len() <= MAX_CONNECTIONS,
            "Host connection state has too many profiles"
        );
        anyhow::ensure!(
            self.contexts.len() <= MAX_CONTEXTS,
            "Host connection state has too many contexts"
        );
        for (name, profile) in &self.profiles {
            anyhow::ensure!(
                !name.eq_ignore_ascii_case("local"),
                "'local' is reserved for the default loopback Host"
            );
            anyhow::ensure!(
                normalized_name(name)? == *name,
                "Host connection name is invalid"
            );
            anyhow::ensure!(
                normalize_host_endpoint(&profile.endpoint)? == profile.endpoint,
                "Host connection endpoint is not normalized"
            );
        }
        if let Some(active) = &self.active {
            anyhow::ensure!(
                self.profiles.contains_key(active),
                "active Host connection does not exist"
            );
        }
        for (endpoint, context) in &self.contexts {
            anyhow::ensure!(
                normalize_host_endpoint(endpoint)? == *endpoint,
                "Host context endpoint is not normalized"
            );
            normalized_context_id(&context.project_id, "project")?;
            normalized_context_id(&context.target_id, "target")?;
        }
        Ok(())
    }

    fn endpoint(&self) -> &str {
        self.active
            .as_ref()
            .and_then(|name| self.profiles.get(name))
            .map(|profile| profile.endpoint.as_str())
            .unwrap_or(LOCAL_HOST_ENDPOINT)
    }

    fn save(&mut self, name: &str, endpoint: &str) -> anyhow::Result<()> {
        let name = normalized_name(name)?;
        anyhow::ensure!(
            !name.eq_ignore_ascii_case("local"),
            "'local' is reserved for the default loopback Host"
        );
        let endpoint = normalize_host_endpoint(endpoint)?;
        anyhow::ensure!(
            !self
                .profiles
                .iter()
                .any(|(existing_name, profile)| existing_name != &name
                    && profile.endpoint == endpoint),
            "another Host connection already uses this endpoint"
        );
        if !self.profiles.contains_key(&name) {
            anyhow::ensure!(
                self.profiles.len() < MAX_CONNECTIONS,
                "at most {MAX_CONNECTIONS} Host connections can be saved"
            );
        }
        let previous_endpoint = self
            .profiles
            .insert(name.clone(), HostConnectionProfile { endpoint })
            .map(|profile| profile.endpoint);
        if let Some(previous_endpoint) = previous_endpoint {
            if !self
                .profiles
                .values()
                .any(|profile| profile.endpoint == previous_endpoint)
            {
                self.contexts.remove(&previous_endpoint);
            }
        }
        self.active = Some(name);
        Ok(())
    }

    fn select(&mut self, name: &str) -> anyhow::Result<()> {
        let name = normalized_name(name)?;
        anyhow::ensure!(
            self.profiles.contains_key(&name),
            "Host connection does not exist"
        );
        self.active = Some(name);
        Ok(())
    }

    fn remove(&mut self, name: &str) -> anyhow::Result<()> {
        let name = normalized_name(name)?;
        let removed = self
            .profiles
            .remove(&name)
            .ok_or_else(|| anyhow::anyhow!("Host connection does not exist"))?;
        if self.active.as_deref() == Some(name.as_str()) {
            self.active = None;
        }
        if !self
            .profiles
            .values()
            .any(|profile| profile.endpoint == removed.endpoint)
        {
            self.contexts.remove(&removed.endpoint);
        }
        Ok(())
    }

    fn set_context(&mut self, project: &str, target: &str) -> anyhow::Result<()> {
        let project_id = normalized_context_id(project, "project")?;
        let target_id = normalized_context_id(target, "target")?;
        if !self.contexts.contains_key(self.endpoint()) {
            anyhow::ensure!(
                self.contexts.len() < MAX_CONTEXTS,
                "Host connection state has too many contexts"
            );
        }
        self.contexts.insert(
            self.endpoint().to_string(),
            HostProjectTargetContext {
                project_id,
                target_id,
            },
        );
        Ok(())
    }
}

pub(crate) struct ActiveHostContext {
    pub endpoint: String,
    pub project_id: Option<String>,
    pub target_id: Option<String>,
}

pub(crate) fn resolve(explicit_endpoint: Option<&str>) -> anyhow::Result<ActiveHostContext> {
    let state = read_state(&state_path()?)?;
    let endpoint = match explicit_endpoint {
        Some(endpoint) => normalize_host_endpoint(endpoint)?,
        None => state.endpoint().to_string(),
    };
    let context = state.contexts.get(&endpoint);
    Ok(ActiveHostContext {
        endpoint,
        project_id: context.map(|value| value.project_id.clone()),
        target_id: context.map(|value| value.target_id.clone()),
    })
}

pub fn list() -> anyhow::Result<()> {
    print_state(&read_state(&state_path()?)?)
}

pub fn save(name: &str, endpoint: &str) -> anyhow::Result<()> {
    mutate_state(|state| state.save(name, endpoint))
}

pub fn select(name: &str) -> anyhow::Result<()> {
    mutate_state(|state| state.select(name))
}

pub fn local() -> anyhow::Result<()> {
    mutate_state(|state| {
        state.active = None;
        Ok(())
    })
}

pub fn remove(name: &str) -> anyhow::Result<()> {
    mutate_state(|state| state.remove(name))
}

pub fn set_context(project: &str, target: &str) -> anyhow::Result<()> {
    mutate_state(|state| state.set_context(project, target))
}

pub fn clear_context() -> anyhow::Result<()> {
    mutate_state(|state| {
        let endpoint = state.endpoint().to_string();
        state.contexts.remove(&endpoint);
        Ok(())
    })
}

fn mutate_state(
    operation: impl FnOnce(&mut HostConnectionState) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let path = state_path()?;
    let state = with_state_lock(&path, || {
        let mut state = read_state(&path)?;
        operation(&mut state)?;
        state.validate()?;
        write_state(&path, &state)?;
        Ok(state)
    })?;
    print_state(&state)
}

fn state_path() -> anyhow::Result<PathBuf> {
    Ok(plurora_core::paths::data_dir()?.join(STATE_FILE))
}

fn read_state(path: &Path) -> anyhow::Result<HostConnectionState> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(HostConnectionState::default())
        }
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "Host connection state must be a regular file"
    );
    let state: HostConnectionState = serde_json::from_slice(
        &std::fs::read(path).context("failed to read Host connection state")?,
    )
    .context("Host connection state is invalid JSON")?;
    state.validate()?;
    Ok(state)
}

fn write_state(path: &Path, state: &HostConnectionState) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Host connection state path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    if path.exists() {
        let metadata = std::fs::symlink_metadata(path)?;
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Host connection state must be a regular file"
        );
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temporary, state)?;
    temporary.write_all(b"\n")?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .context("failed to atomically persist Host connection state")?;
    #[cfg(unix)]
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn with_state_lock<T>(
    path: &Path,
    operation: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Host connection state path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let lock_path = parent.join("client-connections.lock");
    if lock_path.exists() {
        let metadata = std::fs::symlink_metadata(&lock_path)?;
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Host connection lock must be a regular file"
        );
    }
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    lock.lock_exclusive()?;
    operation()
}

fn print_state(state: &HostConnectionState) -> anyhow::Result<()> {
    let active_endpoint = state.endpoint();
    let context = state.contexts.get(active_endpoint);
    let connections = std::iter::once(json!({
        "name": "local",
        "endpoint": LOCAL_HOST_ENDPOINT,
        "active": state.active.is_none(),
    }))
    .chain(state.profiles.iter().map(|(name, profile)| {
        json!({
            "name": name,
            "endpoint": profile.endpoint,
            "active": state.active.as_deref() == Some(name.as_str()),
        })
    }))
    .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "active": {
                "name": state.active.as_deref().unwrap_or("local"),
                "endpoint": active_endpoint,
                "project_id": context.map(|value| value.project_id.as_str()),
                "target_id": context.map(|value| value.target_id.as_str()),
            },
            "connections": connections,
        }))?
    );
    Ok(())
}

fn normalized_name(value: &str) -> anyhow::Result<String> {
    let normalized = value.trim();
    anyhow::ensure!(
        !normalized.is_empty()
            && normalized.len() <= 64
            && !normalized.chars().any(char::is_control),
        "Host connection name must be a bounded non-empty string"
    );
    Ok(normalized.to_string())
}

fn normalized_context_id(value: &str, kind: &str) -> anyhow::Result<String> {
    let normalized = value.trim();
    anyhow::ensure!(
        !normalized.is_empty()
            && normalized.len() <= 256
            && normalized
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._:/@-".contains(&byte)),
        "{kind} id is invalid"
    );
    Ok(normalized.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_context_is_scoped_by_host_and_contains_no_secret() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join(STATE_FILE);
        let mut state = HostConnectionState::default();
        state.set_context("local-project", "local")?;
        state.save("remote", "https://host.example/")?;
        state.set_context("remote-project", "target-a")?;
        write_state(&path, &state)?;

        let loaded = read_state(&path)?;
        assert_eq!(loaded.endpoint(), "https://host.example");
        assert_eq!(
            loaded.contexts["https://host.example"].project_id,
            "remote-project"
        );
        assert_eq!(
            loaded.contexts[LOCAL_HOST_ENDPOINT].project_id,
            "local-project"
        );
        let serialized = std::fs::read_to_string(path)?;
        assert!(!serialized.contains("token"));

        state.remove("remote")?;
        assert!(!state.contexts.contains_key("https://host.example"));
        Ok(())
    }

    #[test]
    fn duplicate_endpoints_and_unsafe_context_ids_are_rejected() -> anyhow::Result<()> {
        let mut state = HostConnectionState::default();
        state.save("one", "https://host.example")?;
        assert!(state.save("two", "https://host.example/").is_err());
        assert!(state.set_context("project one", "target-a").is_err());
        assert!(state.save("insecure", "http://host.example").is_err());
        Ok(())
    }

    #[test]
    fn state_lock_is_exclusive() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let state_path = directory.path().join(STATE_FILE);
        with_state_lock(&state_path, || {
            let competing_lock = OpenOptions::new()
                .read(true)
                .write(true)
                .open(directory.path().join("client-connections.lock"))?;
            assert!(competing_lock.try_lock_exclusive().is_err());
            Ok(())
        })
    }
}
