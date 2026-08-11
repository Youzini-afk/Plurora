//! Public Run protocol conformance for the Phase 4 Installation/Run boundary.
//!
//! These vectors intentionally call the public `host.run.*` methods.  The
//! lifecycle driver below is a deterministic Host fixture, while the
//! Installation, Work, Lock, journal, and request/response types come from the
//! canonical Installation fixture.  This keeps the cases focused on the
//! public Run contract without inventing private wire DTOs or touching the
//! runtime implementation.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use plurora_core::{
    EVENT_RUN_FAILED, EVENT_RUN_STARTED, EVENT_RUN_STARTING, EVENT_RUN_STOPPED, EVENT_RUN_STOPPING,
};
use plurora_runtime::{
    EventStore, InMemoryEventStore, InstallationControl, ObjectStore, OpenSessionRequest,
    ProtocolContext, ProtocolResourceSelector, RunActivation, RunAuthorityRefresh,
    RunAuthorityValidator, RunControl, RunEntrypointPreflight, RunGap, RunLifecycleDriver,
    RunListRequest, RunMutationResult, RunPreparation, RunStartResult, RunStatusInspection,
    RunStatusRequest, RunStatusView, RunView, Runtime, RuntimeConfig,
};
use plurora_service::RunRegistry;
use plurora_work::{
    ArtifactModel, AssemblyId, AssemblyLock, AssemblyNode, AssemblyRevision, InstallationId, RunId,
    RunStatus, WorkEntrypoint, WorkEntrypointTarget, WorkRevision,
};
use serde::Serialize;
use serde_json::{json, Value};

use super::protocol_installation::{create_request, fixture, put_work_pair, InstallationFixture};

const RUN_SESSION: &str = "host_runs";

async fn put_model<T: ArtifactModel>(
    objects: &plurora_runtime::InMemoryObjectStore,
    value: &T,
) -> anyhow::Result<plurora_core::ArtifactDescriptor> {
    let bytes = value.canonical_bytes()?;
    let descriptor = value.artifact_descriptor()?;
    let info = objects.put(bytes.into()).await?;
    anyhow::ensure!(info.digest == descriptor.digest && info.size_bytes == descriptor.size_bytes);
    Ok(descriptor)
}

async fn put_run_work_pair(
    objects: &plurora_runtime::InMemoryObjectStore,
) -> anyhow::Result<(
    plurora_core::ArtifactDescriptor,
    plurora_core::ArtifactDescriptor,
)> {
    let assembly = AssemblyRevision {
        schema: AssemblyRevision::SCHEMA.to_string(),
        assembly_id: AssemblyId::parse("conformance/run-assembly")?,
        nodes: Vec::<AssemblyNode>::new(),
        bindings: Vec::new(),
        exposed_ports: Vec::new(),
        state_slots: Vec::new(),
        annotations: BTreeMap::new(),
    };
    let assembly_descriptor = put_model(objects, &assembly).await?;
    let work = WorkRevision {
        schema: WorkRevision::SCHEMA.to_string(),
        work_id: plurora_work::WorkId::parse("conformance/one")?,
        title: "Run conformance Work".to_string(),
        description: String::new(),
        assembly: assembly_descriptor.clone(),
        content_roots: Vec::new(),
        entrypoints: vec![WorkEntrypoint {
            id: "default".to_string(),
            intent_uri: "plurora.shell.default/play".to_string(),
            target: WorkEntrypointTarget::Surface {
                surface_id: "conformance/run".to_string(),
            },
            annotations: BTreeMap::new(),
        }],
        rights: None,
        transparency: None,
        operational_intent: None,
        annotations: BTreeMap::new(),
    };
    let work_descriptor = put_model(objects, &work).await?;
    let lock = AssemblyLock {
        schema: AssemblyLock::SCHEMA.to_string(),
        assembly: assembly_descriptor,
        nodes: Vec::new(),
        bindings: Vec::new(),
        protocol_profiles: Vec::new(),
        content_roots: Vec::new(),
    };
    let lock_descriptor = put_model(objects, &lock).await?;
    Ok((work_descriptor, lock_descriptor))
}

