use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use plurora_core::{LockEntry, LockSource, Lockfile, ProjectId, ProjectType};
use serde::Serialize;
use serde_json::{json, Value};

use super::executor::{compute_tree_hash, invoke_package_capability};
use super::layout::lockfile_path;
use super::project_kind::read_project_descriptor;
use super::source::value_str;
use super::types::CheckForUpdatesInput;

#[derive(Debug, Serialize)]
struct UpdateCheckRecord {
    id: String,
    package_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
    source_kind: String,
    applicable: bool,
    status: &'static str,
    reason: String,
    available: bool,
    dangling: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    upstream_commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_tree_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    available_tree_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    installed_at_store: Option<String>,
}

#[derive(Debug, Clone)]
struct ProjectPackageFilter {
    project_id: String,
    package_ids: Vec<String>,
    manifest_relatives: Vec<String>,
}

#[derive(Debug, Clone)]
enum ProjectUpdateTarget {
    Native(ProjectPackageFilter),
    External {
        project_id: String,
        project_type: ProjectType,
    },
}

pub(super) async fn check_for_updates(input: Value) -> Result<Value> {
    let input: CheckForUpdatesInput = serde_json::from_value(input)?;
    let lockfile_path = lockfile_path(&input.profile, input.data_dir.as_deref())?;
    if !lockfile_path.exists() {
        if let Some(project_id) = input.project_id.as_deref() {
            if let Some(record) =
                external_project_record_if_exists(project_id, input.data_dir.as_deref())?
            {
                return Ok(json!({ "results": [record] }));
            }
        }
        return Ok(json!({ "results": [] }));
    }

    let lock: Lockfile = toml::from_str(&fs::read_to_string(lockfile_path)?)?;
    let project_target = input
        .project_id
        .as_deref()
        .map(|project_id| project_update_target(project_id, input.data_dir.as_deref(), &lock))
        .transpose()?;
    if let Some(ProjectUpdateTarget::External {
        project_id,
        project_type,
    }) = project_target.as_ref()
    {
        return Ok(json!({
            "results": [external_project_record(project_id, project_type)]
        }));
    }
    let project_filter = project_target.as_ref().map(|target| match target {
        ProjectUpdateTarget::Native(filter) => filter,
        ProjectUpdateTarget::External { .. } => unreachable!("external projects returned above"),
    });

    let mut results = Vec::new();
    for entry in lock.package.iter().filter(|entry| {
        input
            .package_id
            .as_ref()
            .is_none_or(|package_id| package_id == &entry.id)
            && project_filter.is_none_or(|filter| entry_matches_project(entry, filter))
    }) {
        let project_id = project_filter.and_then(|filter| {
            entry_matches_project(entry, filter).then(|| filter.project_id.clone())
        });
        results.push(check_entry(entry, project_id).await);
    }

    results.sort_by(|left, right| {
        left.project_id
            .cmp(&right.project_id)
            .then_with(|| left.package_id.cmp(&right.package_id))
            .then_with(|| left.source_kind.cmp(&right.source_kind))
    });
    Ok(json!({ "results": results }))
}

async fn check_entry(entry: &LockEntry, project_id: Option<String>) -> UpdateCheckRecord {
    let source_kind = source_kind(&entry.source).to_string();
    let dangling = !Path::new(&entry.installed_at_store).is_dir();
    let mut base = UpdateCheckRecord {
        id: entry.id.clone(),
        package_id: entry.id.clone(),
        project_id,
        source_kind,
        applicable: true,
        status: "current",
        reason: "current".to_string(),
        available: false,
        dangling,
        current_commit: entry.commit.clone(),
        upstream_commit: None,
        current_tree_hash: Some(entry.tree_hash.clone()),
        available_tree_hash: None,
        installed_at_store: Some(entry.installed_at_store.clone()),
    };

    match entry.source {
        LockSource::Git => check_git_entry(entry, base).await,
        LockSource::Local => check_local_entry(entry, base).await,
        LockSource::Internal => {
            base.applicable = false;
            base.status = "not_applicable";
            base.reason = "internal source is managed by the host".to_string();
            base.available = false;
            base
        }
    }
}

