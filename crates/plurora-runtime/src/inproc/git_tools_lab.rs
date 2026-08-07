//! Handler for `official/git-tools-lab` capabilities.
//!
//! Pure-Rust git operations used by package installation. All gix operations
//! are blocking and are executed through `tokio::task::spawn_blocking`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use gix::bstr::ByteSlice;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/git-tools-lab";
const DEFAULT_MAX_FILES: u64 = 25_000;
const DEFAULT_MAX_DIRECTORIES: u64 = 25_000;
const DEFAULT_MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
const DEFAULT_MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;
const HARD_MAX_FILES: u64 = 100_000;
const HARD_MAX_DIRECTORIES: u64 = 100_000;
const HARD_MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
const HARD_MAX_DOWNLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
// Public gix-pack `Bundle::write_to_directory` progress id for bytes read from the pack stream.
const GIX_READ_PACK_PROGRESS_ID: gix::progress::Id = *b"BWRB";

#[derive(Debug, Deserialize)]
struct ResolveRefInput {
    remote_url: String,
    #[serde(rename = "ref")]
    ref_name: String,
}

#[derive(Debug, Deserialize)]
struct FetchRefsInput {
    remote_url: String,
}

#[derive(Debug, Deserialize)]
struct FetchTreeInput {
    remote_url: String,
    commit_sha: String,
    #[serde(default, rename = "ref_name")]
    _ref_name: Option<String>,
    dest_dir: String,
    #[serde(default)]
    max_files: Option<u64>,
    #[serde(default)]
    max_directories: Option<u64>,
    #[serde(default)]
    max_total_bytes: Option<u64>,
    #[serde(default)]
    max_download_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ReadSignedTagInput {
    remote_url: String,
    tag: String,
    #[serde(default)]
    max_download_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
struct RemoteRef {
    name: String,
    sha: String,
    kind: RefKind,
    tag_object: Option<String>,
    symbolic_target: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefKind {
    Branch,
    Tag,
}

impl RefKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Branch => "branch",
            Self::Tag => "tag",
        }
    }
}

#[derive(Debug, Default)]
struct TreeWriteStats {
    files_written: u64,
    directories_written: u64,
    total_bytes: u64,
}

#[derive(Debug, Default)]
struct TreeWriteLimits {
    max_files: Option<u64>,
    max_directories: Option<u64>,
    max_total_bytes: Option<u64>,
}

struct FetchBudgetState {
    max_bytes: usize,
    downloaded_bytes: gix::progress::StepShared,
    interrupt: AtomicBool,
    exceeded: AtomicBool,
}

#[derive(Clone)]
struct FetchBudgetProgress {
    state: Arc<FetchBudgetState>,
    counts_download: bool,
    id: gix::progress::Id,
}

impl FetchBudgetProgress {
    fn root(state: Arc<FetchBudgetState>) -> Self {
        Self {
            state,
            counts_download: false,
            id: gix::progress::UNKNOWN,
        }
    }

    fn record(&self, bytes: usize) {
        if !self.counts_download {
            return;
        }
        let previous = self
            .state
            .downloaded_bytes
            .fetch_add(bytes, Ordering::Relaxed);
        if previous > self.state.max_bytes.saturating_sub(bytes) {
            self.state.exceeded.store(true, Ordering::Relaxed);
            self.state.interrupt.store(true, Ordering::Relaxed);
        }
    }
}

impl gix::Count for FetchBudgetProgress {
    fn set(&self, step: usize) {
        if self.counts_download {
            self.state.downloaded_bytes.store(step, Ordering::Relaxed);
            if step > self.state.max_bytes {
                self.state.exceeded.store(true, Ordering::Relaxed);
                self.state.interrupt.store(true, Ordering::Relaxed);
            }
        }
    }

