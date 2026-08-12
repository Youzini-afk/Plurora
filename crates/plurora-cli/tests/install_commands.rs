use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser;
use plurora_cli::cli::Cli;
use plurora_cli::commands::installation::{
    self, InstallationArgs, InstallationCommand, InstallationCreateArgs, InstallationInfoArgs,
    InstallationListArgs, InstallationRemoveArgs, InstallationUpdateArgs, OutputFormat,
    StateActionArg, StateDispositionArg,
};
use plurora_runtime::{
    EventStore, FilesystemObjectStore, InstallationStateSnapshot, InstallationStateSnapshotEntry,
    InstallationView, ObjectStore, Runtime, RuntimeConfig, SqliteEventStore,
    INSTALLATION_STATE_SNAPSHOT_SCHEMA,
};
use plurora_service::{
    acquire_development_host_lease, build_workload_job_registry, development_registry,
    host_access_registry, release_development_host_lease, spawn_development_host_lease_heartbeat,
    target_agent_registry, AppState, DevelopmentHostLease, InstallationRegistry,
};
use plurora_work::{AcquisitionKind, WorkRevision};
use tempfile::tempdir;

const TEST_ROOT_TOKEN: &str = "installation-test-root-token";

#[tokio::test(flavor = "multi_thread")]
async fn installation_commands_cover_restart_cas_and_explicit_state_disposition() {
    let temporary = tempdir().unwrap();
    let cli_data_dir = temporary.path().join("cli-plurora");
    let host_data_dir = temporary.path().join("host-plurora");
    let source_one = temporary.path().join("work-one");
    let source_two = temporary.path().join("work-two");
    write_work(&source_one, "fixture/one", "Fixture One");
    write_work(&source_two, "fixture/two", "Fixture Two");
    let mut host = start_host(&host_data_dir).await.unwrap();

    let missing = rpc_call(
        &host.endpoint,
        "host.installation.create",
        serde_json::json!({
            "work_revision": missing_descriptor(plurora_work::WORK_REVISION_TYPE_URI, 'a'),
            "assembly_lock": missing_descriptor(plurora_work::ASSEMBLY_LOCK_TYPE_URI, 'b'),
            "display_name": "Missing closure",
            "source": {"kind": "work_bundle"},
            "state_bindings": [],
            "secret_policy": {},
            "idempotency_key": "missing-closure"
        }),
    )
    .await;
    assert!(missing.get("error").is_some());
    assert!(active_views(&host_data_dir).is_empty());

    let expected = b"expected object";
    let tampered = rpc_call(
        &host.endpoint,
        "object.put",
        serde_json::json!({
            "mime": "application/octet-stream",
            "content": encode_hex(b"tampered object"),
            "artifact": {
                "descriptor": {
                    "artifact_type_uri": "urn:plurora:test-object:v1",
                    "media_type": "application/octet-stream",
                    "digest": plurora_runtime::sha256_digest(expected),
                    "size_bytes": expected.len() as u64,
                    "references": [],
                    "annotations": {}
                },
                "content_encoding": "hex",
                "scope": {
                    "kind": "installation_create",
                    "work_id": "fixture/one"
                }
            }
        }),
    )
    .await;
    assert!(tampered.get("error").is_some());
    assert!(active_views(&host_data_dir).is_empty());

    run(
        &cli_data_dir,
        &host.endpoint,
        InstallationCommand::Create(InstallationCreateArgs {
            source: source_one.clone(),
            idempotency_key: "create-one".to_string(),
            display_name: None,
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap();
    // CLI retries go through the same Host owner and must not create a second Installation.
    run(
        &cli_data_dir,
        &host.endpoint,
        InstallationCommand::Create(InstallationCreateArgs {
            source: source_one.clone(),
            idempotency_key: "create-one".to_string(),
            display_name: None,
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap();

    let first = only_active_view(&host_data_dir);
    assert_eq!(first.record.display_name, "Fixture One");
    assert_eq!(first.revision, 1);
    assert_eq!(first.record.source.kind, AcquisitionKind::LocalImport);
    assert_eq!(
        first.record.source.source_ref.as_ref(),
        Some(&first.record.work_revision)
    );
    assert!(!serde_json::to_string(&first)
        .unwrap()
        .contains(source_one.to_string_lossy().as_ref()));
    assert!(host_data_dir
        .join("runtime/installations.sqlite3")
        .is_file());
    assert_ne!(cli_data_dir, host_data_dir);
    let staged_objects = object_digests(&cli_data_dir);
    let remote_objects = object_digests(&host_data_dir);
    assert_eq!(remote_objects, staged_objects);
    assert!(
        remote_objects.len() > 3,
        "the full closure, not only roots, must upload"
    );

    run(
        &cli_data_dir,
        &host.endpoint,
        InstallationCommand::List(InstallationListArgs {
            status: None,
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap();
    run(
        &cli_data_dir,
        &host.endpoint,
        InstallationCommand::Info(InstallationInfoArgs {
            installation_id: first.record.installation_id.to_string(),
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap();

    let stale = run(
        &cli_data_dir,
        &host.endpoint,
        InstallationCommand::Update(InstallationUpdateArgs {
            installation_id: first.record.installation_id.to_string(),
            source: source_two.clone(),
            expected_revision: 0,
            idempotency_key: "stale-update".to_string(),
            state_action: StateActionArg::Preserve,
            replacement_snapshot: None,
            display_name: Some("Updated Fixture".to_string()),
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap_err();
    assert!(stale.to_string().contains("runtime/error/conflict"));
    assert!(!stale
        .to_string()
        .contains(source_two.to_string_lossy().as_ref()));

    run(
        &cli_data_dir,
        &host.endpoint,
        InstallationCommand::Update(InstallationUpdateArgs {
            installation_id: first.record.installation_id.to_string(),
            source: source_two.clone(),
            expected_revision: 1,
            idempotency_key: "update-one".to_string(),
            state_action: StateActionArg::Preserve,
            replacement_snapshot: None,
            display_name: Some("Updated Fixture".to_string()),
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap();
    let updated = read_view(&host_data_dir, first.record.installation_id.as_str());
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.record.display_name, "Updated Fixture");

    let first_state = installation_root(&host_data_dir, first.record.installation_id.as_str())
        .join("state/keep.txt");
    fs::write(&first_state, "host state").unwrap();
    run(
        &cli_data_dir,
        &host.endpoint,
        InstallationCommand::Update(InstallationUpdateArgs {
            installation_id: first.record.installation_id.to_string(),
            source: source_two.clone(),
            expected_revision: 2,
            idempotency_key: "reset-state".to_string(),
            state_action: StateActionArg::Reset,
            replacement_snapshot: None,
            display_name: None,
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap();
    assert!(!first_state.exists());

    let replacement_path = temporary.path().join("replacement-state.json");
    let replacement = InstallationStateSnapshot {
        schema: INSTALLATION_STATE_SNAPSHOT_SCHEMA.to_string(),
        entries: vec![InstallationStateSnapshotEntry {
            path: "nested/restored.bin".to_string(),
            bytes: vec![0, 1, 2, 255],
        }],
    };
    fs::write(&replacement_path, replacement.canonical_bytes().unwrap()).unwrap();
    run(
        &cli_data_dir,
        &host.endpoint,
        InstallationCommand::Update(InstallationUpdateArgs {
            installation_id: first.record.installation_id.to_string(),
            source: source_two.clone(),
            expected_revision: 3,
            idempotency_key: "replace-state".to_string(),
            state_action: StateActionArg::Replace,
            replacement_snapshot: Some(replacement_path),
            display_name: None,
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap();
    let restored_state = installation_root(&host_data_dir, first.record.installation_id.as_str())
        .join("state/nested/restored.bin");
    assert_eq!(fs::read(&restored_state).unwrap(), [0, 1, 2, 255]);
    assert_ne!(cli_data_dir, host_data_dir);

    run(
        &cli_data_dir,
        &host.endpoint,
        InstallationCommand::Remove(InstallationRemoveArgs {
            installation_id: first.record.installation_id.to_string(),
            expected_revision: 4,
            state: StateDispositionArg::Keep,
            idempotency_key: "remove-keep".to_string(),
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap();
    assert_eq!(fs::read(restored_state).unwrap(), [0, 1, 2, 255]);

    run(
        &cli_data_dir,
        &host.endpoint,
        InstallationCommand::Create(InstallationCreateArgs {
            source: source_one.clone(),
            idempotency_key: "create-delete".to_string(),
            display_name: Some("Delete Fixture".to_string()),
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap();
    let delete = active_views(&host_data_dir)
        .into_iter()
        .find(|view| view.record.display_name == "Delete Fixture")
        .unwrap();
    let delete_state =
        installation_root(&host_data_dir, delete.record.installation_id.as_str()).join("state");
    fs::write(delete_state.join("delete.txt"), "host state").unwrap();
    run(
        &cli_data_dir,
        &host.endpoint,
        InstallationCommand::Remove(InstallationRemoveArgs {
            installation_id: delete.record.installation_id.to_string(),
            expected_revision: 1,
            state: StateDispositionArg::Delete,
            idempotency_key: "remove-delete".to_string(),
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap();
    assert!(fs::read_dir(delete_state).unwrap().next().is_none());
    // A local Work path is acquisition input only; neither remove mode owns it.
    assert!(source_one.join("work.yaml").is_file());
    assert!(source_two.join("work.yaml").is_file());

    // Projection files are rebuildable, but only the leased Host may hydrate them.
    let projection = installation_root(&host_data_dir, delete.record.installation_id.as_str())
        .join("installation.json");
    fs::write(&projection, "not authoritative").unwrap();
    host.stop().await.unwrap();
    host = start_host(&host_data_dir).await.unwrap();
    run(
        &cli_data_dir,
        &host.endpoint,
        InstallationCommand::Info(InstallationInfoArgs {
            installation_id: delete.record.installation_id.to_string(),
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap();
    let repaired: InstallationView =
        serde_json::from_slice(&fs::read(projection).unwrap()).unwrap();
    assert_eq!(
        repaired.record.installation_id,
        delete.record.installation_id
    );
    host.stop().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn scoped_device_cli_uploads_create_and_update_closures_over_http() {
    let temporary = tempdir().unwrap();
    let host_data_dir = temporary.path().join("host");
    let create_data_dir = temporary.path().join("create-client");
    let update_data_dir = temporary.path().join("update-client");
    let source_one = temporary.path().join("scoped-work-one");
    let source_two = temporary.path().join("scoped-work-two");
    write_work(&source_one, "fixture/scoped-one", "Scoped One");
    write_work(&source_two, "fixture/scoped-two", "Scoped Two");
    let host = start_host(&host_data_dir).await.unwrap();

    let _denied = run_with_token(
        &create_data_dir,
        &host.endpoint,
        "invalid-device-token",
        InstallationCommand::Create(InstallationCreateArgs {
            source: source_one.clone(),
            idempotency_key: "stage-scoped-create".to_string(),
            display_name: None,
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap_err();
    assert!(active_views(&host_data_dir).is_empty());
    let (create_work_id, create_revision_digest) =
        staged_work_revision(&create_data_dir, "fixture/scoped-one");
    let create_token = pair_scoped_device(
        &host.endpoint,
        serde_json::json!([
            {"kind": "work", "id": create_work_id},
            {"kind": "work", "id": create_revision_digest}
        ]),
    )
    .await
    .unwrap();
    run_with_token(
        &create_data_dir,
        &host.endpoint,
        &create_token,
        InstallationCommand::Create(InstallationCreateArgs {
            source: source_one,
            idempotency_key: "scoped-create".to_string(),
            display_name: None,
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap();
    let created = only_active_view(&host_data_dir);
    let listed = rpc_call(&host.endpoint, "object.list", serde_json::json!({})).await;
    assert_eq!(listed["result"], serde_json::json!([]));

    let replacement_path = temporary.path().join("scoped-replacement.json");
    let replacement = InstallationStateSnapshot {
        schema: INSTALLATION_STATE_SNAPSHOT_SCHEMA.to_string(),
        entries: vec![InstallationStateSnapshotEntry {
            path: "scoped/restored.bin".to_string(),
            bytes: vec![9, 8, 7, 0],
        }],
    };
    let replacement_descriptor = replacement.artifact_descriptor().unwrap();
    fs::write(&replacement_path, replacement.canonical_bytes().unwrap()).unwrap();
    let staged_update = InstallationCommand::Update(InstallationUpdateArgs {
        installation_id: created.record.installation_id.to_string(),
        source: source_two.clone(),
        expected_revision: 1,
        idempotency_key: "stage-scoped-update".to_string(),
        state_action: StateActionArg::Replace,
        replacement_snapshot: Some(replacement_path.clone()),
        display_name: Some("Scoped Updated".to_string()),
        format: OutputFormat::Json,
    });
    assert!(run_with_token(
        &update_data_dir,
        &host.endpoint,
        "invalid-device-token",
        staged_update,
    )
    .await
    .is_err());
    assert_eq!(
        read_view(&host_data_dir, created.record.installation_id.as_str()).revision,
        1
    );
    let (update_work_id, update_revision_digest) =
        staged_work_revision(&update_data_dir, "fixture/scoped-two");
    let update_token = pair_scoped_device(
        &host.endpoint,
        serde_json::json!([
            {"kind": "work", "id": update_work_id},
            {"kind": "work", "id": update_revision_digest},
            {"kind": "installation", "id": created.record.installation_id}
        ]),
    )
    .await
    .unwrap();
    run_with_token(
        &update_data_dir,
        &host.endpoint,
        &update_token,
        InstallationCommand::Update(InstallationUpdateArgs {
            installation_id: created.record.installation_id.to_string(),
            source: source_two,
            expected_revision: 1,
            idempotency_key: "scoped-update".to_string(),
            state_action: StateActionArg::Replace,
            replacement_snapshot: Some(replacement_path),
            display_name: Some("Scoped Updated".to_string()),
            format: OutputFormat::Json,
        }),
    )
    .await
    .unwrap();
    let updated = read_view(&host_data_dir, created.record.installation_id.as_str());
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.record.display_name, "Scoped Updated");
    assert_eq!(
        fs::read(
            installation_root(&host_data_dir, created.record.installation_id.as_str())
                .join("state/scoped/restored.bin")
        )
        .unwrap(),
        [9, 8, 7, 0]
    );
    let listed = rpc_call(&host.endpoint, "object.list", serde_json::json!({})).await;
    assert_eq!(listed["result"], serde_json::json!([]));
    let denied_snapshot = rpc_call(
        &host.endpoint,
        "object.get",
        serde_json::json!({
            "installation_state_artifact": replacement_descriptor
        }),
    )
    .await;
    assert_eq!(denied_snapshot["error"]["code"], "runtime/error/internal");
    assert!(!denied_snapshot.to_string().contains("scoped/restored.bin"));
    host.stop().await.unwrap();
}

#[test]
fn retired_project_and_install_aliases_fail_to_parse() {
    for command in [
        "project",
        "install",
        "uninstall",
        "update",
        "list-installed",
        "lockfile",
    ] {
        assert!(
            Cli::try_parse_from(["plurora", command]).is_err(),
            "retired command still parses: {command}"
        );
    }
}

struct TestHost {
    endpoint: String,
    store: Arc<SqliteEventStore>,
    lease: DevelopmentHostLease,
    heartbeat: tokio::task::JoinHandle<()>,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl TestHost {
    async fn stop(self) -> anyhow::Result<()> {
        self.server.abort();
        self.heartbeat.abort();
        release_development_host_lease(self.store, &self.lease).await
    }
}

async fn start_host(data_dir: &Path) -> anyhow::Result<TestHost> {
    let runtime_root = data_dir.join("runtime");
    let object_root = data_dir.join("objects");
    fs::create_dir_all(&runtime_root)?;
    fs::create_dir_all(&object_root)?;
    let store = Arc::new(SqliteEventStore::open(
        runtime_root.join("installations.sqlite3"),
    )?);
    let objects: Arc<dyn ObjectStore> = Arc::new(FilesystemObjectStore::new(object_root));
    let event_store: Arc<dyn EventStore> = store.clone();
    let installations = InstallationRegistry::persistent(event_store, objects.clone(), data_dir)?;
    let development = development_registry();
    let lease = acquire_development_host_lease(store.clone(), development.clone()).await?;
    let heartbeat = spawn_development_host_lease_heartbeat(store.clone(), lease.clone());
    lease.ensure_active()?;
    installations.hydrate().await?;
    let runtime = Arc::new(Runtime::new(
        store.clone(),
        RuntimeConfig {
            object_store: objects,
            installation_control: installations.clone(),
            ..RuntimeConfig::default()
        },
    ));
    let app = plurora_service::app_with_state(AppState {
        runtime,
        static_dir: None,
        access_token: Some(TEST_ROOT_TOKEN.to_string()),
        app_base_domain: None,
        build_jobs: build_workload_job_registry(),
        development,
        host_access: host_access_registry(),
        installations,
        target_agents: target_agent_registry(),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move { axum::serve(listener, app).await });
    Ok(TestHost {
        endpoint: format!("http://{address}"),
        store,
        lease,
        heartbeat,
        server,
    })
}

async fn run(data_dir: &Path, endpoint: &str, command: InstallationCommand) -> anyhow::Result<()> {
    run_with_token(data_dir, endpoint, TEST_ROOT_TOKEN, command).await
}

async fn run_with_token(
    data_dir: &Path,
    endpoint: &str,
    access_token: &str,
    command: InstallationCommand,
) -> anyhow::Result<()> {
    installation::run(InstallationArgs {
        command,
        data_dir: Some(data_dir.to_path_buf()),
        endpoint: Some(endpoint.to_string()),
        access_token: Some(access_token.to_string()),
    })
    .await
}

fn write_work(root: &Path, id: &str, title: &str) {
    fs::create_dir_all(root.join("packages/component")).unwrap();
    fs::create_dir_all(root.join("content/nested")).unwrap();
    fs::write(root.join("content/readme.txt"), format!("content for {id}")).unwrap();
    fs::write(root.join("content/nested/data.bin"), [0_u8, 1, 2, 0xff]).unwrap();
    fs::write(
        root.join("packages/component/manifest.yaml"),
        r#"schema_version: 1
id: fixture/component
version: 1.0.0
entry:
  kind: rust_inproc
  crate_ref: fixture-component
  symbol: register
  abi_version: 1
provides: []
consumes: []
requires: []
contributes: {}
permissions: {}
sandbox_policy: {}
"#,
    )
    .unwrap();
    fs::write(
        root.join("work.yaml"),
        format!(
            "schema: plurora.work-source.v1\nwork:\n  id: {id}\n  title: {title}\n  assembly: assembly.yaml\n  content: [content]\n"
        ),
    )
    .unwrap();
    fs::write(
        root.join("assembly.yaml"),
        format!("schema: plurora.assembly-source.v1\nassembly:\n  id: {id}/main\n  nodes:\n    - id: component\n      component: packages/component/manifest.yaml\n"),
    )
    .unwrap();
}

async fn rpc_call(endpoint: &str, method: &str, params: serde_json::Value) -> serde_json::Value {
    reqwest::Client::new()
        .post(format!("{endpoint}/rpc"))
        .bearer_auth(TEST_ROOT_TOKEN)
        .json(&serde_json::json!({"id": "test", "method": method, "params": params}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn pair_scoped_device(
    endpoint: &str,
    resources: serde_json::Value,
) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let pairing: serde_json::Value = client
        .post(format!("{endpoint}/host/v1/access/pairings"))
        .bearer_auth(TEST_ROOT_TOKEN)
        .json(&serde_json::json!({
            "device_name": "scoped installation CLI",
            "scopes": ["observe", "installation.manage"],
            "resources": resources,
            "pairing_ttl_secs": 60,
            "grant_ttl_secs": 7200
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let claim = client
        .post(format!("{endpoint}/host/v1/access/pair"))
        .json(&serde_json::json!({"pairing_token": pairing["pairing_token"]}))
        .send()
        .await?
        .error_for_status()?;
    let cookie = claim
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| anyhow::anyhow!("pairing response omitted device cookie"))?;
    Ok(cookie
        .split_once('=')
        .and_then(|(_, value)| value.split(';').next())
        .ok_or_else(|| anyhow::anyhow!("pairing response device cookie is malformed"))?
        .to_string())
}

fn staged_work_revision(data_dir: &Path, expected_work_id: &str) -> (String, String) {
    let sha_root = data_dir.join("objects/sha256");
    for entry in fs::read_dir(sha_root).unwrap().filter_map(Result::ok) {
        let Ok(bytes) = fs::read(entry.path()) else {
            continue;
        };
        let Ok(revision) = serde_json::from_slice::<WorkRevision>(&bytes) else {
            continue;
        };
        if revision.work_id.as_str() == expected_work_id {
            return (
                revision.work_id.to_string(),
                format!("sha256:{}", entry.file_name().to_string_lossy()),
            );
        }
    }
    panic!("staged WorkRevision for {expected_work_id} was not found")
}

fn missing_descriptor(artifact_type_uri: &str, marker: char) -> plurora_core::ArtifactDescriptor {
    plurora_core::ArtifactDescriptor {
        artifact_type_uri: artifact_type_uri.to_string(),
        media_type: "application/json".to_string(),
        digest: format!("sha256:{}", marker.to_string().repeat(64)),
        size_bytes: 1,
        references: Vec::new(),
        annotations: Default::default(),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn object_digests(data_dir: &Path) -> BTreeSet<String> {
    fs::read_dir(data_dir.join("objects/sha256"))
        .unwrap()
        .map(|entry| format!("sha256:{}", entry.unwrap().file_name().to_string_lossy()))
        .collect()
}

fn installation_root(data_dir: &Path, installation_id: &str) -> PathBuf {
    data_dir.join("installations").join(installation_id)
}

fn read_view(data_dir: &Path, installation_id: &str) -> InstallationView {
    serde_json::from_slice(
        &fs::read(installation_root(data_dir, installation_id).join("installation.json")).unwrap(),
    )
    .unwrap()
}

fn active_views(data_dir: &Path) -> Vec<InstallationView> {
    fs::read_dir(data_dir.join("installations"))
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let bytes = fs::read(entry.path().join("installation.json")).ok()?;
            serde_json::from_slice::<InstallationView>(&bytes).ok()
        })
        .collect()
}

fn only_active_view(data_dir: &Path) -> InstallationView {
    let mut views = active_views(data_dir);
    assert_eq!(views.len(), 1);
    views.pop().unwrap()
}
