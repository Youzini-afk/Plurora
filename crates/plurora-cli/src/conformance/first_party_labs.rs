use std::path::PathBuf;

use plurora_runtime::{CapabilityInvocationRequest, EventStore, ProtocolContext, RuntimeConfig};
use serde_json::json;

use super::fixtures::*;
use crate::commands::manifest;

pub(crate) async fn assistant_lab_proposal() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/assistant-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let assistant = json!({"kind": "assistant", "assistant_id": "assistant/lab", "delegated_user_id": "user/conformance"});
    let assistant_context = ProtocolContext {
        principal: serde_json::from_value(assistant.clone())?,
        transport: "conformance".to_string(),
        authority: None,
        host_operation: None,
        session_id: None,
        correlation_id: None,
        parent_invocation_id: None,
    };
    let denied = runtime
        .call_protocol(
            &assistant_context,
            "capability.invoke",
            json!({"capability_id": "plurora/assistant-lab/draft_branch_change", "input": {"change": "try branch"}}),
        )
        .await;
    anyhow::ensure!(
        denied.is_err(),
        "assistant package invocation should require grant"
    );
    runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "authority.grant.create",
            json!({"principal": assistant, "permission": "capabilities.invoke", "scope": "plurora/assistant-lab"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let proposal = runtime
        .call_protocol(
            &assistant_context,
            "capability.invoke",
            json!({"capability_id": "plurora/assistant-lab/draft_branch_change", "input": {"change": "try branch"}}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        proposal["output"]["requires_user_approval"] == json!(true),
        "assistant did not return an approval-gated proposal"
    );
    let surfaces = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.list",
            json!({"slot": "assistant_action"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        surfaces.as_array().map(|items| items.len()).unwrap_or(0) == 1,
        "assistant surface contribution missing"
    );
    Ok(())
}

pub(crate) async fn composition_lab() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/composition-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let plan = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/composition-lab/launch_plan".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/composition-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "id": "example/composed-experience",
                "entry_surface_id": "example/composed-experience/entry",
                "packages": ["example/composed-experience"],
            }),
        })
        .await?;
    anyhow::ensure!(
        plan.output["kind"] == json!("composition_launch_plan"),
        "composition lab launch_plan returned wrong kind"
    );
    let graph = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/composition-lab/surface_graph".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/composition-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"entry_surface_id": "example/composed-experience/entry", "surfaces": [{"slot": "experience_entry"}]}),
        })
        .await?;
    anyhow::ensure!(
        graph.output["kind"] == json!("composition_surface_graph"),
        "composition lab surface_graph returned wrong kind"
    );
    Ok(())
}

pub(crate) async fn docker_runtime_lab_contract_and_plan() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/docker-runtime-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let input = json!({
        "image": "nginx:alpine",
        "container_port": 80,
        "host_port": 39180,
        "route_id": "conformance-docker-route",
        "port_lease_id": "lease-conformance-docker-route"
    });
    let plan = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/docker-runtime-lab/plan_container".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/docker-runtime-lab".to_string()),
            version: None,
            session_id: None,
            input,
        })
        .await?;
    anyhow::ensure!(plan.output["valid"] == json!(true));
    anyhow::ensure!(plan.output["docker_performed"] == json!(false));
    anyhow::ensure!(plan.output["bind_host"] == json!("127.0.0.1"));
    anyhow::ensure!(plan.output["requires_host_port_lease"] == json!(true));
    anyhow::ensure!(plan.output["requires_proxy_route"] == json!(true));
    Ok(())
}

pub(crate) async fn docker_runtime_lab_blocks_dangerous_spec() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/docker-runtime-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let validation = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/docker-runtime-lab/validate_spec".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/docker-runtime-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "image": "nginx:alpine",
                "container_port": 80,
                "host_port": 39180,
                "route_id": "bad-route",
                "port_lease_id": "lease-bad-route",
                "privileged": true,
                "network_mode": "host",
                "binds": ["/:/host"],
                "env": {"RAW_SECRET": "sk-dangerous"}
            }),
        })
        .await?;
    anyhow::ensure!(validation.output["valid"] == json!(false));
    let codes = validation.output["diagnostics"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("diagnostics missing"))?
        .iter()
        .filter_map(|item| item["code"].as_str())
        .collect::<Vec<_>>();
    anyhow::ensure!(codes.contains(&"privileged_blocked"));
    anyhow::ensure!(codes.contains(&"network_mode_blocked"));
    anyhow::ensure!(codes.contains(&"mounts_blocked"));
    anyhow::ensure!(codes.contains(&"env_or_secret_blocked"));
    Ok(())
}

pub(crate) async fn docker_runtime_lab_build_image_blocks_secret_and_non_dockerfile(
) -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/docker-runtime-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    for input in [
        json!({
            "approved": true,
            "project_id": "build-test__abc123",
            "build_id": "build-001",
            "context_dir": "/tmp/not-used",
            "strategy": "compose"
        }),
        json!({
            "approved": true,
            "project_id": "build-test__abc123",
            "build_id": "build-001",
            "context_dir": "/tmp/not-used",
            "build_args": {"API_KEY": "secret_ref:env:API_KEY"}
        }),
    ] {
        let result = runtime
            .invoke_capability(CapabilityInvocationRequest {
                handle: None,
                capability_id: Some("plurora/docker-runtime-lab/build_image".to_string()),
                caller_package_id: None,
                provider_package_id: Some("plurora/docker-runtime-lab".to_string()),
                version: None,
                session_id: None,
                input,
            })
            .await?;
        anyhow::ensure!(result.output["kind"] == json!("docker_runtime_lab_rejected"));
        anyhow::ensure!(result.output["docker_performed"] == json!(false));
    }

    let tmp = tempfile::tempdir()?;
    let data_dir = tmp.path().join("data");
    let workspace = data_dir
        .join("projects")
        .join("build-test__abc123")
        .join("workspace");
    std::fs::create_dir_all(&workspace)?;
    let previous_data_dir = std::env::var("PLURORA_DATA_DIR").ok();
    std::env::set_var("PLURORA_DATA_DIR", &data_dir);
    let result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/docker-runtime-lab/build_image".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/docker-runtime-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "approved": true,
                "strategy": "nixpacks",
                "project_id": "build-test__abc123",
                "build_id": "build-001",
                "context_dir": workspace.to_string_lossy(),
                "nixpacks_binary": "definitely-not-plurora-nixpacks"
            }),
        })
        .await?;
    match previous_data_dir {
        Some(value) => std::env::set_var("PLURORA_DATA_DIR", value),
        None => std::env::remove_var("PLURORA_DATA_DIR"),
    }
    anyhow::ensure!(result.output["kind"] == json!("docker_runtime_lab_rejected"));
    anyhow::ensure!(result.output["docker_performed"] == json!(false));
    Ok(())
}

pub(crate) async fn deployment_hub_local_exec_default_deny_all() -> anyhow::Result<()> {
    let store = std::sync::Arc::new(plurora_runtime::InMemoryEventStore::default());
    let runtime = plurora_runtime::Runtime::new(store.clone(), RuntimeConfig::default());
    let response = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "host.exec.start",
            json!({
                "target_id": "local",
                "command": {"program": "echo", "args": ["should-not-run"]},
                "cwd": "/tmp"
            }),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(response["status"]["kind"] == json!("denied"));
    anyhow::ensure!(response["exec_id"].is_null());
    let events = store.list_all().await?;
    let denied = events
        .iter()
        .find(|event| event.kind == plurora_core::EVENT_EXEC_DENIED)
        .ok_or_else(|| anyhow::anyhow!("exec denied event missing"))?;
    let receipt_digest = denied.payload["receipt"]["digest"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("exec denied receipt missing"))?;
    let replay = runtime.replay_effect_receipt(receipt_digest).await?;
    anyhow::ensure!(
        replay.receipt.effect_kind == "exec.start"
            && replay.receipt.status == plurora_core::EffectTerminalStatus::Denied,
        "exec denied receipt is incomplete"
    );
    Ok(())
}