    fn step(&self) -> usize {
        self.counts_download
            .then(|| self.state.downloaded_bytes.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    fn inc_by(&self, step: usize) {
        self.record(step);
    }

    fn counter(&self) -> gix::progress::StepShared {
        if self.counts_download {
            self.state.downloaded_bytes.clone()
        } else {
            Arc::new(AtomicUsize::new(0))
        }
    }
}

impl gix::Progress for FetchBudgetProgress {
    fn init(&mut self, _max: Option<usize>, _unit: Option<gix::progress::Unit>) {}

    fn set_name(&mut self, _name: String) {}

    fn name(&self) -> Option<String> {
        None
    }

    fn id(&self) -> gix::progress::Id {
        self.id
    }

    fn message(&self, _level: gix::progress::MessageLevel, _message: String) {}
}

impl gix::NestedProgress for FetchBudgetProgress {
    type SubProgress = Self;

    fn add_child(&mut self, _name: impl Into<String>) -> Self::SubProgress {
        Self {
            state: self.state.clone(),
            counts_download: self.counts_download,
            id: gix::progress::UNKNOWN,
        }
    }

    fn add_child_with_id(
        &mut self,
        _name: impl Into<String>,
        id: gix::progress::Id,
    ) -> Self::SubProgress {
        Self {
            state: self.state.clone(),
            counts_download: self.counts_download || id == GIX_READ_PACK_PROGRESS_ID,
            id,
        }
    }
}

pub fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }
    let id = request.capability_id.as_str();
    if id.ends_with("/resolve_ref") || id == "git.resolve_ref" {
        Some(async_blocking(resolve_ref, request.input.clone()))
    } else if id.ends_with("/fetch_refs") || id == "git.fetch_refs" {
        Some(async_blocking(fetch_refs, request.input.clone()))
    } else if id.ends_with("/fetch_tree") || id == "git.fetch_tree" {
        Some(async_blocking(fetch_tree, request.input.clone()))
    } else if id.ends_with("/read_signed_tag") || id == "git.read_signed_tag" {
        Some(async_blocking(read_signed_tag, request.input.clone()))
    } else {
        None
    }
}

fn async_blocking(f: fn(Value) -> Result<Value>, input: Value) -> Result<Value> {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(tokio::task::spawn_blocking(move || f(input)))
            .context("git task panicked or was cancelled")?
    })
}

fn resolve_ref(input: Value) -> Result<Value> {
    let input: ResolveRefInput = serde_json::from_value(input)?;
    validate_remote_url(&input.remote_url)?;

    if is_full_sha(&input.ref_name) {
        return Ok(serde_json::json!({
            "commit_sha": input.ref_name,
            "ref_kind": "commit",
            "ref_name": input.ref_name,
        }));
    }

    let refs = list_remote_refs_blocking(&input.remote_url)?;
    let wanted = input.ref_name.trim();
    let candidates = [
        wanted.to_string(),
        format!("refs/heads/{wanted}"),
        format!("refs/tags/{wanted}"),
    ];

    let resolved = resolve_remote_ref(&refs, wanted, &candidates)
        .with_context(|| format!("ref '{wanted}' not found on remote"))?;
    let resolved_name = resolved
        .symbolic_target
        .as_deref()
        .unwrap_or(&resolved.name);

    Ok(serde_json::json!({
        "commit_sha": resolved.sha,
        "ref_kind": resolved.kind.as_str(),
        "ref_name": resolved_name,
    }))
}

fn resolve_remote_ref<'a>(
    refs: &'a [RemoteRef],
    wanted: &str,
    candidates: &[String],
) -> Option<&'a RemoteRef> {
    if wanted.is_empty() || wanted == "HEAD" {
        return refs
            .iter()
            .find(|remote_ref| remote_ref.name == "HEAD")
            .or_else(|| {
                refs.iter()
                    .find(|remote_ref| remote_ref.name == "refs/heads/main")
            })
            .or_else(|| {
                refs.iter()
                    .find(|remote_ref| remote_ref.name == "refs/heads/master")
            });
    }

    refs.iter().find(|remote_ref| {
        candidates
            .iter()
            .any(|candidate| candidate == &remote_ref.name)
    })
}

fn fetch_refs(input: Value) -> Result<Value> {
    let input: FetchRefsInput = serde_json::from_value(input)?;
    validate_remote_url(&input.remote_url)?;
    let refs = list_remote_refs_blocking(&input.remote_url)?;
    let refs: Vec<Value> = refs
        .into_iter()
        .map(|remote_ref| {
            serde_json::json!({
                "name": remote_ref.name,
                "sha": remote_ref.sha,
                "kind": remote_ref.kind.as_str(),
            })
        })
        .collect();
    Ok(serde_json::json!({ "refs": refs }))
}

