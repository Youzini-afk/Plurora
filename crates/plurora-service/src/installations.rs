use std::collections::{BTreeMap, BTreeSet};
use std::fs;
#[cfg(windows)]
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock, Weak};

use anyhow::{anyhow, bail, ensure, Context};
use async_trait::async_trait;
use cap_fs_ext::{DirExt as _, FollowSymlinks, OpenOptionsFollowExt as _};
use chrono::Utc;
use plurora_core::{
    canonical_json_bytes, decode_component_artifact_payload, ArtifactDescriptor,
    ComponentTrustClass, EventEnvelope, EventSequence, COMPONENT_DESCRIPTOR_TYPE_URI,
    INSTALLATION_CREATED, INSTALLATION_REMOVED, INSTALLATION_UPDATED,
};
use plurora_runtime::{
    EventStore, InstallationChange, InstallationControl, InstallationCreateRequest,
    InstallationDiff, InstallationItemDiff, InstallationListRequest, InstallationMutationAuthority,
    InstallationMutationResult, InstallationRemoveRequest, InstallationRollbackPointer,
    InstallationStateAction, InstallationStateAuthorityEvidence, InstallationStateDecision,
    InstallationStateDecisionAction, InstallationStateDecisionReceipt, InstallationStateSlotChange,
    InstallationStateSlotDiff, InstallationStateSlotRequirement,
    InstallationStateSnapshot as StateSnapshot,
    InstallationStateSnapshotEntry as StateSnapshotEntry, InstallationUpdateRequest,
    InstallationView, InstallationWorkSummary, ObjectStore, StateDisposition,
    INSTALLATION_STATE_AUTHORITY_EVIDENCE_MEDIA_TYPE, INSTALLATION_STATE_AUTHORITY_EVIDENCE_SCHEMA,
    INSTALLATION_STATE_AUTHORITY_EVIDENCE_TYPE_URI, INSTALLATION_STATE_OPERATION,
    INSTALLATION_STATE_RECEIPT_MEDIA_TYPE, INSTALLATION_STATE_REPLACEMENT_DECISION_RECEIPT_SCHEMA,
    INSTALLATION_STATE_REPLACEMENT_DECISION_RECEIPT_TYPE_URI,
    INSTALLATION_STATE_RESET_RECEIPT_SCHEMA, INSTALLATION_STATE_RESET_RECEIPT_TYPE_URI,
    INSTALLATION_STATE_SNAPSHOT_MEDIA_TYPE, INSTALLATION_STATE_SNAPSHOT_SCHEMA,
    INSTALLATION_STATE_SNAPSHOT_TYPE_URI,
};
use plurora_work::{
    validate_artifact_descriptor, validate_assembly_closure, validate_state_replacement,
    ArtifactModel, AssemblyLock, AssemblyNodeSource, AssemblyRevision, InstallationId,
    InstallationRecord, InstallationStatus, OperationalIntent, PortDescriptor, PortEndpoint,
    PortId, RightsDeclaration, StateSlotDescriptor, TransparencyDeclaration, WorkRevision,
    ASSEMBLY_LOCK_TYPE_URI, ASSEMBLY_REVISION_TYPE_URI, MAX_ASSEMBLY_DEPTH,
    OPERATIONAL_INTENT_TYPE_URI, RIGHTS_DECLARATION_TYPE_URI, TRANSPARENCY_DECLARATION_TYPE_URI,
    WORK_REVISION_TYPE_URI,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::DevelopmentHostLease;

const JOURNAL_SESSION: &str = "host_installations";
const JOURNAL_WRITER: &str = "host/control-plane";
const JOURNAL_SCHEMA: u16 = 1;
const CAS_ATTEMPTS: usize = 8;
const JOURNAL_PAGE: usize = 1_000;

const UPDATE_STARTED: &str = "host/internal/installation.update_started";
const UPDATE_ROLLBACK: &str = "host/internal/installation.update_rollback";
const REMOVE_STARTED: &str = "host/internal/installation.remove_started";
const REMOVE_ROLLBACK: &str = "host/internal/installation.remove_rollback";
const RECOVERY_BLOCKED: &str = "host/internal/installation.recovery_blocked";
const IDEMPOTENT_NOOP: &str = "host/internal/installation.noop";

/// The Service-owned durable control plane for Installation lifecycle state.
/// The journal is authoritative; files under `installations/` are rebuildable projections.
pub struct InstallationRegistry {
    store: Arc<dyn EventStore>,
    object_store: Arc<dyn ObjectStore>,
    data_root: PathBuf,
    data_root_anchor: same_file::Handle,
    installations_root_anchor: same_file::Handle,
    views: RwLock<BTreeMap<InstallationId, InstallationView>>,
    idempotency: RwLock<BTreeMap<String, IdempotencyClaim>>,
    pending: RwLock<BTreeMap<InstallationId, PendingMutation>>,
    next_sequence: Mutex<EventSequence>,
    /// Per-Installation lifecycle locks. The table is weak so unknown IDs,
    /// failed creates, and removed Installations do not become permanent keys.
    /// A live guard owns the lock strongly, which keeps one exact lock unique
    /// for the ID until every reader/writer has left that lifecycle.
    lifecycles: Mutex<BTreeMap<InstallationId, Weak<tokio::sync::RwLock<()>>>>,
    /// Serializes synchronization and CAS application for the one Installation
    /// journal. It is never retained as a Run or secret-effect lifecycle lease.
    apply: Arc<tokio::sync::Mutex<()>>,
    owner_lease: RwLock<Option<DevelopmentHostLease>>,
    #[cfg(test)]
    projection_failure: Mutex<Option<ProjectionFailure>>,
    #[cfg(test)]
    authority_append_barrier: Mutex<Option<AuthorityAppendBarrier>>,
    _temporary_root: Option<TempDir>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct IdempotencyClaim {
    key_hash: String,
    fingerprint: String,
    result: InstallationMutationResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PendingMutation {
    operation_id: String,
    kind: PendingKind,
    previous: InstallationView,
    state_snapshot: ArtifactDescriptor,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    receipts: Vec<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum PendingKind {
    Update,
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreatedPayload {
    view: InstallationView,
    claim: IdempotencyClaim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpdatedPayload {
    operation_id: Option<String>,
    previous_revision: u64,
    view: InstallationView,
    claim: IdempotencyClaim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemovedPayload {
    operation_id: Option<String>,
    previous_revision: u64,
    view: InstallationView,
    claim: IdempotencyClaim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StartedPayload {
    pending: PendingMutation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RollbackPayload {
    installation_id: InstallationId,
    operation_id: String,
    previous: InstallationView,
    reason_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockedPayload {
    installation_id: InstallationId,
    operation_id: String,
    reason_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NoopPayload {
    claim: IdempotencyClaim,
}

struct PreparedProjection {
    _installation: AnchoredDirectory,
    record_replacement: AtomicReplacement,
    lock_replacement: AtomicReplacement,
}

struct AnchoredDirectory {
    path: PathBuf,
    handle: same_file::Handle,
}

impl AnchoredDirectory {
    fn capability(&self, label: &str) -> anyhow::Result<cap_std::fs::Dir> {
        capability_directory(self, label)
    }

    fn effect_path(&self) -> PathBuf {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            use std::os::fd::AsRawFd;
            // Keep the procfs descriptor link as an intermediate component. A path
            // ending at `/proc/self/fd/<fd>` is itself a symlink under lstat, while
            // the trailing `/.` resolves to the already-open directory without
            // weakening validation for ordinary caller-controlled symlinks.
            return PathBuf::from(format!(
                "/proc/self/fd/{}/.",
                self.handle.as_file().as_raw_fd()
            ));
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            self.path.clone()
        }
    }

    fn ensure_current_path(&self, label: &str) -> anyhow::Result<()> {
        let current = same_file::Handle::from_path(&self.path)
            .map_err(|_| anyhow!("identify {label} failed"))?;
        ensure!(current == self.handle, "{label} identity changed");
        Ok(())
    }
}

struct AtomicReplacement {
    directory: cap_std::fs::Dir,
    target_name: PathBuf,
    temporary_name: Option<PathBuf>,
    _temporary: cap_std::fs::File,
}

impl AtomicReplacement {
    fn publish(mut self) -> anyhow::Result<()> {
        validate_anchored_projection_target(&self.directory, &self.target_name)?;
        let temporary_name = self
            .temporary_name
            .as_ref()
            .ok_or_else(|| anyhow!("projection temporary file is no longer available"))?;
        self.directory
            .rename(temporary_name, &self.directory, &self.target_name)
            .map_err(|_| anyhow!("publish projection file failed"))?;
        self.temporary_name = None;
        Ok(())
    }
}

impl Drop for AtomicReplacement {
    fn drop(&mut self) {
        if let Some(temporary_name) = self.temporary_name.take() {
            let _ = self.directory.remove_file(temporary_name);
        }
    }
}

#[derive(Clone)]
struct ClosureItem {
    descriptor: ArtifactDescriptor,
    depth: usize,
    component_expectation: Option<ComponentExpectation>,
}

#[derive(Clone)]
struct ComponentExpectation {
    behavior_digest: String,
    trust_class: ComponentTrustClass,
}

struct VerifiedInstallationArtifacts {
    work: WorkRevision,
    assembly: AssemblyRevision,
    lock: AssemblyLock,
    lock_bytes: Vec<u8>,
    assemblies: BTreeMap<String, AssemblyRevision>,
    locks: BTreeMap<String, AssemblyLock>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectionFailure {
    Prepare,
    Publish,
}

#[cfg(test)]
struct AuthorityAppendBarrier {
    kind: String,
    entered: Arc<tokio::sync::Barrier>,
    release: Arc<tokio::sync::Barrier>,
}

#[cfg(all(test, unix))]
struct StateOpenSwap {
    relative: PathBuf,
    path: PathBuf,
    replacement: PathBuf,
}

#[cfg(all(test, unix))]
static STATE_OPEN_SWAP: std::sync::OnceLock<Mutex<Option<StateOpenSwap>>> =
    std::sync::OnceLock::new();

#[cfg(all(test, unix))]
struct StateRootOpenSwap {
    path: PathBuf,
    parked: PathBuf,
    replacement: PathBuf,
}

#[cfg(all(test, unix))]
static STATE_ROOT_OPEN_SWAP: std::sync::OnceLock<Mutex<Option<StateRootOpenSwap>>> =
    std::sync::OnceLock::new();

#[cfg(all(test, unix))]
struct InstallationAncestorSwap {
    path: PathBuf,
    parked: PathBuf,
    replacement: PathBuf,
}

#[cfg(all(test, unix))]
static INSTALLATION_ANCESTOR_SWAP: std::sync::OnceLock<Mutex<Option<InstallationAncestorSwap>>> =
    std::sync::OnceLock::new();

#[cfg(all(test, unix))]
static PROJECTION_ANCESTOR_SWAP: std::sync::OnceLock<Mutex<Option<InstallationAncestorSwap>>> =
    std::sync::OnceLock::new();

#[cfg(all(test, unix))]
struct RemoveTreeSwap {
    root: PathBuf,
    target: PathBuf,
    parked: PathBuf,
    replacement: PathBuf,
}

#[cfg(all(test, unix))]
static REMOVE_TREE_SWAP: std::sync::OnceLock<Mutex<Option<RemoveTreeSwap>>> =
    std::sync::OnceLock::new();

impl InstallationRegistry {
    pub fn persistent(
        store: Arc<dyn EventStore>,
        object_store: Arc<dyn ObjectStore>,
        data_dir: impl AsRef<Path>,
    ) -> anyhow::Result<Arc<Self>> {
        let data_root = prepare_data_root(data_dir.as_ref())?;
        Self::build(store, object_store, data_root, None)
    }

    pub fn ephemeral(
        store: Arc<dyn EventStore>,
        object_store: Arc<dyn ObjectStore>,
    ) -> anyhow::Result<Arc<Self>> {
        let temporary_root = tempfile::tempdir().context("create installation data root")?;
        let data_root = prepare_data_root(temporary_root.path())?;
        Self::build(store, object_store, data_root, Some(temporary_root))
    }

    fn build(
        store: Arc<dyn EventStore>,
        object_store: Arc<dyn ObjectStore>,
        data_root: PathBuf,
        temporary_root: Option<TempDir>,
    ) -> anyhow::Result<Arc<Self>> {
        prepare_real_directory(&data_root, "installation data root")?;
        let installations = data_root.join("installations");
        prepare_real_directory(&installations, "installations root")?;
        ensure_contained(&data_root, &installations, "installations root")?;
        let data_root_anchor = open_anchored_directory(&data_root, "installation data root")?;
        let installations_root_anchor =
            open_anchored_directory(&installations, "installations root")?;
        Ok(Arc::new(Self {
            store,
            object_store,
            data_root,
            data_root_anchor: data_root_anchor.handle,
            installations_root_anchor: installations_root_anchor.handle,
            views: RwLock::new(BTreeMap::new()),
            idempotency: RwLock::new(BTreeMap::new()),
            pending: RwLock::new(BTreeMap::new()),
            next_sequence: Mutex::new(0),
            lifecycles: Mutex::new(BTreeMap::new()),
            apply: Arc::new(tokio::sync::Mutex::new(())),
            owner_lease: RwLock::new(None),
            #[cfg(test)]
            projection_failure: Mutex::new(None),
            #[cfg(test)]
            authority_append_barrier: Mutex::new(None),
            _temporary_root: temporary_root,
        }))
    }

    /// Bind this registry to the live Host owner. Once installed, every
    /// Installation journal, state, and projection effect revalidates the
    /// durable lease tail rather than trusting the startup snapshot.
    pub fn install_owner_lease(&self, lease: DevelopmentHostLease) -> anyhow::Result<()> {
        lease.ensure_active()?;
        *self.owner_lease.write().map_err(lock_error)? = Some(lease);
        Ok(())
    }

    async fn ensure_owner_lease(&self) -> anyhow::Result<()> {
        let lease = self.owner_lease.read().map_err(lock_error)?.clone();
        if let Some(lease) = lease {
            lease.ensure_durable_owner().await?;
        }
        Ok(())
    }

    /// Rebuild the complete registry from the dedicated Installation journal.
    /// An incomplete state-changing operation is rolled back before this returns.
    pub async fn hydrate(&self) -> anyhow::Result<usize> {
        self.ensure_owner_lease().await?;
        let outcome = async {
            {
                let _apply = self.apply.lock().await;
                self.reset_memory()?;
                self.sync_journal_locked().await?;
            }
            self.recover_pending().await?;
            let views = self.views_snapshot()?;
            for view in views.values() {
                self.ensure_owner_lease().await?;
                self.materialize_projection(view).await?;
            }
            Ok(views.len())
        }
        .await;
        if outcome.is_err() {
            // A partially replayed control plane must never remain observable after a
            // malformed journal, blocked rollback, or projection-integrity failure.
            let _apply = self.apply.lock().await;
            let _ = self.reset_memory();
        }
        outcome
    }

    fn reset_memory(&self) -> anyhow::Result<()> {
        self.views.write().map_err(lock_error)?.clear();
        self.idempotency.write().map_err(lock_error)?.clear();
        self.pending.write().map_err(lock_error)?.clear();
        *self.next_sequence.lock().map_err(lock_error)? = 0;
        Ok(())
    }

    fn views_snapshot(&self) -> anyhow::Result<BTreeMap<InstallationId, InstallationView>> {
        Ok(self.views.read().map_err(lock_error)?.clone())
    }

    fn lifecycle_lock(
        &self,
        installation_id: &InstallationId,
    ) -> anyhow::Result<Arc<tokio::sync::RwLock<()>>> {
        let mut lifecycles = self.lifecycles.lock().map_err(lock_error)?;
        lifecycles.retain(|_, lifecycle| lifecycle.strong_count() != 0);
        if let Some(lifecycle) = lifecycles.get(installation_id).and_then(Weak::upgrade) {
            return Ok(lifecycle);
        }
        let lifecycle = Arc::new(tokio::sync::RwLock::new(()));
        lifecycles.insert(installation_id.clone(), Arc::downgrade(&lifecycle));
        Ok(lifecycle)
    }

    /// Synchronize the in-memory authority while the caller owns `apply`.
    /// No artifact validation or filesystem effect belongs in this critical
    /// section; it serializes only journal sequence/CAS application.
    async fn sync_journal_locked(&self) -> anyhow::Result<()> {
        loop {
            let next = *self.next_sequence.lock().map_err(lock_error)?;
            let after = next.checked_sub(1);
            let events = self
                .store
                .list_session_range(&JOURNAL_SESSION.to_string(), after, Some(JOURNAL_PAGE))
                .await
                .context("read installation journal")?;
            if events.is_empty() {
                return Ok(());
            }
            for event in &events {
                self.apply_event(event)?;
            }
            if events.len() < JOURNAL_PAGE {
                return Ok(());
            }
        }
    }

    fn apply_event(&self, event: &EventEnvelope) -> anyhow::Result<()> {
        let expected = *self.next_sequence.lock().map_err(lock_error)?;
        ensure!(
            event.session_id == JOURNAL_SESSION,
            "installation journal has wrong session"
        );
        ensure!(
            event.writer_package_id == JOURNAL_WRITER,
            "installation journal has untrusted writer"
        );
        ensure!(
            event.schema_version == JOURNAL_SCHEMA,
            "installation journal has unsupported schema"
        );
        ensure!(
            event.sequence == expected,
            "installation journal sequence gap"
        );

        match event.kind.as_str() {
            INSTALLATION_CREATED => {
                let payload: CreatedPayload = parse_payload(event)?;
                validate_view(&payload.view)?;
                ensure!(
                    payload.claim.result.installation == payload.view
                        && payload.claim.result.diff.is_none()
                        && payload.claim.result.receipts.is_empty()
                        && !payload.claim.result.idempotent,
                    "created installation claim does not match its event"
                );
                ensure!(
                    payload.view.revision == 1,
                    "created installation revision is invalid"
                );
                ensure!(
                    payload.view.record.status == InstallationStatus::Ready,
                    "created installation status is invalid"
                );
                ensure!(
                    payload.view.rollback.is_none(),
                    "created installation must not have a rollback pointer"
                );
                let id = payload.view.record.installation_id.clone();
                ensure!(
                    !self.views.read().map_err(lock_error)?.contains_key(&id),
                    "duplicate installation creation"
                );
                self.install_claim(payload.claim)?;
                self.views
                    .write()
                    .map_err(lock_error)?
                    .insert(id, payload.view);
            }
            INSTALLATION_UPDATED => {
                let payload: UpdatedPayload = parse_payload(event)?;
                validate_view(&payload.view)?;
                ensure!(
                    payload.claim.result.installation == payload.view
                        && payload.claim.result.diff.is_some()
                        && !payload.claim.result.idempotent,
                    "updated installation claim does not match its event"
                );
                let id = payload.view.record.installation_id.clone();
                let old = self
                    .views
                    .read()
                    .map_err(lock_error)?
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| anyhow!("updated installation does not exist"))?;
                ensure!(
                    old.revision == payload.previous_revision,
                    "update revision mismatch"
                );
                ensure!(
                    payload.view.revision
                        == old
                            .revision
                            .checked_add(1)
                            .ok_or_else(|| anyhow!("revision overflow"))?,
                    "update revision is not monotonic"
                );
                ensure!(
                    payload.view.record.created_at == old.record.created_at
                        && payload.view.record.updated_at >= old.record.updated_at
                        && payload.view.record.status == InstallationStatus::Ready,
                    "updated installation violates lifecycle invariants"
                );
                let rollback = payload
                    .view
                    .rollback
                    .as_ref()
                    .ok_or_else(|| anyhow!("updated installation has no rollback pointer"))?;
                ensure!(
                    rollback.revision == old.revision
                        && rollback.work_revision == old.record.work_revision
                        && rollback.assembly_lock == old.record.assembly_lock,
                    "updated installation rollback pointer does not identify its prior active state"
                );
                match payload.operation_id.as_deref() {
                    Some(_) => {
                        let pending = self
                            .pending
                            .read()
                            .map_err(lock_error)?
                            .get(&id)
                            .cloned()
                            .ok_or_else(|| anyhow!("terminal update has no pending start"))?;
                        ensure!(
                            rollback.state_snapshot.as_ref() == Some(&pending.state_snapshot),
                            "updated installation rollback snapshot does not match its start"
                        );
                        ensure!(
                            payload.claim.result.receipts == pending.receipts,
                            "updated installation receipts do not match its start"
                        );
                        validate_state_receipt_descriptors(&pending.receipts)?;
                    }
                    None => {
                        ensure!(
                            rollback.state_snapshot.is_none(),
                            "direct update unexpectedly carries a state snapshot"
                        );
                        ensure!(
                            payload.claim.result.receipts.is_empty(),
                            "direct update unexpectedly carries state receipts"
                        );
                    }
                }
                self.resolve_pending(&id, payload.operation_id.as_deref(), PendingKind::Update)?;
                self.install_claim(payload.claim)?;
                self.views
                    .write()
                    .map_err(lock_error)?
                    .insert(id, payload.view);
            }
            INSTALLATION_REMOVED => {
                let payload: RemovedPayload = parse_payload(event)?;
                validate_view(&payload.view)?;
                ensure!(
                    payload.claim.result.installation == payload.view
                        && payload.claim.result.diff.is_none()
                        && payload.claim.result.receipts.is_empty()
                        && !payload.claim.result.idempotent,
                    "removed installation claim does not match its event"
                );
                let id = payload.view.record.installation_id.clone();
                let old = self
                    .views
                    .read()
                    .map_err(lock_error)?
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| anyhow!("removed installation does not exist"))?;
                ensure!(
                    old.revision == payload.previous_revision,
                    "remove revision mismatch"
                );
                ensure!(
                    payload.view.revision
                        == old
                            .revision
                            .checked_add(1)
                            .ok_or_else(|| anyhow!("revision overflow"))?
                        && payload.view.record.status == InstallationStatus::Removed,
                    "removed installation violates lifecycle invariants"
                );
                let mut expected_record = old.record.clone();
                expected_record.status = InstallationStatus::Removed;
                expected_record.updated_at = payload.view.record.updated_at;
                ensure!(
                    payload.view.record == expected_record
                        && payload.view.record.updated_at >= old.record.updated_at,
                    "removed installation changed active content"
                );
                self.resolve_pending(&id, payload.operation_id.as_deref(), PendingKind::Remove)?;
                self.install_claim(payload.claim)?;
                self.views
                    .write()
                    .map_err(lock_error)?
                    .insert(id, payload.view);
            }
            UPDATE_STARTED | REMOVE_STARTED => {
                let payload: StartedPayload = parse_payload(event)?;
                validate_view(&payload.pending.previous)?;
                let expected_kind = if event.kind == UPDATE_STARTED {
                    PendingKind::Update
                } else {
                    PendingKind::Remove
                };
                ensure!(
                    payload.pending.kind == expected_kind,
                    "pending mutation kind mismatch"
                );
                ensure!(
                    !payload.pending.operation_id.is_empty(),
                    "pending mutation operation id is empty"
                );
                validate_artifact_descriptor(&payload.pending.state_snapshot)
                    .map_err(|_| anyhow!("pending state snapshot descriptor is invalid"))?;
                ensure!(
                    payload.pending.state_snapshot.artifact_type_uri
                        == INSTALLATION_STATE_SNAPSHOT_TYPE_URI,
                    "pending state snapshot descriptor type is invalid"
                );
                match expected_kind {
                    PendingKind::Update => {
                        validate_state_receipt_descriptors(&payload.pending.receipts)?
                    }
                    PendingKind::Remove => ensure!(
                        payload.pending.receipts.is_empty(),
                        "pending removal unexpectedly carries state decision receipts"
                    ),
                }
                let id = payload.pending.previous.record.installation_id.clone();
                let current = self
                    .views
                    .read()
                    .map_err(lock_error)?
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| anyhow!("pending installation does not exist"))?;
                ensure!(
                    current == payload.pending.previous,
                    "pending mutation base mismatch"
                );
                ensure!(
                    !self.pending.read().map_err(lock_error)?.contains_key(&id),
                    "installation already has a pending mutation"
                );
                let mut transitional = current;
                transitional.record.status = match expected_kind {
                    PendingKind::Update => InstallationStatus::Updating,
                    PendingKind::Remove => InstallationStatus::Removing,
                };
                self.views
                    .write()
                    .map_err(lock_error)?
                    .insert(id.clone(), transitional);
                self.pending
                    .write()
                    .map_err(lock_error)?
                    .insert(id, payload.pending);
            }
            UPDATE_ROLLBACK | REMOVE_ROLLBACK => {
                let payload: RollbackPayload = parse_payload(event)?;
                validate_view(&payload.previous)?;
                ensure!(
                    payload.previous.record.installation_id == payload.installation_id,
                    "rollback installation mismatch"
                );
                let expected_kind = if event.kind == UPDATE_ROLLBACK {
                    PendingKind::Update
                } else {
                    PendingKind::Remove
                };
                let pending = self
                    .pending
                    .write()
                    .map_err(lock_error)?
                    .remove(&payload.installation_id)
                    .ok_or_else(|| anyhow!("rollback has no pending mutation"))?;
                ensure!(
                    pending.kind == expected_kind && pending.operation_id == payload.operation_id,
                    "rollback operation mismatch"
                );
                ensure!(
                    pending.previous == payload.previous,
                    "rollback base mismatch"
                );
                self.views
                    .write()
                    .map_err(lock_error)?
                    .insert(payload.installation_id, payload.previous);
            }
            RECOVERY_BLOCKED => {
                let payload: BlockedPayload = parse_payload(event)?;
                let pending = self
                    .pending
                    .read()
                    .map_err(lock_error)?
                    .get(&payload.installation_id)
                    .cloned()
                    .ok_or_else(|| anyhow!("blocked recovery has no pending mutation"))?;
                ensure!(
                    pending.operation_id == payload.operation_id,
                    "blocked recovery operation mismatch"
                );
                let mut blocked = pending.previous;
                blocked.record.status = InstallationStatus::Blocked;
                self.views
                    .write()
                    .map_err(lock_error)?
                    .insert(payload.installation_id, blocked);
            }
            IDEMPOTENT_NOOP => {
                let payload: NoopPayload = parse_payload(event)?;
                validate_view(&payload.claim.result.installation)?;
                ensure!(
                    payload.claim.result.idempotent,
                    "no-op installation claim is not idempotent"
                );
                ensure!(
                    payload.claim.result.receipts.is_empty(),
                    "no-op installation claim unexpectedly carries receipts"
                );
                let installation = &payload.claim.result.installation;
                ensure!(
                    self.views
                        .read()
                        .map_err(lock_error)?
                        .get(&installation.record.installation_id)
                        == Some(installation),
                    "no-op installation claim does not match current state"
                );
                self.install_claim(payload.claim)?;
            }
            _ => bail!("installation journal contains unknown event kind"),
        }
        *self.next_sequence.lock().map_err(lock_error)? = expected
            .checked_add(1)
            .ok_or_else(|| anyhow!("installation journal sequence overflow"))?;
        Ok(())
    }

    fn resolve_pending(
        &self,
        installation_id: &InstallationId,
        operation_id: Option<&str>,
        expected_kind: PendingKind,
    ) -> anyhow::Result<()> {
        match operation_id {
            Some(operation_id) => {
                let pending = self
                    .pending
                    .write()
                    .map_err(lock_error)?
                    .remove(installation_id)
                    .ok_or_else(|| anyhow!("terminal mutation has no pending start"))?;
                ensure!(
                    pending.kind == expected_kind && pending.operation_id == operation_id,
                    "terminal mutation does not match its start"
                );
            }
            None => ensure!(
                !self
                    .pending
                    .read()
                    .map_err(lock_error)?
                    .contains_key(installation_id),
                "direct mutation conflicts with a pending operation"
            ),
        }
        Ok(())
    }

    fn install_claim(&self, claim: IdempotencyClaim) -> anyhow::Result<()> {
        validate_claim(&claim)?;
        let mut claims = self.idempotency.write().map_err(lock_error)?;
        if let Some(existing) = claims.get(&claim.key_hash) {
            ensure!(
                existing == &claim,
                "idempotency claim was reused inconsistently"
            );
            return Ok(());
        }
        claims.insert(claim.key_hash.clone(), claim);
        Ok(())
    }

    fn claimed_result(
        &self,
        key_hash: &str,
        fingerprint: &str,
    ) -> anyhow::Result<Option<InstallationMutationResult>> {
        let claims = self.idempotency.read().map_err(lock_error)?;
        let Some(claim) = claims.get(key_hash) else {
            return Ok(None);
        };
        ensure!(
            claim.fingerprint == fingerprint,
            "idempotency_conflict: key already identifies a different mutation"
        );
        let mut result = claim.result.clone();
        result.idempotent = true;
        Ok(Some(result))
    }

    async fn append_payload_locked<T: Serialize>(
        &self,
        kind: &str,
        payload: &T,
    ) -> anyhow::Result<bool> {
        let sequence = *self.next_sequence.lock().map_err(lock_error)?;
        let payload =
            serde_json::to_value(payload).context("encode installation journal payload")?;
        self.ensure_owner_lease().await?;
        let appended = self
            .store
            .append_with_sequence_if_next(
                JOURNAL_SESSION.to_string(),
                sequence,
                JOURNAL_WRITER.to_string(),
                kind.to_string(),
                JOURNAL_SCHEMA,
                payload,
                serde_json::json!({}),
            )
            .await
            .context("append installation journal")?;
        let Some(event) = appended else {
            return Ok(false);
        };
        self.apply_event(&event)?;
        Ok(true)
    }

    /// Append a mutation payload only while the exact resource grant is still
    /// current. Owner lease validation deliberately happens first so authority
    /// refresh is the final asynchronous boundary before the journal effect.
    async fn append_payload_with_installation_authority_locked<T: Serialize>(
        &self,
        kind: &str,
        payload: &T,
        installation_id: &InstallationId,
        authority: &InstallationMutationAuthority,
    ) -> anyhow::Result<bool> {
        let sequence = *self.next_sequence.lock().map_err(lock_error)?;
        let payload =
            serde_json::to_value(payload).context("encode installation journal payload")?;
        self.ensure_owner_lease().await?;
        authority
            .refresh_current_for_installation(installation_id)
            .await?;
        let appended = self
            .store
            .append_with_sequence_if_next(
                JOURNAL_SESSION.to_string(),
                sequence,
                JOURNAL_WRITER.to_string(),
                kind.to_string(),
                JOURNAL_SCHEMA,
                payload,
                serde_json::json!({}),
            )
            .await
            .context("append installation journal")?;
        let Some(event) = appended else {
            return Ok(false);
        };
        self.apply_event(&event)?;
        Ok(true)
    }

    /// Create is authorized against the exact Work rather than a not-yet-existing
    /// Installation. Keep the Host owner check before the grant refresh so the
    /// refresh is the final asynchronous boundary before the journal append.
    async fn append_payload_with_work_authority_locked<T: Serialize>(
        &self,
        kind: &str,
        payload: &T,
        work_id: &plurora_work::WorkId,
        authority: &InstallationMutationAuthority,
    ) -> anyhow::Result<bool> {
        let sequence = *self.next_sequence.lock().map_err(lock_error)?;
        let payload =
            serde_json::to_value(payload).context("encode installation journal payload")?;
        self.ensure_owner_lease().await?;
        authority.refresh_current_for_work(work_id).await?;
        let appended = self
            .store
            .append_with_sequence_if_next(
                JOURNAL_SESSION.to_string(),
                sequence,
                JOURNAL_WRITER.to_string(),
                kind.to_string(),
                JOURNAL_SCHEMA,
                payload,
                serde_json::json!({}),
            )
            .await
            .context("append installation journal")?;
        let Some(event) = appended else {
            return Ok(false);
        };
        self.apply_event(&event)?;
        Ok(true)
    }

    /// Test barriers sit immediately before, rather than inside, the short
    /// journal critical section. Production still refreshes authority after
    /// acquiring `apply`, so a grant cannot expire while waiting for its turn.
    async fn before_authority_append(&self, kind: &str) -> anyhow::Result<()> {
        #[cfg(test)]
        self.wait_authority_append_barrier(kind).await?;
        #[cfg(not(test))]
        let _ = kind;
        Ok(())
    }

    /// Test/recovery helper for an unauthenticated internal event. Normal
    /// mutation paths use event-specific preconditions under the same lock.
    async fn append_payload<T: Serialize>(&self, kind: &str, payload: &T) -> anyhow::Result<bool> {
        let _apply = self.apply.lock().await;
        self.sync_journal_locked().await?;
        self.append_payload_locked(kind, payload).await
    }

    async fn recover_pending(&self) -> anyhow::Result<()> {
        loop {
            let pending = self
                .pending
                .read()
                .map_err(lock_error)?
                .values()
                .next()
                .cloned();
            let Some(pending) = pending else {
                return Ok(());
            };
            let id = pending.previous.record.installation_id.clone();
            let _lifecycle = self.lifecycle_lock(&id)?.write_owned().await;
            {
                let _apply = self.apply.lock().await;
                self.sync_journal_locked().await?;
                let current = self.pending.read().map_err(lock_error)?.get(&id).cloned();
                let Some(current) = current else {
                    continue;
                };
                ensure!(
                    current == pending,
                    "pending recovery changed before lifecycle exclusion"
                );
            }
            if let Err(_error) = self.restore_state(&id, &pending.state_snapshot).await {
                let blocked = BlockedPayload {
                    installation_id: id,
                    operation_id: pending.operation_id,
                    reason_code: "state_restore_failed".to_string(),
                };
                self.append_pending_blocked(&blocked).await?;
                bail!("installation recovery blocked: state_restore_failed");
            }
            let rollback = RollbackPayload {
                installation_id: id,
                operation_id: pending.operation_id,
                previous: pending.previous,
                reason_code: "incomplete_operation".to_string(),
            };
            let kind = match pending.kind {
                PendingKind::Update => UPDATE_ROLLBACK,
                PendingKind::Remove => REMOVE_ROLLBACK,
            };
            self.append_pending_rollback(kind, &rollback).await?;
        }
    }

    async fn append_pending_blocked(&self, payload: &BlockedPayload) -> anyhow::Result<()> {
        for _ in 0..CAS_ATTEMPTS {
            let _apply = self.apply.lock().await;
            self.sync_journal_locked().await?;
            let pending = self
                .pending
                .read()
                .map_err(lock_error)?
                .get(&payload.installation_id)
                .cloned();
            let Some(pending) = pending else {
                return Ok(());
            };
            ensure!(
                pending.operation_id == payload.operation_id,
                "recovery operation was replaced before it could be blocked"
            );
            let current = self
                .views
                .read()
                .map_err(lock_error)?
                .get(&payload.installation_id)
                .cloned()
                .ok_or_else(|| anyhow!("pending installation disappeared"))?;
            if current.record.status == InstallationStatus::Blocked {
                return Ok(());
            }
            ensure!(
                matches!(
                    current.record.status,
                    InstallationStatus::Updating | InstallationStatus::Removing
                ),
                "pending recovery no longer owns the current installation state"
            );
            if self
                .append_payload_locked(RECOVERY_BLOCKED, payload)
                .await?
            {
                return Ok(());
            }
        }
        bail!("installation recovery contention did not converge")
    }

    async fn append_pending_rollback(
        &self,
        kind: &str,
        payload: &RollbackPayload,
    ) -> anyhow::Result<()> {
        let expected_kind = match kind {
            UPDATE_ROLLBACK => PendingKind::Update,
            REMOVE_ROLLBACK => PendingKind::Remove,
            _ => bail!("invalid rollback event kind"),
        };
        for _ in 0..CAS_ATTEMPTS {
            let _apply = self.apply.lock().await;
            self.sync_journal_locked().await?;
            let pending = self
                .pending
                .read()
                .map_err(lock_error)?
                .get(&payload.installation_id)
                .cloned();
            let Some(pending) = pending else {
                // Another registry already committed either this rollback or the terminal
                // mutation. Both are known terminal states; replaying our stale payload
                // would poison the append-only journal.
                ensure!(
                    self.views
                        .read()
                        .map_err(lock_error)?
                        .contains_key(&payload.installation_id),
                    "resolved installation disappeared"
                );
                return Ok(());
            };
            ensure!(
                pending.kind == expected_kind
                    && pending.operation_id == payload.operation_id
                    && pending.previous == payload.previous,
                "rollback no longer matches the pending operation"
            );
            let current = self
                .views
                .read()
                .map_err(lock_error)?
                .get(&payload.installation_id)
                .cloned()
                .ok_or_else(|| anyhow!("pending installation disappeared"))?;
            ensure!(
                current.record.status != InstallationStatus::Blocked,
                "installation recovery is blocked"
            );
            if self.append_payload_locked(kind, payload).await? {
                return Ok(());
            }
        }
        bail!("installation rollback contention did not converge")
    }

    async fn verify_work_and_lock(
        &self,
        work_descriptor: &ArtifactDescriptor,
        lock_descriptor: &ArtifactDescriptor,
    ) -> anyhow::Result<VerifiedInstallationArtifacts> {
        ensure!(
            work_descriptor.artifact_type_uri == WORK_REVISION_TYPE_URI,
            "work revision descriptor type is invalid"
        );
        ensure!(
            lock_descriptor.artifact_type_uri == ASSEMBLY_LOCK_TYPE_URI,
            "assembly lock descriptor type is invalid"
        );
        let (work, _) = self
            .verified_artifact_model::<WorkRevision>(work_descriptor, "work revision")
            .await?;
        let (lock, lock_bytes) = self
            .verified_artifact_model::<AssemblyLock>(lock_descriptor, "assembly lock")
            .await?;
        ensure!(
            work.assembly == lock.assembly,
            "work and assembly lock disagree"
        );

        let mut pending = vec![
            ClosureItem {
                descriptor: work_descriptor.clone(),
                depth: 1,
                component_expectation: None,
            },
            ClosureItem {
                descriptor: lock_descriptor.clone(),
                depth: 1,
                component_expectation: None,
            },
        ];
        let mut descriptors = BTreeMap::<String, ArtifactDescriptor>::new();
        let mut expanded = BTreeSet::<String>::new();
        let mut assemblies = BTreeMap::<String, AssemblyRevision>::new();
        let mut locks = BTreeMap::<String, AssemblyLock>::new();

        while let Some(item) = pending.pop() {
            ensure!(
                item.depth <= MAX_ASSEMBLY_DEPTH,
                "work_too_complex: assembly nesting depth is exceeded"
            );
            if let Some(existing) = descriptors.get(&item.descriptor.digest) {
                ensure!(
                    existing == &item.descriptor,
                    "artifact_digest_mismatch: one digest has inconsistent descriptors"
                );
            } else {
                descriptors.insert(item.descriptor.digest.clone(), item.descriptor.clone());
            }

            if item.descriptor.artifact_type_uri == COMPONENT_DESCRIPTOR_TYPE_URI {
                let bytes = self.verified_object_bytes(&item.descriptor).await?;
                let component = decode_component_artifact_payload(&item.descriptor, &bytes)
                    .map_err(|_| anyhow!("component descriptor object is malformed or invalid"))?;
                if let Some(expected) = item.component_expectation {
                    ensure!(
                        component.behavior.digest == expected.behavior_digest,
                        "component behavior digest differs from its AssemblyLock pin"
                    );
                    ensure!(
                        component.trust_class == expected.trust_class,
                        "component trust class differs from its AssemblyLock pin"
                    );
                }
                if expanded.insert(item.descriptor.digest.clone()) {
                    pending.push(ClosureItem {
                        descriptor: component.behavior,
                        depth: item.depth,
                        component_expectation: None,
                    });
                    pending.extend(component.protocol_artifacts.into_iter().map(|descriptor| {
                        ClosureItem {
                            descriptor,
                            depth: item.depth,
                            component_expectation: None,
                        }
                    }));
                    pending.extend(component.content_roots.into_iter().map(|descriptor| {
                        ClosureItem {
                            descriptor,
                            depth: item.depth,
                            component_expectation: None,
                        }
                    }));
                    pending.extend(component.surface_artifacts.into_iter().map(|descriptor| {
                        ClosureItem {
                            descriptor,
                            depth: item.depth,
                            component_expectation: None,
                        }
                    }));
                }
                continue;
            }

            if !expanded.insert(item.descriptor.digest.clone()) {
                continue;
            }
            match item.descriptor.artifact_type_uri.as_str() {
                WORK_REVISION_TYPE_URI => {
                    let (value, _) = self
                        .verified_artifact_model::<WorkRevision>(&item.descriptor, "work revision")
                        .await?;
                    pending.extend(value.referenced_artifacts().into_iter().map(|descriptor| {
                        ClosureItem {
                            descriptor: descriptor.clone(),
                            depth: item.depth,
                            component_expectation: None,
                        }
                    }));
                }
                ASSEMBLY_REVISION_TYPE_URI => {
                    let (value, _) = self
                        .verified_artifact_model::<AssemblyRevision>(
                            &item.descriptor,
                            "assembly revision",
                        )
                        .await?;
                    for node in &value.nodes {
                        let (descriptor, depth) = match &node.source {
                            AssemblyNodeSource::Component { component } => {
                                (component.clone(), item.depth)
                            }
                            AssemblyNodeSource::Assembly { assembly } => (
                                assembly.clone(),
                                item.depth
                                    .checked_add(1)
                                    .ok_or_else(|| anyhow!("assembly nesting depth overflow"))?,
                            ),
                        };
                        pending.push(ClosureItem {
                            descriptor,
                            depth,
                            component_expectation: None,
                        });
                        if let Some(configuration) = &node.configuration {
                            pending.push(ClosureItem {
                                descriptor: configuration.clone(),
                                depth: item.depth,
                                component_expectation: None,
                            });
                        }
                    }
                    for slot in &value.state_slots {
                        if let Some(schema) = &slot.schema_ref {
                            pending.push(ClosureItem {
                                descriptor: schema.clone(),
                                depth: item.depth,
                                component_expectation: None,
                            });
                        }
                    }
                    assemblies.insert(item.descriptor.digest, value);
                }
                ASSEMBLY_LOCK_TYPE_URI => {
                    let (value, _) = self
                        .verified_artifact_model::<AssemblyLock>(&item.descriptor, "assembly lock")
                        .await?;
                    pending.push(ClosureItem {
                        descriptor: value.assembly.clone(),
                        depth: item.depth,
                        component_expectation: None,
                    });
                    for node in &value.nodes {
                        let (depth, component_expectation) =
                            if node.artifact.artifact_type_uri == ASSEMBLY_LOCK_TYPE_URI {
                                (
                                    item.depth.checked_add(1).ok_or_else(|| {
                                        anyhow!("assembly lock nesting depth overflow")
                                    })?,
                                    None,
                                )
                            } else {
                                (
                                    item.depth,
                                    Some(ComponentExpectation {
                                        behavior_digest: node.behavior_digest.clone().ok_or_else(
                                            || anyhow!("component lock has no behavior digest"),
                                        )?,
                                        trust_class: node.trust_class.ok_or_else(|| {
                                            anyhow!("component lock has no trust class")
                                        })?,
                                    }),
                                )
                            };
                        pending.push(ClosureItem {
                            descriptor: node.artifact.clone(),
                            depth,
                            component_expectation,
                        });
                    }
                    pending.extend(value.bindings.iter().map(|binding| ClosureItem {
                        descriptor: binding.provider_component.clone(),
                        depth: item.depth,
                        component_expectation: None,
                    }));
                    pending.extend(value.content_roots.iter().cloned().map(|descriptor| {
                        ClosureItem {
                            descriptor,
                            depth: item.depth,
                            component_expectation: None,
                        }
                    }));
                    locks.insert(item.descriptor.digest, value);
                }
                RIGHTS_DECLARATION_TYPE_URI => {
                    let (value, _) = self
                        .verified_artifact_model::<RightsDeclaration>(
                            &item.descriptor,
                            "rights declaration",
                        )
                        .await?;
                    pending.extend(value.referenced_artifacts().into_iter().map(|descriptor| {
                        ClosureItem {
                            descriptor: descriptor.clone(),
                            depth: item.depth,
                            component_expectation: None,
                        }
                    }));
                }
                TRANSPARENCY_DECLARATION_TYPE_URI => {
                    let (value, _) = self
                        .verified_artifact_model::<TransparencyDeclaration>(
                            &item.descriptor,
                            "transparency declaration",
                        )
                        .await?;
                    pending.extend(value.referenced_artifacts().into_iter().map(|descriptor| {
                        ClosureItem {
                            descriptor: descriptor.clone(),
                            depth: item.depth,
                            component_expectation: None,
                        }
                    }));
                }
                OPERATIONAL_INTENT_TYPE_URI => {
                    let _ = self
                        .verified_artifact_model::<OperationalIntent>(
                            &item.descriptor,
                            "operational intent",
                        )
                        .await?;
                }
                _ => {
                    let _ = self.verified_object_bytes(&item.descriptor).await?;
                    for digest in &item.descriptor.references {
                        self.object_store
                            .verify(digest)
                            .await
                            .map_err(|_| anyhow!("referenced object is unavailable or corrupt"))?;
                    }
                }
            }
        }

        validate_assembly_closure(&work.assembly, &assemblies)
            .map_err(|_| anyhow!("assembly revision closure is incomplete or invalid"))?;
        validate_lock_closure(&locks, &assemblies)?;
        validate_assembly_port_catalogs(&work.assembly, &assemblies)?;
        let assembly = assemblies
            .get(&work.assembly.digest)
            .cloned()
            .ok_or_else(|| anyhow!("root AssemblyRevision is missing after closure validation"))?;
        Ok(VerifiedInstallationArtifacts {
            work,
            assembly,
            lock,
            lock_bytes,
            assemblies,
            locks,
        })
    }

    async fn verified_artifact_model<T>(
        &self,
        descriptor: &ArtifactDescriptor,
        label: &str,
    ) -> anyhow::Result<(T, Vec<u8>)>
    where
        T: ArtifactModel + for<'de> Deserialize<'de>,
    {
        ensure!(
            descriptor.artifact_type_uri == T::ARTIFACT_TYPE_URI,
            "{label} descriptor type is invalid"
        );
        let bytes = self.verified_object_bytes(descriptor).await?;
        let value: T =
            serde_json::from_slice(&bytes).map_err(|_| anyhow!("{label} object is malformed"))?;
        value
            .validate()
            .map_err(|_| anyhow!("{label} object is invalid"))?;
        ensure!(
            value
                .artifact_descriptor()
                .map_err(|_| anyhow!("{label} object is invalid"))?
                == *descriptor,
            "{label} descriptor does not match canonical object content"
        );
        Ok((value, bytes))
    }

    async fn verified_object_bytes(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> anyhow::Result<Vec<u8>> {
        validate_artifact_descriptor(descriptor)
            .map_err(|_| anyhow!("artifact descriptor is invalid"))?;
        let info = self
            .object_store
            .verify(&descriptor.digest)
            .await
            .map_err(|_| anyhow!("referenced object is unavailable or corrupt"))?;
        ensure!(
            info.size_bytes == descriptor.size_bytes,
            "referenced object size does not match its descriptor"
        );
        let bytes = self
            .object_store
            .get(&descriptor.digest)
            .await
            .map_err(|_| anyhow!("referenced object is unavailable or corrupt"))?;
        ensure!(
            u64::try_from(bytes.len()).ok() == Some(descriptor.size_bytes),
            "referenced object size does not match its descriptor"
        );
        Ok(bytes.to_vec())
    }

    async fn issue_state_decision_receipts(
        &self,
        request: &InstallationUpdateRequest,
    ) -> anyhow::Result<Vec<ArtifactDescriptor>> {
        let (receipt_type, receipt_schema, action, replacement_snapshot_digest) =
            match &request.state_action {
                InstallationStateAction::Preserve => return Ok(Vec::new()),
                InstallationStateAction::Replace {
                    replacement_snapshot,
                } => (
                    INSTALLATION_STATE_REPLACEMENT_DECISION_RECEIPT_TYPE_URI,
                    INSTALLATION_STATE_REPLACEMENT_DECISION_RECEIPT_SCHEMA,
                    InstallationStateDecisionAction::Replace,
                    Some(replacement_snapshot.digest.clone()),
                ),
                InstallationStateAction::Reset => (
                    INSTALLATION_STATE_RESET_RECEIPT_TYPE_URI,
                    INSTALLATION_STATE_RESET_RECEIPT_SCHEMA,
                    InstallationStateDecisionAction::Reset,
                    None,
                ),
            };
        let authority = request.authority.as_ref().ok_or_else(|| {
            anyhow!("authority_denied: current authority context is required for state mutation")
        })?;
        authority
            .refresh_current_for_installation(&request.installation_id)
            .await?;
        let evidence = InstallationStateAuthorityEvidence {
            schema: INSTALLATION_STATE_AUTHORITY_EVIDENCE_SCHEMA.to_string(),
            action: "installation.manage".to_string(),
            installation_id: request.installation_id.clone(),
            grant_id: authority.grant_id().map(str::to_owned),
            expires_at_ms: authority.expires_at_ms(),
        };
        let evidence = self
            .put_state_artifact(
                INSTALLATION_STATE_AUTHORITY_EVIDENCE_TYPE_URI,
                INSTALLATION_STATE_AUTHORITY_EVIDENCE_MEDIA_TYPE,
                Vec::new(),
                &evidence,
                "authority evidence",
            )
            .await?;

        authority
            .refresh_current_for_installation(&request.installation_id)
            .await?;
        let receipt = InstallationStateDecisionReceipt {
            schema: receipt_schema.to_string(),
            installation_id: request.installation_id.clone(),
            expected_revision: request.expected_revision,
            candidate_work_digest: request.work_revision.digest.clone(),
            candidate_lock_digest: request.assembly_lock.digest.clone(),
            replacement_snapshot_digest,
            operation: INSTALLATION_STATE_OPERATION.to_string(),
            action,
            decision: InstallationStateDecision::Allow,
            authority_evidence: vec![evidence.clone()],
        };
        let receipt = self
            .put_state_artifact(
                receipt_type,
                INSTALLATION_STATE_RECEIPT_MEDIA_TYPE,
                vec![evidence.digest.clone()],
                &receipt,
                "state decision receipt",
            )
            .await?;
        authority
            .refresh_current_for_installation(&request.installation_id)
            .await?;

        let mut receipts = vec![receipt, evidence];
        receipts.sort_by(|left, right| {
            left.digest
                .cmp(&right.digest)
                .then(left.artifact_type_uri.cmp(&right.artifact_type_uri))
        });
        receipts.dedup();
        validate_state_receipt_descriptors(&receipts)?;
        Ok(receipts)
    }

    async fn put_state_artifact<T: Serialize>(
        &self,
        artifact_type_uri: &str,
        media_type: &str,
        mut references: Vec<String>,
        value: &T,
        label: &str,
    ) -> anyhow::Result<ArtifactDescriptor> {
        references.sort();
        references.dedup();
        let bytes = canonical_json_bytes(value).map_err(|_| anyhow!("encode {label} failed"))?;
        let info = self
            .object_store
            .put(bytes.into())
            .await
            .map_err(|_| anyhow!("store {label} failed"))?;
        let descriptor = ArtifactDescriptor {
            artifact_type_uri: artifact_type_uri.to_string(),
            media_type: media_type.to_string(),
            digest: info.digest,
            size_bytes: info.size_bytes,
            references,
            annotations: BTreeMap::new(),
        };
        validate_artifact_descriptor(&descriptor).map_err(|_| anyhow!("{label} is invalid"))?;
        Ok(descriptor)
    }

    async fn refresh_create_authority(
        &self,
        request: &InstallationCreateRequest,
    ) -> anyhow::Result<()> {
        request
            .authority
            .as_ref()
            .ok_or_else(|| {
                anyhow!(
                    "authority_denied: current exact Work authority is required for Installation creation"
                )
            })?
            .refresh_current_for_work(&request.work_id)
            .await
    }

    async fn refresh_update_authority(
        &self,
        request: &InstallationUpdateRequest,
    ) -> anyhow::Result<()> {
        request
            .authority
            .as_ref()
            .ok_or_else(|| {
                anyhow!(
                    "authority_denied: current exact Installation authority is required for update"
                )
            })?
            .refresh_current_for_installation(&request.installation_id)
            .await
    }

    async fn refresh_remove_authority(
        &self,
        request: &InstallationRemoveRequest,
    ) -> anyhow::Result<()> {
        request
            .authority
            .as_ref()
            .ok_or_else(|| {
                anyhow!(
                    "authority_denied: current authority context is required for Installation removal"
                )
            })?
            .refresh_current_for_installation(&request.installation_id)
            .await
    }

    fn installation_dir(&self, installation_id: &InstallationId) -> PathBuf {
        self.data_root
            .join("installations")
            .join(installation_id.as_str())
    }

    #[cfg(test)]
    fn state_dir(&self, installation_id: &InstallationId) -> PathBuf {
        self.installation_dir(installation_id).join("state")
    }

    fn prepare_installation_tree(
        &self,
        installation_id: &InstallationId,
    ) -> anyhow::Result<AnchoredDirectory> {
        let installations = self.data_root.join("installations");
        prepare_existing_real_directory(&self.data_root, "installation data root")?;
        prepare_existing_real_directory(&installations, "installations root")?;
        ensure!(
            same_file::Handle::from_path(&self.data_root)
                .map_err(|_| anyhow!("identify installation data root failed"))?
                == self.data_root_anchor,
            "installation data root identity changed"
        );
        ensure!(
            same_file::Handle::from_path(&installations)
                .map_err(|_| anyhow!("identify installations root failed"))?
                == self.installations_root_anchor,
            "installations root identity changed"
        );
        ensure_contained(&self.data_root, &installations, "installations root")?;
        let installations_anchor = AnchoredDirectory {
            path: installations,
            handle: open_anchored_directory(
                &self.data_root.join("installations"),
                "installations root",
            )?
            .handle,
        };
        let installations_directory = installations_anchor.capability("installations root")?;
        let installation_name = Path::new(installation_id.as_str());
        prepare_capability_directory(
            &installations_directory,
            installation_name,
            "installation projection root",
        )?;
        let installation_directory = installations_directory
            .open_dir(installation_name)
            .map_err(|_| anyhow!("open installation projection root failed"))?;
        let installation_handle =
            capability_directory_handle(&installation_directory, "installation projection root")?;
        let mut anchored = open_anchored_directory(
            &self.installation_dir(installation_id),
            "installation projection root",
        )?;
        ensure!(
            anchored.handle == installation_handle,
            "installation projection root changed while its capability opened"
        );
        anchored.path = self.installation_dir(installation_id);
        anchored.ensure_current_path("installation projection root")?;
        let installation_directory = anchored.capability("installation projection root")?;
        for child in ["state", "diagnostics"] {
            prepare_capability_directory(
                &installation_directory,
                Path::new(child),
                "installation projection directory",
            )?;
        }
        match installation_directory.symlink_metadata("secrets.dat") {
            Ok(metadata) => ensure!(
                metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
                "installation secret store is not a regular file"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => bail!("inspect installation secret store failed"),
        }
        Ok(anchored)
    }

    async fn materialize_projection(&self, view: &InstallationView) -> anyhow::Result<()> {
        let prepared = self.prepare_projection(view).await?;
        self.publish_projection(prepared).await
    }

    async fn prepare_projection(
        &self,
        view: &InstallationView,
    ) -> anyhow::Result<PreparedProjection> {
        #[cfg(test)]
        if self.take_projection_failure(ProjectionFailure::Prepare)? {
            bail!("injected projection preparation failure");
        }
        validate_view(view)?;
        self.ensure_owner_lease().await?;
        let installation = self.prepare_installation_tree(&view.record.installation_id)?;
        self.cleanup_state_temporaries(&installation)?;
        let verified = self
            .verify_work_and_lock(&view.record.work_revision, &view.record.assembly_lock)
            .await?;
        ensure!(
            view.work_summary == InstallationWorkSummary::from_work_revision(&verified.work),
            "installation Work summary differs from its exact verified WorkRevision"
        );
        let record_bytes = canonical_json_bytes(view).context("encode installation projection")?;
        self.ensure_owner_lease().await?;
        let record_replacement =
            prepare_atomic_replacement(&installation, "installation.json", &record_bytes)?;
        let lock_replacement =
            prepare_atomic_replacement(&installation, "assembly.lock.json", &verified.lock_bytes)?;
        Ok(PreparedProjection {
            _installation: installation,
            record_replacement,
            lock_replacement,
        })
    }

    fn discard_uncommitted_create_projection(
        &self,
        prepared: PreparedProjection,
    ) -> anyhow::Result<()> {
        let installation_name = prepared
            ._installation
            .path
            .file_name()
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("Installation projection has no directory name"))?;
        drop(prepared);
        let owner =
            open_anchored_directory(&self.data_root.join("installations"), "installations root")?;
        ensure!(
            owner.handle == self.installations_root_anchor,
            "installations root identity changed"
        );
        remove_safe_tree(&owner, &installation_name)
            .map_err(|error| anyhow!("discard uncommitted Installation projection failed: {error}"))
    }

    async fn publish_projection(&self, prepared: PreparedProjection) -> anyhow::Result<()> {
        #[cfg(test)]
        if self.take_projection_failure(ProjectionFailure::Publish)? {
            bail!("injected projection publication failure");
        }
        self.ensure_owner_lease().await?;
        prepared.publish()
    }

    async fn publish_committed_projection(
        &self,
        view: &InstallationView,
        prepared: PreparedProjection,
    ) {
        if let Err(error) = self.publish_projection(prepared).await {
            tracing::error!(
                target: "plurora_service::installations",
                installation_id = %view.record.installation_id,
                revision = view.revision,
                committed = true,
                repair = "rehydrate_installation_projection",
                %error,
                "installation mutation committed but projection publication failed"
            );
        }
    }

    #[cfg(test)]
    fn inject_projection_failure(&self, failure: ProjectionFailure) -> anyhow::Result<()> {
        *self.projection_failure.lock().map_err(lock_error)? = Some(failure);
        Ok(())
    }

    #[cfg(test)]
    fn take_projection_failure(&self, expected: ProjectionFailure) -> anyhow::Result<bool> {
        let mut failure = self.projection_failure.lock().map_err(lock_error)?;
        if failure.as_ref() == Some(&expected) {
            *failure = None;
            return Ok(true);
        }
        Ok(false)
    }

    #[cfg(test)]
    fn inject_authority_append_barrier(
        &self,
        kind: &str,
        entered: Arc<tokio::sync::Barrier>,
        release: Arc<tokio::sync::Barrier>,
    ) -> anyhow::Result<()> {
        *self.authority_append_barrier.lock().map_err(lock_error)? = Some(AuthorityAppendBarrier {
            kind: kind.to_string(),
            entered,
            release,
        });
        Ok(())
    }

    #[cfg(test)]
    async fn wait_authority_append_barrier(&self, kind: &str) -> anyhow::Result<()> {
        let barrier = {
            let mut pending = self.authority_append_barrier.lock().map_err(lock_error)?;
            if !pending.as_ref().is_some_and(|barrier| barrier.kind == kind) {
                return Ok(());
            }
            pending.take().expect("checked authority append barrier")
        };
        barrier.entered.wait().await;
        barrier.release.wait().await;
        Ok(())
    }

    fn cleanup_state_temporaries(&self, installation: &AnchoredDirectory) -> anyhow::Result<()> {
        let directory = installation.capability("installation projection root")?;
        for entry in directory
            .entries()
            .map_err(|_| anyhow!("read installation projection root failed"))?
        {
            let entry = entry.map_err(|_| anyhow!("read installation projection root failed"))?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with(".state-staging-") || name.starts_with(".state-backup-") {
                remove_safe_tree(installation, Path::new(name))?;
            }
        }
        Ok(())
    }

    fn state_has_entries(&self, installation_id: &InstallationId) -> anyhow::Result<bool> {
        let installation = self.prepare_installation_tree(installation_id)?;
        Ok(!read_state_entries(&installation)?.is_empty())
    }

    async fn snapshot_state(
        &self,
        installation_id: &InstallationId,
    ) -> anyhow::Result<ArtifactDescriptor> {
        self.ensure_owner_lease().await?;
        let installation = self.prepare_installation_tree(installation_id)?;
        let snapshot = StateSnapshot {
            schema: INSTALLATION_STATE_SNAPSHOT_SCHEMA.to_string(),
            entries: read_state_entries(&installation)?,
        };
        let bytes = snapshot
            .canonical_bytes()
            .context("encode installation state snapshot")?;
        self.ensure_owner_lease().await?;
        let info = self
            .object_store
            .put(bytes.clone().into())
            .await
            .map_err(|_| anyhow!("store installation state snapshot failed"))?;
        Ok(ArtifactDescriptor {
            artifact_type_uri: INSTALLATION_STATE_SNAPSHOT_TYPE_URI.to_string(),
            media_type: INSTALLATION_STATE_SNAPSHOT_MEDIA_TYPE.to_string(),
            digest: info.digest,
            size_bytes: info.size_bytes,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        })
    }

    async fn load_snapshot(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> anyhow::Result<StateSnapshot> {
        ensure!(
            descriptor.artifact_type_uri == INSTALLATION_STATE_SNAPSHOT_TYPE_URI
                && descriptor.media_type == INSTALLATION_STATE_SNAPSHOT_MEDIA_TYPE
                && descriptor.references.is_empty(),
            "state snapshot uses an unsupported type"
        );
        let bytes = self.verified_object_bytes(descriptor).await?;
        let snapshot: StateSnapshot =
            serde_json::from_slice(&bytes).map_err(|_| anyhow!("state snapshot is malformed"))?;
        ensure!(
            snapshot
                .canonical_bytes()
                .map_err(|_| anyhow!("state snapshot is invalid"))?
                == bytes,
            "state snapshot is not canonical"
        );
        Ok(snapshot)
    }

    async fn restore_state(
        &self,
        installation_id: &InstallationId,
        descriptor: &ArtifactDescriptor,
    ) -> anyhow::Result<()> {
        let snapshot = self.load_snapshot(descriptor).await?;
        self.replace_state(installation_id, &snapshot).await
    }

    async fn replace_state(
        &self,
        installation_id: &InstallationId,
        snapshot: &StateSnapshot,
    ) -> anyhow::Result<()> {
        snapshot.validate()?;
        self.ensure_owner_lease().await?;
        self.replace_state_effect(installation_id, snapshot)
    }

    fn replace_state_effect(
        &self,
        installation_id: &InstallationId,
        snapshot: &StateSnapshot,
    ) -> anyhow::Result<()> {
        let installation = self.prepare_installation_tree(installation_id)?;
        #[cfg(all(test, unix))]
        apply_installation_ancestor_swap(&installation.path)?;
        installation.ensure_current_path("installation projection root")?;
        let directory = installation.capability("installation projection root")?;
        let state = PathBuf::from("state");
        #[cfg(not(windows))]
        let state_directory = directory
            .open_dir(&state)
            .map_err(|_| anyhow!("open installation state root failed"))?;
        #[cfg(not(windows))]
        let state_handle =
            capability_directory_handle(&state_directory, "installation state root")?;
        #[cfg(windows)]
        let state_identity = {
            let metadata = directory
                .symlink_metadata(&state)
                .map_err(|_| anyhow!("inspect installation state root failed"))?;
            ensure!(
                metadata.file_type().is_dir() && !metadata.file_type().is_symlink(),
                "installation state root is not a real directory"
            );
            let state_directory = directory
                .open_dir(&state)
                .map_err(|_| anyhow!("open installation state root failed"))?;
            windows_directory_identity(&state_directory, "installation state root")?
        };
        let nonce = uuid::Uuid::new_v4();
        let staging = PathBuf::from(format!(".state-staging-{nonce}"));
        let backup = PathBuf::from(format!(".state-backup-{nonce}"));
        prepare_new_capability_directory(&directory, &staging, "state staging directory")?;
        let staging_directory = directory
            .open_dir(&staging)
            .map_err(|_| anyhow!("open state staging directory failed"))?;
        #[cfg(not(windows))]
        let staging_handle =
            capability_directory_handle(&staging_directory, "state staging directory")?;
        #[cfg(windows)]
        let staging_identity =
            windows_directory_identity(&staging_directory, "state staging directory")?;
        if let Err(error) = write_snapshot_tree(&staging_directory, snapshot) {
            let _ = remove_safe_tree(&installation, &staging);
            return Err(error);
        }
        #[cfg(windows)]
        drop(staging_directory);
        directory
            .rename(&state, &directory, &backup)
            .map_err(|_| anyhow!("replace installation state failed"))?;
        #[cfg(not(windows))]
        if !capability_path_matches_directory(&directory, &backup, &state_handle)? {
            let _ = directory.rename(&backup, &directory, &state);
            let _ = remove_safe_tree(&installation, &staging);
            bail!("replace installation state failed: active state identity changed");
        }
        #[cfg(windows)]
        if !windows_path_matches_directory(&directory, &backup, state_identity)? {
            let _ = directory.rename(&backup, &directory, &state);
            let _ = remove_safe_tree(&installation, &staging);
            bail!("replace installation state failed: active state identity changed");
        }
        if directory.rename(&staging, &directory, &state).is_err() {
            let _ = directory.rename(&backup, &directory, &state);
            let _ = remove_safe_tree(&installation, &staging);
            bail!("replace installation state failed");
        }
        #[cfg(not(windows))]
        if !capability_path_matches_directory(&directory, &state, &staging_handle)? {
            bail!("replace installation state failed: staged state identity changed");
        }
        #[cfg(windows)]
        if !windows_path_matches_directory(&directory, &state, staging_identity)? {
            bail!("replace installation state failed: staged state identity changed");
        }
        remove_safe_tree(&installation, &backup)
            .map_err(|error| anyhow!("installation state cleanup failed: {error}"))?;
        Ok(())
    }

    async fn rollback_started(
        &self,
        pending: &PendingMutation,
        reason_code: &str,
    ) -> anyhow::Result<()> {
        let id = pending.previous.record.installation_id.clone();
        if self
            .restore_state(&id, &pending.state_snapshot)
            .await
            .is_err()
        {
            let blocked = BlockedPayload {
                installation_id: id,
                operation_id: pending.operation_id.clone(),
                reason_code: "state_restore_failed".to_string(),
            };
            self.append_pending_blocked(&blocked).await?;
            bail!("installation rollback blocked: state_restore_failed");
        }
        let rollback = RollbackPayload {
            installation_id: id,
            operation_id: pending.operation_id.clone(),
            previous: pending.previous.clone(),
            reason_code: reason_code.to_string(),
        };
        let kind = match pending.kind {
            PendingKind::Update => UPDATE_ROLLBACK,
            PendingKind::Remove => REMOVE_ROLLBACK,
        };
        self.append_pending_rollback(kind, &rollback).await
    }

    async fn rollback_started_without_state_effect(
        &self,
        pending: &PendingMutation,
        reason_code: &str,
    ) -> anyhow::Result<()> {
        let rollback = RollbackPayload {
            installation_id: pending.previous.record.installation_id.clone(),
            operation_id: pending.operation_id.clone(),
            previous: pending.previous.clone(),
            reason_code: reason_code.to_string(),
        };
        let kind = match pending.kind {
            PendingKind::Update => UPDATE_ROLLBACK,
            PendingKind::Remove => REMOVE_ROLLBACK,
        };
        self.append_pending_rollback(kind, &rollback).await
    }

    async fn persist_noop_claim(
        &self,
        claim: IdempotencyClaim,
        authority: &InstallationMutationAuthority,
    ) -> anyhow::Result<()> {
        self.before_authority_append(IDEMPOTENT_NOOP).await?;
        for _ in 0..CAS_ATTEMPTS {
            let _apply = self.apply.lock().await;
            self.sync_journal_locked().await?;
            if let Some(existing) = self
                .idempotency
                .read()
                .map_err(lock_error)?
                .get(&claim.key_hash)
                .cloned()
            {
                ensure!(
                    existing == claim,
                    "idempotency claim was resolved inconsistently"
                );
                return Ok(());
            }
            let installation = &claim.result.installation;
            ensure!(
                self.views
                    .read()
                    .map_err(lock_error)?
                    .get(&installation.record.installation_id)
                    == Some(installation),
                "revision_conflict: no-op mutation base changed before commit"
            );
            ensure!(
                !self
                    .pending
                    .read()
                    .map_err(lock_error)?
                    .contains_key(&installation.record.installation_id),
                "installation has a pending mutation"
            );
            if self
                .append_payload_with_installation_authority_locked(
                    IDEMPOTENT_NOOP,
                    &NoopPayload {
                        claim: claim.clone(),
                    },
                    &installation.record.installation_id,
                    authority,
                )
                .await?
            {
                return Ok(());
            }
        }
        bail!("installation no-op contention did not converge")
    }
}

#[async_trait]
impl InstallationControl for InstallationRegistry {
    async fn list(
        &self,
        request: InstallationListRequest,
    ) -> anyhow::Result<Vec<InstallationView>> {
        let mut values = self
            .views
            .read()
            .map_err(lock_error)?
            .values()
            .filter(|view| request.status.is_none() || request.status == Some(view.record.status))
            .cloned()
            .collect::<Vec<_>>();
        values.sort_by(|left, right| {
            left.record.created_at.cmp(&right.record.created_at).then(
                left.record
                    .installation_id
                    .cmp(&right.record.installation_id),
            )
        });
        Ok(values)
    }

    async fn get(
        &self,
        installation_id: &InstallationId,
    ) -> anyhow::Result<Option<InstallationView>> {
        Ok(self
            .views
            .read()
            .map_err(lock_error)?
            .get(installation_id)
            .cloned())
    }

    async fn create(
        &self,
        request: InstallationCreateRequest,
    ) -> anyhow::Result<InstallationMutationResult> {
        request.validate()?;
        self.ensure_owner_lease().await?;
        let key_hash = hash_bytes(request.idempotency_key.as_bytes());
        let fingerprint = mutation_fingerprint("create", &request)?;
        {
            let _apply = self.apply.lock().await;
            self.sync_journal_locked().await?;
            self.refresh_create_authority(&request).await?;
            if let Some(result) = self.claimed_result(&key_hash, &fingerprint)? {
                return Ok(result);
            }
        }
        self.refresh_create_authority(&request).await?;
        let verified = self
            .verify_work_and_lock(&request.work_revision, &request.assembly_lock)
            .await?;
        ensure!(
            verified.work.work_id == request.work_id,
            "work_id_mismatch: requested Work does not match canonical WorkRevision content"
        );
        self.refresh_create_authority(&request).await?;

        // Allocate one candidate identity only after request authority and the
        // complete immutable closure have validated. CAS retries keep this same
        // identity; a competing idempotency winner discards its uncommitted tree.
        let installation_id = InstallationId::new();
        let _lifecycle = self.lifecycle_lock(&installation_id)?.write_owned().await;
        let now = Utc::now();
        let record = InstallationRecord {
            schema_version: InstallationRecord::SCHEMA_VERSION,
            installation_id: installation_id.clone(),
            work_revision: request.work_revision.clone(),
            assembly_lock: request.assembly_lock.clone(),
            display_name: request.display_name.clone(),
            source: request.source.clone(),
            state_bindings: request.state_bindings.clone(),
            secret_policy: request.secret_policy.clone(),
            created_at: now,
            updated_at: now,
            status: InstallationStatus::Ready,
        };
        record
            .validate()
            .map_err(|_| anyhow!("installation record is invalid"))?;
        let view = InstallationView {
            record,
            work_summary: InstallationWorkSummary::from_work_revision(&verified.work),
            revision: 1,
            rollback: None,
        };
        let result = InstallationMutationResult {
            installation: view.clone(),
            diff: None,
            receipts: Vec::new(),
            idempotent: false,
        };
        let claim = IdempotencyClaim {
            key_hash: key_hash.clone(),
            fingerprint: fingerprint.clone(),
            result: result.clone(),
        };
        self.refresh_create_authority(&request).await?;
        let projection = self.prepare_projection(&result.installation).await?;
        self.refresh_create_authority(&request).await?;
        let authority = request
            .authority
            .as_ref()
            .ok_or_else(|| anyhow!("authority_denied: trusted Work authority is required"))?;
        if let Err(error) = self.before_authority_append(INSTALLATION_CREATED).await {
            self.discard_uncommitted_create_projection(projection)?;
            return Err(error);
        }

        enum CreateCommit {
            Appended,
            Existing(InstallationMutationResult),
            Contended,
        }
        let commit = async {
            for _ in 0..CAS_ATTEMPTS {
                let _apply = self.apply.lock().await;
                self.sync_journal_locked().await?;
                self.refresh_create_authority(&request).await?;
                if let Some(result) = self.claimed_result(&key_hash, &fingerprint)? {
                    return Ok::<CreateCommit, anyhow::Error>(CreateCommit::Existing(result));
                }
                ensure!(
                    !self
                        .views
                        .read()
                        .map_err(lock_error)?
                        .contains_key(&installation_id),
                    "installation identity already exists"
                );
                if self
                    .append_payload_with_work_authority_locked(
                        INSTALLATION_CREATED,
                        &CreatedPayload {
                            view: view.clone(),
                            claim: claim.clone(),
                        },
                        &request.work_id,
                        authority,
                    )
                    .await?
                {
                    return Ok(CreateCommit::Appended);
                }
            }
            Ok(CreateCommit::Contended)
        }
        .await;
        match commit {
            Ok(CreateCommit::Appended) => {
                self.publish_committed_projection(&result.installation, projection)
                    .await;
                Ok(result)
            }
            Ok(CreateCommit::Existing(existing)) => {
                self.discard_uncommitted_create_projection(projection)?;
                Ok(existing)
            }
            Ok(CreateCommit::Contended) => {
                self.discard_uncommitted_create_projection(projection)?;
                bail!("installation create contention did not converge")
            }
            Err(error) => {
                self.discard_uncommitted_create_projection(projection)?;
                Err(error)
            }
        }
    }

    async fn update(
        &self,
        request: InstallationUpdateRequest,
    ) -> anyhow::Result<InstallationMutationResult> {
        request.validate()?;
        self.ensure_owner_lease().await?;
        let key_hash = hash_bytes(request.idempotency_key.as_bytes());
        let fingerprint = mutation_fingerprint("update", &request)?;
        let _lifecycle = self
            .lifecycle_lock(&request.installation_id)?
            .write_owned()
            .await;
        let previous = {
            let _apply = self.apply.lock().await;
            self.sync_journal_locked().await?;
            self.refresh_update_authority(&request).await?;
            if let Some(result) = self.claimed_result(&key_hash, &fingerprint)? {
                return Ok(result);
            }
            let previous = self
                .views
                .read()
                .map_err(lock_error)?
                .get(&request.installation_id)
                .cloned()
                .ok_or_else(|| anyhow!("installation_not_found"))?;
            ensure!(
                previous.record.status == InstallationStatus::Ready,
                "installation is not ready for update"
            );
            ensure!(
                previous.revision == request.expected_revision,
                "revision_conflict: installation revision is stale"
            );
            ensure!(
                !self
                    .pending
                    .read()
                    .map_err(lock_error)?
                    .contains_key(&request.installation_id),
                "installation has a pending mutation"
            );
            previous
        };

        self.refresh_update_authority(&request).await?;
        let candidate_artifacts = self
            .verify_work_and_lock(&request.work_revision, &request.assembly_lock)
            .await?;
        self.refresh_update_authority(&request).await?;
        let replacement = match &request.state_action {
            InstallationStateAction::Preserve => None,
            InstallationStateAction::Replace {
                replacement_snapshot,
            } => Some(self.load_snapshot(replacement_snapshot).await?),
            InstallationStateAction::Reset => Some(StateSnapshot::empty()),
        };
        self.refresh_update_authority(&request).await?;
        let current_artifacts = self
            .verify_work_and_lock(
                &previous.record.work_revision,
                &previous.record.assembly_lock,
            )
            .await?;
        self.refresh_update_authority(&request).await?;
        let candidate = update_candidate(&previous.record, &request);
        candidate
            .validate()
            .map_err(|_| anyhow!("installation update is invalid"))?;
        let diff = installation_diff(
            &previous.record,
            &candidate,
            &current_artifacts,
            &candidate_artifacts,
        )?;
        if diff_is_empty(&diff)
            && matches!(&request.state_action, InstallationStateAction::Preserve)
        {
            let result = InstallationMutationResult {
                installation: previous,
                diff: Some(diff),
                receipts: Vec::new(),
                idempotent: true,
            };
            let claim = IdempotencyClaim {
                key_hash,
                fingerprint,
                result: result.clone(),
            };
            self.refresh_update_authority(&request).await?;
            let authority = request.authority.as_ref().ok_or_else(|| {
                anyhow!("authority_denied: trusted Installation authority is required")
            })?;
            self.persist_noop_claim(claim, authority).await?;
            return Ok(result);
        }

        validate_update_state_action(
            &request.state_action,
            &diff.state_slots,
            self.state_has_entries(&request.installation_id)?,
        )?;
        self.refresh_update_authority(&request).await?;

        let state_change = !matches!(request.state_action, InstallationStateAction::Preserve);
        let operation_id = state_change.then(|| uuid::Uuid::new_v4().to_string());
        let state_snapshot = if state_change {
            self.refresh_update_authority(&request).await?;
            let snapshot = self.snapshot_state(&request.installation_id).await?;
            self.refresh_update_authority(&request).await?;
            Some(snapshot)
        } else {
            None
        };
        let receipts = if state_change {
            self.issue_state_decision_receipts(&request).await?
        } else {
            Vec::new()
        };
        let rollback = InstallationRollbackPointer {
            revision: previous.revision,
            work_revision: previous.record.work_revision.clone(),
            assembly_lock: previous.record.assembly_lock.clone(),
            state_snapshot: state_snapshot.clone(),
        };
        let mut new_view = InstallationView {
            record: candidate,
            work_summary: InstallationWorkSummary::from_work_revision(&candidate_artifacts.work),
            revision: previous
                .revision
                .checked_add(1)
                .ok_or_else(|| anyhow!("installation revision overflow"))?,
            rollback: Some(rollback),
        };
        new_view.record.status = InstallationStatus::Ready;

        let result = InstallationMutationResult {
            installation: new_view.clone(),
            diff: Some(diff),
            receipts: receipts.clone(),
            idempotent: false,
        };
        let claim = IdempotencyClaim {
            key_hash: key_hash.clone(),
            fingerprint: fingerprint.clone(),
            result: result.clone(),
        };
        self.refresh_update_authority(&request).await?;
        let projection = self.prepare_projection(&result.installation).await?;
        self.refresh_update_authority(&request).await?;
        let authority = request.authority.as_ref().ok_or_else(|| {
            anyhow!("authority_denied: trusted Installation authority is required")
        })?;

        if let Some(operation_id) = operation_id {
            let pending = PendingMutation {
                operation_id: operation_id.clone(),
                kind: PendingKind::Update,
                previous: previous.clone(),
                state_snapshot: state_snapshot.expect("state-changing update has snapshot"),
                receipts,
            };
            self.before_authority_append(UPDATE_STARTED).await?;
            let mut started = false;
            for _ in 0..CAS_ATTEMPTS {
                let _apply = self.apply.lock().await;
                self.sync_journal_locked().await?;
                if let Some(existing) = self.claimed_result(&key_hash, &fingerprint)? {
                    return Ok(existing);
                }
                let current = self
                    .views
                    .read()
                    .map_err(lock_error)?
                    .get(&request.installation_id)
                    .cloned()
                    .ok_or_else(|| anyhow!("installation_not_found"))?;
                ensure!(
                    current == previous,
                    "revision_conflict: installation changed before update start"
                );
                ensure!(
                    !self
                        .pending
                        .read()
                        .map_err(lock_error)?
                        .contains_key(&request.installation_id),
                    "installation has a pending mutation"
                );
                if self
                    .append_payload_with_installation_authority_locked(
                        UPDATE_STARTED,
                        &StartedPayload {
                            pending: pending.clone(),
                        },
                        &request.installation_id,
                        authority,
                    )
                    .await?
                {
                    started = true;
                    break;
                }
            }
            if !started {
                bail!("installation update start contention did not converge");
            }

            let replacement = replacement
                .as_ref()
                .expect("state-changing update has a validated replacement snapshot");
            self.ensure_owner_lease().await?;
            if let Err(error) = self.refresh_update_authority(&request).await {
                self.rollback_started_without_state_effect(&pending, "authority_expired")
                    .await?;
                return Err(error);
            }
            if let Err(error) = self.replace_state_effect(&request.installation_id, replacement) {
                self.rollback_started(&pending, "state_replace_failed")
                    .await?;
                return Err(error);
            }
            let payload = UpdatedPayload {
                operation_id: Some(operation_id),
                previous_revision: previous.revision,
                view: new_view,
                claim,
            };
            self.before_authority_append(INSTALLATION_UPDATED).await?;
            let terminal = async {
                for _ in 0..CAS_ATTEMPTS {
                    let _apply = self.apply.lock().await;
                    self.sync_journal_locked().await?;
                    let journal_pending = self
                        .pending
                        .read()
                        .map_err(lock_error)?
                        .get(&request.installation_id)
                        .cloned();
                    ensure!(
                        journal_pending.as_ref() == Some(&pending),
                        "terminal update no longer owns its pending operation"
                    );
                    let mut transitional = previous.clone();
                    transitional.record.status = InstallationStatus::Updating;
                    ensure!(
                        self.views
                            .read()
                            .map_err(lock_error)?
                            .get(&request.installation_id)
                            == Some(&transitional),
                        "terminal update base changed after state effect"
                    );
                    if self
                        .append_payload_with_installation_authority_locked(
                            INSTALLATION_UPDATED,
                            &payload,
                            &request.installation_id,
                            authority,
                        )
                        .await?
                    {
                        return Ok::<bool, anyhow::Error>(true);
                    }
                }
                Ok(false)
            }
            .await;
            match terminal {
                Ok(true) => {}
                Ok(false) => {
                    self.rollback_started(&pending, "terminal_append_conflict")
                        .await?;
                    bail!("installation update terminal contention did not converge");
                }
                Err(error) => {
                    self.rollback_started(&pending, "terminal_append_failed_after_state_effect")
                        .await?;
                    return Err(error);
                }
            }
            self.publish_committed_projection(&result.installation, projection)
                .await;
            return Ok(result);
        }

        let payload = UpdatedPayload {
            operation_id: None,
            previous_revision: previous.revision,
            view: new_view,
            claim,
        };
        self.before_authority_append(INSTALLATION_UPDATED).await?;
        for _ in 0..CAS_ATTEMPTS {
            let _apply = self.apply.lock().await;
            self.sync_journal_locked().await?;
            if let Some(existing) = self.claimed_result(&key_hash, &fingerprint)? {
                return Ok(existing);
            }
            ensure!(
                self.views
                    .read()
                    .map_err(lock_error)?
                    .get(&request.installation_id)
                    == Some(&previous),
                "revision_conflict: installation changed before update commit"
            );
            ensure!(
                !self
                    .pending
                    .read()
                    .map_err(lock_error)?
                    .contains_key(&request.installation_id),
                "installation has a pending mutation"
            );
            if self
                .append_payload_with_installation_authority_locked(
                    INSTALLATION_UPDATED,
                    &payload,
                    &request.installation_id,
                    authority,
                )
                .await?
            {
                drop(_apply);
                self.publish_committed_projection(&result.installation, projection)
                    .await;
                return Ok(result);
            }
        }
        bail!("installation update contention did not converge")
    }

    async fn remove(
        &self,
        request: InstallationRemoveRequest,
    ) -> anyhow::Result<InstallationMutationResult> {
        request.validate()?;
        self.ensure_owner_lease().await?;
        let key_hash = hash_bytes(request.idempotency_key.as_bytes());
        let fingerprint = mutation_fingerprint("remove", &request)?;
        let _lifecycle = self
            .lifecycle_lock(&request.installation_id)?
            .write_owned()
            .await;
        let previous = {
            let _apply = self.apply.lock().await;
            self.sync_journal_locked().await?;
            self.refresh_remove_authority(&request).await?;
            if let Some(result) = self.claimed_result(&key_hash, &fingerprint)? {
                return Ok(result);
            }
            let previous = self
                .views
                .read()
                .map_err(lock_error)?
                .get(&request.installation_id)
                .cloned()
                .ok_or_else(|| anyhow!("installation_not_found"))?;
            previous
        };

        if previous.record.status == InstallationStatus::Removed {
            let result = InstallationMutationResult {
                installation: previous,
                diff: None,
                receipts: Vec::new(),
                idempotent: true,
            };
            let authority = request.authority.as_ref().ok_or_else(|| {
                anyhow!("authority_denied: trusted Installation authority is required")
            })?;
            self.persist_noop_claim(
                IdempotencyClaim {
                    key_hash,
                    fingerprint,
                    result: result.clone(),
                },
                authority,
            )
            .await?;
            return Ok(result);
        }
        ensure!(
            previous.record.status == InstallationStatus::Ready,
            "installation is not ready for removal"
        );
        ensure!(
            previous.revision == request.expected_revision,
            "revision_conflict: installation revision is stale"
        );
        ensure!(
            !self
                .pending
                .read()
                .map_err(lock_error)?
                .contains_key(&request.installation_id),
            "installation has a pending mutation"
        );

        let mut removed = previous.clone();
        removed.revision = removed
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow!("installation revision overflow"))?;
        removed.record.status = InstallationStatus::Removed;
        removed.record.updated_at = Utc::now();
        let result = InstallationMutationResult {
            installation: removed.clone(),
            diff: None,
            receipts: Vec::new(),
            idempotent: false,
        };
        let claim = IdempotencyClaim {
            key_hash: key_hash.clone(),
            fingerprint: fingerprint.clone(),
            result: result.clone(),
        };
        self.refresh_remove_authority(&request).await?;
        let projection = self.prepare_projection(&result.installation).await?;
        self.refresh_remove_authority(&request).await?;
        let authority = request.authority.as_ref().ok_or_else(|| {
            anyhow!("authority_denied: trusted Installation authority is required")
        })?;

        if request.state_disposition == StateDisposition::Keep {
            let payload = RemovedPayload {
                operation_id: None,
                previous_revision: previous.revision,
                view: removed,
                claim,
            };
            self.before_authority_append(INSTALLATION_REMOVED).await?;
            for _ in 0..CAS_ATTEMPTS {
                let _apply = self.apply.lock().await;
                self.sync_journal_locked().await?;
                if let Some(existing) = self.claimed_result(&key_hash, &fingerprint)? {
                    return Ok(existing);
                }
                ensure!(
                    self.views
                        .read()
                        .map_err(lock_error)?
                        .get(&request.installation_id)
                        == Some(&previous),
                    "revision_conflict: installation changed before remove commit"
                );
                ensure!(
                    !self
                        .pending
                        .read()
                        .map_err(lock_error)?
                        .contains_key(&request.installation_id),
                    "installation has a pending mutation"
                );
                if self
                    .append_payload_with_installation_authority_locked(
                        INSTALLATION_REMOVED,
                        &payload,
                        &request.installation_id,
                        authority,
                    )
                    .await?
                {
                    drop(_apply);
                    self.publish_committed_projection(&result.installation, projection)
                        .await;
                    return Ok(result);
                }
            }
            bail!("installation remove contention did not converge");
        }

        self.refresh_remove_authority(&request).await?;
        let state_snapshot = self.snapshot_state(&request.installation_id).await?;
        let pending = PendingMutation {
            operation_id: uuid::Uuid::new_v4().to_string(),
            kind: PendingKind::Remove,
            previous: previous.clone(),
            state_snapshot,
            receipts: Vec::new(),
        };
        self.before_authority_append(REMOVE_STARTED).await?;
        let mut started = false;
        for _ in 0..CAS_ATTEMPTS {
            let _apply = self.apply.lock().await;
            self.sync_journal_locked().await?;
            if let Some(existing) = self.claimed_result(&key_hash, &fingerprint)? {
                return Ok(existing);
            }
            ensure!(
                self.views
                    .read()
                    .map_err(lock_error)?
                    .get(&request.installation_id)
                    == Some(&previous),
                "revision_conflict: installation changed before remove start"
            );
            ensure!(
                !self
                    .pending
                    .read()
                    .map_err(lock_error)?
                    .contains_key(&request.installation_id),
                "installation has a pending mutation"
            );
            if self
                .append_payload_with_installation_authority_locked(
                    REMOVE_STARTED,
                    &StartedPayload {
                        pending: pending.clone(),
                    },
                    &request.installation_id,
                    authority,
                )
                .await?
            {
                started = true;
                break;
            }
        }
        if !started {
            bail!("installation remove start contention did not converge");
        }

        let empty = StateSnapshot::empty();
        self.ensure_owner_lease().await?;
        if let Err(error) = authority
            .refresh_current_for_installation(&request.installation_id)
            .await
        {
            self.rollback_started_without_state_effect(&pending, "authority_expired")
                .await?;
            return Err(error);
        }
        if let Err(error) = self.replace_state_effect(&request.installation_id, &empty) {
            self.rollback_started(&pending, "state_delete_failed")
                .await?;
            return Err(error);
        }
        let payload = RemovedPayload {
            operation_id: Some(pending.operation_id.clone()),
            previous_revision: previous.revision,
            view: removed,
            claim,
        };
        self.before_authority_append(INSTALLATION_REMOVED).await?;
        let terminal = async {
            for _ in 0..CAS_ATTEMPTS {
                let _apply = self.apply.lock().await;
                self.sync_journal_locked().await?;
                ensure!(
                    self.pending
                        .read()
                        .map_err(lock_error)?
                        .get(&request.installation_id)
                        == Some(&pending),
                    "terminal remove no longer owns its pending operation"
                );
                let mut transitional = previous.clone();
                transitional.record.status = InstallationStatus::Removing;
                ensure!(
                    self.views
                        .read()
                        .map_err(lock_error)?
                        .get(&request.installation_id)
                        == Some(&transitional),
                    "terminal remove base changed after state effect"
                );
                if self
                    .append_payload_with_installation_authority_locked(
                        INSTALLATION_REMOVED,
                        &payload,
                        &request.installation_id,
                        authority,
                    )
                    .await?
                {
                    return Ok::<bool, anyhow::Error>(true);
                }
            }
            Ok(false)
        }
        .await;
        match terminal {
            Ok(true) => {}
            Ok(false) => {
                self.rollback_started(&pending, "terminal_append_conflict")
                    .await?;
                bail!("installation remove terminal contention did not converge");
            }
            Err(error) => {
                self.rollback_started(
                    &pending,
                    "terminal_append_or_authority_failed_after_state_effect",
                )
                .await?;
                return Err(error);
            }
        }
        self.publish_committed_projection(&result.installation, projection)
            .await;
        Ok(result)
    }

    async fn validate_issued_state_artifact(
        &self,
        installation_id: &InstallationId,
        descriptor: &ArtifactDescriptor,
    ) -> anyhow::Result<()> {
        validate_artifact_descriptor(descriptor)
            .map_err(|_| anyhow!("state artifact descriptor is invalid"))?;
        self.ensure_owner_lease().await?;
        let _apply = self.apply.lock().await;
        self.sync_journal_locked().await?;
        let terminal = self
            .idempotency
            .read()
            .map_err(lock_error)?
            .values()
            .any(|claim| {
                &claim.result.installation.record.installation_id == installation_id
                    && claim
                        .result
                        .receipts
                        .iter()
                        .any(|issued| issued == descriptor)
            });
        let pending = self
            .pending
            .read()
            .map_err(lock_error)?
            .values()
            .any(|pending| {
                &pending.previous.record.installation_id == installation_id
                    && pending.receipts.iter().any(|issued| issued == descriptor)
            });
        ensure!(
            terminal || pending,
            "state artifact was not issued by the authoritative Installation journal"
        );
        Ok(())
    }

    fn installation_secret_store_path(
        &self,
        installation_id: &InstallationId,
    ) -> anyhow::Result<PathBuf> {
        ensure!(
            self.views
                .read()
                .map_err(lock_error)?
                .contains_key(installation_id),
            "installation_not_found"
        );
        let installation = self.installation_dir(installation_id);
        prepare_existing_real_directory(&installation, "installation projection root")?;
        ensure_contained(
            &self.data_root.join("installations"),
            &installation,
            "installation projection root",
        )?;
        let path = installation.join("secrets.dat");
        if path.exists() {
            ensure_real_regular_file(&path, "installation secret store")?;
            ensure_contained(&installation, &path, "installation secret store")?;
        }
        Ok(path)
    }

    async fn acquire_ready_secret_store(
        &self,
        installation_id: &InstallationId,
        expected_revision: u64,
    ) -> anyhow::Result<plurora_runtime::InstallationSecretStoreGuard> {
        self.ensure_owner_lease().await?;
        let lifecycle = self.lifecycle_lock(installation_id)?.read_owned().await;
        let view = {
            let _apply = self.apply.lock().await;
            self.sync_journal_locked().await?;
            self.ensure_owner_lease().await?;
            let view = self
                .views
                .read()
                .map_err(lock_error)?
                .get(installation_id)
                .cloned()
                .ok_or_else(|| anyhow!("installation_not_found"))?;
            ensure!(
                view.record.installation_id == *installation_id
                    && view.revision == expected_revision
                    && view.record.status == InstallationStatus::Ready,
                "installation secret store precondition is stale"
            );
            view
        };
        let installation = self.prepare_installation_tree(installation_id)?;
        let path = installation.effect_path().join("secrets.dat");
        {
            let _apply = self.apply.lock().await;
            self.sync_journal_locked().await?;
            ensure!(
                self.views.read().map_err(lock_error)?.get(installation_id) == Some(&view),
                "installation secret store precondition changed while resolving its path"
            );
        }
        plurora_runtime::InstallationSecretStoreGuard::verified(
            installation_id,
            expected_revision,
            &view,
            path,
            Box::new((lifecycle, installation)),
        )
    }

    async fn acquire_ready_for_run(
        &self,
        installation_id: &InstallationId,
        expected_revision: u64,
    ) -> anyhow::Result<plurora_runtime::RunInstallationGuard> {
        self.acquire_ready_for_run_inner(installation_id, Some(expected_revision))
            .await
    }

    async fn inspect_current_ready_for_run(
        &self,
        installation_id: &InstallationId,
    ) -> anyhow::Result<plurora_runtime::RunInstallationArtifacts> {
        self.ensure_owner_lease().await?;
        let artifacts = self.verified_run_artifacts(installation_id, None).await?;
        self.ensure_owner_lease().await?;
        let current = self
            .views
            .read()
            .map_err(lock_error)?
            .get(installation_id)
            .cloned()
            .ok_or_else(|| anyhow!("installation_not_found"))?;
        ensure!(
            current.revision == artifacts.installation.revision
                && current.record.status == InstallationStatus::Ready
                && current.record.work_revision == artifacts.installation.record.work_revision
                && current.record.assembly_lock == artifacts.installation.record.assembly_lock,
            "installation_revision_conflict: retry Run status"
        );
        Ok(artifacts)
    }
}

impl InstallationRegistry {
    async fn acquire_ready_for_run_inner(
        &self,
        installation_id: &InstallationId,
        expected_revision: Option<u64>,
    ) -> anyhow::Result<plurora_runtime::RunInstallationGuard> {
        self.ensure_owner_lease().await?;
        let lifecycle = self.lifecycle_lock(installation_id)?.read_owned().await;
        {
            let _apply = self.apply.lock().await;
            self.sync_journal_locked().await?;
            let view = self
                .views
                .read()
                .map_err(lock_error)?
                .get(installation_id)
                .cloned()
                .ok_or_else(|| anyhow!("installation_not_found"))?;
            ensure!(
                expected_revision.is_none_or(|expected| view.revision == expected)
                    && view.record.status == InstallationStatus::Ready,
                "Run Installation precondition is stale"
            );
        }
        let artifacts = self
            .verified_run_artifacts(installation_id, expected_revision)
            .await?;
        let revision = artifacts.installation.revision;
        {
            let _apply = self.apply.lock().await;
            self.sync_journal_locked().await?;
            ensure!(
                self.views.read().map_err(lock_error)?.get(installation_id)
                    == Some(&artifacts.installation),
                "Run Installation precondition changed while verifying its closure"
            );
        }
        plurora_runtime::RunInstallationGuard::verified(
            installation_id,
            revision,
            artifacts,
            Box::new(lifecycle),
        )
    }

    async fn verified_run_artifacts(
        &self,
        installation_id: &InstallationId,
        expected_revision: Option<u64>,
    ) -> anyhow::Result<plurora_runtime::RunInstallationArtifacts> {
        let view = self
            .views
            .read()
            .map_err(lock_error)?
            .get(installation_id)
            .cloned()
            .ok_or_else(|| anyhow!("installation_not_found"))?;
        ensure!(
            view.record.installation_id == *installation_id
                && expected_revision.is_none_or(|expected| view.revision == expected)
                && view.record.status == InstallationStatus::Ready,
            "Run Installation precondition is stale"
        );
        let verified = self
            .verify_work_and_lock(&view.record.work_revision, &view.record.assembly_lock)
            .await?;
        ensure!(
            view.work_summary == InstallationWorkSummary::from_work_revision(&verified.work),
            "installation Work summary differs from its exact verified WorkRevision"
        );
        self.ensure_owner_lease().await?;
        Ok(plurora_runtime::RunInstallationArtifacts {
            installation: view,
            work: verified.work,
            assemblies: verified.assemblies,
            locks: verified.locks,
        })
    }
}

fn update_candidate(
    previous: &InstallationRecord,
    request: &InstallationUpdateRequest,
) -> InstallationRecord {
    let mut record = previous.clone();
    record.work_revision = request.work_revision.clone();
    record.assembly_lock = request.assembly_lock.clone();
    if let Some(display_name) = &request.display_name {
        record.display_name = display_name.clone();
    }
    if let Some(source) = &request.source {
        record.source = source.clone();
    }
    if let Some(state_bindings) = &request.state_bindings {
        record.state_bindings = state_bindings.clone();
    }
    if let Some(secret_policy) = &request.secret_policy {
        record.secret_policy = secret_policy.clone();
    }
    record.updated_at = Utc::now();
    record.status = InstallationStatus::Ready;
    record
}

fn installation_diff(
    old: &InstallationRecord,
    new: &InstallationRecord,
    current: &VerifiedInstallationArtifacts,
    candidate: &VerifiedInstallationArtifacts,
) -> anyhow::Result<InstallationDiff> {
    Ok(InstallationDiff {
        work_revision_changed: old.work_revision != new.work_revision,
        assembly_lock_changed: old.assembly_lock != new.assembly_lock,
        display_name_changed: old.display_name != new.display_name,
        source_changed: old.source != new.source,
        state_bindings_changed: old.state_bindings != new.state_bindings,
        secret_policy_changed: old.secret_policy != new.secret_policy,
        work_entrypoints: diff_items(
            &current.work.entrypoints,
            &candidate.work.entrypoints,
            |entrypoint| entrypoint.id.clone(),
        ),
        work_content_roots: diff_items(
            &current.work.content_roots,
            &candidate.work.content_roots,
            |descriptor| descriptor.digest.clone(),
        ),
        work_rights: diff_optional(&current.work.rights, &candidate.work.rights),
        work_transparency: diff_optional(&current.work.transparency, &candidate.work.transparency),
        work_operational_intent: diff_optional(
            &current.work.operational_intent,
            &candidate.work.operational_intent,
        ),
        assembly_nodes: diff_items(&current.assembly.nodes, &candidate.assembly.nodes, |node| {
            node.node_id.to_string()
        }),
        assembly_bindings: diff_items(
            &current.assembly.bindings,
            &candidate.assembly.bindings,
            |binding| binding.binding_id.clone(),
        ),
        assembly_exposed_ports: diff_items(
            &current.assembly.exposed_ports,
            &candidate.assembly.exposed_ports,
            |port| port.port_id.to_string(),
        ),
        state_slots: diff_state_slots(
            &current.assembly.state_slots,
            &candidate.assembly.state_slots,
        )?,
        assembly_lock_nodes: diff_items(&current.lock.nodes, &candidate.lock.nodes, |node| {
            node.node_id.to_string()
        }),
        assembly_lock_bindings: diff_items(
            &current.lock.bindings,
            &candidate.lock.bindings,
            |binding| binding.binding_id.clone(),
        ),
        assembly_lock_protocol_profiles: diff_items(
            &current.lock.protocol_profiles,
            &candidate.lock.protocol_profiles,
            |profile| {
                format!(
                    "{}@{}#{}",
                    profile.protocol_id, profile.version, profile.profile
                )
            },
        ),
        assembly_lock_content_roots: diff_items(
            &current.lock.content_roots,
            &candidate.lock.content_roots,
            |descriptor| descriptor.digest.clone(),
        ),
    })
}

fn diff_items<T, F>(current: &[T], candidate: &[T], key: F) -> Vec<InstallationItemDiff<T>>
where
    T: Clone + PartialEq,
    F: Fn(&T) -> String,
{
    let current = current
        .iter()
        .map(|value| (key(value), value))
        .collect::<BTreeMap<_, _>>();
    let candidate = candidate
        .iter()
        .map(|value| (key(value), value))
        .collect::<BTreeMap<_, _>>();
    current
        .keys()
        .chain(candidate.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|id| {
            let change = match (current.get(&id), candidate.get(&id)) {
                (None, Some(after)) => InstallationChange::Added {
                    after: (*after).clone(),
                },
                (Some(before), None) => InstallationChange::Removed {
                    before: (*before).clone(),
                },
                (Some(before), Some(after)) if before != after => InstallationChange::Changed {
                    before: (*before).clone(),
                    after: (*after).clone(),
                },
                _ => return None,
            };
            Some(InstallationItemDiff { id, change })
        })
        .collect()
}

fn diff_optional<T: Clone + PartialEq>(
    current: &Option<T>,
    candidate: &Option<T>,
) -> Option<InstallationChange<T>> {
    match (current, candidate) {
        (None, Some(after)) => Some(InstallationChange::Added {
            after: after.clone(),
        }),
        (Some(before), None) => Some(InstallationChange::Removed {
            before: before.clone(),
        }),
        (Some(before), Some(after)) if before != after => Some(InstallationChange::Changed {
            before: before.clone(),
            after: after.clone(),
        }),
        _ => None,
    }
}

fn diff_state_slots(
    current: &[StateSlotDescriptor],
    candidate: &[StateSlotDescriptor],
) -> anyhow::Result<Vec<InstallationStateSlotDiff>> {
    let current = current
        .iter()
        .map(|slot| (slot.state_slot_id.clone(), slot))
        .collect::<BTreeMap<_, _>>();
    let candidate = candidate
        .iter()
        .map(|slot| (slot.state_slot_id.clone(), slot))
        .collect::<BTreeMap<_, _>>();
    let ids = current
        .keys()
        .chain(candidate.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter();
    let mut differences = Vec::new();
    for state_slot_id in ids {
        let (change, required_action) =
            match (current.get(&state_slot_id), candidate.get(&state_slot_id)) {
                (None, Some(after)) => (
                    InstallationStateSlotChange::Added {
                        after: (*after).clone(),
                    },
                    InstallationStateSlotRequirement::None,
                ),
                (Some(before), None) => (
                    InstallationStateSlotChange::Removed {
                        before: (*before).clone(),
                    },
                    if before.is_durable() {
                        InstallationStateSlotRequirement::Reset
                    } else {
                        InstallationStateSlotRequirement::None
                    },
                ),
                (Some(before), Some(after)) if before != after => {
                    let required_action = state_slot_requirement(before, after)?;
                    (
                        InstallationStateSlotChange::Changed {
                            before: (*before).clone(),
                            after: (*after).clone(),
                        },
                        required_action,
                    )
                }
                _ => continue,
            };
        differences.push(InstallationStateSlotDiff {
            state_slot_id,
            change,
            required_action,
        });
    }
    Ok(differences)
}

fn state_slot_requirement(
    current: &StateSlotDescriptor,
    candidate: &StateSlotDescriptor,
) -> anyhow::Result<InstallationStateSlotRequirement> {
    let validation = validate_state_replacement(current, candidate, false);
    if !current.is_durable() {
        validation.map_err(|_| anyhow!("state replacement model validation failed"))?;
        return Ok(InstallationStateSlotRequirement::None);
    }
    let current_schema = current.schema_ref.as_ref().map(|value| &value.digest);
    let candidate_schema = candidate.schema_ref.as_ref().map(|value| &value.digest);
    let compatible = current.owner_node_id == candidate.owner_node_id
        && current_schema == candidate_schema
        && current.scope == candidate.scope
        && current.portability == candidate.portability;
    if compatible {
        validation.map_err(|_| anyhow!("state replacement model validation failed"))?;
        return Ok(InstallationStateSlotRequirement::None);
    }
    if candidate.migration_port.is_some() {
        validation.map_err(|_| anyhow!("state replacement migration contract is invalid"))?;
        return Ok(InstallationStateSlotRequirement::Replace);
    }
    validate_state_replacement(current, candidate, true)
        .map_err(|_| anyhow!("state replacement reset contract is invalid"))?;
    Ok(InstallationStateSlotRequirement::Reset)
}

fn validate_update_state_action(
    action: &InstallationStateAction,
    state_slots: &[InstallationStateSlotDiff],
    state_has_entries: bool,
) -> anyhow::Result<()> {
    if !state_has_entries {
        return Ok(());
    }
    let requires_reset = state_slots
        .iter()
        .any(|slot| slot.required_action == InstallationStateSlotRequirement::Reset);
    let requires_replace = state_slots
        .iter()
        .any(|slot| slot.required_action == InstallationStateSlotRequirement::Replace);
    match action {
        InstallationStateAction::Preserve if requires_reset => bail!("state_reset_required"),
        InstallationStateAction::Preserve if requires_replace => {
            bail!("state_migration_required")
        }
        InstallationStateAction::Replace { .. } if requires_reset => {
            bail!("state_reset_required")
        }
        InstallationStateAction::Preserve
        | InstallationStateAction::Replace { .. }
        | InstallationStateAction::Reset => Ok(()),
    }
}

fn diff_is_empty(diff: &InstallationDiff) -> bool {
    !diff.work_revision_changed
        && !diff.assembly_lock_changed
        && !diff.display_name_changed
        && !diff.source_changed
        && !diff.state_bindings_changed
        && !diff.secret_policy_changed
        && diff.work_entrypoints.is_empty()
        && diff.work_content_roots.is_empty()
        && diff.work_rights.is_none()
        && diff.work_transparency.is_none()
        && diff.work_operational_intent.is_none()
        && diff.assembly_nodes.is_empty()
        && diff.assembly_bindings.is_empty()
        && diff.assembly_exposed_ports.is_empty()
        && diff.state_slots.is_empty()
        && diff.assembly_lock_nodes.is_empty()
        && diff.assembly_lock_bindings.is_empty()
        && diff.assembly_lock_protocol_profiles.is_empty()
        && diff.assembly_lock_content_roots.is_empty()
}

fn validate_lock_closure(
    locks: &BTreeMap<String, AssemblyLock>,
    assemblies: &BTreeMap<String, AssemblyRevision>,
) -> anyhow::Result<()> {
    for lock in locks.values() {
        let assembly = assemblies
            .get(&lock.assembly.digest)
            .ok_or_else(|| anyhow!("assembly lock references a missing AssemblyRevision"))?;
        let revision_nodes = assembly
            .nodes
            .iter()
            .map(|node| (&node.node_id, node))
            .collect::<BTreeMap<_, _>>();
        ensure!(
            revision_nodes.len() == lock.nodes.len(),
            "assembly lock node set differs from its AssemblyRevision"
        );
        for node_lock in &lock.nodes {
            let node = revision_nodes
                .get(&node_lock.node_id)
                .ok_or_else(|| anyhow!("assembly lock contains an unknown node"))?;
            match &node.source {
                AssemblyNodeSource::Component { component } => ensure!(
                    &node_lock.artifact == component,
                    "assembly lock component pin differs from its AssemblyRevision node"
                ),
                AssemblyNodeSource::Assembly { assembly } => {
                    ensure!(
                        node_lock.artifact.artifact_type_uri == ASSEMBLY_LOCK_TYPE_URI,
                        "nested AssemblyRevision node is not pinned by an AssemblyLock"
                    );
                    let child = locks
                        .get(&node_lock.artifact.digest)
                        .ok_or_else(|| anyhow!("nested AssemblyLock object is missing"))?;
                    ensure!(
                        &child.assembly == assembly,
                        "nested AssemblyLock resolves a different AssemblyRevision"
                    );
                }
            }
        }
    }
    Ok(())
}

fn validate_assembly_port_catalogs(
    root: &ArtifactDescriptor,
    assemblies: &BTreeMap<String, AssemblyRevision>,
) -> anyhow::Result<()> {
    let mut cache = BTreeMap::new();
    let mut visiting = BTreeSet::new();
    let _ = assembly_boundary_ports(&root.digest, assemblies, &mut cache, &mut visiting)?;
    Ok(())
}

fn assembly_boundary_ports(
    digest: &str,
    assemblies: &BTreeMap<String, AssemblyRevision>,
    cache: &mut BTreeMap<String, BTreeMap<PortId, PortDescriptor>>,
    visiting: &mut BTreeSet<String>,
) -> anyhow::Result<BTreeMap<PortId, PortDescriptor>> {
    if let Some(cached) = cache.get(digest) {
        return Ok(cached.clone());
    }
    ensure!(
        visiting.insert(digest.to_string()),
        "assembly_cycle: Assembly port validation encountered a containment cycle"
    );
    let result: anyhow::Result<BTreeMap<PortId, PortDescriptor>> = (|| {
        let assembly = assemblies
            .get(digest)
            .ok_or_else(|| anyhow!("assembly revision closure is incomplete"))?;
        let mut catalog = BTreeMap::<PortEndpoint, PortDescriptor>::new();
        for node in &assembly.nodes {
            match &node.source {
                AssemblyNodeSource::Component { .. } => {
                    for port in &node.ports {
                        catalog.insert(
                            PortEndpoint {
                                node_id: node.node_id.clone(),
                                port_id: port.port_id.clone(),
                            },
                            port.clone(),
                        );
                    }
                }
                AssemblyNodeSource::Assembly { assembly: child } => {
                    for (port_id, port) in
                        assembly_boundary_ports(&child.digest, assemblies, cache, visiting)?
                    {
                        catalog.insert(
                            PortEndpoint {
                                node_id: node.node_id.clone(),
                                port_id,
                            },
                            port,
                        );
                    }
                }
            }
        }
        assembly
            .validate_ports(&catalog)
            .map_err(|_| anyhow!("Assembly port and migration contracts are invalid"))?;
        let mut boundary = BTreeMap::new();
        for exposure in &assembly.exposed_ports {
            let mut port = catalog
                .get(&exposure.target)
                .cloned()
                .ok_or_else(|| anyhow!("Assembly exposure target is missing"))?;
            port.port_id = exposure.port_id.clone();
            boundary.insert(exposure.port_id.clone(), port);
        }
        Ok(boundary)
    })();
    visiting.remove(digest);
    let boundary = result?;
    cache.insert(digest.to_string(), boundary.clone());
    Ok(boundary)
}

fn validate_view(view: &InstallationView) -> anyhow::Result<()> {
    ensure!(view.revision > 0, "installation revision must be positive");
    validate_installation_work_summary(&view.work_summary)?;
    view.record
        .validate()
        .map_err(|_| anyhow!("installation journal contains an invalid record"))?;
    if let Some(rollback) = &view.rollback {
        ensure!(
            rollback.revision < view.revision,
            "rollback revision is invalid"
        );
        validate_artifact_descriptor(&rollback.work_revision)
            .map_err(|_| anyhow!("rollback work descriptor is invalid"))?;
        validate_artifact_descriptor(&rollback.assembly_lock)
            .map_err(|_| anyhow!("rollback lock descriptor is invalid"))?;
        if let Some(snapshot) = &rollback.state_snapshot {
            validate_artifact_descriptor(snapshot)
                .map_err(|_| anyhow!("rollback state descriptor is invalid"))?;
            ensure!(
                snapshot.artifact_type_uri == INSTALLATION_STATE_SNAPSHOT_TYPE_URI,
                "rollback state descriptor type is invalid"
            );
        }
    }
    Ok(())
}

fn validate_installation_work_summary(summary: &InstallationWorkSummary) -> anyhow::Result<()> {
    for descriptor in summary
        .content_roots
        .iter()
        .chain(summary.rights.iter())
        .chain(summary.transparency.iter())
        .chain(summary.operational_intent.iter())
    {
        validate_artifact_descriptor(descriptor)
            .map_err(|_| anyhow!("installation Work summary descriptor is invalid"))?;
    }
    Ok(())
}

fn validate_claim(claim: &IdempotencyClaim) -> anyhow::Result<()> {
    ensure!(
        is_sha256_hex(&claim.key_hash),
        "idempotency key hash is invalid"
    );
    ensure!(
        is_sha256_hex(&claim.fingerprint),
        "mutation fingerprint is invalid"
    );
    validate_view(&claim.result.installation)?;
    if !claim.result.receipts.is_empty() {
        validate_state_receipt_descriptors(&claim.result.receipts)?;
    }
    Ok(())
}

fn validate_state_receipt_descriptors(receipts: &[ArtifactDescriptor]) -> anyhow::Result<()> {
    ensure!(
        receipts.len() == 2,
        "state mutation must carry one decision receipt and one authority evidence descriptor"
    );
    for receipt in receipts {
        validate_artifact_descriptor(receipt)
            .map_err(|_| anyhow!("state mutation receipt descriptor is invalid"))?;
    }
    ensure!(
        receipts.windows(2).all(|pair| {
            pair[0].digest < pair[1].digest
                || (pair[0].digest == pair[1].digest
                    && pair[0].artifact_type_uri < pair[1].artifact_type_uri)
        }),
        "state mutation receipt descriptors are not uniquely sorted"
    );
    let evidence = receipts
        .iter()
        .find(|descriptor| {
            descriptor.artifact_type_uri == INSTALLATION_STATE_AUTHORITY_EVIDENCE_TYPE_URI
        })
        .ok_or_else(|| anyhow!("state mutation has no authority evidence descriptor"))?;
    ensure!(
        evidence.media_type == INSTALLATION_STATE_AUTHORITY_EVIDENCE_MEDIA_TYPE
            && evidence.references.is_empty(),
        "state authority evidence descriptor is invalid"
    );
    let decision = receipts
        .iter()
        .find(|descriptor| {
            matches!(
                descriptor.artifact_type_uri.as_str(),
                INSTALLATION_STATE_RESET_RECEIPT_TYPE_URI
                    | INSTALLATION_STATE_REPLACEMENT_DECISION_RECEIPT_TYPE_URI
            )
        })
        .ok_or_else(|| anyhow!("state mutation has no decision receipt descriptor"))?;
    ensure!(
        decision.media_type == INSTALLATION_STATE_RECEIPT_MEDIA_TYPE
            && decision.references == [evidence.digest.clone()],
        "state decision receipt descriptor does not bind its authority evidence"
    );
    ensure!(
        evidence.digest != decision.digest,
        "state mutation receipt descriptors repeat an object"
    );
    Ok(())
}

fn mutation_fingerprint<T: Serialize>(kind: &str, request: &T) -> anyhow::Result<String> {
    let bytes = canonical_json_bytes(&serde_json::json!({"kind": kind, "request": request}))
        .context("encode installation mutation fingerprint")?;
    Ok(hash_bytes(&bytes))
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn parse_payload<T: for<'de> Deserialize<'de>>(event: &EventEnvelope) -> anyhow::Result<T> {
    serde_json::from_value(event.payload.clone())
        .map_err(|_| anyhow!("installation journal payload is malformed"))
}

fn lock_error<T>(_error: std::sync::PoisonError<T>) -> anyhow::Error {
    anyhow!("installation registry lock poisoned")
}

fn prepare_data_root(path: &Path) -> anyhow::Result<PathBuf> {
    if path.exists() {
        prepare_existing_real_directory(path, "installation data root")?;
    } else {
        fs::create_dir_all(path).map_err(|_| anyhow!("create installation data root failed"))?;
        prepare_existing_real_directory(path, "installation data root")?;
    }
    fs::canonicalize(path).map_err(|_| anyhow!("resolve installation data root failed"))
}

fn prepare_real_directory(path: &Path, label: &str) -> anyhow::Result<()> {
    if path.exists() {
        prepare_existing_real_directory(path, label)
    } else {
        fs::create_dir(path).map_err(|_| anyhow!("create {label} failed"))?;
        prepare_existing_real_directory(path, label)
    }
}

fn open_anchored_directory(path: &Path, label: &str) -> anyhow::Result<AnchoredDirectory> {
    prepare_existing_real_directory(path, label)?;
    #[cfg(windows)]
    let file = {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_SHARE_WRITE: u32 = 0x0000_0002;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        OpenOptions::new()
            .read(true)
            // Deliberately omit FILE_SHARE_DELETE: while an effect owns this
            // handle Windows cannot replace or rename the directory ancestor.
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .map_err(|_| anyhow!("open {label} anchor failed"))?
    };
    #[cfg(not(windows))]
    let file = fs::File::open(path).map_err(|_| anyhow!("open {label} anchor failed"))?;
    let handle = same_file::Handle::from_file(file)
        .map_err(|_| anyhow!("identify {label} anchor failed"))?;
    let current =
        same_file::Handle::from_path(path).map_err(|_| anyhow!("identify {label} failed"))?;
    ensure!(current == handle, "{label} changed while its anchor opened");
    let stable_path = fs::canonicalize(path).map_err(|_| anyhow!("resolve {label} failed"))?;
    let canonical = same_file::Handle::from_path(&stable_path)
        .map_err(|_| anyhow!("identify resolved {label} failed"))?;
    ensure!(canonical == handle, "{label} changed while it was resolved");
    Ok(AnchoredDirectory {
        path: stable_path,
        handle,
    })
}

fn capability_directory(
    directory: &AnchoredDirectory,
    label: &str,
) -> anyhow::Result<cap_std::fs::Dir> {
    let directory = cap_std::fs::Dir::from_std_file(
        directory
            .handle
            .as_file()
            .try_clone()
            .map_err(|_| anyhow!("clone {label} anchor failed"))?,
    );
    ensure!(
        directory
            .dir_metadata()
            .map_err(|_| anyhow!("inspect {label} failed"))?
            .is_dir(),
        "{label} is not a real directory"
    );
    Ok(directory)
}

fn capability_directory_handle(
    directory: &cap_std::fs::Dir,
    label: &str,
) -> anyhow::Result<same_file::Handle> {
    same_file::Handle::from_file(
        directory
            .try_clone()
            .map_err(|_| anyhow!("clone {label} capability failed"))?
            .into_std_file(),
    )
    .map_err(|_| anyhow!("identify {label} capability failed"))
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowsDirectoryIdentity {
    volume_serial_number: u32,
    file_index: u64,
}

#[cfg(windows)]
#[repr(C)]
struct WindowsFileTime {
    low_date_time: u32,
    high_date_time: u32,
}

#[cfg(windows)]
#[repr(C)]
struct WindowsByHandleFileInformation {
    file_attributes: u32,
    creation_time: WindowsFileTime,
    last_access_time: WindowsFileTime,
    last_write_time: WindowsFileTime,
    volume_serial_number: u32,
    file_size_high: u32,
    file_size_low: u32,
    number_of_links: u32,
    file_index_high: u32,
    file_index_low: u32,
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetFileInformationByHandle(
        file: *mut std::ffi::c_void,
        information: *mut WindowsByHandleFileInformation,
    ) -> i32;
}

#[cfg(windows)]
fn windows_directory_identity(
    directory: &cap_std::fs::Dir,
    label: &str,
) -> anyhow::Result<WindowsDirectoryIdentity> {
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle;

    let file = directory
        .try_clone()
        .map_err(|_| anyhow!("clone {label} capability failed"))?
        .into_std_file();
    let mut information = MaybeUninit::<WindowsByHandleFileInformation>::uninit();
    // SAFETY: `file` remains open for the call and `information` points to enough writable
    // storage for the exact Windows BY_HANDLE_FILE_INFORMATION layout above.
    let identified =
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) };
    ensure!(identified != 0, "identify {label} capability failed");
    // SAFETY: a successful GetFileInformationByHandle call initializes the whole structure.
    let information = unsafe { information.assume_init() };
    Ok(WindowsDirectoryIdentity {
        volume_serial_number: information.volume_serial_number,
        file_index: (u64::from(information.file_index_high) << 32)
            | u64::from(information.file_index_low),
    })
}

#[cfg(windows)]
fn windows_path_matches_directory(
    owner: &cap_std::fs::Dir,
    name: &Path,
    expected: WindowsDirectoryIdentity,
) -> anyhow::Result<bool> {
    let metadata = match owner.symlink_metadata(name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => bail!("inspect anchored directory entry failed"),
    };
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Ok(false);
    }
    let directory = owner
        .open_dir(name)
        .map_err(|_| anyhow!("open anchored directory entry failed"))?;
    Ok(windows_directory_identity(&directory, "anchored directory entry")? == expected)
}

fn prepare_capability_directory(
    owner: &cap_std::fs::Dir,
    name: &Path,
    label: &str,
) -> anyhow::Result<()> {
    match owner.symlink_metadata(name) {
        Ok(metadata) => ensure!(
            metadata.file_type().is_dir() && !metadata.file_type().is_symlink(),
            "{label} is not a real directory"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            owner
                .create_dir(name)
                .map_err(|_| anyhow!("create {label} failed"))?;
            let metadata = owner
                .symlink_metadata(name)
                .map_err(|_| anyhow!("inspect {label} failed"))?;
            ensure!(
                metadata.file_type().is_dir() && !metadata.file_type().is_symlink(),
                "{label} is not a real directory"
            );
        }
        Err(_) => bail!("inspect {label} failed"),
    }
    let opened = owner
        .open_dir(name)
        .map_err(|_| anyhow!("open {label} failed"))?;
    ensure!(
        opened
            .dir_metadata()
            .map_err(|_| anyhow!("inspect {label} failed"))?
            .is_dir(),
        "{label} is not a real directory"
    );
    Ok(())
}

fn prepare_new_capability_directory(
    owner: &cap_std::fs::Dir,
    name: &Path,
    label: &str,
) -> anyhow::Result<()> {
    match owner.symlink_metadata(name) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Ok(_) => bail!("{label} already exists"),
        Err(_) => bail!("inspect {label} failed"),
    }
    owner
        .create_dir(name)
        .map_err(|_| anyhow!("create {label} failed"))?;
    prepare_capability_directory(owner, name, label)
}

#[cfg(not(windows))]
fn capability_path_matches_directory(
    owner: &cap_std::fs::Dir,
    name: &Path,
    expected: &same_file::Handle,
) -> anyhow::Result<bool> {
    let metadata = match owner.symlink_metadata(name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => bail!("inspect anchored directory entry failed"),
    };
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Ok(false);
    }
    let directory = owner
        .open_dir(name)
        .map_err(|_| anyhow!("open anchored directory entry failed"))?;
    Ok(&capability_directory_handle(&directory, "anchored directory entry")? == expected)
}

fn prepare_existing_real_directory(path: &Path, label: &str) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| anyhow!("inspect {label} failed"))?;
    ensure!(
        metadata.file_type().is_dir(),
        "{label} is not a real directory"
    );
    ensure!(
        !metadata.file_type().is_symlink(),
        "{label} must not be a symlink"
    );
    ensure!(
        !is_reparse_point(&metadata),
        "{label} must not be a reparse point"
    );
    Ok(())
}

fn ensure_real_regular_file(path: &Path, label: &str) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| anyhow!("inspect {label} failed"))?;
    ensure!(
        metadata.file_type().is_file(),
        "{label} is not a regular file"
    );
    ensure!(
        !metadata.file_type().is_symlink(),
        "{label} must not be a symlink"
    );
    ensure!(
        !is_reparse_point(&metadata),
        "{label} must not be a reparse point"
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

fn ensure_contained(root: &Path, candidate: &Path, label: &str) -> anyhow::Result<()> {
    let root = fs::canonicalize(root).map_err(|_| anyhow!("resolve containment root failed"))?;
    let candidate = fs::canonicalize(candidate).map_err(|_| anyhow!("resolve {label} failed"))?;
    ensure!(
        candidate.starts_with(&root),
        "{label} escapes its owner root"
    );
    Ok(())
}

impl PreparedProjection {
    fn publish(self) -> anyhow::Result<()> {
        self._installation
            .ensure_current_path("installation projection root")?;
        #[cfg(all(test, unix))]
        apply_projection_ancestor_swap(&self._installation.path)?;
        self.record_replacement.publish()?;
        self.lock_replacement.publish()?;
        sync_directory(&self._installation)
    }
}

fn prepare_atomic_replacement(
    directory: &AnchoredDirectory,
    file_name: &str,
    bytes: &[u8],
) -> anyhow::Result<AtomicReplacement> {
    use cap_std::fs::OpenOptions as CapabilityOpenOptions;

    directory.ensure_current_path("projection directory")?;
    let directory = capability_directory(directory, "projection directory")?;
    let target_name = PathBuf::from(file_name);
    validate_anchored_projection_target(&directory, &target_name)?;
    let temporary_name = PathBuf::from(format!(".projection-{}.tmp", uuid::Uuid::new_v4()));
    let mut options = CapabilityOpenOptions::new();
    options.write(true).create_new(true);
    let mut temporary = directory
        .open_with(&temporary_name, &options)
        .map_err(|_| anyhow!("create projection temporary file failed"))?;
    if temporary
        .write_all(bytes)
        .and_then(|_| temporary.flush())
        .and_then(|_| temporary.sync_all())
        .is_err()
    {
        drop(temporary);
        let _ = directory.remove_file(&temporary_name);
        bail!("write projection temporary file failed");
    }
    Ok(AtomicReplacement {
        directory,
        target_name,
        temporary_name: Some(temporary_name),
        _temporary: temporary,
    })
}

fn validate_anchored_projection_target(
    directory: &cap_std::fs::Dir,
    name: &Path,
) -> anyhow::Result<()> {
    match directory.symlink_metadata(name) {
        Ok(metadata) => {
            ensure!(
                metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
                "projection file is not a regular file"
            );
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => bail!("inspect projection file failed"),
    }
}

#[cfg(unix)]
fn sync_directory(directory: &AnchoredDirectory) -> anyhow::Result<()> {
    directory
        .handle
        .as_file()
        .sync_all()
        .map_err(|_| anyhow!("sync projection directory failed"))
}

#[cfg(not(unix))]
fn sync_directory(_directory: &AnchoredDirectory) -> anyhow::Result<()> {
    Ok(())
}

fn read_state_entries(installation: &AnchoredDirectory) -> anyhow::Result<Vec<StateSnapshotEntry>> {
    #[cfg(all(test, unix))]
    apply_installation_ancestor_swap(&installation.path)?;

    // The Installation handle, not its ambient path, is the authority for the
    // state read. Every descendant is then opened one component at a time with
    // no-follow semantics from its already-open parent capability.
    let installation_directory = installation.capability("installation projection root")?;
    #[cfg(all(test, unix))]
    apply_state_root_open_swap(&installation.path)?;
    let state_name = Path::new("state");
    let state_metadata = installation_directory
        .symlink_metadata(state_name)
        .map_err(|_| anyhow!("inspect installation state root failed"))?;
    ensure!(
        state_metadata.file_type().is_dir() && !state_metadata.file_type().is_symlink(),
        "installation state root must be a real directory, not a symlink or reparse point"
    );
    let state_directory = installation_directory
        .open_dir_nofollow(state_name)
        .map_err(|_| anyhow!("open installation state root failed"))?;
    ensure!(
        state_directory
            .dir_metadata()
            .map_err(|_| anyhow!("inspect installation state root failed"))?
            .is_dir(),
        "installation state root is not a real directory"
    );
    let state_handle = capability_directory_handle(&state_directory, "installation state root")?;
    ensure!(
        state_directory_still_matches(&installation_directory, state_name, &state_handle)?,
        "installation state root changed while it was opened"
    );

    let mut entries = Vec::new();
    read_state_directory(&state_directory, Path::new(""), &state_handle, &mut entries)?;
    ensure!(
        state_directory_still_matches(&installation_directory, state_name, &state_handle)?,
        "installation state root changed during traversal"
    );
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn read_state_directory(
    directory: &cap_std::fs::Dir,
    prefix: &Path,
    expected_handle: &same_file::Handle,
    entries: &mut Vec<StateSnapshotEntry>,
) -> anyhow::Result<()> {
    ensure!(
        &capability_directory_handle(directory, "installation state directory")? == expected_handle,
        "installation state directory changed during traversal"
    );
    let mut children = directory
        .entries()
        .map_err(|_| anyhow!("read installation state directory failed"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| anyhow!("read installation state directory failed"))?;
    children.sort_by_key(|entry| entry.file_name());

    for child in children {
        let name = child.file_name();
        ensure_single_relative_component(Path::new(&name), "installation state entry")?;
        let relative = prefix.join(&name);
        let file_type = child
            .file_type()
            .map_err(|_| anyhow!("inspect installation state entry failed"))?;
        ensure!(
            !file_type.is_symlink(),
            "installation state must not contain symlinks or reparse points"
        );

        if file_type.is_dir() {
            let child_directory = directory
                .open_dir_nofollow(&name)
                .map_err(|_| anyhow!("open installation state directory failed"))?;
            ensure!(
                child_directory
                    .dir_metadata()
                    .map_err(|_| anyhow!("inspect installation state directory failed"))?
                    .is_dir(),
                "installation state directory is not a real directory"
            );
            let child_handle =
                capability_directory_handle(&child_directory, "installation state directory")?;
            read_state_directory(&child_directory, &relative, &child_handle, entries)?;
            ensure!(
                state_directory_still_matches(directory, Path::new(&name), &child_handle)?,
                "installation state directory changed during traversal"
            );
        } else if file_type.is_file() {
            let mut options = cap_std::fs::OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let mut file = directory
                .open_with(&name, &options)
                .map_err(|_| anyhow!("open installation state file failed"))?;
            let metadata = file
                .metadata()
                .map_err(|_| anyhow!("inspect installation state file failed"))?;
            ensure!(
                metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
                "installation state file is not a regular file"
            );
            let opened_handle = capability_file_handle(&file, "installation state file")?;
            #[cfg(all(test, unix))]
            apply_state_open_swap(&relative)?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)
                .map_err(|_| anyhow!("read installation state file failed"))?;
            ensure!(
                state_file_still_matches(directory, Path::new(&name), &opened_handle)?,
                "installation state file changed while it was read"
            );
            entries.push(StateSnapshotEntry {
                path: portable_state_path(&relative)?,
                bytes,
            });
        } else {
            bail!("installation state contains a non-regular entry");
        }
    }
    ensure!(
        &capability_directory_handle(directory, "installation state directory")? == expected_handle,
        "installation state directory changed during traversal"
    );
    Ok(())
}

fn capability_file_handle(
    file: &cap_std::fs::File,
    label: &str,
) -> anyhow::Result<same_file::Handle> {
    same_file::Handle::from_file(
        file.try_clone()
            .map_err(|_| anyhow!("clone {label} capability failed"))?
            .into_std(),
    )
    .map_err(|_| anyhow!("identify {label} capability failed"))
}

fn state_directory_still_matches(
    owner: &cap_std::fs::Dir,
    name: &Path,
    expected: &same_file::Handle,
) -> anyhow::Result<bool> {
    let metadata = match owner.symlink_metadata(name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => bail!("inspect installation state directory failed"),
    };
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Ok(false);
    }
    let current = match owner.open_dir_nofollow(name) {
        Ok(current) => current,
        Err(_) => return Ok(false),
    };
    Ok(&capability_directory_handle(&current, "installation state directory")? == expected)
}

fn state_file_still_matches(
    owner: &cap_std::fs::Dir,
    name: &Path,
    expected: &same_file::Handle,
) -> anyhow::Result<bool> {
    let metadata = match owner.symlink_metadata(name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => bail!("inspect installation state file failed"),
    };
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Ok(false);
    }
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let current = match owner.open_with(name, &options) {
        Ok(current) => current,
        Err(_) => return Ok(false),
    };
    let current_metadata = current
        .metadata()
        .map_err(|_| anyhow!("inspect installation state file failed"))?;
    if !current_metadata.file_type().is_file() || current_metadata.file_type().is_symlink() {
        return Ok(false);
    }
    Ok(&capability_file_handle(&current, "installation state file")? == expected)
}

fn portable_state_path(path: &Path) -> anyhow::Result<String> {
    let mut segments = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => segments.push(
                segment
                    .to_str()
                    .ok_or_else(|| anyhow!("installation state path is not UTF-8"))?
                    .to_string(),
            ),
            _ => bail!("installation state path is not relative"),
        }
    }
    ensure!(
        !segments.is_empty(),
        "installation state file has an empty path"
    );
    Ok(segments.join("/"))
}

#[cfg(all(test, unix))]
fn inject_state_open_swap(
    relative: PathBuf,
    path: PathBuf,
    replacement: PathBuf,
) -> anyhow::Result<()> {
    let swap = STATE_OPEN_SWAP.get_or_init(|| Mutex::new(None));
    *swap.lock().map_err(lock_error)? = Some(StateOpenSwap {
        relative,
        path,
        replacement,
    });
    Ok(())
}

#[cfg(all(test, unix))]
fn apply_state_open_swap(relative: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::symlink;

    let swap = STATE_OPEN_SWAP.get_or_init(|| Mutex::new(None));
    let mut swap = swap.lock().map_err(lock_error)?;
    let should_apply = swap
        .as_ref()
        .is_some_and(|pending| pending.relative == relative);
    if !should_apply {
        return Ok(());
    }
    let pending = swap.take().expect("checked pending state swap");
    fs::remove_file(&pending.path).map_err(|_| anyhow!("apply state test swap failed"))?;
    symlink(pending.replacement, pending.path).map_err(|_| anyhow!("apply state test swap failed"))
}

#[cfg(all(test, unix))]
fn inject_state_root_open_swap(
    path: PathBuf,
    parked: PathBuf,
    replacement: PathBuf,
) -> anyhow::Result<()> {
    let swap = STATE_ROOT_OPEN_SWAP.get_or_init(|| Mutex::new(None));
    *swap.lock().map_err(lock_error)? = Some(StateRootOpenSwap {
        path,
        parked,
        replacement,
    });
    Ok(())
}

#[cfg(all(test, unix))]
fn apply_state_root_open_swap(installation: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::symlink;

    let swap = STATE_ROOT_OPEN_SWAP.get_or_init(|| Mutex::new(None));
    let mut swap = swap.lock().map_err(lock_error)?;
    if !swap
        .as_ref()
        .is_some_and(|pending| pending.path.parent() == Some(installation))
    {
        return Ok(());
    }
    let pending = swap.take().expect("checked pending state-root swap");
    fs::rename(&pending.path, &pending.parked)
        .map_err(|_| anyhow!("apply state root test swap failed"))?;
    symlink(&pending.replacement, &pending.path)
        .map_err(|_| anyhow!("apply state root test swap failed"))
}

#[cfg(all(test, unix))]
fn inject_installation_ancestor_swap(
    path: PathBuf,
    parked: PathBuf,
    replacement: PathBuf,
) -> anyhow::Result<()> {
    let swap = INSTALLATION_ANCESTOR_SWAP.get_or_init(|| Mutex::new(None));
    *swap.lock().map_err(lock_error)? = Some(InstallationAncestorSwap {
        path,
        parked,
        replacement,
    });
    Ok(())
}

#[cfg(all(test, unix))]
fn apply_installation_ancestor_swap(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::symlink;

    let swap = INSTALLATION_ANCESTOR_SWAP.get_or_init(|| Mutex::new(None));
    let mut swap = swap.lock().map_err(lock_error)?;
    if !swap.as_ref().is_some_and(|pending| pending.path == path) {
        return Ok(());
    }
    let pending = swap.take().expect("checked pending ancestor swap");
    fs::rename(&pending.path, &pending.parked)
        .map_err(|_| anyhow!("apply installation ancestor test swap failed"))?;
    symlink(&pending.replacement, &pending.path)
        .map_err(|_| anyhow!("apply installation ancestor test swap failed"))
}

#[cfg(all(test, unix))]
fn inject_projection_ancestor_swap(
    path: PathBuf,
    parked: PathBuf,
    replacement: PathBuf,
) -> anyhow::Result<()> {
    let swap = PROJECTION_ANCESTOR_SWAP.get_or_init(|| Mutex::new(None));
    *swap.lock().map_err(lock_error)? = Some(InstallationAncestorSwap {
        path,
        parked,
        replacement,
    });
    Ok(())
}

#[cfg(all(test, unix))]
fn apply_projection_ancestor_swap(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::symlink;

    let swap = PROJECTION_ANCESTOR_SWAP.get_or_init(|| Mutex::new(None));
    let mut swap = swap.lock().map_err(lock_error)?;
    if !swap.as_ref().is_some_and(|pending| pending.path == path) {
        return Ok(());
    }
    let pending = swap.take().expect("checked pending projection swap");
    fs::rename(&pending.path, &pending.parked)
        .map_err(|_| anyhow!("apply projection ancestor test swap failed"))?;
    symlink(&pending.replacement, &pending.path)
        .map_err(|_| anyhow!("apply projection ancestor test swap failed"))
}

#[cfg(all(test, unix))]
fn inject_remove_tree_swap(
    root: PathBuf,
    target: PathBuf,
    parked: PathBuf,
    replacement: PathBuf,
) -> anyhow::Result<()> {
    let swap = REMOVE_TREE_SWAP.get_or_init(|| Mutex::new(None));
    *swap.lock().map_err(lock_error)? = Some(RemoveTreeSwap {
        root,
        target,
        parked,
        replacement,
    });
    Ok(())
}

#[cfg(all(test, unix))]
fn apply_remove_tree_swap(root: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::symlink;

    let swap = REMOVE_TREE_SWAP.get_or_init(|| Mutex::new(None));
    let mut swap = swap.lock().map_err(lock_error)?;
    if !swap.as_ref().is_some_and(|pending| pending.root == root) {
        return Ok(());
    }
    let pending = swap.take().expect("checked pending remove-tree swap");
    fs::rename(&pending.target, &pending.parked)
        .map_err(|_| anyhow!("apply remove-tree test swap failed"))?;
    symlink(&pending.replacement, &pending.target)
        .map_err(|_| anyhow!("apply remove-tree test swap failed"))
}

fn safe_relative_path(value: &str) -> anyhow::Result<PathBuf> {
    ensure!(
        !value.is_empty() && !value.contains('\\'),
        "state snapshot path is invalid"
    );
    let path = Path::new(value);
    ensure!(!path.is_absolute(), "state snapshot path is absolute");
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => result.push(segment),
            _ => bail!("state snapshot path escapes its root"),
        }
    }
    ensure!(
        !result.as_os_str().is_empty(),
        "state snapshot path is empty"
    );
    Ok(result)
}

fn write_snapshot_tree(root: &cap_std::fs::Dir, snapshot: &StateSnapshot) -> anyhow::Result<()> {
    ensure!(
        root.dir_metadata()
            .map_err(|_| anyhow!("inspect state staging root failed"))?
            .is_dir(),
        "state staging root is not a real directory"
    );
    for entry in &snapshot.entries {
        let relative = safe_relative_path(&entry.path)?;
        let parent_relative = relative.parent().unwrap_or_else(|| Path::new(""));
        let parent = create_snapshot_directories(root, parent_relative)?;
        let file_name = relative
            .file_name()
            .ok_or_else(|| anyhow!("state snapshot file has no name"))?;
        let mut options = cap_std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = parent
            .open_with(file_name, &options)
            .map_err(|_| anyhow!("create installation state file failed"))?;
        file.write_all(&entry.bytes)
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all())
            .map_err(|_| anyhow!("write installation state file failed"))?;
    }
    #[cfg(unix)]
    sync_capability_directory(root, "state staging root")?;
    Ok(())
}

