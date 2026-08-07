//! Handler for `official/integrity-lab` capabilities.
//!
//! Provides deterministic SHA-256 hashing plus GPG detached signature
//! verification for package installation.  Sequoia is LGPL-2.0-or-later, which
//! is compatible with AGPL-3.0 deployments; `crypto-rust` keeps the backend
//! pure Rust and avoids system OpenSSL/nettle/botan dependencies.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use sequoia_openpgp as openpgp;
use serde_json::Value;
use sha2::{Digest, Sha256};

use openpgp::parse::{stream::*, Parse};
use openpgp::policy::StandardPolicy;
use openpgp::Cert;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/integrity-lab";

pub const TREE_HASH_SCHEMA_VERSION: u32 = 2;

const EXCLUDED_NAMES: &[&str] = &[
    ".git",
    ".gitignore",
    ".DS_Store",
    "node_modules",
    "target",
    "__pycache__",
];
const EXTERNAL_WORKSPACE_PROFILE: &str = "external_workspace_v1";
const EXTERNAL_WORKSPACE_EXCLUDED_NAMES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    ".DS_Store",
    "node_modules",
    "target",
    ".venv",
    "venv",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
];
const EXTERNAL_WORKSPACE_MAX_FILES: u64 = 25_000;
const EXTERNAL_WORKSPACE_MAX_DIRECTORIES: u64 = 25_000;
const EXTERNAL_WORKSPACE_MAX_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceTreeHash {
    pub sha256: String,
    pub files_hashed: u64,
    pub total_bytes: u64,
}

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }

    match request.capability_id.as_str() {
        "integrity.compute_tree_hash" | "official/integrity-lab/compute_tree_hash" => {
            Some(compute_tree_hash(request))
        }
        "integrity.compute_manifest_hash" | "official/integrity-lab/compute_manifest_hash" => {
            Some(compute_manifest_hash(request))
        }
        "integrity.verify_gpg_signature" | "official/integrity-lab/verify_gpg_signature" => {
            Some(verify_gpg_signature(request))
        }
        "integrity.fingerprint_public_key" | "official/integrity-lab/fingerprint_public_key" => {
            Some(fingerprint_public_key(request))
        }
        _ => None,
    }
}

fn input_str<'a>(input: &'a Value, key: &str) -> Result<&'a str> {
    input
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing or invalid string field '{key}'"))
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{}", to_hex(&digest))
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn compute_tree_hash(request: &InprocInvocation) -> Result<Value> {
    let dir = PathBuf::from(input_str(&request.input, "dir")?);
    let external_workspace = request
        .input
        .get("profile")
        .and_then(Value::as_str)
        .is_some_and(|profile| profile == EXTERNAL_WORKSPACE_PROFILE);
    let summary = if external_workspace {
        compute_external_workspace_tree_hash(&dir)?
    } else {
        compute_workspace_tree_hash(&dir, EXCLUDED_NAMES, false)?
    };

    Ok(serde_json::json!({
        "sha256": summary.sha256,
        "files_hashed": summary.files_hashed,
        "total_bytes": summary.total_bytes,
    }))
}

pub fn compute_external_workspace_tree_hash(dir: &Path) -> Result<WorkspaceTreeHash> {
    compute_workspace_tree_hash(dir, EXTERNAL_WORKSPACE_EXCLUDED_NAMES, true)
}

