//! Conformance for the modular simulation Work kit.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use plurora_core::PackageEntry;
use plurora_runtime::{CapabilityInvocationRequest, InMemoryEventStore, Runtime, RuntimeConfig};
use plurora_work::{check_port_compatibility, AssemblyId, NodeId, WorkEntrypointTarget};
use serde_json::{json, Value};

use crate::commands::{manifest, work};

const FIRST_PARTY_PACKAGE: &str = "plurora/modular-simulation";
const COMMUNITY_PACKAGE: &str = "community/modular-simulation";
const SIMULATION_MANIFEST: &str = "packages/plurora/modular-simulation/manifest.yaml";
const RENDERER_MANIFEST: &str = "packages/plurora/modular-simulation-renderer/manifest.yaml";
const RENDERER_BUNDLE: &str = "packages/plurora/modular-simulation-renderer/bundle.mjs";
const WORK_RENDERER_BUNDLE: &str = "examples/works/modular-simulation/packages/renderer/bundle.mjs";
const COMMUNITY_MANIFEST: &str = "examples/packages/community-modular-simulation/manifest.yaml";
const ADVISOR_MANIFEST: &str = "examples/packages/modular-simulation-ai-advisor/manifest.yaml";
const WORK_KIT: &str = "examples/works/modular-simulation";
const SERVER_WORK: &str = "examples/works/modular-simulation-server";
const COMMUNITY_WORK: &str = "examples/works/modular-simulation-community";
const ADVISOR_WORK: &str = "examples/works/modular-simulation-ai-advisor";

async fn runtime_with(manifests: &[&str]) -> anyhow::Result<Runtime<InMemoryEventStore>> {
    let runtime = Runtime::new(
        Arc::new(InMemoryEventStore::default()),
        RuntimeConfig::default(),
    );
    for path in manifests {
        runtime
            .load_package(manifest::read_manifest(PathBuf::from(path)).await?)
            .await?;
    }
    Ok(runtime)
}

async fn invoke(
    runtime: &Runtime<InMemoryEventStore>,
    provider: Option<&str>,
    capability: &str,
    input: Value,
) -> anyhow::Result<Value> {
    let capability_namespace = if provider == Some("example/ai-advisor") {
        "example/ai-advisor"
    } else {
        FIRST_PARTY_PACKAGE
    };
    Ok(runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(format!("{capability_namespace}/{capability}")),
            caller_package_id: None,
            provider_package_id: provider.map(str::to_string),
            version: None,
            session_id: None,
            input,
        })
        .await?
        .output)
}

pub(crate) async fn work_kit_materializes_as_portable_closure() -> anyhow::Result<()> {
    let report = work::check_work_path(Path::new(WORK_KIT))?;
    let encoded = serde_json::to_value(&report)?;
    anyhow::ensure!(encoded["complete"] == true && encoded["portable"] == true);
    anyhow::ensure!(report.nodes.len() == 2);
    anyhow::ensure!(report
        .nodes
        .iter()
        .any(|node| node.node_path.as_slice() == [NodeId::parse("simulation").unwrap()]));
    let exposed = report
        .exposed_ports
        .iter()
        .map(|port| port.port_id.as_str())
        .collect::<Vec<_>>();
    for expected in ["input", "save", "inspect", "server", "ai-advisor"] {
        anyhow::ensure!(exposed.contains(&expected));
    }
    anyhow::ensure!(encoded["state_slots"]
        .as_array()
        .is_some_and(|slots| slots.iter().any(|slot| {
            slot["state_slot_id"] == "save"
                && slot["descriptor"]["portability"] == "portable"
                && slot["descriptor"]["backup_policy"] == "required"
        })));
    let advisor = work::check_work_path(Path::new(ADVISOR_WORK))?;
    anyhow::ensure!(advisor.nodes.len() == 1 && advisor.exposed_ports.len() == 1);
    anyhow::ensure!(advisor.exposed_ports[0].port_id.as_str() == "suggest");
    let consumer = report
        .exposed_ports
        .iter()
        .find(|port| port.port_id.as_str() == "ai-advisor")
        .expect("optional AI consumer Port");
    check_port_compatibility(&advisor.exposed_ports[0].descriptor, &consumer.descriptor)?;
    Ok(())
}