/// Deterministic driver used only to exercise the public Run protocol.  It
/// models the Host's preflight and activation boundary, including the
/// structured gaps returned before a Run journal entry is created.
struct FixtureDriver {
    activations: AtomicUsize,
    stops: AtomicUsize,
    work_revision: plurora_core::ArtifactDescriptor,
    installations: Arc<plurora_service::InstallationRegistry>,
}

impl FixtureDriver {
    fn new(
        work_revision: plurora_core::ArtifactDescriptor,
        installations: Arc<plurora_service::InstallationRegistry>,
    ) -> Arc<Self> {
        Arc::new(Self {
            activations: AtomicUsize::new(0),
            stops: AtomicUsize::new(0),
            work_revision,
            installations,
        })
    }

    async fn revision(&self, installation_id: &InstallationId) -> anyhow::Result<u64> {
        Ok(self
            .installations
            .get(installation_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("installation_not_found"))?
            .revision)
    }

    fn gap(entrypoint_id: &str) -> Option<RunGap> {
        let (reason, next_step) = match entrypoint_id {
            "missing" => (
                "artifact_missing",
                "load the exact locked local Package before starting the Run",
            ),
            "ambiguous" => (
                "binding_ambiguous",
                "leave exactly one loaded Package matching the AssemblyLock",
            ),
            "unsupported" => (
                "unsupported_backend",
                "use a Host-supported local Package entrypoint",
            ),
            "managed" | "target_unsatisfied" => (
                "target_unsatisfied",
                "apply the required Realization before starting the Run",
            ),
            "binding" => (
                "binding_unavailable",
                "persist the required Launch binding before starting the Run",
            ),
            _ => return None,
        };
        Some(RunGap::new(reason, next_step))
    }
}

#[async_trait]
impl RunLifecycleDriver for FixtureDriver {
    async fn inspect_status(
        &self,
        request: &RunStatusRequest,
    ) -> anyhow::Result<RunStatusInspection> {
        let entrypoint_id = request.entrypoint_id.clone();
        let preflight = entrypoint_id.map(|entrypoint_id| RunEntrypointPreflight {
            gaps: Self::gap(&entrypoint_id).into_iter().collect(),
            entrypoint_id,
        });
        Ok(RunStatusInspection {
            installation_revision: self.revision(&request.installation_id).await?,
            work_revision: self.work_revision.clone(),
            preflight,
        })
    }

    async fn prepare_start(
        &self,
        request: &plurora_runtime::RunStartRequest,
    ) -> anyhow::Result<RunPreparation> {
        let revision = self.revision(&request.installation_id).await?;
        if let Some(gap) = Self::gap(&request.entrypoint_id) {
            return Ok(RunPreparation::blocked(
                revision,
                request.entrypoint_id.clone(),
                vec![gap],
            ));
        }
        Ok(RunPreparation::ready(
            revision,
            request.entrypoint_id.clone(),
            Box::new(()),
        ))
    }

    async fn activate(
        &self,
        _run_id: &RunId,
        _preparation: RunPreparation,
    ) -> anyhow::Result<RunActivation> {
        let number = self.activations.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(RunActivation::new(
            Some(format!("fixture-run-context-{number}")),
            Vec::new(),
            Vec::new(),
            Box::new(()),
        ))
    }

