use std::fs::{self, Permissions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use plurora_runtime::ContractMaturity;
use serde::Serialize;

const MAX_MIGRATION_FILE_BYTES: u64 = 8 * 1024 * 1024;
const SKIPPED_DIRECTORIES: &[&str] = &[
    ".git",
    ".next",
    ".turbo",
    ".yarn",
    "build",
    "coverage",
    "dist",
    "node_modules",
    "target",
    "vendor",
];
const SUPPORTED_SOURCE_EXTENSIONS: &[&str] = &[
    "bash", "c", "cc", "cjs", "cpp", "cs", "fish", "go", "h", "hpp", "htm", "html", "java", "js",
    "jsx", "kt", "kts", "md", "mdx", "mjs", "php", "ps1", "py", "rb", "rs", "sh", "svelte",
    "swift", "ts", "tsx", "vue", "zsh",
];

#[derive(Debug, Clone)]
struct ReplacementSpec {
    legacy_id: String,
    canonical_id: String,
    maturity: ContractMaturity,
    support_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ContractMigrationReplacement {
    legacy_id: String,
    canonical_id: String,
    maturity: ContractMaturity,
    #[serde(skip_serializing_if = "Option::is_none")]
    support_until: Option<String>,
    occurrences: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ContractMigrationEdit {
    path: String,
    replacements: Vec<ContractMigrationReplacement>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ContractMigrationSkip {
    path: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ContractMigrationReport {
    contract_registry_version: String,
    root: String,
    mode: String,
    scope: String,
    scanned_files: usize,
    skipped_files: usize,
    changed_files: usize,
    replacement_count: usize,
    edits: Vec<ContractMigrationEdit>,
    skipped: Vec<ContractMigrationSkip>,
    excluded_paths: Vec<ContractMigrationSkip>,
}

struct PlannedWrite {
    path: PathBuf,
    original: Vec<u8>,
    migrated: Vec<u8>,
    permissions: Permissions,
}

pub(crate) async fn migrate(
    path: PathBuf,
    write: bool,
    json: bool,
    all_aliases: bool,
) -> Result<()> {
    let report = migrate_path(&path, write, all_aliases)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        let action = if write { "applied" } else { "previewed" };
        println!(
            "Contract Registry {} migration {action} ({}): {} replacement(s) in {} file(s); {} file(s) scanned, {} skipped.",
            report.contract_registry_version,
            report.scope,
            report.replacement_count,
            report.changed_files,
            report.scanned_files,
            report.skipped_files
        );
        for edit in &report.edits {
            println!("{}", edit.path);
            for replacement in &edit.replacements {
                println!(
                    "  {} -> {} ({:?}, {})",
                    replacement.legacy_id,
                    replacement.canonical_id,
                    replacement.maturity,
                    replacement.occurrences
                );
            }
        }
        for skipped in &report.skipped {
            println!("skipped {}: {}", skipped.path, skipped.reason);
        }
        for excluded in &report.excluded_paths {
            println!("excluded {}: {}", excluded.path, excluded.reason);
        }
        if !write && report.replacement_count > 0 {
            println!("Run again with --write to apply this migration.");
        }
    }
    Ok(())
}

fn migrate_path(path: &Path, write: bool, all_aliases: bool) -> Result<ContractMigrationReport> {
    anyhow::ensure!(
        path.exists(),
        "migration path does not exist: {}",
        path.display()
    );
    let root = fs::canonicalize(path)
        .with_context(|| format!("resolving migration path {}", path.display()))?;
    let mut files = Vec::new();
    let mut excluded_paths = Vec::new();
    collect_files(&root, &root, &mut files, &mut excluded_paths)?;
    files.sort();

    let replacements = plurora_runtime::contract_aliases()
        .iter()
        .filter(|alias| all_aliases || alias.deprecated_in.is_some())
        .filter_map(|alias| {
            alias.replacement.as_ref().and_then(|replacement| {
                (replacement != &alias.id).then(|| ReplacementSpec {
                    legacy_id: alias.id.clone(),
                    canonical_id: replacement.clone(),
                    maturity: alias.maturity,
                    support_until: alias.support_until.clone(),
                })
            })
        })
        .collect::<Vec<_>>();

    let mut scanned_files = 0;
    let mut replacement_count = 0;
    let mut edits = Vec::new();
    let mut skipped = Vec::new();
    let mut writes = Vec::new();

    for file in files {
        let display_path = migration_display_path(&root, &file);
        if !is_supported_source_file(&file) {
            skipped.push(ContractMigrationSkip {
                path: display_path,
                reason: "unsupported file type".to_string(),
            });
            continue;
        }
        let metadata = fs::metadata(&file)
            .with_context(|| format!("reading metadata for {}", file.display()))?;
        if metadata.len() > MAX_MIGRATION_FILE_BYTES {
            skipped.push(ContractMigrationSkip {
                path: display_path,
                reason: format!("file exceeds {MAX_MIGRATION_FILE_BYTES} bytes"),
            });
            continue;
        }
        let original = fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
        let Ok(mut contents) = String::from_utf8(original.clone()) else {
            skipped.push(ContractMigrationSkip {
                path: display_path,
                reason: "file is not UTF-8".to_string(),
            });
            continue;
        };
        scanned_files += 1;

        let mut file_replacements = Vec::new();
        for replacement in &replacements {
            let (migrated, occurrences) =
                replace_contract_id(&contents, &replacement.legacy_id, &replacement.canonical_id);
            if occurrences == 0 {
                continue;
            }
            contents = migrated;
            replacement_count += occurrences;
            file_replacements.push(ContractMigrationReplacement {
                legacy_id: replacement.legacy_id.clone(),
                canonical_id: replacement.canonical_id.clone(),
                maturity: replacement.maturity,
                support_until: replacement.support_until.clone(),
                occurrences,
            });
        }

        if file_replacements.is_empty() {
            continue;
        }
        edits.push(ContractMigrationEdit {
            path: display_path,
            replacements: file_replacements,
        });
        writes.push(PlannedWrite {
            path: file,
            original,
            migrated: contents.into_bytes(),
            permissions: metadata.permissions(),
        });
    }

    if write {
        apply_writes_with_rollback(&writes)?;
    }

    Ok(ContractMigrationReport {
        contract_registry_version: plurora_runtime::CONTRACT_REGISTRY_VERSION.to_string(),
        root: normalized_path(&root),
        mode: if write { "write" } else { "preview" }.to_string(),
        scope: if all_aliases {
            "all_registered_aliases"
        } else {
            "lifecycle_tracked_aliases"
        }
        .to_string(),
        scanned_files,
        skipped_files: skipped.len(),
        changed_files: edits.len(),
        replacement_count,
        edits,
        skipped,
        excluded_paths,
    })
}

fn replace_contract_id(input: &str, legacy_id: &str, canonical_id: &str) -> (String, usize) {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    let mut replacements = 0;

    for (start, _) in input.match_indices(legacy_id) {
        let end = start + legacy_id.len();
        let has_left_boundary = input[..start]
            .chars()
            .next_back()
            .map_or(true, |value| !is_contract_id_character(value));
        let has_right_boundary = input[end..]
            .chars()
            .next()
            .map_or(true, |value| !is_contract_id_character(value));
        if !has_left_boundary || !has_right_boundary {
            continue;
        }
        output.push_str(&input[cursor..start]);
        output.push_str(canonical_id);
        cursor = end;
        replacements += 1;
    }

    if replacements == 0 {
        return (input.to_string(), 0);
    }
    output.push_str(&input[cursor..]);
    (output, replacements)
}

fn is_contract_id_character(value: char) -> bool {
    value.is_ascii_alphanumeric() || matches!(value, '.' | '_' | '-' | '/')
}

fn is_supported_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|extension| SUPPORTED_SOURCE_EXTENSIONS.contains(&extension.as_str()))
}

fn apply_writes_with_rollback(writes: &[PlannedWrite]) -> Result<()> {
    let mut applied = Vec::new();
    for (index, write) in writes.iter().enumerate() {
        if let Err(error) = atomic_replace(&write.path, &write.migrated, &write.permissions) {
            let mut rollback_failures = Vec::new();
            for applied_index in applied.into_iter().rev() {
                let previous: &PlannedWrite = &writes[applied_index];
                if let Err(rollback_error) =
                    atomic_replace(&previous.path, &previous.original, &previous.permissions)
                {
                    rollback_failures
                        .push(format!("{}: {rollback_error:#}", previous.path.display()));
                }
            }
            if rollback_failures.is_empty() {
                return Err(error).with_context(|| {
                    format!(
                        "migration write failed at {}; earlier writes were rolled back",
                        write.path.display()
                    )
                });
            }
            anyhow::bail!(
                "migration write failed at {}: {error:#}; rollback also failed for {}",
                write.path.display(),
                rollback_failures.join(", ")
            );
        }
        applied.push(index);
    }
    Ok(())
}

fn atomic_replace(path: &Path, contents: &[u8], permissions: &Permissions) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("migration target has no parent: {}", path.display()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("creating migration staging file for {}", path.display()))?;
    temporary
        .write_all(contents)
        .with_context(|| format!("staging migration for {}", path.display()))?;
    temporary
        .as_file_mut()
        .set_permissions(permissions.clone())
        .with_context(|| format!("preserving permissions for {}", path.display()))?;
    temporary
        .as_file_mut()
        .sync_all()
        .with_context(|| format!("syncing migration staging file for {}", path.display()))?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("atomically replacing {}", path.display()))?;
    Ok(())
}

