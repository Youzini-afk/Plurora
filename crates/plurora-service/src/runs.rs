use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, RwLock};

use anyhow::{anyhow, ensure};
use async_trait::async_trait;
use chrono::Utc;
use plurora_core::{
    EventEnvelope, EventSequence, EVENT_RUN_FAILED, EVENT_RUN_STARTED, EVENT_RUN_STARTING,
    EVENT_RUN_STOPPED, EVENT_RUN_STOPPING, PLATFORM_RUNTIME_ID,
};
use plurora_runtime::{
    EventStore, RunActivation, RunControl, RunLifecycleDriver, RunListRequest, RunMutationResult,
    RunStartRequest, RunStartResult, RunStatusRequest, RunStatusView, RunStopRequest, RunView,
};
use plurora_work::{HealthStatus, NodeInstanceStatus, RunHealth, RunId, RunRecord, RunStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::DevelopmentHostLease;

const JOURNAL_SESSION: &str = "host_runs";
const JOURNAL_WRITER: &str = PLATFORM_RUNTIME_ID;
const JOURNAL_SCHEMA: u16 = 1;
const JOURNAL_PAGE: usize = 1_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
enum RunOperation {
    Start,
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunIdempotencyClaim {
    operation: RunOperation,
    key_hash: String,
    request_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunJournalPayload {
    run: RunView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    idempotency: Option<RunIdempotencyClaim>,
}

#[derive(Debug, Clone)]
struct AppliedIdempotency {
    fingerprint: String,
    run_id: RunId,
}

#[derive(Debug, Default)]
struct RunState {
    next_sequence: EventSequence,
    runs: BTreeMap<RunId, RunView>,
    idempotency: HashMap<(plurora_work::InstallationId, RunOperation, String), AppliedIdempotency>,
}

pub struct RunRegistry {
    store: Arc<dyn EventStore>,
    state: RwLock<RunState>,
    apply: Arc<Mutex<()>>,
    active: Mutex<HashMap<RunId, RunActivation>>,
    driver: RwLock<Option<Arc<dyn RunLifecycleDriver>>>,
    owner_lease: RwLock<Option<DevelopmentHostLease>>,
}

impl std::fmt::Debug for RunRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RunRegistry")
            .field("store", &"configured")
            .field(
                "driver",
                &self
                    .driver
                    .read()
                    .map(|driver| driver.is_some())
                    .unwrap_or(false),
            )
            .finish_non_exhaustive()
    }
}

impl RunRegistry {
    pub fn new(store: Arc<dyn EventStore>) -> Arc<Self> {
        Arc::new(Self {
            store,
            state: RwLock::new(RunState::default()),
            apply: Arc::new(Mutex::new(())),
            active: Mutex::new(HashMap::new()),
            driver: RwLock::new(None),
            owner_lease: RwLock::new(None),
        })
    }

    pub fn install_owner_lease(&self, lease: DevelopmentHostLease) -> anyhow::Result<()> {
        lease.ensure_active()?;
        *self.owner_lease.write().map_err(lock_error)? = Some(lease);
        Ok(())
    }

    pub fn install_driver(&self, driver: Arc<dyn RunLifecycleDriver>) -> anyhow::Result<()> {
        let mut installed = self.driver.write().map_err(lock_error)?;
        ensure!(
            installed.is_none(),
            "Run lifecycle driver is already installed"
        );
        *installed = Some(driver);
        Ok(())
    }

    fn driver(&self) -> anyhow::Result<Arc<dyn RunLifecycleDriver>> {
        self.driver
            .read()
            .map_err(lock_error)?
            .clone()
            .ok_or_else(|| anyhow!("Run lifecycle driver is unavailable"))
    }

    async fn ensure_owner_lease(&self) -> anyhow::Result<()> {
        let lease = self.owner_lease.read().map_err(lock_error)?.clone();
        if let Some(lease) = lease {
            lease.ensure_durable_owner().await?;
        }
        Ok(())
    }

    fn next_sequence(&self) -> anyhow::Result<EventSequence> {
        Ok(self.state.read().map_err(lock_error)?.next_sequence)
    }

    fn apply_event(&self, event: &EventEnvelope) -> anyhow::Result<()> {
        ensure!(
            event.session_id == JOURNAL_SESSION && event.writer_package_id == JOURNAL_WRITER,
            "invalid Run journal envelope"
        );
        let payload: RunJournalPayload = serde_json::from_value(event.payload.clone())?;
        let mut state = self.state.write().map_err(lock_error)?;
        if event.sequence < state.next_sequence {
            return Ok(());
        }
        validate_journal_event(&state, event.sequence, &event.kind, &payload)?;
        if let Some(claim) = payload.idempotency.as_ref() {
            let key = (
                payload.run.record.installation_id.clone(),
                claim.operation.clone(),
                claim.key_hash.clone(),
            );
            if let Some(previous) = state.idempotency.get(&key) {
                ensure!(
                    previous.fingerprint == claim.request_fingerprint
                        && previous.run_id == payload.run.record.run_id,
                    "Run idempotency claim conflicts with durable history"
                );
            } else {
                state.idempotency.insert(
                    key,
                    AppliedIdempotency {
                        fingerprint: claim.request_fingerprint.clone(),
                        run_id: payload.run.record.run_id.clone(),
                    },
                );
            }
        }
        state
            .runs
            .insert(payload.run.record.run_id.clone(), payload.run);
        state.next_sequence = event.sequence.saturating_add(1);
        Ok(())
    }

    async fn sync_journal(&self) -> anyhow::Result<usize> {
        let mut loaded = 0usize;
        loop {
            let next = self.next_sequence()?;
            let events = self
                .store
                .list_session_range(
                    &JOURNAL_SESSION.to_string(),
                    next.checked_sub(1),
                    Some(JOURNAL_PAGE),
                )
                .await?;
            if events.is_empty() {
                break;
            }
            for event in &events {
                self.apply_event(event)?;
                loaded = loaded.saturating_add(1);
            }
            if events.len() < JOURNAL_PAGE {
                break;
            }
        }
        Ok(loaded)
    }

    async fn append(&self, kind: &'static str, payload: RunJournalPayload) -> anyhow::Result<()> {
        self.ensure_owner_lease().await?;
        self.append_after_guards(kind, payload).await
    }

    async fn append_with_installation_authority(
        &self,
        kind: &'static str,
        payload: RunJournalPayload,
        authority: &plurora_runtime::RunMutationAuthority,
        installation_id: &plurora_work::InstallationId,
    ) -> anyhow::Result<()> {
        self.ensure_owner_lease().await?;
        authority
            .refresh_current_for_installation(installation_id)
            .await?;
        self.append_after_guards(kind, payload).await
    }

    async fn append_with_run_authority(
        &self,
        kind: &'static str,
        payload: RunJournalPayload,
        authority: &plurora_runtime::RunMutationAuthority,
        installation_id: &plurora_work::InstallationId,
        run_id: &RunId,
    ) -> anyhow::Result<()> {
        self.ensure_owner_lease().await?;
        authority
            .refresh_current_for_run(installation_id, run_id)
            .await?;
        self.append_after_guards(kind, payload).await
    }

    async fn append_after_guards(
        &self,
        kind: &'static str,
        payload: RunJournalPayload,
    ) -> anyhow::Result<()> {
        let expected = {
            let state = self.state.read().map_err(lock_error)?;
            let expected = state.next_sequence;
            validate_journal_event(&state, expected, kind, &payload)?;
            expected
        };
        let event = self
            .store
            .append_with_sequence_if_next(
                JOURNAL_SESSION.to_string(),
                expected,
                JOURNAL_WRITER.to_string(),
                kind.to_string(),
                JOURNAL_SCHEMA,
                serde_json::to_value(payload)?,
                serde_json::json!({"owner": "host_control_plane", "paths": "none"}),
            )
            .await?
            .ok_or_else(|| anyhow!("Run journal changed concurrently"))?;
        self.apply_event(&event)
    }

    fn lookup_idempotency(
        &self,
        installation_id: &plurora_work::InstallationId,
        operation: RunOperation,
        key: &str,
        fingerprint: &str,
    ) -> anyhow::Result<Option<RunView>> {
        let state = self.state.read().map_err(lock_error)?;
        let Some(claim) = state.idempotency.get(&(
            installation_id.clone(),
            operation,
            idempotency_key_hash(key),
        )) else {
            return Ok(None);
        };
        ensure!(
            claim.fingerprint == fingerprint,
            "idempotency_conflict: idempotency_key was used for a different Run request"
        );
        Ok(state.runs.get(&claim.run_id).cloned())
    }

    async fn resume_starting_start(
        &self,
        request: &RunStartRequest,
        authority: &plurora_runtime::RunMutationAuthority,
        mut current: RunView,
        idempotent: bool,
    ) -> anyhow::Result<RunStartResult> {
        loop {
            if current.record.status != RunStatus::Starting {
                if !is_active(current.record.status) {
                    self.cleanup_terminal_activation(&current.record.run_id)
                        .await?;
                }
                return Ok(RunStartResult {
                    run: Some(current),
                    gaps: Vec::new(),
                    idempotent,
                });
            }

            let activation = {
                let active = self.active.lock().await;
                active.get(&current.record.run_id).map(|activation| {
                    (
                        activation.context_id.clone(),
                        activation.node_instances.clone(),
                        activation.bindings.clone(),
                    )
                })
            };
            let (kind, next) = if let Some((context_id, node_instances, bindings)) = activation {
                (
                    EVENT_RUN_STARTED,
                    running_run(current.clone(), context_id, node_instances, bindings),
                )
            } else {
                (
                    EVENT_RUN_FAILED,
                    interrupted_run(current.clone(), "outcome_unknown"),
                )
            };

            match self
                .append_with_installation_authority(
                    kind,
                    RunJournalPayload {
                        run: next.clone(),
                        idempotency: None,
                    },
                    authority,
                    &request.installation_id,
                )
                .await
            {
                Ok(()) => {
                    if next.record.status == RunStatus::Interrupted {
                        self.cleanup_terminal_activation(&next.record.run_id)
                            .await?;
                    }
                    return Ok(RunStartResult {
                        run: Some(next),
                        gaps: Vec::new(),
                        idempotent,
                    });
                }
                Err(error) if is_run_journal_cas_loss(&error) => {
                    self.sync_journal().await?;
                    current = self
                        .state
                        .read()
                        .map_err(lock_error)?
                        .runs
                        .get(&current.record.run_id)
                        .cloned()
                        .ok_or_else(|| anyhow!("Run disappeared after journal CAS loss"))?;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn stop_starting(
        &self,
        request: &RunStopRequest,
        authority: &plurora_runtime::RunMutationAuthority,
        fingerprint: &str,
        mut current: RunView,
    ) -> anyhow::Result<RunMutationResult> {
        loop {
            ensure!(
                current.revision == request.expected_revision,
                "run_revision_conflict"
            );
            ensure!(
                current.record.status == RunStatus::Starting,
                "run_revision_conflict"
            );
            let interrupted = interrupted_run(current.clone(), "explicit_stop_before_start_commit");
            match self
                .append_with_run_authority(
                    EVENT_RUN_FAILED,
                    RunJournalPayload {
                        run: interrupted.clone(),
                        idempotency: Some(RunIdempotencyClaim {
                            operation: RunOperation::Stop,
                            key_hash: idempotency_key_hash(&request.idempotency_key),
                            request_fingerprint: fingerprint.to_string(),
                        }),
                    },
                    authority,
                    &request.installation_id,
                    &request.run_id,
                )
                .await
            {
                Ok(()) => {
                    self.cleanup_terminal_activation(&request.run_id).await?;
                    return Ok(RunMutationResult {
                        run: interrupted,
                        idempotent: false,
                    });
                }
                Err(error) if is_run_journal_cas_loss(&error) => {
                    self.sync_journal().await?;
                    let durable = self
                        .state
                        .read()
                        .map_err(lock_error)?
                        .runs
                        .get(&request.run_id)
                        .filter(|run| run.record.installation_id == request.installation_id)
                        .cloned()
                        .ok_or_else(|| anyhow!("Run disappeared after journal CAS loss"))?;
                    if !is_active(durable.record.status) {
                        self.cleanup_terminal_activation(&request.run_id).await?;
                        return Ok(RunMutationResult {
                            run: durable,
                            idempotent: true,
                        });
                    }
                    current = durable;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn cleanup_terminal_activation(&self, run_id: &RunId) -> anyhow::Result<()> {
        let driver = self.driver()?;
        let activation = { self.active.lock().await.remove(run_id) };
        let Some(mut activation) = activation else {
            return Ok(());
        };
        if driver.stop(run_id, &mut activation).await.is_err() {
            let replaced = self.active.lock().await.insert(run_id.clone(), activation);
            ensure!(
                replaced.is_none(),
                "Run activation cleanup ownership changed concurrently"
            );
            return Err(anyhow!(
                "Run activation cleanup failed after terminal Run commit"
            ));
        }
        Ok(())
    }

    pub async fn hydrate(&self) -> anyhow::Result<usize> {
        self.ensure_owner_lease().await?;
        let _apply = self.apply.lock().await;
        let loaded = self.sync_journal().await?;
        let active = self
            .state
            .read()
            .map_err(lock_error)?
            .runs
            .values()
            .filter(|run| is_active(run.record.status))
            .cloned()
            .collect::<Vec<_>>();
        for mut run in active {
            run.revision = run.revision.saturating_add(1);
            run.record.status = RunStatus::Interrupted;
            run.record.stopped_at = Some(Utc::now());
            run.record.health = RunHealth {
                status: HealthStatus::Unhealthy,
                reason_code: Some("host_restart".to_string()),
                diagnostic_refs: Vec::new(),
            };
            for node in &mut run.record.node_instances {
                node.status = NodeInstanceStatus::Interrupted;
            }
            self.append(
                EVENT_RUN_FAILED,
                RunJournalPayload {
                    run,
                    idempotency: None,
                },
            )
            .await?;
        }
        self.active.lock().await.clear();
        Ok(loaded)
    }
}

#[async_trait]
impl RunControl for RunRegistry {
    async fn list(&self, request: RunListRequest) -> anyhow::Result<Vec<RunView>> {
        let _apply = self.apply.lock().await;
        self.sync_journal().await?;
        let mut runs = self
            .state
            .read()
            .map_err(lock_error)?
            .runs
            .values()
            .filter(|run| {
                request
                    .installation_id
                    .as_ref()
                    .is_none_or(|installation_id| &run.record.installation_id == installation_id)
                    && request
                        .status
                        .is_none_or(|status| run.record.status == status)
            })
            .cloned()
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| {
            right
                .record
                .started_at
                .cmp(&left.record.started_at)
                .then_with(|| left.record.run_id.cmp(&right.record.run_id))
        });
        Ok(runs)
    }

    async fn get(
        &self,
        request: plurora_runtime::RunGetRequest,
    ) -> anyhow::Result<Option<RunView>> {
        let _apply = self.apply.lock().await;
        self.sync_journal().await?;
        Ok(self
            .state
            .read()
            .map_err(lock_error)?
            .runs
            .get(&request.run_id)
            .filter(|run| run.record.installation_id == request.installation_id)
            .cloned())
    }

    async fn status(&self, request: RunStatusRequest) -> anyhow::Result<RunStatusView> {
        let _apply = self.apply.lock().await;
        self.sync_journal().await?;
        let inspection = self.driver()?.inspect_status(&request).await?;
        let active_run = self
            .state
            .read()
            .map_err(lock_error)?
            .runs
            .values()
            .find(|run| {
                run.record.installation_id == request.installation_id
                    && is_active(run.record.status)
            })
            .cloned();
        Ok(RunStatusView {
            installation_id: request.installation_id,
            installation_revision: inspection.installation_revision,
            work_revision: inspection.work_revision,
            active_run,
            preflight: inspection.preflight,
        })
    }

    async fn start(&self, request: RunStartRequest) -> anyhow::Result<RunStartResult> {
        request.validate()?;
        let authority = request
            .authority
            .as_ref()
            .ok_or_else(|| anyhow!("authority_denied: Run start authority is missing"))?;
        let fingerprint = start_fingerprint(&request);
        let _apply = self.apply.lock().await;
        self.sync_journal().await?;
        authority
            .refresh_current_for_installation(&request.installation_id)
            .await?;
        if let Some(run) = self.lookup_idempotency(
            &request.installation_id,
            RunOperation::Start,
            &request.idempotency_key,
            &fingerprint,
        )? {
            if run.record.status == RunStatus::Starting {
                return self
                    .resume_starting_start(&request, authority, run, true)
                    .await;
            }
            return Ok(RunStartResult {
                run: Some(run),
                gaps: Vec::new(),
                idempotent: true,
            });
        }
        let activation_run_ids = self
            .active
            .lock()
            .await
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        ensure!(
            !self
                .state
                .read()
                .map_err(lock_error)?
                .runs
                .values()
                .any(|run| run.record.installation_id == request.installation_id
                    && (is_active(run.record.status)
                        || activation_run_ids.contains(&run.record.run_id))),
            "active_run_exists: Installation already has an active Run"
        );

        let driver = self.driver()?;
        let preparation = driver.prepare_start(&request).await?;
        ensure!(
            preparation.installation_revision == request.expected_installation_revision,
            "installation_revision_conflict"
        );
        if !preparation.gaps.is_empty() {
            return Ok(RunStartResult {
                run: None,
                gaps: preparation.gaps,
                idempotent: false,
            });
        }

        authority
            .refresh_current_for_installation(&request.installation_id)
            .await?;
        let now = Utc::now();
        let run_id = RunId::new();
        let starting = RunView {
            record: RunRecord {
                run_id: run_id.clone(),
                installation_id: request.installation_id.clone(),
                context_id: None,
                status: RunStatus::Starting,
                node_instances: Vec::new(),
                bindings: Vec::new(),
                started_at: now,
                stopped_at: None,
                health: RunHealth {
                    status: HealthStatus::Unknown,
                    reason_code: None,
                    diagnostic_refs: Vec::new(),
                },
            },
            revision: 1,
            installation_revision: request.expected_installation_revision,
            entrypoint_id: preparation.entrypoint_id.clone(),
        };
        self.append_with_installation_authority(
            EVENT_RUN_STARTING,
            RunJournalPayload {
                run: starting.clone(),
                idempotency: Some(RunIdempotencyClaim {
                    operation: RunOperation::Start,
                    key_hash: idempotency_key_hash(&request.idempotency_key),
                    request_fingerprint: fingerprint,
                }),
            },
            authority,
            &request.installation_id,
        )
        .await?;

        self.ensure_owner_lease().await?;
        if let Err(error) = authority
            .refresh_current_for_installation(&request.installation_id)
            .await
        {
            self.append(
                EVENT_RUN_FAILED,
                RunJournalPayload {
                    run: failed_run(starting, "authority_denied"),
                    idempotency: None,
                },
            )
            .await?;
            return Err(error);
        }
        let mut activation = match driver.activate(&run_id, preparation).await {
            Ok(activation) => activation,
            Err(error) => {
                let failed = failed_run(starting, "activation_failed");
                self.append(
                    EVENT_RUN_FAILED,
                    RunJournalPayload {
                        run: failed,
                        idempotency: None,
                    },
                )
                .await?;
                let _ = error;
                return Err(anyhow!("Run activation failed"));
            }
        };
        let failure_base = starting.clone();
        let running = running_run(
            starting.clone(),
            activation.context_id.clone(),
            activation.node_instances.clone(),
            activation.bindings.clone(),
        );
        if let Err(error) = self
            .append_with_installation_authority(
                EVENT_RUN_STARTED,
                RunJournalPayload {
                    run: running.clone(),
                    idempotency: None,
                },
                authority,
                &request.installation_id,
            )
            .await
        {
            if is_run_journal_cas_loss(&error) {
                self.active.lock().await.insert(run_id, activation);
                return self
                    .resume_starting_start(&request, authority, starting, false)
                    .await;
            }
            if driver.stop(&run_id, &mut activation).await.is_err() {
                self.active.lock().await.insert(run_id, activation);
                return Err(anyhow!(
                    "outcome_unknown: Run activation commit failed and cleanup requires retry"
                ));
            }
            let _ = self
                .append(
                    EVENT_RUN_FAILED,
                    RunJournalPayload {
                        run: failed_run(failure_base, "activation_commit_failed"),
                        idempotency: None,
                    },
                )
                .await;
            return Err(error);
        }
        self.active.lock().await.insert(run_id, activation);
        Ok(RunStartResult {
            run: Some(running),
            gaps: Vec::new(),
            idempotent: false,
        })
    }

    async fn stop(&self, request: RunStopRequest) -> anyhow::Result<RunMutationResult> {
        request.validate()?;
        let authority = request
            .authority
            .as_ref()
            .ok_or_else(|| anyhow!("authority_denied: Run stop authority is missing"))?;
        let fingerprint = stop_fingerprint(&request);
        let _apply = self.apply.lock().await;
        self.sync_journal().await?;
        authority
            .refresh_current_for_run(&request.installation_id, &request.run_id)
            .await?;
        let (stopping, replayed, terminal_claim) = if let Some(run) = self.lookup_idempotency(
            &request.installation_id,
            RunOperation::Stop,
            &request.idempotency_key,
            &fingerprint,
        )? {
            if run.record.status == RunStatus::Stopping {
                (run, true, None)
            } else {
                self.ensure_owner_lease().await?;
                authority
                    .refresh_current_for_run(&request.installation_id, &request.run_id)
                    .await?;
                self.cleanup_terminal_activation(&request.run_id).await?;
                return Ok(RunMutationResult {
                    run,
                    idempotent: true,
                });
            }
        } else {
            let current = self
                .state
                .read()
                .map_err(lock_error)?
                .runs
                .get(&request.run_id)
                .filter(|run| run.record.installation_id == request.installation_id)
                .cloned()
                .ok_or_else(|| anyhow!("Run not found for the Installation"))?;
            ensure!(
                current.revision == request.expected_revision,
                "run_revision_conflict"
            );
            if !is_active(current.record.status) {
                return Ok(RunMutationResult {
                    run: current,
                    idempotent: true,
                });
            }
            if current.record.status == RunStatus::Starting {
                return self
                    .stop_starting(&request, authority, &fingerprint, current)
                    .await;
            }
            if current.record.status == RunStatus::Stopping {
                (
                    current,
                    false,
                    Some(RunIdempotencyClaim {
                        operation: RunOperation::Stop,
                        key_hash: idempotency_key_hash(&request.idempotency_key),
                        request_fingerprint: fingerprint,
                    }),
                )
            } else {
                let mut stopping = current;
                stopping.revision = stopping.revision.saturating_add(1);
                stopping.record.status = RunStatus::Stopping;
                for node in &mut stopping.record.node_instances {
                    node.status = NodeInstanceStatus::Stopping;
                }
                self.append_with_run_authority(
                    EVENT_RUN_STOPPING,
                    RunJournalPayload {
                        run: stopping.clone(),
                        idempotency: Some(RunIdempotencyClaim {
                            operation: RunOperation::Stop,
                            key_hash: idempotency_key_hash(&request.idempotency_key),
                            request_fingerprint: fingerprint,
                        }),
                    },
                    authority,
                    &request.installation_id,
                    &request.run_id,
                )
                .await?;
                (stopping, false, None)
            }
        };
        if !self.active.lock().await.contains_key(&request.run_id) {
            let interrupted = interrupted_run(stopping, "outcome_unknown");
            self.append(
                EVENT_RUN_FAILED,
                RunJournalPayload {
                    run: interrupted.clone(),
                    idempotency: terminal_claim,
                },
            )
            .await?;
            return Ok(RunMutationResult {
                run: interrupted,
                idempotent: replayed,
            });
        }
        let mut stopped = stopping;
        stopped.revision = stopped.revision.saturating_add(1);
        stopped.record.status = RunStatus::Stopped;
        stopped.record.stopped_at = Some(Utc::now());
        stopped.record.health = RunHealth {
            status: HealthStatus::Unknown,
            reason_code: Some("explicit_stop".to_string()),
            diagnostic_refs: Vec::new(),
        };
        for node in &mut stopped.record.node_instances {
            node.status = NodeInstanceStatus::Stopped;
        }
        self.append_with_run_authority(
            EVENT_RUN_STOPPED,
            RunJournalPayload {
                run: stopped.clone(),
                idempotency: terminal_claim,
            },
            authority,
            &request.installation_id,
            &request.run_id,
        )
        .await?;
        self.ensure_owner_lease().await?;
        authority
            .refresh_current_for_run(&request.installation_id, &request.run_id)
            .await?;
        self.cleanup_terminal_activation(&request.run_id).await?;
        Ok(RunMutationResult {
            run: stopped,
            idempotent: replayed,
        })
    }

    async fn package_activation_lost(
        &self,
        _package_id: &plurora_core::PackageId,
        mut run_ids: Vec<RunId>,
    ) -> anyhow::Result<()> {
        self.ensure_owner_lease().await?;
        let _apply = self.apply.lock().await;
        run_ids.sort();
        run_ids.dedup();

        let mut cleanup_failed = false;
        for run_id in run_ids {
            loop {
                self.sync_journal().await?;
                let Some(current) = self
                    .state
                    .read()
                    .map_err(lock_error)?
                    .runs
                    .get(&run_id)
                    .cloned()
                else {
                    break;
                };

                if !is_active(current.record.status) {
                    self.ensure_owner_lease().await?;
                    if self.cleanup_terminal_activation(&run_id).await.is_err() {
                        cleanup_failed = true;
                    }
                    break;
                }

                let activation_is_owned = self.active.lock().await.contains_key(&run_id);
                let terminal = if activation_is_owned {
                    failed_run(current, "package_activation_lost")
                } else {
                    interrupted_run(current, "package_activation_lost")
                };
                match self
                    .append(
                        EVENT_RUN_FAILED,
                        RunJournalPayload {
                            run: terminal,
                            idempotency: None,
                        },
                    )
                    .await
                {
                    Ok(()) => {
                        self.ensure_owner_lease().await?;
                        if self.cleanup_terminal_activation(&run_id).await.is_err() {
                            cleanup_failed = true;
                        }
                        break;
                    }
                    Err(error) if is_run_journal_cas_loss(&error) => {
                        // Another valid journal append won the sequence. Resync
                        // and converge from durable state rather than guessing a
                        // second terminal revision.
                        continue;
                    }
                    Err(error) => return Err(error),
                }
            }
        }

        if cleanup_failed {
            return Err(anyhow!(
                "Run activation cleanup failed after package activation loss was committed"
            ));
        }
        Ok(())
    }
}

fn validate_journal_event(
    state: &RunState,
    sequence: EventSequence,
    kind: &str,
    payload: &RunJournalPayload,
) -> anyhow::Result<()> {
    payload
        .run
        .record
        .validate()
        .map_err(|_| anyhow!("Run journal payload is invalid"))?;
    let expected_status = match kind {
        EVENT_RUN_STARTING => RunStatus::Starting,
        EVENT_RUN_STARTED => RunStatus::Running,
        EVENT_RUN_STOPPING => RunStatus::Stopping,
        EVENT_RUN_STOPPED => RunStatus::Stopped,
        EVENT_RUN_FAILED => {
            ensure!(
                matches!(
                    payload.run.record.status,
                    RunStatus::Failed | RunStatus::Interrupted
                ),
                "Run failed event has a non-terminal status"
            );
            payload.run.record.status
        }
        _ => return Err(anyhow!("unexpected Run journal event kind")),
    };
    ensure!(
        payload.run.record.status == expected_status,
        "Run journal kind and status disagree"
    );
    ensure!(
        sequence == state.next_sequence,
        "Run journal sequence is not contiguous"
    );

    if let Some(previous) = state.runs.get(&payload.run.record.run_id) {
        ensure!(
            previous.record.installation_id == payload.run.record.installation_id
                && previous.installation_revision == payload.run.installation_revision
                && previous.entrypoint_id == payload.run.entrypoint_id
                && payload.run.revision == previous.revision.saturating_add(1),
            "Run journal mutation changed immutable identity or skipped a revision"
        );
        ensure_valid_transition(previous.record.status, payload.run.record.status)?;
    } else {
        ensure!(
            kind == EVENT_RUN_STARTING && payload.run.revision == 1,
            "Run journal must introduce a Run with starting revision one"
        );
        ensure!(
            !state.runs.values().any(|run| {
                run.record.installation_id == payload.run.record.installation_id
                    && is_active(run.record.status)
            }),
            "Run journal contains more than one active Run for an Installation"
        );
    }

    if let Some(claim) = payload.idempotency.as_ref() {
        let key = (
            payload.run.record.installation_id.clone(),
            claim.operation.clone(),
            claim.key_hash.clone(),
        );
        if let Some(previous) = state.idempotency.get(&key) {
            ensure!(
                previous.fingerprint == claim.request_fingerprint
                    && previous.run_id == payload.run.record.run_id,
                "Run idempotency claim conflicts with durable history"
            );
        }
    }
    Ok(())
}

fn ensure_valid_transition(before: RunStatus, after: RunStatus) -> anyhow::Result<()> {
    let valid = matches!(
        (before, after),
        (
            RunStatus::Starting,
            RunStatus::Running | RunStatus::Failed | RunStatus::Interrupted
        ) | (
            RunStatus::Running | RunStatus::Degraded,
            RunStatus::Stopping | RunStatus::Failed | RunStatus::Interrupted
        ) | (
            RunStatus::Stopping,
            RunStatus::Stopped | RunStatus::Failed | RunStatus::Interrupted
        )
    );
    ensure!(
        valid,
        "Run journal contains an invalid lifecycle transition"
    );
    Ok(())
}

fn is_active(status: RunStatus) -> bool {
    matches!(
        status,
        RunStatus::Starting | RunStatus::Running | RunStatus::Degraded | RunStatus::Stopping
    )
}

fn running_run(
    mut run: RunView,
    context_id: Option<String>,
    node_instances: Vec<plurora_work::NodeInstanceRecord>,
    bindings: Vec<plurora_work::ActiveBindingRecord>,
) -> RunView {
    run.revision = run.revision.saturating_add(1);
    run.record.context_id = context_id;
    run.record.status = RunStatus::Running;
    run.record.node_instances = node_instances;
    run.record.bindings = bindings;
    run.record.health = RunHealth {
        status: HealthStatus::Healthy,
        reason_code: None,
        diagnostic_refs: Vec::new(),
    };
    run
}

fn failed_run(mut run: RunView, reason_code: &str) -> RunView {
    run.revision = run.revision.saturating_add(1);
    run.record.status = RunStatus::Failed;
    run.record.stopped_at = Some(Utc::now());
    run.record.health = RunHealth {
        status: HealthStatus::Unhealthy,
        reason_code: Some(reason_code.to_string()),
        diagnostic_refs: Vec::new(),
    };
    for node in &mut run.record.node_instances {
        node.status = NodeInstanceStatus::Failed;
    }
    run
}

fn interrupted_run(mut run: RunView, reason_code: &str) -> RunView {
    run.revision = run.revision.saturating_add(1);
    run.record.status = RunStatus::Interrupted;
    run.record.stopped_at = Some(Utc::now());
    run.record.health = RunHealth {
        status: HealthStatus::Unknown,
        reason_code: Some(reason_code.to_string()),
        diagnostic_refs: Vec::new(),
    };
    for node in &mut run.record.node_instances {
        node.status = NodeInstanceStatus::Interrupted;
    }
    run
}

fn start_fingerprint(request: &RunStartRequest) -> String {
    fingerprint(&serde_json::json!({
        "operation": "start",
        "installation_id": request.installation_id,
        "expected_installation_revision": request.expected_installation_revision,
        "entrypoint_id": request.entrypoint_id,
    }))
}

fn stop_fingerprint(request: &RunStopRequest) -> String {
    fingerprint(&serde_json::json!({
        "operation": "stop",
        "installation_id": request.installation_id,
        "run_id": request.run_id,
        "expected_revision": request.expected_revision,
    }))
}

fn fingerprint(value: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(value).expect("Run fingerprint value serializes");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn idempotency_key_hash(key: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(key.as_bytes()))
}

fn lock_error<T>(_: std::sync::PoisonError<T>) -> anyhow::Error {
    anyhow!("Run registry lock is poisoned")
}

fn is_run_journal_cas_loss(error: &anyhow::Error) -> bool {
    error.to_string() == "Run journal changed concurrently"
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

    use plurora_core::{ArtifactDescriptor, EventKind, PackageId, SessionId};
    use plurora_runtime::{
        InMemoryEventStore, ProtocolContext, RunEntrypointPreflight, RunGap, RunPreparation,
        RunStatusInspection, Runtime, RuntimeConfig,
    };
    use plurora_work::{InstallationId, WORK_REVISION_TYPE_URI};
    use serde_json::json;

    use super::*;

    fn protocol_error(error: plurora_runtime::ProtocolError) -> anyhow::Error {
        anyhow!("{}: {}", error.code, error.message)
    }

    struct TestDriver {
        blocked: AtomicBool,
        follow_request_revision: AtomicBool,
        installation_revision: AtomicU64,
        inspections: AtomicUsize,
        prepares: AtomicUsize,
        activations: AtomicUsize,
        stops: AtomicUsize,
        stop_failures_remaining: AtomicUsize,
        active_leases: Arc<AtomicUsize>,
        released_runs: std::sync::Mutex<HashSet<RunId>>,
        context_id: std::sync::Mutex<Option<String>>,
    }

    struct TestRunLease(Arc<AtomicUsize>);

    impl Drop for TestRunLease {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    struct CasCollisionStore {
        inner: Arc<InMemoryEventStore>,
        collide_terminal_once: AtomicBool,
        fail_started_once: AtomicBool,
        collide_started_once: AtomicBool,
    }

    impl CasCollisionStore {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                inner: Arc::new(InMemoryEventStore::default()),
                collide_terminal_once: AtomicBool::new(true),
                fail_started_once: AtomicBool::new(false),
                collide_started_once: AtomicBool::new(false),
            })
        }

        fn for_start_recovery() -> Arc<Self> {
            Arc::new(Self {
                inner: Arc::new(InMemoryEventStore::default()),
                collide_terminal_once: AtomicBool::new(false),
                fail_started_once: AtomicBool::new(true),
                collide_started_once: AtomicBool::new(true),
            })
        }
    }

    #[async_trait]
    impl EventStore for CasCollisionStore {
        async fn append(&self, event: EventEnvelope) -> anyhow::Result<()> {
            self.inner.append(event).await
        }

        async fn list_all(&self) -> anyhow::Result<Vec<EventEnvelope>> {
            self.inner.list_all().await
        }

        async fn list_session(&self, session_id: &SessionId) -> anyhow::Result<Vec<EventEnvelope>> {
            self.inner.list_session(session_id).await
        }

        async fn list_session_range(
            &self,
            session_id: &SessionId,
            after_sequence: Option<EventSequence>,
            limit: Option<usize>,
        ) -> anyhow::Result<Vec<EventEnvelope>> {
            self.inner
                .list_session_range(session_id, after_sequence, limit)
                .await
        }

        async fn next_sequence(&self, session_id: &SessionId) -> anyhow::Result<EventSequence> {
            self.inner.next_sequence(session_id).await
        }

        fn subscribe(&self) -> tokio::sync::broadcast::Receiver<EventEnvelope> {
            self.inner.subscribe()
        }

        async fn append_with_sequence_if_next(
            &self,
            session_id: SessionId,
            expected_next_sequence: EventSequence,
            writer_package_id: PackageId,
            kind: EventKind,
            schema_version: u16,
            payload_json: serde_json::Value,
            metadata_json: serde_json::Value,
        ) -> anyhow::Result<Option<EventEnvelope>> {
            if kind == EVENT_RUN_STARTED && self.fail_started_once.swap(false, Ordering::SeqCst) {
                anyhow::bail!("injected ordinary host/run.started append failure");
            }
            let collide = (kind == EVENT_RUN_FAILED
                && self.collide_terminal_once.swap(false, Ordering::SeqCst))
                || (kind == EVENT_RUN_STARTED
                    && self.collide_started_once.swap(false, Ordering::SeqCst));
            if collide {
                let now = Utc::now();
                let collision = RunView {
                    record: RunRecord {
                        run_id: RunId::new(),
                        installation_id: InstallationId::new(),
                        context_id: None,
                        status: RunStatus::Starting,
                        node_instances: Vec::new(),
                        bindings: Vec::new(),
                        started_at: now,
                        stopped_at: None,
                        health: RunHealth {
                            status: HealthStatus::Unknown,
                            reason_code: None,
                            diagnostic_refs: Vec::new(),
                        },
                    },
                    revision: 1,
                    installation_revision: 1,
                    entrypoint_id: "collision".to_string(),
                };
                let inserted = self
                    .inner
                    .append_with_sequence_if_next(
                        session_id,
                        expected_next_sequence,
                        writer_package_id,
                        EVENT_RUN_STARTING.to_string(),
                        schema_version,
                        serde_json::to_value(RunJournalPayload {
                            run: collision,
                            idempotency: None,
                        })?,
                        metadata_json,
                    )
                    .await?;
                ensure!(inserted.is_some(), "injected CAS collision must append");
                return Ok(None);
            }
            self.inner
                .append_with_sequence_if_next(
                    session_id,
                    expected_next_sequence,
                    writer_package_id,
                    kind,
                    schema_version,
                    payload_json,
                    metadata_json,
                )
                .await
        }
    }

    impl TestDriver {
        fn ready() -> Arc<Self> {
            Arc::new(Self {
                blocked: AtomicBool::new(false),
                follow_request_revision: AtomicBool::new(true),
                installation_revision: AtomicU64::new(0),
                inspections: AtomicUsize::new(0),
                prepares: AtomicUsize::new(0),
                activations: AtomicUsize::new(0),
                stops: AtomicUsize::new(0),
                stop_failures_remaining: AtomicUsize::new(0),
                active_leases: Arc::new(AtomicUsize::new(0)),
                released_runs: std::sync::Mutex::new(HashSet::new()),
                context_id: std::sync::Mutex::new(None),
            })
        }

        fn at_revision(revision: u64) -> Arc<Self> {
            Arc::new(Self {
                blocked: AtomicBool::new(false),
                follow_request_revision: AtomicBool::new(false),
                installation_revision: AtomicU64::new(revision),
                inspections: AtomicUsize::new(0),
                prepares: AtomicUsize::new(0),
                activations: AtomicUsize::new(0),
                stops: AtomicUsize::new(0),
                stop_failures_remaining: AtomicUsize::new(0),
                active_leases: Arc::new(AtomicUsize::new(0)),
                released_runs: std::sync::Mutex::new(HashSet::new()),
                context_id: std::sync::Mutex::new(None),
            })
        }

        fn blocked() -> Arc<Self> {
            Arc::new(Self {
                blocked: AtomicBool::new(true),
                follow_request_revision: AtomicBool::new(true),
                installation_revision: AtomicU64::new(0),
                inspections: AtomicUsize::new(0),
                prepares: AtomicUsize::new(0),
                activations: AtomicUsize::new(0),
                stops: AtomicUsize::new(0),
                stop_failures_remaining: AtomicUsize::new(0),
                active_leases: Arc::new(AtomicUsize::new(0)),
                released_runs: std::sync::Mutex::new(HashSet::new()),
                context_id: std::sync::Mutex::new(None),
            })
        }

        fn fail_next_stop(&self) {
            self.stop_failures_remaining.store(1, Ordering::SeqCst);
        }
    }

    #[async_trait]
    impl RunLifecycleDriver for TestDriver {
        async fn inspect_status(
            &self,
            request: &RunStatusRequest,
        ) -> anyhow::Result<RunStatusInspection> {
            self.inspections.fetch_add(1, Ordering::SeqCst);
            let revision = self.installation_revision.load(Ordering::SeqCst);
            let gaps = match request.entrypoint_id.as_deref() {
                Some("missing") => vec![RunGap::new(
                    "artifact_missing",
                    "load the exact locked Package",
                )],
                Some("managed") => vec![RunGap::new(
                    "target_unsatisfied",
                    "apply the required Realization",
                )],
                Some("unsupported") => vec![RunGap::new(
                    "unsupported_backend",
                    "use a Host-supported entrypoint",
                )],
                _ => Vec::new(),
            };
            Ok(RunStatusInspection {
                installation_revision: revision.max(1),
                work_revision: work_revision_descriptor(),
                preflight: request.entrypoint_id.clone().map(|entrypoint_id| {
                    RunEntrypointPreflight {
                        entrypoint_id,
                        gaps,
                    }
                }),
            })
        }

        async fn prepare_start(&self, request: &RunStartRequest) -> anyhow::Result<RunPreparation> {
            self.prepares.fetch_add(1, Ordering::SeqCst);
            let installation_revision = if self.follow_request_revision.load(Ordering::SeqCst) {
                self.installation_revision
                    .store(request.expected_installation_revision, Ordering::SeqCst);
                request.expected_installation_revision
            } else {
                self.installation_revision.load(Ordering::SeqCst)
            };
            if self.blocked.load(Ordering::SeqCst) {
                return Ok(RunPreparation::blocked(
                    installation_revision,
                    request.entrypoint_id.clone(),
                    vec![RunGap::new(
                        "artifact_missing",
                        "load the exact locked Package",
                    )],
                ));
            }
            Ok(RunPreparation::ready(
                installation_revision,
                request.entrypoint_id.clone(),
                Box::new(()),
            ))
        }

        async fn activate(
            &self,
            _run_id: &RunId,
            _preparation: RunPreparation,
        ) -> anyhow::Result<RunActivation> {
            self.activations.fetch_add(1, Ordering::SeqCst);
            self.active_leases.fetch_add(1, Ordering::SeqCst);
            Ok(RunActivation::new(
                Some(
                    self.context_id
                        .lock()
                        .expect("context id lock")
                        .clone()
                        .unwrap_or_else(|| plurora_core::new_id("ctx")),
                ),
                Vec::new(),
                Vec::new(),
                Box::new(TestRunLease(self.active_leases.clone())),
            ))
        }

        async fn stop(
            &self,
            run_id: &RunId,
            _activation: &mut RunActivation,
        ) -> anyhow::Result<()> {
            self.stops.fetch_add(1, Ordering::SeqCst);
            if self.stop_failures_remaining.swap(0, Ordering::SeqCst) > 0 {
                anyhow::bail!("injected Run stop failure");
            }
            ensure!(
                self.released_runs
                    .lock()
                    .expect("released Run lock")
                    .insert(run_id.clone()),
                "Run resources were already released"
            );
            Ok(())
        }
    }

    fn work_revision_descriptor() -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: WORK_REVISION_TYPE_URI.to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    fn runtime_with_driver(
        driver: Arc<TestDriver>,
    ) -> (
        Arc<InMemoryEventStore>,
        Arc<RunRegistry>,
        Runtime<InMemoryEventStore>,
    ) {
        let store = Arc::new(InMemoryEventStore::default());
        let registry = RunRegistry::new(store.clone());
        registry.install_driver(driver).unwrap();
        let runtime = Runtime::new(
            store.clone(),
            RuntimeConfig {
                run_control: registry.clone(),
                ..RuntimeConfig::default()
            },
        );
        (store, registry, runtime)
    }

    fn runtime_with_fault_store(
        store: Arc<CasCollisionStore>,
        driver: Arc<TestDriver>,
    ) -> (Arc<RunRegistry>, Runtime<CasCollisionStore>) {
        let registry = RunRegistry::new(store.clone());
        registry.install_driver(driver).unwrap();
        let runtime = Runtime::new(
            store,
            RuntimeConfig {
                run_control: registry.clone(),
                ..RuntimeConfig::default()
            },
        );
        (registry, runtime)
    }

    async fn start<S>(
        runtime: &Runtime<S>,
        installation_id: &InstallationId,
        expected_revision: u64,
        key: &str,
    ) -> anyhow::Result<RunStartResult>
    where
        S: EventStore,
    {
        Ok(serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("run-registry-test"),
                    "host.run.start",
                    json!({
                        "installation_id": installation_id,
                        "expected_installation_revision": expected_revision,
                        "entrypoint_id": "default",
                        "idempotency_key": key,
                    }),
                )
                .await
                .map_err(protocol_error)?,
        )?)
    }

    async fn seed_starting(
        registry: &RunRegistry,
        installation_id: &InstallationId,
        expected_revision: u64,
        key: &str,
    ) -> anyhow::Result<RunView> {
        let request = RunStartRequest {
            installation_id: installation_id.clone(),
            expected_installation_revision: expected_revision,
            entrypoint_id: "default".to_string(),
            idempotency_key: key.to_string(),
            authority: None,
        };
        let now = Utc::now();
        let starting = RunView {
            record: RunRecord {
                run_id: RunId::new(),
                installation_id: installation_id.clone(),
                context_id: None,
                status: RunStatus::Starting,
                node_instances: Vec::new(),
                bindings: Vec::new(),
                started_at: now,
                stopped_at: None,
                health: RunHealth {
                    status: HealthStatus::Unknown,
                    reason_code: None,
                    diagnostic_refs: Vec::new(),
                },
            },
            revision: 1,
            installation_revision: expected_revision,
            entrypoint_id: request.entrypoint_id.clone(),
        };
        registry
            .append(
                EVENT_RUN_STARTING,
                RunJournalPayload {
                    run: starting.clone(),
                    idempotency: Some(RunIdempotencyClaim {
                        operation: RunOperation::Start,
                        key_hash: idempotency_key_hash(key),
                        request_fingerprint: start_fingerprint(&request),
                    }),
                },
            )
            .await?;
        Ok(starting)
    }

    #[tokio::test]
    async fn run_lifecycle_is_durable_revisioned_and_idempotent() -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (store, _registry, runtime) = runtime_with_driver(driver.clone());
        let installation_id = InstallationId::new();
        let context = runtime
            .open_session(plurora_runtime::OpenSessionRequest::default())
            .await?;
        *driver.context_id.lock().expect("context id lock") = Some(context.id.clone());

        let started = start(&runtime, &installation_id, 7, "start-1").await?;
        let started = started.run.expect("Run starts");
        assert_eq!(started.record.status, RunStatus::Running);
        assert_eq!(started.revision, 2);
        assert_eq!(started.installation_revision, 7);

        let replay = start(&runtime, &installation_id, 7, "start-1").await?;
        assert!(replay.idempotent);
        assert_eq!(replay.run.unwrap().record.run_id, started.record.run_id);
        assert!(start(&runtime, &installation_id, 8, "start-1")
            .await
            .unwrap_err()
            .to_string()
            .contains("idempotency_conflict"));
        assert!(start(&runtime, &installation_id, 7, "another-start")
            .await
            .unwrap_err()
            .to_string()
            .contains("active_run_exists"));

        let listed: Vec<RunView> = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("run-registry-test"),
                    "host.run.list",
                    json!({"installation_id": installation_id}),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert_eq!(listed.len(), 1);
        let installation_only = ProtocolContext::host_device(
            "observe-grant",
            vec!["observe".to_string()],
            vec![plurora_runtime::ProtocolResourceSelector {
                owner: "host".to_string(),
                kind: "installation".to_string(),
                id: Some(installation_id.to_string()),
            }],
            Vec::new(),
            "test",
        );
        let child_view: RunView = serde_json::from_value(
            runtime
                .call_protocol(
                    &installation_only,
                    "host.run.get",
                    json!({
                        "installation_id": installation_id,
                        "run_id": started.record.run_id,
                    }),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert_eq!(child_view, started);
        let exact_run = ProtocolContext::host_device(
            "observe-grant",
            vec!["observe".to_string()],
            vec![
                plurora_runtime::ProtocolResourceSelector {
                    owner: "host".to_string(),
                    kind: "installation".to_string(),
                    id: Some(installation_id.to_string()),
                },
                plurora_runtime::ProtocolResourceSelector {
                    owner: "host".to_string(),
                    kind: "run".to_string(),
                    id: Some(started.record.run_id.to_string()),
                },
            ],
            Vec::new(),
            "test",
        );
        let exact_view: RunView = serde_json::from_value(
            runtime
                .call_protocol(
                    &exact_run,
                    "host.run.get",
                    json!({
                        "installation_id": installation_id,
                        "run_id": started.record.run_id,
                    }),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert_eq!(exact_view, started);
        let got: RunStatusView = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("run-registry-test"),
                    "host.run.status",
                    json!({"installation_id": installation_id}),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert_eq!(got.installation_revision, 7);
        assert_eq!(got.work_revision, work_revision_descriptor());
        assert_eq!(got.active_run, Some(started.clone()));
        assert!(got.preflight.is_none());
        runtime.close_session(context.id).await?;
        let after_context_close: RunStatusView = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("run-registry-test"),
                    "host.run.status",
                    json!({"installation_id": installation_id}),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert_eq!(
            after_context_close
                .active_run
                .expect("active Run remains visible")
                .record
                .status,
            RunStatus::Running
        );

        let stale_stop = runtime
            .call_protocol(
                &ProtocolContext::host_dev("run-registry-test"),
                "host.run.stop",
                json!({
                    "installation_id": installation_id,
                    "run_id": started.record.run_id,
                    "expected_revision": 1,
                    "idempotency_key": "stale-stop",
                }),
            )
            .await
            .expect_err("stale Run revision is rejected");
        assert!(stale_stop.message.contains("run_revision_conflict"));
        assert!(runtime
            .call_protocol(
                &ProtocolContext::host_dev("run-registry-test"),
                "host.run.get",
                json!({
                    "installation_id": InstallationId::new(),
                    "run_id": started.record.run_id,
                }),
            )
            .await
            .is_err());

        let stopped: RunMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("run-registry-test"),
                    "host.run.stop",
                    json!({
                        "installation_id": installation_id,
                        "run_id": started.record.run_id,
                        "expected_revision": 2,
                        "idempotency_key": "stop-1",
                    }),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert_eq!(stopped.run.record.status, RunStatus::Stopped);
        assert_eq!(stopped.run.revision, 4);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 1);

        let replay: RunMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("run-registry-test"),
                    "host.run.stop",
                    json!({
                        "installation_id": installation_id,
                        "run_id": started.record.run_id,
                        "expected_revision": 2,
                        "idempotency_key": "stop-1",
                    }),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert!(replay.idempotent);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 1);
        let conflict = runtime
            .call_protocol(
                &ProtocolContext::host_dev("run-registry-test"),
                "host.run.stop",
                json!({
                    "installation_id": installation_id,
                    "run_id": started.record.run_id,
                    "expected_revision": 3,
                    "idempotency_key": "stop-1",
                }),
            )
            .await
            .expect_err("same stop key with a different fingerprint conflicts");
        assert!(conflict.message.contains("idempotency_conflict"));

        let events = store.list_all().await?;
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == EVENT_RUN_STARTING)
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == EVENT_RUN_STARTED)
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == EVENT_RUN_STOPPING)
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == EVENT_RUN_STOPPED)
                .count(),
            1
        );
        let journal_text = serde_json::to_string(&events)?;
        assert!(!journal_text.contains("start-1"));
        assert!(!journal_text.contains("stop-1"));
        assert!(journal_text.contains("key_hash"));
        Ok(())
    }

    #[tokio::test]
    async fn preflight_gap_creates_no_run_or_effect() -> anyhow::Result<()> {
        let driver = TestDriver::blocked();
        let (store, _registry, runtime) = runtime_with_driver(driver.clone());
        let result = start(&runtime, &InstallationId::new(), 1, "blocked").await?;
        assert!(result.run.is_none());
        assert_eq!(result.gaps[0].reason_code, "artifact_missing");
        assert!(store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?
            .is_empty());
        assert_eq!(driver.stops.load(Ordering::SeqCst), 0);
        Ok(())
    }

    #[tokio::test]
    async fn status_reports_overview_and_structured_preflight_without_effects() -> anyhow::Result<()>
    {
        let driver = TestDriver::at_revision(9);
        let (store, _registry, runtime) = runtime_with_driver(driver.clone());
        let installation_id = InstallationId::new();
        let host = ProtocolContext::host_dev("run-status-test");
        let observe = ProtocolContext::host_device(
            "status-observe",
            vec!["observe".to_string()],
            vec![plurora_runtime::ProtocolResourceSelector {
                owner: "host".to_string(),
                kind: "installation".to_string(),
                id: Some(installation_id.to_string()),
            }],
            Vec::new(),
            "test",
        );

        let legacy = runtime
            .call_protocol(
                &host,
                "host.run.status",
                json!({
                    "installation_id": installation_id,
                    "run_id": RunId::new(),
                }),
            )
            .await
            .expect_err("Run status no longer accepts a Run id");
        assert!(legacy.message.contains("unknown field"));
        assert_eq!(driver.inspections.load(Ordering::SeqCst), 0);

        let overview: RunStatusView = serde_json::from_value(
            runtime
                .call_protocol(
                    &host,
                    "host.run.status",
                    json!({"installation_id": installation_id}),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert_eq!(overview.installation_revision, 9);
        assert_eq!(overview.work_revision, work_revision_descriptor());
        assert!(overview.active_run.is_none());
        assert!(overview.preflight.is_none());

        for (entrypoint_id, expected_gap) in [
            ("default", None),
            ("missing", Some("artifact_missing")),
            ("managed", Some("target_unsatisfied")),
            ("unsupported", Some("unsupported_backend")),
        ] {
            let status: RunStatusView = serde_json::from_value(
                runtime
                    .call_protocol(
                        &host,
                        "host.run.status",
                        json!({
                            "installation_id": installation_id,
                            "entrypoint_id": entrypoint_id,
                        }),
                    )
                    .await
                    .map_err(protocol_error)?,
            )?;
            let preflight = status.preflight.expect("requested preflight is returned");
            assert_eq!(preflight.entrypoint_id, entrypoint_id);
            assert_eq!(
                preflight.gaps.first().map(|gap| gap.reason_code.as_str()),
                expected_gap
            );
        }

        assert!(store.list_all().await?.is_empty());
        assert_eq!(driver.inspections.load(Ordering::SeqCst), 5);
        assert_eq!(driver.prepares.load(Ordering::SeqCst), 0);
        assert_eq!(driver.activations.load(Ordering::SeqCst), 0);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 0);

        let exact: RunStatusView = serde_json::from_value(
            runtime
                .call_protocol(
                    &observe,
                    "host.run.status",
                    json!({"installation_id": installation_id}),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert_eq!(exact.installation_id, installation_id);
        assert!(store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?
            .is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn start_rejects_revision_returned_by_stale_status() -> anyhow::Result<()> {
        let driver = TestDriver::at_revision(11);
        let (store, _registry, runtime) = runtime_with_driver(driver.clone());
        let installation_id = InstallationId::new();
        let status: RunStatusView = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("run-registry-test"),
                    "host.run.status",
                    json!({"installation_id": installation_id}),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        driver.installation_revision.store(12, Ordering::SeqCst);

        let error = start(
            &runtime,
            &installation_id,
            status.installation_revision,
            "stale-status",
        )
        .await
        .expect_err("start must revalidate the current Installation revision");
        assert!(error.to_string().contains("installation_revision_conflict"));
        assert!(store.list_all().await?.is_empty());
        assert_eq!(driver.prepares.load(Ordering::SeqCst), 1);
        assert_eq!(driver.activations.load(Ordering::SeqCst), 0);
        Ok(())
    }

    #[tokio::test]
    async fn starting_replay_commits_owned_activation_without_reactivation() -> anyhow::Result<()> {
        let store = CasCollisionStore::for_start_recovery();
        let driver = TestDriver::ready();
        driver.fail_next_stop();
        let (registry, runtime) = runtime_with_fault_store(store.clone(), driver.clone());
        let installation_id = InstallationId::new();

        let first = start(&runtime, &installation_id, 1, "recover-start")
            .await
            .expect_err("started append and cleanup are injected to fail");
        assert!(first.to_string().contains("outcome_unknown"));
        let starting = registry
            .list(RunListRequest {
                installation_id: Some(installation_id.clone()),
                status: Some(RunStatus::Starting),
            })
            .await?
            .pop()
            .expect("Starting remains durable");
        assert_eq!(starting.revision, 1);
        assert!(registry
            .active
            .lock()
            .await
            .contains_key(&starting.record.run_id));

        let recovered = start(&runtime, &installation_id, 1, "recover-start").await?;
        let running = recovered.run.expect("owned activation commits Running");
        assert!(recovered.idempotent);
        assert_eq!(running.record.run_id, starting.record.run_id);
        assert_eq!(running.record.status, RunStatus::Running);
        assert_eq!(running.revision, 2);
        assert_eq!(driver.activations.load(Ordering::SeqCst), 1);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 1);
        assert!(registry
            .active
            .lock()
            .await
            .contains_key(&running.record.run_id));
        assert_eq!(
            store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_RUN_STARTED)
                .count(),
            1
        );

        let stopped: RunMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("run-recovery-test"),
                    "host.run.stop",
                    json!({
                        "installation_id": installation_id,
                        "run_id": running.record.run_id,
                        "expected_revision": running.revision,
                        "idempotency_key": "release-recovered-start",
                    }),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert_eq!(stopped.run.record.status, RunStatus::Stopped);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 2);
        assert_eq!(
            driver
                .released_runs
                .lock()
                .expect("released Run lock")
                .len(),
            1
        );
        assert!(!registry
            .active
            .lock()
            .await
            .contains_key(&running.record.run_id));
        Ok(())
    }

    #[tokio::test]
    async fn public_stop_interrupts_starting_then_cleans_owned_activation_once(
    ) -> anyhow::Result<()> {
        let store = CasCollisionStore::for_start_recovery();
        let driver = TestDriver::ready();
        driver.fail_next_stop();
        let (registry, runtime) = runtime_with_fault_store(store.clone(), driver.clone());
        let installation_id = InstallationId::new();
        start(&runtime, &installation_id, 1, "stop-starting-start")
            .await
            .expect_err("started append and first cleanup are injected to fail");
        let starting = registry
            .list(RunListRequest {
                installation_id: Some(installation_id.clone()),
                status: Some(RunStatus::Starting),
            })
            .await?
            .pop()
            .expect("Starting remains durable");

        let stopped: RunMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("stop-starting-test"),
                    "host.run.stop",
                    json!({
                        "installation_id": installation_id,
                        "run_id": starting.record.run_id,
                        "expected_revision": starting.revision,
                        "idempotency_key": "stop-starting",
                    }),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert!(!stopped.idempotent);
        assert_eq!(stopped.run.record.status, RunStatus::Interrupted);
        assert_eq!(stopped.run.revision, 2);
        assert_eq!(
            stopped.run.record.health.reason_code.as_deref(),
            Some("explicit_stop_before_start_commit")
        );
        assert_eq!(driver.stops.load(Ordering::SeqCst), 2);
        assert_eq!(
            driver
                .released_runs
                .lock()
                .expect("released Run lock")
                .len(),
            1
        );
        assert!(!registry
            .active
            .lock()
            .await
            .contains_key(&starting.record.run_id));

        let replay: RunMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("stop-starting-test"),
                    "host.run.stop",
                    json!({
                        "installation_id": installation_id,
                        "run_id": starting.record.run_id,
                        "expected_revision": starting.revision,
                        "idempotency_key": "stop-starting",
                    }),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert!(replay.idempotent);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 2);
        let conflict = runtime
            .call_protocol(
                &ProtocolContext::host_dev("stop-starting-test"),
                "host.run.stop",
                json!({
                    "installation_id": installation_id,
                    "run_id": starting.record.run_id,
                    "expected_revision": stopped.run.revision,
                    "idempotency_key": "stop-starting",
                }),
            )
            .await
            .expect_err("same stop key with another revision conflicts");
        assert!(conflict.message.contains("idempotency_conflict"));

        let events = store.list_session(&JOURNAL_SESSION.to_string()).await?;
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == EVENT_RUN_STOPPING)
                .count(),
            0
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == EVENT_RUN_FAILED)
                .count(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn terminal_starting_stop_replay_retries_failed_cleanup_without_second_event(
    ) -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (store, registry, runtime) = runtime_with_driver(driver.clone());
        let installation_id = InstallationId::new();
        let starting = seed_starting(&registry, &installation_id, 6, "terminal-starting").await?;
        let activation = driver
            .activate(
                &starting.record.run_id,
                RunPreparation::ready(6, "default".to_string(), Box::new(())),
            )
            .await?;
        registry
            .active
            .lock()
            .await
            .insert(starting.record.run_id.clone(), activation);
        driver.fail_next_stop();

        let stop_params = json!({
            "installation_id": installation_id,
            "run_id": starting.record.run_id,
            "expected_revision": starting.revision,
            "idempotency_key": "terminal-starting-stop",
        });
        let first = runtime
            .call_protocol(
                &ProtocolContext::host_dev("terminal-starting-stop"),
                "host.run.stop",
                stop_params.clone(),
            )
            .await
            .expect_err("the first terminal cleanup is injected to fail");
        assert!(first.message.contains("cleanup failed"));
        let terminal = registry
            .get(plurora_runtime::RunGetRequest {
                installation_id: installation_id.clone(),
                run_id: starting.record.run_id.clone(),
            })
            .await?
            .expect("Interrupted Run remains durable");
        assert_eq!(terminal.record.status, RunStatus::Interrupted);
        assert_eq!(terminal.revision, 2);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 1);
        assert_eq!(driver.active_leases.load(Ordering::SeqCst), 1);
        assert!(registry
            .active
            .lock()
            .await
            .contains_key(&starting.record.run_id));

        let replay: RunMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("terminal-starting-stop"),
                    "host.run.stop",
                    stop_params,
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert!(replay.idempotent);
        assert_eq!(replay.run.record.status, RunStatus::Interrupted);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 2);
        assert_eq!(driver.active_leases.load(Ordering::SeqCst), 0);
        assert!(!registry
            .active
            .lock()
            .await
            .contains_key(&starting.record.run_id));
        assert_eq!(
            store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_RUN_FAILED)
                .count(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn public_stop_of_starting_resyncs_cas_and_commits_one_terminal() -> anyhow::Result<()> {
        let store = CasCollisionStore::new();
        let driver = TestDriver::ready();
        let (registry, runtime) = runtime_with_fault_store(store.clone(), driver.clone());
        let installation_id = InstallationId::new();
        let starting = seed_starting(&registry, &installation_id, 6, "cas-stop-start").await?;

        let stopped: RunMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("cas-stop-starting"),
                    "host.run.stop",
                    json!({
                        "installation_id": installation_id,
                        "run_id": starting.record.run_id,
                        "expected_revision": starting.revision,
                        "idempotency_key": "cas-stop-starting",
                    }),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert_eq!(stopped.run.record.status, RunStatus::Interrupted);
        assert_eq!(stopped.run.revision, 2);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 0);
        let events = store.list_session(&JOURNAL_SESSION.to_string()).await?;
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == EVENT_RUN_FAILED)
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == EVENT_RUN_STOPPING)
                .count(),
            0
        );
        Ok(())
    }

    #[tokio::test]
    async fn starting_replay_without_activation_interrupts_without_reactivation(
    ) -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (_store, registry, runtime) = runtime_with_driver(driver.clone());
        let installation_id = InstallationId::new();
        let starting = seed_starting(&registry, &installation_id, 4, "unowned-start").await?;

        let recovered = start(&runtime, &installation_id, 4, "unowned-start").await?;
        let interrupted = recovered.run.expect("Starting converges safely");
        assert!(recovered.idempotent);
        assert_eq!(interrupted.record.run_id, starting.record.run_id);
        assert_eq!(interrupted.record.status, RunStatus::Interrupted);
        assert_eq!(interrupted.revision, 2);
        assert_eq!(
            interrupted.record.health.reason_code.as_deref(),
            Some("outcome_unknown")
        );
        assert_eq!(driver.prepares.load(Ordering::SeqCst), 0);
        assert_eq!(driver.activations.load(Ordering::SeqCst), 0);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 0);
        Ok(())
    }

    #[tokio::test]
    async fn new_stop_key_marks_stopping_without_activation_as_outcome_unknown(
    ) -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (store, registry, runtime) = runtime_with_driver(driver.clone());
        let installation_id = InstallationId::new();
        let started = start(&runtime, &installation_id, 1, "start")
            .await?
            .run
            .expect("Run starts");
        let mut stopping = started.clone();
        stopping.revision = 3;
        stopping.record.status = RunStatus::Stopping;
        let original_request = RunStopRequest {
            installation_id: installation_id.clone(),
            run_id: started.record.run_id.clone(),
            expected_revision: 2,
            idempotency_key: "recover-stop".to_string(),
            authority: None,
        };
        registry
            .append(
                EVENT_RUN_STOPPING,
                RunJournalPayload {
                    run: stopping,
                    idempotency: Some(RunIdempotencyClaim {
                        operation: RunOperation::Stop,
                        key_hash: idempotency_key_hash(&original_request.idempotency_key),
                        request_fingerprint: stop_fingerprint(&original_request),
                    }),
                },
            )
            .await?;
        registry.active.lock().await.remove(&started.record.run_id);
        let recovery_request = RunStopRequest {
            installation_id: installation_id.clone(),
            run_id: started.record.run_id.clone(),
            expected_revision: 3,
            idempotency_key: "replacement-stop-key".to_string(),
            authority: None,
        };

        let recovered: RunMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("run-recovery-test"),
                    "host.run.stop",
                    serde_json::to_value(&recovery_request)?,
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert!(!recovered.idempotent);
        assert_eq!(recovered.run.record.status, RunStatus::Interrupted);
        assert_eq!(
            recovered.run.record.health.reason_code.as_deref(),
            Some("outcome_unknown")
        );
        assert_eq!(driver.stops.load(Ordering::SeqCst), 0);

        let replay: RunMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("run-recovery-test"),
                    "host.run.stop",
                    serde_json::to_value(recovery_request)?,
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert!(replay.idempotent);
        assert_eq!(replay.run.record.status, RunStatus::Interrupted);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 0);
        assert_eq!(
            store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_RUN_STOPPING)
                .count(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn replay_resumes_stopping_when_activation_is_still_owned() -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (_store, registry, runtime) = runtime_with_driver(driver.clone());
        let installation_id = InstallationId::new();
        let started = start(&runtime, &installation_id, 1, "start")
            .await?
            .run
            .expect("Run starts");
        let mut stopping = started.clone();
        stopping.revision = 3;
        stopping.record.status = RunStatus::Stopping;
        let request = RunStopRequest {
            installation_id: installation_id.clone(),
            run_id: started.record.run_id.clone(),
            expected_revision: 2,
            idempotency_key: "recover-stop".to_string(),
            authority: None,
        };
        registry
            .append(
                EVENT_RUN_STOPPING,
                RunJournalPayload {
                    run: stopping,
                    idempotency: Some(RunIdempotencyClaim {
                        operation: RunOperation::Stop,
                        key_hash: idempotency_key_hash(&request.idempotency_key),
                        request_fingerprint: stop_fingerprint(&request),
                    }),
                },
            )
            .await?;

        let recovered: RunMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("run-recovery-test"),
                    "host.run.stop",
                    serde_json::to_value(request)?,
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert!(recovered.idempotent);
        assert_eq!(recovered.run.record.status, RunStatus::Stopped);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[tokio::test]
    async fn terminal_running_stop_replay_retries_failed_cleanup_without_second_event(
    ) -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (store, registry, runtime) = runtime_with_driver(driver.clone());
        let installation_id = InstallationId::new();
        let started = start(&runtime, &installation_id, 1, "start")
            .await?
            .run
            .expect("Run starts");
        driver.fail_next_stop();

        let stop_params = json!({
            "installation_id": installation_id,
            "run_id": started.record.run_id,
            "expected_revision": started.revision,
            "idempotency_key": "terminal-running-stop",
        });
        let first_error = runtime
            .call_protocol(
                &ProtocolContext::host_dev("run-retry-test"),
                "host.run.stop",
                stop_params.clone(),
            )
            .await
            .expect_err("the first driver stop is injected to fail");
        assert!(first_error.message.contains("cleanup failed"));
        let terminal = registry
            .get(plurora_runtime::RunGetRequest {
                installation_id: installation_id.clone(),
                run_id: started.record.run_id.clone(),
            })
            .await?
            .expect("Stopped Run remains durable");
        assert_eq!(terminal.record.status, RunStatus::Stopped);
        assert_eq!(terminal.revision, 4);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 1);
        assert_eq!(driver.active_leases.load(Ordering::SeqCst), 1);
        assert!(registry
            .active
            .lock()
            .await
            .contains_key(&started.record.run_id));
        let replacement = start(
            &runtime,
            &installation_id,
            started.installation_revision,
            "replacement-before-cleanup",
        )
        .await
        .expect_err("retained activation ownership blocks a replacement Run");
        assert!(replacement.to_string().contains("active_run_exists"));

        let replay: RunMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("run-retry-test"),
                    "host.run.stop",
                    stop_params.clone(),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        assert!(replay.idempotent);
        assert_eq!(replay.run, terminal);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 2);
        assert_eq!(driver.active_leases.load(Ordering::SeqCst), 0);
        assert_eq!(
            driver
                .released_runs
                .lock()
                .expect("released Run lock")
                .len(),
            1
        );
        assert!(!registry
            .active
            .lock()
            .await
            .contains_key(&started.record.run_id));

        let conflict = runtime
            .call_protocol(
                &ProtocolContext::host_dev("run-retry-test"),
                "host.run.stop",
                json!({
                    "installation_id": installation_id,
                    "run_id": started.record.run_id,
                    "expected_revision": terminal.revision,
                    "idempotency_key": "terminal-running-stop",
                }),
            )
            .await
            .expect_err("the terminal key retains its durable fingerprint");
        assert!(conflict.message.contains("idempotency_conflict"));

        let events = store.list_session(&JOURNAL_SESSION.to_string()).await?;
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == EVENT_RUN_STOPPING)
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == EVENT_RUN_STOPPED)
                .count(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_stop_replay_serializes_cleanup_without_double_stop() -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (store, registry, runtime) = runtime_with_driver(driver.clone());
        let installation_id = InstallationId::new();
        let started = start(&runtime, &installation_id, 1, "concurrent-stop-start")
            .await?
            .run
            .expect("Run starts");
        let stop_params = json!({
            "installation_id": installation_id,
            "run_id": started.record.run_id,
            "expected_revision": started.revision,
            "idempotency_key": "concurrent-stop",
        });
        let left_context = ProtocolContext::host_dev("concurrent-stop-left");
        let right_context = ProtocolContext::host_dev("concurrent-stop-right");

        let (left, right) = tokio::join!(
            runtime.call_protocol(&left_context, "host.run.stop", stop_params.clone(),),
            runtime.call_protocol(&right_context, "host.run.stop", stop_params,),
        );
        let left: RunMutationResult = serde_json::from_value(left.map_err(protocol_error)?)?;
        let right: RunMutationResult = serde_json::from_value(right.map_err(protocol_error)?)?;
        assert_eq!(left.run.record.status, RunStatus::Stopped);
        assert_eq!(right.run, left.run);
        assert_ne!(left.idempotent, right.idempotent);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 1);
        assert_eq!(driver.active_leases.load(Ordering::SeqCst), 0);
        assert!(registry.active.lock().await.is_empty());
        assert_eq!(
            store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_RUN_STOPPED)
                .count(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn stopping_one_installation_run_does_not_disturb_another() -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (_store, registry, runtime) = runtime_with_driver(driver.clone());
        let installation_a = InstallationId::new();
        let installation_b = InstallationId::new();
        let run_a = start(&runtime, &installation_a, 1, "start-a")
            .await?
            .run
            .unwrap();
        let run_b = start(&runtime, &installation_b, 1, "start-b")
            .await?
            .run
            .unwrap();

        runtime
            .call_protocol(
                &ProtocolContext::host_dev("run-registry-test"),
                "host.run.stop",
                json!({
                    "installation_id": installation_a,
                    "run_id": run_a.record.run_id,
                    "expected_revision": 2,
                    "idempotency_key": "stop-a",
                }),
            )
            .await
            .map_err(protocol_error)?;
        assert_eq!(driver.stops.load(Ordering::SeqCst), 1);
        let untouched = registry
            .get(plurora_runtime::RunGetRequest {
                installation_id: installation_b,
                run_id: run_b.record.run_id,
            })
            .await?
            .unwrap();
        assert_eq!(untouched.record.status, RunStatus::Running);
        Ok(())
    }

    #[tokio::test]
    async fn package_activation_loss_commits_one_failed_terminal_before_cleanup(
    ) -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (store, registry, runtime) = runtime_with_driver(driver.clone());
        let installation_id = InstallationId::new();
        let started = start(&runtime, &installation_id, 1, "loss-start")
            .await?
            .run
            .expect("Run starts");
        let package_id = "example/lost-provider".to_string();

        registry
            .package_activation_lost(
                &package_id,
                vec![started.record.run_id.clone(), started.record.run_id.clone()],
            )
            .await?;
        registry
            .package_activation_lost(&package_id, vec![started.record.run_id.clone()])
            .await?;

        let terminal = registry
            .get(plurora_runtime::RunGetRequest {
                installation_id,
                run_id: started.record.run_id.clone(),
            })
            .await?
            .expect("Run remains durable");
        assert_eq!(terminal.record.status, RunStatus::Failed);
        assert_eq!(
            terminal.record.health.reason_code.as_deref(),
            Some("package_activation_lost")
        );
        assert_eq!(driver.stops.load(Ordering::SeqCst), 1);
        assert!(!registry
            .active
            .lock()
            .await
            .contains_key(&started.record.run_id));
        assert_eq!(
            store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_RUN_FAILED)
                .count(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn package_activation_loss_retries_retained_cleanup_before_acknowledging_all_runs(
    ) -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (store, registry, runtime) = runtime_with_driver(driver.clone());
        let installation_a = InstallationId::new();
        let installation_b = InstallationId::new();
        let run_a = start(&runtime, &installation_a, 1, "retry-loss-a")
            .await?
            .run
            .expect("first Run starts");
        let run_b = start(&runtime, &installation_b, 1, "retry-loss-b")
            .await?
            .run
            .expect("second Run starts");
        let package_id = "example/retry-lost-provider".to_string();
        let affected = vec![run_b.record.run_id.clone(), run_a.record.run_id.clone()];
        driver.fail_next_stop();

        let first = registry
            .package_activation_lost(&package_id, affected.clone())
            .await
            .expect_err("one activation cleanup is injected to fail");
        assert!(first.to_string().contains("cleanup failed"));
        assert_eq!(driver.stops.load(Ordering::SeqCst), 2);
        assert_eq!(driver.active_leases.load(Ordering::SeqCst), 1);
        assert_eq!(registry.active.lock().await.len(), 1);
        for (installation_id, run_id) in [
            (installation_a.clone(), run_a.record.run_id.clone()),
            (installation_b.clone(), run_b.record.run_id.clone()),
        ] {
            let terminal = registry
                .get(plurora_runtime::RunGetRequest {
                    installation_id,
                    run_id,
                })
                .await?
                .expect("package loss terminal remains durable");
            assert_eq!(terminal.record.status, RunStatus::Failed);
            assert_eq!(
                terminal.record.health.reason_code.as_deref(),
                Some("package_activation_lost")
            );
        }
        assert_eq!(
            store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_RUN_FAILED)
                .count(),
            2
        );

        registry
            .package_activation_lost(&package_id, affected)
            .await?;
        assert_eq!(driver.stops.load(Ordering::SeqCst), 3);
        assert_eq!(driver.active_leases.load(Ordering::SeqCst), 0);
        assert!(registry.active.lock().await.is_empty());
        assert_eq!(
            store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_RUN_FAILED)
                .count(),
            2
        );
        Ok(())
    }

    #[tokio::test]
    async fn package_activation_loss_interrupts_unknown_ownership_and_is_exact(
    ) -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (_store, registry, runtime) = runtime_with_driver(driver.clone());
        let installation_a = InstallationId::new();
        let installation_b = InstallationId::new();
        let run_a = start(&runtime, &installation_a, 1, "loss-a")
            .await?
            .run
            .unwrap();
        let run_b = start(&runtime, &installation_b, 1, "loss-b")
            .await?
            .run
            .unwrap();
        registry.active.lock().await.remove(&run_a.record.run_id);

        registry
            .package_activation_lost(
                &"example/lost-provider".to_string(),
                vec![run_a.record.run_id.clone()],
            )
            .await?;

        let interrupted = registry
            .get(plurora_runtime::RunGetRequest {
                installation_id: installation_a,
                run_id: run_a.record.run_id,
            })
            .await?
            .unwrap();
        let untouched = registry
            .get(plurora_runtime::RunGetRequest {
                installation_id: installation_b,
                run_id: run_b.record.run_id,
            })
            .await?
            .unwrap();
        assert_eq!(interrupted.record.status, RunStatus::Interrupted);
        assert_eq!(
            interrupted.record.health.reason_code.as_deref(),
            Some("package_activation_lost")
        );
        assert_eq!(untouched.record.status, RunStatus::Running);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 0);
        Ok(())
    }

    #[tokio::test]
    async fn package_activation_loss_fails_all_shared_runs_and_leaves_other_runs_active(
    ) -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (_store, registry, runtime) = runtime_with_driver(driver.clone());
        let installation_a = InstallationId::new();
        let installation_b = InstallationId::new();
        let installation_other = InstallationId::new();
        let run_a = start(&runtime, &installation_a, 1, "shared-a")
            .await?
            .run
            .unwrap();
        let run_b = start(&runtime, &installation_b, 1, "shared-b")
            .await?
            .run
            .unwrap();
        let other = start(&runtime, &installation_other, 1, "other-package")
            .await?
            .run
            .unwrap();

        registry
            .package_activation_lost(
                &"example/shared-provider".to_string(),
                vec![run_b.record.run_id.clone(), run_a.record.run_id.clone()],
            )
            .await?;

        for (installation_id, run_id) in [
            (installation_a, run_a.record.run_id),
            (installation_b, run_b.record.run_id),
        ] {
            let terminal = registry
                .get(plurora_runtime::RunGetRequest {
                    installation_id,
                    run_id,
                })
                .await?
                .unwrap();
            assert_eq!(terminal.record.status, RunStatus::Failed);
        }
        let untouched = registry
            .get(plurora_runtime::RunGetRequest {
                installation_id: installation_other,
                run_id: other.record.run_id.clone(),
            })
            .await?
            .unwrap();
        assert_eq!(untouched.record.status, RunStatus::Running);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 2);
        assert!(registry
            .active
            .lock()
            .await
            .contains_key(&other.record.run_id));
        Ok(())
    }

    #[tokio::test]
    async fn package_activation_loss_resyncs_after_cas_loss_and_commits_once() -> anyhow::Result<()>
    {
        let store = CasCollisionStore::new();
        let registry = RunRegistry::new(store.clone());
        let driver = TestDriver::ready();
        registry.install_driver(driver.clone())?;
        let runtime = Runtime::new(
            store.clone(),
            RuntimeConfig {
                run_control: registry.clone(),
                ..RuntimeConfig::default()
            },
        );
        let installation_id = InstallationId::new();
        let started = start(&runtime, &installation_id, 1, "cas-loss-start")
            .await?
            .run
            .unwrap();

        registry
            .package_activation_lost(
                &"example/cas-loss".to_string(),
                vec![started.record.run_id.clone()],
            )
            .await?;

        let terminal = registry
            .get(plurora_runtime::RunGetRequest {
                installation_id,
                run_id: started.record.run_id,
            })
            .await?
            .unwrap();
        assert_eq!(terminal.record.status, RunStatus::Failed);
        assert_eq!(terminal.revision, 3);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 1);
        assert_eq!(
            store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .iter()
                .filter(|event| event.kind == EVENT_RUN_FAILED)
                .count(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn explicit_stop_wins_race_with_late_package_loss_without_second_terminal(
    ) -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (store, registry, runtime) = runtime_with_driver(driver.clone());
        let installation_id = InstallationId::new();
        let started = start(&runtime, &installation_id, 1, "race-start")
            .await?
            .run
            .unwrap();
        let stopped: RunMutationResult = serde_json::from_value(
            runtime
                .call_protocol(
                    &ProtocolContext::host_dev("loss-stop-race"),
                    "host.run.stop",
                    json!({
                        "installation_id": installation_id,
                        "run_id": started.record.run_id,
                        "expected_revision": started.revision,
                        "idempotency_key": "race-stop",
                    }),
                )
                .await
                .map_err(protocol_error)?,
        )?;
        registry
            .package_activation_lost(
                &"example/lost-provider".to_string(),
                vec![started.record.run_id],
            )
            .await?;

        assert_eq!(stopped.run.record.status, RunStatus::Stopped);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 1);
        let terminal_events = store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?
            .into_iter()
            .filter(|event| matches!(event.kind.as_str(), EVENT_RUN_STOPPED | EVENT_RUN_FAILED))
            .count();
        assert_eq!(terminal_events, 1);
        Ok(())
    }

    struct RevokedGrant;

    #[async_trait]
    impl plurora_runtime::RunAuthorityValidator for RevokedGrant {
        async fn validate_current(
            &self,
            _grant_id: &str,
            _installation_id: &InstallationId,
            _run_id: Option<&RunId>,
        ) -> anyhow::Result<()> {
            anyhow::bail!("revoked")
        }
    }

    struct ToggleGrant {
        current: AtomicBool,
    }

    #[async_trait]
    impl plurora_runtime::RunAuthorityValidator for ToggleGrant {
        async fn validate_current(
            &self,
            _grant_id: &str,
            _installation_id: &InstallationId,
            _run_id: Option<&RunId>,
        ) -> anyhow::Result<()> {
            ensure!(self.current.load(Ordering::SeqCst), "revoked");
            Ok(())
        }
    }

    #[tokio::test]
    async fn starting_recovery_requires_current_authority_before_mutation_or_cleanup(
    ) -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (store, registry, runtime) = runtime_with_driver(driver.clone());
        let installation_id = InstallationId::new();
        let starting = seed_starting(&registry, &installation_id, 2, "authority-retry").await?;
        let validator = Arc::new(ToggleGrant {
            current: AtomicBool::new(false),
        });
        let context = ProtocolContext::host_device(
            "run-grant",
            vec!["run".to_string()],
            vec![plurora_runtime::ProtocolResourceSelector {
                owner: "host".to_string(),
                kind: "installation".to_string(),
                id: Some(installation_id.to_string()),
            }],
            Vec::new(),
            "test",
        )
        .with_verified_authority_expiry(Some(Utc::now().timestamp_millis() + 60_000))
        .with_run_authority_refresh(plurora_runtime::RunAuthorityRefresh::new(validator));

        let error = runtime
            .call_protocol(
                &context,
                "host.run.start",
                json!({
                    "installation_id": installation_id,
                    "expected_installation_revision": 2,
                    "entrypoint_id": "default",
                    "idempotency_key": "authority-retry",
                }),
            )
            .await
            .expect_err("revoked authority cannot recover Starting");
        assert!(error.message.contains("authority_denied"));
        let durable = registry
            .get(plurora_runtime::RunGetRequest {
                installation_id,
                run_id: starting.record.run_id,
            })
            .await?
            .expect("Starting remains durable");
        assert_eq!(durable.record.status, RunStatus::Starting);
        assert_eq!(durable.revision, 1);
        assert_eq!(driver.activations.load(Ordering::SeqCst), 0);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 0);
        assert_eq!(
            store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn starting_recovery_requires_current_host_owner_before_mutation_or_cleanup(
    ) -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let registry = RunRegistry::new(store.clone());
        let driver = TestDriver::ready();
        registry.install_driver(driver.clone())?;
        let development = crate::development_registry();
        let lease = crate::acquire_development_host_lease(store.clone(), development).await?;
        registry.install_owner_lease(lease.clone())?;
        let installation_id = InstallationId::new();
        let starting = seed_starting(&registry, &installation_id, 3, "owner-retry").await?;
        crate::release_development_host_lease(store.clone(), &lease).await?;
        let runtime = Runtime::new(
            store.clone(),
            RuntimeConfig {
                run_control: registry.clone(),
                ..RuntimeConfig::default()
            },
        );

        let error = start(&runtime, &installation_id, 3, "owner-retry")
            .await
            .expect_err("released Host ownership cannot recover Starting");
        assert!(error.to_string().contains("lease"));
        let durable = registry
            .get(plurora_runtime::RunGetRequest {
                installation_id,
                run_id: starting.record.run_id,
            })
            .await?
            .expect("Starting remains durable");
        assert_eq!(durable.record.status, RunStatus::Starting);
        assert_eq!(durable.revision, 1);
        assert_eq!(driver.activations.load(Ordering::SeqCst), 0);
        assert_eq!(driver.stops.load(Ordering::SeqCst), 0);
        assert_eq!(
            store
                .list_session(&JOURNAL_SESSION.to_string())
                .await?
                .len(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn device_authority_is_exact_current_and_zero_effect_on_denial() -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (store, _registry, runtime) = runtime_with_driver(driver.clone());
        let installation_id = InstallationId::new();
        let exact_resource = vec![plurora_runtime::ProtocolResourceSelector {
            owner: "host".to_string(),
            kind: "installation".to_string(),
            id: Some(installation_id.to_string()),
        }];
        let revoked = ProtocolContext::host_device(
            "grant",
            vec!["run".to_string()],
            exact_resource.clone(),
            Vec::new(),
            "test",
        )
        .with_verified_authority_expiry(Some(Utc::now().timestamp_millis() + 60_000))
        .with_run_authority_refresh(plurora_runtime::RunAuthorityRefresh::new(Arc::new(
            RevokedGrant,
        )));
        assert!(runtime
            .call_protocol(
                &revoked,
                "host.run.start",
                json!({
                    "installation_id": installation_id,
                    "expected_installation_revision": 1,
                    "entrypoint_id": "default",
                    "idempotency_key": "revoked",
                }),
            )
            .await
            .is_err());

        let other = InstallationId::new();
        let wrong_installation = ProtocolContext::host_device(
            "grant",
            vec!["run".to_string()],
            exact_resource.clone(),
            Vec::new(),
            "test",
        )
        .with_verified_authority_expiry(Some(Utc::now().timestamp_millis() + 60_000));
        assert!(runtime
            .call_protocol(
                &wrong_installation,
                "host.run.start",
                json!({
                    "installation_id": other,
                    "expected_installation_revision": 1,
                    "entrypoint_id": "default",
                    "idempotency_key": "wrong-installation",
                }),
            )
            .await
            .is_err());

        let expired = ProtocolContext::host_device(
            "grant",
            vec!["run".to_string()],
            exact_resource,
            Vec::new(),
            "test",
        )
        .with_verified_authority_expiry(Some(Utc::now().timestamp_millis() - 1));
        assert!(runtime
            .call_protocol(
                &expired,
                "host.run.start",
                json!({
                    "installation_id": installation_id,
                    "expected_installation_revision": 1,
                    "entrypoint_id": "default",
                    "idempotency_key": "expired",
                }),
            )
            .await
            .is_err());
        assert_eq!(driver.prepares.load(Ordering::SeqCst), 0);
        assert_eq!(driver.activations.load(Ordering::SeqCst), 0);
        assert!(store
            .list_session(&JOURNAL_SESSION.to_string())
            .await?
            .is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn hydrate_interrupts_active_run_without_reactivation() -> anyhow::Result<()> {
        let driver = TestDriver::ready();
        let (store, _source, runtime) = runtime_with_driver(driver);
        let installation_id = InstallationId::new();
        let started = start(&runtime, &installation_id, 3, "start").await?;
        let run_id = started.run.unwrap().record.run_id;

        let recovered = RunRegistry::new(store);
        let recovery_driver = TestDriver::ready();
        recovered.install_driver(recovery_driver.clone())?;
        recovered.hydrate().await?;
        let view = recovered
            .get(plurora_runtime::RunGetRequest {
                installation_id,
                run_id,
            })
            .await?
            .expect("recovered Run");
        assert_eq!(view.record.status, RunStatus::Interrupted);
        assert_eq!(view.revision, 3);
        assert_eq!(recovery_driver.stops.load(Ordering::SeqCst), 0);
        Ok(())
    }

    #[tokio::test]
    async fn hydrate_interrupts_uncommitted_starting_without_reactivation() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let source = RunRegistry::new(store.clone());
        source.install_driver(TestDriver::ready())?;
        let installation_id = InstallationId::new();
        let starting = seed_starting(&source, &installation_id, 5, "restart-starting").await?;

        let recovered = RunRegistry::new(store);
        let recovery_driver = TestDriver::ready();
        recovered.install_driver(recovery_driver.clone())?;
        recovered.hydrate().await?;
        let view = recovered
            .get(plurora_runtime::RunGetRequest {
                installation_id,
                run_id: starting.record.run_id,
            })
            .await?
            .expect("Starting is terminalized on restart");
        assert_eq!(view.record.status, RunStatus::Interrupted);
        assert_eq!(view.revision, 2);
        assert_eq!(
            view.record.health.reason_code.as_deref(),
            Some("host_restart")
        );
        assert_eq!(recovery_driver.activations.load(Ordering::SeqCst), 0);
        assert_eq!(recovery_driver.stops.load(Ordering::SeqCst), 0);
        Ok(())
    }
}