fn fetch_tree(input: Value) -> Result<Value> {
    let input: FetchTreeInput = serde_json::from_value(input)?;
    validate_remote_url(&input.remote_url)?;
    let requested_dest = PathBuf::from(&input.dest_dir);
    validate_dest_dir(&requested_dest)?;
    if !is_full_sha(&input.commit_sha) {
        anyhow::bail!("commit_sha must be a 40-character hex SHA");
    }
    let limits = TreeWriteLimits {
        max_files: Some(input.max_files.unwrap_or(DEFAULT_MAX_FILES)),
        max_directories: Some(input.max_directories.unwrap_or(DEFAULT_MAX_DIRECTORIES)),
        max_total_bytes: Some(input.max_total_bytes.unwrap_or(DEFAULT_MAX_TOTAL_BYTES)),
    };
    validate_tree_write_limits(&limits)?;
    let max_download_bytes = validate_download_budget(input.max_download_bytes)?;

    let parent = requested_dest
        .parent()
        .with_context(|| format!("dest_dir has no parent: {}", requested_dest.display()))?;
    let file_name = requested_dest
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| {
            format!(
                "dest_dir must end in a valid UTF-8 directory name: {}",
                requested_dest.display()
            )
        })?;
    let parent_metadata = fs::symlink_metadata(parent).with_context(|| {
        format!(
            "dest_dir parent must already exist as a real directory: {}",
            parent.display()
        )
    })?;
    anyhow::ensure!(
        parent_metadata.is_dir() && !parent_metadata.file_type().is_symlink(),
        "dest_dir parent must be a real directory, not a symlink: {}",
        parent.display()
    );
    let parent = fs::canonicalize(parent).with_context(|| {
        format!(
            "failed to canonicalize dest_dir parent {}",
            parent.display()
        )
    })?;
    let dest = parent.join(file_name);
    match fs::symlink_metadata(&dest) {
        Ok(_) => anyhow::bail!("dest_dir already exists: {}", dest.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let tmp = parent.join(format!("{file_name}.tmp.{}", Uuid::new_v4()));
    let repo_tmp = parent.join(format!("{file_name}.repo.tmp.{}", Uuid::new_v4()));

    let outcome = (|| -> Result<Value> {
        let (repo, downloaded_bytes) =
            fetch_bare_with_budget(&input.remote_url, &repo_tmp, max_download_bytes)?;
        let commit_id = gix::ObjectId::from_hex(input.commit_sha.as_bytes())?;
        let commit = repo.find_object(commit_id)?.peel_to_commit()?;
        let tree = commit.tree()?;
        let tree_hash = tree.id.to_string();

        fs::create_dir(&tmp)?;
        let mut stats = TreeWriteStats::default();
        write_tree_recursive(&tree, &tmp, &tmp, &mut stats, &limits)
            .with_context(|| format!("failed to write tree to {}", tmp.display()))?;
        fs::rename(&tmp, &dest).with_context(|| {
            format!(
                "failed to atomically rename {} to {}",
                tmp.display(),
                dest.display()
            )
        })?;
        Ok(serde_json::json!({
            "files_written": stats.files_written,
            "directories_written": stats.directories_written,
            "total_bytes": stats.total_bytes,
            "downloaded_bytes": downloaded_bytes,
            "tree_hash": tree_hash,
        }))
    })();

    if outcome.is_err() {
        fs::remove_dir_all(&tmp).ok();
    }
    fs::remove_dir_all(&repo_tmp).ok();
    outcome
}

fn read_signed_tag(input: Value) -> Result<Value> {
    let input: ReadSignedTagInput = serde_json::from_value(input)?;
    validate_remote_url(&input.remote_url)?;
    let max_download_bytes = validate_download_budget(input.max_download_bytes)?;
    let refs = list_remote_refs_blocking(&input.remote_url)?;
    let wanted = input.tag.trim();
    let candidates = [wanted.to_string(), format!("refs/tags/{wanted}")];
    let tag_ref = refs
        .iter()
        .find(|remote_ref| {
            remote_ref.kind == RefKind::Tag
                && candidates
                    .iter()
                    .any(|candidate| candidate == &remote_ref.name)
        })
        .with_context(|| format!("tag '{wanted}' not found on remote"))?;

    let tmp = std::env::temp_dir().join(format!("plurora-git-tag-{}", Uuid::new_v4()));
    let output = (|| -> Result<Value> {
        let (repo, _) = fetch_bare_with_budget(&input.remote_url, &tmp, max_download_bytes)?;
        if let Some(tag_object) = &tag_ref.tag_object {
            let id = gix::ObjectId::from_hex(tag_object.as_bytes())?;
            let tag = repo.find_object(id)?.try_into_tag()?;
            let decoded = tag.decode()?;
            let signed_data = signed_data_before_pgp(&tag.data);
            Ok(serde_json::json!({
                "tag_object": tag_object,
                "pgp_signature": decoded.pgp_signature.map(bstr_to_string),
                "signed_data": bytes_to_string(signed_data),
                "tagger": decoded.tagger()?.map(signature_to_json),
                "message": bstr_to_string(decoded.message),
            }))
        } else {
            let id = gix::ObjectId::from_hex(tag_ref.sha.as_bytes())?;
            let commit = repo.find_object(id)?.peel_to_commit()?;
            let decoded = commit.decode()?;
            Ok(serde_json::json!({
                "tag_object": Value::Null,
                "pgp_signature": Value::Null,
                "signed_data": bytes_to_string(&commit.data),
                "tagger": signature_to_json(decoded.committer()?),
                "message": bstr_to_string(decoded.message),
            }))
        }
    })();
    fs::remove_dir_all(&tmp).ok();
    output
}

fn list_remote_refs_blocking(remote_url: &str) -> Result<Vec<RemoteRef>> {
    use gix::protocol::{
        self,
        transport::{client::blocking_io::connect, Protocol, Service},
    };

    let url = gix::Url::try_from(remote_url)?;
    let mut transport = connect::connect(
        url,
        connect::Options {
            version: Protocol::V2,
            ssh: Default::default(),
            trace: false,
        },
    )?;
    let mut authenticate = |_action: protocol::credentials::helper::Action| {
        Ok::<_, protocol::credentials::protocol::Error>(None)
    };
    let mut progress = gix::progress::Discard;
    let mut handshake = protocol::handshake(
        &mut transport,
        Service::UploadPack,
        &mut authenticate,
        Vec::new(),
        &mut progress,
    )?;

    let refs = match handshake.refs.take() {
        Some(refs) => refs,
        None => {
            protocol::LsRefsCommand::new(None, &handshake.capabilities, ls_refs_agent_feature())
                .invoke_blocking(&mut transport, &mut progress, false)?
        }
    };

    Ok(refs.iter().filter_map(remote_ref_from_gix).collect())
}

fn ls_refs_agent_feature() -> (&'static str, Option<std::borrow::Cow<'static, str>>) {
    // Git protocol v2 validates ls-refs feature names against server-advertised
    // capabilities. The feature name must be the standard `agent`; the custom
    // identity belongs in the value. Passing `plurora` as the feature name
    // makes gix reject the command before it reaches GitHub with:
    // `ls-refs: capability plurora is not supported`.
    (
        "agent",
        Some(std::borrow::Cow::Owned(gix::protocol::agent(
            gix::env::agent(),
        ))),
    )
}

fn remote_ref_from_gix(remote_ref: &gix::protocol::handshake::Ref) -> Option<RemoteRef> {
    match remote_ref {
        gix::protocol::handshake::Ref::Peeled {
            full_ref_name,
            tag,
            object,
        } => classify_remote_ref(
            full_ref_name.as_bstr(),
            object.to_string(),
            Some(tag.to_string()),
        ),
        gix::protocol::handshake::Ref::Direct {
            full_ref_name,
            object,
        } => classify_remote_ref(full_ref_name.as_bstr(), object.to_string(), None),
        gix::protocol::handshake::Ref::Symbolic {
            full_ref_name,
            tag,
            object,
            target,
        } => {
            let name = bstr_to_string(full_ref_name.as_bstr());
            let symbolic_target = bstr_to_string(target.as_bstr());
            if name == "HEAD" && symbolic_target.starts_with("refs/heads/") {
                Some(RemoteRef {
                    name,
                    sha: object.to_string(),
                    kind: RefKind::Branch,
                    tag_object: tag.as_ref().map(ToString::to_string),
                    symbolic_target: Some(symbolic_target),
                })
            } else {
                classify_remote_ref(
                    full_ref_name.as_bstr(),
                    object.to_string(),
                    tag.as_ref().map(ToString::to_string),
                )
            }
        }
        gix::protocol::handshake::Ref::Unborn { .. } => None,
    }
}

fn classify_remote_ref(
    full_ref_name: &gix::bstr::BStr,
    sha: String,
    tag_object: Option<String>,
) -> Option<RemoteRef> {
    let name = bstr_to_string(full_ref_name);
    if name.starts_with("refs/heads/") {
        Some(RemoteRef {
            name,
            sha,
            kind: RefKind::Branch,
            tag_object: None,
            symbolic_target: None,
        })
    } else if name.starts_with("refs/tags/") {
        Some(RemoteRef {
            name,
            sha,
            kind: RefKind::Tag,
            tag_object,
            symbolic_target: None,
        })
    } else {
        None
    }
}

fn fetch_bare_with_budget(
    remote_url: &str,
    path: &Path,
    max_download_bytes: usize,
) -> Result<(gix::Repository, u64)> {
    // We only need the object database so `fetch_tree` can read `commit_sha` and
    // write its tree. Avoid `prepare_clone(...).with_ref_name(...)`: the gix
    // 0.83 clone helper has a name-only refspec panic path for HEAD/default
    // branch fetches. A bare full fetch is slower than a shallow single-branch
    // clone, but it is deterministic and avoids treating package installation as
    // a branch checkout operation.
    let mut prep = gix::prepare_clone_bare(remote_url, path)?;
    let budget = Arc::new(FetchBudgetState {
        max_bytes: max_download_bytes,
        downloaded_bytes: Arc::new(AtomicUsize::new(0)),
        interrupt: AtomicBool::new(false),
        exceeded: AtomicBool::new(false),
    });
    let fetched = prep.fetch_only(FetchBudgetProgress::root(budget.clone()), &budget.interrupt);
    if budget.exceeded.load(Ordering::Relaxed) {
        anyhow::bail!("git transport download budget exceeded (max {max_download_bytes} bytes)");
    }
    let (repo, _) = fetched?;
    Ok((
        repo,
        u64::try_from(budget.downloaded_bytes.load(Ordering::Relaxed))?,
    ))
}

fn write_tree_recursive(
    tree: &gix::Tree<'_>,
    root: &Path,
    dest: &Path,
    stats: &mut TreeWriteStats,
    limits: &TreeWriteLimits,
) -> Result<()> {
    for entry in tree.iter() {
        let entry = entry?;
        let name = bstr_to_string(entry.filename());
        if name == ".git" || name.contains('/') || name.contains('\\') || name == ".." {
            anyhow::bail!("unsafe tree entry name: {name}");
        }
        let out = dest.join(&name);
        if entry.mode().is_tree() {
            reserve_tree_directory(stats, limits)?;
            fs::create_dir(&out)?;
            let child = entry.object()?.try_into_tree()?;
            write_tree_recursive(&child, root, &out, stats, limits)?;
        } else if entry.mode().is_blob_or_symlink() {
            let blob = entry.object()?.try_into_blob()?;
            reserve_tree_blob(stats, limits, blob.data.len() as u64)?;
            if entry.mode().is_link() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::symlink;
                    let target = std::str::from_utf8(&blob.data)
                        .context("git symlink target must be valid UTF-8")?;
                    validate_tree_symlink_target(root, &out, Path::new(target))?;
                    symlink(target, &out)?;
                }
                #[cfg(not(unix))]
                {
                    anyhow::bail!("git symlink entries are not supported on this platform");
                }
            } else {
                let mut file = fs::File::create(&out)?;
                file.write_all(&blob.data)?;
                #[cfg(unix)]
                if entry.mode().is_executable() {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&out, fs::Permissions::from_mode(0o755))?;
                }
            }
        } else {
            anyhow::bail!("unsupported git tree entry mode for {name}");
        }
    }
    Ok(())
}