#[cfg(unix)]
fn sync_capability_directory(directory: &cap_std::fs::Dir, label: &str) -> anyhow::Result<()> {
    // `cap_std::fs::Dir::open_dir` deliberately returns an `O_PATH` descriptor
    // on Linux. It is suitable for handle-relative traversal and identity checks,
    // but `fsync(2)` rejects it with EBADF. Re-open `.` relative to the already
    // held capability to obtain a readable directory descriptor without
    // reintroducing an ambient pathname or following a caller-controlled link.
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true);
    let durable = directory
        .open_with(Path::new("."), &options)
        .map_err(|_| anyhow!("open {label} for sync failed"))?;
    ensure!(
        durable
            .metadata()
            .map_err(|_| anyhow!("inspect {label} for sync failed"))?
            .is_dir(),
        "{label} is not a real directory"
    );
    durable
        .sync_all()
        .map_err(|_| anyhow!("sync {label} failed"))
}

fn create_snapshot_directories(
    root: &cap_std::fs::Dir,
    relative: &Path,
) -> anyhow::Result<cap_std::fs::Dir> {
    let mut current = root
        .try_clone()
        .map_err(|_| anyhow!("clone state staging root failed"))?;
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            bail!("state snapshot directory is invalid");
        };
        prepare_capability_directory(&current, Path::new(segment), "state staging directory")?;
        current = current
            .open_dir(segment)
            .map_err(|_| anyhow!("open state staging directory failed"))?;
    }
    Ok(current)
}