/// Test composition-lab diagnostics output with v2 fields (capabilities, permissions, replacements, compatibility).
pub(crate) async fn composition_lab_diagnostics() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/composition-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // launch_plan with v2 fields
    let plan = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/composition-lab/launch_plan".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/composition-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "id": "example/diag-comp",
                "entry_surface_id": "example/diag-comp/entry",
                "packages": ["example/diag-comp"],
                "required_capabilities": ["example/diag-comp/echo"],
                "optional_packages": ["example/optional-extra"],
                "permission_expectations": ["capabilities.invoke"],
                "replacement_candidates": ["example/diag-comp-alt"],
                "compatibility_notes": ["Requires kernel v0.1.0"],
                "surfaces": [{"slot": "experience_entry"}],
            }),
        })
        .await?;
    anyhow::ensure!(
        plan.output["kind"] == json!("composition_launch_plan"),
        "composition lab launch_plan v2 returned wrong kind"
    );
    anyhow::ensure!(
        plan.output.get("required_capabilities").is_some(),
        "launch_plan missing required_capabilities"
    );
    anyhow::ensure!(
        plan.output.get("permission_expectations").is_some(),
        "launch_plan missing permission_expectations"
    );
    anyhow::ensure!(
        plan.output.get("replacement_candidates").is_some(),
        "launch_plan missing replacement_candidates"
    );
    anyhow::ensure!(
        plan.output.get("compatibility_notes").is_some(),
        "launch_plan missing compatibility_notes"
    );

    // surface_graph with v2 fields
    let graph = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/composition-lab/surface_graph".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/composition-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "entry_surface_id": "example/diag-comp/entry",
                "surfaces": [{"slot": "experience_entry"}],
                "required_capabilities": ["example/diag-comp/echo"],
                "permission_expectations": ["capabilities.invoke"],
                "replacement_candidates": ["example/diag-comp-alt"],
                "compatibility_notes": ["Requires kernel v0.1.0"],
            }),
        })
        .await?;
    anyhow::ensure!(
        graph.output["kind"] == json!("composition_surface_graph"),
        "composition lab surface_graph v2 returned wrong kind"
    );
    anyhow::ensure!(
        graph.output.get("required_capabilities").is_some(),
        "surface_graph missing required_capabilities"
    );
    anyhow::ensure!(
        graph.output.get("permission_expectations").is_some(),
        "surface_graph missing permission_expectations"
    );
    anyhow::ensure!(
        graph.output.get("replacement_candidates").is_some(),
        "surface_graph missing replacement_candidates"
    );
    anyhow::ensure!(
        graph.output.get("compatibility_notes").is_some(),
        "surface_graph missing compatibility_notes"
    );

    // compat_report capability
    let report = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/composition-lab/compat_report".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/composition-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "id": "example/diag-comp",
                "required_capabilities": ["example/diag-comp/echo", "example/missing/cap"],
                "available_capabilities": ["example/diag-comp/echo"],
                "permission_expectations": ["capabilities.invoke"],
                "replacement_candidates": ["example/diag-comp-alt"],
                "compatibility_notes": ["Requires kernel v0.1.0"],
                "surfaces": [{"slot": "experience_entry"}],
            }),
        })
        .await?;
    anyhow::ensure!(
        report.output["kind"] == json!("composition_compat_report"),
        "composition lab compat_report returned wrong kind"
    );
    anyhow::ensure!(
        report.output.get("missing_required_capabilities").is_some(),
        "compat_report missing missing_required_capabilities"
    );
    let missing = report.output["missing_required_capabilities"]
        .as_array()
        .unwrap();
    anyhow::ensure!(
        missing.len() == 1,
        "compat_report should report exactly 1 missing capability"
    );
    anyhow::ensure!(
        missing[0] == json!("example/missing/cap"),
        "compat_report wrong missing capability"
    );

    Ok(())
}

pub(crate) async fn asset_lab() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from("packages/plurora/asset-lab/manifest.yaml"))
                .await?,
        )
        .await?;
    let preview = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/asset-lab/preview".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/asset-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"asset_id": "asset/demo", "mime": "application/json", "content": "{\"hello\":\"world\"}"}),
        })
        .await?;
    anyhow::ensure!(
        preview.output["kind"] == json!("asset_preview"),
        "asset lab preview returned wrong kind"
    );
    let import_plan = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/asset-lab/import_plan".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/asset-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"mime": "application/json", "metadata": {"source": "conformance"}}),
        })
        .await?;
    anyhow::ensure!(
        import_plan.output["requires_user_approval"] == json!(true),
        "asset import plan must require approval"
    );
    Ok(())
}

pub(crate) async fn projection_lab() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/projection-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let plan = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/projection-lab/rebuild_plan".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/projection-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"projection_id": "example/projection/state", "source_kind_prefix": "example/projection"}),
        })
        .await?;
    anyhow::ensure!(
        plan.output["kind"] == json!("projection_rebuild_plan"),
        "projection lab rebuild_plan returned wrong kind"
    );
    let source = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/projection-lab/explain_source_events".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/projection-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"projection_id": "example/projection/state", "events": [{"sequence": 1}], "source_kind_prefix": "example/projection"}),
        })
        .await?;
    anyhow::ensure!(
        source.output["event_count"] == json!(1),
        "projection lab source event count mismatch"
    );
    Ok(())
}

pub(crate) async fn playable_seed() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/playable-seed/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let launch = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/playable-seed/launch".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/playable-seed".to_string()),
            version: None,
            session_id: None,
            input: json!({"title": "Conformance Seed"}),
        })
        .await?;
    anyhow::ensure!(
        launch.output["kind"] == json!("playable_seed_launch"),
        "playable seed launch returned wrong kind"
    );
    let render = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/playable-seed/render_payload".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/playable-seed".to_string()),
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await?;
    anyhow::ensure!(
        render.output["kind"] == json!("playable_seed_render_payload"),
        "playable seed render returned wrong kind"
    );
    let proposal = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/playable-seed/propose_change".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/playable-seed".to_string()),
            version: None,
            session_id: None,
            input: json!({"change": "add one seed block"}),
        })
        .await?;
    anyhow::ensure!(
        proposal.output["requires_user_approval"] == json!(true),
        "playable seed change must require approval"
    );
    let surfaces = runtime
        .list_surface_contributions(Some("experience_entry".to_string()))
        .await;
    let has_entry = surfaces
        .as_array()
        .map(|records| {
            records
                .iter()
                .any(|record| record["package_id"] == json!("plurora/playable-seed"))
        })
        .unwrap_or(false);
    anyhow::ensure!(has_entry, "playable seed entry surface missing");
    Ok(())
}

pub(crate) async fn persona_lab() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from("packages/plurora/persona-lab/manifest.yaml"))
                .await?,
        )
        .await?;
    let imported = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/persona-lab/import_profile".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/persona-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"source": "conformance", "data": {"spec": "chara_card_v2", "data": {"name": "Mira", "description": "Maps dream cities", "extensions": {"unknown": true}}}}),
        })
        .await?;
    anyhow::ensure!(
        imported.output["kind"] == json!("persona_profile"),
        "persona import returned wrong kind"
    );
    anyhow::ensure!(
        imported.output["core"]["name"] == json!("Mira"),
        "persona import lost name"
    );
    let fragment = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/persona-lab/render_fragment".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/persona-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"profile": imported.output}),
        })
        .await?;
    anyhow::ensure!(
        fragment.output["kind"] == json!("persona_fragment"),
        "persona render returned wrong kind"
    );
    anyhow::ensure!(
        fragment.output.get("provenance").is_some(),
        "persona render missing provenance"
    );
    Ok(())
}

pub(crate) async fn knowledge_lab() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/knowledge-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let imported = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/knowledge-lab/import_collection".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/knowledge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"format": "worldbook-like", "data": {"name": "Dream City", "entries": {"1": {"key": ["bell"], "content": "Alleys rotate."}}}}),
        })
        .await?;
    anyhow::ensure!(
        imported.output["kind"] == json!("knowledge_collection"),
        "knowledge import returned wrong kind"
    );
    let matched = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/knowledge-lab/match_entries".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/knowledge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"query": "the bell rings", "entries": imported.output["entries"]}),
        })
        .await?;
    anyhow::ensure!(
        matched.output["kind"] == json!("knowledge_match_result"),
        "knowledge match returned wrong kind"
    );
    anyhow::ensure!(
        matched.output["matches"]
            .as_array()
            .map(|m| !m.is_empty())
            .unwrap_or(false),
        "knowledge match missed keyword"
    );
    let plan = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/knowledge-lab/injection_plan".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/knowledge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"matches": matched.output["matches"]}),
        })
        .await?;
    anyhow::ensure!(
        plan.output["kind"] == json!("knowledge_injection_plan"),
        "knowledge plan returned wrong kind"
    );
    anyhow::ensure!(
        plan.output["plan_only"] == json!(true),
        "knowledge injection must be plan-only"
    );
    Ok(())
}

pub(crate) async fn context_lab() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from("packages/plurora/context-lab/manifest.yaml"))
                .await?,
        )
        .await?;
    let preview = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/context-lab/assemble_preview".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/context-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"budget": 20, "sources": [{"id": "short", "text": "fits"}, {"id": "long", "text": "this source should be omitted by budget"}]}),
        })
        .await?;
    anyhow::ensure!(
        preview.output["kind"] == json!("context_preview"),
        "context preview returned wrong kind"
    );
    anyhow::ensure!(
        preview.output["omitted"]
            .as_array()
            .map(|o| !o.is_empty())
            .unwrap_or(false),
        "context preview should report omitted sources"
    );
    let rendered = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/context-lab/render_template".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/context-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"template": "Hello {{name}}", "variables": {"name": "Plurora"}}),
        })
        .await?;
    anyhow::ensure!(
        rendered.output["rendered"] == json!("Hello Plurora"),
        "context template render failed"
    );
    Ok(())
}

pub(crate) async fn text_transform_lab() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/text-transform-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let preview = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/text-transform-lab/apply_preview".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/text-transform-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"text": "Mira whispers now", "rules": [{"id": "quiet", "find": "whispers", "replace": "says quietly"}]}),
        })
        .await?;
    anyhow::ensure!(
        preview.output["kind"] == json!("text_transform_preview"),
        "text transform preview returned wrong kind"
    );
    anyhow::ensure!(
        preview.output["output"] == json!("Mira says quietly now"),
        "text transform did not apply deterministic replacement"
    );
    anyhow::ensure!(
        preview.output["trace"]
            .as_array()
            .map(|t| !t.is_empty())
            .unwrap_or(false),
        "text transform missing trace"
    );
    let validation = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/text-transform-lab/validate_rules".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/text-transform-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"rules": [{"id": "bad"}]}),
        })
        .await?;
    anyhow::ensure!(
        validation.output["valid"] == json!(false),
        "invalid transform rule should be reported"
    );
    Ok(())
}