#[cfg(any(unix, test))]
fn validate_tree_symlink_target(root: &Path, link: &Path, target: &Path) -> Result<()> {
    anyhow::ensure!(
        !target.as_os_str().is_empty() && !target.is_absolute(),
        "git symlink target must be relative: {}",
        link.display()
    );
    let parent = link
        .parent()
        .context("git symlink has no parent")?
        .strip_prefix(root)
        .context("git symlink escaped materialization root")?;
    let mut depth = parent
        .components()
        .filter(|component| matches!(component, std::path::Component::Normal(_)))
        .count();
    for component in target.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(_) => depth += 1,
            std::path::Component::ParentDir if depth > 0 => depth -= 1,
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                anyhow::bail!(
                    "git symlink target escapes materialization root: {}",
                    link.display()
                )
            }
        }
    }
    Ok(())
}

fn validate_tree_write_limits(limits: &TreeWriteLimits) -> Result<()> {
    for (name, limit, hard_max) in [
        ("max_files", limits.max_files, HARD_MAX_FILES),
        (
            "max_directories",
            limits.max_directories,
            HARD_MAX_DIRECTORIES,
        ),
        (
            "max_total_bytes",
            limits.max_total_bytes,
            HARD_MAX_TOTAL_BYTES,
        ),
    ] {
        if limit.is_some_and(|limit| limit == 0) {
            anyhow::bail!("{name} must be greater than zero when provided");
        }
        if limit.is_some_and(|limit| limit > hard_max) {
            anyhow::bail!("{name} must not exceed {hard_max}");
        }
    }
    Ok(())
}