#[cfg(not(windows))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SafeTreeEntryKind {
    Directory,
    File,
}

#[cfg(not(windows))]
struct SafeTreeIdentity {
    kind: SafeTreeEntryKind,
    handle: same_file::Handle,
}

fn remove_safe_tree(owner: &AnchoredDirectory, name: &Path) -> anyhow::Result<()> {
    ensure_single_relative_component(name, "installation temporary state root")?;
    owner.ensure_current_path("installation owner root")?;
    let owner_directory = owner.capability("installation owner root")?;
    let metadata = owner_directory
        .symlink_metadata(name)
        .map_err(|_| anyhow!("inspect installation temporary state root failed"))?;
    ensure!(
        metadata.file_type().is_dir() && !metadata.file_type().is_symlink(),
        "installation temporary state root is not a real directory"
    );
    let tree = owner_directory
        .open_dir(name)
        .map_err(|_| anyhow!("open installation temporary state root failed"))?;

    #[cfg(windows)]
    {
        // Windows directory handles deny delete sharing. Keeping this handle open
        // pins the validated leaf while every child is removed relative to it.
        // After the tree is empty, move it to an unpredictable owner-relative
        // name and compare its detached file identity before the final remove_dir.
        let tree_identity = windows_directory_identity(&tree, "installation temporary state root")?;
        remove_windows_tree_contents(&tree)?;
        drop(tree);
        let quarantine = PathBuf::from(format!(".removal-{}.tmp", uuid::Uuid::new_v4()));
        owner_directory
            .rename(name, &owner_directory, &quarantine)
            .map_err(|_| anyhow!("quarantine installation temporary state failed"))?;
        if !windows_path_matches_directory(&owner_directory, &quarantine, tree_identity)? {
            // The stable name has already been detached. Never restore or delete
            // a quarantine whose identity is not the Host-owned tree we opened.
            bail!("installation temporary state identity changed before removal");
        }
        owner_directory
            .remove_dir(&quarantine)
            .map_err(|_| anyhow!("remove installation temporary state failed"))?;
        return Ok(());
    }

    #[cfg(not(windows))]
    let tree_handle = capability_directory_handle(&tree, "installation temporary state root")?;
    #[cfg(not(windows))]
    let mut inventory = BTreeMap::new();
    #[cfg(not(windows))]
    inventory_safe_tree(&tree, Path::new(""), &mut inventory)?;

    #[cfg(all(test, unix))]
    apply_remove_tree_swap(&owner.path.join(name))?;

    // First move the validated leaf to an unpredictable owner-relative name. If
    // the caller-controlled leaf changed after validation, the moved identity no
    // longer matches and nothing is recursively removed.
    #[cfg(not(windows))]
    let quarantine = PathBuf::from(format!(".removal-{}.tmp", uuid::Uuid::new_v4()));
    #[cfg(not(windows))]
    owner_directory
        .rename(name, &owner_directory, &quarantine)
        .map_err(|_| anyhow!("quarantine installation temporary state failed"))?;
    #[cfg(not(windows))]
    let quarantined = owner_directory.open_dir(&quarantine);
    #[cfg(not(windows))]
    let identity_matches = quarantined
        .as_ref()
        .ok()
        .and_then(|directory| {
            capability_directory_handle(directory, "quarantined installation state")
                .ok()
                .map(|handle| handle == tree_handle)
        })
        .unwrap_or(false);
    #[cfg(not(windows))]
    if !identity_matches {
        // The quarantine now names an exchanged object rather than the opened
        // Host-owned tree. Restoring or deleting it would mutate an identity we
        // never authorized, so preserve it under the unpredictable quarantine
        // name and fail closed.
        bail!("installation temporary state identity changed before removal");
    }
    #[cfg(not(windows))]
    let quarantined = quarantined.expect("identity match requires an open directory");
    #[cfg(not(windows))]
    if let Err(error) = remove_inventory_tree(&quarantined, Path::new(""), &inventory) {
        // Some entries may already be gone, so this tree no longer represents a
        // valid stable state. Keep the exact Host-owned identity under its random
        // quarantine name for diagnosis/recovery and fail the enclosing effect.
        // A second name-based move would reopen a TOCTOU substitution window.
        return Err(error);
    }
    #[cfg(not(windows))]
    ensure!(
        capability_path_matches_directory(&owner_directory, &quarantine, &tree_handle)?,
        "installation temporary state identity changed during removal"
    );
    #[cfg(not(windows))]
    owner_directory
        .remove_dir(&quarantine)
        .map_err(|_| anyhow!("remove installation temporary state failed"))?;
    #[cfg(not(windows))]
    Ok(())
}