pub(crate) async fn model_connector_lab() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/model-connector-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let families = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-connector-lab/describe_families".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-connector-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await?;
    anyhow::ensure!(
        families.output["kind"] == json!("model_provider_families"),
        "model connector families wrong kind"
    );
    anyhow::ensure!(
        families.output["families"]
            .as_array()
            .map(|f| f.len() >= 6)
            .unwrap_or(false),
        "expected declared provider families"
    );
    let valid = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-connector-lab/validate_profile".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-connector-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"provider_family": "openai-compatible", "base_url": "http://127.0.0.1:11434/v1", "model_id": "fixture", "secret_ref": "env:LOCAL_KEY"}),
        })
        .await?;
    anyhow::ensure!(
        valid.output["valid"] == json!(true),
        "valid connector profile rejected"
    );
    anyhow::ensure!(
        valid.output["verification_level"] == json!("not_verified"),
        "connector Alpha must not claim live verification"
    );
    let raw_secret = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-connector-lab/validate_profile".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-connector-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"provider_family": "openai", "model_id": "fixture", "api_key": "rawSecretPlaceholder1234567890ABCDEF"}),
        })
        .await?;
    anyhow::ensure!(
        raw_secret.output["valid"] == json!(false),
        "raw secret profile should be invalid"
    );
    let plan = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-connector-lab/discovery_plan".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-connector-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"provider_family": "google"}),
        })
        .await?;
    anyhow::ensure!(
        plan.output["network_performed"] == json!(false),
        "discovery plan must not perform network"
    );
    Ok(())
}

pub(crate) async fn model_routing_lab() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/model-routing-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let bindings = json!([
        {"profile_id": "profile/low", "priority": 1, "fallback": true},
        {"profile_id": "profile/high", "priority": 10, "fallback": false}
    ]);
    let resolved = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-routing-lab/resolve_binding".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-routing-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"consumer_slot": "play.primary", "bindings": bindings}),
        })
        .await?;
    anyhow::ensure!(
        resolved.output["kind"] == json!("model_route_resolution"),
        "model routing resolution wrong kind"
    );
    anyhow::ensure!(
        resolved.output["selected"]["profile_id"] == json!("profile/high"),
        "model routing did not select highest priority"
    );
    anyhow::ensure!(
        resolved.output["inference_performed"] == json!(false),
        "model routing must not invoke inference"
    );
    let params = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-routing-lab/params_normalize".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-routing-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"params": {"temperature": 0.2, "max_tokens": 128, "provider_options": {"openai": {"reasoning_effort": "low"}}}}),
        })
        .await?;
    anyhow::ensure!(
        params.output["kind"] == json!("model_params_normalized"),
        "model params normalization wrong kind"
    );
    anyhow::ensure!(
        params.output["params"]["max_output_tokens"] == json!(128),
        "model params did not normalize max_tokens"
    );
    anyhow::ensure!(
        params.output["provider_specific_namespaced"] == json!(true),
        "provider-specific params must stay namespaced"
    );
    Ok(())
}

pub(crate) async fn capability_tool_bridge_lab() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/capability-tool-bridge-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // discover_tools: ambiguous providers marked rejected
    let discovery = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/capability-tool-bridge-lab/discover_tools".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/capability-tool-bridge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "capabilities": [
                    {
                        "capability_id": "example/echo",
                        "providers": ["plurora/pkg-a", "thirdparty/pkg-b"]
                    }
                ]
            }),
        })
        .await?;
    anyhow::ensure!(
        discovery.output["kind"] == json!("tool_bridge_discovery"),
        "tool bridge discovery wrong kind"
    );
    let tools = discovery.output["tools"].as_array().unwrap();
    anyhow::ensure!(
        tools.len() == 1,
        "tool bridge discovery should return 1 tool"
    );
    anyhow::ensure!(
        tools[0]["status"] == json!("rejected"),
        "ambiguous tool should be rejected"
    );
    anyhow::ensure!(
        tools[0]["ambiguous"] == json!(true),
        "ambiguous tool should be flagged"
    );
    // No official preference
    anyhow::ensure!(
        tools[0]["provider_package_id"].is_null(),
        "ambiguous tool should not auto-select first-party provider"
    );

    // discover_tools: explicit third-party provider works as plan
    let discovery_explicit = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/capability-tool-bridge-lab/discover_tools".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/capability-tool-bridge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "capabilities": [
                    {
                        "capability_id": "example/echo",
                        "providers": ["plurora/pkg-a", "thirdparty/pkg-b"],
                        "provider_package_id": "thirdparty/pkg-b"
                    }
                ]
            }),
        })
        .await?;
    let tools_explicit = discovery_explicit.output["tools"].as_array().unwrap();
    anyhow::ensure!(
        tools_explicit[0]["status"] == json!("available"),
        "explicit third-party provider should be available"
    );
    anyhow::ensure!(
        tools_explicit[0]["provider_package_id"] == json!("thirdparty/pkg-b"),
        "explicit provider should be preserved"
    );

    // discover_tools: explicit provider must be in candidate providers when candidates are supplied
    let discovery_bad_provider = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/capability-tool-bridge-lab/discover_tools".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/capability-tool-bridge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "capabilities": [
                    {
                        "capability_id": "example/echo",
                        "providers": ["plurora/pkg-a", "thirdparty/pkg-b"],
                        "provider_package_id": "thirdparty/not-a-provider"
                    }
                ]
            }),
        })
        .await?;
    let bad_provider_tools = discovery_bad_provider.output["tools"].as_array().unwrap();
    anyhow::ensure!(
        bad_provider_tools[0]["status"] == json!("rejected"),
        "explicit non-candidate provider should be rejected"
    );
    anyhow::ensure!(
        bad_provider_tools[0]["rejection_reason"] == json!("provider_not_in_candidates"),
        "explicit non-candidate provider wrong reason"
    );

    // invoke_tool: missing provider rejected
    let invoke_missing = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/capability-tool-bridge-lab/invoke_tool".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/capability-tool-bridge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "capability_id": "example/echo"
            }),
        })
        .await?;
    anyhow::ensure!(
        invoke_missing.output["kind"] == json!("tool_bridge_invocation_plan"),
        "invoke_tool wrong kind"
    );
    anyhow::ensure!(
        invoke_missing.output["status"] == json!("rejected"),
        "missing provider should be rejected"
    );
    anyhow::ensure!(
        invoke_missing.output["rejection_reason"] == json!("missing_provider"),
        "missing provider wrong reason"
    );

    // invoke_tool: explicit third-party provider produces plan
    let invoke_explicit = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/capability-tool-bridge-lab/invoke_tool".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/capability-tool-bridge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "capability_id": "example/echo",
                "provider_package_id": "thirdparty/my-tool"
            }),
        })
        .await?;
    anyhow::ensure!(
        invoke_explicit.output["status"] == json!("plan_ready"),
        "explicit third-party invoke should be plan_ready"
    );
    anyhow::ensure!(
        invoke_explicit.output["method"] == json!("capability.invoke"),
        "invoke_tool method should be capability.invoke"
    );
    anyhow::ensure!(
        invoke_explicit.output["requires_user_approval"] == json!(true),
        "invoke plan must require approval"
    );

    // invoke_tool: explicit provider must match supplied candidates
    let invoke_bad_provider = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/capability-tool-bridge-lab/invoke_tool".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/capability-tool-bridge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "capability_id": "example/echo",
                "provider_package_id": "thirdparty/not-a-provider",
                "providers": ["plurora/pkg-a", "thirdparty/pkg-b"]
            }),
        })
        .await?;
    anyhow::ensure!(
        invoke_bad_provider.output["status"] == json!("rejected"),
        "invoke explicit non-candidate provider should reject"
    );
    anyhow::ensure!(
        invoke_bad_provider.output["rejection_reason"] == json!("provider_not_in_candidates"),
        "invoke explicit non-candidate wrong reason"
    );

    // preview_tool_permissions: denied reports missing permission
    let preview_denied = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(
                "plurora/capability-tool-bridge-lab/preview_tool_permissions".to_string(),
            ),
            caller_package_id: None,
            provider_package_id: Some("plurora/capability-tool-bridge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "required_permissions": ["capabilities.invoke"],
                "grants": [],
                "provider_package_id": "plurora/echo"
            }),
        })
        .await?;
    anyhow::ensure!(
        preview_denied.output["kind"] == json!("tool_bridge_permission_preview"),
        "preview wrong kind"
    );
    anyhow::ensure!(
        preview_denied.output["allowed"] == json!(false),
        "denied preview should not be allowed"
    );
    let missing = preview_denied.output["missing_permissions"]
        .as_array()
        .unwrap();
    anyhow::ensure!(missing.len() == 1, "should report 1 missing permission");

    // preview_tool_permissions: granted with wildcard
    let preview_granted = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(
                "plurora/capability-tool-bridge-lab/preview_tool_permissions".to_string(),
            ),
            caller_package_id: None,
            provider_package_id: Some("plurora/capability-tool-bridge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "required_permissions": ["capabilities.invoke"],
                "grants": ["*"],
                "provider_package_id": "plurora/echo"
            }),
        })
        .await?;
    anyhow::ensure!(
        preview_granted.output["allowed"] == json!(true),
        "granted preview should be allowed"
    );

    // raw secret payload: unsafe_blocked
    let raw_secret = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/capability-tool-bridge-lab/invoke_tool".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/capability-tool-bridge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "capability_id": "example/echo",
                "provider_package_id": "plurora/pkg",
                "api_key": "rawSecretPlaceholder1234567890ABCDEF"
            }),
        })
        .await?;
    anyhow::ensure!(
        raw_secret.output["redaction_state"] == json!("unsafe_blocked"),
        "raw secret should be unsafe_blocked"
    );
    anyhow::ensure!(
        raw_secret.output["status"] == json!("rejected"),
        "raw secret invoke should be rejected"
    );

    // stream_tool: missing provider rejected
    let stream_missing = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/capability-tool-bridge-lab/stream_tool".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/capability-tool-bridge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "capability_id": "example/stream"
            }),
        })
        .await?;
    anyhow::ensure!(
        stream_missing.output["kind"] == json!("tool_bridge_stream_plan"),
        "stream_tool wrong kind"
    );
    anyhow::ensure!(
        stream_missing.output["status"] == json!("rejected"),
        "missing provider stream should be rejected"
    );
    anyhow::ensure!(
        stream_missing.output["method"] == json!("capability.stream"),
        "stream method should be capability.stream"
    );

    // stream_tool: explicit provider must match supplied candidates
    let stream_bad_provider = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/capability-tool-bridge-lab/stream_tool".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/capability-tool-bridge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "capability_id": "example/stream",
                "provider_package_id": "thirdparty/not-a-provider",
                "providers": ["plurora/pkg-a", "thirdparty/pkg-b"]
            }),
        })
        .await?;
    anyhow::ensure!(
        stream_bad_provider.output["status"] == json!("rejected"),
        "stream explicit non-candidate provider should reject"
    );
    anyhow::ensure!(
        stream_bad_provider.output["rejection_reason"] == json!("provider_not_in_candidates"),
        "stream explicit non-candidate wrong reason"
    );

    // explain_tool_call: audit-safe summary
    let explain = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/capability-tool-bridge-lab/explain_tool_call".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/capability-tool-bridge-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "capability_id": "example/echo",
                "provider_package_id": "thirdparty/my-tool",
                "method": "capability.invoke"
            }),
        })
        .await?;
    anyhow::ensure!(
        explain.output["kind"] == json!("tool_bridge_explanation"),
        "explain wrong kind"
    );
    anyhow::ensure!(
        explain.output["redaction_state"] == json!("clean"),
        "explain with clean input should be clean"
    );

    // surfaces discoverable: forge_panel, assistant_action, home_card
    let forge_surfaces = runtime
        .list_surface_contributions(Some("forge_panel".to_string()))
        .await;
    let has_forge = forge_surfaces
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|r| r["package_id"] == json!("plurora/capability-tool-bridge-lab"))
        })
        .unwrap_or(false);
    anyhow::ensure!(has_forge, "tool bridge forge_panel surface missing");

    let assistant_surfaces = runtime
        .list_surface_contributions(Some("assistant_action".to_string()))
        .await;
    let has_assistant = assistant_surfaces
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|r| r["package_id"] == json!("plurora/capability-tool-bridge-lab"))
        })
        .unwrap_or(false);
    anyhow::ensure!(
        has_assistant,
        "tool bridge assistant_action surface missing"
    );

    let home_surfaces = runtime
        .list_surface_contributions(Some("home_card".to_string()))
        .await;
    let has_home = home_surfaces
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|r| r["package_id"] == json!("plurora/capability-tool-bridge-lab"))
        })
        .unwrap_or(false);
    anyhow::ensure!(has_home, "tool bridge home_card surface missing");

    Ok(())
}