fn validate_download_budget(max_download_bytes: Option<u64>) -> Result<usize> {
    let max_download_bytes = max_download_bytes.unwrap_or(DEFAULT_MAX_DOWNLOAD_BYTES);
    anyhow::ensure!(
        max_download_bytes > 0,
        "max_download_bytes must be greater than zero"
    );
    anyhow::ensure!(
        max_download_bytes <= HARD_MAX_DOWNLOAD_BYTES,
        "max_download_bytes must not exceed {HARD_MAX_DOWNLOAD_BYTES}"
    );
    usize::try_from(max_download_bytes).context("max_download_bytes does not fit this platform")
}

fn reserve_tree_directory(stats: &mut TreeWriteStats, limits: &TreeWriteLimits) -> Result<()> {
    stats.directories_written = stats
        .directories_written
        .checked_add(1)
        .context("git tree directory count overflow")?;
    if limits
        .max_directories
        .is_some_and(|limit| stats.directories_written > limit)
    {
        anyhow::bail!("git tree directory count limit exceeded");
    }
    Ok(())
}

fn reserve_tree_blob(
    stats: &mut TreeWriteStats,
    limits: &TreeWriteLimits,
    bytes: u64,
) -> Result<()> {
    stats.files_written = stats
        .files_written
        .checked_add(1)
        .context("git tree file count overflow")?;
    stats.total_bytes = stats
        .total_bytes
        .checked_add(bytes)
        .context("git tree byte count overflow")?;
    if limits
        .max_files
        .is_some_and(|limit| stats.files_written > limit)
    {
        anyhow::bail!("git tree file count limit exceeded");
    }
    if limits
        .max_total_bytes
        .is_some_and(|limit| stats.total_bytes > limit)
    {
        anyhow::bail!("git tree byte limit exceeded");
    }
    Ok(())
}

