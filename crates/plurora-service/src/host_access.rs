use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use axum::extract::{Extension, Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use plurora_core::{EventEnvelope, EventSequence};
use plurora_runtime::{EventStore, ProtocolContext, ProtocolResourceSelector};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{AppState, ServiceError, HOST_SESSION_COOKIE};

pub const REMOTE_HOST_SESSION_COOKIE: &str = "__Host-plurora_remote_session";

const HOST_ACCESS_JOURNAL_SESSION: &str = "host_control_access";
const HOST_ACCESS_JOURNAL_EVENT: &str = "host/control/v1/access.transition";
const HOST_ACCESS_JOURNAL_WRITER: &str = "host/control-plane";
const DEFAULT_PAIRING_TTL_SECS: u64 = 5 * 60;
const MAX_PAIRING_TTL_SECS: u64 = 10 * 60;
const DEFAULT_GRANT_TTL_SECS: u64 = 90 * 24 * 60 * 60;
const MAX_GRANT_TTL_SECS: u64 = 365 * 24 * 60 * 60;
const MAX_DELEGATION_DEPTH: u16 = 32;
const MAX_BULK_GRANT_REVOCATIONS: usize = 256;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum HostAccessScope {
    Observe,
    ProjectOperate,
    Deploy,
    DevelopPropose,
    DevelopApprove,
    DevelopExecute,
    AccessManage,
}

impl HostAccessScope {
    pub const ALL: [Self; 7] = [
        Self::Observe,
        Self::ProjectOperate,
        Self::Deploy,
        Self::DevelopPropose,
        Self::DevelopApprove,
        Self::DevelopExecute,
        Self::AccessManage,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::ProjectOperate => "project_operate",
            Self::Deploy => "deploy",
            Self::DevelopPropose => "develop_propose",
            Self::DevelopApprove => "develop_approve",
            Self::DevelopExecute => "develop_execute",
            Self::AccessManage => "access_manage",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum HostAccessResourceKind {
    Project,
    Target,
}

impl HostAccessResourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Target => "target",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
pub struct HostAccessResourceSelector {
    pub kind: HostAccessResourceKind,
    /// Omitted means every resource of this kind. Exact ids are compared
    /// structurally and never by prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

fn default_host_access_resources() -> BTreeSet<HostAccessResourceSelector> {
    BTreeSet::from([
        HostAccessResourceSelector {
            kind: HostAccessResourceKind::Project,
            id: None,
        },
        HostAccessResourceSelector {
            kind: HostAccessResourceKind::Target,
            id: None,
        },
    ])
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostAccessIdentityKind {
    Root,
    Device,
}

#[derive(Debug, Clone)]
pub struct HostAccessIdentity {
    pub kind: HostAccessIdentityKind,
    pub grant_id: Option<String>,
    pub device_name: String,
    pub scopes: BTreeSet<HostAccessScope>,
    pub resources: BTreeSet<HostAccessResourceSelector>,
    pub delegation_chain: Vec<String>,
    pub expires_at_ms: Option<i64>,
}

impl HostAccessIdentity {
    pub fn root() -> Self {
        Self {
            kind: HostAccessIdentityKind::Root,
            grant_id: None,
            device_name: "Host root credential".to_string(),
            scopes: HostAccessScope::ALL.into_iter().collect(),
            resources: default_host_access_resources(),
            delegation_chain: Vec::new(),
            expires_at_ms: None,
        }
    }

    pub fn allows(&self, scope: HostAccessScope) -> bool {
        self.kind == HostAccessIdentityKind::Root || self.scopes.contains(&scope)
    }

    pub fn allows_resource(&self, kind: HostAccessResourceKind, id: &str) -> bool {
        self.kind == HostAccessIdentityKind::Root
            || self.resources.iter().any(|selector| {
                selector.kind == kind
                    && selector.id.as_deref().is_none_or(|selected| selected == id)
            })
    }

    pub fn allows_project(&self, project_id: &str) -> bool {
        self.allows_resource(HostAccessResourceKind::Project, project_id)
    }

    pub fn allows_target(&self, target_id: &str) -> bool {
        self.allows_resource(HostAccessResourceKind::Target, target_id)
    }

    pub fn allows_all(&self, kind: HostAccessResourceKind) -> bool {
        self.kind == HostAccessIdentityKind::Root
            || self
                .resources
                .iter()
                .any(|selector| selector.kind == kind && selector.id.is_none())
    }

    fn allows_selector(&self, candidate: &HostAccessResourceSelector) -> bool {
        if self.kind == HostAccessIdentityKind::Root {
            return true;
        }
        self.resources
            .iter()
            .any(|owned| selector_covers(owned, candidate))
    }

    pub fn protocol_context(&self, transport: impl Into<String>) -> ProtocolContext {
        let transport = transport.into();
        if self.kind == HostAccessIdentityKind::Root {
            return ProtocolContext::host_admin(transport);
        }
        ProtocolContext::host_device(
            self.grant_id
                .clone()
                .expect("device identities always carry a grant id"),
            self.scopes
                .iter()
                .map(|scope| scope.as_str().to_string())
                .collect(),
            self.resources
                .iter()
                .map(|selector| ProtocolResourceSelector {
                    owner: "host".to_string(),
                    kind: selector.kind.as_str().to_string(),
                    id: selector.id.clone(),
                })
                .collect(),
            self.delegation_chain.clone(),
            transport,
        )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredPairing {
    id: String,
    device_name: String,
    scopes: BTreeSet<HostAccessScope>,
    #[serde(default = "default_host_access_resources")]
    resources: BTreeSet<HostAccessResourceSelector>,
    #[serde(default)]
    parent_grant_id: Option<String>,
    #[serde(default)]
    delegation_depth: u16,
    secret_digest: String,
    created_at_ms: i64,
    expires_at_ms: i64,
    grant_expires_at_ms: i64,
    status: HostPairingStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum HostPairingStatus {
    Pending,
    Claimed {
        grant_id: String,
        claimed_at_ms: i64,
    },
    Cancelled {
        cancelled_at_ms: i64,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredGrant {
    id: String,
    device_name: String,
    scopes: BTreeSet<HostAccessScope>,
    #[serde(default = "default_host_access_resources")]
    resources: BTreeSet<HostAccessResourceSelector>,
    #[serde(default)]
    parent_grant_id: Option<String>,
    #[serde(default)]
    delegation_depth: u16,
    token_digest: String,
    created_at_ms: i64,
    expires_at_ms: i64,
    revoked_at_ms: Option<i64>,
}

fn selector_covers(
    owned: &HostAccessResourceSelector,
    candidate: &HostAccessResourceSelector,
) -> bool {
    owned.kind == candidate.kind
        && match (&owned.id, &candidate.id) {
            (None, _) => true,
            (Some(owned), Some(candidate)) => owned == candidate,
            (Some(_), None) => false,
        }
}

fn ensure_delegated_authority(
    parent: Option<&StoredGrant>,
    scopes: &BTreeSet<HostAccessScope>,
    resources: &BTreeSet<HostAccessResourceSelector>,
    delegation_depth: u16,
    expires_at_ms: i64,
    issued_at_ms: i64,
) -> anyhow::Result<()> {
    let Some(parent) = parent else {
        anyhow::ensure!(
            delegation_depth == 0,
            "root-issued Host grant must have delegation depth zero"
        );
        return Ok(());
    };
    anyhow::ensure!(
        parent.revoked_at_ms.is_none() && parent.expires_at_ms > issued_at_ms,
        "Host grant parent is not active"
    );
    anyhow::ensure!(
        delegation_depth == parent.delegation_depth.saturating_add(1)
            && delegation_depth <= MAX_DELEGATION_DEPTH,
        "Host grant delegation depth is invalid"
    );
    anyhow::ensure!(
        scopes.is_subset(&parent.scopes) && !scopes.contains(&HostAccessScope::AccessManage),
        "delegated Host grant scopes exceed its parent"
    );
    anyhow::ensure!(
        resources.iter().all(|candidate| parent
            .resources
            .iter()
            .any(|owned| selector_covers(owned, candidate))),
        "delegated Host grant resources exceed its parent"
    );
    anyhow::ensure!(
        expires_at_ms <= parent.expires_at_ms,
        "delegated Host grant outlives its parent"
    );
    Ok(())
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum HostAccessJournalEvent {
    PairingCreated {
        pairing: StoredPairing,
    },
    PairingClaimed {
        pairing_id: String,
        claimed_at_ms: i64,
        grant: StoredGrant,
    },
    PairingCancelled {
        pairing_id: String,
        cancelled_at_ms: i64,
    },
    GrantRevoked {
        grant_id: String,
        revoked_at_ms: i64,
    },
    GrantsRevoked {
        grant_ids: Vec<String>,
        revoked_at_ms: i64,
    },
}

#[derive(Debug, Default)]
struct HostAccessState {
    next_sequence: EventSequence,
    pairings: HashMap<String, StoredPairing>,
    grants: HashMap<String, StoredGrant>,
}

#[derive(Debug)]
pub struct HostAccessRegistry {
    state: Mutex<HostAccessState>,
    instance_id: uuid::Uuid,
}

impl Default for HostAccessRegistry {
    fn default() -> Self {
        Self {
            state: Mutex::new(HostAccessState::default()),
            instance_id: uuid::Uuid::new_v4(),
        }
    }
}

pub fn host_access_registry() -> Arc<HostAccessRegistry> {
    Arc::new(HostAccessRegistry::default())
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAccessGrantView {
    pub id: String,
    pub device_name: String,
    pub scopes: Vec<HostAccessScope>,
    pub resources: Vec<HostAccessResourceSelector>,
    pub parent_grant_id: Option<String>,
    pub delegation_depth: u16,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    pub revoked_at_ms: Option<i64>,
    pub active: bool,
}

impl StoredGrant {
    fn view(&self, now_ms: i64) -> HostAccessGrantView {
        HostAccessGrantView {
            id: self.id.clone(),
            device_name: self.device_name.clone(),
            scopes: self.scopes.iter().copied().collect(),
            resources: self.resources.iter().cloned().collect(),
            parent_grant_id: self.parent_grant_id.clone(),
            delegation_depth: self.delegation_depth,
            created_at_ms: self.created_at_ms,
            expires_at_ms: self.expires_at_ms,
            revoked_at_ms: self.revoked_at_ms,
            active: self.revoked_at_ms.is_none() && self.expires_at_ms > now_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HostPairingView {
    pub id: String,
    pub device_name: String,
    pub scopes: Vec<HostAccessScope>,
    pub resources: Vec<HostAccessResourceSelector>,
    pub parent_grant_id: Option<String>,
    pub delegation_depth: u16,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    pub grant_expires_at_ms: i64,
    pub status: String,
    pub grant_id: Option<String>,
}

impl StoredPairing {
    fn view(&self, now_ms: i64) -> HostPairingView {
        let (status, grant_id) = match &self.status {
            HostPairingStatus::Pending if self.expires_at_ms <= now_ms => {
                ("expired".to_string(), None)
            }
            HostPairingStatus::Pending => ("pending".to_string(), None),
            HostPairingStatus::Claimed { grant_id, .. } => {
                ("claimed".to_string(), Some(grant_id.clone()))
            }
            HostPairingStatus::Cancelled { .. } => ("cancelled".to_string(), None),
        };
        HostPairingView {
            id: self.id.clone(),
            device_name: self.device_name.clone(),
            scopes: self.scopes.iter().copied().collect(),
            resources: self.resources.iter().cloned().collect(),
            parent_grant_id: self.parent_grant_id.clone(),
            delegation_depth: self.delegation_depth,
            created_at_ms: self.created_at_ms,
            expires_at_ms: self.expires_at_ms,
            grant_expires_at_ms: self.grant_expires_at_ms,
            status,
            grant_id,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreatePairingRequest {
    device_name: String,
    scopes: BTreeSet<HostAccessScope>,
    #[serde(default = "default_host_access_resources")]
    resources: BTreeSet<HostAccessResourceSelector>,
    #[serde(default)]
    pairing_ttl_secs: Option<u64>,
    #[serde(default)]
    grant_ttl_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
struct CreatePairingResponse {
    pairing: HostPairingView,
    pairing_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimPairingRequest {
    pairing_token: String,
}

#[derive(Debug, Serialize)]
struct ClaimPairingResponse {
    grant: HostAccessGrantView,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BulkRevokeGrantsRequest {
    grant_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BulkRevokeGrantsResponse {
    grants: Vec<HostAccessGrantView>,
}

#[derive(Debug, Serialize)]
struct HostAccessOverview {
    identity: HostAccessIdentityView,
    grants: Vec<HostAccessGrantView>,
    pairings: Vec<HostPairingView>,
}

#[derive(Debug, Serialize)]
pub struct HostAccessIdentityView {
    pub kind: HostAccessIdentityKind,
    pub grant_id: Option<String>,
    pub device_name: String,
    pub scopes: Vec<HostAccessScope>,
    pub resources: Vec<HostAccessResourceSelector>,
    pub delegation_chain: Vec<String>,
    pub expires_at_ms: Option<i64>,
}

impl From<&HostAccessIdentity> for HostAccessIdentityView {
    fn from(identity: &HostAccessIdentity) -> Self {
        Self {
            kind: identity.kind,
            grant_id: identity.grant_id.clone(),
            device_name: identity.device_name.clone(),
            scopes: identity.scopes.iter().copied().collect(),
            resources: identity.resources.iter().cloned().collect(),
            delegation_chain: identity.delegation_chain.clone(),
            expires_at_ms: identity.expires_at_ms,
        }
    }
}

impl HostAccessRegistry {
    pub(crate) fn instance_id(&self) -> uuid::Uuid {
        self.instance_id
    }

    fn grant_is_active(state: &HostAccessState, grant_id: &str, now_ms: i64) -> bool {
        let mut current = Some(grant_id);
        let mut visited = BTreeSet::new();
        while let Some(id) = current {
            if !visited.insert(id.to_string())
                || visited.len() > usize::from(MAX_DELEGATION_DEPTH) + 1
            {
                return false;
            }
            let Some(grant) = state.grants.get(id) else {
                return false;
            };
            if grant.revoked_at_ms.is_some() || grant.expires_at_ms <= now_ms {
                return false;
            }
            current = grant.parent_grant_id.as_deref();
        }
        true
    }

    fn next_sequence(&self) -> EventSequence {
        self.state
            .lock()
            .expect("Host access state lock poisoned")
            .next_sequence
    }

    fn apply_event(&self, envelope: &EventEnvelope) -> anyhow::Result<()> {
        anyhow::ensure!(
            envelope.session_id == HOST_ACCESS_JOURNAL_SESSION
                && envelope.kind == HOST_ACCESS_JOURNAL_EVENT,
            "invalid Host access journal envelope"
        );
        let transition: HostAccessJournalEvent = serde_json::from_value(envelope.payload.clone())?;
        let mut state = self.state.lock().expect("Host access state lock poisoned");
        if envelope.sequence < state.next_sequence {
            return Ok(());
        }
        anyhow::ensure!(
            envelope.sequence == state.next_sequence,
            "Host access journal sequence is not contiguous"
        );
        match transition {
            HostAccessJournalEvent::PairingCreated { pairing } => {
                anyhow::ensure!(
                    matches!(pairing.status, HostPairingStatus::Pending),
                    "new Host pairing must be pending"
                );
                anyhow::ensure!(
                    !state.pairings.contains_key(&pairing.id),
                    "Host pairing id was reused"
                );
                let parent = pairing
                    .parent_grant_id
                    .as_deref()
                    .map(|parent_id| {
                        state.grants.get(parent_id).ok_or_else(|| {
                            anyhow::anyhow!("Host pairing parent grant does not exist")
                        })
                    })
                    .transpose()?;
                ensure_delegated_authority(
                    parent,
                    &pairing.scopes,
                    &pairing.resources,
                    pairing.delegation_depth,
                    pairing.grant_expires_at_ms,
                    pairing.created_at_ms,
                )?;
                state.pairings.insert(pairing.id.clone(), pairing);
            }
            HostAccessJournalEvent::PairingClaimed {
                pairing_id,
                claimed_at_ms,
                grant,
            } => {
                anyhow::ensure!(
                    !state.grants.contains_key(&grant.id),
                    "Host grant id was reused"
                );
                let pairing = state
                    .pairings
                    .get(&pairing_id)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("claimed Host pairing does not exist"))?;
                anyhow::ensure!(
                    matches!(pairing.status, HostPairingStatus::Pending)
                        && claimed_at_ms <= pairing.expires_at_ms,
                    "Host pairing was not claimable"
                );
                anyhow::ensure!(
                    grant.device_name == pairing.device_name
                        && grant.scopes == pairing.scopes
                        && grant.resources == pairing.resources
                        && grant.parent_grant_id == pairing.parent_grant_id
                        && grant.delegation_depth == pairing.delegation_depth
                        && grant.expires_at_ms == pairing.grant_expires_at_ms,
                    "Host grant does not match its pairing"
                );
                let parent = grant
                    .parent_grant_id
                    .as_deref()
                    .map(|parent_id| {
                        state
                            .grants
                            .get(parent_id)
                            .ok_or_else(|| anyhow::anyhow!("Host grant parent does not exist"))
                    })
                    .transpose()?;
                ensure_delegated_authority(
                    parent,
                    &grant.scopes,
                    &grant.resources,
                    grant.delegation_depth,
                    grant.expires_at_ms,
                    claimed_at_ms,
                )?;
                state
                    .pairings
                    .get_mut(&pairing_id)
                    .expect("claimed Host pairing was validated above")
                    .status = HostPairingStatus::Claimed {
                    grant_id: grant.id.clone(),
                    claimed_at_ms,
                };
                state.grants.insert(grant.id.clone(), grant);
            }
            HostAccessJournalEvent::PairingCancelled {
                pairing_id,
                cancelled_at_ms,
            } => {
                let pairing = state
                    .pairings
                    .get_mut(&pairing_id)
                    .ok_or_else(|| anyhow::anyhow!("cancelled Host pairing does not exist"))?;
                anyhow::ensure!(
                    matches!(pairing.status, HostPairingStatus::Pending),
                    "only a pending Host pairing can be cancelled"
                );
                pairing.status = HostPairingStatus::Cancelled { cancelled_at_ms };
            }
            HostAccessJournalEvent::GrantRevoked {
                grant_id,
                revoked_at_ms,
            } => {
                apply_grant_revocation(&mut state, &grant_id, revoked_at_ms)?;
            }
            HostAccessJournalEvent::GrantsRevoked {
                grant_ids,
                revoked_at_ms,
            } => {
                anyhow::ensure!(
                    !grant_ids.is_empty() && grant_ids.len() <= MAX_BULK_GRANT_REVOCATIONS,
                    "bulk Host grant revocation has an invalid size"
                );
                let mut previous = None;
                for grant_id in &grant_ids {
                    anyhow::ensure!(
                        previous.is_none_or(|value: &String| value < grant_id),
                        "bulk Host grant revocation ids must be sorted and unique"
                    );
                    let grant = state
                        .grants
                        .get(grant_id)
                        .ok_or_else(|| anyhow::anyhow!("revoked Host grant does not exist"))?;
                    anyhow::ensure!(
                        grant.revoked_at_ms.is_none(),
                        "Host grant was already revoked"
                    );
                    previous = Some(grant_id);
                }
                for grant_id in grant_ids {
                    apply_grant_revocation(&mut state, &grant_id, revoked_at_ms)?;
                }
            }
        }
        state.next_sequence = envelope.sequence.saturating_add(1);
        Ok(())
    }

    fn pairing(&self, pairing_id: &str) -> Option<StoredPairing> {
        self.state
            .lock()
            .expect("Host access state lock poisoned")
            .pairings
            .get(pairing_id)
            .cloned()
    }

    fn grant(&self, grant_id: &str) -> Option<StoredGrant> {
        self.state
            .lock()
            .expect("Host access state lock poisoned")
            .grants
            .get(grant_id)
            .cloned()
    }

    pub(crate) fn grant_is_currently_active(&self, grant_id: &str) -> bool {
        let now_ms = Utc::now().timestamp_millis();
        let state = self.state.lock().expect("Host access state lock poisoned");
        Self::grant_is_active(&state, grant_id, now_ms)
    }

    fn overview(&self, identity: &HostAccessIdentity) -> HostAccessOverview {
        let now_ms = Utc::now().timestamp_millis();
        let state = self.state.lock().expect("Host access state lock poisoned");
        let mut grants = state
            .grants
            .values()
            .map(|grant| {
                let mut view = grant.view(now_ms);
                view.active = Self::grant_is_active(&state, &grant.id, now_ms);
                view
            })
            .collect::<Vec<_>>();
        grants.sort_by_key(|grant| std::cmp::Reverse(grant.created_at_ms));
        let mut pairings = state
            .pairings
            .values()
            .map(|pairing| pairing.view(now_ms))
            .collect::<Vec<_>>();
        pairings.sort_by_key(|pairing| std::cmp::Reverse(pairing.created_at_ms));
        HostAccessOverview {
            identity: identity.into(),
            grants,
            pairings,
        }
    }

    fn authenticate(&self, token: &str) -> Option<HostAccessIdentity> {
        let grant_id = access_token_grant_id(token)?;
        let now_ms = Utc::now().timestamp_millis();
        let state = self.state.lock().expect("Host access state lock poisoned");
        let grant = state.grants.get(grant_id)?.clone();
        if grant.revoked_at_ms.is_some() || grant.expires_at_ms <= now_ms {
            return None;
        }
        let candidate_digest = credential_digest("access", token);
        if !constant_time_eq(candidate_digest.as_bytes(), grant.token_digest.as_bytes()) {
            return None;
        }
        let mut delegation_chain = Vec::new();
        let mut parent_grant_id = grant.parent_grant_id.as_deref();
        let mut remaining_depth = grant.delegation_depth;
        let mut delegated_scopes = grant.scopes.clone();
        let mut delegated_resources = grant.resources.clone();
        let mut delegated_expiry = grant.expires_at_ms;
        while let Some(parent_id) = parent_grant_id {
            if delegation_chain.len() >= usize::from(MAX_DELEGATION_DEPTH)
                || delegation_chain.iter().any(|id| id == parent_id)
            {
                return None;
            }
            let parent = state.grants.get(parent_id)?;
            if parent.revoked_at_ms.is_some()
                || parent.expires_at_ms <= now_ms
                || remaining_depth == 0
                || parent.delegation_depth.saturating_add(1) != remaining_depth
                || !delegated_scopes.is_subset(&parent.scopes)
                || delegated_scopes.contains(&HostAccessScope::AccessManage)
                || !delegated_resources.iter().all(|candidate| {
                    parent
                        .resources
                        .iter()
                        .any(|owned| selector_covers(owned, candidate))
                })
                || delegated_expiry > parent.expires_at_ms
            {
                return None;
            }
            delegation_chain.push(parent.id.clone());
            remaining_depth = parent.delegation_depth;
            delegated_scopes = parent.scopes.clone();
            delegated_resources = parent.resources.clone();
            delegated_expiry = parent.expires_at_ms;
            parent_grant_id = parent.parent_grant_id.as_deref();
        }
        if remaining_depth != 0 {
            return None;
        }
        Some(HostAccessIdentity {
            kind: HostAccessIdentityKind::Device,
            grant_id: Some(grant.id),
            device_name: grant.device_name,
            scopes: grant.scopes,
            resources: grant.resources,
            delegation_chain,
            expires_at_ms: Some(grant.expires_at_ms),
        })
    }
}

fn apply_grant_revocation(
    state: &mut HostAccessState,
    grant_id: &str,
    revoked_at_ms: i64,
) -> anyhow::Result<()> {
    let grant = state
        .grants
        .get_mut(grant_id)
        .ok_or_else(|| anyhow::anyhow!("revoked Host grant does not exist"))?;
    anyhow::ensure!(
        grant.revoked_at_ms.is_none(),
        "Host grant was already revoked"
    );
    grant.revoked_at_ms = Some(revoked_at_ms);
    Ok(())
}

pub(super) fn public_routes<S>() -> Router<AppState<S>>
where
    S: EventStore,
{
    Router::new()
        .route("/host/v1/access/pair/inspect", post(inspect_pairing::<S>))
        .route("/host/v1/access/pair", post(claim_pairing::<S>))
        .layer(middleware::from_fn(access_response_headers))
}

pub(super) fn protected_routes<S>() -> Router<AppState<S>>
where
    S: EventStore,
{
    Router::new()
        .route("/host/v1/access/me", get(access_me))
        .route("/host/v1/access", get(access_overview::<S>))
        .route("/host/v1/access/pairings", post(create_pairing::<S>))
        .route(
            "/host/v1/access/pairings/:pairing_id/cancel",
            post(cancel_pairing::<S>),
        )
        .route(
            "/host/v1/access/grants/:grant_id/revoke",
            post(revoke_grant::<S>),
        )
        .route(
            "/host/v1/access/grants/revoke",
            post(bulk_revoke_grants::<S>),
        )
        .route("/host/v1/access/logout", post(logout))
        .layer(middleware::from_fn(access_response_headers))
}

async fn access_response_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

async fn access_me(
    Extension(identity): Extension<HostAccessIdentity>,
) -> Json<HostAccessIdentityView> {
    Json((&identity).into())
}

async fn access_overview<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
) -> Result<Json<HostAccessOverview>, ServiceError>
where
    S: EventStore,
{
    require_identity_scope(&identity, HostAccessScope::AccessManage)?;
    sync_host_access_journal(state.runtime.store().as_ref(), state.host_access.as_ref())
        .await
        .map_err(access_internal_error)?;
    Ok(Json(state.host_access.overview(&identity)))
}

async fn create_pairing<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Json(request): Json<CreatePairingRequest>,
) -> Result<(StatusCode, Json<CreatePairingResponse>), ServiceError>
where
    S: EventStore,
{
    require_identity_scope(&identity, HostAccessScope::AccessManage)?;
    validate_device_name(&request.device_name)?;
    if request.scopes.is_empty() || !request.scopes.contains(&HostAccessScope::Observe) {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "paired devices must receive at least the observe scope",
        ));
    }
    if request.scopes.iter().any(|scope| !identity.allows(*scope)) {
        return Err(ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "a Host access grant cannot exceed the caller's scopes",
        ));
    }
    for selector in &request.resources {
        validate_resource_selector(selector)?;
    }
    if request
        .resources
        .iter()
        .any(|selector| !identity.allows_selector(selector))
    {
        return Err(ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "a Host access grant cannot exceed the caller's project or target resources",
        ));
    }
    if request.scopes.contains(&HostAccessScope::AccessManage)
        && identity.kind != HostAccessIdentityKind::Root
    {
        return Err(ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "only the Host root credential can delegate access management",
        ));
    }
    let pairing_ttl_secs = request.pairing_ttl_secs.unwrap_or(DEFAULT_PAIRING_TTL_SECS);
    let grant_ttl_secs = request.grant_ttl_secs.unwrap_or(DEFAULT_GRANT_TTL_SECS);
    if !(60..=MAX_PAIRING_TTL_SECS).contains(&pairing_ttl_secs)
        || !(60 * 60..=MAX_GRANT_TTL_SECS).contains(&grant_ttl_secs)
    {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "Host access TTL is outside the supported range",
        ));
    }
    if identity.expires_at_ms.is_some_and(|caller_expiry| {
        Utc::now()
            .timestamp_millis()
            .saturating_add(seconds_to_millis(grant_ttl_secs))
            > caller_expiry
    }) {
        return Err(ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "a delegated Host access grant cannot outlive its parent grant",
        ));
    }

    let delegation_depth = u16::try_from(identity.delegation_chain.len())
        .unwrap_or(u16::MAX)
        .saturating_add(u16::from(identity.kind == HostAccessIdentityKind::Device));
    if delegation_depth > MAX_DELEGATION_DEPTH {
        return Err(ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "Host access delegation depth limit reached",
        ));
    }

    let (pairing, pairing_token) = create_pairing_record(
        state.runtime.store().as_ref(),
        state.host_access.as_ref(),
        request.device_name.trim().to_string(),
        request.scopes,
        request.resources,
        identity.grant_id.clone(),
        delegation_depth,
        pairing_ttl_secs,
        grant_ttl_secs,
    )
    .await
    .map_err(access_internal_error)?;
    Ok((
        StatusCode::CREATED,
        Json(CreatePairingResponse {
            pairing: pairing.view(Utc::now().timestamp_millis()),
            pairing_token,
        }),
    ))
}

async fn claim_pairing<S>(
    State(state): State<AppState<S>>,
    Json(request): Json<ClaimPairingRequest>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: EventStore,
{
    let (grant, access_token) = claim_pairing_record(
        state.runtime.store().as_ref(),
        state.host_access.as_ref(),
        request.pairing_token.trim(),
    )
    .await
    .map_err(|error| {
        tracing::info!(error = %error, "Host pairing claim was rejected");
        ServiceError::with_status(
            StatusCode::UNAUTHORIZED,
            "invalid, expired, or already consumed pairing token",
        )
    })?;
    let now_ms = Utc::now().timestamp_millis();
    let max_age = grant.expires_at_ms.saturating_sub(now_ms).max(0) / 1000;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{REMOTE_HOST_SESSION_COOKIE}={access_token}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age={max_age}"
        ))
        .expect("Host access token is always a valid cookie value"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    Ok((
        StatusCode::CREATED,
        headers,
        Json(ClaimPairingResponse {
            grant: grant.view(now_ms),
        }),
    ))
}

async fn inspect_pairing<S>(
    State(state): State<AppState<S>>,
    Json(request): Json<ClaimPairingRequest>,
) -> Result<Json<HostPairingView>, ServiceError>
where
    S: EventStore,
{
    let pairing = inspect_pairing_record(
        state.runtime.store().as_ref(),
        state.host_access.as_ref(),
        request.pairing_token.trim(),
    )
    .await
    .map_err(|error| {
        tracing::info!(error = %error, "Host pairing inspection was rejected");
        ServiceError::with_status(
            StatusCode::UNAUTHORIZED,
            "invalid, expired, or already consumed pairing token",
        )
    })?;
    Ok(Json(pairing.view(Utc::now().timestamp_millis())))
}

async fn cancel_pairing<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path(pairing_id): Path<String>,
) -> Result<Json<HostPairingView>, ServiceError>
where
    S: EventStore,
{
    require_identity_scope(&identity, HostAccessScope::AccessManage)?;
    let pairing = cancel_pairing_record(
        state.runtime.store().as_ref(),
        state.host_access.as_ref(),
        &pairing_id,
    )
    .await
    .map_err(access_conflict_error)?;
    Ok(Json(pairing.view(Utc::now().timestamp_millis())))
}

async fn revoke_grant<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Path(grant_id): Path<String>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: EventStore,
{
    require_identity_scope(&identity, HostAccessScope::AccessManage)?;
    let grant = revoke_grant_record(
        state.runtime.store().as_ref(),
        state.host_access.as_ref(),
        &grant_id,
    )
    .await
    .map_err(access_conflict_error)?;
    let mut headers = HeaderMap::new();
    if identity.grant_id.as_deref() == Some(grant_id.as_str()) {
        headers.insert(header::SET_COOKIE, expired_remote_cookie());
    }
    Ok((headers, Json(grant.view(Utc::now().timestamp_millis()))))
}

async fn bulk_revoke_grants<S>(
    State(state): State<AppState<S>>,
    Extension(identity): Extension<HostAccessIdentity>,
    Json(request): Json<BulkRevokeGrantsRequest>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: EventStore,
{
    require_identity_scope(&identity, HostAccessScope::AccessManage)?;
    let grant_ids = normalize_bulk_grant_ids(request.grant_ids).map_err(access_conflict_error)?;
    let grants = revoke_grant_records(
        state.runtime.store().as_ref(),
        state.host_access.as_ref(),
        &grant_ids,
    )
    .await
    .map_err(access_conflict_error)?;
    let mut headers = HeaderMap::new();
    if identity
        .grant_id
        .as_ref()
        .is_some_and(|grant_id| grant_ids.binary_search(grant_id).is_ok())
    {
        headers.insert(header::SET_COOKIE, expired_remote_cookie());
    }
    let now_ms = Utc::now().timestamp_millis();
    Ok((
        headers,
        Json(BulkRevokeGrantsResponse {
            grants: grants.into_iter().map(|grant| grant.view(now_ms)).collect(),
        }),
    ))
}

async fn logout() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.append(header::SET_COOKIE, expired_remote_cookie());
    headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{HOST_SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"
        ))
        .expect("static cookie is valid"),
    );
    (StatusCode::NO_CONTENT, headers)
}

fn expired_remote_cookie() -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{REMOTE_HOST_SESSION_COOKIE}=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0"
    ))
    .expect("static cookie is valid")
}

fn require_identity_scope(
    identity: &HostAccessIdentity,
    scope: HostAccessScope,
) -> Result<(), ServiceError> {
    if identity.allows(scope) {
        Ok(())
    } else {
        Err(ServiceError::with_status(
            StatusCode::FORBIDDEN,
            "the authenticated Host access grant does not include the required scope",
        ))
    }
}

fn validate_device_name(name: &str) -> Result<(), ServiceError> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > 80
        || trimmed.chars().any(|ch| ch.is_control())
    {
        return Err(ServiceError::with_status(
            StatusCode::BAD_REQUEST,
            "device_name must contain 1..=80 printable characters",
        ));
    }
    Ok(())
}

fn validate_resource_selector(selector: &HostAccessResourceSelector) -> Result<(), ServiceError> {
    if let Some(id) = selector.id.as_deref() {
        let trimmed = id.trim();
        if trimmed.is_empty()
            || trimmed.len() > 160
            || trimmed != id
            || trimmed.chars().any(|ch| ch.is_control())
        {
            return Err(ServiceError::with_status(
                StatusCode::BAD_REQUEST,
                "Host access resource ids must contain 1..=160 non-control characters without surrounding whitespace",
            ));
        }
    }
    Ok(())
}

fn access_internal_error(error: anyhow::Error) -> ServiceError {
    tracing::warn!(error = %error, "Host access control-plane operation failed");
    ServiceError::with_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Host access control-plane operation failed; details redacted",
    )
}

fn access_conflict_error(error: anyhow::Error) -> ServiceError {
    tracing::info!(error = %error, "Host access state transition was rejected");
    ServiceError::with_status(
        StatusCode::CONFLICT,
        "Host access state changed or is no longer eligible for this operation",
    )
}

async fn append_access_event<S>(
    store: &S,
    registry: &HostAccessRegistry,
    expected_next: EventSequence,
    transition: &HostAccessJournalEvent,
) -> anyhow::Result<Option<EventEnvelope>>
where
    S: EventStore,
{
    store
        .append_with_sequence_if_next(
            HOST_ACCESS_JOURNAL_SESSION.to_string(),
            expected_next,
            HOST_ACCESS_JOURNAL_WRITER.to_string(),
            HOST_ACCESS_JOURNAL_EVENT.to_string(),
            1,
            serde_json::to_value(transition)?,
            json!({
                "owner": "host_control_plane",
                "credential_material": "sha256_digest_only"
            }),
        )
        .await
        .and_then(|event| {
            if let Some(event) = &event {
                registry.apply_event(event)?;
            }
            Ok(event)
        })
}

pub(crate) async fn sync_host_access_journal<S>(
    store: &S,
    registry: &HostAccessRegistry,
) -> anyhow::Result<usize>
where
    S: EventStore,
{
    let mut loaded = 0usize;
    loop {
        let next = registry.next_sequence();
        let events = store
            .list_session_range(
                &HOST_ACCESS_JOURNAL_SESSION.to_string(),
                next.checked_sub(1),
                Some(1_000),
            )
            .await?;
        if events.is_empty() {
            break;
        }
        for event in &events {
            registry.apply_event(event)?;
            loaded = loaded.saturating_add(1);
        }
        if events.len() < 1_000 {
            break;
        }
    }
    Ok(loaded)
}

pub async fn hydrate_host_access_control_plane<S>(
    store: Arc<S>,
    registry: Arc<HostAccessRegistry>,
) -> anyhow::Result<usize>
where
    S: EventStore,
{
    sync_host_access_journal(store.as_ref(), registry.as_ref()).await
}

pub async fn authenticate_host_access_token<S>(
    store: &S,
    registry: &HostAccessRegistry,
    token: &str,
) -> anyhow::Result<Option<HostAccessIdentity>>
where
    S: EventStore,
{
    sync_host_access_journal(store, registry).await?;
    Ok(registry.authenticate(token))
}

async fn create_pairing_record<S>(
    store: &S,
    registry: &HostAccessRegistry,
    device_name: String,
    scopes: BTreeSet<HostAccessScope>,
    resources: BTreeSet<HostAccessResourceSelector>,
    parent_grant_id: Option<String>,
    delegation_depth: u16,
    pairing_ttl_secs: u64,
    grant_ttl_secs: u64,
) -> anyhow::Result<(StoredPairing, String)>
where
    S: EventStore,
{
    let id = plurora_core::new_id("pair");
    let pairing_token = format!("yggpair.{id}.{}", random_secret());
    let now_ms = Utc::now().timestamp_millis();
    let pairing = StoredPairing {
        id,
        device_name,
        scopes,
        resources,
        parent_grant_id,
        delegation_depth,
        secret_digest: credential_digest("pairing", &pairing_token),
        created_at_ms: now_ms,
        expires_at_ms: now_ms.saturating_add(seconds_to_millis(pairing_ttl_secs)),
        grant_expires_at_ms: now_ms.saturating_add(seconds_to_millis(grant_ttl_secs)),
        status: HostPairingStatus::Pending,
    };
    let transition = HostAccessJournalEvent::PairingCreated {
        pairing: pairing.clone(),
    };
    for _ in 0..8 {
        sync_host_access_journal(store, registry).await?;
        let parent = pairing
            .parent_grant_id
            .as_deref()
            .map(|parent_id| {
                registry
                    .grant(parent_id)
                    .ok_or_else(|| anyhow::anyhow!("Host pairing parent grant does not exist"))
            })
            .transpose()?;
        ensure_delegated_authority(
            parent.as_ref(),
            &pairing.scopes,
            &pairing.resources,
            pairing.delegation_depth,
            pairing.grant_expires_at_ms,
            Utc::now().timestamp_millis(),
        )?;
        let expected_next = registry.next_sequence();
        if append_access_event(store, registry, expected_next, &transition)
            .await?
            .is_some()
        {
            return Ok((pairing, pairing_token));
        }
    }
    anyhow::bail!("Host access journal changed too frequently to create a pairing")
}

async fn claim_pairing_record<S>(
    store: &S,
    registry: &HostAccessRegistry,
    pairing_token: &str,
) -> anyhow::Result<(StoredGrant, String)>
where
    S: EventStore,
{
    let pairing_id = pairing_token_id(pairing_token)
        .ok_or_else(|| anyhow::anyhow!("invalid Host pairing token format"))?;
    let grant_id = plurora_core::new_id("grant");
    let access_token = format!("yggaccess.{grant_id}.{}", random_secret());
    for _ in 0..8 {
        sync_host_access_journal(store, registry).await?;
        let pairing = registry
            .pairing(pairing_id)
            .ok_or_else(|| anyhow::anyhow!("Host pairing does not exist"))?;
        let now_ms = Utc::now().timestamp_millis();
        anyhow::ensure!(
            matches!(pairing.status, HostPairingStatus::Pending) && pairing.expires_at_ms > now_ms,
            "Host pairing is expired or already consumed"
        );
        let candidate_digest = credential_digest("pairing", pairing_token);
        anyhow::ensure!(
            constant_time_eq(
                candidate_digest.as_bytes(),
                pairing.secret_digest.as_bytes()
            ),
            "Host pairing secret did not match"
        );
        let parent = pairing
            .parent_grant_id
            .as_deref()
            .map(|parent_id| {
                registry
                    .grant(parent_id)
                    .ok_or_else(|| anyhow::anyhow!("Host pairing parent grant does not exist"))
            })
            .transpose()?;
        ensure_delegated_authority(
            parent.as_ref(),
            &pairing.scopes,
            &pairing.resources,
            pairing.delegation_depth,
            pairing.grant_expires_at_ms,
            now_ms,
        )?;
        let grant = StoredGrant {
            id: grant_id.clone(),
            device_name: pairing.device_name,
            scopes: pairing.scopes,
            resources: pairing.resources,
            parent_grant_id: pairing.parent_grant_id,
            delegation_depth: pairing.delegation_depth,
            token_digest: credential_digest("access", &access_token),
            created_at_ms: now_ms,
            expires_at_ms: pairing.grant_expires_at_ms,
            revoked_at_ms: None,
        };
        let transition = HostAccessJournalEvent::PairingClaimed {
            pairing_id: pairing.id,
            claimed_at_ms: now_ms,
            grant: grant.clone(),
        };
        let expected_next = registry.next_sequence();
        if append_access_event(store, registry, expected_next, &transition)
            .await?
            .is_some()
        {
            return Ok((grant, access_token));
        }
    }
    anyhow::bail!("Host access journal changed too frequently to claim a pairing")
}

async fn inspect_pairing_record<S>(
    store: &S,
    registry: &HostAccessRegistry,
    pairing_token: &str,
) -> anyhow::Result<StoredPairing>
where
    S: EventStore,
{
    let pairing_id = pairing_token_id(pairing_token)
        .ok_or_else(|| anyhow::anyhow!("invalid Host pairing token format"))?;
    sync_host_access_journal(store, registry).await?;
    let pairing = registry
        .pairing(pairing_id)
        .ok_or_else(|| anyhow::anyhow!("Host pairing does not exist"))?;
    anyhow::ensure!(
        matches!(pairing.status, HostPairingStatus::Pending)
            && pairing.expires_at_ms > Utc::now().timestamp_millis(),
        "Host pairing is expired or already consumed"
    );
    let candidate_digest = credential_digest("pairing", pairing_token);
    anyhow::ensure!(
        constant_time_eq(
            candidate_digest.as_bytes(),
            pairing.secret_digest.as_bytes()
        ),
        "Host pairing secret did not match"
    );
    Ok(pairing)
}

async fn cancel_pairing_record<S>(
    store: &S,
    registry: &HostAccessRegistry,
    pairing_id: &str,
) -> anyhow::Result<StoredPairing>
where
    S: EventStore,
{
    for _ in 0..8 {
        sync_host_access_journal(store, registry).await?;
        let pairing = registry
            .pairing(pairing_id)
            .ok_or_else(|| anyhow::anyhow!("Host pairing does not exist"))?;
        anyhow::ensure!(
            matches!(pairing.status, HostPairingStatus::Pending),
            "Host pairing is no longer pending"
        );
        let transition = HostAccessJournalEvent::PairingCancelled {
            pairing_id: pairing_id.to_string(),
            cancelled_at_ms: Utc::now().timestamp_millis(),
        };
        let expected_next = registry.next_sequence();
        if append_access_event(store, registry, expected_next, &transition)
            .await?
            .is_some()
        {
            return registry
                .pairing(pairing_id)
                .ok_or_else(|| anyhow::anyhow!("cancelled Host pairing disappeared"));
        }
    }
    anyhow::bail!("Host access journal changed too frequently to cancel a pairing")
}

async fn revoke_grant_record<S>(
    store: &S,
    registry: &HostAccessRegistry,
    grant_id: &str,
) -> anyhow::Result<StoredGrant>
where
    S: EventStore,
{
    for _ in 0..8 {
        sync_host_access_journal(store, registry).await?;
        let grant = registry
            .grant(grant_id)
            .ok_or_else(|| anyhow::anyhow!("Host grant does not exist"))?;
        anyhow::ensure!(
            grant.revoked_at_ms.is_none(),
            "Host grant is already revoked"
        );
        let transition = HostAccessJournalEvent::GrantRevoked {
            grant_id: grant_id.to_string(),
            revoked_at_ms: Utc::now().timestamp_millis(),
        };
        let expected_next = registry.next_sequence();
        if append_access_event(store, registry, expected_next, &transition)
            .await?
            .is_some()
        {
            return registry
                .grant(grant_id)
                .ok_or_else(|| anyhow::anyhow!("revoked Host grant disappeared"));
        }
    }
    anyhow::bail!("Host access journal changed too frequently to revoke a grant")
}

fn normalize_bulk_grant_ids(mut grant_ids: Vec<String>) -> anyhow::Result<Vec<String>> {
    anyhow::ensure!(!grant_ids.is_empty(), "grant_ids cannot be empty");
    anyhow::ensure!(
        grant_ids.len() <= MAX_BULK_GRANT_REVOCATIONS,
        "at most {MAX_BULK_GRANT_REVOCATIONS} grants can be revoked at once"
    );
    for grant_id in &mut grant_ids {
        *grant_id = grant_id.trim().to_string();
    }
    anyhow::ensure!(
        grant_ids.iter().all(|grant_id| !grant_id.is_empty()),
        "grant_ids cannot contain an empty id"
    );
    grant_ids.sort();
    grant_ids.dedup();
    Ok(grant_ids)
}

async fn revoke_grant_records<S>(
    store: &S,
    registry: &HostAccessRegistry,
    grant_ids: &[String],
) -> anyhow::Result<Vec<StoredGrant>>
where
    S: EventStore,
{
    for _ in 0..8 {
        sync_host_access_journal(store, registry).await?;
        let grants = grant_ids
            .iter()
            .map(|grant_id| {
                registry
                    .grant(grant_id)
                    .ok_or_else(|| anyhow::anyhow!("Host grant does not exist: {grant_id}"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let active_ids = grants
            .iter()
            .filter(|grant| grant.revoked_at_ms.is_none())
            .map(|grant| grant.id.clone())
            .collect::<Vec<_>>();
        if active_ids.is_empty() {
            return Ok(grants);
        }
        let transition = HostAccessJournalEvent::GrantsRevoked {
            grant_ids: active_ids,
            revoked_at_ms: Utc::now().timestamp_millis(),
        };
        let expected_next = registry.next_sequence();
        if append_access_event(store, registry, expected_next, &transition)
            .await?
            .is_some()
        {
            return grant_ids
                .iter()
                .map(|grant_id| {
                    registry
                        .grant(grant_id)
                        .ok_or_else(|| anyhow::anyhow!("revoked Host grant disappeared"))
                })
                .collect();
        }
    }
    anyhow::bail!("Host access journal changed too frequently to revoke grants")
}

fn seconds_to_millis(seconds: u64) -> i64 {
    i64::try_from(seconds)
        .unwrap_or(i64::MAX)
        .saturating_mul(1_000)
}

fn random_secret() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn credential_digest(domain: &str, credential: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"plurora-host-access-v1\0");
    hasher.update(domain.as_bytes());
    hasher.update(b"\0");
    hasher.update(credential.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |difference, (left, right)| difference | (left ^ right))
        == 0
}

fn pairing_token_id(token: &str) -> Option<&str> {
    token
        .strip_prefix("yggpair.")?
        .split_once('.')
        .and_then(|(id, secret)| {
            (!id.is_empty()
                && secret.len() == 64
                && secret.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then_some(id)
        })
}

fn access_token_grant_id(token: &str) -> Option<&str> {
    token
        .strip_prefix("yggaccess.")?
        .split_once('.')
        .and_then(|(id, secret)| {
            (!id.is_empty()
                && secret.len() == 64
                && secret.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then_some(id)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use plurora_runtime::InMemoryEventStore;

    #[tokio::test]
    async fn pairing_is_single_use_and_grants_are_revocable() -> anyhow::Result<()> {
        let store = InMemoryEventStore::default();
        let registry = HostAccessRegistry::default();
        let scopes = BTreeSet::from([HostAccessScope::Observe, HostAccessScope::ProjectOperate]);
        let (_, pairing_token) = create_pairing_record(
            &store,
            &registry,
            "Phone".to_string(),
            scopes.clone(),
            default_host_access_resources(),
            None,
            0,
            300,
            3600,
        )
        .await?;
        let (grant, access_token) = claim_pairing_record(&store, &registry, &pairing_token).await?;
        assert!(claim_pairing_record(&store, &registry, &pairing_token)
            .await
            .is_err());
        let identity = authenticate_host_access_token(&store, &registry, &access_token)
            .await?
            .expect("active device grant authenticates");
        assert_eq!(identity.scopes, scopes);
        revoke_grant_record(&store, &registry, &grant.id).await?;
        assert!(
            authenticate_host_access_token(&store, &registry, &access_token)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn bulk_revocation_is_atomic_and_idempotent() -> anyhow::Result<()> {
        let store = InMemoryEventStore::default();
        let registry = HostAccessRegistry::default();
        let mut grants = Vec::new();
        for device_name in ["Phone", "Tablet"] {
            let (_, pairing_token) = create_pairing_record(
                &store,
                &registry,
                device_name.to_string(),
                BTreeSet::from([HostAccessScope::Observe]),
                default_host_access_resources(),
                None,
                0,
                300,
                3600,
            )
            .await?;
            grants.push(claim_pairing_record(&store, &registry, &pairing_token).await?);
        }

        let missing = vec![grants[0].0.id.clone(), "missing-grant".to_string()];
        assert!(revoke_grant_records(&store, &registry, &missing)
            .await
            .is_err());
        assert!(
            authenticate_host_access_token(&store, &registry, &grants[0].1)
                .await?
                .is_some()
        );

        let ids = normalize_bulk_grant_ids(vec![
            format!(" {} ", grants[1].0.id),
            grants[0].0.id.clone(),
            grants[0].0.id.clone(),
        ])?;
        let revoked = revoke_grant_records(&store, &registry, &ids).await?;
        assert_eq!(revoked.len(), 2);
        assert!(revoked.iter().all(|grant| grant.revoked_at_ms.is_some()));
        assert_eq!(
            revoke_grant_records(&store, &registry, &ids).await?.len(),
            2
        );
        for (_, access_token) in grants {
            assert!(
                authenticate_host_access_token(&store, &registry, &access_token)
                    .await?
                    .is_none()
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn access_journal_hydrates_without_raw_credentials() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let source = HostAccessRegistry::default();
        let (_, pairing_token) = create_pairing_record(
            store.as_ref(),
            &source,
            "Tablet".to_string(),
            BTreeSet::from([HostAccessScope::Observe]),
            default_host_access_resources(),
            None,
            0,
            300,
            3600,
        )
        .await?;
        let (_, access_token) =
            claim_pairing_record(store.as_ref(), &source, &pairing_token).await?;

        let hydrated = Arc::new(HostAccessRegistry::default());
        assert_eq!(
            hydrate_host_access_control_plane(store.clone(), hydrated.clone()).await?,
            2
        );
        assert!(hydrated.authenticate(&access_token).is_some());
        for event in store
            .list_session_range(&HOST_ACCESS_JOURNAL_SESSION.to_string(), None, None)
            .await?
        {
            let serialized = serde_json::to_string(&event.payload)?;
            assert!(!serialized.contains(&pairing_token));
            assert!(!serialized.contains(&access_token));
        }
        Ok(())
    }

    #[tokio::test]
    async fn legacy_access_journal_without_resources_hydrates_as_wildcard() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let registry = Arc::new(HostAccessRegistry::default());
        let now_ms = Utc::now().timestamp_millis();
        let pairing_id = "legacy-pairing";
        let grant_id = "legacy-grant";
        let access_token = format!("yggaccess.{grant_id}.{}", "a".repeat(64));

        let created = json!({
            "kind": "pairing_created",
            "pairing": {
                "id": pairing_id,
                "device_name": "Legacy device",
                "scopes": ["observe"],
                "secret_digest": credential_digest("pairing", "legacy-pairing-token"),
                "created_at_ms": now_ms,
                "expires_at_ms": now_ms + 60_000,
                "grant_expires_at_ms": now_ms + 3_600_000,
                "status": { "kind": "pending" }
            }
        });
        let claimed = json!({
            "kind": "pairing_claimed",
            "pairing_id": pairing_id,
            "claimed_at_ms": now_ms + 1,
            "grant": {
                "id": grant_id,
                "device_name": "Legacy device",
                "scopes": ["observe"],
                "token_digest": credential_digest("access", &access_token),
                "created_at_ms": now_ms + 1,
                "expires_at_ms": now_ms + 3_600_000,
                "revoked_at_ms": null
            }
        });

        for (sequence, payload) in [(0, created), (1, claimed)] {
            store
                .append_with_sequence_if_next(
                    HOST_ACCESS_JOURNAL_SESSION.to_string(),
                    sequence,
                    HOST_ACCESS_JOURNAL_WRITER.to_string(),
                    HOST_ACCESS_JOURNAL_EVENT.to_string(),
                    1,
                    payload,
                    json!({"legacy_fixture": true}),
                )
                .await?
                .expect("legacy journal event appends contiguously");
        }

        assert_eq!(
            hydrate_host_access_control_plane(store, registry.clone()).await?,
            2
        );
        let identity = registry
            .authenticate(&access_token)
            .expect("legacy grant remains authenticatable");
        assert!(identity.allows_project("any-project"));
        assert!(identity.allows_target("any-target"));
        Ok(())
    }

    #[tokio::test]
    async fn delegated_grant_is_project_exact_and_parent_revocation_cascades() -> anyhow::Result<()>
    {
        let store = InMemoryEventStore::default();
        let registry = HostAccessRegistry::default();
        let project_a = BTreeSet::from([HostAccessResourceSelector {
            kind: HostAccessResourceKind::Project,
            id: Some("project-a".to_string()),
        }]);
        let (_, parent_pairing_token) = create_pairing_record(
            &store,
            &registry,
            "Admin tablet".to_string(),
            BTreeSet::from([HostAccessScope::Observe, HostAccessScope::AccessManage]),
            project_a.clone(),
            None,
            0,
            300,
            3600,
        )
        .await?;
        let (parent, _) = claim_pairing_record(&store, &registry, &parent_pairing_token).await?;

        let (_, child_pairing_token) = create_pairing_record(
            &store,
            &registry,
            "Project phone".to_string(),
            BTreeSet::from([HostAccessScope::Observe]),
            project_a,
            Some(parent.id.clone()),
            1,
            300,
            3500,
        )
        .await?;
        let (_, child_access_token) =
            claim_pairing_record(&store, &registry, &child_pairing_token).await?;
        let child = authenticate_host_access_token(&store, &registry, &child_access_token)
            .await?
            .expect("child grant authenticates while its parent is active");
        assert!(child.allows_project("project-a"));
        assert!(!child.allows_project("project-ab"));
        assert_eq!(child.delegation_chain, vec![parent.id.clone()]);

        revoke_grant_record(&store, &registry, &parent.id).await?;
        assert!(
            authenticate_host_access_token(&store, &registry, &child_access_token)
                .await?
                .is_none(),
            "revoking an ancestor must invalidate descendants"
        );
        Ok(())
    }

    #[tokio::test]
    async fn hydration_rejects_an_overbroad_delegated_authority() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let source = HostAccessRegistry::default();
        let project_a = BTreeSet::from([HostAccessResourceSelector {
            kind: HostAccessResourceKind::Project,
            id: Some("project-a".to_string()),
        }]);
        let (_, parent_pairing_token) = create_pairing_record(
            store.as_ref(),
            &source,
            "Parent".to_string(),
            BTreeSet::from([HostAccessScope::Observe, HostAccessScope::AccessManage]),
            project_a,
            None,
            0,
            300,
            3600,
        )
        .await?;
        let (parent, _) =
            claim_pairing_record(store.as_ref(), &source, &parent_pairing_token).await?;
        let now_ms = Utc::now().timestamp_millis();
        let overbroad = StoredPairing {
            id: "malformed-child-pairing".to_string(),
            device_name: "Malformed child".to_string(),
            scopes: BTreeSet::from([HostAccessScope::Observe]),
            resources: BTreeSet::from([HostAccessResourceSelector {
                kind: HostAccessResourceKind::Project,
                id: Some("project-b".to_string()),
            }]),
            parent_grant_id: Some(parent.id),
            delegation_depth: 1,
            secret_digest: credential_digest("pairing", "not-a-credential"),
            created_at_ms: now_ms,
            expires_at_ms: now_ms + 60_000,
            grant_expires_at_ms: now_ms + 3_000_000,
            status: HostPairingStatus::Pending,
        };
        store
            .append_with_sequence_if_next(
                HOST_ACCESS_JOURNAL_SESSION.to_string(),
                2,
                HOST_ACCESS_JOURNAL_WRITER.to_string(),
                HOST_ACCESS_JOURNAL_EVENT.to_string(),
                1,
                serde_json::to_value(HostAccessJournalEvent::PairingCreated {
                    pairing: overbroad,
                })?,
                json!({"malformed_fixture": true}),
            )
            .await?
            .expect("malformed event is present in the raw store fixture");

        let hydrated = Arc::new(HostAccessRegistry::default());
        assert!(
            hydrate_host_access_control_plane(store, hydrated)
                .await
                .is_err(),
            "journal replay must fail closed when delegated resources exceed the parent"
        );
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_revocation_has_one_winner_and_invalidates_the_credential(
    ) -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let registry = Arc::new(HostAccessRegistry::default());
        let (_, pairing_token) = create_pairing_record(
            store.as_ref(),
            registry.as_ref(),
            "Concurrent revoke device".to_string(),
            BTreeSet::from([HostAccessScope::Observe]),
            default_host_access_resources(),
            None,
            0,
            300,
            3600,
        )
        .await?;
        let (grant, access_token) =
            claim_pairing_record(store.as_ref(), registry.as_ref(), &pairing_token).await?;

        let barrier = Arc::new(tokio::sync::Barrier::new(3));
        let revoke = |store: Arc<InMemoryEventStore>,
                      registry: Arc<HostAccessRegistry>,
                      barrier: Arc<tokio::sync::Barrier>| {
            let grant_id = grant.id.clone();
            tokio::spawn(async move {
                barrier.wait().await;
                revoke_grant_record(store.as_ref(), registry.as_ref(), &grant_id).await
            })
        };
        let left = revoke(store.clone(), registry.clone(), barrier.clone());
        let right = revoke(store.clone(), registry.clone(), barrier.clone());
        barrier.wait().await;
        let (left, right) = tokio::join!(left, right,);
        let left = left?;
        let right = right?;
        assert_eq!(
            usize::from(left.is_ok()) + usize::from(right.is_ok()),
            1,
            "the append-if-next journal must serialize concurrent revocation"
        );
        assert!(
            authenticate_host_access_token(store.as_ref(), registry.as_ref(), &access_token)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn expired_grant_never_authenticates() -> anyhow::Result<()> {
        let store = InMemoryEventStore::default();
        let registry = HostAccessRegistry::default();
        let (_, pairing_token) = create_pairing_record(
            &store,
            &registry,
            "Expired device".to_string(),
            BTreeSet::from([HostAccessScope::Observe]),
            default_host_access_resources(),
            None,
            0,
            300,
            0,
        )
        .await?;
        let (_, access_token) = claim_pairing_record(&store, &registry, &pairing_token).await?;
        assert!(
            authenticate_host_access_token(&store, &registry, &access_token)
                .await?
                .is_none()
        );
        Ok(())
    }
}