pub(crate) async fn model_provider_lab() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/model-provider-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // list_supported_families: returns all eight families
    let families = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/list_supported_families".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await?;
    anyhow::ensure!(
        families.output["kind"] == json!("model_provider_families"),
        "model provider families wrong kind"
    );
    let family_list = families.output["families"].as_array().unwrap();
    anyhow::ensure!(
        family_list.len() == 8,
        "expected 8 provider families, got {}",
        family_list.len()
    );
    let family_ids: Vec<&str> = family_list
        .iter()
        .filter_map(|f| f["id"].as_str())
        .collect();
    for expected in [
        "openai",
        "anthropic",
        "gemini",
        "openai_compatible",
        "openrouter",
        "deepseek",
        "xai",
        "fireworks",
    ] {
        anyhow::ensure!(
            family_ids.contains(&expected),
            "missing family: {}",
            expected
        );
    }
    // Every family must have network_performed: false
    for f in family_list {
        anyhow::ensure!(
            f["network_performed"] == json!(false),
            "family {} should have network_performed:false",
            f["id"]
        );
    }
    anyhow::ensure!(
        families.output["network_performed"] == json!(false),
        "families output must have network_performed:false"
    );
    anyhow::ensure!(
        families.output["inference_performed"] == json!(false),
        "families output must have inference_performed:false"
    );

    // validate_profile: raw secret rejected
    let raw_secret = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/validate_profile".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "family": "openai",
                "credential": "rawSecretPlaceholder1234567890ABCDEF",
                "model": "gpt-4o"
            }),
        })
        .await?;
    anyhow::ensure!(
        raw_secret.output["valid"] == json!(false),
        "raw secret profile should be invalid"
    );

    // validate_profile: openai_compatible with http base_url rejected
    let http_invalid = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/validate_profile".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "family": "openai_compatible",
                "credential": "secret_ref:env:MY_KEY",
                "model": "local",
                "base_url": "http://127.0.0.1:11434/v1"
            }),
        })
        .await?;
    anyhow::ensure!(
        http_invalid.output["valid"] == json!(false),
        "openai_compatible http base_url should be invalid"
    );

    // validate_profile: openai_compatible with https base_url accepted
    let https_valid = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/validate_profile".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "family": "openai_compatible",
                "credential": "secret_ref:env:MY_KEY",
                "model": "local",
                "base_url": "https://my-llm.example.com/v1"
            }),
        })
        .await?;
    anyhow::ensure!(
        https_valid.output["valid"] == json!(true),
        "openai_compatible https base_url should be valid"
    );

    // normalize_request: anthropic
    let norm_anthropic = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/normalize_request".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "anthropic",
                    "model": "claude-3-5-sonnet-20241022",
                    "credential": "secret_ref:env:ANTHROPIC_KEY"
                },
                "messages": [{"role": "user", "content": "hello"}],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        norm_anthropic.output["request_dialect"] == json!("anthropic_messages"),
        "anthropic normalize wrong dialect"
    );
    anyhow::ensure!(
        norm_anthropic.output["endpoint"]
            .as_str()
            .unwrap_or_default()
            .ends_with("/v1/messages"),
        "anthropic wrong endpoint"
    );
    anyhow::ensure!(
        norm_anthropic.output["network_performed"] == json!(false),
        "normalize must not perform network"
    );
    anyhow::ensure!(
        norm_anthropic.output["inference_performed"] == json!(false),
        "normalize must not perform inference"
    );
    // No raw secret echoed
    let output_str = serde_json::to_string(&norm_anthropic.output).unwrap();
    anyhow::ensure!(
        !output_str.contains("sk-"),
        "no raw secret should appear in normalize output"
    );

    // normalize_request: gemini
    let norm_gemini = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/normalize_request".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "gemini",
                    "model": "gemini-2.0-flash",
                    "credential": "secret_ref:env:GEMINI_KEY"
                },
                "messages": [],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        norm_gemini.output["request_dialect"] == json!("gemini_generate_content"),
        "gemini normalize wrong dialect"
    );
    anyhow::ensure!(
        norm_gemini.output["stream_family"] == json!("typed_chunk_stream"),
        "gemini wrong stream family"
    );

    // normalize_request: openrouter
    let norm_openrouter = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/normalize_request".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "openrouter",
                    "model": "openai/gpt-4o",
                    "credential": "secret_ref:env:OPENROUTER_KEY"
                },
                "messages": [],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        norm_openrouter.output["request_dialect"] == json!("openai_chat")
            || norm_openrouter.output["request_dialect"] == json!("stateless_responses"),
        "openrouter normalize wrong dialect"
    );
    anyhow::ensure!(
        norm_openrouter.output["network_performed"] == json!(false),
        "openrouter normalize must not perform network"
    );

    // normalize_request: deepseek
    let norm_deepseek = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/normalize_request".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "deepseek",
                    "model": "deepseek-chat",
                    "credential": "secret_ref:env:DEEPSEEK_KEY"
                },
                "messages": [],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        norm_deepseek.output["request_dialect"] == json!("openai_chat"),
        "deepseek normalize wrong dialect"
    );
    anyhow::ensure!(
        norm_deepseek.output["endpoint"]
            .as_str()
            .unwrap_or_default()
            .contains("deepseek.com"),
        "deepseek wrong endpoint"
    );

    // normalize_request: xai
    let norm_xai = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/normalize_request".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "xai",
                    "model": "grok-2",
                    "credential": "secret_ref:env:XAI_KEY"
                },
                "messages": [],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        norm_xai.output["request_dialect"] == json!("openai_chat")
            || norm_xai.output["request_dialect"] == json!("openai_responses"),
        "xai normalize wrong dialect"
    );
    anyhow::ensure!(
        norm_xai.output["endpoint"]
            .as_str()
            .unwrap_or_default()
            .contains("x.ai"),
        "xai wrong endpoint"
    );

    // normalize_request: fireworks
    let norm_fireworks = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/normalize_request".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "fireworks",
                    "model": "accounts/fireworks/models/llama-v3p1-70b",
                    "credential": "secret_ref:env:FIREWORKS_KEY"
                },
                "messages": [],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        norm_fireworks.output["request_dialect"] == json!("openai_chat")
            || norm_fireworks.output["request_dialect"] == json!("fireworks_responses"),
        "fireworks normalize wrong dialect"
    );
    anyhow::ensure!(
        norm_fireworks.output["endpoint"]
            .as_str()
            .unwrap_or_default()
            .contains("fireworks.ai"),
        "fireworks wrong endpoint"
    );

    // explain_error: 401 -> authentication
    let err_401 = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/explain_error".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "status": 401,
                "family": "openai",
                "stage": "request"
            }),
        })
        .await?;
    anyhow::ensure!(
        err_401.output["error_kind"] == json!("authentication"),
        "401 should map to authentication"
    );
    anyhow::ensure!(
        err_401.output["retryable"] == json!(false),
        "401 should not be retryable"
    );

    // explain_error: 429 -> rate_limit
    let err_429 = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/explain_error".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "status": 429,
                "family": "anthropic",
                "stage": "request"
            }),
        })
        .await?;
    anyhow::ensure!(
        err_429.output["error_kind"] == json!("rate_limit"),
        "429 should map to rate_limit"
    );
    anyhow::ensure!(
        err_429.output["retryable"] == json!(true),
        "429 should be retryable"
    );

    // explain_error: 529 -> overloaded
    let err_529 = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/explain_error".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "status": 529,
                "family": "anthropic",
                "stage": "stream"
            }),
        })
        .await?;
    anyhow::ensure!(
        err_529.output["error_kind"] == json!("overloaded"),
        "529 should map to overloaded"
    );
    anyhow::ensure!(
        err_529.output["retryable"] == json!(true),
        "529 should be retryable"
    );
    anyhow::ensure!(
        err_529.output["network_performed"] == json!(false),
        "explain_error must not perform network"
    );
    anyhow::ensure!(
        err_529.output["inference_performed"] == json!(false),
        "explain_error must not perform inference"
    );

    // echo: returns input
    let echo = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/echo".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"hello": "provider"}),
        })
        .await?;
    anyhow::ensure!(
        echo.output["hello"] == json!("provider"),
        "model-provider-lab echo did not return input"
    );

    Ok(())
}