async fn check_git_entry(entry: &LockEntry, mut record: UpdateCheckRecord) -> UpdateCheckRecord {
    let Some(url) = entry.url.as_deref().filter(|url| !url.is_empty()) else {
        record.status = if record.dangling {
            "repair_required"
        } else {
            "check_failed"
        };
        record.reason = "git source missing url".to_string();
        record.available = record.dangling;
        return record;
    };
    let ref_name = entry.r#ref.as_deref().unwrap_or("HEAD");
    match invoke_package_capability(
        "plurora/git-tools-lab",
        "plurora/git-tools-lab/resolve_ref",
        json!({ "remote_url": url, "ref": ref_name }),
    )
    .await
    {
        Ok(output) => {
            let upstream = value_str(&output, "commit_sha").map(str::to_string);
            match upstream {
                Ok(upstream) => {
                    record.upstream_commit = Some(upstream.clone());
                    let changed = entry.commit.as_deref() != Some(upstream.as_str());
                    apply_status(
                        &mut record,
                        changed,
                        "upstream commit differs from lockfile",
                        "git ref resolves to locked commit",
                    );
                }
                Err(error) => mark_check_failed(&mut record, error),
            }
        }
        Err(error) => mark_check_failed(&mut record, error),
    }
    record
}

async fn check_local_entry(entry: &LockEntry, mut record: UpdateCheckRecord) -> UpdateCheckRecord {
    let Some(source_path) = local_source_path(entry) else {
        record.status = if record.dangling {
            "repair_required"
        } else {
            "not_applicable"
        };
        record.applicable = record.dangling;
        record.available = record.dangling;
        record.reason = if record.dangling {
            "installed store path is missing and lockfile entry has no absolute local source_path"
                .to_string()
        } else {
            "local source update check requires an absolute source_path in the lockfile entry"
                .to_string()
        };
        return record;
    };

    match compute_tree_hash(&source_path).await {
        Ok(tree_hash) => {
            record.available_tree_hash = Some(tree_hash.clone());
            let changed = tree_hash != entry.tree_hash;
            apply_status(
                &mut record,
                changed,
                "local tree hash differs from lockfile",
                "local tree hash matches lockfile",
            );
        }
        Err(error) => mark_check_failed(&mut record, error),
    }
    record
}

fn apply_status(
    record: &mut UpdateCheckRecord,
    changed: bool,
    changed_reason: &str,
    current_reason: &str,
) {
    if record.dangling {
        record.status = "repair_required";
        record.available = true;
        record.reason = if changed {
            format!("installed store path is missing; {changed_reason}")
        } else {
            "installed store path is missing".to_string()
        };
    } else if changed {
        record.status = "update_available";
        record.available = true;
        record.reason = changed_reason.to_string();
    } else {
        record.status = "current";
        record.available = false;
        record.reason = current_reason.to_string();
    }
}

fn mark_check_failed(record: &mut UpdateCheckRecord, error: impl std::fmt::Display) {
    if record.dangling {
        record.status = "repair_required";
        record.available = true;
        record.reason = format!("installed store path is missing; update check failed: {error}");
    } else {
        record.status = "check_failed";
        record.available = false;
        record.reason = format!("update check failed: {error}");
    }
}

fn project_update_target(
    project_id: &str,
    data_dir_override: Option<&str>,
    lock: &Lockfile,
) -> Result<ProjectUpdateTarget> {
    let project_id = ProjectId::new(project_id)?;
    let descriptor_path = project_dir(data_dir_override, &project_id).join("project.yaml");
    let descriptor = read_project_descriptor(&descriptor_path)?;
    if matches!(
        descriptor.project.project_type,
        ProjectType::ExternalWrapped | ProjectType::ExternalWorkspace
    ) {
        return Ok(ProjectUpdateTarget::External {
            project_id: project_id.as_str().to_string(),
            project_type: descriptor.project.project_type,
        });
    }

    let mut manifest_relatives = descriptor.project.packages.clone();
    manifest_relatives.sort();
    manifest_relatives.dedup();

    let mut package_ids = lock
        .package
        .iter()
        .filter(|entry| {
            entry
                .manifest_relative_path
                .as_ref()
                .is_some_and(|relative| {
                    manifest_relatives
                        .iter()
                        .any(|candidate| candidate == relative)
                })
        })
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    package_ids.sort();
    package_ids.dedup();

    Ok(ProjectUpdateTarget::Native(ProjectPackageFilter {
        project_id: project_id.as_str().to_string(),
        package_ids,
        manifest_relatives,
    }))
}