fn compute_workspace_tree_hash(
    dir: &Path,
    excluded_names: &[&str],
    external_workspace: bool,
) -> Result<WorkspaceTreeHash> {
    anyhow::ensure!(dir.is_absolute(), "dir must be an absolute path");
    let metadata = fs::symlink_metadata(dir)
        .with_context(|| format!("failed to inspect tree root {}", dir.display()))?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "dir must be a real directory, not a symlink"
    );
    let root = fs::canonicalize(dir)
        .with_context(|| format!("failed to canonicalize tree root {}", dir.display()))?;

    let mut entries = Vec::new();
    let mut external_stats = external_workspace.then(ExternalTreeStats::default);
    collect_tree_entries(
        &root,
        &root,
        &mut entries,
        excluded_names,
        external_workspace,
        &mut external_stats,
    )?;
    entries.sort_by(|a, b| a.relative.cmp(&b.relative));

    let mut hasher = Sha256::new();
    let mut files_hashed = 0u64;
    let mut total_bytes = 0u64;

    for entry in entries {
        hasher.update(entry.relative.as_bytes());
        hasher.update(b"\0");
        match entry.kind {
            TreeEntryKind::File {
                path,
                size,
                identity,
                handle,
            } => {
                hasher.update(b"file\0");
                hasher.update(size.to_string().as_bytes());
                hasher.update(b"\0");
                let mut file = fs::File::open(&path)
                    .with_context(|| format!("failed to open file {}", path.display()))?;
                let opened_handle = same_file::Handle::from_file(file.try_clone()?)?;
                let opened = file.metadata()?;
                anyhow::ensure!(
                    opened.is_file()
                        && file_identity(&opened) == identity
                        && opened_handle == handle,
                    "workspace file changed while tree hashing: {}",
                    path.display()
                );
                let mut buffer = [0u8; 16 * 1024];
                let mut actual_size = 0u64;
                loop {
                    let read = file.read(&mut buffer)?;
                    if read == 0 {
                        break;
                    }
                    actual_size = actual_size.saturating_add(read as u64);
                    if external_workspace {
                        anyhow::ensure!(
                            total_bytes.saturating_add(actual_size) <= EXTERNAL_WORKSPACE_MAX_BYTES,
                            "external workspace byte limit exceeded while hashing"
                        );
                    }
                    hasher.update(&buffer[..read]);
                }
                let after = fs::symlink_metadata(&path)?;
                anyhow::ensure!(
                    !after.file_type().is_symlink()
                        && file_identity(&after) == identity
                        && same_file::Handle::from_path(&path)? == handle
                        && actual_size == size,
                    "workspace file changed during tree hashing: {}",
                    path.display()
                );
                hasher.update(b"\0");
                files_hashed += 1;
                total_bytes = total_bytes.saturating_add(actual_size);
            }
            TreeEntryKind::Symlink { target } => {
                hasher.update(b"symlink\0");
                let target = target.to_string_lossy();
                hasher.update(target.as_bytes());
                hasher.update(b"\0");
            }
        }
    }

    let digest = hasher.finalize();
    Ok(WorkspaceTreeHash {
        sha256: format!("sha256:{}", to_hex(&digest)),
        files_hashed,
        total_bytes,
    })
}

struct TreeEntry {
    relative: String,
    kind: TreeEntryKind,
}

enum TreeEntryKind {
    File {
        path: PathBuf,
        size: u64,
        identity: FileIdentity,
        handle: same_file::Handle,
    },
    Symlink {
        target: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileIdentity {
    len: u64,
    modified: Option<std::time::SystemTime>,
    created: Option<std::time::SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

fn file_identity(metadata: &fs::Metadata) -> FileIdentity {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;

    FileIdentity {
        len: metadata.len(),
        modified: metadata.modified().ok(),
        created: metadata.created().ok(),
        #[cfg(unix)]
        device: metadata.dev(),
        #[cfg(unix)]
        inode: metadata.ino(),
    }
}

#[derive(Default)]
struct ExternalTreeStats {
    files: u64,
    directories: u64,
    bytes: u64,
}

fn collect_tree_entries(
    root: &Path,
    dir: &Path,
    out: &mut Vec<TreeEntry>,
    excluded_names: &[&str],
    require_contained_symlinks: bool,
    external_stats: &mut Option<ExternalTreeStats>,
) -> Result<()> {
    let directory_handle = validated_tree_directory_handle(root, dir)?;
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if let Some(name) = name.to_str() {
            if excluded_names.contains(&name) {
                continue;
            }
        }

        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path)?;
            if require_contained_symlinks {
                anyhow::ensure!(
                    !target.is_absolute(),
                    "external workspace contains an absolute symlink: {}",
                    path.display()
                );
                let resolved = fs::canonicalize(path.parent().unwrap_or(root).join(&target))
                    .with_context(|| {
                        format!(
                            "external workspace contains a dangling symlink: {}",
                            path.display()
                        )
                    })?;
                anyhow::ensure!(
                    resolved.starts_with(root),
                    "external workspace symlink escapes its root: {}",
                    path.display()
                );
            }
            if let Some(stats) = external_stats.as_mut() {
                add_external_tree_file(stats, target.as_os_str().len() as u64)?;
            }
            out.push(TreeEntry {
                relative: relative_path(root, &path)?,
                kind: TreeEntryKind::Symlink { target },
            });
        } else if metadata.is_dir() {
            if let Some(stats) = external_stats.as_mut() {
                add_external_tree_directory(stats)?;
            }
            collect_tree_entries(
                root,
                &path,
                out,
                excluded_names,
                require_contained_symlinks,
                external_stats,
            )?;
        } else if metadata.is_file() {
            let file = fs::File::open(&path)?;
            let handle = same_file::Handle::from_file(file.try_clone()?)?;
            let opened = file.metadata()?;
            let current = fs::symlink_metadata(&path)?;
            anyhow::ensure!(
                opened.is_file()
                    && current.is_file()
                    && !current.file_type().is_symlink()
                    && file_identity(&opened) == file_identity(&metadata)
                    && same_file::Handle::from_path(&path)? == handle,
                "workspace file changed while tree entries were collected: {}",
                path.display()
            );
            if let Some(stats) = external_stats.as_mut() {
                add_external_tree_file(stats, opened.len())?;
            }
            out.push(TreeEntry {
                relative: relative_path(root, &path)?,
                kind: TreeEntryKind::File {
                    path,
                    size: opened.len(),
                    identity: file_identity(&opened),
                    handle,
                },
            });
        }
    }
    anyhow::ensure!(
        validated_tree_directory_handle(root, dir)? == directory_handle,
        "workspace directory changed during tree traversal: {}",
        dir.display()
    );
    Ok(())
}

