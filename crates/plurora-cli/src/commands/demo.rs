use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use plurora_core::PLATFORM_RUNTIME_ID;
use plurora_runtime::{
    AppendEventRequest, CapabilityInvocationRequest, EventStore, InMemoryEventStore,
    OpenSessionRequest, ProtocolContext, Runtime, RuntimeConfig, SqliteEventStore,
};
use serde_json::json;

use super::manifest::read_manifest;

pub(crate) async fn demo() -> Result<()> {
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store.clone(), RuntimeConfig::default());

    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime.load_package(demo_event_writer_manifest()).await?;
    runtime
        .append_event(AppendEventRequest {
            session_id: session.id.clone(),
            writer_package_id: "example/echo".to_string(),
            kind: "example/echo/event.demo".to_string(),
            payload: json!({"message": "content-free platform event"}),
            metadata: json!({"created_by": "plurora-cli demo"}),
        })
        .await?;

    let events = store.list_session(&session.id).await?;

    println!("session_id: {}", session.id);
    println!("platform_runtime_id: {PLATFORM_RUNTIME_ID}");
    println!("\nevents:");
    for event in events {
        println!(
            "- #{} {} {}",
            event.sequence, event.writer_package_id, event.kind
        );
    }

    Ok(())
}

pub(crate) fn demo_event_writer_manifest() -> plurora_core::PackageManifest {
    use plurora_core::{
        EntryDescriptor, EventPermissions, PackageContributions, PackageEntry, PackageManifest,
        PermissionSet, SandboxPolicy,
    };

    PackageManifest {
        schema_version: 1,
        id: "example/echo".to_string(),
        version: "0.1.0".to_string(),
        display_name: Some("Demo Event Writer".to_string()),
        description: None,
        author: None,
        license: None,
        entry: EntryDescriptor::v1(PackageEntry::RustInproc {
            crate_ref: "example-echo".to_string(),
            symbol: "register".to_string(),
            abi_version: 1,
        }),
        provides: Vec::new(),
        consumes: Vec::new(),
        requires: Vec::new(),
        contributes: PackageContributions::default(),
        permissions: PermissionSet {
            events: EventPermissions {
                read: false,
                append: true,
            },
            ..PermissionSet::default()
        },
        sandbox_policy: SandboxPolicy::default(),
    }
}

pub(crate) async fn sqlite_demo(path: PathBuf) -> Result<()> {
    let store = Arc::new(SqliteEventStore::open(&path)?);
    let runtime = Runtime::new(store.clone(), RuntimeConfig::default());
    let session = runtime.open_session(OpenSessionRequest::default()).await?;
    runtime.load_package(sqlite_event_writer_manifest()).await?;
    runtime
        .append_event(AppendEventRequest {
            session_id: session.id.clone(),
            writer_package_id: "example/sqlite".to_string(),
            kind: "example/sqlite/event.demo".to_string(),
            payload: json!({"durable": true}),
            metadata: json!({}),
        })
        .await?;
    drop(runtime);
    drop(store);

    let reopened = SqliteEventStore::open(&path)?;
    let events = reopened.list_session(&session.id).await?;
    println!("sqlite_path: {}", path.display());
    println!("session_id: {}", session.id);
    for event in events {
        println!(
            "- #{} {} {}",
            event.sequence, event.writer_package_id, event.kind
        );
    }
    Ok(())
}

pub(crate) fn sqlite_event_writer_manifest() -> plurora_core::PackageManifest {
    plurora_core::PackageManifest {
        id: "example/sqlite".to_string(),
        ..demo_event_writer_manifest()
    }
}

pub(crate) async fn serve(bind: std::net::SocketAddr) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    println!("Plurora kernel service listening on http://{bind}");
    axum::serve(listener, plurora_service::app()).await?;
    Ok(())
}

#[derive(Debug)]
pub(crate) struct BlankLoopResult {
    pub(crate) session_id: String,
    pub(crate) branch_id: String,
    pub(crate) asset_id: String,
    pub(crate) projection_id: String,
}