fn entry_matches_project(entry: &LockEntry, filter: &ProjectPackageFilter) -> bool {
    entry
        .manifest_relative_path
        .as_ref()
        .is_some_and(|relative| {
            filter
                .manifest_relatives
                .iter()
                .any(|candidate| candidate == relative)
        })
        || filter
            .package_ids
            .iter()
            .any(|package_id| package_id == &entry.id)
}

fn local_source_path(entry: &LockEntry) -> Option<PathBuf> {
    entry
        .source_path
        .as_deref()
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn external_project_record_if_exists(
    project_id: &str,
    data_dir_override: Option<&str>,
) -> Result<Option<UpdateCheckRecord>> {
    let project_id = ProjectId::new(project_id)?;
    let descriptor_path = project_dir(data_dir_override, &project_id).join("project.yaml");
    if !descriptor_path.is_file() {
        return Ok(None);
    }
    let descriptor = read_project_descriptor(&descriptor_path)?;
    Ok(matches!(
        descriptor.project.project_type,
        ProjectType::ExternalWrapped | ProjectType::ExternalWorkspace
    )
    .then(|| external_project_record(project_id.as_str(), &descriptor.project.project_type)))
}

fn external_project_record(project_id: &str, project_type: &ProjectType) -> UpdateCheckRecord {
    UpdateCheckRecord {
        id: project_id.to_string(),
        package_id: project_id.to_string(),
        project_id: Some(project_id.to_string()),
        source_kind: project_type_source_kind(project_type).to_string(),
        applicable: false,
        status: "not_applicable",
        reason: "external project updates are not handled by install-lab check_for_updates"
            .to_string(),
        available: false,
        dangling: false,
        current_commit: None,
        upstream_commit: None,
        current_tree_hash: None,
        available_tree_hash: None,
        installed_at_store: None,
    }
}

fn project_type_source_kind(project_type: &ProjectType) -> &'static str {
    match project_type {
        ProjectType::PluroraNative => "project",
        ProjectType::ExternalWrapped => "external_wrapped",
        ProjectType::ExternalWorkspace => "external_workspace",
    }
}

fn project_dir(data_dir_override: Option<&str>, id: &ProjectId) -> PathBuf {
    if let Some(dir) = data_dir_override {
        PathBuf::from(dir).join("projects").join(id.as_str())
    } else {
        plurora_core::paths::project_dir(id)
            .unwrap_or_else(|_| PathBuf::from("projects").join(id.as_str()))
    }
}