pub(crate) async fn deterministic_reducer_and_portable_save() -> anyhow::Result<()> {
    let runtime = runtime_with(&[SIMULATION_MANIFEST]).await?;
    let created = invoke(
        &runtime,
        Some(FIRST_PARTY_PACKAGE),
        "create_state",
        json!({"title": "Conformance Colony"}),
    )
    .await?;
    let built = invoke(
        &runtime,
        Some(FIRST_PARTY_PACKAGE),
        "apply_input",
        json!({
            "state": created["state"].clone(),
            "action": {"kind": "build", "x": 1, "y": 2, "structure": "farm"}
        }),
    )
    .await?;
    let advanced = invoke(
        &runtime,
        Some(FIRST_PARTY_PACKAGE),
        "apply_input",
        json!({
            "state": built["state"].clone(),
            "action": {"kind": "advance"}
        }),
    )
    .await?;
    anyhow::ensure!(advanced["state"]["turn"] == 1);

    let first = invoke(
        &runtime,
        Some(FIRST_PARTY_PACKAGE),
        "export_save",
        json!({"state": advanced["state"].clone()}),
    )
    .await?;
    let second = invoke(
        &runtime,
        Some(FIRST_PARTY_PACKAGE),
        "export_save",
        json!({"state": advanced["state"].clone()}),
    )
    .await?;
    anyhow::ensure!(first["portable"] == true && first["digest"] == second["digest"]);
    anyhow::ensure!(first["digest"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:") && digest.len() == 71));
    Ok(())
}

pub(crate) async fn optional_ai_has_no_ambient_provider() -> anyhow::Result<()> {
    let runtime = runtime_with(&[SIMULATION_MANIFEST, ADVISOR_MANIFEST]).await?;
    let created = invoke(
        &runtime,
        Some(FIRST_PARTY_PACKAGE),
        "create_state",
        json!({}),
    )
    .await?;
    let result = invoke(
        &runtime,
        Some(FIRST_PARTY_PACKAGE),
        "request_ai_move",
        json!({"state": created["state"].clone()}),
    )
    .await?;
    anyhow::ensure!(result["available"] == false);
    anyhow::ensure!(result["reason_code"] == "binding_unavailable");
    anyhow::ensure!(result["inference_performed"] == false);

    let advice = invoke(
        &runtime,
        Some("example/ai-advisor"),
        "suggest",
        json!({"state": created["state"].clone()}),
    )
    .await?;
    anyhow::ensure!(advice["policy"] == "deterministic_local_v1");
    anyhow::ensure!(advice["network_performed"] == false);
    Ok(())
}

pub(crate) async fn renderer_is_static_surface_only() -> anyhow::Result<()> {
    let manifest = manifest::read_manifest(PathBuf::from(RENDERER_MANIFEST)).await?;
    anyhow::ensure!(matches!(
        manifest.entry.kind,
        PackageEntry::SurfaceBundle { .. }
    ));
    let runtime = runtime_with(&[RENDERER_MANIFEST]).await?;
    let contributions = runtime.list_surface_contributions(None).await;
    let encoded = serde_json::to_string(&contributions)?;
    anyhow::ensure!(encoded.contains("plurora/modular-simulation-renderer/play"));
    anyhow::ensure!(encoded.contains("plurora/modular-simulation-renderer/inspector"));
    let package_bundle = std::fs::read(RENDERER_BUNDLE)?;
    let work_bundle = std::fs::read(WORK_RENDERER_BUNDLE)?;
    anyhow::ensure!(!package_bundle.is_empty() && package_bundle == work_bundle);
    Ok(())
}

pub(crate) async fn community_replacement_has_no_publisher_priority() -> anyhow::Result<()> {
    let runtime = runtime_with(&[SIMULATION_MANIFEST, COMMUNITY_MANIFEST]).await?;
    let ambiguous = invoke(&runtime, None, "create_state", json!({}))
        .await
        .expect_err("two compatible providers require explicit selection");
    anyhow::ensure!(ambiguous.to_string().contains("ambiguous"));

    let first_party = invoke(
        &runtime,
        Some(FIRST_PARTY_PACKAGE),
        "create_state",
        json!({"title": "Equal Providers"}),
    )
    .await?;
    let community = invoke(
        &runtime,
        Some(COMMUNITY_PACKAGE),
        "create_state",
        json!({"title": "Equal Providers"}),
    )
    .await?;
    anyhow::ensure!(first_party["state"] == community["state"]);
    anyhow::ensure!(community["package_id"] == COMMUNITY_PACKAGE);
    anyhow::ensure!(
        work::check_work_path(Path::new(COMMUNITY_WORK))?
            .nodes
            .len()
            == 1
    );
    Ok(())
}

pub(crate) async fn promotion_candidate_is_not_persisted_or_published() -> anyhow::Result<()> {
    let output = work::promote_work_path(
        Path::new(WORK_KIT),
        &["simulation".to_string()],
        "example/modular-simulation-core",
    )?;
    anyhow::ensure!(output.candidate.selected_nodes == vec![NodeId::parse("simulation")?]);
    anyhow::ensure!(
        output.nested_assembly.assembly_id == AssemblyId::parse("example/modular-simulation-core")?
    );
    anyhow::ensure!(output.candidate.boundary_ports.len() == 5);
    anyhow::ensure!(output
        .candidate
        .state_slot_ids
        .iter()
        .any(|slot| slot.as_str() == "save"));
    anyhow::ensure!(output
        .candidate_artifact
        .descriptor
        .digest
        .starts_with("sha256:"));
    anyhow::ensure!(output
        .candidate
        .diagnostics
        .iter()
        .all(|diagnostic| !diagnostic.message.contains("published")));
    Ok(())
}

pub(crate) async fn fork_intent_and_state_migration_are_explicit() -> anyhow::Result<()> {
    let server = work::materialize_work_revision(Path::new(SERVER_WORK))?;
    anyhow::ensure!(server.work_id.as_str() == "example/modular-simulation-server");
    anyhow::ensure!(server.operational_intent.is_some());
    anyhow::ensure!(matches!(
        server.entrypoints[0].target,
        WorkEntrypointTarget::AssemblyPort { ref port_id } if port_id.as_str() == "server"
    ));

    let runtime = runtime_with(&[SIMULATION_MANIFEST]).await?;
    let migrated = invoke(
        &runtime,
        Some(FIRST_PARTY_PACKAGE),
        "migrate_save",
        json!({
            "save": {
                "schema": "modular-simulation.save.v0",
                "title": "Legacy Colony",
                "round": 7,
                "energy": 9,
                "food": 8,
                "materials": 7,
                "structures": [{"kind": "generator", "x": 0, "y": 0}]
            }
        }),
    )
    .await?;
    anyhow::ensure!(migrated["migrated"] == true);
    anyhow::ensure!(migrated["state"]["schema"] == "modular-simulation.save.v1");
    anyhow::ensure!(migrated["state"]["turn"] == 7);
    Ok(())
}

pub(crate) fn modular_simulation_cases() -> Vec<super::runner::ConformanceCase> {
    macro_rules! case {
        ($id:expr, [$($tag:expr),*], $func:path) => {
            super::registry::case($id, &[$($tag),*], || Box::pin($func()))
        };
    }
    vec![
        case!(
            "modular_simulation.work_kit_materializes",
            ["modular_simulation", "work", "assembly", "portability"],
            work_kit_materializes_as_portable_closure
        ),
        case!(
            "modular_simulation.reducer_and_portable_save",
            ["modular_simulation", "runtime", "state", "portability"],
            deterministic_reducer_and_portable_save
        ),
        case!(
            "modular_simulation.optional_ai_no_ambient_provider",
            ["modular_simulation", "powerbox", "binding", "negative"],
            optional_ai_has_no_ambient_provider
        ),
        case!(
            "modular_simulation.renderer_static_surface",
            ["modular_simulation", "surface", "web"],
            renderer_is_static_surface_only
        ),
        case!(
            "modular_simulation.community_replacement_no_priority",
            [
                "modular_simulation",
                "replacement",
                "third_party",
                "negative"
            ],
            community_replacement_has_no_publisher_priority
        ),
        case!(
            "modular_simulation.promotion_candidate_no_publish",
            ["modular_simulation", "promotion", "agent", "no_effect"],
            promotion_candidate_is_not_persisted_or_published
        ),
        case!(
            "modular_simulation.fork_and_state_migration",
            [
                "modular_simulation",
                "fork",
                "state",
                "migration",
                "realization"
            ],
            fork_intent_and_state_migration_are_explicit
        ),
    ]
}