    async fn stop(&self, _run_id: &RunId, _activation: &mut RunActivation) -> anyhow::Result<()> {
        self.stops.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct AllowRefresh;

#[async_trait]
impl RunAuthorityValidator for AllowRefresh {
    async fn validate_current(
        &self,
        _grant_id: &str,
        _installation_id: &InstallationId,
        _run_id: Option<&RunId>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

struct Harness {
    fixture: InstallationFixture,
    runtime: Arc<Runtime<InMemoryEventStore>>,
    driver: Arc<FixtureDriver>,
}

impl Harness {
    async fn new() -> anyhow::Result<Self> {
        let mut fixture = fixture().await?;
        let (work_revision, assembly_lock) = put_run_work_pair(&fixture.objects).await?;
        fixture.work_revision = work_revision;
        fixture.assembly_lock = assembly_lock;
        let runs = RunRegistry::new(fixture.store.clone());
        let runtime = Arc::new(Runtime::new(
            fixture.store.clone(),
            RuntimeConfig {
                object_store: fixture.objects.clone(),
                installation_control: fixture.registry.clone(),
                run_control: runs.clone(),
                ..RuntimeConfig::default()
            },
        ));
        // Keep one exact, loaded local Package in the canonical Host fixture.
        // The deterministic lifecycle driver still controls which preflight
        // vector is selected, while package loading and Installation closure
        // use the same public Host path as the real Assembly driver.
        runtime
            .load_package(crate::conformance::fixtures::echo_package(
                "conformance/run-package",
                "conformance/run-package/echo",
            ))
            .await?;
        let driver = FixtureDriver::new(fixture.work_revision.clone(), fixture.registry.clone());
        runs.install_driver(driver.clone())?;
        Ok(Self {
            fixture,
            runtime,
            driver,
        })
    }

    async fn call<T: Serialize>(&self, method: &str, params: T) -> anyhow::Result<Value> {
        self.runtime
            .call_protocol(
                &ProtocolContext::host_dev("run-conformance"),
                method,
                serde_json::to_value(params)?,
            )
            .await
            .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message))
    }

    async fn create(
        &self,
        key: &str,
    ) -> anyhow::Result<plurora_runtime::InstallationMutationResult> {
        Ok(serde_json::from_value(
            self.call(
                "host.installation.create",
                create_request(
                    self.fixture.work_id.clone(),
                    self.fixture.work_revision.clone(),
                    self.fixture.assembly_lock.clone(),
                    "Run conformance Installation",
                    key,
                    plurora_work::InstallationSecretPolicy::default(),
                ),
            )
            .await?,
        )?)
    }

    fn device(
        &self,
        grant: &str,
        action: &str,
        installation_id: &InstallationId,
    ) -> ProtocolContext {
        self.device_with_run(grant, action, installation_id, None)
    }

    fn device_with_run(
        &self,
        grant: &str,
        action: &str,
        installation_id: &InstallationId,
        run_id: Option<&RunId>,
    ) -> ProtocolContext {
        let mut resources = vec![ProtocolResourceSelector {
            owner: "host".to_string(),
            kind: "installation".to_string(),
            id: Some(installation_id.to_string()),
        }];
        if let Some(run_id) = run_id {
            resources.push(ProtocolResourceSelector {
                owner: "host".to_string(),
                kind: "run".to_string(),
                id: Some(run_id.to_string()),
            });
        }
        ProtocolContext::host_device(
            grant,
            vec![action.to_string()],
            resources,
            Vec::new(),
            "run-conformance-device",
        )
        .with_verified_authority_expiry(Some(Utc::now().timestamp_millis() + 60_000))
        .with_run_authority_refresh(RunAuthorityRefresh::new(Arc::new(AllowRefresh)))
    }
}

async fn run_call<T: Serialize>(
    runtime: &Runtime<InMemoryEventStore>,
    context: &ProtocolContext,
    method: &str,
    params: T,
) -> anyhow::Result<Value> {
    runtime
        .call_protocol(context, method, serde_json::to_value(params)?)
        .await
        .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.message))
}

pub(crate) async fn status_is_effect_free() -> anyhow::Result<()> {
    let harness = Harness::new().await?;
    let installation = harness.create("run-status-create").await?;
    let before = harness.fixture.store.list_all().await?.len();

    let overview: RunStatusView = serde_json::from_value(
        harness
            .call(
                "host.run.status",
                RunStatusRequest {
                    installation_id: installation.installation.record.installation_id.clone(),
                    entrypoint_id: None,
                },
            )
            .await?,
    )?;
    anyhow::ensure!(overview.installation_id == installation.installation.record.installation_id);
    anyhow::ensure!(overview.installation_revision == installation.installation.revision);
    anyhow::ensure!(overview.work_revision == harness.fixture.work_revision);
    anyhow::ensure!(overview.active_run.is_none());
    anyhow::ensure!(overview.preflight.is_none());

    let preflight: RunStatusView = serde_json::from_value(
        harness
            .call(
                "host.run.status",
                RunStatusRequest {
                    installation_id: installation.installation.record.installation_id.clone(),
                    entrypoint_id: Some("default".to_string()),
                },
            )
            .await?,
    )?;
    anyhow::ensure!(preflight.installation_revision == installation.installation.revision);
    anyhow::ensure!(preflight.work_revision == harness.fixture.work_revision);
    anyhow::ensure!(preflight
        .preflight
        .is_some_and(|value| value.gaps.is_empty()));
    anyhow::ensure!(harness
        .fixture
        .store
        .list_session(&RUN_SESSION.to_string())
        .await?
        .is_empty());
    anyhow::ensure!(harness.fixture.store.list_all().await?.len() == before);
    anyhow::ensure!(harness.driver.activations.load(Ordering::SeqCst) == 0);
    anyhow::ensure!(harness.driver.stops.load(Ordering::SeqCst) == 0);
    Ok(())
}