fn ensure_single_relative_component(path: &Path, label: &str) -> anyhow::Result<()> {
    let mut components = path.components();
    ensure!(
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none(),
        "{label} is not an owner-relative child"
    );
    Ok(())
}

#[cfg(not(windows))]
fn inventory_safe_tree(
    directory: &cap_std::fs::Dir,
    prefix: &Path,
    inventory: &mut BTreeMap<PathBuf, SafeTreeIdentity>,
) -> anyhow::Result<()> {
    let mut entries = directory
        .entries()
        .map_err(|_| anyhow!("read installation temporary state failed"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| anyhow!("read installation temporary state failed"))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        ensure_single_relative_component(Path::new(&name), "installation state entry")?;
        let path = prefix.join(&name);
        let file_type = entry
            .file_type()
            .map_err(|_| anyhow!("inspect installation state entry failed"))?;
        ensure!(
            !file_type.is_symlink(),
            "installation state must not contain symlinks or reparse points"
        );
        let identity = if file_type.is_dir() {
            let child = entry
                .open_dir()
                .map_err(|_| anyhow!("open installation state directory failed"))?;
            let handle = capability_directory_handle(&child, "installation state directory")?;
            inventory_safe_tree(&child, &path, inventory)?;
            SafeTreeIdentity {
                kind: SafeTreeEntryKind::Directory,
                handle,
            }
        } else if file_type.is_file() {
            let file = entry
                .open()
                .map_err(|_| anyhow!("open installation state file failed"))?;
            let handle = same_file::Handle::from_file(file.into_std())
                .map_err(|_| anyhow!("identify installation state file failed"))?;
            SafeTreeIdentity {
                kind: SafeTreeEntryKind::File,
                handle,
            }
        } else {
            bail!("installation state contains a non-regular entry");
        };
        ensure!(
            inventory.insert(path, identity).is_none(),
            "installation state repeats an entry"
        );
    }
    Ok(())
}