pub(crate) async fn model_provider_lab_invoke_core() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/model-provider-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // invoke openai (chat dialect)
    let inv_openai = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "openai",
                    "model": "gpt-4o",
                    "credential": "secret_ref:env:OPENAI_KEY"
                },
                "messages": [{"role": "user", "content": "hello"}],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_openai.output["kind"] == json!("model_provider_invoke_result"),
        "openai invoke wrong kind"
    );
    anyhow::ensure!(
        inv_openai.output["request_dialect"] == json!("openai_chat"),
        "openai invoke wrong dialect"
    );
    anyhow::ensure!(
        inv_openai.output["endpoint"]
            .as_str()
            .unwrap_or_default()
            .ends_with("/v1/chat/completions"),
        "openai invoke wrong endpoint"
    );
    anyhow::ensure!(
        inv_openai.output["method"] == json!("POST"),
        "openai invoke wrong method"
    );
    anyhow::ensure!(
        inv_openai.output["network_performed"] == json!(false),
        "openai invoke must not perform network"
    );
    anyhow::ensure!(
        inv_openai.output["inference_performed"] == json!(false),
        "openai invoke must not perform inference"
    );
    anyhow::ensure!(
        inv_openai.output["executor_kind"] == json!("fake_local"),
        "openai invoke executor_kind must be fake_local"
    );
    anyhow::ensure!(
        inv_openai.output["live_call_supported"] == json!(false),
        "openai invoke live_call_supported must be false"
    );
    anyhow::ensure!(
        inv_openai.output["outbound_request_shape"]["destination_host"] == json!("api.openai.com"),
        "openai invoke wrong destination_host"
    );
    anyhow::ensure!(
        inv_openai.output["outbound_request_shape"]["method"] == json!("POST"),
        "openai invoke outbound wrong method"
    );
    anyhow::ensure!(
        inv_openai.output["outbound_request_shape"]["redaction_state"] == json!("redacted"),
        "openai invoke outbound must be redacted"
    );
    // Response has id, stop_reason, usage, provider_request_id
    anyhow::ensure!(
        inv_openai.output["response"]["id"].is_string(),
        "openai invoke response missing id"
    );
    anyhow::ensure!(
        inv_openai.output["response"]["stop_reason"].is_string(),
        "openai invoke response missing stop_reason"
    );
    anyhow::ensure!(
        inv_openai.output["response"]["usage"].is_object(),
        "openai invoke response missing usage"
    );
    // No raw secret echoed
    let output_str = serde_json::to_string(&inv_openai.output).unwrap();
    anyhow::ensure!(
        !output_str.contains("sk-"),
        "no raw secret should appear in invoke output"
    );

    // invoke openai with preferResponses (responses dialect)
    let inv_openai_resp = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "openai",
                    "model": "gpt-4o",
                    "credential": "secret_ref:env:OPENAI_KEY",
                    "extra": {"preferResponses": true}
                },
                "messages": [{"role": "user", "content": "hello"}],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_openai_resp.output["request_dialect"] == json!("openai_responses"),
        "openai responses invoke wrong dialect"
    );
    anyhow::ensure!(
        inv_openai_resp.output["endpoint"]
            .as_str()
            .unwrap_or_default()
            .ends_with("/v1/responses"),
        "openai responses wrong endpoint"
    );
    anyhow::ensure!(
        inv_openai_resp.output["response"]["object"] == json!("response"),
        "openai responses fake response wrong object"
    );

    // invoke anthropic
    let inv_anthropic = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "anthropic",
                    "model": "claude-3-5-sonnet-20241022",
                    "credential": "secret_ref:env:ANTHROPIC_KEY"
                },
                "messages": [{"role": "user", "content": "hello"}],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_anthropic.output["request_dialect"] == json!("anthropic_messages"),
        "anthropic invoke wrong dialect"
    );
    anyhow::ensure!(
        inv_anthropic.output["endpoint"]
            .as_str()
            .unwrap_or_default()
            .ends_with("/v1/messages"),
        "anthropic invoke wrong endpoint"
    );
    anyhow::ensure!(
        inv_anthropic.output["outbound_request_shape"]["destination_host"]
            == json!("api.anthropic.com"),
        "anthropic invoke wrong destination_host"
    );
    anyhow::ensure!(
        inv_anthropic.output["outbound_request_shape"]["path"] == json!("/v1/messages"),
        "anthropic invoke wrong outbound path"
    );
    anyhow::ensure!(
        inv_anthropic.output["response"]["type"] == json!("message"),
        "anthropic invoke wrong response type"
    );
    anyhow::ensure!(
        inv_anthropic.output["response"]["content"].is_array(),
        "anthropic invoke response missing content"
    );
    anyhow::ensure!(
        inv_anthropic.output["response"]["usage"]["input_tokens"].is_number(),
        "anthropic invoke response missing usage"
    );
    anyhow::ensure!(
        inv_anthropic.output["network_performed"] == json!(false),
        "anthropic invoke must not perform network"
    );
    anyhow::ensure!(
        inv_anthropic.output["executor_kind"] == json!("fake_local"),
        "anthropic invoke executor_kind must be fake_local"
    );

    // invoke gemini
    let inv_gemini = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "gemini",
                    "model": "gemini-2.0-flash",
                    "credential": "secret_ref:env:GEMINI_KEY"
                },
                "messages": [],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_gemini.output["request_dialect"] == json!("gemini_generate_content"),
        "gemini invoke wrong dialect"
    );
    anyhow::ensure!(
        inv_gemini.output["outbound_request_shape"]["destination_host"]
            == json!("generativelanguage.googleapis.com"),
        "gemini invoke wrong destination_host"
    );
    anyhow::ensure!(
        inv_gemini.output["response"]["candidates"].is_array(),
        "gemini invoke response missing candidates"
    );
    anyhow::ensure!(
        inv_gemini.output["response"]["usageMetadata"].is_object(),
        "gemini invoke response missing usageMetadata"
    );
    anyhow::ensure!(
        inv_gemini.output["executor_kind"] == json!("fake_local"),
        "gemini invoke executor_kind must be fake_local"
    );

    // invoke with raw credential rejected
    let inv_raw = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "openai",
                    "model": "gpt-4o",
                    "credential": "rawSecretPlaceholder1234567890ABCDEF"
                },
                "messages": [],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_raw.output["normalized_error"]["error_kind"] == json!("secret_unavailable"),
        "raw credential invoke should produce secret_unavailable error"
    );
    anyhow::ensure!(
        inv_raw.output["network_performed"] == json!(false),
        "raw credential invoke must not perform network"
    );
    // No raw secret echoed
    let raw_output_str = serde_json::to_string(&inv_raw.output).unwrap();
    anyhow::ensure!(
        !raw_output_str.contains("sk-"),
        "no sk- pattern in raw credential invoke output"
    );

    // invoke with raw-looking header rejected
    let inv_raw_header = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "openai",
                    "model": "gpt-4o",
                    "credential": "secret_ref:env:OPENAI_KEY",
                    "headers": {"X-Debug-Secret": "rawSecretPlaceholder1234567890ABCDEF"}
                },
                "messages": [],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_raw_header.output["normalized_error"]["error_kind"] == json!("secret_unavailable"),
        "raw header invoke should produce secret_unavailable error"
    );
    anyhow::ensure!(
        inv_raw_header.output["network_performed"] == json!(false),
        "raw header invoke must not perform network"
    );

    // invoke with non-HTTPS base_url rejected before producing outbound_request_shape
    let inv_http_base = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "openai",
                    "model": "gpt-4o",
                    "credential": "secret_ref:env:OPENAI_KEY",
                    "baseUrl": "http://api.openai.com"
                },
                "messages": [],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_http_base.output["normalized_error"]["error_kind"] == json!("network_denied"),
        "non-HTTPS base_url invoke should be network_denied"
    );
    anyhow::ensure!(
        inv_http_base.output["network_performed"] == json!(false),
        "non-HTTPS base_url invoke must not perform network"
    );

    // invoke openai_compatible (requires explicit HTTPS base_url)
    let inv_compat = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "openai_compatible",
                    "model": "my-custom-model",
                    "credential": "secret_ref:env:COMPAT_KEY",
                    "baseUrl": "https://my-llm.example.com/v1"
                },
                "messages": [{"role": "user", "content": "hello"}],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_compat.output["request_dialect"] == json!("openai_chat"),
        "openai_compatible invoke wrong dialect"
    );
    anyhow::ensure!(
        inv_compat.output["endpoint"] == json!("https://my-llm.example.com/v1/chat/completions"),
        "openai_compatible invoke wrong endpoint"
    );
    anyhow::ensure!(
        inv_compat.output["outbound_request_shape"]["destination_host"]
            == json!("my-llm.example.com"),
        "openai_compatible invoke wrong destination_host"
    );
    anyhow::ensure!(
        inv_compat.output["outbound_request_shape"]["path"] == json!("/v1/chat/completions"),
        "openai_compatible invoke wrong outbound path"
    );
    anyhow::ensure!(
        inv_compat.output["response"]["object"] == json!("chat.completion"),
        "openai_compatible invoke wrong response object"
    );
    anyhow::ensure!(
        inv_compat.output["response"]["choices"].is_array(),
        "openai_compatible invoke missing choices"
    );
    anyhow::ensure!(
        inv_compat.output["response"]["usage"].is_object(),
        "openai_compatible invoke missing usage"
    );
    anyhow::ensure!(
        inv_compat.output["executor_kind"] == json!("fake_local"),
        "openai_compatible invoke executor_kind must be fake_local"
    );
    anyhow::ensure!(
        inv_compat.output["network_performed"] == json!(false),
        "openai_compatible invoke must not perform network"
    );

    // invoke openai_compatible missing base_url → bad_request
    let inv_compat_no_base = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "openai_compatible",
                    "model": "my-model",
                    "credential": "secret_ref:env:COMPAT_KEY"
                },
                "messages": [],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_compat_no_base.output["normalized_error"]["error_kind"] == json!("bad_request"),
        "openai_compatible missing base_url should be bad_request"
    );
    anyhow::ensure!(
        inv_compat_no_base.output["network_performed"] == json!(false),
        "missing base_url invoke must not perform network"
    );

    // invoke openai_compatible http base_url → network_denied
    let inv_compat_http = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "openai_compatible",
                    "model": "my-model",
                    "credential": "secret_ref:env:COMPAT_KEY",
                    "baseUrl": "http://insecure.example.com/v1"
                },
                "messages": [],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_compat_http.output["normalized_error"]["error_kind"] == json!("network_denied"),
        "openai_compatible http base_url should be network_denied"
    );

    // invoke openrouter (chat dialect)
    let inv_or = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "openrouter",
                    "model": "openai/gpt-4o",
                    "credential": "secret_ref:env:OPENROUTER_KEY"
                },
                "messages": [{"role": "user", "content": "hello"}],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_or.output["request_dialect"] == json!("openai_chat"),
        "openrouter invoke wrong dialect"
    );
    anyhow::ensure!(
        inv_or.output["endpoint"]
            .as_str()
            .unwrap_or_default()
            .ends_with("/chat/completions"),
        "openrouter invoke wrong endpoint"
    );
    anyhow::ensure!(
        inv_or.output["outbound_request_shape"]["destination_host"] == json!("openrouter.ai"),
        "openrouter invoke wrong destination_host"
    );
    anyhow::ensure!(
        inv_or.output["response"]["choices"].is_array(),
        "openrouter invoke missing choices"
    );
    anyhow::ensure!(
        inv_or.output["executor_kind"] == json!("fake_local"),
        "openrouter invoke executor_kind must be fake_local"
    );

    // invoke openrouter with preferResponses
    let inv_or_resp = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "openrouter",
                    "model": "openai/gpt-4o",
                    "credential": "secret_ref:env:OPENROUTER_KEY",
                    "extra": {"preferResponses": true}
                },
                "messages": [],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_or_resp.output["request_dialect"] == json!("stateless_responses"),
        "openrouter responses wrong dialect"
    );
    anyhow::ensure!(
        inv_or_resp.output["endpoint"]
            .as_str()
            .unwrap_or_default()
            .ends_with("/responses"),
        "openrouter responses wrong endpoint"
    );
    anyhow::ensure!(
        inv_or_resp.output["response"]["object"] == json!("response"),
        "openrouter responses wrong response object"
    );

    // invoke deepseek
    let inv_ds = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "deepseek",
                    "model": "deepseek-chat",
                    "credential": "secret_ref:env:DEEPSEEK_KEY"
                },
                "messages": [{"role": "user", "content": "hello"}],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_ds.output["request_dialect"] == json!("openai_chat"),
        "deepseek invoke wrong dialect"
    );
    anyhow::ensure!(
        inv_ds.output["endpoint"]
            .as_str()
            .unwrap_or_default()
            .ends_with("/chat/completions"),
        "deepseek invoke wrong endpoint"
    );
    anyhow::ensure!(
        inv_ds.output["outbound_request_shape"]["destination_host"] == json!("api.deepseek.com"),
        "deepseek invoke wrong destination_host"
    );
    anyhow::ensure!(
        inv_ds.output["response"]["choices"].is_array(),
        "deepseek invoke missing choices"
    );
    anyhow::ensure!(
        inv_ds.output["response"]["usage"]["prompt_cache_hit_tokens"].is_number(),
        "deepseek invoke missing cache usage"
    );
    anyhow::ensure!(
        inv_ds.output["executor_kind"] == json!("fake_local"),
        "deepseek invoke executor_kind must be fake_local"
    );

    // invoke xai (chat dialect)
    let inv_xai = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "xai",
                    "model": "grok-3",
                    "credential": "secret_ref:env:XAI_KEY"
                },
                "messages": [{"role": "user", "content": "hello"}],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_xai.output["request_dialect"] == json!("openai_chat"),
        "xai invoke wrong dialect"
    );
    anyhow::ensure!(
        inv_xai.output["endpoint"]
            .as_str()
            .unwrap_or_default()
            .ends_with("/v1/chat/completions"),
        "xai invoke wrong endpoint"
    );
    anyhow::ensure!(
        inv_xai.output["outbound_request_shape"]["destination_host"] == json!("api.x.ai"),
        "xai invoke wrong destination_host"
    );
    anyhow::ensure!(
        inv_xai.output["response"]["choices"].is_array(),
        "xai invoke missing choices"
    );
    anyhow::ensure!(
        inv_xai.output["executor_kind"] == json!("fake_local"),
        "xai invoke executor_kind must be fake_local"
    );

    // invoke xai with preferResponses
    let inv_xai_resp = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "xai",
                    "model": "grok-3",
                    "credential": "secret_ref:env:XAI_KEY",
                    "extra": {"preferResponses": true}
                },
                "messages": [],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_xai_resp.output["request_dialect"] == json!("openai_responses"),
        "xai responses wrong dialect"
    );
    anyhow::ensure!(
        inv_xai_resp.output["response"]["object"] == json!("response"),
        "xai responses wrong response object"
    );

    // invoke fireworks (chat dialect)
    let inv_fw = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "fireworks",
                    "model": "accounts/fireworks/models/llama-v3p1-8b-instruct",
                    "credential": "secret_ref:env:FIREWORKS_KEY"
                },
                "messages": [{"role": "user", "content": "hello"}],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_fw.output["request_dialect"] == json!("openai_chat"),
        "fireworks invoke wrong dialect"
    );
    anyhow::ensure!(
        inv_fw.output["endpoint"]
            .as_str()
            .unwrap_or_default()
            .ends_with("/chat/completions"),
        "fireworks invoke wrong endpoint"
    );
    anyhow::ensure!(
        inv_fw.output["outbound_request_shape"]["destination_host"] == json!("api.fireworks.ai"),
        "fireworks invoke wrong destination_host"
    );
    anyhow::ensure!(
        inv_fw.output["response"]["choices"].is_array(),
        "fireworks invoke missing choices"
    );
    anyhow::ensure!(
        inv_fw.output["executor_kind"] == json!("fake_local"),
        "fireworks invoke executor_kind must be fake_local"
    );

    // invoke fireworks with preferResponses
    let inv_fw_resp = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/invoke".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "profile": {
                    "family": "fireworks",
                    "model": "accounts/fireworks/models/llama-v3p1-8b-instruct",
                    "credential": "secret_ref:env:FIREWORKS_KEY",
                    "extra": {"preferResponses": true}
                },
                "messages": [],
                "stream": false
            }),
        })
        .await?;
    anyhow::ensure!(
        inv_fw_resp.output["request_dialect"] == json!("fireworks_responses"),
        "fireworks responses wrong dialect"
    );
    anyhow::ensure!(
        inv_fw_resp.output["response"]["object"] == json!("response"),
        "fireworks responses wrong response object"
    );

    Ok(())
}