pub(crate) async fn preflight_gaps_do_not_create_run() -> anyhow::Result<()> {
    let harness = Harness::new().await?;
    let installation = harness.create("run-gap-create").await?;
    let installation_id = installation.installation.record.installation_id.clone();
    let expected_revision = installation.installation.revision;

    for (entrypoint_id, reason) in [
        ("missing", "artifact_missing"),
        ("ambiguous", "binding_ambiguous"),
        ("unsupported", "unsupported_backend"),
        ("managed", "target_unsatisfied"),
    ] {
        let status: RunStatusView = serde_json::from_value(
            harness
                .call(
                    "host.run.status",
                    RunStatusRequest {
                        installation_id: installation_id.clone(),
                        entrypoint_id: Some(entrypoint_id.to_string()),
                    },
                )
                .await?,
        )?;
        anyhow::ensure!(status
            .preflight
            .as_ref()
            .and_then(|value| value.gaps.first())
            .is_some_and(|gap| gap.reason_code == reason));

        let result: RunStartResult = serde_json::from_value(
            harness
                .call(
                    "host.run.start",
                    json!({
                        "installation_id": installation_id.clone(),
                        "expected_installation_revision": expected_revision,
                        "entrypoint_id": entrypoint_id,
                        "idempotency_key": format!("gap-{entrypoint_id}"),
                    }),
                )
                .await?,
        )?;
        anyhow::ensure!(result.run.is_none());
        anyhow::ensure!(result.gaps.len() == 1 && result.gaps[0].reason_code == reason);
    }
    anyhow::ensure!(harness
        .fixture
        .store
        .list_session(&RUN_SESSION.to_string())
        .await?
        .is_empty());
    anyhow::ensure!(harness.driver.activations.load(Ordering::SeqCst) == 0);
    anyhow::ensure!(harness.driver.stops.load(Ordering::SeqCst) == 0);
    let events = harness.fixture.store.list_all().await?;
    anyhow::ensure!(!events.iter().any(|event| {
        event.kind.contains("deploy")
            || event.kind.contains("target")
            || event.session_id == RUN_SESSION
    }));
    Ok(())
}