fn migration_display_path(root: &Path, file: &Path) -> String {
    if root.is_dir() {
        normalized_path(file.strip_prefix(root).unwrap_or(file))
    } else {
        normalized_path(file)
    }
}

fn normalized_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized
        .strip_prefix("//?/")
        .unwrap_or(&normalized)
        .to_string()
}

fn collect_files(
    root: &Path,
    path: &Path,
    files: &mut Vec<PathBuf>,
    excluded_paths: &mut Vec<ContractMigrationSkip>,
) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("reading metadata for {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        excluded_paths.push(ContractMigrationSkip {
            path: migration_display_path(root, path),
            reason: "symbolic link".to_string(),
        });
        return Ok(());
    }
    if metadata.is_file() {
        files.push(path.to_path_buf());
        return Ok(());
    }
    anyhow::ensure!(
        metadata.is_dir(),
        "migration path is not a file or directory"
    );

    let mut entries = fs::read_dir(path)
        .with_context(|| format!("reading directory {}", path.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let child = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir()
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| SKIPPED_DIRECTORIES.contains(&name))
        {
            excluded_paths.push(ContractMigrationSkip {
                path: migration_display_path(root, &child),
                reason: "excluded directory".to_string(),
            });
            continue;
        }
        collect_files(root, &child, files, excluded_paths)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_is_a_preview_until_write_is_requested() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("src/client.ts");
        fs::create_dir_all(source.parent().unwrap())?;
        fs::write(
            &source,
            "call('kernel.v1.host.info'); call('kernel.v1.target.list');\n",
        )?;

        let preview = migrate_path(temp.path(), false, false)?;
        assert_eq!(preview.mode, "preview");
        assert_eq!(preview.scope, "lifecycle_tracked_aliases");
        assert_eq!(preview.changed_files, 1);
        assert_eq!(preview.replacement_count, 2);
        assert!(fs::read_to_string(&source)?.contains("kernel.v1.host.info"));

        let applied = migrate_path(temp.path(), true, false)?;
        assert_eq!(applied.mode, "write");
        assert_eq!(applied.replacement_count, 2);
        assert_eq!(
            fs::read_to_string(&source)?,
            "call('host.info'); call('host.target.list');\n"
        );
        Ok(())
    }

    #[test]
    fn all_aliases_requires_an_explicit_scope() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("client.ts");
        fs::write(&source, "call('kernel.v1.project.list');\n")?;

        assert_eq!(
            migrate_path(temp.path(), false, false)?.replacement_count,
            0
        );
        let report = migrate_path(temp.path(), true, true)?;
        assert_eq!(report.scope, "all_registered_aliases");
        assert_eq!(report.replacement_count, 1);
        assert_eq!(fs::read_to_string(source)?, "call('host.project.list');\n");
        Ok(())
    }

    #[test]
    fn migration_requires_contract_id_boundaries() {
        let (migrated, replacements) = replace_contract_id(
            "kernel.v1.target.list.extra kernel.v1.target.list xkernel.v1.target.list",
            "kernel.v1.target.list",
            "host.target.list",
        );
        assert_eq!(replacements, 1);
        assert_eq!(
            migrated,
            "kernel.v1.target.list.extra host.target.list xkernel.v1.target.list"
        );
    }

    #[test]
    fn migration_reports_unsupported_files_and_skips_generated_directories() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("src/main.rs");
        let unsupported = temp.path().join("config.json");
        let generated = temp.path().join("target/generated.rs");
        fs::create_dir_all(source.parent().unwrap())?;
        fs::create_dir_all(generated.parent().unwrap())?;
        fs::write(&source, "kernel.v1.target.list")?;
        fs::write(&unsupported, "\"kernel.v1.target.list\"")?;
        fs::write(&generated, "kernel.v1.target.list")?;

        let report = migrate_path(temp.path(), true, false)?;
        assert_eq!(report.changed_files, 1);
        assert_eq!(report.skipped_files, 1);
        assert_eq!(report.skipped[0].path, "config.json");
        assert_eq!(report.skipped[0].reason, "unsupported file type");
        assert_eq!(report.excluded_paths.len(), 1);
        assert_eq!(report.excluded_paths[0].path, "target");
        assert_eq!(report.excluded_paths[0].reason, "excluded directory");
        assert_eq!(fs::read_to_string(&source)?, "host.target.list");
        assert_eq!(
            fs::read_to_string(&unsupported)?,
            "\"kernel.v1.target.list\""
        );
        assert_eq!(fs::read_to_string(&generated)?, "kernel.v1.target.list");
        Ok(())
    }
}