fn source_kind(source: &LockSource) -> &'static str {
    match source {
        LockSource::Internal => "internal",
        LockSource::Git => "git",
        LockSource::Local => "local",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InMemoryEventStore, Runtime, RuntimeConfig};
    use plurora_core::{
        CapabilityDescriptor, EntryDescriptor, LockEntry, PackageEntry, PackageManifest,
        PermissionSet,
    };
    use std::sync::Arc;

    fn hash(ch: char) -> String {
        format!("sha256:{}", ch.to_string().repeat(64))
    }

    fn lock_entry(id: &str, source: LockSource, tree_hash: String, store: &Path) -> LockEntry {
        LockEntry {
            id: id.to_string(),
            version: "0.1.0".to_string(),
            source,
            url: None,
            source_path: None,
            r#ref: None,
            commit: None,
            tree_hash,
            manifest_hash: hash('m'),
            surface_bundle_hash: None,
            package_envelope_digest: None,
            component_pins: Vec::new(),
            protocol_profile_pins: Vec::new(),
            content_roots: Vec::new(),
            signed: false,
            signed_by: None,
            installed_at_store: store.to_string_lossy().to_string(),
            manifest_relative_path: None,
            granted_capabilities: Vec::new(),
            granted_network: Vec::new(),
            granted_secrets: Vec::new(),
            requires: Vec::new(),
        }
    }

    fn write_lockfile(data: &Path, entry: LockEntry) -> Result<()> {
        fs::create_dir_all(data.join("profiles"))?;
        let mut lock = Lockfile::new("default", hash('p'));
        lock.package.push(entry);
        fs::write(
            data.join("profiles/default.lock.toml"),
            toml::to_string_pretty(&lock)?,
        )?;
        Ok(())
    }

    fn write_manifest(dir: &Path, id: &str, source_path: Option<&Path>) -> Result<()> {
        fs::create_dir_all(dir)?;
        let metadata = source_path
            .map(|path| format!("metadata:\n  source_path: {}\n", path.display()))
            .unwrap_or_default();
        fs::write(
            dir.join("manifest.yaml"),
            format!(
                "schema_version: 1\nid: {id}\nversion: 0.1.0\nentry:\n  kind: rust_inproc\n  crate_ref: fixture\n  contract: v1\n  symbol: register\n  abi_version: 1\nprovides: []\npermissions: {{}}\n{metadata}"
            ),
        )?;
        Ok(())
    }

    async fn test_runtime() -> Result<Runtime<InMemoryEventStore>> {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        runtime
            .load_package(package_manifest(
                "plurora/integrity-lab",
                "plurora-foundation",
                "register",
                vec![CapabilityDescriptor {
                    id: "plurora/integrity-lab/compute_tree_hash".to_string(),
                    version: "1.0.0".to_string(),
                    input_schema: json!({}),
                    output_schema: json!({}),
                    streaming: false,
                    side_effects: Vec::new(),
                    description: None,
                }],
                PermissionSet::default(),
            ))
            .await?;
        let mut permissions = PermissionSet::default();
        permissions
            .capabilities
            .invoke
            .push("plurora/integrity-lab/*".to_string());
        runtime
            .load_package(package_manifest(
                "plurora/install-lab",
                "plurora-install-lab",
                "plurora_install_lab",
                Vec::new(),
                permissions,
            ))
            .await?;
        Ok(runtime)
    }

    fn package_manifest(
        id: &str,
        crate_ref: &str,
        symbol: &str,
        provides: Vec<CapabilityDescriptor>,
        permissions: PermissionSet,
    ) -> PackageManifest {
        PackageManifest {
            schema_version: 1,
            id: id.to_string(),
            version: "0.1.0".to_string(),
            display_name: None,
            description: None,
            author: None,
            license: None,
            entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                crate_ref: crate_ref.to_string(),
                symbol: symbol.to_string(),
                abi_version: 1,
            }),
            provides,
            consumes: Vec::new(),
            requires: Vec::new(),
            contributes: Default::default(),
            permissions,
            sandbox_policy: Default::default(),
        }
    }

    #[tokio::test]
    async fn local_source_reports_current_and_changed_tree_hash() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let data = tmp.path().join("data");
        let source = tmp.path().join("source");
        write_manifest(&source, "fixture/local", Some(&source))?;
        fs::write(source.join("content.txt"), "one")?;
        let runtime = test_runtime().await?;
        let tree_hash =
            crate::inproc::with_runtime_invoker(runtime.clone(), None, compute_tree_hash(&source))
                .await?;
        let store = data.join("store/current");
        write_manifest(&store, "fixture/local", Some(&source))?;
        let mut entry = lock_entry("fixture/local", LockSource::Local, tree_hash, &store);
        entry.source_path = Some(source.to_string_lossy().to_string());
        write_lockfile(&data, entry)?;

        let current = crate::inproc::with_runtime_invoker(
            runtime.clone(),
            None,
            check_for_updates(json!({ "data_dir": data })),
        )
        .await?;
        assert_eq!(current["results"][0]["status"], json!("current"));
        assert_eq!(current["results"][0]["available"], json!(false));

        fs::write(source.join("content.txt"), "two")?;
        let changed = crate::inproc::with_runtime_invoker(
            runtime,
            None,
            check_for_updates(json!({ "data_dir": tmp.path().join("data") })),
        )
        .await?;
        assert_eq!(changed["results"][0]["status"], json!("update_available"));
        assert_eq!(changed["results"][0]["available"], json!(true));
        assert_ne!(
            changed["results"][0]["current_tree_hash"],
            changed["results"][0]["available_tree_hash"]
        );
        Ok(())
    }

    #[tokio::test]
    async fn local_source_without_absolute_lock_source_path_is_not_applicable() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let data = tmp.path().join("data");
        let source = tmp.path().join("source");
        write_manifest(&source, "fixture/local", Some(&source))?;
        let store = data.join("store/current");
        write_manifest(&store, "fixture/local", Some(&source))?;
        let entry = lock_entry("fixture/local", LockSource::Local, hash('l'), &store);
        write_lockfile(&data, entry)?;

        let checked = check_for_updates(json!({ "data_dir": data })).await?;
        assert_eq!(checked["results"][0]["status"], json!("not_applicable"));
        assert_eq!(checked["results"][0]["applicable"], json!(false));
        assert_eq!(checked["results"][0]["available"], json!(false));
        assert_eq!(checked["results"][0]["source_kind"], json!("local"));
        assert!(checked["results"][0]["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("source_path"));
        Ok(())
    }

    #[tokio::test]
    async fn dangling_store_reports_repair_required() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let data = tmp.path().join("data");
        let source = tmp.path().join("source");
        write_manifest(&source, "fixture/local", Some(&source))?;
        let runtime = test_runtime().await?;
        let tree_hash =
            crate::inproc::with_runtime_invoker(runtime.clone(), None, compute_tree_hash(&source))
                .await?;
        let missing_store = data.join("store/missing");
        write_lockfile(
            &data,
            lock_entry(
                "fixture/local",
                LockSource::Local,
                tree_hash,
                &missing_store,
            ),
        )?;

        let checked = crate::inproc::with_runtime_invoker(
            runtime,
            None,
            check_for_updates(json!({ "data_dir": data })),
        )
        .await?;
        assert_eq!(checked["results"][0]["status"], json!("repair_required"));
        assert_eq!(checked["results"][0]["dangling"], json!(true));
        assert_eq!(checked["results"][0]["available"], json!(true));
        Ok(())
    }

    #[tokio::test]
    async fn external_project_update_check_returns_single_not_applicable_record() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let data = tmp.path().join("data");
        let store = data.join("store/local");
        fs::create_dir_all(&store)?;
        let source = tmp.path().join("source");
        fs::create_dir_all(&source)?;
        let mut entry = lock_entry("fixture/local", LockSource::Local, hash('l'), &store);
        entry.source_path = Some(source.to_string_lossy().to_string());
        write_lockfile(&data, entry)?;

        let project_id = "external__abc123";
        let project_dir = data.join("projects").join(project_id);
        fs::create_dir_all(&project_dir)?;
        fs::write(
            project_dir.join("project.yaml"),
            format!(
                "schema_version: 1\nproject:\n  id: {project_id}\n  title: External\n  type: external_workspace\n  packages: []\n  external:\n    source: {}\n    workspace_root: {}\n",
                source.display(),
                source.display()
            ),
        )?;

        let checked = check_for_updates(json!({
            "data_dir": data,
            "project_id": project_id,
        }))
        .await?;
        let results = checked["results"].as_array().expect("results array");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["status"], json!("not_applicable"));
        assert_eq!(results[0]["available"], json!(false));
        assert_eq!(results[0]["project_id"], json!(project_id));
        assert_eq!(results[0]["source_kind"], json!("external_workspace"));
        assert_eq!(results[0]["package_id"], json!(project_id));
        Ok(())
    }

    #[tokio::test]
    async fn unsupported_source_reports_not_applicable() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let data = tmp.path().join("data");
        let store = data.join("store/internal");
        fs::create_dir_all(&store)?;
        write_lockfile(
            &data,
            lock_entry("plurora/internal", LockSource::Internal, hash('i'), &store),
        )?;

        let checked = check_for_updates(json!({ "data_dir": data })).await?;
        assert_eq!(checked["results"][0]["status"], json!("not_applicable"));
        assert_eq!(checked["results"][0]["applicable"], json!(false));
        Ok(())
    }
}