fn validate_remote_url(url: &str) -> Result<()> {
    let parsed = url::Url::parse(url)?;
    if parsed.scheme() != "https" {
        anyhow::bail!("only HTTPS URLs supported, got: {}", parsed.scheme());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        anyhow::bail!("URL must not contain userinfo");
    }
    if parsed.host_str().is_none() {
        anyhow::bail!("URL must have a host");
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        anyhow::bail!("URL must not contain query or fragment");
    }
    Ok(())
}

fn validate_dest_dir(dest: &Path) -> Result<()> {
    if !dest.is_absolute() {
        anyhow::bail!("dest_dir must be absolute, got: {}", dest.display());
    }
    for component in dest.components() {
        if matches!(component, std::path::Component::ParentDir) {
            anyhow::bail!("dest_dir must not contain ..");
        }
    }
    Ok(())
}

fn is_full_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn signed_data_before_pgp(data: &[u8]) -> &[u8] {
    const MARKER: &[u8] = b"-----BEGIN PGP SIGNATURE-----";
    match data
        .windows(MARKER.len())
        .position(|window| window == MARKER)
    {
        Some(0) => &data[..0],
        Some(pos) if data.get(pos.wrapping_sub(1)) == Some(&b'\n') => &data[..pos - 1],
        Some(pos) => &data[..pos],
        None => data,
    }
}