pub(crate) async fn lifecycle_events_cas_and_replay() -> anyhow::Result<()> {
    let harness = Harness::new().await?;
    let installation = harness.create("run-lifecycle-create").await?;
    let installation_id = installation.installation.record.installation_id.clone();

    let started: RunStartResult = serde_json::from_value(
        harness
            .call(
                "host.run.start",
                json!({
                    "installation_id": installation_id.clone(),
                    "expected_installation_revision": installation.installation.revision,
                    "entrypoint_id": "default",
                    "idempotency_key": "run-start",
                }),
            )
            .await?,
    )?;
    let run = started.run.clone().expect("Run is started");
    anyhow::ensure!(run.record.status == RunStatus::Running && run.revision == 2);

    let replay: RunStartResult = serde_json::from_value(
        harness
            .call(
                "host.run.start",
                json!({
                    "installation_id": run.record.installation_id.clone(),
                    "expected_installation_revision": run.installation_revision,
                    "entrypoint_id": "default",
                    "idempotency_key": "run-start",
                }),
            )
            .await?,
    )?;
    anyhow::ensure!(replay.idempotent && replay.run.as_ref().is_some_and(|value| value == &run));

    let second = harness
        .call(
            "host.run.start",
            json!({
                "installation_id": run.record.installation_id.clone(),
                "expected_installation_revision": run.installation_revision,
                "entrypoint_id": "default",
                "idempotency_key": "run-start-second",
            }),
        )
        .await
        .expect_err("an Installation cannot have two active Runs");
    anyhow::ensure!(second.to_string().contains("active_run_exists"));

    let stale = harness
        .call(
            "host.run.stop",
            json!({
                "installation_id": run.record.installation_id.clone(),
                "run_id": run.record.run_id.clone(),
                "expected_revision": 1,
                "idempotency_key": "run-stop-stale",
            }),
        )
        .await
        .expect_err("stale Run CAS must be rejected");
    anyhow::ensure!(stale.to_string().contains("run_revision_conflict"));

    // Closing an unrelated context is not a public Run takeover or stop.
    let context = harness
        .runtime
        .open_session(OpenSessionRequest::default())
        .await?;
    harness.runtime.close_session(context.id).await?;
    let still_running: RunStatusView = serde_json::from_value(
        harness
            .call(
                "host.run.status",
                RunStatusRequest {
                    installation_id: run.record.installation_id.clone(),
                    entrypoint_id: None,
                },
            )
            .await?,
    )?;
    anyhow::ensure!(still_running
        .active_run
        .is_some_and(|value| value.record.status == RunStatus::Running));

    let stopped: RunMutationResult = serde_json::from_value(
        harness
            .call(
                "host.run.stop",
                json!({
                    "installation_id": run.record.installation_id.clone(),
                    "run_id": run.record.run_id.clone(),
                    "expected_revision": 2,
                    "idempotency_key": "run-stop",
                }),
            )
            .await?,
    )?;
    anyhow::ensure!(stopped.run.record.status == RunStatus::Stopped && stopped.run.revision == 4);
    anyhow::ensure!(harness.driver.stops.load(Ordering::SeqCst) == 1);

    let stop_replay: RunMutationResult = serde_json::from_value(
        harness
            .call(
                "host.run.stop",
                json!({
                    "installation_id": run.record.installation_id.clone(),
                    "run_id": run.record.run_id.clone(),
                    "expected_revision": 2,
                    "idempotency_key": "run-stop",
                }),
            )
            .await?,
    )?;
    anyhow::ensure!(stop_replay.idempotent && harness.driver.stops.load(Ordering::SeqCst) == 1);

    let conflict = harness
        .call(
            "host.run.stop",
            json!({
                "installation_id": run.record.installation_id.clone(),
                "run_id": run.record.run_id.clone(),
                "expected_revision": 3,
                "idempotency_key": "run-stop",
            }),
        )
        .await
        .expect_err("same idempotency key with another fingerprint must conflict");
    anyhow::ensure!(conflict.to_string().contains("idempotency_conflict"));

    let events = harness
        .fixture
        .store
        .list_session(&RUN_SESSION.to_string())
        .await?;
    for kind in [
        EVENT_RUN_STARTING,
        EVENT_RUN_STARTED,
        EVENT_RUN_STOPPING,
        EVENT_RUN_STOPPED,
    ] {
        anyhow::ensure!(events.iter().filter(|event| event.kind == kind).count() == 1);
    }
    Ok(())
}

pub(crate) async fn restart_marks_incomplete_run_interrupted() -> anyhow::Result<()> {
    let harness = Harness::new().await?;
    let installation = harness.create("run-restart-create").await?;
    let started: RunStartResult = serde_json::from_value(
        harness
            .call(
                "host.run.start",
                json!({
                    "installation_id": installation.installation.record.installation_id,
                    "expected_installation_revision": installation.installation.revision,
                    "entrypoint_id": "default",
                    "idempotency_key": "restart-start",
                }),
            )
            .await?,
    )?;
    let run = started.run.expect("Run is started");

    let recovered = RunRegistry::new(harness.fixture.store.clone());
    let recovery_driver = FixtureDriver::new(
        harness.fixture.work_revision.clone(),
        harness.fixture.registry.clone(),
    );
    recovered.install_driver(recovery_driver.clone())?;
    recovered.hydrate().await?;
    let restored = recovered
        .get(plurora_runtime::RunGetRequest {
            installation_id: run.record.installation_id.clone(),
            run_id: run.record.run_id.clone(),
        })
        .await?
        .expect("durable Run survives restart");
    anyhow::ensure!(restored.record.status == RunStatus::Interrupted);
    anyhow::ensure!(restored.record.health.reason_code.as_deref() == Some("host_restart"));
    anyhow::ensure!(recovery_driver.activations.load(Ordering::SeqCst) == 0);
    anyhow::ensure!(recovery_driver.stops.load(Ordering::SeqCst) == 0);
    anyhow::ensure!(recovered
        .list(RunListRequest {
            installation_id: Some(run.record.installation_id.clone()),
            status: Some(RunStatus::Running),
        })
        .await?
        .is_empty());
    anyhow::ensure!(
        harness
            .fixture
            .store
            .list_session(&RUN_SESSION.to_string())
            .await?
            .iter()
            .filter(|event| event.kind == EVENT_RUN_FAILED)
            .count()
            == 1
    );
    Ok(())
}