fn validated_tree_directory_handle(root: &Path, dir: &Path) -> Result<same_file::Handle> {
    let metadata = fs::symlink_metadata(dir)?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "workspace tree contains a directory symlink: {}",
        dir.display()
    );
    let canonical = fs::canonicalize(dir)?;
    anyhow::ensure!(
        canonical.starts_with(root),
        "workspace directory escaped its root: {}",
        dir.display()
    );
    Ok(same_file::Handle::from_path(dir)?)
}

fn add_external_tree_directory(stats: &mut ExternalTreeStats) -> Result<()> {
    stats.directories = stats.directories.saturating_add(1);
    anyhow::ensure!(
        stats.directories <= EXTERNAL_WORKSPACE_MAX_DIRECTORIES,
        "external workspace directory count limit exceeded"
    );
    Ok(())
}

fn add_external_tree_file(stats: &mut ExternalTreeStats, bytes: u64) -> Result<()> {
    stats.files = stats.files.saturating_add(1);
    stats.bytes = stats.bytes.saturating_add(bytes);
    anyhow::ensure!(
        stats.files <= EXTERNAL_WORKSPACE_MAX_FILES,
        "external workspace file count limit exceeded"
    );
    anyhow::ensure!(
        stats.bytes <= EXTERNAL_WORKSPACE_MAX_BYTES,
        "external workspace byte limit exceeded"
    );
    Ok(())
}

fn relative_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root)?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn compute_manifest_hash(request: &InprocInvocation) -> Result<Value> {
    let manifest_path = PathBuf::from(input_str(&request.input, "manifest_path")?);
    let raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read manifest {}", manifest_path.display()))?;
    let value: Value = match manifest_path.extension().and_then(|ext| ext.to_str()) {
        Some("json") => serde_json::from_str(&raw).or_else(|_| serde_yaml::from_str(&raw))?,
        _ => serde_yaml::from_str(&raw)?,
    };
    let canonical = canonicalize_json(value);
    let bytes = serde_json::to_vec(&canonical)?;
    Ok(serde_json::json!({ "sha256": sha256_prefixed(&bytes) }))
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<String, Value> = map
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json(value)))
                .collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        other => other,
    }
}