fn signature_to_json(signature: gix::actor::SignatureRef<'_>) -> Value {
    serde_json::json!({
        "name": bstr_to_string(signature.name),
        "email": bstr_to_string(signature.email),
        "date": signature.time,
    })
}

fn bstr_to_string(value: &gix::bstr::BStr) -> String {
    String::from_utf8_lossy(value.as_ref()).into_owned()
}

fn bytes_to_string(value: &[u8]) -> String {
    String::from_utf8_lossy(value).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_https_remote_urls() {
        for url in [
            "ssh://github.com/example/repo.git",
            "git://github.com/example/repo.git",
            "file:///tmp/repo.git",
        ] {
            assert!(validate_remote_url(url).is_err(), "accepted {url}");
        }
    }

    #[test]
    fn rejects_remote_url_userinfo() {
        assert!(validate_remote_url("https://user:pass@example.com/repo.git").is_err());
        assert!(validate_remote_url("https://example.com/repo.git?token=secret").is_err());
        assert!(validate_remote_url("https://example.com/repo.git#secret").is_err());
    }

    #[test]
    fn rejects_relative_dest_dir() {
        assert!(validate_dest_dir(Path::new("relative/path")).is_err());
    }

    #[test]
    fn rejects_parent_components() {
        assert!(validate_dest_dir(Path::new("/tmp/../repo")).is_err());
    }

    #[test]
    fn tree_write_limits_fail_before_unbounded_materialization() {
        let limits = TreeWriteLimits {
            max_files: Some(1),
            max_directories: Some(1),
            max_total_bytes: Some(4),
        };
        let mut stats = TreeWriteStats::default();
        reserve_tree_directory(&mut stats, &limits).unwrap();
        assert!(reserve_tree_directory(&mut stats, &limits).is_err());

        let mut stats = TreeWriteStats::default();
        reserve_tree_blob(&mut stats, &limits, 4).unwrap();
        assert!(reserve_tree_blob(&mut stats, &limits, 1).is_err());

        let mut stats = TreeWriteStats::default();
        assert!(reserve_tree_blob(&mut stats, &limits, 5).is_err());
    }

    #[test]
    fn git_fetch_budget_interrupts_pack_download() {
        let state = Arc::new(FetchBudgetState {
            max_bytes: 4,
            downloaded_bytes: Arc::new(AtomicUsize::new(0)),
            interrupt: AtomicBool::new(false),
            exceeded: AtomicBool::new(false),
        });
        let mut root = FetchBudgetProgress::root(state.clone());
        let other = gix::NestedProgress::add_child_with_id(&mut root, "objects", *b"OBJS");
        gix::Count::inc_by(&other, 10);
        assert_eq!(state.downloaded_bytes.load(Ordering::Relaxed), 0);
        let pack = gix::NestedProgress::add_child_with_id(
            &mut root,
            "read pack",
            GIX_READ_PACK_PROGRESS_ID,
        );

        gix::Count::inc_by(&pack, 4);
        assert!(!state.interrupt.load(Ordering::Relaxed));
        gix::Count::inc_by(&pack, 1);
        assert!(state.exceeded.load(Ordering::Relaxed));
        assert!(state.interrupt.load(Ordering::Relaxed));
    }

    #[test]
    fn git_fetch_defaults_are_bounded() {
        assert_eq!(
            validate_download_budget(None).unwrap(),
            DEFAULT_MAX_DOWNLOAD_BYTES as usize
        );
        assert!(validate_download_budget(Some(0)).is_err());
        assert!(validate_download_budget(Some(HARD_MAX_DOWNLOAD_BYTES + 1)).is_err());

        let defaults = TreeWriteLimits {
            max_files: Some(DEFAULT_MAX_FILES),
            max_directories: Some(DEFAULT_MAX_DIRECTORIES),
            max_total_bytes: Some(DEFAULT_MAX_TOTAL_BYTES),
        };
        assert!(validate_tree_write_limits(&defaults).is_ok());
        assert!(validate_tree_write_limits(&TreeWriteLimits {
            max_files: Some(HARD_MAX_FILES + 1),
            ..TreeWriteLimits::default()
        })
        .is_err());
    }

    #[test]
    fn git_symlink_targets_must_remain_inside_materialization_root() {
        let root = Path::new("root");
        let link = root.join("nested/link");
        assert!(validate_tree_symlink_target(root, &link, Path::new("../target")).is_ok());
        assert!(validate_tree_symlink_target(root, &link, Path::new("../../escape")).is_err());
        assert!(validate_tree_symlink_target(root, &link, Path::new("/absolute")).is_err());
        assert!(validate_tree_symlink_target(root, &link, Path::new("")).is_err());
    }

    #[test]
    fn resolves_head_to_symbolic_default_branch() {
        let refs = vec![
            remote_branch(
                "refs/heads/main",
                "1111111111111111111111111111111111111111",
            ),
            RemoteRef {
                name: "HEAD".to_string(),
                sha: "2222222222222222222222222222222222222222".to_string(),
                kind: RefKind::Branch,
                tag_object: None,
                symbolic_target: Some("refs/heads/trunk".to_string()),
            },
            remote_branch(
                "refs/heads/trunk",
                "2222222222222222222222222222222222222222",
            ),
        ];
        let candidates = [
            "HEAD".to_string(),
            "refs/heads/HEAD".to_string(),
            "refs/tags/HEAD".to_string(),
        ];
        let resolved = resolve_remote_ref(&refs, "HEAD", &candidates).expect("HEAD resolves");
        assert_eq!(resolved.name, "HEAD");
        assert_eq!(
            resolved.symbolic_target.as_deref(),
            Some("refs/heads/trunk")
        );
    }

    #[test]
    fn resolves_head_to_main_when_symbolic_head_missing() {
        let refs = vec![
            remote_branch(
                "refs/heads/feature",
                "1111111111111111111111111111111111111111",
            ),
            remote_branch(
                "refs/heads/main",
                "2222222222222222222222222222222222222222",
            ),
        ];
        let candidates = [
            "HEAD".to_string(),
            "refs/heads/HEAD".to_string(),
            "refs/tags/HEAD".to_string(),
        ];
        let resolved =
            resolve_remote_ref(&refs, "HEAD", &candidates).expect("fallback main resolves");
        assert_eq!(resolved.name, "refs/heads/main");
    }

    #[test]
    fn resolves_head_to_master_when_main_missing() {
        let refs = vec![
            remote_branch(
                "refs/heads/feature",
                "1111111111111111111111111111111111111111",
            ),
            remote_branch(
                "refs/heads/master",
                "2222222222222222222222222222222222222222",
            ),
        ];
        let candidates = [
            "HEAD".to_string(),
            "refs/heads/HEAD".to_string(),
            "refs/tags/HEAD".to_string(),
        ];
        let resolved =
            resolve_remote_ref(&refs, "HEAD", &candidates).expect("fallback master resolves");
        assert_eq!(resolved.name, "refs/heads/master");
    }

    #[test]
    fn resolves_explicit_branch_without_head_fallback() {
        let refs = vec![
            remote_branch(
                "refs/heads/main",
                "1111111111111111111111111111111111111111",
            ),
            remote_branch("refs/heads/dev", "2222222222222222222222222222222222222222"),
        ];
        let candidates = [
            "dev".to_string(),
            "refs/heads/dev".to_string(),
            "refs/tags/dev".to_string(),
        ];
        let resolved =
            resolve_remote_ref(&refs, "dev", &candidates).expect("explicit branch resolves");
        assert_eq!(resolved.name, "refs/heads/dev");
    }

    #[test]
    fn ls_refs_uses_standard_agent_feature_name() {
        let (feature, value) = ls_refs_agent_feature();
        assert_eq!(feature, "agent");
        assert!(value
            .as_deref()
            .is_some_and(|agent| agent.starts_with("git/")));
    }

    fn remote_branch(name: &str, sha: &str) -> RemoteRef {
        RemoteRef {
            name: name.to_string(),
            sha: sha.to_string(),
            kind: RefKind::Branch,
            tag_object: None,
            symbolic_target: None,
        }
    }
}