pub(crate) async fn exact_installation_and_run_authority() -> anyhow::Result<()> {
    let harness = Harness::new().await?;
    let first = harness.create("run-authority-first").await?;
    let second = harness.create("run-authority-second").await?;
    let first_id = first.installation.record.installation_id.clone();
    let second_id = second.installation.record.installation_id.clone();

    let first_context = harness.device("grant-first", "run", &first_id);
    let started: RunStartResult = serde_json::from_value(
        run_call(
            &harness.runtime,
            &first_context,
            "host.run.start",
            json!({
                "installation_id": first_id.clone(),
                "expected_installation_revision": first.installation.revision,
                "entrypoint_id": "default",
                "idempotency_key": "authority-start",
            }),
        )
        .await?,
    )?;
    let run = started
        .run
        .expect("parent Installation can create Host-generated RunId");

    let observed = harness.device("grant-first-observe", "observe", &first_id);
    let listed: Vec<RunView> = serde_json::from_value(
        run_call(
            &harness.runtime,
            &observed,
            "host.run.list",
            RunListRequest {
                installation_id: Some(first_id.clone()),
                status: None,
            },
        )
        .await?,
    )?;
    anyhow::ensure!(listed.len() == 1 && listed[0].record.run_id == run.record.run_id);
    let parent_only = run_call(
        &harness.runtime,
        &observed,
        "host.run.get",
        json!({"installation_id": first_id.clone(), "run_id": run.record.run_id.clone()}),
    )
    .await
    .expect_err("Run get requires exact Installation and Run authority");
    anyhow::ensure!(parent_only.to_string().contains("permission_denied"));

    let observed_run = harness.device_with_run(
        "grant-first-observe-run",
        "observe",
        &first_id,
        Some(&run.record.run_id),
    );
    let got: RunView = serde_json::from_value(
        run_call(
            &harness.runtime,
            &observed_run,
            "host.run.get",
            json!({"installation_id": first_id.clone(), "run_id": run.record.run_id.clone()}),
        )
        .await?,
    )?;
    anyhow::ensure!(got == run);

    let other_installation = harness.device_with_run(
        "grant-second",
        "observe",
        &second_id,
        Some(&run.record.run_id),
    );
    let denied = run_call(
        &harness.runtime,
        &other_installation,
        "host.run.get",
        json!({"installation_id": second_id.clone(), "run_id": run.record.run_id.clone()}),
    )
    .await
    .expect_err("another Installation cannot read this Run");
    anyhow::ensure!(denied.to_string().contains("Run not found"));

    let missing_run_id = RunId::new();
    let missing_run_context = harness.device_with_run(
        "grant-first-missing-run",
        "observe",
        &first_id,
        Some(&missing_run_id),
    );
    let missing_run = run_call(
        &harness.runtime,
        &missing_run_context,
        "host.run.get",
        json!({"installation_id": first_id.clone(), "run_id": missing_run_id}),
    )
    .await
    .expect_err("a sibling/unknown Run is never guessed or adopted");
    anyhow::ensure!(missing_run.to_string().contains("Run not found"));

    let parent_stop = harness.device("grant-first-stop-parent", "run", &first_id);
    let denied_stop = run_call(
        &harness.runtime,
        &parent_stop,
        "host.run.stop",
        json!({
            "installation_id": first_id.clone(),
            "run_id": run.record.run_id.clone(),
            "expected_revision": run.revision,
            "idempotency_key": "authority-stop-parent",
        }),
    )
    .await
    .expect_err("Run stop requires exact Installation and Run authority");
    anyhow::ensure!(denied_stop.to_string().contains("permission_denied"));

    let stop_context = harness.device_with_run(
        "grant-first-stop",
        "run",
        &first_id,
        Some(&run.record.run_id),
    );
    let stopped: RunMutationResult = serde_json::from_value(
        run_call(
            &harness.runtime,
            &stop_context,
            "host.run.stop",
            json!({
                "installation_id": first_id.clone(),
                "run_id": run.record.run_id.clone(),
                "expected_revision": run.revision,
                "idempotency_key": "authority-stop",
            }),
        )
        .await?,
    )?;
    anyhow::ensure!(stopped.run.record.status == RunStatus::Stopped);
    Ok(())
}