pub(crate) async fn model_provider_lab_normalize_stream() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/model-provider-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // --- Default fake stream samples for all eight families ---

    let families_and_stream_families = [
        ("openai", "semantic_sse"),
        ("anthropic", "semantic_sse"),
        ("gemini", "typed_chunk_stream"),
        ("openai_compatible", "delta_sse"),
        ("openrouter", "semantic_sse"),
        ("deepseek", "delta_sse"),
        ("xai", "semantic_sse"),
        ("fireworks", "delta_sse"),
    ];

    for (family, expected_sf) in &families_and_stream_families {
        let result = runtime
            .invoke_capability(CapabilityInvocationRequest {
                handle: None,
                capability_id: Some("plurora/model-provider-lab/normalize_stream".to_string()),
                caller_package_id: None,
                provider_package_id: Some("plurora/model-provider-lab".to_string()),
                version: None,
                session_id: None,
                input: json!({
                    "family": family,
                }),
            })
            .await?;
        anyhow::ensure!(
            result.output["kind"] == json!("model_provider_stream_normalization"),
            "{} normalize_stream wrong kind",
            family
        );
        anyhow::ensure!(
            result.output["family"] == json!(family),
            "{} normalize_stream wrong family",
            family
        );
        anyhow::ensure!(
            result.output["stream_family"] == json!(expected_sf),
            "{} normalize_stream wrong stream_family: got {:?}",
            family,
            result.output["stream_family"]
        );
        anyhow::ensure!(
            result.output["network_performed"] == json!(false),
            "{} normalize_stream must not perform network",
            family
        );
        anyhow::ensure!(
            result.output["inference_performed"] == json!(false),
            "{} normalize_stream must not perform inference",
            family
        );

        // Frames must have start + at least one chunk + end
        let frames = result.output["frames"]
            .as_array()
            .unwrap_or_else(|| panic!("{} missing frames array", family));
        anyhow::ensure!(
            !frames.is_empty(),
            "{} normalize_stream must have frames",
            family
        );
        let kinds: Vec<&str> = frames
            .iter()
            .map(|f| f["kind"].as_str().unwrap_or_default())
            .collect();
        anyhow::ensure!(
            kinds.first() == Some(&"start"),
            "{} first frame must be start, got {:?}",
            family,
            kinds.first()
        );
        anyhow::ensure!(
            kinds.last() == Some(&"end"),
            "{} last frame must be end, got {:?}",
            family,
            kinds.last()
        );
        anyhow::ensure!(
            kinds.contains(&"chunk"),
            "{} must have at least one chunk frame",
            family
        );
        anyhow::ensure!(
            result.output["terminal_frame_consistent"] == json!(true),
            "{} terminal_frame_consistent must be true",
            family
        );

        // Every frame must have invocation_id, sequence, redaction_state, metadata
        for (i, frame) in frames.iter().enumerate() {
            anyhow::ensure!(
                frame["invocation_id"].is_string(),
                "{} frame {} missing invocation_id",
                family,
                i
            );
            anyhow::ensure!(
                frame["sequence"].is_number(),
                "{} frame {} missing sequence",
                family,
                i
            );
            anyhow::ensure!(
                frame["redaction_state"] == json!("redacted"),
                "{} frame {} must be redacted",
                family,
                i
            );
            anyhow::ensure!(
                frame["metadata"]["provider_family"] == json!(family),
                "{} frame {} wrong metadata.provider_family",
                family,
                i
            );
        }

        // No raw secrets in output
        let output_str = serde_json::to_string(&result.output).unwrap();
        anyhow::ensure!(
            !output_str.contains("sk-"),
            "{} no sk- in normalize_stream output",
            family
        );
    }

    // --- Normalize OpenAI delta_sse sample_provider_events ---
    let openai_events = json!([
        {"choices": [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": null}]},
        {"choices": [{"index": 0, "delta": {"content": "Hello"}, "finish_reason": null}]},
        {"choices": [{"index": 0, "delta": {"content": " world"}, "finish_reason": null}]},
        {"choices": [{"index": 0, "delta": {"content": ""}, "finish_reason": "stop"}]},
        {"usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}}
    ]);
    let openai_result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/normalize_stream".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "family": "openai",
                "invocation_id": "inv_test_openai_stream",
                "sample_provider_events": openai_events,
            }),
        })
        .await?;
    let oframes = openai_result.output["frames"].as_array().unwrap();
    let okinds: Vec<&str> = oframes
        .iter()
        .map(|f| f["kind"].as_str().unwrap_or_default())
        .collect();
    // Should have at least some chunk and end frames
    anyhow::ensure!(
        okinds.contains(&"chunk"),
        "openai sample events must produce chunk frames"
    );
    // Must have an end frame (from finish_reason or [DONE])
    anyhow::ensure!(
        okinds.contains(&"end") || okinds.contains(&"progress"),
        "openai sample events must produce end or progress frame"
    );
    anyhow::ensure!(
        openai_result.output["terminal_frame_consistent"] == json!(true),
        "openai sample terminal_frame_consistent must be true"
    );
    // Frame invocation_id must match
    for frame in oframes {
        anyhow::ensure!(
            frame["invocation_id"] == json!("inv_test_openai_stream"),
            "openai sample frame invocation_id mismatch"
        );
    }

    // --- Normalize Anthropic semantic_sse sample_provider_events ---
    let anthropic_events = json!([
        {"type": "message_start", "message": {"id": "msg_001", "role": "assistant", "usage": {"input_tokens": 10}}},
        {"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}},
        {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello world"}},
        {"type": "content_block_stop", "index": 0},
        {"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 5}},
        {"type": "message_stop"}
    ]);
    let anthropic_result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/normalize_stream".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "family": "anthropic",
                "invocation_id": "inv_test_anthropic_stream",
                "sample_provider_events": anthropic_events,
            }),
        })
        .await?;
    let aframes = anthropic_result.output["frames"].as_array().unwrap();
    let akinds: Vec<&str> = aframes
        .iter()
        .map(|f| f["kind"].as_str().unwrap_or_default())
        .collect();
    anyhow::ensure!(
        akinds.contains(&"start"),
        "anthropic sample must have start frame"
    );
    anyhow::ensure!(
        akinds.contains(&"chunk"),
        "anthropic sample must have chunk frame"
    );
    anyhow::ensure!(
        akinds.contains(&"end"),
        "anthropic sample must have end frame"
    );
    anyhow::ensure!(
        anthropic_result.output["terminal_frame_consistent"] == json!(true),
        "anthropic sample terminal_frame_consistent must be true"
    );

    // --- Normalize Gemini typed_chunk_stream sample_provider_events ---
    let gemini_events = json!([
        {"candidates": [{"content": {"parts": [{"text": "Hello"}], "role": "model"}, "index": 0}]},
        {"candidates": [{"content": {"parts": [{"text": " world"}], "role": "model"}, "index": 0}]},
        {"candidates": [{"content": {"parts": [], "role": "model"}, "finishReason": "STOP", "index": 0}]},
        {"usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 5, "totalTokenCount": 15}}
    ]);
    let gemini_result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/normalize_stream".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "family": "gemini",
                "invocation_id": "inv_test_gemini_stream",
                "sample_provider_events": gemini_events,
            }),
        })
        .await?;
    let gframes = gemini_result.output["frames"].as_array().unwrap();
    let gkinds: Vec<&str> = gframes
        .iter()
        .map(|f| f["kind"].as_str().unwrap_or_default())
        .collect();
    anyhow::ensure!(
        gkinds.contains(&"chunk"),
        "gemini sample must have chunk frame"
    );
    anyhow::ensure!(
        gkinds.contains(&"end"),
        "gemini sample must have end frame (from finishReason)"
    );
    anyhow::ensure!(
        gemini_result.output["terminal_frame_consistent"] == json!(true),
        "gemini sample terminal_frame_consistent must be true"
    );

    // --- Normalize OpenRouter delta_sse sample_provider_events ---
    let or_events = json!([
        {"choices": [{"index": 0, "delta": {"content": "Hi"}, "finish_reason": null}]},
        {"choices": [{"index": 0, "delta": {"content": ""}, "finish_reason": "stop"}]}
    ]);
    let or_result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/normalize_stream".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "family": "openrouter",
                "sample_provider_events": or_events,
            }),
        })
        .await?;
    let orframes = or_result.output["frames"].as_array().unwrap();
    let orkinds: Vec<&str> = orframes
        .iter()
        .map(|f| f["kind"].as_str().unwrap_or_default())
        .collect();
    anyhow::ensure!(
        orkinds.contains(&"chunk"),
        "openrouter sample must have chunk frame"
    );
    anyhow::ensure!(
        orkinds.contains(&"end"),
        "openrouter sample must have end frame"
    );

    // --- Unsupported family ---
    let bad_result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/model-provider-lab/normalize_stream".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/model-provider-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "family": "nonexistent",
            }),
        })
        .await?;
    anyhow::ensure!(
        bad_result.output["kind"] == json!("model_provider_stream_normalization"),
        "unsupported family should still return right kind"
    );
    anyhow::ensure!(
        bad_result.output["frames"]
            .as_array()
            .map(|a| a.is_empty())
            .unwrap_or(false),
        "unsupported family should have empty frames"
    );
    anyhow::ensure!(
        bad_result.output["terminal_frame_consistent"] == json!(false),
        "unsupported family terminal_frame_consistent must be false"
    );

    Ok(())
}