pub(crate) async fn run_blank_play_creation_loop<S: EventStore>(
    runtime: &Runtime<S>,
) -> Result<BlankLoopResult> {
    for manifest in [
        "packages/plurora/assistant-lab/manifest.yaml",
        "packages/plurora/blank-experience/manifest.yaml",
    ] {
        runtime
            .load_package(read_manifest(PathBuf::from(manifest)).await?)
            .await?;
    }
    let session = runtime
        .open_session(OpenSessionRequest {
            labels: vec!["play-create".to_string()],
            active_package_set: vec![
                "plurora/blank-experience".to_string(),
                "plurora/assistant-lab".to_string(),
            ],
            metadata: json!({"surface": "play"}),
        })
        .await?;
    let seed = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/blank-experience/create_seed".to_string()),
            caller_package_id: None,
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({"title": "Blank Loop", "intent": "prove play-create substrate"}),
        })
        .await?;
    let assistant = json!({"kind": "assistant", "assistant_id": "assistant/blank-loop", "delegated_user_id": "user/demo"});
    runtime
        .call_protocol(
            &ProtocolContext::host_dev("demo"),
            "authority.grant.create",
            json!({"principal": assistant, "permission": "capabilities.invoke", "scope": "plurora/assistant-lab"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let assistant_context = ProtocolContext {
        principal: serde_json::from_value(assistant)?,
        transport: "demo".to_string(),
        authority: None,
        host_operation: None,
        session_id: None,
        correlation_id: None,
        parent_invocation_id: None,
    };
    let proposal = runtime
        .call_protocol(
            &assistant_context,
            "capability.invoke",
            json!({"capability_id": "plurora/assistant-lab/draft_branch_change", "input": {"seed": seed.output, "change": "try a first branch"}}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        proposal["output"]["requires_user_approval"] == json!(true),
        "assistant proposal must require approval"
    );
    let branch = runtime
        .fork_session(
            session.id.clone(),
            0,
            json!({"proposal": proposal["output"].clone()}),
        )
        .await?;
    let asset = runtime
        .put_asset(plurora_runtime::runtime::AssetPutRequest {
            origin_package_id: Some("plurora/blank-experience".to_string()),
            mime: "application/json".to_string(),
            content: serde_json::to_string(&json!({"seed": seed.output, "branch_id": branch.id}))?,
            metadata: json!({"kind": "blank_experience_seed"}),
        })
        .await?;
    let projection_id = "plurora/blank-experience/projection/demo".to_string();
    runtime
        .projection_register(plurora_runtime::runtime::ProjectionDefinition {
            id: projection_id.clone(),
            session_id: session.id.clone(),
            source_kind_prefix: Some("context/".to_string()),
            state: json!({}),
        })
        .await?;
    runtime.projection_rebuild(&projection_id).await?;
    Ok(BlankLoopResult {
        session_id: session.id,
        branch_id: branch.id,
        asset_id: asset.id,
        projection_id,
    })
}

pub(crate) async fn play_create_demo() -> Result<()> {
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());
    let result = run_blank_play_creation_loop(&runtime).await?;
    println!("blank play-creation loop ok");
    println!("session_id: {}", result.session_id);
    println!("branch_id: {}", result.branch_id);
    println!("asset_id: {}", result.asset_id);
    println!("projection_id: {}", result.projection_id);
    Ok(())
}

pub(crate) async fn playable_board_demo() -> Result<()> {
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store, RuntimeConfig::default());

    // Load required packages
    for manifest_path in [
        "packages/plurora/playable-creation-board/manifest.yaml",
        "packages/plurora/agentic-forge-lab/manifest.yaml",
        "packages/plurora/experience-runtime-lab/manifest.yaml",
    ] {
        runtime
            .load_package(read_manifest(PathBuf::from(manifest_path)).await?)
            .await?;
    }

    let pkg = "plurora/playable-creation-board";

    // 1. Launch
    let launch = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{pkg}/launch")),
            caller_package_id: None,
            provider_package_id: Some(pkg.to_string()),
            version: None,
            session_id: None,
            input: json!({"board_id": "board:demo", "title": "Demo Board", "modules": [{"id": "mod_1", "kind": "grid"}], "constraints": [{"id": "c_1", "rule": "max_markers=10"}]}),
        })
        .await?;
    anyhow::ensure!(
        launch.output["kind"] == json!("playable_creation_board_launched"),
        "launch must succeed"
    );
    println!("[1] launch: board_id={}", launch.output["board_id"]);

    // 2. Three player actions
    for seq in 1..=3 {
        let action = runtime
            .invoke_capability(CapabilityInvocationRequest {
                handle: None,
                capability_id: Some(format!("{pkg}/record_player_action")),
                caller_package_id: None,
                provider_package_id: Some(pkg.to_string()),
                version: None,
                session_id: None,
                input: json!({
                    "board_id": "board:demo",
                    "action_kind": "place_marker",
                    "sequence": seq,
                    "payload": {"marker_id": format!("m{}", seq), "position": [seq, seq]}
                }),
            })
            .await?;
        anyhow::ensure!(
            action.output["kind"] == json!("playable_creation_board_action_recorded"),
            "action {} must succeed",
            seq
        );
        println!(
            "[2.{seq}] action: action_id={}, state_delta={}",
            action.output["action_id"], action.output["state_delta_asset_ref"]
        );
    }

    // 3. project_state and render_payload
    let state = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{pkg}/project_state")),
            caller_package_id: None,
            provider_package_id: Some(pkg.to_string()),
            version: None,
            session_id: None,
            input: json!({"board_id": "board:demo", "lifecycle_state": "running"}),
        })
        .await?;
    println!("[3] project_state: kind={}", state.output["kind"]);

    let render = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{pkg}/render_payload")),
            caller_package_id: None,
            provider_package_id: Some(pkg.to_string()),
            version: None,
            session_id: None,
            input: json!({"board_id": "board:demo"}),
        })
        .await?;
    println!("[3b] render_payload: kind={}", render.output["kind"]);

    // 4. Checkpoint + recovery
    let checkpoint = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{pkg}/create_checkpoint")),
            caller_package_id: None,
            provider_package_id: Some(pkg.to_string()),
            version: None,
            session_id: None,
            input: json!({
                "board_id": "board:demo",
                "state_snapshot": {"markers": 3},
                "sequence": 1,
            }),
        })
        .await?;
    anyhow::ensure!(
        checkpoint.output["kind"] == json!("playable_creation_board_checkpoint"),
        "checkpoint must succeed"
    );
    println!("[4] checkpoint: id={}", checkpoint.output["checkpoint_id"]);

    let inspect = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{pkg}/inspect_checkpoint")),
            caller_package_id: None,
            provider_package_id: Some(pkg.to_string()),
            version: None,
            session_id: None,
            input: json!({
                "checkpoint_id": checkpoint.output["checkpoint_id"],
                "board_id": "board:demo",
                "state_snapshot": {"markers": 3},
                "format": "snapshot",
                "sequence": 1,
            }),
        })
        .await?;
    anyhow::ensure!(
        inspect.output["valid"] == json!(true),
        "checkpoint must be valid"
    );
    println!("[4b] inspect: valid={}", inspect.output["valid"]);

    let recovery = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{pkg}/draft_recovery")),
            caller_package_id: None,
            provider_package_id: Some(pkg.to_string()),
            version: None,
            session_id: None,
            input: json!({
                "board_id": "board:demo",
                "failure_kind": "constraint_violation",
                "last_checkpoint_ref": checkpoint.output["checkpoint_id"],
            }),
        })
        .await?;
    println!(
        "[4c] recovery: strategy={}",
        recovery.output["recovery_strategy"]
    );

    // 5. request_change + bind_agent_run
    let change_req = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{pkg}/request_change")),
            caller_package_id: None,
            provider_package_id: Some(pkg.to_string()),
            version: None,
            session_id: None,
            input: json!({
                "board_id": "board:demo",
                "objective": "add a new grid module with placement constraint",
                "change_kind": "add_module",
                "risk": "medium",
            }),
        })
        .await?;
    anyhow::ensure!(
        change_req.output["kind"] == json!("playable_creation_board_change_request"),
        "request_change must succeed"
    );
    println!(
        "[5] request_change: objective={}",
        change_req.output["objective"]
    );

    let binding = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{pkg}/bind_agent_run")),
            caller_package_id: None,
            provider_package_id: Some(pkg.to_string()),
            version: None,
            session_id: None,
            input: json!({
                "board_id": "board:demo",
                "agent_package_id": "plurora/agentic-forge-lab",
            }),
        })
        .await?;
    anyhow::ensure!(
        binding.output["kind"] == json!("playable_creation_board_agent_run_binding"),
        "bind_agent_run must succeed"
    );
    println!(
        "[5b] bind_agent_run: scoped={}",
        binding.output["scoped_to_branch"]
    );

    // 6. Agentic forge loop (start_run → export_plan → create_candidate → compare → draft_promote)
    let forge_pkg = "plurora/agentic-forge-lab";
    let start_run = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{forge_pkg}/start_run")),
            caller_package_id: None,
            provider_package_id: Some(forge_pkg.to_string()),
            version: None,
            session_id: None,
            input: json!({"objective": "add grid module with placement constraint"}),
        })
        .await?;
    let run_id = start_run.output["run_id"].as_str().unwrap_or("run_unknown");
    println!("[6] forge start_run: run_id={}", run_id);

    let export_plan = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{forge_pkg}/export_plan_graph")),
            caller_package_id: None,
            provider_package_id: Some(forge_pkg.to_string()),
            version: None,
            session_id: None,
            input: json!({"run_id": run_id}),
        })
        .await?;
    println!(
        "[6b] export_plan: nodes={}",
        export_plan.output["plan_graph"]["nodes"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
    );

    let create_cand = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{forge_pkg}/create_candidate")),
            caller_package_id: None,
            provider_package_id: Some(forge_pkg.to_string()),
            version: None,
            session_id: None,
            input: json!({
                "run_id": run_id,
                "target_branch_ref": "branch:target:default",
                "scratch_branch_ref": "branch:scratch:default",
                "target_revision": 1,
                "changed_asset_refs": ["asset:board_state:board:demo"],
                "diff_summary": "add grid module with placement constraint",
            }),
        })
        .await?;
    let cand_id = create_cand.output["candidate"]["candidate_id"]
        .as_str()
        .unwrap_or("cand_unknown");
    println!(
        "[6c] create_candidate: id={}, target_unchanged={}",
        cand_id, create_cand.output["target_branch_unchanged"]
    );

    let compare = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{forge_pkg}/compare_candidate")),
            caller_package_id: None,
            provider_package_id: Some(forge_pkg.to_string()),
            version: None,
            session_id: None,
            input: json!({
                "candidate_id": cand_id,
                "target_revision": 1,
                "current_target_revision": 1,
            }),
        })
        .await?;
    println!("[6d] compare: stale={}", compare.output["stale"]);

    let promote = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{forge_pkg}/draft_promote_proposal")),
            caller_package_id: None,
            provider_package_id: Some(forge_pkg.to_string()),
            version: None,
            session_id: None,
            input: json!({
                "candidate_id": cand_id,
                "run_id": run_id,
                "target_revision": 1,
                "current_target_revision": 1,
                "target_branch_ref": "branch:target:default",
                "scratch_branch_ref": "branch:scratch:default",
            }),
        })
        .await?;
    anyhow::ensure!(
        promote.output["kind"] == json!("agentic_forge_promote_proposal_draft"),
        "promote must produce proposal draft"
    );
    println!(
        "[6e] draft_promote: target_unchanged={}, direct_mutation={}",
        promote.output["target_branch_unchanged"], promote.output["direct_mutation"]
    );

    // 7. Provenance
    let provenance = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{pkg}/explain_provenance")),
            caller_package_id: None,
            provider_package_id: Some(pkg.to_string()),
            version: None,
            session_id: None,
            input: json!({
                "board_id": "board:demo",
                "action_id": "action:board:demo:1",
                "agent_run_ref": run_id,
                "candidate_ref": cand_id,
            }),
        })
        .await?;
    println!(
        "[7] provenance: chain_len={}",
        provenance.output["chain"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
    );

    println!("playable-creation-board demo ok");
    Ok(())
}