pub(crate) async fn stale_installation_revision_rejects_start() -> anyhow::Result<()> {
    let harness = Harness::new().await?;
    let installation = harness.create("run-stale-create").await?;
    let old_revision = installation.installation.revision;
    let status: RunStatusView = serde_json::from_value(
        harness
            .call(
                "host.run.status",
                RunStatusRequest {
                    installation_id: installation.installation.record.installation_id.clone(),
                    entrypoint_id: Some("default".to_string()),
                },
            )
            .await?,
    )?;
    anyhow::ensure!(status.installation_revision == old_revision);

    let (work_revision, assembly_lock) =
        put_work_pair(&harness.fixture.objects, "run-stale-updated").await?;
    let updated: plurora_runtime::InstallationMutationResult = serde_json::from_value(
        harness
            .call(
                "host.installation.update",
                plurora_runtime::InstallationUpdateRequest {
                    installation_id: installation.installation.record.installation_id.clone(),
                    expected_revision: old_revision,
                    work_revision,
                    assembly_lock,
                    display_name: None,
                    source: None,
                    state_bindings: None,
                    secret_policy: None,
                    state_action: plurora_runtime::InstallationStateAction::Preserve,
                    idempotency_key: "run-stale-update".to_string(),
                    authority: None,
                },
            )
            .await?,
    )?;
    anyhow::ensure!(updated.installation.revision > old_revision);
    let error = harness
        .call(
            "host.run.start",
            json!({
                "installation_id": installation.installation.record.installation_id,
                "expected_installation_revision": old_revision,
                "entrypoint_id": "default",
                "idempotency_key": "run-stale-start",
            }),
        )
        .await
        .expect_err("a status revision is stale after Installation update");
    anyhow::ensure!(error.to_string().contains("installation_revision_conflict"));
    anyhow::ensure!(harness
        .fixture
        .store
        .list_session(&RUN_SESSION.to_string())
        .await?
        .is_empty());
    anyhow::ensure!(harness.driver.activations.load(Ordering::SeqCst) == 0);
    Ok(())
}

pub(crate) fn run_cases() -> Vec<super::runner::ConformanceCase> {
    macro_rules! case {
        ($id:expr, [$($tag:expr),*], $func:path) => {
            super::registry::case($id, &[$($tag),*], || Box::pin($func()))
        };
    }
    vec![
        case!(
            "run.status_effect_free",
            ["protocol", "run", "installation", "no_effect"],
            status_is_effect_free
        ),
        case!(
            "run.preflight_gaps_no_run_or_deploy",
            [
                "protocol",
                "run",
                "package",
                "realization",
                "no_effect",
                "negative"
            ],
            preflight_gaps_do_not_create_run
        ),
        case!(
            "run.lifecycle_events_cas_idempotency",
            ["protocol", "run", "event", "idempotency", "concurrency"],
            lifecycle_events_cas_and_replay
        ),
        case!(
            "run.restart_interrupts_without_reactivate",
            ["protocol", "run", "restart", "journal", "rehydrate"],
            restart_marks_incomplete_run_interrupted
        ),
        case!(
            "run.exact_installation_and_run_authority",
            ["protocol", "run", "permission", "authority", "installation"],
            exact_installation_and_run_authority
        ),
        case!(
            "run.stale_installation_revision_rejected",
            ["protocol", "run", "installation", "concurrency", "cas"],
            stale_installation_revision_rejects_start
        ),
    ]
}