pub(crate) async fn pi_agent_runtime_lab() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/pi-agent-runtime-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    // run: deterministic no-inference no-network plan
    let run_result = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/pi-agent-runtime-lab/run".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/pi-agent-runtime-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await?;
    anyhow::ensure!(
        run_result.output["kind"] == json!("pi_agent_run_plan"),
        "pi-agent run returned wrong kind"
    );
    anyhow::ensure!(
        run_result.output["inference_performed"] == json!(false),
        "pi-agent run must not perform inference"
    );
    anyhow::ensure!(
        run_result.output["network_performed"] == json!(false),
        "pi-agent run must not perform network"
    );
    anyhow::ensure!(
        run_result.output["trace_events"].is_array(),
        "pi-agent run missing trace_events"
    );
    anyhow::ensure!(
        run_result.output["stream_frames"].is_array(),
        "pi-agent run missing stream_frames"
    );
    anyhow::ensure!(
        run_result.output["proposal_draft"].is_object(),
        "pi-agent run missing proposal_draft"
    );
    anyhow::ensure!(
        run_result.output["provenance"]["package_id"] == json!("plurora/pi-agent-runtime-lab"),
        "pi-agent run provenance mismatch"
    );

    // explain_run: no-inference explanation
    let explain = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/pi-agent-runtime-lab/explain_run".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/pi-agent-runtime-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"trace_events": [{"step": 1}, {"step": 2}]}),
        })
        .await?;
    anyhow::ensure!(
        explain.output["kind"] == json!("pi_agent_run_explanation"),
        "pi-agent explain_run wrong kind"
    );
    anyhow::ensure!(
        explain.output["inference_performed"] == json!(false),
        "pi-agent explain_run must not claim inference"
    );
    anyhow::ensure!(
        explain.output["network_performed"] == json!(false),
        "pi-agent explain_run must not claim network"
    );
    anyhow::ensure!(
        explain.output["trace_event_count"] == json!(2),
        "pi-agent explain_run wrong event count"
    );

    // draft_proposal: approval-gated
    let proposal = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/pi-agent-runtime-lab/draft_proposal".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/pi-agent-runtime-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"change": "agent-driven modification"}),
        })
        .await?;
    anyhow::ensure!(
        proposal.output["kind"] == json!("pi_agent_proposal"),
        "pi-agent draft_proposal wrong kind"
    );
    anyhow::ensure!(
        proposal.output["requires_user_approval"] == json!(true),
        "pi-agent proposal must require approval"
    );
    anyhow::ensure!(
        proposal.output["provenance"]["package_id"] == json!("plurora/pi-agent-runtime-lab"),
        "pi-agent proposal provenance mismatch"
    );

    // summarize_trace: no-inference summary
    let trace = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/pi-agent-runtime-lab/summarize_trace".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/pi-agent-runtime-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"trace_events": [{"e": 1}, {"e": 2}, {"e": 3}]}),
        })
        .await?;
    anyhow::ensure!(
        trace.output["kind"] == json!("pi_agent_trace_summary"),
        "pi-agent summarize_trace wrong kind"
    );
    anyhow::ensure!(
        trace.output["event_count"] == json!(3),
        "pi-agent summarize_trace wrong event count"
    );
    anyhow::ensure!(
        trace.output["inference_performed"] == json!(false),
        "pi-agent summarize_trace must not claim inference"
    );
    anyhow::ensure!(
        trace.output["network_performed"] == json!(false),
        "pi-agent summarize_trace must not claim network"
    );

    // echo: passthrough
    let echo = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/pi-agent-runtime-lab/echo".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/pi-agent-runtime-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"hello": "agent"}),
        })
        .await?;
    anyhow::ensure!(
        echo.output["kind"] == json!("pi_agent_echo"),
        "pi-agent echo wrong kind"
    );
    anyhow::ensure!(
        echo.output["input"]["hello"] == json!("agent"),
        "pi-agent echo did not pass through input"
    );

    // surfaces discoverable: assistant_action, forge_panel, home_card
    let assistant_surfaces = runtime
        .list_surface_contributions(Some("assistant_action".to_string()))
        .await;
    let has_assistant = assistant_surfaces
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|r| r["package_id"] == json!("plurora/pi-agent-runtime-lab"))
        })
        .unwrap_or(false);
    anyhow::ensure!(has_assistant, "pi-agent assistant_action surface missing");

    let forge_surfaces = runtime
        .list_surface_contributions(Some("forge_panel".to_string()))
        .await;
    let has_forge = forge_surfaces
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|r| r["package_id"] == json!("plurora/pi-agent-runtime-lab"))
        })
        .unwrap_or(false);
    anyhow::ensure!(has_forge, "pi-agent forge_panel surface missing");

    let home_surfaces = runtime
        .list_surface_contributions(Some("home_card".to_string()))
        .await;
    let has_home = home_surfaces
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|r| r["package_id"] == json!("plurora/pi-agent-runtime-lab"))
        })
        .unwrap_or(false);
    anyhow::ensure!(has_home, "pi-agent home_card surface missing");

    Ok(())
}