fn verify_gpg_signature(request: &InprocInvocation) -> Result<Value> {
    let public_keys = request
        .input
        .get("public_keys")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if public_keys.is_empty() {
        return Ok(verify_error("no public keys provided"));
    }

    let data = match input_str(&request.input, "data").and_then(|data| {
        BASE64
            .decode(data)
            .map_err(|error| anyhow::anyhow!("invalid base64 data: {error}"))
    }) {
        Ok(data) => data,
        Err(error) => return Ok(verify_error(error.to_string())),
    };
    let signature = match input_str(&request.input, "signature") {
        Ok(signature) => signature,
        Err(error) => return Ok(verify_error(format!("invalid signature format: {error}"))),
    };

    let mut certs = Vec::new();
    for (idx, key) in public_keys.iter().enumerate() {
        let Some(key) = key.as_str() else {
            return Ok(verify_error(format!(
                "invalid public key at index {idx}: expected armored string"
            )));
        };
        match Cert::from_bytes(key.as_bytes()) {
            Ok(cert) => certs.push(cert),
            Err(error) => {
                return Ok(verify_error(format!(
                    "invalid public key at index {idx}: {error}"
                )));
            }
        }
    }

    let helper = IntegrityVerificationHelper::new(certs);
    let policy = StandardPolicy::new();
    let mut verifier = match DetachedVerifierBuilder::from_bytes(signature.as_bytes())
        .and_then(|builder| builder.with_policy(&policy, None, helper))
    {
        Ok(verifier) => verifier,
        Err(error) => {
            return Ok(verify_error(format!("invalid signature format: {error}")));
        }
    };

    match verifier.verify_bytes(&data) {
        Ok(()) => {
            let helper = verifier.into_helper();
            Ok(serde_json::json!({
                "verified": true,
                "key_fingerprint": helper.key_fingerprint,
                "signing_time": helper.signing_time,
                "error": Value::Null,
            }))
        }
        Err(error) => Ok(verify_error(error.to_string())),
    }
}

fn verify_error(message: impl Into<String>) -> Value {
    serde_json::json!({
        "verified": false,
        "key_fingerprint": Value::Null,
        "signing_time": Value::Null,
        "error": message.into(),
    })
}

struct IntegrityVerificationHelper {
    certs: Vec<Cert>,
    key_fingerprint: Option<String>,
    signing_time: Option<String>,
}

impl IntegrityVerificationHelper {
    fn new(certs: Vec<Cert>) -> Self {
        Self {
            certs,
            key_fingerprint: None,
            signing_time: None,
        }
    }
}

impl VerificationHelper for IntegrityVerificationHelper {
    fn get_certs(&mut self, ids: &[openpgp::KeyHandle]) -> openpgp::Result<Vec<Cert>> {
        Ok(self
            .certs
            .iter()
            .filter(|cert| {
                ids.is_empty()
                    || cert
                        .keys()
                        .any(|key| ids.iter().any(|id| key.key().key_handle().aliases(id)))
            })
            .cloned()
            .collect())
    }

    fn check(&mut self, structure: MessageStructure) -> openpgp::Result<()> {
        for layer in structure.into_iter() {
            if let MessageLayer::SignatureGroup { results } = layer {
                for result in results {
                    if let Ok(good) = result {
                        self.key_fingerprint = Some(good.ka.key().fingerprint().to_string());
                        self.signing_time = good
                            .sig
                            .signature_creation_time()
                            .map(system_time_to_rfc3339);
                        return Ok(());
                    }
                }
            }
        }
        Err(anyhow::anyhow!("no valid signature"))
    }
}

fn system_time_to_rfc3339(time: SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Utc> = time
        .duration_since(UNIX_EPOCH)
        .map(|duration| chrono::DateTime::<chrono::Utc>::from(UNIX_EPOCH + duration))
        .unwrap_or_else(|_| chrono::DateTime::<chrono::Utc>::from(UNIX_EPOCH));
    datetime.to_rfc3339()
}