#[cfg(windows)]
fn remove_windows_tree_contents(directory: &cap_std::fs::Dir) -> anyhow::Result<()> {
    let mut entries = directory
        .entries()
        .map_err(|_| anyhow!("read installation temporary state failed"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| anyhow!("read installation temporary state failed"))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|_| anyhow!("inspect installation state entry failed"))?;
        ensure!(
            !file_type.is_symlink(),
            "installation state must not contain symlinks or reparse points"
        );
        if file_type.is_dir() {
            let child = entry
                .open_dir()
                .map_err(|_| anyhow!("open installation state directory failed"))?;
            let child_identity =
                windows_directory_identity(&child, "installation state directory")?;
            remove_windows_tree_contents(&child)?;
            drop(child);
            let current = entry
                .open_dir()
                .map_err(|_| anyhow!("reopen installation state directory failed"))?;
            ensure!(
                windows_directory_identity(&current, "installation state directory")?
                    == child_identity,
                "installation temporary state directory identity changed"
            );
            drop(current);
            entry
                .remove_dir()
                .map_err(|_| anyhow!("remove installation state directory failed"))?;
        } else if file_type.is_file() {
            entry
                .remove_file()
                .map_err(|_| anyhow!("remove installation state file failed"))?;
        } else {
            bail!("installation state contains a non-regular entry");
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn remove_inventory_tree(
    directory: &cap_std::fs::Dir,
    prefix: &Path,
    inventory: &BTreeMap<PathBuf, SafeTreeIdentity>,
) -> anyhow::Result<()> {
    let mut entries = directory
        .entries()
        .map_err(|_| anyhow!("read installation temporary state failed"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| anyhow!("read installation temporary state failed"))?;
    entries.sort_by_key(|entry| entry.file_name());
    let expected_count = inventory
        .keys()
        .filter(|path| path.parent().is_some_and(|parent| parent == prefix))
        .count();
    ensure!(
        entries.len() == expected_count,
        "installation temporary state changed before removal"
    );
    for entry in entries {
        let name = entry.file_name();
        let path = prefix.join(&name);
        let expected = inventory
            .get(&path)
            .ok_or_else(|| anyhow!("installation temporary state changed before removal"))?;
        let file_type = entry
            .file_type()
            .map_err(|_| anyhow!("inspect installation state entry failed"))?;
        ensure!(
            !file_type.is_symlink(),
            "installation temporary state changed to a link before removal"
        );
        match expected.kind {
            SafeTreeEntryKind::Directory => {
                ensure!(
                    file_type.is_dir(),
                    "installation temporary state entry type changed"
                );
                let child = entry
                    .open_dir()
                    .map_err(|_| anyhow!("open installation state directory failed"))?;
                let child_handle =
                    capability_directory_handle(&child, "installation state directory")?;
                ensure!(
                    &child_handle == &expected.handle,
                    "installation temporary state directory identity changed"
                );
                remove_inventory_tree(&child, &path, inventory)?;
                let current = entry
                    .open_dir()
                    .map_err(|_| anyhow!("reopen installation state directory failed"))?;
                let current_handle =
                    capability_directory_handle(&current, "installation state directory")?;
                ensure!(
                    &current_handle == &expected.handle,
                    "installation temporary state directory identity changed"
                );
                entry
                    .remove_dir()
                    .map_err(|_| anyhow!("remove installation state directory failed"))?;
            }
            SafeTreeEntryKind::File => {
                ensure!(
                    file_type.is_file(),
                    "installation temporary state entry type changed"
                );
                let file = entry
                    .open()
                    .map_err(|_| anyhow!("open installation state file failed"))?;
                let file_handle = same_file::Handle::from_file(file.into_std())
                    .map_err(|_| anyhow!("identify installation state file failed"))?;
                ensure!(
                    &file_handle == &expected.handle,
                    "installation temporary state file identity changed"
                );
                entry
                    .remove_file()
                    .map_err(|_| anyhow!("remove installation state file failed"))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

    use plurora_core::{EventEnvelope, ProtocolProfilePin, COMPONENT_DESCRIPTOR_TYPE_URI};
    use plurora_runtime::{
        InMemoryEventStore, InMemoryObjectStore, InstallationAuthorityRefresh,
        InstallationAuthoritySubject, InstallationAuthorityValidator, ProtocolContext,
        ProtocolResourceSelector, Runtime, RuntimeConfig,
    };

    struct TestAuthorityValidator {
        calls: AtomicUsize,
        fail_after: usize,
        first_delay: Duration,
    }

    #[async_trait]
    impl InstallationAuthorityValidator for TestAuthorityValidator {
        async fn validate_current(
            &self,
            grant_id: &str,
            _subject: &InstallationAuthoritySubject,
        ) -> anyhow::Result<()> {
            ensure!(grant_id == "grant-live", "grant revoked");
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if call == 1 && !self.first_delay.is_zero() {
                tokio::time::sleep(self.first_delay).await;
            }
            ensure!(call <= self.fail_after, "grant revoked");
            Ok(())
        }
    }

    fn test_authority_refresh(
        fail_after: usize,
        first_delay: Duration,
    ) -> InstallationAuthorityRefresh {
        InstallationAuthorityRefresh::new(Arc::new(TestAuthorityValidator {
            calls: AtomicUsize::new(0),
            fail_after,
            first_delay,
        }))
    }

    struct ToggleAuthorityValidator {
        active: Arc<AtomicBool>,
        grant_id: String,
    }

    #[async_trait]
    impl InstallationAuthorityValidator for ToggleAuthorityValidator {
        async fn validate_current(
            &self,
            grant_id: &str,
            _subject: &InstallationAuthoritySubject,
        ) -> anyhow::Result<()> {
            ensure!(grant_id == self.grant_id, "unexpected grant");
            ensure!(self.active.load(Ordering::SeqCst), "grant revoked");
            Ok(())
        }
    }

    struct BoundaryAuthorityValidator {
        active: Arc<AtomicBool>,
        grant_id: String,
        calls: AtomicUsize,
        boundary_call: usize,
        entered: Arc<tokio::sync::Barrier>,
        release: Arc<tokio::sync::Barrier>,
    }

    #[async_trait]
    impl InstallationAuthorityValidator for BoundaryAuthorityValidator {
        async fn validate_current(
            &self,
            grant_id: &str,
            _subject: &InstallationAuthoritySubject,
        ) -> anyhow::Result<()> {
            ensure!(grant_id == self.grant_id, "unexpected grant");
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if call == self.boundary_call {
                self.entered.wait().await;
                self.release.wait().await;
            }
            ensure!(self.active.load(Ordering::SeqCst), "grant revoked");
            Ok(())
        }
    }
    use plurora_work::{
        AcquisitionKind, AcquisitionRecord, AssemblyBinding, AssemblyLock, AssemblyNode,
        AssemblyNodeSource, AssemblyPortExposure, BindingLock, BindingPhase, EffectClass, NodeId,
        NodeLock, PortContract, PortDirection, PortMultiplicity, PortRole, SelectedTransport,
        TransportPolicy, TransportRequirements, WorkEntrypoint, WorkEntrypointTarget, WorkId,
        WorkRevision, ASSEMBLY_LOCK_TYPE_URI, ASSEMBLY_REVISION_TYPE_URI,
    };

    struct Fixture {
        _data: TempDir,
        store: Arc<InMemoryEventStore>,
        objects: Arc<InMemoryObjectStore>,
        registry: Arc<InstallationRegistry>,
        request: InstallationCreateRequest,
    }

    impl Fixture {
        fn runtime(&self) -> Runtime<InMemoryEventStore> {
            Runtime::new(
                self.store.clone(),
                RuntimeConfig {
                    object_store: self.objects.clone(),
                    installation_control: self.registry.clone(),
                    ..RuntimeConfig::default()
                },
            )
        }

        async fn create(
            &self,
            request: InstallationCreateRequest,
        ) -> anyhow::Result<InstallationMutationResult> {
            let value = self
                .runtime()
                .call_protocol(
                    &ProtocolContext::host_dev("installation-registry-test"),
                    "host.installation.create",
                    serde_json::to_value(request)?,
                )
                .await
                .map_err(|error| anyhow!("{}: {}", error.code, error.message))?;
            Ok(serde_json::from_value(value)?)
        }

        async fn update(
            &self,
            request: InstallationUpdateRequest,
        ) -> anyhow::Result<InstallationMutationResult> {
            let value = self
                .runtime()
                .call_protocol(
                    &ProtocolContext::host_dev("installation-registry-test"),
                    "host.installation.update",
                    serde_json::to_value(request)?,
                )
                .await
                .map_err(|error| anyhow!("{}: {}", error.code, error.message))?;
            Ok(serde_json::from_value(value)?)
        }
    }

    async fn fixture() -> anyhow::Result<Fixture> {
        let data = tempfile::tempdir()?;
        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(InMemoryObjectStore::default());
        let (work_revision, assembly_lock) = put_work_pair(&objects, "one", Vec::new()).await?;
        let registry =
            InstallationRegistry::persistent(store.clone(), objects.clone(), data.path())?;
        let request = InstallationCreateRequest {
            work_id: WorkId::parse("tests/one")?,
            work_revision,
            assembly_lock,
            display_name: "Example".to_string(),
            source: AcquisitionRecord {
                kind: AcquisitionKind::WorkBundle,
                source_ref: None,
                provenance_refs: Vec::new(),
                update_channel: None,
            },
            state_bindings: Vec::new(),
            secret_policy: Default::default(),
            idempotency_key: "create-one".to_string(),
            authority: None,
        };
        Ok(Fixture {
            _data: data,
            store,
            objects,
            registry,
            request,
        })
    }

    async fn put_json<T: Serialize>(
        objects: &InMemoryObjectStore,
        artifact_type_uri: &str,
        value: &T,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let bytes = canonical_json_bytes(value)?;
        let info = objects.put(bytes.into()).await?;
        Ok(ArtifactDescriptor {
            artifact_type_uri: artifact_type_uri.to_string(),
            media_type: "application/json".to_string(),
            digest: info.digest,
            size_bytes: info.size_bytes,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        })
    }

    async fn put_model<T: ArtifactModel>(
        objects: &InMemoryObjectStore,
        value: &T,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let bytes = value.canonical_bytes()?;
        let descriptor = value.artifact_descriptor()?;
        let info = objects.put(bytes.into()).await?;
        assert_eq!(info.digest, descriptor.digest);
        assert_eq!(info.size_bytes, descriptor.size_bytes);
        Ok(descriptor)
    }

    async fn put_blob(
        objects: &InMemoryObjectStore,
        artifact_type_uri: &str,
        bytes: &[u8],
    ) -> anyhow::Result<ArtifactDescriptor> {
        let info = objects.put(bytes.to_vec().into()).await?;
        Ok(ArtifactDescriptor {
            artifact_type_uri: artifact_type_uri.to_string(),
            media_type: "application/octet-stream".to_string(),
            digest: info.digest,
            size_bytes: info.size_bytes,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        })
    }

    async fn wait_for_lifecycle_writer(
        lifecycle: &Arc<tokio::sync::RwLock<()>>,
    ) -> anyhow::Result<()> {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if lifecycle.try_read().is_err() {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .map_err(|_| anyhow!("lifecycle writer did not queue behind its active reader"))?;
        Ok(())
    }

    fn missing_descriptor(artifact_type_uri: &str, marker: char) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: artifact_type_uri.to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", marker.to_string().repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    async fn put_work_pair(
        objects: &InMemoryObjectStore,
        marker: &str,
        content_roots: Vec<ArtifactDescriptor>,
    ) -> anyhow::Result<(ArtifactDescriptor, ArtifactDescriptor)> {
        let assembly = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: plurora_work::AssemblyId::parse(format!("tests/{marker}-assembly"))?,
            nodes: Vec::new(),
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let assembly = put_model(objects, &assembly).await?;
        let work = WorkRevision {
            schema: WorkRevision::SCHEMA.to_string(),
            work_id: WorkId::parse(format!("tests/{marker}"))?,
            title: format!("Work {marker}"),
            description: String::new(),
            assembly: assembly.clone(),
            content_roots: content_roots.clone(),
            entrypoints: Vec::new(),
            rights: None,
            transparency: None,
            operational_intent: None,
            annotations: BTreeMap::new(),
        };
        let lock = AssemblyLock {
            schema: AssemblyLock::SCHEMA.to_string(),
            assembly,
            nodes: Vec::new(),
            bindings: Vec::new(),
            protocol_profiles: Vec::new(),
            content_roots,
        };
        Ok((
            put_model(objects, &work).await?,
            put_model(objects, &lock).await?,
        ))
    }

    fn update_request(
        created: &InstallationView,
        work_revision: ArtifactDescriptor,
        assembly_lock: ArtifactDescriptor,
        key: &str,
    ) -> InstallationUpdateRequest {
        InstallationUpdateRequest {
            installation_id: created.record.installation_id.clone(),
            expected_revision: created.revision,
            work_revision,
            assembly_lock,
            display_name: None,
            source: None,
            state_bindings: None,
            secret_policy: None,
            state_action: InstallationStateAction::Preserve,
            idempotency_key: key.to_string(),
            authority: None,
        }
    }

    #[tokio::test]
    async fn create_idempotency_replays_same_request_and_rejects_reuse() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let first = fixture.create(fixture.request.clone()).await?;
        let work: WorkRevision = serde_json::from_slice(
            &fixture
                .objects
                .get(&first.installation.record.work_revision.digest)
                .await?,
        )?;
        assert_eq!(
            first.installation.work_summary,
            InstallationWorkSummary::from_work_revision(&work)
        );
        let projection_root = fixture
            .registry
            .installation_dir(&first.installation.record.installation_id);
        let projection_record = projection_root.join("installation.json");
        let projection_lock = projection_root.join("assembly.lock.json");
        fs::remove_file(&projection_record)?;
        fs::remove_file(&projection_lock)?;
        let second = fixture.create(fixture.request.clone()).await?;
        assert_eq!(first.installation, second.installation);
        assert!(!first.idempotent);
        assert!(second.idempotent);
        assert!(!projection_record.exists());
        assert!(!projection_lock.exists());

        let mut conflicting = fixture.request.clone();
        conflicting.display_name = "Different".to_string();
        let error = fixture.create(conflicting).await.unwrap_err();
        assert!(error.to_string().contains("idempotency_conflict"));
        assert!(!error
            .to_string()
            .contains(fixture._data.path().to_string_lossy().as_ref()));
        Ok(())
    }

    #[tokio::test]
    async fn hydrate_rejects_journal_without_required_work_summary() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        fixture.create(fixture.request.clone()).await?;
        let mut event = fixture
            .store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?
            .into_iter()
            .next()
            .expect("created event");
        event
            .payload
            .get_mut("view")
            .and_then(serde_json::Value::as_object_mut)
            .expect("created view")
            .remove("work_summary");
        event
            .payload
            .pointer_mut("/claim/result/installation")
            .and_then(serde_json::Value::as_object_mut)
            .expect("claimed installation")
            .remove("work_summary");

        let strict_store = Arc::new(InMemoryEventStore::default());
        strict_store.append(event).await?;
        let data = tempfile::tempdir()?;
        let restarted =
            InstallationRegistry::persistent(strict_store, fixture.objects.clone(), data.path())?;
        assert!(restarted
            .hydrate()
            .await
            .expect_err("missing Work summary must not hydrate")
            .to_string()
            .contains("payload is malformed"));
        assert!(restarted
            .list(InstallationListRequest::default())
            .await?
            .is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn create_rejects_missing_assembly_node_nested_lock_and_content_closure_before_journal(
    ) -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let source = fixture.request.source.clone();
        let mut cases = Vec::new();

        let missing_assembly = missing_descriptor(ASSEMBLY_REVISION_TYPE_URI, 'a');
        let missing_assembly_work = WorkRevision {
            schema: WorkRevision::SCHEMA.to_string(),
            work_id: WorkId::parse("tests/missing-assembly")?,
            title: "Missing assembly".to_string(),
            description: String::new(),
            assembly: missing_assembly.clone(),
            content_roots: Vec::new(),
            entrypoints: Vec::new(),
            rights: None,
            transparency: None,
            operational_intent: None,
            annotations: BTreeMap::new(),
        };
        let missing_assembly_lock = AssemblyLock {
            schema: AssemblyLock::SCHEMA.to_string(),
            assembly: missing_assembly,
            nodes: Vec::new(),
            bindings: Vec::new(),
            protocol_profiles: Vec::new(),
            content_roots: Vec::new(),
        };
        cases.push((
            "missing-assembly",
            put_model(&fixture.objects, &missing_assembly_work).await?,
            put_model(&fixture.objects, &missing_assembly_lock).await?,
        ));

        let missing_component = missing_descriptor(COMPONENT_DESCRIPTOR_TYPE_URI, 'b');
        let node_id = plurora_work::NodeId::parse("component")?;
        let component_assembly = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: plurora_work::AssemblyId::parse("tests/missing-component-assembly")?,
            nodes: vec![plurora_work::AssemblyNode {
                node_id: node_id.clone(),
                source: AssemblyNodeSource::Component {
                    component: missing_component.clone(),
                },
                ports: Vec::new(),
                configuration: None,
                annotations: BTreeMap::new(),
            }],
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let component_assembly_ref = put_model(&fixture.objects, &component_assembly).await?;
        let component_work = WorkRevision {
            schema: WorkRevision::SCHEMA.to_string(),
            work_id: WorkId::parse("tests/missing-component")?,
            title: "Missing component".to_string(),
            description: String::new(),
            assembly: component_assembly_ref.clone(),
            content_roots: Vec::new(),
            entrypoints: Vec::new(),
            rights: None,
            transparency: None,
            operational_intent: None,
            annotations: BTreeMap::new(),
        };
        let component_lock = AssemblyLock {
            schema: AssemblyLock::SCHEMA.to_string(),
            assembly: component_assembly_ref,
            nodes: vec![plurora_work::NodeLock {
                node_id,
                artifact: missing_component,
                behavior_digest: Some(format!("sha256:{}", "c".repeat(64))),
                trust_class: Some(ComponentTrustClass::IsolatedProcess),
            }],
            bindings: Vec::new(),
            protocol_profiles: Vec::new(),
            content_roots: Vec::new(),
        };
        cases.push((
            "missing-component-provider",
            put_model(&fixture.objects, &component_work).await?,
            put_model(&fixture.objects, &component_lock).await?,
        ));

        let missing_nested_revision = missing_descriptor(ASSEMBLY_REVISION_TYPE_URI, 'd');
        let nested_node_id = plurora_work::NodeId::parse("nested")?;
        let parent_assembly = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: plurora_work::AssemblyId::parse("tests/missing-nested-assembly")?,
            nodes: vec![plurora_work::AssemblyNode {
                node_id: nested_node_id.clone(),
                source: AssemblyNodeSource::Assembly {
                    assembly: missing_nested_revision,
                },
                ports: Vec::new(),
                configuration: None,
                annotations: BTreeMap::new(),
            }],
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let parent_assembly_ref = put_model(&fixture.objects, &parent_assembly).await?;
        let nested_work = WorkRevision {
            schema: WorkRevision::SCHEMA.to_string(),
            work_id: WorkId::parse("tests/missing-nested")?,
            title: "Missing nested lock".to_string(),
            description: String::new(),
            assembly: parent_assembly_ref.clone(),
            content_roots: Vec::new(),
            entrypoints: Vec::new(),
            rights: None,
            transparency: None,
            operational_intent: None,
            annotations: BTreeMap::new(),
        };
        let nested_lock = AssemblyLock {
            schema: AssemblyLock::SCHEMA.to_string(),
            assembly: parent_assembly_ref,
            nodes: vec![plurora_work::NodeLock {
                node_id: nested_node_id,
                artifact: missing_descriptor(ASSEMBLY_LOCK_TYPE_URI, 'e'),
                behavior_digest: None,
                trust_class: None,
            }],
            bindings: Vec::new(),
            protocol_profiles: Vec::new(),
            content_roots: Vec::new(),
        };
        cases.push((
            "missing-nested-lock",
            put_model(&fixture.objects, &nested_work).await?,
            put_model(&fixture.objects, &nested_lock).await?,
        ));

        let missing_content = missing_descriptor("urn:test:content:v1", 'f');
        let (content_work, content_lock) =
            put_work_pair(&fixture.objects, "missing-content", vec![missing_content]).await?;
        cases.push(("missing-content", content_work, content_lock));

        for (key, work_revision, assembly_lock) in cases {
            let request = InstallationCreateRequest {
                work_id: WorkId::parse(format!("tests/{key}"))?,
                work_revision,
                assembly_lock,
                display_name: key.to_string(),
                source: source.clone(),
                state_bindings: Vec::new(),
                secret_policy: Default::default(),
                idempotency_key: key.to_string(),
                authority: None,
            };
            assert!(fixture.create(request).await.is_err(), "{key}");
        }
        assert!(fixture
            .registry
            .list(InstallationListRequest::default())
            .await?
            .is_empty());
        assert!(fixture
            .store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?
            .is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn stale_update_is_rejected_and_diff_keeps_rollback_roots() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let content_root =
            put_blob(&fixture.objects, "urn:test:content:v1", b"content-root").await?;
        let (work, lock) = put_work_pair(&fixture.objects, "two", vec![content_root]).await?;
        let mut stale = update_request(&created, work.clone(), lock.clone(), "stale");
        stale.expected_revision = 0;
        assert!(fixture
            .update(stale)
            .await
            .unwrap_err()
            .to_string()
            .contains("revision_conflict"));

        let updated = fixture
            .update(update_request(&created, work, lock, "update-two"))
            .await?;
        let diff = updated.diff.as_ref().expect("update diff");
        assert!(diff.work_revision_changed);
        assert!(diff.assembly_lock_changed);
        let rollback = updated
            .installation
            .rollback
            .as_ref()
            .expect("rollback pointer");
        assert_eq!(rollback.revision, created.revision);
        assert_eq!(rollback.work_revision, created.record.work_revision);
        assert_eq!(rollback.assembly_lock, created.record.assembly_lock);
        assert!(rollback.state_snapshot.is_none());
        let current_work: WorkRevision = serde_json::from_slice(
            &fixture
                .objects
                .get(&updated.installation.record.work_revision.digest)
                .await?,
        )?;
        assert_eq!(
            updated.installation.work_summary,
            InstallationWorkSummary::from_work_revision(&current_work)
        );
        assert_ne!(
            updated.installation.work_summary, created.work_summary,
            "the summary must advance with the exact WorkRevision"
        );
        Ok(())
    }

    #[test]
    fn state_slot_diff_drives_preserve_replace_reset_run_and_empty_state_rules(
    ) -> anyhow::Result<()> {
        fn slot(
            id: &str,
            owner: &str,
            schema_marker: char,
            scope: plurora_work::StateScope,
            portability: plurora_work::StatePortability,
            migration: bool,
        ) -> StateSlotDescriptor {
            let owner_node_id = plurora_work::NodeId::parse(owner).unwrap();
            StateSlotDescriptor {
                state_slot_id: plurora_work::StateSlotId::parse(id).unwrap(),
                owner_node_id: owner_node_id.clone(),
                schema_ref: Some(missing_descriptor(
                    "urn:plurora:test:state-schema:v1",
                    schema_marker,
                )),
                scope,
                portability,
                migration_port: migration.then_some(PortEndpoint {
                    node_id: owner_node_id,
                    port_id: PortId::parse("migrate").unwrap(),
                }),
                backup_policy: plurora_work::BackupPolicy::Required,
                annotations: BTreeMap::new(),
            }
        }

        let current = slot(
            "save",
            "owner-a",
            'a',
            plurora_work::StateScope::Installation,
            plurora_work::StatePortability::Portable,
            false,
        );
        let mut compatible = current.clone();
        compatible.backup_policy = plurora_work::BackupPolicy::Allowed;
        let compatible_diff = diff_state_slots(&[current.clone()], &[compatible])?;
        assert_eq!(
            compatible_diff[0].required_action,
            InstallationStateSlotRequirement::None
        );
        validate_update_state_action(&InstallationStateAction::Preserve, &compatible_diff, true)?;

        let migratable = slot(
            "save",
            "owner-b",
            'b',
            plurora_work::StateScope::User,
            plurora_work::StatePortability::OpaqueExportable,
            true,
        );
        let migratable_diff = diff_state_slots(&[current.clone()], &[migratable.clone()])?;
        assert_eq!(
            migratable_diff[0].required_action,
            InstallationStateSlotRequirement::Replace
        );
        assert!(validate_update_state_action(
            &InstallationStateAction::Preserve,
            &migratable_diff,
            true
        )
        .unwrap_err()
        .to_string()
        .contains("state_migration_required"));
        validate_update_state_action(
            &InstallationStateAction::Replace {
                replacement_snapshot: missing_descriptor(INSTALLATION_STATE_SNAPSHOT_TYPE_URI, 'e'),
            },
            &migratable_diff,
            true,
        )?;

        let mut no_port = migratable;
        no_port.migration_port = None;
        let reset_diff = diff_state_slots(&[current.clone()], &[no_port])?;
        assert_eq!(
            reset_diff[0].required_action,
            InstallationStateSlotRequirement::Reset
        );
        assert!(
            validate_update_state_action(
                &InstallationStateAction::Replace {
                    replacement_snapshot: missing_descriptor(
                        INSTALLATION_STATE_SNAPSHOT_TYPE_URI,
                        'f',
                    ),
                },
                &reset_diff,
                true,
            )
            .is_err()
        );
        validate_update_state_action(&InstallationStateAction::Reset, &reset_diff, true)?;
        let removed_diff = diff_state_slots(&[current.clone()], &[])?;
        assert_eq!(
            removed_diff[0].required_action,
            InstallationStateSlotRequirement::Reset
        );

        let run = slot(
            "run-cache",
            "owner-a",
            'c',
            plurora_work::StateScope::Run,
            plurora_work::StatePortability::HostBound,
            false,
        );
        let run_changed = slot(
            "run-cache",
            "owner-b",
            'd',
            plurora_work::StateScope::Run,
            plurora_work::StatePortability::ExternalAuthority,
            false,
        );
        let run_diff = diff_state_slots(&[run], &[run_changed])?;
        assert_eq!(
            run_diff[0].required_action,
            InstallationStateSlotRequirement::None
        );
        validate_update_state_action(&InstallationStateAction::Preserve, &run_diff, true)?;

        validate_update_state_action(&InstallationStateAction::Preserve, &removed_diff, false)?;
        Ok(())
    }

    #[test]
    fn structured_installation_diff_is_typed_and_stably_sorted() -> anyhow::Result<()> {
        fn endpoint(node: &str, port: &str) -> PortEndpoint {
            PortEndpoint {
                node_id: NodeId::parse(node).unwrap(),
                port_id: PortId::parse(port).unwrap(),
            }
        }
        fn export_port(id: &str) -> PortDescriptor {
            PortDescriptor {
                port_id: PortId::parse(id).unwrap(),
                contract: PortContract {
                    protocol_id: "tests.diff".to_string(),
                    interface_id: "value".to_string(),
                    version: "1.0.0".to_string(),
                    profiles: Vec::new(),
                },
                interaction: plurora_work::InteractionModelId(
                    plurora_work::INTERACTION_ARTIFACT.to_string(),
                ),
                role: PortRole::Export {
                    multiplicity: PortMultiplicity {
                        min: 0,
                        max: Some(1),
                    },
                    effect_class: EffectClass::Pure,
                },
                transport: TransportRequirements::default(),
                annotations: BTreeMap::new(),
            }
        }
        fn node(id: &str, marker: char) -> AssemblyNode {
            AssemblyNode {
                node_id: NodeId::parse(id).unwrap(),
                source: AssemblyNodeSource::Component {
                    component: missing_descriptor(COMPONENT_DESCRIPTOR_TYPE_URI, marker),
                },
                ports: vec![export_port("out")],
                configuration: Some(missing_descriptor("urn:plurora:test:config:v1", marker)),
                annotations: BTreeMap::new(),
            }
        }
        fn binding(id: &str, node: &str) -> AssemblyBinding {
            AssemblyBinding {
                binding_id: id.to_string(),
                provider: endpoint(node, "out"),
                consumer: endpoint(node, "in"),
                phase: BindingPhase::Authoring,
                transport_policy: TransportPolicy::default(),
                annotations: BTreeMap::new(),
            }
        }
        fn exposure(id: &str, node: &str) -> AssemblyPortExposure {
            AssemblyPortExposure {
                port_id: PortId::parse(id).unwrap(),
                direction: PortDirection::Export,
                target: endpoint(node, "out"),
                annotations: BTreeMap::new(),
            }
        }

        let current_assembly = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: plurora_work::AssemblyId::parse("tests/diff-current")?,
            nodes: Vec::new(),
            bindings: Vec::new(),
            exposed_ports: Vec::new(),
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let candidate_assembly = AssemblyRevision {
            schema: AssemblyRevision::SCHEMA.to_string(),
            assembly_id: plurora_work::AssemblyId::parse("tests/diff-candidate")?,
            nodes: vec![node("z-node", 'c'), node("a-node", 'b')],
            bindings: vec![
                binding("z-binding", "z-node"),
                binding("a-binding", "a-node"),
            ],
            exposed_ports: vec![exposure("z-port", "z-node"), exposure("a-port", "a-node")],
            state_slots: Vec::new(),
            annotations: BTreeMap::new(),
        };
        let current_assembly_ref = missing_descriptor(ASSEMBLY_REVISION_TYPE_URI, '1');
        let candidate_assembly_ref = missing_descriptor(ASSEMBLY_REVISION_TYPE_URI, '2');
        let root_z = missing_descriptor("urn:plurora:test:content:v1", 'd');
        let root_a = missing_descriptor("urn:plurora:test:content:v1", 'a');
        let rights = missing_descriptor(RIGHTS_DECLARATION_TYPE_URI, 'e');
        let transparency = missing_descriptor(TRANSPARENCY_DECLARATION_TYPE_URI, 'f');
        let intent = missing_descriptor(OPERATIONAL_INTENT_TYPE_URI, '9');
        let current_work = WorkRevision {
            schema: WorkRevision::SCHEMA.to_string(),
            work_id: WorkId::parse("tests/diff")?,
            title: "Diff".to_string(),
            description: String::new(),
            assembly: current_assembly_ref.clone(),
            content_roots: Vec::new(),
            entrypoints: Vec::new(),
            rights: None,
            transparency: None,
            operational_intent: None,
            annotations: BTreeMap::new(),
        };
        let candidate_work = WorkRevision {
            assembly: candidate_assembly_ref.clone(),
            content_roots: vec![root_z.clone(), root_a.clone()],
            entrypoints: vec![
                WorkEntrypoint {
                    id: "z-entry".to_string(),
                    intent_uri: "tests.intent.z".to_string(),
                    target: WorkEntrypointTarget::Surface {
                        surface_id: "tests/surface-z".to_string(),
                    },
                    annotations: BTreeMap::new(),
                },
                WorkEntrypoint {
                    id: "a-entry".to_string(),
                    intent_uri: "tests.intent.a".to_string(),
                    target: WorkEntrypointTarget::Surface {
                        surface_id: "tests/surface-a".to_string(),
                    },
                    annotations: BTreeMap::new(),
                },
            ],
            rights: Some(rights.clone()),
            transparency: Some(transparency.clone()),
            operational_intent: Some(intent.clone()),
            ..current_work.clone()
        };
        let current_lock = AssemblyLock {
            schema: AssemblyLock::SCHEMA.to_string(),
            assembly: current_assembly_ref,
            nodes: Vec::new(),
            bindings: Vec::new(),
            protocol_profiles: Vec::new(),
            content_roots: Vec::new(),
        };
        let candidate_lock = AssemblyLock {
            schema: AssemblyLock::SCHEMA.to_string(),
            assembly: candidate_assembly_ref,
            nodes: candidate_assembly
                .nodes
                .iter()
                .map(|node| NodeLock {
                    node_id: node.node_id.clone(),
                    artifact: node.source.artifact().clone(),
                    behavior_digest: Some(format!("sha256:{}", "7".repeat(64))),
                    trust_class: Some(ComponentTrustClass::SandboxedComponent),
                })
                .collect(),
            bindings: vec![
                BindingLock {
                    binding_id: "z-binding".to_string(),
                    provider: endpoint("z-node", "out"),
                    consumer: endpoint("z-node", "in"),
                    provider_component: missing_descriptor(COMPONENT_DESCRIPTOR_TYPE_URI, 'c'),
                    transport: SelectedTransport {
                        class_id: "inproc".to_string(),
                        properties: BTreeMap::new(),
                    },
                    phase: BindingPhase::Authoring,
                },
                BindingLock {
                    binding_id: "a-binding".to_string(),
                    provider: endpoint("a-node", "out"),
                    consumer: endpoint("a-node", "in"),
                    provider_component: missing_descriptor(COMPONENT_DESCRIPTOR_TYPE_URI, 'b'),
                    transport: SelectedTransport {
                        class_id: "inproc".to_string(),
                        properties: BTreeMap::new(),
                    },
                    phase: BindingPhase::Authoring,
                },
            ],
            protocol_profiles: vec![
                ProtocolProfilePin {
                    protocol_id: "z.protocol".to_string(),
                    version: "1.0.0".to_string(),
                    profile: "z".to_string(),
                },
                ProtocolProfilePin {
                    protocol_id: "a.protocol".to_string(),
                    version: "1.0.0".to_string(),
                    profile: "a".to_string(),
                },
            ],
            content_roots: vec![root_z, root_a],
        };
        let installation_id = InstallationId::new();
        let old_record = InstallationRecord {
            schema_version: InstallationRecord::SCHEMA_VERSION,
            installation_id,
            work_revision: missing_descriptor(WORK_REVISION_TYPE_URI, '3'),
            assembly_lock: missing_descriptor(ASSEMBLY_LOCK_TYPE_URI, '4'),
            display_name: "Diff".to_string(),
            source: AcquisitionRecord {
                kind: AcquisitionKind::WorkBundle,
                source_ref: None,
                provenance_refs: Vec::new(),
                update_channel: None,
            },
            state_bindings: Vec::new(),
            secret_policy: Default::default(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: InstallationStatus::Ready,
        };
        let mut new_record = old_record.clone();
        new_record.work_revision = missing_descriptor(WORK_REVISION_TYPE_URI, '5');
        new_record.assembly_lock = missing_descriptor(ASSEMBLY_LOCK_TYPE_URI, '6');
        let diff = installation_diff(
            &old_record,
            &new_record,
            &VerifiedInstallationArtifacts {
                work: current_work,
                assembly: current_assembly,
                lock: current_lock,
                lock_bytes: Vec::new(),
                assemblies: BTreeMap::new(),
                locks: BTreeMap::new(),
            },
            &VerifiedInstallationArtifacts {
                work: candidate_work,
                assembly: candidate_assembly,
                lock: candidate_lock,
                lock_bytes: Vec::new(),
                assemblies: BTreeMap::new(),
                locks: BTreeMap::new(),
            },
        )?;

        assert_eq!(
            diff.work_entrypoints
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["a-entry", "z-entry"]
        );
        assert_eq!(
            diff.assembly_nodes
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["a-node", "z-node"]
        );
        assert_eq!(
            diff.assembly_bindings
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["a-binding", "z-binding"]
        );
        assert_eq!(
            diff.assembly_exposed_ports
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["a-port", "z-port"]
        );
        assert_eq!(
            diff.assembly_lock_bindings
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["a-binding", "z-binding"]
        );
        assert_eq!(
            diff.assembly_lock_protocol_profiles
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["a.protocol@1.0.0#a", "z.protocol@1.0.0#z"]
        );
        assert_eq!(diff.work_content_roots.len(), 2);
        assert_eq!(diff.assembly_lock_content_roots.len(), 2);
        assert!(
            matches!(diff.work_rights, Some(InstallationChange::Added { after }) if after == rights)
        );
        assert!(
            matches!(diff.work_transparency, Some(InstallationChange::Added { after }) if after == transparency)
        );
        assert!(
            matches!(diff.work_operational_intent, Some(InstallationChange::Added { after }) if after == intent)
        );
        assert!(matches!(
            diff.assembly_nodes[0].change,
            InstallationChange::Added { .. }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn update_rejects_missing_artifact_closure_without_advancing_revision(
    ) -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let missing = missing_descriptor("urn:test:content:v1", '9');
        let (work, lock) =
            put_work_pair(&fixture.objects, "missing-update-content", vec![missing]).await?;
        let error = fixture
            .update(update_request(&created, work, lock, "missing-update"))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("unavailable or corrupt"));
        assert_eq!(
            fixture
                .registry
                .get(&created.record.installation_id)
                .await?,
            Some(created)
        );
        assert_eq!(
            fixture
                .store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn preserve_keeps_state_when_work_and_lock_change_without_state_slot_diff(
    ) -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        fs::write(
            fixture
                .registry
                .state_dir(&created.record.installation_id)
                .join("save.bin"),
            b"state",
        )?;
        let (work, lock) = put_work_pair(&fixture.objects, "three", Vec::new()).await?;
        let updated = fixture
            .update(update_request(&created, work, lock, "migration-needed"))
            .await?;
        assert_eq!(updated.installation.revision, created.revision + 1);
        assert!(updated
            .diff
            .as_ref()
            .is_some_and(|diff| diff.work_revision_changed && diff.assembly_lock_changed));
        assert_eq!(
            fs::read(
                fixture
                    .registry
                    .state_dir(&created.record.installation_id)
                    .join("save.bin")
            )?,
            b"state"
        );
        Ok(())
    }

    #[tokio::test]
    async fn state_replace_fails_closed_without_current_authority() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let state = fixture.registry.state_dir(&created.record.installation_id);
        fs::write(state.join("save.bin"), b"old-state")?;

        let mut missing_request = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "missing-replacement-state",
        );
        missing_request.state_action = InstallationStateAction::Replace {
            replacement_snapshot: missing_descriptor(INSTALLATION_STATE_SNAPSHOT_TYPE_URI, '8'),
        };
        assert!(fixture
            .update(missing_request)
            .await
            .unwrap_err()
            .to_string()
            .contains("unavailable or corrupt"));
        assert_eq!(fs::read(state.join("save.bin"))?, b"old-state");

        let malformed = put_json(
            &fixture.objects,
            INSTALLATION_STATE_SNAPSHOT_TYPE_URI,
            &serde_json::json!({
                "schema": INSTALLATION_STATE_SNAPSHOT_SCHEMA,
                "entries": [{"path": "escape/../save.bin", "bytes": [1]}]
            }),
        )
        .await?;
        let mut malformed_request = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "malformed-replacement-state",
        );
        malformed_request.state_action = InstallationStateAction::Replace {
            replacement_snapshot: malformed,
        };
        assert!(fixture
            .update(malformed_request)
            .await
            .unwrap_err()
            .to_string()
            .contains("state snapshot is invalid"));
        assert_eq!(fs::read(state.join("save.bin"))?, b"old-state");

        let replacement = StateSnapshot {
            schema: INSTALLATION_STATE_SNAPSHOT_SCHEMA.to_string(),
            entries: vec![StateSnapshotEntry {
                path: "nested/save.bin".to_string(),
                bytes: b"new-state".to_vec(),
            }],
        };
        let replacement = put_json(
            &fixture.objects,
            INSTALLATION_STATE_SNAPSHOT_TYPE_URI,
            &replacement,
        )
        .await?;
        let mut request = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "replace-state",
        );
        request.state_action = InstallationStateAction::Replace {
            replacement_snapshot: replacement,
        };
        let error = fixture.registry.update(request).await.unwrap_err();
        assert!(error.to_string().contains("authority_denied"));
        assert_eq!(fs::read(state.join("save.bin"))?, b"old-state");
        assert!(!state.join("nested").exists());
        assert_eq!(
            fixture
                .registry
                .get(&created.record.installation_id)
                .await?,
            Some(created)
        );
        assert_eq!(
            fixture
                .store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn host_issues_state_receipts_under_current_exact_runtime_authority() -> anyhow::Result<()>
    {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let state = fixture.registry.state_dir(&created.record.installation_id);
        fs::write(state.join("save.bin"), b"old-state")?;
        let expires_at_ms = Utc::now().timestamp_millis() + 60_000;
        let mut request = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "authorized-reset",
        );
        request.state_action = InstallationStateAction::Reset;
        let runtime = Runtime::new(
            fixture.store.clone(),
            RuntimeConfig {
                object_store: fixture.objects.clone(),
                installation_control: fixture.registry.clone(),
                ..RuntimeConfig::default()
            },
        );
        let context = |grant_id: &str,
                       installation_id: &InstallationId,
                       expiry: i64,
                       refresh: InstallationAuthorityRefresh| {
            ProtocolContext::host_device(
                grant_id,
                vec!["installation.manage".to_string()],
                vec![ProtocolResourceSelector {
                    owner: "host".to_string(),
                    kind: "installation".to_string(),
                    id: Some(installation_id.to_string()),
                }],
                Vec::new(),
                "installation-authority-test",
            )
            .with_verified_authority_expiry(Some(expiry))
            .with_installation_authority_refresh(refresh)
        };
        let params = serde_json::to_value(&request)?;

        let expired = context(
            "grant-live",
            &created.record.installation_id,
            Utc::now().timestamp_millis() - 1,
            test_authority_refresh(usize::MAX, Duration::ZERO),
        );
        assert!(runtime
            .call_protocol(&expired, "host.installation.update", params.clone())
            .await
            .is_err());

        let wrong_subject = context(
            "grant-live",
            &InstallationId::new(),
            expires_at_ms,
            test_authority_refresh(usize::MAX, Duration::ZERO),
        );
        assert!(runtime
            .call_protocol(&wrong_subject, "host.installation.update", params.clone())
            .await
            .is_err());

        let expires_during_refresh = context(
            "grant-live",
            &created.record.installation_id,
            Utc::now().timestamp_millis() + 5,
            test_authority_refresh(usize::MAX, Duration::from_millis(20)),
        );
        let error = runtime
            .call_protocol(
                &expires_during_refresh,
                "host.installation.update",
                params.clone(),
            )
            .await
            .expect_err("authority that expires across refresh I/O must fail closed");
        assert!(
            error.message.contains("authority_denied"),
            "unexpected create error: {}",
            error.message
        );
        assert_eq!(fs::read(state.join("save.bin"))?, b"old-state");

        let revoked_during_effect = context(
            "grant-live",
            &created.record.installation_id,
            expires_at_ms,
            test_authority_refresh(6, Duration::ZERO),
        );
        let error = runtime
            .call_protocol(
                &revoked_during_effect,
                "host.installation.update",
                params.clone(),
            )
            .await
            .expect_err("revoked authority must be refreshed before replace_state");
        assert!(error.message.contains("authority_denied"));
        assert_eq!(fs::read(state.join("save.bin"))?, b"old-state");

        let current = context(
            "grant-live",
            &created.record.installation_id,
            expires_at_ms,
            test_authority_refresh(usize::MAX, Duration::ZERO),
        );
        let result: InstallationMutationResult = serde_json::from_value(
            runtime
                .call_protocol(&current, "host.installation.update", params)
                .await
                .map_err(|error| anyhow!("{}: {}", error.code, error.message))?,
        )?;
        assert_eq!(result.installation.revision, created.revision + 1);
        assert_eq!(result.receipts.len(), 2);
        assert!(fs::read_dir(&state)?.next().is_none());
        for descriptor in &result.receipts {
            let request: plurora_runtime::ObjectGetRequest = serde_json::from_value(
                serde_json::json!({"installation_state_artifact": descriptor}),
            )?;
            let fetched = runtime.get_object(request).await?;
            let fetched_wire = serde_json::to_value(&fetched)?;
            assert!(fetched_wire
                .as_object()
                .is_some_and(|value| value.len() == 3));
            assert!(fetched_wire.get("descriptor").is_some());
            assert!(fetched_wire.get("content").is_some());
            assert!(fetched_wire.get("content_encoding").is_some());
            let plurora_runtime::ObjectGetResponse::InstallationStateArtifact(fetched) = fetched
            else {
                bail!("state selector returned the ordinary Asset response shape");
            };
            assert_eq!(fetched.descriptor, *descriptor);
        }
        let mut tampered = result.receipts[0].clone();
        tampered
            .annotations
            .insert("forged".to_string(), serde_json::Value::Bool(true));
        assert!(runtime
            .get_object(
                plurora_runtime::ObjectGetRequest::InstallationStateArtifact(
                    plurora_runtime::InstallationStateArtifactGetParams {
                        installation_state_artifact: tampered,
                    },
                )
            )
            .await
            .is_err());

        let evidence_descriptor = result
            .receipts
            .iter()
            .find(|descriptor| {
                descriptor.artifact_type_uri == INSTALLATION_STATE_AUTHORITY_EVIDENCE_TYPE_URI
            })
            .expect("Host authority evidence descriptor");
        let receipt_descriptor = result
            .receipts
            .iter()
            .find(|descriptor| {
                descriptor.artifact_type_uri == INSTALLATION_STATE_RESET_RECEIPT_TYPE_URI
            })
            .expect("Host reset decision receipt descriptor");
        assert_eq!(
            receipt_descriptor.references,
            [evidence_descriptor.digest.clone()]
        );
        let evidence: InstallationStateAuthorityEvidence =
            serde_json::from_slice(&fixture.objects.get(&evidence_descriptor.digest).await?)?;
        assert_eq!(
            evidence.schema,
            INSTALLATION_STATE_AUTHORITY_EVIDENCE_SCHEMA
        );
        assert_eq!(evidence.action, "installation.manage");
        assert_eq!(evidence.installation_id, created.record.installation_id);
        assert_eq!(evidence.grant_id.as_deref(), Some("grant-live"));
        assert_eq!(evidence.expires_at_ms, Some(expires_at_ms));
        let receipt: InstallationStateDecisionReceipt =
            serde_json::from_slice(&fixture.objects.get(&receipt_descriptor.digest).await?)?;
        assert_eq!(receipt.schema, INSTALLATION_STATE_RESET_RECEIPT_SCHEMA);
        assert_eq!(receipt.installation_id, created.record.installation_id);
        assert_eq!(receipt.expected_revision, created.revision);
        assert_eq!(
            receipt.candidate_work_digest,
            created.record.work_revision.digest
        );
        assert_eq!(
            receipt.candidate_lock_digest,
            created.record.assembly_lock.digest
        );
        assert_eq!(receipt.replacement_snapshot_digest, None);
        assert_eq!(receipt.operation, INSTALLATION_STATE_OPERATION);
        assert_eq!(receipt.action, InstallationStateDecisionAction::Reset);
        assert_eq!(receipt.decision, InstallationStateDecision::Allow);
        assert_eq!(
            receipt.authority_evidence,
            vec![evidence_descriptor.clone()]
        );

        let replacement_snapshot = StateSnapshot {
            schema: INSTALLATION_STATE_SNAPSHOT_SCHEMA.to_string(),
            entries: vec![StateSnapshotEntry {
                path: "nested/restored.bin".to_string(),
                bytes: vec![0, 1, 2, 255],
            }],
        };
        let replacement_descriptor = put_json(
            &fixture.objects,
            INSTALLATION_STATE_SNAPSHOT_TYPE_URI,
            &replacement_snapshot,
        )
        .await?;
        let mut replacement_request = update_request(
            &result.installation,
            result.installation.record.work_revision.clone(),
            result.installation.record.assembly_lock.clone(),
            "authorized-replace",
        );
        replacement_request.state_action = InstallationStateAction::Replace {
            replacement_snapshot: replacement_descriptor.clone(),
        };
        let replacement_result: InstallationMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &current,
                    "host.installation.update",
                    serde_json::to_value(&replacement_request)?,
                )
                .await
                .map_err(|error| anyhow!("{}: {}", error.code, error.message))?,
        )?;
        assert_eq!(replacement_result.receipts.len(), 2);
        assert_eq!(fs::read(state.join("nested/restored.bin"))?, [0, 1, 2, 255]);
        let replacement_evidence = replacement_result
            .receipts
            .iter()
            .find(|descriptor| {
                descriptor.artifact_type_uri == INSTALLATION_STATE_AUTHORITY_EVIDENCE_TYPE_URI
            })
            .expect("Host replacement authority evidence descriptor");
        let replacement_decision_descriptor = replacement_result
            .receipts
            .iter()
            .find(|descriptor| {
                descriptor.artifact_type_uri
                    == INSTALLATION_STATE_REPLACEMENT_DECISION_RECEIPT_TYPE_URI
            })
            .expect("Host replacement decision receipt descriptor");
        assert_eq!(
            replacement_decision_descriptor.references,
            [replacement_evidence.digest.clone()]
        );
        let replacement_decision: InstallationStateDecisionReceipt = serde_json::from_slice(
            &fixture
                .objects
                .get(&replacement_decision_descriptor.digest)
                .await?,
        )?;
        assert_eq!(
            replacement_decision.schema,
            INSTALLATION_STATE_REPLACEMENT_DECISION_RECEIPT_SCHEMA
        );
        assert_eq!(
            replacement_decision.installation_id,
            created.record.installation_id
        );
        assert_eq!(
            replacement_decision.expected_revision,
            result.installation.revision
        );
        assert_eq!(
            replacement_decision.candidate_work_digest,
            result.installation.record.work_revision.digest
        );
        assert_eq!(
            replacement_decision.candidate_lock_digest,
            result.installation.record.assembly_lock.digest
        );
        assert_eq!(
            replacement_decision.replacement_snapshot_digest.as_deref(),
            Some(replacement_descriptor.digest.as_str())
        );
        assert_eq!(replacement_decision.operation, INSTALLATION_STATE_OPERATION);
        assert_eq!(
            replacement_decision.action,
            InstallationStateDecisionAction::Replace
        );
        assert_eq!(
            replacement_decision.decision,
            InstallationStateDecision::Allow
        );
        assert_eq!(
            replacement_decision.authority_evidence,
            vec![replacement_evidence.clone()]
        );
        for descriptor in &replacement_result.receipts {
            let fetched = runtime
                .get_object(
                    plurora_runtime::ObjectGetRequest::InstallationStateArtifact(
                        plurora_runtime::InstallationStateArtifactGetParams {
                            installation_state_artifact: descriptor.clone(),
                        },
                    ),
                )
                .await?;
            let plurora_runtime::ObjectGetResponse::InstallationStateArtifact(fetched) = fetched
            else {
                bail!("state selector returned the ordinary Asset response shape");
            };
            assert_eq!(fetched.descriptor, *descriptor);
        }

        let restarted = InstallationRegistry::persistent(
            fixture.store.clone(),
            fixture.objects.clone(),
            fixture._data.path(),
        )?;
        assert_eq!(restarted.hydrate().await?, 1);
        let restarted_runtime = Runtime::new(
            fixture.store.clone(),
            RuntimeConfig {
                object_store: fixture.objects.clone(),
                installation_control: restarted.clone(),
                ..RuntimeConfig::default()
            },
        );
        for descriptor in result
            .receipts
            .iter()
            .chain(replacement_result.receipts.iter())
        {
            let fetched = restarted_runtime
                .get_object(
                    plurora_runtime::ObjectGetRequest::InstallationStateArtifact(
                        plurora_runtime::InstallationStateArtifactGetParams {
                            installation_state_artifact: descriptor.clone(),
                        },
                    ),
                )
                .await?;
            let plurora_runtime::ObjectGetResponse::InstallationStateArtifact(fetched) = fetched
            else {
                bail!("state selector returned the ordinary Asset response shape");
            };
            assert_eq!(fetched.descriptor, *descriptor);
        }
        let replay: InstallationMutationResult = serde_json::from_value(
            restarted_runtime
                .call_protocol(
                    &ProtocolContext::host_dev("installation-state-replay-test"),
                    "host.installation.update",
                    serde_json::to_value(replacement_request)?,
                )
                .await
                .map_err(|error| anyhow!("{}: {}", error.code, error.message))?,
        )?;
        assert!(replay.idempotent);
        assert_eq!(replay.receipts, replacement_result.receipts);
        Ok(())
    }

    #[test]
    fn installation_wire_rejects_client_supplied_receipts_and_authority() {
        let base = serde_json::json!({
            "installation_id": InstallationId::new(),
            "expected_revision": 1,
            "work_revision": missing_descriptor(WORK_REVISION_TYPE_URI, 'a'),
            "assembly_lock": missing_descriptor(ASSEMBLY_LOCK_TYPE_URI, 'b'),
            "state_action": {"kind": "reset"},
            "idempotency_key": "wire-reject"
        });
        for forged in [
            serde_json::json!({"approval_ref": missing_descriptor("urn:test:receipt", 'c')}),
            serde_json::json!({"migration_receipt": missing_descriptor("urn:test:receipt", 'd')}),
            serde_json::json!({"authority": {"grant_id": "forged"}}),
        ] {
            let mut value = base.clone();
            value
                .as_object_mut()
                .unwrap()
                .extend(forged.as_object().unwrap().clone());
            assert!(serde_json::from_value::<InstallationUpdateRequest>(value).is_err());
        }
        assert!(
            serde_json::from_value::<InstallationRemoveRequest>(serde_json::json!({
                "installation_id": InstallationId::new(),
                "expected_revision": 1,
                "state_disposition": "delete",
                "idempotency_key": "wire-remove-reject",
                "authority": {"grant_id": "forged"}
            }))
            .is_err()
        );
    }

    async fn recovery_fixture_receipts(
        registry: &InstallationRegistry,
        view: &InstallationView,
    ) -> anyhow::Result<Vec<ArtifactDescriptor>> {
        let evidence = registry
            .put_state_artifact(
                INSTALLATION_STATE_AUTHORITY_EVIDENCE_TYPE_URI,
                INSTALLATION_STATE_AUTHORITY_EVIDENCE_MEDIA_TYPE,
                Vec::new(),
                &InstallationStateAuthorityEvidence {
                    schema: INSTALLATION_STATE_AUTHORITY_EVIDENCE_SCHEMA.to_string(),
                    action: "installation.manage".to_string(),
                    installation_id: view.record.installation_id.clone(),
                    grant_id: None,
                    expires_at_ms: None,
                },
                "recovery fixture authority evidence",
            )
            .await?;
        let receipt = registry
            .put_state_artifact(
                INSTALLATION_STATE_RESET_RECEIPT_TYPE_URI,
                INSTALLATION_STATE_RECEIPT_MEDIA_TYPE,
                vec![evidence.digest.clone()],
                &InstallationStateDecisionReceipt {
                    schema: INSTALLATION_STATE_RESET_RECEIPT_SCHEMA.to_string(),
                    installation_id: view.record.installation_id.clone(),
                    expected_revision: view.revision,
                    candidate_work_digest: view.record.work_revision.digest.clone(),
                    candidate_lock_digest: view.record.assembly_lock.digest.clone(),
                    replacement_snapshot_digest: None,
                    operation: INSTALLATION_STATE_OPERATION.to_string(),
                    action: InstallationStateDecisionAction::Reset,
                    decision: InstallationStateDecision::Allow,
                    authority_evidence: vec![evidence.clone()],
                },
                "recovery fixture state decision receipt",
            )
            .await?;
        let mut receipts = vec![evidence, receipt];
        receipts.sort_by(|left, right| {
            left.digest
                .cmp(&right.digest)
                .then(left.artifact_type_uri.cmp(&right.artifact_type_uri))
        });
        validate_state_receipt_descriptors(&receipts)?;
        Ok(receipts)
    }

    #[tokio::test]
    async fn remove_keep_and_delete_only_touch_host_owned_state() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let runtime = Runtime::new(
            fixture.store.clone(),
            RuntimeConfig {
                object_store: fixture.objects.clone(),
                installation_control: fixture.registry.clone(),
                ..RuntimeConfig::default()
            },
        );
        let keep = fixture.create(fixture.request.clone()).await?.installation;
        let keep_state = fixture.registry.state_dir(&keep.record.installation_id);
        fs::write(keep_state.join("keep.txt"), b"keep")?;
        let missing_authority = fixture
            .registry
            .remove(InstallationRemoveRequest {
                installation_id: keep.record.installation_id.clone(),
                expected_revision: keep.revision,
                state_disposition: StateDisposition::Keep,
                idempotency_key: "remove-keep-missing-authority".to_string(),
                authority: None,
            })
            .await
            .expect_err("remove must fail closed without the Host-only authority sidecar");
        assert!(missing_authority.to_string().contains("authority_denied"));
        assert_eq!(
            fixture.registry.get(&keep.record.installation_id).await?,
            Some(keep.clone())
        );
        assert_eq!(fs::read(keep_state.join("keep.txt"))?, b"keep");
        let kept: InstallationMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("installation-remove-state-test"),
                    "host.installation.remove",
                    serde_json::to_value(InstallationRemoveRequest {
                        installation_id: keep.record.installation_id.clone(),
                        expected_revision: keep.revision,
                        state_disposition: StateDisposition::Keep,
                        idempotency_key: "remove-keep".to_string(),
                        authority: None,
                    })?,
                )
                .await
                .map_err(|error| anyhow!(error.message))?,
        )?;
        assert_eq!(kept.installation.record.status, InstallationStatus::Removed);
        assert_eq!(fs::read(keep_state.join("keep.txt"))?, b"keep");

        let mut second_request = fixture.request.clone();
        second_request.idempotency_key = "create-delete".to_string();
        second_request.display_name = "Delete".to_string();
        let delete = fixture.create(second_request).await?.installation;
        let delete_state = fixture.registry.state_dir(&delete.record.installation_id);
        fs::write(delete_state.join("delete.txt"), b"delete")?;
        let external_workspace = fixture._data.path().join("workspaces-user-owned");
        fs::create_dir(&external_workspace)?;
        fs::write(external_workspace.join("keep.txt"), b"outside")?;
        runtime
            .call_protocol(
                &ProtocolContext::host_dev("installation-remove-state-test"),
                "host.installation.remove",
                serde_json::to_value(InstallationRemoveRequest {
                    installation_id: delete.record.installation_id,
                    expected_revision: delete.revision,
                    state_disposition: StateDisposition::Delete,
                    idempotency_key: "remove-delete".to_string(),
                    authority: None,
                })?,
            )
            .await
            .map_err(|error| anyhow!(error.message))?;
        assert!(fs::read_dir(delete_state)?.next().is_none());
        assert_eq!(fs::read(external_workspace.join("keep.txt"))?, b"outside");
        Ok(())
    }

    #[tokio::test]
    async fn remove_refreshes_device_authority_after_waiting_for_apply_lock() -> anyhow::Result<()>
    {
        async fn create_named(fixture: &Fixture, name: &str) -> anyhow::Result<InstallationView> {
            let mut request = fixture.request.clone();
            request.display_name = name.to_string();
            request.idempotency_key = format!("remove-race-create-{name}");
            Ok(fixture.create(request).await?.installation)
        }

        fn context(
            installation_id: &InstallationId,
            expires_at_ms: i64,
            active: Arc<AtomicBool>,
        ) -> ProtocolContext {
            ProtocolContext::host_device(
                "grant-remove-race",
                vec!["installation.manage".to_string()],
                vec![ProtocolResourceSelector {
                    owner: "host".to_string(),
                    kind: "installation".to_string(),
                    id: Some(installation_id.to_string()),
                }],
                Vec::new(),
                "remove-authority-race",
            )
            .with_verified_authority_expiry(Some(expires_at_ms))
            .with_installation_authority_refresh(InstallationAuthorityRefresh::new(Arc::new(
                ToggleAuthorityValidator {
                    active,
                    grant_id: "grant-remove-race".to_string(),
                },
            )))
        }

        let fixture = fixture().await?;
        let runtime = Arc::new(Runtime::new(
            fixture.store.clone(),
            RuntimeConfig {
                object_store: fixture.objects.clone(),
                installation_control: fixture.registry.clone(),
                ..RuntimeConfig::default()
            },
        ));

        let delete = create_named(&fixture, "delete-revoked").await?;
        let delete_state = fixture.registry.state_dir(&delete.record.installation_id);
        fs::write(delete_state.join("save.bin"), b"must-survive-revocation")?;
        let delete_active = Arc::new(AtomicBool::new(true));
        let delete_context = context(
            &delete.record.installation_id,
            Utc::now().timestamp_millis() + 60_000,
            delete_active.clone(),
        );
        let delete_params = serde_json::to_value(InstallationRemoveRequest {
            installation_id: delete.record.installation_id.clone(),
            expected_revision: delete.revision,
            state_disposition: StateDisposition::Delete,
            idempotency_key: "remove-race-delete".to_string(),
            authority: None,
        })?;
        let held = fixture.registry.apply.lock().await;
        let delete_runtime = runtime.clone();
        let delete_task = tokio::spawn(async move {
            delete_runtime
                .call_protocol(&delete_context, "host.installation.remove", delete_params)
                .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !delete_task.is_finished(),
            "remove must wait for the apply lock"
        );
        delete_active.store(false, Ordering::SeqCst);
        drop(held);
        let error = delete_task
            .await?
            .expect_err("revocation while waiting for the apply lock must fail closed");
        assert!(error.message.contains("authority_denied"));
        assert_eq!(
            fs::read(delete_state.join("save.bin"))?,
            b"must-survive-revocation"
        );
        assert_eq!(
            fixture.registry.get(&delete.record.installation_id).await?,
            Some(delete)
        );

        let keep = create_named(&fixture, "keep-expired").await?;
        let keep_state = fixture.registry.state_dir(&keep.record.installation_id);
        fs::write(keep_state.join("save.bin"), b"must-survive-expiry")?;
        let keep_active = Arc::new(AtomicBool::new(true));
        let keep_context = context(
            &keep.record.installation_id,
            Utc::now().timestamp_millis() + 200,
            keep_active,
        );
        let keep_params = serde_json::to_value(InstallationRemoveRequest {
            installation_id: keep.record.installation_id.clone(),
            expected_revision: keep.revision,
            state_disposition: StateDisposition::Keep,
            idempotency_key: "remove-race-keep".to_string(),
            authority: None,
        })?;
        let held = fixture.registry.apply.lock().await;
        let keep_runtime = runtime.clone();
        let keep_task = tokio::spawn(async move {
            keep_runtime
                .call_protocol(&keep_context, "host.installation.remove", keep_params)
                .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !keep_task.is_finished(),
            "remove must wait for the apply lock"
        );
        tokio::time::sleep(Duration::from_millis(220)).await;
        drop(held);
        let error = keep_task
            .await?
            .expect_err("expiry while waiting for the apply lock must fail closed");
        assert!(error.message.contains("authority_denied"));
        assert_eq!(
            fs::read(keep_state.join("save.bin"))?,
            b"must-survive-expiry"
        );
        assert_eq!(
            fixture.registry.get(&keep.record.installation_id).await?,
            Some(keep)
        );

        let terminal = create_named(&fixture, "terminal-boundary-revoked").await?;
        let terminal_state = fixture.registry.state_dir(&terminal.record.installation_id);
        fs::write(terminal_state.join("save.bin"), b"terminal-boundary-state")?;
        let terminal_projection = fixture
            .registry
            .installation_dir(&terminal.record.installation_id)
            .join("installation.json");
        let terminal_projection_before = fs::read(&terminal_projection)?;
        let terminal_journal_before = fixture
            .store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?
            .len();
        let terminal_active = Arc::new(AtomicBool::new(true));
        let terminal_entered = Arc::new(tokio::sync::Barrier::new(2));
        let terminal_release = Arc::new(tokio::sync::Barrier::new(2));
        let terminal_context = ProtocolContext::host_device(
            "grant-remove-terminal-boundary",
            vec!["installation.manage".to_string()],
            vec![ProtocolResourceSelector {
                owner: "host".to_string(),
                kind: "installation".to_string(),
                id: Some(terminal.record.installation_id.to_string()),
            }],
            Vec::new(),
            "remove-terminal-boundary",
        )
        .with_verified_authority_expiry(Some(Utc::now().timestamp_millis() + 60_000))
        .with_installation_authority_refresh(InstallationAuthorityRefresh::new(Arc::new(
            BoundaryAuthorityValidator {
                active: terminal_active.clone(),
                grant_id: "grant-remove-terminal-boundary".to_string(),
                calls: AtomicUsize::new(0),
                boundary_call: 2,
                entered: terminal_entered.clone(),
                release: terminal_release.clone(),
            },
        )));
        let terminal_params = serde_json::to_value(InstallationRemoveRequest {
            installation_id: terminal.record.installation_id.clone(),
            expected_revision: terminal.revision,
            state_disposition: StateDisposition::Keep,
            idempotency_key: "remove-terminal-boundary".to_string(),
            authority: None,
        })?;
        let terminal_runtime = runtime.clone();
        let terminal_task = tokio::spawn(async move {
            terminal_runtime
                .call_protocol(
                    &terminal_context,
                    "host.installation.remove",
                    terminal_params,
                )
                .await
        });
        terminal_entered.wait().await;
        assert!(!terminal_task.is_finished());
        terminal_active.store(false, Ordering::SeqCst);
        terminal_release.wait().await;
        let error = terminal_task
            .await?
            .expect_err("revocation at the terminal append boundary must fail closed");
        assert!(error.message.contains("authority_denied"));
        assert_eq!(
            fixture
                .store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            terminal_journal_before
        );
        assert_eq!(
            fixture
                .registry
                .get(&terminal.record.installation_id)
                .await?,
            Some(terminal.clone())
        );
        assert_eq!(
            fs::read(terminal_state.join("save.bin"))?,
            b"terminal-boundary-state"
        );
        assert_eq!(fs::read(terminal_projection)?, terminal_projection_before);

        let legal = create_named(&fixture, "legal-replay").await?;
        let legal_state = fixture.registry.state_dir(&legal.record.installation_id);
        fs::write(legal_state.join("save.bin"), b"replay-must-not-touch-state")?;
        let legal_params = serde_json::to_value(InstallationRemoveRequest {
            installation_id: legal.record.installation_id.clone(),
            expected_revision: legal.revision,
            state_disposition: StateDisposition::Keep,
            idempotency_key: "remove-race-legal".to_string(),
            authority: None,
        })?;
        let legal_context = context(
            &legal.record.installation_id,
            Utc::now().timestamp_millis() + 60_000,
            Arc::new(AtomicBool::new(true)),
        );
        let first: InstallationMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &legal_context,
                    "host.installation.remove",
                    legal_params.clone(),
                )
                .await
                .map_err(|error| anyhow!(error.message))?,
        )?;
        let projection_root = fixture
            .registry
            .installation_dir(&legal.record.installation_id);
        let projection_record = projection_root.join("installation.json");
        let projection_lock = projection_root.join("assembly.lock.json");
        fs::remove_file(&projection_record)?;
        fs::remove_file(&projection_lock)?;
        let journal_after_first = fixture
            .store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?
            .len();
        let replay: InstallationMutationResult = serde_json::from_value(
            runtime
                .call_protocol(&legal_context, "host.installation.remove", legal_params)
                .await
                .map_err(|error| anyhow!(error.message))?,
        )?;
        assert!(!first.idempotent);
        assert!(replay.idempotent);
        assert_eq!(first.installation, replay.installation);
        assert!(!projection_record.exists());
        assert!(!projection_lock.exists());
        assert_eq!(
            fs::read(legal_state.join("save.bin"))?,
            b"replay-must-not-touch-state"
        );
        assert_eq!(
            fixture
                .store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            journal_after_first
        );

        let boundary_active = Arc::new(AtomicBool::new(true));
        let boundary_entered = Arc::new(tokio::sync::Barrier::new(2));
        let boundary_release = Arc::new(tokio::sync::Barrier::new(2));
        let boundary_context = ProtocolContext::host_device(
            "grant-remove-boundary",
            vec!["installation.manage".to_string()],
            vec![ProtocolResourceSelector {
                owner: "host".to_string(),
                kind: "installation".to_string(),
                id: Some(legal.record.installation_id.to_string()),
            }],
            Vec::new(),
            "remove-authority-boundary",
        )
        .with_verified_authority_expiry(Some(Utc::now().timestamp_millis() + 60_000))
        .with_installation_authority_refresh(InstallationAuthorityRefresh::new(Arc::new(
            BoundaryAuthorityValidator {
                active: boundary_active.clone(),
                grant_id: "grant-remove-boundary".to_string(),
                calls: AtomicUsize::new(0),
                boundary_call: 2,
                entered: boundary_entered.clone(),
                release: boundary_release.clone(),
            },
        )));
        let noop_params = serde_json::to_value(InstallationRemoveRequest {
            installation_id: legal.record.installation_id.clone(),
            expected_revision: first.installation.revision,
            state_disposition: StateDisposition::Delete,
            idempotency_key: "remove-race-noop-boundary".to_string(),
            authority: None,
        })?;
        let boundary_runtime = runtime.clone();
        let denied_task = tokio::spawn(async move {
            boundary_runtime
                .call_protocol(
                    &boundary_context,
                    "host.installation.remove",
                    noop_params.clone(),
                )
                .await
                .map(|value| (value, noop_params))
        });
        boundary_entered.wait().await;
        assert!(!denied_task.is_finished());
        boundary_active.store(false, Ordering::SeqCst);
        boundary_release.wait().await;
        let error = denied_task
            .await?
            .expect_err("revocation at the no-op append boundary must fail closed");
        assert!(error.message.contains("authority_denied"));
        assert_eq!(
            fixture
                .store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            journal_after_first
        );
        assert!(!projection_record.exists());
        assert!(!projection_lock.exists());
        assert_eq!(
            fs::read(legal_state.join("save.bin"))?,
            b"replay-must-not-touch-state"
        );
        assert_eq!(
            fixture.registry.get(&legal.record.installation_id).await?,
            Some(first.installation.clone())
        );

        let legal_noop_params = serde_json::to_value(InstallationRemoveRequest {
            installation_id: legal.record.installation_id.clone(),
            expected_revision: first.installation.revision,
            state_disposition: StateDisposition::Delete,
            idempotency_key: "remove-race-noop-boundary".to_string(),
            authority: None,
        })?;
        let legal_noop: InstallationMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("remove-noop-after-revocation"),
                    "host.installation.remove",
                    legal_noop_params.clone(),
                )
                .await
                .map_err(|error| anyhow!(error.message))?,
        )?;
        assert!(legal_noop.idempotent);
        assert_eq!(legal_noop.installation, first.installation);
        let legal_noop_replay: InstallationMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("remove-noop-replay"),
                    "host.installation.remove",
                    legal_noop_params,
                )
                .await
                .map_err(|error| anyhow!(error.message))?,
        )?;
        assert!(legal_noop_replay.idempotent);
        let conflicting_noop = serde_json::to_value(InstallationRemoveRequest {
            installation_id: legal.record.installation_id.clone(),
            expected_revision: first.installation.revision,
            state_disposition: StateDisposition::Keep,
            idempotency_key: "remove-race-noop-boundary".to_string(),
            authority: None,
        })?;
        let conflict = runtime
            .call_protocol(
                &ProtocolContext::host_dev("remove-noop-conflict"),
                "host.installation.remove",
                conflicting_noop,
            )
            .await
            .expect_err("same key with a different remove fingerprint must conflict");
        assert!(conflict.message.contains("idempotency_conflict"));
        Ok(())
    }

    #[tokio::test]
    async fn create_and_preserve_update_refresh_authority_after_apply_lock_wait(
    ) -> anyhow::Result<()> {
        fn device_context(
            grant_id: &str,
            kind: &str,
            id: String,
            expires_at_ms: i64,
            active: Arc<AtomicBool>,
        ) -> ProtocolContext {
            ProtocolContext::host_device(
                grant_id,
                vec!["installation.manage".to_string()],
                vec![ProtocolResourceSelector {
                    owner: "host".to_string(),
                    kind: kind.to_string(),
                    id: Some(id),
                }],
                Vec::new(),
                "durable-mutation-authority-race",
            )
            .with_verified_authority_expiry(Some(expires_at_ms))
            .with_installation_authority_refresh(InstallationAuthorityRefresh::new(Arc::new(
                ToggleAuthorityValidator {
                    active,
                    grant_id: grant_id.to_string(),
                },
            )))
        }

        let fixture = fixture().await?;
        let runtime = Arc::new(fixture.runtime());
        let installations_root = fixture._data.path().join("installations");

        let revoked = Arc::new(AtomicBool::new(true));
        let create_context = device_context(
            "grant-create-race",
            "work",
            fixture.request.work_id.to_string(),
            Utc::now().timestamp_millis() + 60_000,
            revoked.clone(),
        );
        let create_params = serde_json::to_value(&fixture.request)?;
        let held = fixture.registry.apply.lock().await;
        let create_runtime = runtime.clone();
        let create_task = tokio::spawn(async move {
            create_runtime
                .call_protocol(&create_context, "host.installation.create", create_params)
                .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !create_task.is_finished(),
            "create must wait for the apply lock"
        );
        revoked.store(false, Ordering::SeqCst);
        drop(held);
        let error = create_task
            .await?
            .expect_err("revoked create authority must fail after acquiring the apply lock");
        assert!(error.message.contains("authority_denied"));
        assert!(!error.message.contains(fixture.request.work_id.as_str()));
        assert!(!error
            .message
            .contains(fixture.request.work_revision.digest.as_str()));
        assert!(fixture
            .store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?
            .is_empty());
        assert!(fs::read_dir(&installations_root)?.next().is_none());

        let mut mismatched = fixture.request.clone();
        mismatched.work_id = WorkId::parse("tests/not-canonical-work")?;
        mismatched.idempotency_key = "create-work-id-mismatch".to_string();
        let mismatch = runtime
            .call_protocol(
                &ProtocolContext::host_dev("create-work-id-mismatch"),
                "host.installation.create",
                serde_json::to_value(mismatched)?,
            )
            .await
            .expect_err("request WorkId must match canonical WorkRevision content");
        assert!(mismatch.message.contains("work_id_mismatch"));
        assert!(fixture
            .store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?
            .is_empty());
        assert!(fs::read_dir(&installations_root)?.next().is_none());

        let wrong_work = device_context(
            "grant-wrong-work",
            "work",
            "tests/other-work".to_string(),
            Utc::now().timestamp_millis() + 60_000,
            Arc::new(AtomicBool::new(true)),
        );
        let denied = runtime
            .call_protocol(
                &wrong_work,
                "host.installation.create",
                serde_json::to_value(&fixture.request)?,
            )
            .await
            .expect_err("a grant for another Work must not create this Installation");
        assert_eq!(denied.code, "runtime/error/permission_denied");

        let legal_create = device_context(
            "grant-create-legal",
            "work",
            fixture.request.work_id.to_string(),
            Utc::now().timestamp_millis() + 60_000,
            Arc::new(AtomicBool::new(true)),
        );
        let first: InstallationMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &legal_create,
                    "host.installation.create",
                    serde_json::to_value(&fixture.request)?,
                )
                .await
                .map_err(|error| anyhow!(error.message))?,
        )?;
        let replay: InstallationMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &legal_create,
                    "host.installation.create",
                    serde_json::to_value(&fixture.request)?,
                )
                .await
                .map_err(|error| anyhow!(error.message))?,
        )?;
        assert!(!first.idempotent && replay.idempotent);
        assert_eq!(first.installation, replay.installation);

        let (work, lock) = put_work_pair(&fixture.objects, "preserve-race", Vec::new()).await?;
        let mut preserve = update_request(
            &first.installation,
            work,
            lock,
            "preserve-expired-after-dispatch",
        );
        preserve.display_name = Some("must not become active".to_string());
        let expires_while_waiting = device_context(
            "grant-preserve-expiry",
            "installation",
            first.installation.record.installation_id.to_string(),
            Utc::now().timestamp_millis() + 100,
            Arc::new(AtomicBool::new(true)),
        );
        let preserve_params = serde_json::to_value(&preserve)?;
        let held = fixture.registry.apply.lock().await;
        let preserve_runtime = runtime.clone();
        let preserve_task = tokio::spawn(async move {
            preserve_runtime
                .call_protocol(
                    &expires_while_waiting,
                    "host.installation.update",
                    preserve_params,
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !preserve_task.is_finished(),
            "Preserve update must wait for the apply lock"
        );
        tokio::time::sleep(Duration::from_millis(120)).await;
        drop(held);
        let error = preserve_task
            .await?
            .expect_err("expired Preserve authority must fail after acquiring the apply lock");
        assert!(error.message.contains("authority_denied"));
        assert_eq!(
            fixture
                .registry
                .get(&first.installation.record.installation_id)
                .await?,
            Some(first.installation.clone())
        );
        assert_eq!(
            fixture
                .store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            1
        );
        let projected: InstallationView = serde_json::from_slice(&fs::read(
            fixture
                .registry
                .installation_dir(&first.installation.record.installation_id)
                .join("installation.json"),
        )?)?;
        assert_eq!(projected, first.installation);

        preserve.idempotency_key = "preserve-legal-replay".to_string();
        preserve.display_name = Some("legally active".to_string());
        let legal_update = device_context(
            "grant-preserve-legal",
            "installation",
            first.installation.record.installation_id.to_string(),
            Utc::now().timestamp_millis() + 60_000,
            Arc::new(AtomicBool::new(true)),
        );
        let first_update: InstallationMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &legal_update,
                    "host.installation.update",
                    serde_json::to_value(&preserve)?,
                )
                .await
                .map_err(|error| anyhow!(error.message))?,
        )?;
        let update_projection_root = fixture
            .registry
            .installation_dir(&first.installation.record.installation_id);
        let update_projection_record = update_projection_root.join("installation.json");
        let update_projection_lock = update_projection_root.join("assembly.lock.json");
        fs::remove_file(&update_projection_record)?;
        fs::remove_file(&update_projection_lock)?;
        let replay_update: InstallationMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &legal_update,
                    "host.installation.update",
                    serde_json::to_value(&preserve)?,
                )
                .await
                .map_err(|error| anyhow!(error.message))?,
        )?;
        assert!(!first_update.idempotent && replay_update.idempotent);
        assert_eq!(first_update.installation, replay_update.installation);
        assert!(!update_projection_record.exists());
        assert!(!update_projection_lock.exists());
        Ok(())
    }

    #[tokio::test]
    async fn authority_append_barrier_blocks_create_and_update_commits_after_revoke_or_expiry(
    ) -> anyhow::Result<()> {
        fn context(
            grant_id: &str,
            kind: &str,
            id: String,
            expires_at_ms: i64,
            active: Arc<AtomicBool>,
        ) -> ProtocolContext {
            ProtocolContext::host_device(
                grant_id,
                vec!["installation.manage".to_string()],
                vec![ProtocolResourceSelector {
                    owner: "host".to_string(),
                    kind: kind.to_string(),
                    id: Some(id),
                }],
                Vec::new(),
                "authority-append-barrier",
            )
            .with_verified_authority_expiry(Some(expires_at_ms))
            .with_installation_authority_refresh(InstallationAuthorityRefresh::new(Arc::new(
                ToggleAuthorityValidator {
                    active,
                    grant_id: grant_id.to_string(),
                },
            )))
        }

        async fn wait_at_barrier(
            entered: &tokio::sync::Barrier,
            task: &tokio::task::JoinHandle<
                Result<serde_json::Value, plurora_runtime::ProtocolError>,
            >,
        ) -> anyhow::Result<()> {
            tokio::time::timeout(Duration::from_secs(5), entered.wait())
                .await
                .map_err(|_| anyhow!("authority append did not reach its post-owner barrier"))?;
            ensure!(
                !task.is_finished(),
                "mutation passed its authority append barrier"
            );
            Ok(())
        }

        let fixture = fixture().await?;
        let runtime = Arc::new(fixture.runtime());

        let create_active = Arc::new(AtomicBool::new(true));
        let create_entered = Arc::new(tokio::sync::Barrier::new(2));
        let create_release = Arc::new(tokio::sync::Barrier::new(2));
        fixture.registry.inject_authority_append_barrier(
            INSTALLATION_CREATED,
            create_entered.clone(),
            create_release.clone(),
        )?;
        let create_context = context(
            "grant-create-append",
            "work",
            fixture.request.work_id.to_string(),
            Utc::now().timestamp_millis() + 60_000,
            create_active.clone(),
        );
        let create_params = serde_json::to_value(&fixture.request)?;
        let create_runtime = runtime.clone();
        let create_task = tokio::spawn(async move {
            create_runtime
                .call_protocol(&create_context, "host.installation.create", create_params)
                .await
        });
        wait_at_barrier(&create_entered, &create_task).await?;
        create_active.store(false, Ordering::SeqCst);
        create_release.wait().await;
        let error = create_task
            .await?
            .expect_err("revoked Work grant must block the create terminal commit");
        assert!(
            error.message.contains("authority_denied"),
            "unexpected create error: {}",
            error.message
        );
        assert!(fixture
            .store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?
            .is_empty());
        assert!(fs::read_dir(fixture._data.path().join("installations"))?
            .next()
            .is_none());

        let created = fixture.create(fixture.request.clone()).await?.installation;
        let state = fixture.registry.state_dir(&created.record.installation_id);
        fs::write(state.join("save.bin"), b"original-state")?;
        let projection = fixture
            .registry
            .installation_dir(&created.record.installation_id)
            .join("installation.json");
        let projection_before = fs::read(&projection)?;

        let noop_active = Arc::new(AtomicBool::new(true));
        let noop_entered = Arc::new(tokio::sync::Barrier::new(2));
        let noop_release = Arc::new(tokio::sync::Barrier::new(2));
        fixture.registry.inject_authority_append_barrier(
            IDEMPOTENT_NOOP,
            noop_entered.clone(),
            noop_release.clone(),
        )?;
        let noop = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "noop-terminal-revoked",
        );
        let noop_context = context(
            "grant-noop-append",
            "installation",
            created.record.installation_id.to_string(),
            Utc::now().timestamp_millis() + 60_000,
            noop_active.clone(),
        );
        let noop_runtime = runtime.clone();
        let noop_task = tokio::spawn(async move {
            noop_runtime
                .call_protocol(
                    &noop_context,
                    "host.installation.update",
                    serde_json::to_value(noop).expect("serialize no-op request"),
                )
                .await
        });
        wait_at_barrier(&noop_entered, &noop_task).await?;
        noop_active.store(false, Ordering::SeqCst);
        noop_release.wait().await;
        let error = noop_task
            .await?
            .expect_err("revoked Installation grant must block the no-op claim");
        assert!(error.message.contains("authority_denied"));
        assert_eq!(
            fixture
                .registry
                .get(&created.record.installation_id)
                .await?,
            Some(created.clone())
        );
        assert_eq!(fs::read(state.join("save.bin"))?, b"original-state");
        assert_eq!(fs::read(&projection)?, projection_before);
        assert_eq!(
            fixture
                .store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            1
        );

        let preserve_entered = Arc::new(tokio::sync::Barrier::new(2));
        let preserve_release = Arc::new(tokio::sync::Barrier::new(2));
        fixture.registry.inject_authority_append_barrier(
            INSTALLATION_UPDATED,
            preserve_entered.clone(),
            preserve_release.clone(),
        )?;
        let mut preserve = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "preserve-terminal-expiry",
        );
        preserve.display_name = Some("must not commit".to_string());
        let preserve_context = context(
            "grant-preserve-append",
            "installation",
            created.record.installation_id.to_string(),
            Utc::now().timestamp_millis() + 100,
            Arc::new(AtomicBool::new(true)),
        );
        let preserve_runtime = runtime.clone();
        let preserve_task = tokio::spawn(async move {
            preserve_runtime
                .call_protocol(
                    &preserve_context,
                    "host.installation.update",
                    serde_json::to_value(preserve).expect("serialize Preserve request"),
                )
                .await
        });
        wait_at_barrier(&preserve_entered, &preserve_task).await?;
        tokio::time::sleep(Duration::from_millis(120)).await;
        preserve_release.wait().await;
        let error = preserve_task
            .await?
            .expect_err("expired Installation grant must block Preserve terminal commit");
        assert!(error.message.contains("authority_denied"));
        assert_eq!(
            fixture
                .registry
                .get(&created.record.installation_id)
                .await?,
            Some(created.clone())
        );
        assert_eq!(fs::read(state.join("save.bin"))?, b"original-state");
        assert_eq!(fs::read(&projection)?, projection_before);
        assert_eq!(
            fixture
                .store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            1
        );

        let start_active = Arc::new(AtomicBool::new(true));
        let start_entered = Arc::new(tokio::sync::Barrier::new(2));
        let start_release = Arc::new(tokio::sync::Barrier::new(2));
        fixture.registry.inject_authority_append_barrier(
            UPDATE_STARTED,
            start_entered.clone(),
            start_release.clone(),
        )?;
        let mut start_request = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "reset-start-revoked",
        );
        start_request.state_action = InstallationStateAction::Reset;
        let start_context = context(
            "grant-start-append",
            "installation",
            created.record.installation_id.to_string(),
            Utc::now().timestamp_millis() + 60_000,
            start_active.clone(),
        );
        let start_runtime = runtime.clone();
        let start_task = tokio::spawn(async move {
            start_runtime
                .call_protocol(
                    &start_context,
                    "host.installation.update",
                    serde_json::to_value(start_request).expect("serialize Reset request"),
                )
                .await
        });
        wait_at_barrier(&start_entered, &start_task).await?;
        start_active.store(false, Ordering::SeqCst);
        start_release.wait().await;
        let error = start_task
            .await?
            .expect_err("revoked grant must block UPDATE_STARTED");
        assert!(error.message.contains("authority_denied"));
        assert_eq!(fs::read(state.join("save.bin"))?, b"original-state");
        assert_eq!(fs::read(&projection)?, projection_before);
        assert_eq!(
            fixture
                .store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            1
        );

        let terminal_active = Arc::new(AtomicBool::new(true));
        let terminal_entered = Arc::new(tokio::sync::Barrier::new(2));
        let terminal_release = Arc::new(tokio::sync::Barrier::new(2));
        fixture.registry.inject_authority_append_barrier(
            INSTALLATION_UPDATED,
            terminal_entered.clone(),
            terminal_release.clone(),
        )?;
        let mut terminal_request = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "reset-terminal-revoked",
        );
        terminal_request.state_action = InstallationStateAction::Reset;
        let terminal_context = context(
            "grant-terminal-append",
            "installation",
            created.record.installation_id.to_string(),
            Utc::now().timestamp_millis() + 60_000,
            terminal_active.clone(),
        );
        let terminal_runtime = runtime.clone();
        let terminal_task = tokio::spawn(async move {
            terminal_runtime
                .call_protocol(
                    &terminal_context,
                    "host.installation.update",
                    serde_json::to_value(terminal_request).expect("serialize Reset request"),
                )
                .await
        });
        wait_at_barrier(&terminal_entered, &terminal_task).await?;
        assert!(fs::read_dir(&state)?.next().is_none());
        terminal_active.store(false, Ordering::SeqCst);
        terminal_release.wait().await;
        let error = terminal_task
            .await?
            .expect_err("revoked grant must block the update terminal commit");
        assert!(error.message.contains("authority_denied"));
        assert_eq!(
            fixture
                .registry
                .get(&created.record.installation_id)
                .await?,
            Some(created.clone())
        );
        assert_eq!(fs::read(state.join("save.bin"))?, b"original-state");
        assert_eq!(fs::read(&projection)?, projection_before);
        let journal = fixture
            .store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?;
        assert_eq!(journal.len(), 3);
        assert_eq!(journal[1].kind, UPDATE_STARTED);
        assert_eq!(journal[2].kind, UPDATE_ROLLBACK);
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_lifecycle_lookup_returns_one_stable_lock_per_installation(
    ) -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let installation_id = InstallationId::new();
        let start = Arc::new(tokio::sync::Barrier::new(3));

        let left_registry = fixture.registry.clone();
        let left_id = installation_id.clone();
        let left_start = start.clone();
        let left = tokio::spawn(async move {
            left_start.wait().await;
            left_registry.lifecycle_lock(&left_id)
        });
        let right_registry = fixture.registry.clone();
        let right_id = installation_id.clone();
        let right_start = start.clone();
        let right = tokio::spawn(async move {
            right_start.wait().await;
            right_registry.lifecycle_lock(&right_id)
        });

        start.wait().await;
        let left = left.await??;
        let right = right.await??;
        assert!(Arc::ptr_eq(&left, &right));
        let lifecycles = fixture.registry.lifecycles.lock().map_err(lock_error)?;
        assert_eq!(lifecycles.len(), 1);
        let stored = lifecycles
            .get(&installation_id)
            .and_then(Weak::upgrade)
            .expect("concurrent lookup installed a live lifecycle lock");
        assert!(Arc::ptr_eq(&stored, &left,));
        Ok(())
    }

    #[tokio::test]
    async fn terminal_append_wait_on_one_installation_does_not_block_another() -> anyhow::Result<()>
    {
        let fixture = fixture().await?;
        let first = fixture.create(fixture.request.clone()).await?.installation;
        let mut second_create = fixture.request.clone();
        second_create.display_name = "Second".to_string();
        second_create.idempotency_key = "create-second-terminal-isolation".to_string();
        let second = fixture.create(second_create).await?.installation;

        let entered = Arc::new(tokio::sync::Barrier::new(2));
        let release = Arc::new(tokio::sync::Barrier::new(2));
        fixture.registry.inject_authority_append_barrier(
            INSTALLATION_UPDATED,
            entered.clone(),
            release.clone(),
        )?;
        let mut first_request = update_request(
            &first,
            first.record.work_revision.clone(),
            first.record.assembly_lock.clone(),
            "first-terminal-isolation",
        );
        first_request.display_name = Some("First committed later".to_string());
        let first_runtime = fixture.runtime();
        let first_task = tokio::spawn(async move {
            first_runtime
                .call_protocol(
                    &ProtocolContext::host_dev("first-terminal-isolation"),
                    "host.installation.update",
                    serde_json::to_value(first_request).expect("serialize first update"),
                )
                .await
        });
        tokio::time::timeout(Duration::from_secs(5), entered.wait())
            .await
            .map_err(|_| anyhow!("first update did not reach its terminal append boundary"))?;
        ensure!(
            !first_task.is_finished(),
            "first update passed the injected terminal append boundary"
        );

        let second_run = tokio::time::timeout(
            Duration::from_secs(5),
            fixture
                .registry
                .acquire_ready_for_run(&second.record.installation_id, second.revision),
        )
        .await
        .map_err(|_| {
            anyhow!("another Installation Run waited on the terminal append boundary")
        })??;
        drop(second_run);

        let mut second_request = update_request(
            &second,
            second.record.work_revision.clone(),
            second.record.assembly_lock.clone(),
            "second-during-first-terminal",
        );
        second_request.display_name = Some("Second committed first".to_string());
        let second_value = tokio::time::timeout(
            Duration::from_secs(5),
            fixture.runtime().call_protocol(
                &ProtocolContext::host_dev("second-during-first-terminal"),
                "host.installation.update",
                serde_json::to_value(second_request)?,
            ),
        )
        .await
        .map_err(|_| anyhow!("another Installation update waited on the terminal append boundary"))?
        .map_err(|error| anyhow!("{}: {}", error.code, error.message))?;
        let second_updated: InstallationMutationResult = serde_json::from_value(second_value)?;
        assert_eq!(
            second_updated.installation.record.display_name,
            "Second committed first"
        );
        ensure!(
            !first_task.is_finished(),
            "first update left its terminal append barrier while another ID progressed"
        );

        release.wait().await;
        let first_value = first_task
            .await?
            .map_err(|error| anyhow!("{}: {}", error.code, error.message))?;
        let first_updated: InstallationMutationResult = serde_json::from_value(first_value)?;
        assert_eq!(
            first_updated.installation.record.display_name,
            "First committed later"
        );
        let journal = fixture
            .store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?;
        let terminal_ids = journal
            .iter()
            .filter(|event| event.kind == INSTALLATION_UPDATED)
            .map(|event| {
                event
                    .payload
                    .pointer("/view/record/installation_id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| anyhow!("updated event has no Installation ID"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        assert_eq!(
            terminal_ids,
            [
                second.record.installation_id.to_string(),
                first.record.installation_id.to_string(),
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn same_installation_update_and_remove_queue_behind_run_and_secret_guards(
    ) -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let lifecycle = fixture
            .registry
            .lifecycle_lock(&created.record.installation_id)?;
        let run = fixture
            .registry
            .acquire_ready_for_run(&created.record.installation_id, created.revision)
            .await?;
        let mut update = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "update-queued-behind-run",
        );
        update.display_name = Some("Updated after Run".to_string());
        let runtime = fixture.runtime();
        let update_task = tokio::spawn(async move {
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("update-queued-behind-run"),
                    "host.installation.update",
                    serde_json::to_value(update).expect("serialize update"),
                )
                .await
        });
        wait_for_lifecycle_writer(&lifecycle).await?;
        ensure!(
            !update_task.is_finished(),
            "same-ID update was not queued behind the Run guard"
        );
        drop(run);
        let updated: InstallationMutationResult = serde_json::from_value(
            update_task
                .await?
                .map_err(|error| anyhow!("{}: {}", error.code, error.message))?,
        )?;

        let lifecycle = fixture
            .registry
            .lifecycle_lock(&updated.installation.record.installation_id)?;
        let secret = fixture
            .registry
            .acquire_ready_secret_store(
                &updated.installation.record.installation_id,
                updated.installation.revision,
            )
            .await?;
        let runtime = fixture.runtime();
        let remove_id = updated.installation.record.installation_id.clone();
        let remove_task = tokio::spawn(async move {
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("remove-queued-behind-secret"),
                    "host.installation.remove",
                    serde_json::to_value(InstallationRemoveRequest {
                        installation_id: remove_id,
                        expected_revision: updated.installation.revision,
                        state_disposition: StateDisposition::Keep,
                        idempotency_key: "remove-queued-behind-secret".to_string(),
                        authority: None,
                    })
                    .expect("serialize remove"),
                )
                .await
        });
        wait_for_lifecycle_writer(&lifecycle).await?;
        ensure!(
            !remove_task.is_finished(),
            "same-ID remove was not queued behind the secret guard"
        );
        drop(secret);
        let removed: InstallationMutationResult = serde_json::from_value(
            remove_task
                .await?
                .map_err(|error| anyhow!("{}: {}", error.code, error.message))?,
        )?;
        assert_eq!(
            removed.installation.record.status,
            InstallationStatus::Removed
        );
        Ok(())
    }

    #[tokio::test]
    async fn lifecycle_weak_table_reclaims_unknown_failed_and_released_ids() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let mut invalid = fixture.request.clone();
        invalid.work_revision = missing_descriptor(WORK_REVISION_TYPE_URI, 'f');
        invalid.idempotency_key = "failed-create-has-no-lifecycle-id".to_string();
        assert!(fixture.create(invalid).await.is_err());
        assert!(fixture
            .registry
            .lifecycles
            .lock()
            .map_err(lock_error)?
            .is_empty());

        let first_unknown = InstallationId::new();
        assert!(fixture
            .registry
            .acquire_ready_for_run(&first_unknown, 1)
            .await
            .is_err());
        let first_weak = fixture
            .registry
            .lifecycles
            .lock()
            .map_err(lock_error)?
            .get(&first_unknown)
            .cloned()
            .expect("unknown lookup installed a weak lifecycle entry");
        assert!(first_weak.upgrade().is_none());

        let second_unknown = InstallationId::new();
        assert!(fixture
            .registry
            .acquire_ready_secret_store(&second_unknown, 1)
            .await
            .is_err());
        let lifecycles = fixture.registry.lifecycles.lock().map_err(lock_error)?;
        assert!(!lifecycles.contains_key(&first_unknown));
        assert!(lifecycles
            .get(&second_unknown)
            .is_some_and(|lifecycle| lifecycle.upgrade().is_none()));
        drop(lifecycles);

        let active_id = InstallationId::new();
        let active = fixture.registry.lifecycle_lock(&active_id)?;
        let active_again = fixture.registry.lifecycle_lock(&active_id)?;
        assert!(Arc::ptr_eq(&active, &active_again));
        let old = Arc::downgrade(&active);
        drop(active);
        drop(active_again);
        let replacement = fixture.registry.lifecycle_lock(&active_id)?;
        assert!(old.upgrade().is_none());
        assert!(fixture
            .registry
            .lifecycles
            .lock()
            .map_err(lock_error)?
            .get(&active_id)
            .and_then(Weak::upgrade)
            .is_some_and(|stored| Arc::ptr_eq(&stored, &replacement)));
        drop(replacement);

        let created = fixture.create(fixture.request.clone()).await?.installation;
        let removed_id = created.record.installation_id.clone();
        fixture
            .runtime()
            .call_protocol(
                &ProtocolContext::host_dev("weak-table-remove"),
                "host.installation.remove",
                serde_json::to_value(InstallationRemoveRequest {
                    installation_id: removed_id.clone(),
                    expected_revision: created.revision,
                    state_disposition: StateDisposition::Keep,
                    idempotency_key: "weak-table-remove".to_string(),
                    authority: None,
                })?,
            )
            .await
            .map_err(|error| anyhow!("{}: {}", error.code, error.message))?;
        assert!(fixture
            .registry
            .lifecycles
            .lock()
            .map_err(lock_error)?
            .get(&removed_id)
            .is_some_and(|lifecycle| lifecycle.upgrade().is_none()));
        drop(fixture.registry.lifecycle_lock(&InstallationId::new())?);
        assert!(!fixture
            .registry
            .lifecycles
            .lock()
            .map_err(lock_error)?
            .contains_key(&removed_id));
        Ok(())
    }

    #[tokio::test]
    async fn run_and_secret_guards_share_one_installation_read_lease() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let lifecycle = fixture
            .registry
            .lifecycle_lock(&created.record.installation_id)?;
        let run = fixture
            .registry
            .acquire_ready_for_run(&created.record.installation_id, created.revision)
            .await?;
        let secret = tokio::time::timeout(
            Duration::from_secs(1),
            fixture
                .registry
                .acquire_ready_secret_store(&created.record.installation_id, created.revision),
        )
        .await
        .map_err(|_| anyhow!("Run and secret readers did not share the lifecycle lease"))??;

        let mut request = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "shared-reader-update",
        );
        request.display_name = Some("after shared readers".to_string());
        let runtime = fixture.runtime();
        let update = tokio::spawn(async move {
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("shared-reader-update"),
                    "host.installation.update",
                    serde_json::to_value(request).expect("serialize update"),
                )
                .await
        });
        wait_for_lifecycle_writer(&lifecycle).await?;
        assert!(!update.is_finished());
        drop(run);
        assert!(
            lifecycle.try_write().is_err(),
            "dropping only one shared reader released the exclusive lifecycle"
        );
        drop(secret);
        let updated: InstallationMutationResult = serde_json::from_value(
            update
                .await?
                .map_err(|error| anyhow!("{}: {}", error.code, error.message))?,
        )?;
        assert_eq!(
            updated.installation.record.display_name,
            "after shared readers"
        );
        Ok(())
    }

    #[tokio::test]
    async fn ready_secret_store_guard_checks_before_filesystem_and_excludes_update(
    ) -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let missing = InstallationId::new();
        assert!(fixture
            .registry
            .acquire_ready_secret_store(&missing, 1)
            .await
            .is_err());
        assert!(!fixture.registry.installation_dir(&missing).exists());
        assert!(fixture
            .registry
            .acquire_ready_secret_store(&created.record.installation_id, created.revision + 1)
            .await
            .is_err());

        let guard = fixture
            .registry
            .acquire_ready_secret_store(&created.record.installation_id, created.revision)
            .await?;
        assert_eq!(guard.installation_id(), &created.record.installation_id);
        let mut request = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "secret-guard-update",
        );
        request.display_name = Some("after guarded secret effect".to_string());
        let runtime = fixture.runtime();
        let params = serde_json::to_value(request)?;
        let update = tokio::spawn(async move {
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("secret-guard-update"),
                    "host.installation.update",
                    params,
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !update.is_finished(),
            "lifecycle update must wait until the secret effect guard is dropped"
        );
        drop(guard);
        let result: InstallationMutationResult = serde_json::from_value(
            update
                .await?
                .map_err(|error| anyhow!("{}: {}", error.code, error.message))?,
        )?;
        assert_eq!(
            result.installation.record.display_name,
            "after guarded secret effect"
        );
        Ok(())
    }

    #[tokio::test]
    async fn ready_run_guard_pins_verified_closure_and_excludes_update() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        assert!(fixture
            .registry
            .acquire_ready_for_run(&created.record.installation_id, created.revision + 1)
            .await
            .is_err());

        let guard = fixture
            .registry
            .acquire_ready_for_run(&created.record.installation_id, created.revision)
            .await?;
        assert_eq!(guard.artifacts().installation, created);
        assert_eq!(
            guard.artifacts().work.artifact_descriptor()?,
            created.record.work_revision
        );
        assert!(guard
            .artifacts()
            .locks
            .contains_key(&created.record.assembly_lock.digest));
        let inspected = tokio::time::timeout(
            Duration::from_secs(1),
            fixture
                .registry
                .inspect_current_ready_for_run(&created.record.installation_id),
        )
        .await
        .map_err(|_| anyhow!("effect-free Run status waited on the active Run lease"))??;
        assert_eq!(inspected.installation, created);

        let mut request = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "run-guard-update",
        );
        request.display_name = Some("after guarded Run".to_string());
        let runtime = fixture.runtime();
        let update = tokio::spawn(async move {
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("run-guard-update"),
                    "host.installation.update",
                    serde_json::to_value(request).expect("serialize update"),
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !update.is_finished(),
            "Installation update must wait while the active Run owns its exact revision"
        );
        drop(guard);
        let result: InstallationMutationResult = serde_json::from_value(
            update
                .await?
                .map_err(|error| anyhow!("{}: {}", error.code, error.message))?,
        )?;
        assert_eq!(result.installation.record.display_name, "after guarded Run");
        Ok(())
    }

    #[tokio::test]
    async fn hydrate_rebuilds_from_journal_without_projection_authority() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        fs::write(
            fixture
                .registry
                .installation_dir(&created.record.installation_id)
                .join("installation.json"),
            b"not authoritative",
        )?;
        let restarted =
            InstallationRegistry::persistent(fixture.store, fixture.objects, fixture._data.path())?;
        assert_eq!(restarted.hydrate().await?, 1);
        assert_eq!(
            restarted.get(&created.record.installation_id).await?,
            Some(created.clone())
        );
        let projection: InstallationView = serde_json::from_slice(&fs::read(
            restarted
                .installation_dir(&created.record.installation_id)
                .join("installation.json"),
        )?)?;
        assert_eq!(projection, created);
        Ok(())
    }

    #[tokio::test]
    async fn projection_preparation_failure_does_not_commit_or_advance_revision(
    ) -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        fixture
            .registry
            .inject_projection_failure(ProjectionFailure::Prepare)?;
        let mut request = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "prepare-failure",
        );
        request.display_name = Some("Not committed".to_string());
        assert!(fixture.update(request).await.is_err());
        assert_eq!(
            fixture
                .registry
                .get(&created.record.installation_id)
                .await?,
            Some(created)
        );
        assert_eq!(
            fixture
                .store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn projection_path_conflict_fails_before_commit() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let projection = fixture
            .registry
            .installation_dir(&created.record.installation_id)
            .join("installation.json");
        fs::remove_file(&projection)?;
        fs::create_dir(&projection)?;
        let mut request = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "path-conflict",
        );
        request.display_name = Some("Not committed".to_string());
        assert!(fixture.update(request).await.is_err());
        assert_eq!(
            fixture
                .registry
                .get(&created.record.installation_id)
                .await?,
            Some(created)
        );
        assert_eq!(
            fixture
                .store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn projection_publish_failure_returns_committed_result_and_hydrate_repairs(
    ) -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        fixture
            .registry
            .inject_projection_failure(ProjectionFailure::Publish)?;
        let mut request = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "publish-failure",
        );
        request.display_name = Some("Committed".to_string());
        let committed = fixture.update(request).await?;
        assert_eq!(committed.installation.revision, created.revision + 1);
        assert_eq!(committed.installation.record.display_name, "Committed");
        let stale_projection: InstallationView = serde_json::from_slice(&fs::read(
            fixture
                .registry
                .installation_dir(&created.record.installation_id)
                .join("installation.json"),
        )?)?;
        assert_eq!(stale_projection, created);

        let restarted =
            InstallationRegistry::persistent(fixture.store, fixture.objects, fixture._data.path())?;
        assert_eq!(restarted.hydrate().await?, 1);
        let repaired: InstallationView = serde_json::from_slice(&fs::read(
            restarted
                .installation_dir(&created.record.installation_id)
                .join("installation.json"),
        )?)?;
        assert_eq!(repaired, committed.installation);
        Ok(())
    }

    #[tokio::test]
    async fn malformed_or_gapped_journal_fails_closed() -> anyhow::Result<()> {
        let data = tempfile::tempdir()?;
        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(InMemoryObjectStore::default());
        store
            .append(EventEnvelope {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: JOURNAL_SESSION.to_string(),
                sequence: 1,
                writer_package_id: JOURNAL_WRITER.to_string(),
                kind: INSTALLATION_CREATED.to_string(),
                schema_version: JOURNAL_SCHEMA,
                timestamp: Utc::now(),
                payload: serde_json::json!({"malformed": true}),
                metadata: serde_json::json!({}),
            })
            .await?;
        let registry = InstallationRegistry::persistent(store, objects, data.path())?;
        assert!(registry
            .hydrate()
            .await
            .unwrap_err()
            .to_string()
            .contains("sequence gap"));
        assert!(registry
            .list(InstallationListRequest::default())
            .await?
            .is_empty());

        let data = tempfile::tempdir()?;
        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(InMemoryObjectStore::default());
        store
            .append(EventEnvelope {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: JOURNAL_SESSION.to_string(),
                sequence: 0,
                writer_package_id: JOURNAL_WRITER.to_string(),
                kind: INSTALLATION_CREATED.to_string(),
                schema_version: JOURNAL_SCHEMA,
                timestamp: Utc::now(),
                payload: serde_json::json!({"malformed": true}),
                metadata: serde_json::json!({}),
            })
            .await?;
        let registry = InstallationRegistry::persistent(store, objects, data.path())?;
        assert!(registry
            .hydrate()
            .await
            .unwrap_err()
            .to_string()
            .contains("payload is malformed"));
        assert!(registry
            .list(InstallationListRequest::default())
            .await?
            .is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn incomplete_update_is_rolled_back_during_hydration() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let state = fixture.registry.state_dir(&created.record.installation_id);
        fs::write(state.join("save.bin"), b"original")?;
        let snapshot = fixture
            .registry
            .snapshot_state(&created.record.installation_id)
            .await?;
        let pending = PendingMutation {
            operation_id: uuid::Uuid::new_v4().to_string(),
            kind: PendingKind::Update,
            previous: created.clone(),
            state_snapshot: snapshot,
            receipts: recovery_fixture_receipts(&fixture.registry, &created).await?,
        };
        assert!(
            fixture
                .registry
                .append_payload(
                    UPDATE_STARTED,
                    &StartedPayload {
                        pending: pending.clone(),
                    },
                )
                .await?
        );
        fixture
            .registry
            .replace_state(
                &created.record.installation_id,
                &StateSnapshot {
                    schema: INSTALLATION_STATE_SNAPSHOT_SCHEMA.to_string(),
                    entries: Vec::new(),
                },
            )
            .await?;
        let restarted =
            InstallationRegistry::persistent(fixture.store, fixture.objects, fixture._data.path())?;
        assert_eq!(restarted.hydrate().await?, 1);
        assert_eq!(fs::read(state.join("save.bin"))?, b"original");
        assert_eq!(
            restarted.get(&created.record.installation_id).await?,
            Some(created)
        );
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_registries_converge_on_one_create_claim() -> anyhow::Result<()> {
        let first_data = tempfile::tempdir()?;
        let second_data = tempfile::tempdir()?;
        let store = Arc::new(InMemoryEventStore::default());
        let objects = Arc::new(InMemoryObjectStore::default());
        let (work_revision, assembly_lock) = put_work_pair(&objects, "race", Vec::new()).await?;
        let request = InstallationCreateRequest {
            work_id: WorkId::parse("tests/race")?,
            work_revision,
            assembly_lock,
            display_name: "Race".to_string(),
            source: AcquisitionRecord {
                kind: AcquisitionKind::WorkBundle,
                source_ref: None,
                provenance_refs: Vec::new(),
                update_channel: None,
            },
            state_bindings: Vec::new(),
            secret_policy: Default::default(),
            idempotency_key: "race-create".to_string(),
            authority: None,
        };
        let first =
            InstallationRegistry::persistent(store.clone(), objects.clone(), first_data.path())?;
        let second =
            InstallationRegistry::persistent(store.clone(), objects.clone(), second_data.path())?;
        let first_runtime = Runtime::new(
            store.clone(),
            RuntimeConfig {
                object_store: objects.clone(),
                installation_control: first.clone(),
                ..RuntimeConfig::default()
            },
        );
        let second_runtime = Runtime::new(
            store.clone(),
            RuntimeConfig {
                object_store: objects,
                installation_control: second.clone(),
                ..RuntimeConfig::default()
            },
        );
        let first_params = serde_json::to_value(request.clone())?;
        let second_params = serde_json::to_value(request)?;
        let first_context = ProtocolContext::host_dev("concurrent-create-one");
        let second_context = ProtocolContext::host_dev("concurrent-create-two");
        let (left, right) = tokio::join!(
            first_runtime.call_protocol(&first_context, "host.installation.create", first_params,),
            second_runtime.call_protocol(
                &second_context,
                "host.installation.create",
                second_params,
            )
        );
        let left: InstallationMutationResult = serde_json::from_value(
            left.map_err(|error| anyhow!("{}: {}", error.code, error.message))?,
        )?;
        let right: InstallationMutationResult = serde_json::from_value(
            right.map_err(|error| anyhow!("{}: {}", error.code, error.message))?,
        )?;
        assert_eq!(left.installation, right.installation);
        assert_ne!(left.idempotent, right.idempotent);
        assert_eq!(
            store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            1
        );
        for registry in [&first, &second] {
            let cleanup_id = InstallationId::new();
            drop(registry.lifecycle_lock(&cleanup_id)?);
            let lifecycles = registry.lifecycles.lock().map_err(lock_error)?;
            assert_eq!(lifecycles.len(), 1);
            assert!(lifecycles
                .get(&cleanup_id)
                .is_some_and(|lifecycle| lifecycle.upgrade().is_none()));
        }
        Ok(())
    }

    #[tokio::test]
    async fn owner_revocation_stops_old_registry_and_takeover_resumes_from_journal(
    ) -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let first_owner = crate::acquire_development_host_lease(
            fixture.store.clone(),
            crate::development_registry(),
        )
        .await?;
        fixture.registry.install_owner_lease(first_owner.clone())?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        crate::release_development_host_lease(fixture.store.clone(), &first_owner).await?;
        let event_count = fixture
            .store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?
            .len();
        let mut rejected = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "revoked-owner-update",
        );
        rejected.display_name = Some("must not commit".to_string());
        assert!(fixture.update(rejected).await.is_err());
        assert_eq!(
            fixture
                .store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            event_count
        );

        let second_owner = crate::acquire_development_host_lease(
            fixture.store.clone(),
            crate::development_registry(),
        )
        .await?;
        let restarted = InstallationRegistry::persistent(
            fixture.store.clone(),
            fixture.objects.clone(),
            fixture._data.path(),
        )?;
        restarted.install_owner_lease(second_owner.clone())?;
        assert_eq!(restarted.hydrate().await?, 1);
        let mut accepted = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "takeover-update",
        );
        accepted.display_name = Some("takeover committed".to_string());
        let updated: InstallationMutationResult = serde_json::from_value(
            Runtime::new(
                fixture.store.clone(),
                RuntimeConfig {
                    object_store: fixture.objects.clone(),
                    installation_control: restarted.clone(),
                    ..RuntimeConfig::default()
                },
            )
            .call_protocol(
                &ProtocolContext::host_dev("takeover-update-test"),
                "host.installation.update",
                serde_json::to_value(accepted)?,
            )
            .await
            .map_err(|error| anyhow!("{}: {}", error.code, error.message))?,
        )?;
        assert_eq!(
            updated.installation.record.display_name,
            "takeover committed"
        );
        crate::release_development_host_lease(fixture.store.clone(), &second_owner).await?;
        Ok(())
    }

    #[tokio::test]
    async fn stale_noop_claim_after_cas_loss_is_not_appended_or_replayed() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let second_data = tempfile::tempdir()?;
        let second = InstallationRegistry::persistent(
            fixture.store.clone(),
            fixture.objects.clone(),
            second_data.path(),
        )?;
        assert_eq!(second.hydrate().await?, 1);

        let noop_request = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "stale-noop",
        );
        let mut actual = update_request(
            &created,
            created.record.work_revision.clone(),
            created.record.assembly_lock.clone(),
            "actual-update",
        );
        actual.display_name = Some("Concurrent winner".to_string());
        let updated: InstallationMutationResult = serde_json::from_value(
            Runtime::new(
                fixture.store.clone(),
                RuntimeConfig {
                    object_store: fixture.objects.clone(),
                    installation_control: second.clone(),
                    ..RuntimeConfig::default()
                },
            )
            .call_protocol(
                &ProtocolContext::host_dev("actual-update-test"),
                "host.installation.update",
                serde_json::to_value(actual)?,
            )
            .await
            .map_err(|error| anyhow!("{}: {}", error.code, error.message))?,
        )?;
        assert_eq!(updated.installation.revision, 2);

        let error = fixture.update(noop_request).await.unwrap_err();
        assert!(error.to_string().contains("revision_conflict"));
        assert_eq!(
            fixture
                .store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            2
        );
        let third_data = tempfile::tempdir()?;
        let restarted =
            InstallationRegistry::persistent(fixture.store, fixture.objects, third_data.path())?;
        assert_eq!(restarted.hydrate().await?, 1);
        assert_eq!(
            restarted.get(&created.record.installation_id).await?,
            Some(updated.installation)
        );
        Ok(())
    }

    #[tokio::test]
    async fn stale_pending_rollback_after_terminal_commit_does_not_poison_hydration(
    ) -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let snapshot = fixture
            .registry
            .snapshot_state(&created.record.installation_id)
            .await?;
        let pending = PendingMutation {
            operation_id: "operation-race".to_string(),
            kind: PendingKind::Update,
            previous: created.clone(),
            state_snapshot: snapshot.clone(),
            receipts: recovery_fixture_receipts(&fixture.registry, &created).await?,
        };
        assert!(
            fixture
                .registry
                .append_payload(
                    UPDATE_STARTED,
                    &StartedPayload {
                        pending: pending.clone(),
                    },
                )
                .await?
        );

        let second_data = tempfile::tempdir()?;
        let second = InstallationRegistry::persistent(
            fixture.store.clone(),
            fixture.objects.clone(),
            second_data.path(),
        )?;
        {
            let _apply = second.apply.lock().await;
            second.sync_journal_locked().await?;
        }
        let mut updated_view = created.clone();
        updated_view.revision += 1;
        updated_view.record.display_name = "Terminal winner".to_string();
        updated_view.record.updated_at = Utc::now();
        updated_view.record.status = InstallationStatus::Ready;
        updated_view.rollback = Some(InstallationRollbackPointer {
            revision: created.revision,
            work_revision: created.record.work_revision.clone(),
            assembly_lock: created.record.assembly_lock.clone(),
            state_snapshot: Some(snapshot),
        });
        let result = InstallationMutationResult {
            installation: updated_view.clone(),
            diff: Some(InstallationDiff {
                display_name_changed: true,
                ..Default::default()
            }),
            receipts: pending.receipts.clone(),
            idempotent: false,
        };
        let terminal = UpdatedPayload {
            operation_id: Some(pending.operation_id.clone()),
            previous_revision: created.revision,
            view: updated_view.clone(),
            claim: IdempotencyClaim {
                key_hash: hash_bytes(b"pending-terminal-key"),
                fingerprint: hash_bytes(b"pending-terminal-fingerprint"),
                result,
            },
        };
        assert!(
            fixture
                .registry
                .append_payload(INSTALLATION_UPDATED, &terminal)
                .await?
        );

        let stale_rollback = RollbackPayload {
            installation_id: created.record.installation_id.clone(),
            operation_id: pending.operation_id,
            previous: created.clone(),
            reason_code: "terminal_append_conflict".to_string(),
        };
        second
            .append_pending_rollback(UPDATE_ROLLBACK, &stale_rollback)
            .await?;
        assert_eq!(
            fixture
                .store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            3
        );
        let third_data = tempfile::tempdir()?;
        let restarted =
            InstallationRegistry::persistent(fixture.store, fixture.objects, third_data.path())?;
        assert_eq!(restarted.hydrate().await?, 1);
        assert_eq!(
            restarted.get(&created.record.installation_id).await?,
            Some(updated_view)
        );
        Ok(())
    }

    #[tokio::test]
    async fn state_snapshot_reads_regular_tree_in_deterministic_order() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let state = fixture.registry.state_dir(&created.record.installation_id);
        fs::create_dir(state.join("nested"))?;
        fs::write(state.join("z.bin"), [3, 2, 1])?;
        fs::write(state.join("nested/a.bin"), [0, 1, 2, 255])?;

        let descriptor = fixture
            .registry
            .snapshot_state(&created.record.installation_id)
            .await?;
        let snapshot = fixture.registry.load_snapshot(&descriptor).await?;
        assert_eq!(
            snapshot.entries,
            vec![
                StateSnapshotEntry {
                    path: "nested/a.bin".to_string(),
                    bytes: vec![0, 1, 2, 255],
                },
                StateSnapshotEntry {
                    path: "z.bin".to_string(),
                    bytes: vec![3, 2, 1],
                },
            ]
        );
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn state_snapshot_and_projection_reject_symlinks() -> anyhow::Result<()> {
        use std::os::unix::fs::symlink;

        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let outside = tempfile::tempdir()?;
        fs::write(outside.path().join("secret"), b"secret")?;
        symlink(
            outside.path().join("secret"),
            fixture
                .registry
                .state_dir(&created.record.installation_id)
                .join("escape"),
        )?;
        assert!(fixture
            .registry
            .snapshot_state(&created.record.installation_id)
            .await
            .is_err());

        let data = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        symlink(outside.path(), data.path().join("installations"))?;
        assert!(InstallationRegistry::persistent(
            Arc::new(InMemoryEventStore::default()),
            Arc::new(InMemoryObjectStore::default()),
            data.path(),
        )
        .is_err());
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn anchored_effect_path_is_validated_as_the_open_directory() -> anyhow::Result<()> {
        let data = tempfile::tempdir()?;
        let anchor = open_anchored_directory(data.path(), "test anchor")?;
        let effect_path = anchor.effect_path();

        prepare_existing_real_directory(&effect_path, "test anchor effect path")?;
        fs::write(effect_path.join("through-anchor"), b"anchored")?;
        assert_eq!(fs::read(data.path().join("through-anchor"))?, b"anchored");
        prepare_atomic_replacement(&anchor, "atomic-through-anchor", b"atomic")?.publish()?;
        assert_eq!(
            fs::read(data.path().join("atomic-through-anchor"))?,
            b"atomic"
        );
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn capability_directory_sync_reopens_linux_o_path_handle() -> anyhow::Result<()> {
        let data = tempfile::tempdir()?;
        let anchor = open_anchored_directory(data.path(), "test sync owner")?;
        let owner = anchor.capability("test sync owner")?;
        owner.create_dir("staging")?;
        let staging = owner.open_dir("staging")?;
        staging.write("state.bin", b"durable")?;

        sync_capability_directory(&staging, "test staging directory")?;
        assert_eq!(fs::read(data.path().join("staging/state.bin"))?, b"durable");
        Ok(())
    }

    #[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
    #[test]
    fn anchored_atomic_replacement_uses_handle_relative_non_linux_unix_io() -> anyhow::Result<()> {
        use std::os::unix::fs::symlink;

        let data = tempfile::tempdir()?;
        let anchor = open_anchored_directory(data.path(), "test anchor")?;
        prepare_atomic_replacement(&anchor, "through-anchor", b"anchored")?.publish()?;
        assert_eq!(fs::read(data.path().join("through-anchor"))?, b"anchored");

        let outside = tempfile::tempdir()?;
        let outside_file = outside.path().join("outside");
        fs::write(&outside_file, b"outside")?;
        symlink(&outside_file, data.path().join("symlink-target"))?;
        assert!(prepare_atomic_replacement(&anchor, "symlink-target", b"rejected").is_err());
        assert_eq!(fs::read(outside_file)?, b"outside");
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn projection_publish_remains_on_anchor_after_post_validation_ancestor_swap(
    ) -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let prepared = fixture.registry.prepare_projection(&created).await?;
        let installation = fixture
            .registry
            .installation_dir(&created.record.installation_id);
        let parked = installation.with_file_name(format!(
            "{}.projection-parked",
            created.record.installation_id.as_str()
        ));
        let outside = tempfile::tempdir()?;
        inject_projection_ancestor_swap(
            installation.clone(),
            parked.clone(),
            outside.path().to_path_buf(),
        )?;

        fixture.registry.publish_projection(prepared).await?;
        let projected: InstallationView =
            serde_json::from_slice(&fs::read(parked.join("installation.json"))?)?;
        assert_eq!(projected, created);
        assert!(parked.join("assembly.lock.json").is_file());
        assert!(!outside.path().join("installation.json").exists());
        assert!(!outside.path().join("assembly.lock.json").exists());

        fs::remove_file(&installation)?;
        fs::rename(&parked, &installation)?;
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn state_snapshot_never_follows_terminal_or_leaf_swaps() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let installation = fixture
            .registry
            .installation_dir(&created.record.installation_id);
        let state = fixture.registry.state_dir(&created.record.installation_id);
        let state_file = fixture
            .registry
            .state_dir(&created.record.installation_id)
            .join("save.bin");
        fs::write(&state_file, b"owned")?;
        let outside = tempfile::tempdir()?;
        let outside_file = outside.path().join("outside.bin");
        fs::write(&outside_file, b"outside")?;
        inject_state_open_swap(
            PathBuf::from("save.bin"),
            state_file.clone(),
            outside_file.clone(),
        )?;
        assert!(fixture
            .registry
            .snapshot_state(&created.record.installation_id)
            .await
            .is_err());
        assert_eq!(fs::read(outside_file)?, b"outside");

        fs::remove_file(&state_file)?;
        fs::write(&state_file, b"owned")?;
        let state_parked = installation.join("state-parked");
        inject_state_root_open_swap(
            state.clone(),
            state_parked.clone(),
            outside.path().to_path_buf(),
        )?;
        assert!(fixture
            .registry
            .snapshot_state(&created.record.installation_id)
            .await
            .is_err());
        assert_eq!(fs::read(state_parked.join("save.bin"))?, b"owned");
        assert_eq!(fs::read(outside.path().join("outside.bin"))?, b"outside");
        fs::remove_file(&state)?;
        fs::rename(&state_parked, &state)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn safe_tree_removal_quarantines_post_validation_leaf_replacement() -> anyhow::Result<()> {
        let owner_root = tempfile::tempdir()?;
        let owner = open_anchored_directory(owner_root.path(), "test removal owner")?;

        let leaf = owner_root.path().join("leaf");
        fs::create_dir(&leaf)?;
        fs::write(leaf.join("owned.bin"), b"owned")?;
        let leaf_identity = same_file::Handle::from_path(&leaf)?;
        let leaf_parked = owner_root.path().join("leaf-parked");
        let outside_leaf = tempfile::tempdir()?;
        fs::write(outside_leaf.path().join("external.bin"), b"external")?;
        let outside_identity = same_file::Handle::from_path(outside_leaf.path())?;
        inject_remove_tree_swap(
            leaf.clone(),
            leaf.clone(),
            leaf_parked.clone(),
            outside_leaf.path().to_path_buf(),
        )?;
        let error = remove_safe_tree(&owner, Path::new("leaf"))
            .expect_err("a replaced leaf must not be recursively removed");
        assert!(error.to_string().contains("identity changed"));
        assert!(!error
            .to_string()
            .contains(owner_root.path().to_string_lossy().as_ref()));
        assert!(matches!(
            fs::symlink_metadata(&leaf),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ));
        let mut exchanged_quarantines = fs::read_dir(owner_root.path())?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        exchanged_quarantines.retain(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".removal-") && name.ends_with(".tmp"))
        });
        assert_eq!(exchanged_quarantines.len(), 1);
        let exchanged_quarantine = &exchanged_quarantines[0];
        assert!(fs::symlink_metadata(exchanged_quarantine)?
            .file_type()
            .is_symlink());
        assert!(same_file::Handle::from_path(exchanged_quarantine)? == outside_identity);
        assert!(same_file::Handle::from_path(&leaf_parked)? == leaf_identity);
        assert_eq!(fs::read(leaf_parked.join("owned.bin"))?, b"owned");
        assert_eq!(
            fs::read(outside_leaf.path().join("external.bin"))?,
            b"external"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn safe_tree_removal_keeps_original_quarantine_when_inventory_changes() -> anyhow::Result<()> {
        let owner_root = tempfile::tempdir()?;
        let owner = open_anchored_directory(owner_root.path(), "test removal owner")?;
        let nested_root = owner_root.path().join("nested-root");
        let nested = nested_root.join("nested");
        fs::create_dir_all(&nested)?;
        fs::write(nested.join("owned.bin"), b"nested-owned")?;
        let root_identity = same_file::Handle::from_path(&nested_root)?;
        let nested_identity = same_file::Handle::from_path(&nested)?;
        let nested_parked = nested_root.join("nested-parked");
        let outside_nested = tempfile::tempdir()?;
        fs::write(
            outside_nested.path().join("external.bin"),
            b"nested-external",
        )?;
        inject_remove_tree_swap(
            nested_root.clone(),
            nested.clone(),
            nested_parked.clone(),
            outside_nested.path().to_path_buf(),
        )?;
        let error = remove_safe_tree(&owner, Path::new("nested-root"))
            .expect_err("an inventory change must fail without restoring the stable name");
        assert!(error.to_string().contains("changed before removal"));
        assert!(!error
            .to_string()
            .contains(owner_root.path().to_string_lossy().as_ref()));
        assert!(matches!(
            fs::symlink_metadata(&nested_root),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ));

        let mut quarantines = fs::read_dir(owner_root.path())?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        quarantines.retain(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".removal-") && name.ends_with(".tmp"))
        });
        assert_eq!(quarantines.len(), 1);
        let quarantine = &quarantines[0];
        assert!(fs::symlink_metadata(quarantine)?.file_type().is_dir());
        assert!(same_file::Handle::from_path(quarantine)? == root_identity);
        assert!(same_file::Handle::from_path(quarantine.join("nested-parked"))? == nested_identity);
        assert!(fs::symlink_metadata(quarantine.join("nested"))?
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read(quarantine.join("nested-parked/owned.bin"))?,
            b"nested-owned"
        );
        assert_eq!(
            fs::read(outside_nested.path().join("external.bin"))?,
            b"nested-external"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn safe_tree_removal_deletes_unchanged_tree_without_quarantine() -> anyhow::Result<()> {
        let owner_root = tempfile::tempdir()?;
        let owner = open_anchored_directory(owner_root.path(), "test removal owner")?;
        let stable = owner_root.path().join("stable");
        fs::create_dir_all(stable.join("nested"))?;
        fs::write(stable.join("root.bin"), b"root")?;
        fs::write(stable.join("nested/leaf.bin"), b"leaf")?;

        remove_safe_tree(&owner, Path::new("stable"))?;
        assert!(matches!(
            fs::symlink_metadata(&stable),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ));
        let leftovers = fs::read_dir(owner_root.path())?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<Result<Vec<_>, _>>()?;
        assert!(leftovers.is_empty());
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn state_and_projection_reject_controlled_installation_ancestor_swap(
    ) -> anyhow::Result<()> {
        use std::os::unix::fs::symlink;

        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let installation = fixture
            .registry
            .installation_dir(&created.record.installation_id);
        let parked = installation.with_file_name(format!(
            "{}.parked",
            created.record.installation_id.as_str()
        ));
        let outside = tempfile::tempdir()?;
        let state = fixture.registry.state_dir(&created.record.installation_id);
        fs::write(state.join("save.bin"), b"owned")?;
        inject_installation_ancestor_swap(
            installation.clone(),
            parked.clone(),
            outside.path().to_path_buf(),
        )?;
        let error = fixture
            .registry
            .replace_state(
                &created.record.installation_id,
                &StateSnapshot {
                    schema: INSTALLATION_STATE_SNAPSHOT_SCHEMA.to_string(),
                    entries: Vec::new(),
                },
            )
            .await
            .expect_err("ancestor replacement must invalidate the anchored effect");
        assert!(error.to_string().contains("identity changed"));
        assert!(!outside.path().join("state").exists());
        assert_eq!(fs::read(parked.join("state/save.bin"))?, b"owned");

        fs::remove_file(&installation)?;
        fs::rename(&parked, &installation)?;

        let outside_state = outside.path().join("state");
        fs::create_dir(&outside_state)?;
        fs::write(outside_state.join("external.bin"), b"external")?;
        inject_installation_ancestor_swap(
            installation.clone(),
            parked.clone(),
            outside.path().to_path_buf(),
        )?;
        let descriptor = fixture
            .registry
            .snapshot_state(&created.record.installation_id)
            .await?;
        let snapshot = fixture.registry.load_snapshot(&descriptor).await?;
        assert_eq!(
            snapshot.entries,
            vec![StateSnapshotEntry {
                path: "save.bin".to_string(),
                bytes: b"owned".to_vec(),
            }]
        );
        assert_eq!(fs::read(outside_state.join("external.bin"))?, b"external");
        fs::remove_file(&installation)?;
        fs::rename(&parked, &installation)?;

        let prepared = fixture.registry.prepare_projection(&created).await?;
        fs::rename(&installation, &parked)?;
        symlink(outside.path(), &installation)?;
        assert!(fixture.registry.publish_projection(prepared).await.is_err());
        assert!(!outside.path().join("installation.json").exists());
        assert!(!outside.path().join("assembly.lock.json").exists());
        fs::remove_file(&installation)?;
        fs::rename(&parked, &installation)?;
        Ok(())
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn state_snapshot_rejects_reparse_root_and_pins_installation_ancestor(
    ) -> anyhow::Result<()> {
        use std::os::windows::fs::symlink_dir;

        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let installation = fixture
            .registry
            .installation_dir(&created.record.installation_id);
        let state = fixture.registry.state_dir(&created.record.installation_id);
        fs::write(state.join("save.bin"), b"owned")?;
        let anchored = fixture
            .registry
            .prepare_installation_tree(&created.record.installation_id)?;
        let parked_installation = installation.with_file_name(format!(
            "{}.snapshot-parked",
            created.record.installation_id.as_str()
        ));
        assert!(
            fs::rename(&installation, &parked_installation).is_err(),
            "the Installation anchor must prevent an ancestor junction swap"
        );
        assert_eq!(
            read_state_entries(&anchored)?,
            vec![StateSnapshotEntry {
                path: "save.bin".to_string(),
                bytes: b"owned".to_vec(),
            }]
        );
        drop(anchored);

        let outside = tempfile::tempdir()?;
        fs::write(outside.path().join("external.bin"), b"external")?;
        let parked_state = installation.join("state-parked");
        fs::rename(&state, &parked_state)?;
        if symlink_dir(outside.path(), &state).is_err() {
            fs::rename(&parked_state, &state)?;
            return Ok(());
        }
        assert!(fixture
            .registry
            .snapshot_state(&created.record.installation_id)
            .await
            .is_err());
        assert_eq!(fs::read(outside.path().join("external.bin"))?, b"external");
        fs::remove_dir(&state)?;
        fs::rename(&parked_state, &state)?;
        Ok(())
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn projection_anchor_blocks_windows_ancestor_replacement() -> anyhow::Result<()> {
        let fixture = fixture().await?;
        let created = fixture.create(fixture.request.clone()).await?.installation;
        let prepared = fixture.registry.prepare_projection(&created).await?;
        let installation = fixture
            .registry
            .installation_dir(&created.record.installation_id);
        let parked = installation.with_file_name(format!(
            "{}.parked",
            created.record.installation_id.as_str()
        ));
        assert!(
            fs::rename(&installation, &parked).is_err(),
            "the non-delete-sharing directory handle must prevent a junction swap"
        );
        fixture.registry.publish_projection(prepared).await?;
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn projection_root_rejects_windows_reparse_points() -> anyhow::Result<()> {
        use std::os::windows::fs::symlink_dir;

        let data = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        if symlink_dir(outside.path(), data.path().join("installations")).is_err() {
            // Creating a test symlink requires a Windows developer-mode or privilege grant.
            return Ok(());
        }
        assert!(InstallationRegistry::persistent(
            Arc::new(InMemoryEventStore::default()),
            Arc::new(InMemoryObjectStore::default()),
            data.path(),
        )
        .is_err());
        Ok(())
    }
}