// ---------------------------------------------------------------------------
// Experience Beta 2 — asset-lab and projection-lab Beta 2 conformance cases
// ---------------------------------------------------------------------------

/// asset-lab content_address capability: returns stable content address and metadata convention.
pub(crate) async fn asset_lab_content_address() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from("packages/plurora/asset-lab/manifest.yaml"))
                .await?,
        )
        .await?;

    // content_address: deterministic for same content
    let ca1 = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/asset-lab/content_address".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/asset-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"content": "hello world", "disclosure": "test"}),
        })
        .await?;
    anyhow::ensure!(
        ca1.output["kind"] == json!("asset_content_address"),
        "content_address wrong kind"
    );
    anyhow::ensure!(
        ca1.output["content_address"]
            .as_str()
            .map(|s| s.starts_with("sha256:"))
            .unwrap_or(false),
        "content_address must use sha256 scheme"
    );
    anyhow::ensure!(
        ca1.output["metadata_convention"].is_object(),
        "must have metadata_convention"
    );
    anyhow::ensure!(
        ca1.output["metadata_convention"]["disclosure"] == json!("test"),
        "metadata_convention must have disclosure"
    );
    anyhow::ensure!(ca1.output["inference_performed"] == json!(false));
    anyhow::ensure!(ca1.output["network_performed"] == json!(false));

    // Deterministic: same content → same address
    let ca2 = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/asset-lab/content_address".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/asset-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"content": "hello world"}),
        })
        .await?;
    anyhow::ensure!(
        ca1.output["content_address"] == ca2.output["content_address"],
        "content_address must be deterministic"
    );

    // Different content → different address
    let ca3 = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/asset-lab/content_address".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/asset-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({"content": "different content"}),
        })
        .await?;
    anyhow::ensure!(
        ca1.output["content_address"] != ca3.output["content_address"],
        "different content must produce different address"
    );

    Ok(())
}

/// asset-lab provenance_graph capability: returns provenance graph shape.
pub(crate) async fn asset_lab_provenance_graph() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from("packages/plurora/asset-lab/manifest.yaml"))
                .await?,
        )
        .await?;

    let pg = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/asset-lab/provenance_graph".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/asset-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "asset_id": "ast_test_123",
                "nodes": [{"id": "source", "kind": "player_action"}],
                "edges": [{"from": "source", "to": "derived", "relation": "produces"}],
                "source_refs": ["asset:source:1"],
                "derived_refs": ["asset:derived:1"],
                "disclosure": "debug",
            }),
        })
        .await?;
    anyhow::ensure!(
        pg.output["kind"] == json!("asset_provenance_graph"),
        "provenance_graph wrong kind"
    );
    anyhow::ensure!(pg.output["asset_id"] == json!("ast_test_123"));
    anyhow::ensure!(pg.output["nodes"].is_array(), "must have nodes");
    anyhow::ensure!(pg.output["edges"].is_array(), "must have edges");
    anyhow::ensure!(pg.output["source_refs"].is_array(), "must have source_refs");
    anyhow::ensure!(
        pg.output["derived_refs"].is_array(),
        "must have derived_refs"
    );
    anyhow::ensure!(pg.output["disclosure"] == json!("debug"));
    anyhow::ensure!(pg.output["inference_performed"] == json!(false));
    anyhow::ensure!(pg.output["network_performed"] == json!(false));

    Ok(())
}

/// projection-lab state_snapshot capability: returns state snapshot convention and diff preview shape.
pub(crate) async fn projection_lab_state_snapshot() -> anyhow::Result<()> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/projection-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;

    let ss = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some("plurora/projection-lab/state_snapshot".to_string()),
            caller_package_id: None,
            provider_package_id: Some("plurora/projection-lab".to_string()),
            version: None,
            session_id: None,
            input: json!({
                "projection_id": "example/projection/state",
                "branch_ref": "branch:target:default",
            }),
        })
        .await?;
    anyhow::ensure!(
        ss.output["kind"] == json!("projection_state_snapshot"),
        "state_snapshot wrong kind"
    );
    anyhow::ensure!(
        ss.output["state_snapshot_convention"].is_object(),
        "must have state_snapshot_convention"
    );
    anyhow::ensure!(
        ss.output["state_snapshot_convention"]["snapshot_metadata_fields"].is_array(),
        "must have snapshot_metadata_fields"
    );
    anyhow::ensure!(
        ss.output["diff_preview_shape"].is_object(),
        "must have diff_preview_shape"
    );
    anyhow::ensure!(
        ss.output["diff_preview_shape"]["branch_aware"] == json!(true),
        "diff must be branch_aware"
    );
    anyhow::ensure!(ss.output["inference_performed"] == json!(false));
    anyhow::ensure!(ss.output["network_performed"] == json!(false));

    Ok(())
}