fn fingerprint_public_key(request: &InprocInvocation) -> Result<Value> {
    let public_key = input_str(&request.input, "public_key")?;
    let cert = Cert::from_bytes(public_key.as_bytes())?;
    let user_ids: Vec<String> = cert
        .userids()
        .map(|userid| String::from_utf8_lossy(userid.userid().value()).to_string())
        .collect();
    Ok(serde_json::json!({
        "fingerprint": cert.fingerprint().to_string(),
        "user_ids": user_ids,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use openpgp::armor;
    use openpgp::cert::prelude::*;
    use openpgp::serialize::stream::{Armorer, Message, Signer};
    use openpgp::serialize::SerializeInto;

    fn request(capability_id: &str, input: Value) -> InprocInvocation {
        InprocInvocation {
            capability_id: capability_id.to_string(),
            provider_package_id: PACKAGE_ID.to_string(),
            session_id: None,
            input,
        }
    }

    #[test]
    fn manifest_hash_yaml_json_equivalent_unit() -> Result<()> {
        let yaml: Value = serde_yaml::from_str("b: 2\na: 1\n")?;
        let json: Value = serde_json::from_str(r#"{"a":1,"b":2}"#)?;
        assert_eq!(canonicalize_json(yaml), canonicalize_json(json));
        Ok(())
    }

    #[test]
    fn tree_hash_changes_when_only_dist_content_changes() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let package_dir = tmp.path().join("package");
        let dist_dir = package_dir.join("dist");
        fs::create_dir_all(&dist_dir)?;
        fs::write(package_dir.join("manifest.yaml"), "id: fixture/dist\n")?;
        fs::write(dist_dir.join("bundle.mjs"), "export const version = 1;\n")?;

        let first = compute_tree_hash(&request(
            "integrity.compute_tree_hash",
            serde_json::json!({ "dir": package_dir }),
        ))?;

        fs::write(dist_dir.join("bundle.mjs"), "export const version = 2;\n")?;
        let second = compute_tree_hash(&request(
            "integrity.compute_tree_hash",
            serde_json::json!({ "dir": package_dir }),
        ))?;

        assert_ne!(first["sha256"], second["sha256"]);
        assert_eq!(first["files_hashed"], serde_json::json!(2));
        assert_eq!(second["files_hashed"], serde_json::json!(2));
        Ok(())
    }

    #[test]
    fn external_workspace_hash_includes_gitignore_and_excludes_dependency_caches() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let workspace = tmp.path().join("workspace");
        fs::create_dir_all(workspace.join("node_modules"))?;
        fs::write(workspace.join("app.ts"), "export const version = 1;\n")?;
        fs::write(workspace.join(".gitignore"), "dist/\n")?;
        fs::write(workspace.join("node_modules/dependency.js"), "one\n")?;
        let hash = || {
            compute_tree_hash(&request(
                "integrity.compute_tree_hash",
                serde_json::json!({
                    "dir": workspace,
                    "profile": EXTERNAL_WORKSPACE_PROFILE,
                }),
            ))
        };

        let first = hash()?;
        fs::write(workspace.join("node_modules/dependency.js"), "two\n")?;
        let cache_changed = hash()?;
        assert_eq!(first["sha256"], cache_changed["sha256"]);
        fs::write(workspace.join(".gitignore"), "dist/\n.env\n")?;
        let gitignore_changed = hash()?;
        assert_ne!(first["sha256"], gitignore_changed["sha256"]);
        assert_eq!(gitignore_changed["files_hashed"], serde_json::json!(2));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn external_workspace_hash_rejects_symlinks_that_escape_root() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let workspace = tmp.path().join("workspace");
        fs::create_dir(&workspace)?;
        fs::write(tmp.path().join("outside.txt"), "outside\n")?;
        std::os::unix::fs::symlink("../outside.txt", workspace.join("escape"))?;
        let result = compute_tree_hash(&request(
            "integrity.compute_tree_hash",
            serde_json::json!({
                "dir": workspace,
                "profile": EXTERNAL_WORKSPACE_PROFILE,
            }),
        ));
        assert!(result.unwrap_err().to_string().contains("escapes its root"));
        Ok(())
    }

    #[test]
    fn gpg_signature_roundtrip_unit() -> Result<()> {
        let data = b"integrity-lab runtime unit data";
        let (cert, _) =
            CertBuilder::general_purpose(None, Some("Runtime Test <runtime@example.test>"))
                .generate()?;
        let policy = StandardPolicy::new();
        let signing_keypair = cert
            .keys()
            .secret()
            .with_policy(&policy, None)
            .supported()
            .alive()
            .revoked(false)
            .for_signing()
            .next()
            .context("missing signing key")?
            .key()
            .clone()
            .into_keypair()?;
        let signing_fingerprint = signing_keypair.public().fingerprint().to_string();
        let mut signature = Vec::new();
        {
            let message = Message::new(&mut signature);
            let message = Armorer::new(message).kind(armor::Kind::Signature).build()?;
            let mut signer = Signer::new(message, signing_keypair).detached().build()?;
            signer.write_all(data)?;
            signer.finalize()?;
        }

        let public_key = String::from_utf8(cert.armored().to_vec()?)?;
        let output = verify_gpg_signature(&request(
            "integrity.verify_gpg_signature",
            serde_json::json!({
                "data": BASE64.encode(data),
                "signature": String::from_utf8(signature)?,
                "public_keys": [public_key],
            }),
        ))?;
        assert_eq!(output["verified"], true);
        assert_eq!(output["key_fingerprint"], signing_fingerprint);
        Ok(())
    }
}
