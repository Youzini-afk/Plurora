//! Deterministic, state-rich reference simulation used by the modular Work kit.
//!
//! State is explicit input/output. The package owns no process-global game
//! state, performs no network access, and can use an AI adviser only through an
//! exact Runtime-phase consumer Port selected by the Host Powerbox.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{safety, InprocInvocation};
use crate::{invoke_capability_from_inproc_port, CapabilityInvocationRequest};
use plurora_work::PortId;

const FIRST_PARTY_PACKAGE_ID: &str = "plurora/modular-simulation";
const COMMUNITY_PACKAGE_ID: &str = "community/modular-simulation";
const ADVISOR_PACKAGE_ID: &str = "example/ai-advisor";
const STATE_SCHEMA: &str = "modular-simulation.save.v1";
const LEGACY_STATE_SCHEMA: &str = "modular-simulation.save.v0";
const BOARD_WIDTH: u8 = 5;
const BOARD_HEIGHT: u8 = 5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SimulationCell {
    x: u8,
    y: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    structure: Option<String>,
    #[serde(default)]
    workers: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SimulationState {
    schema: String,
    revision: u64,
    turn: u64,
    title: String,
    population: u32,
    resources: BTreeMap<String, i64>,
    cells: Vec<SimulationCell>,
    #[serde(default)]
    objectives: Vec<String>,
    #[serde(default)]
    log: Vec<String>,
}

pub async fn try_handle(request: &InprocInvocation) -> Option<anyhow::Result<Value>> {
    if !supports_provider(&request.provider_package_id) {
        return None;
    }
    if safety::contains_raw_secret(&request.input) {
        return Some(Ok(rejected(request)));
    }
    let capability = request.capability_id.rsplit('/').next().unwrap_or_default();
    let result = match capability {
        "describe_contract" => describe_contract(request),
        "create_state" => create_state(request),
        "apply_input" => apply_input(request),
        "render_state" => render_state(request),
        "export_save" => export_save(request),
        "migrate_save" => migrate_save(request),
        "inspect_state" => inspect_state(request),
        "request_ai_move" => request_ai_move(request).await,
        "server_tick" => server_tick(request),
        "suggest" => suggest_move(request),
        _ => return None,
    };
    Some(result)
}

fn supports_provider(package_id: &str) -> bool {
    matches!(
        package_id,
        FIRST_PARTY_PACKAGE_ID | COMMUNITY_PACKAGE_ID | ADVISOR_PACKAGE_ID
    )
}

fn rejected(request: &InprocInvocation) -> Value {
    serde_json::json!({
        "kind": "modular_simulation_input_rejected",
        "reason_code": "raw_secret",
        "next_step": "replace credential material with a Host-managed secret reference",
        "package_id": request.provider_package_id,
        "inference_performed": false,
        "network_performed": false,
    })
}

fn describe_contract(request: &InprocInvocation) -> anyhow::Result<Value> {
    Ok(serde_json::json!({
        "kind": "modular_simulation_contract",
        "package_id": request.provider_package_id,
        "state_schema": STATE_SCHEMA,
        "board": {"width": BOARD_WIDTH, "height": BOARD_HEIGHT},
        "structures": ["farm", "generator", "habitat"],
        "actions": ["build", "assign", "advance"],
        "ports": {
            "input": "apply_input",
            "portable_save": "export_save",
            "migration": "migrate_save",
            "optional_ai_import": "ai-advisor",
            "server": "server_tick"
        },
        "state_is_explicit": true,
        "inference_performed": false,
        "network_performed": false,
    }))
}

fn create_state(request: &InprocInvocation) -> anyhow::Result<Value> {
    let title = request
        .input
        .get("title")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Tidelight Colony");
    let objective = request
        .input
        .get("objective")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("reach population 12 while keeping food and energy non-negative");
    let state = initial_state(title, objective);
    Ok(state_result(
        "modular_simulation_state_created",
        request,
        state,
    ))
}

fn initial_state(title: &str, objective: &str) -> SimulationState {
    let mut cells = Vec::new();
    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            cells.push(SimulationCell {
                x,
                y,
                structure: None,
                workers: 0,
            });
        }
    }
    SimulationState {
        schema: STATE_SCHEMA.to_string(),
        revision: 0,
        turn: 0,
        title: title.to_string(),
        population: 4,
        resources: BTreeMap::from([
            ("energy".to_string(), 12),
            ("food".to_string(), 10),
            ("materials".to_string(), 8),
        ]),
        cells,
        objectives: vec![objective.to_string()],
        log: vec!["simulation initialized".to_string()],
    }
}

fn apply_input(request: &InprocInvocation) -> anyhow::Result<Value> {
    let mut state = state_from_input(&request.input)?;
    let action = request
        .input
        .get("action")
        .ok_or_else(|| anyhow::anyhow!("simulation input requires an action"))?;
    let action_kind = action
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("simulation action kind is missing"))?;
    match action_kind {
        "build" => apply_build(&mut state, action)?,
        "assign" => apply_assign(&mut state, action)?,
        "advance" => apply_advance(&mut state),
        _ => anyhow::bail!("simulation action kind is unsupported"),
    }
    state.revision = state.revision.saturating_add(1);
    validate_state(&state)?;
    Ok(state_result(
        "modular_simulation_input_applied",
        request,
        state,
    ))
}

fn apply_build(state: &mut SimulationState, action: &Value) -> anyhow::Result<()> {
    let (x, y) = action_coordinate(action)?;
    let structure = action
        .get("structure")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("build action requires a structure"))?;
    let cost = match structure {
        "farm" => 2,
        "generator" => 3,
        "habitat" => 4,
        _ => anyhow::bail!("build action structure is unsupported"),
    };
    let cell = state
        .cells
        .iter_mut()
        .find(|cell| cell.x == x && cell.y == y)
        .ok_or_else(|| anyhow::anyhow!("build action coordinate is outside the board"))?;
    anyhow::ensure!(cell.structure.is_none(), "build action cell is occupied");
    let materials = state.resources.entry("materials".to_string()).or_default();
    anyhow::ensure!(*materials >= cost, "build action lacks materials");
    *materials -= cost;
    cell.structure = Some(structure.to_string());
    if structure == "habitat" {
        state.population = state.population.saturating_add(2);
    }
    state.log.push(format!("built {structure} at {x},{y}"));
    Ok(())
}

fn apply_assign(state: &mut SimulationState, action: &Value) -> anyhow::Result<()> {
    let (x, y) = action_coordinate(action)?;
    let workers = action
        .get("workers")
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .ok_or_else(|| anyhow::anyhow!("assign action requires a worker count"))?;
    let assigned_elsewhere = state
        .cells
        .iter()
        .filter(|cell| !(cell.x == x && cell.y == y))
        .map(|cell| u32::from(cell.workers))
        .sum::<u32>();
    anyhow::ensure!(
        assigned_elsewhere.saturating_add(u32::from(workers)) <= state.population,
        "assign action exceeds the available population"
    );
    let cell = state
        .cells
        .iter_mut()
        .find(|cell| cell.x == x && cell.y == y)
        .ok_or_else(|| anyhow::anyhow!("assign action coordinate is outside the board"))?;
    anyhow::ensure!(
        cell.structure.is_some(),
        "assign action cell has no structure"
    );
    cell.workers = workers;
    state
        .log
        .push(format!("assigned {workers} workers at {x},{y}"));
    Ok(())
}

fn apply_advance(state: &mut SimulationState) {
    let mut energy_delta = -(i64::from(state.population) / 2);
    let mut food_delta = -i64::from(state.population);
    let mut materials_delta = 1;
    for cell in &state.cells {
        let workers = i64::from(cell.workers);
        match cell.structure.as_deref() {
            Some("generator") => energy_delta += 2 + workers * 2,
            Some("farm") => food_delta += 2 + workers * 2,
            Some("habitat") => materials_delta += workers,
            _ => {}
        }
    }
    *state.resources.entry("energy".to_string()).or_default() += energy_delta;
    *state.resources.entry("food".to_string()).or_default() += food_delta;
    *state.resources.entry("materials".to_string()).or_default() += materials_delta;
    state.turn = state.turn.saturating_add(1);
    state.log.push(format!(
        "advanced turn {} (energy {energy_delta:+}, food {food_delta:+}, materials {materials_delta:+})",
        state.turn
    ));
}

fn action_coordinate(action: &Value) -> anyhow::Result<(u8, u8)> {
    let x = action
        .get("x")
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .ok_or_else(|| anyhow::anyhow!("simulation action x coordinate is missing"))?;
    let y = action
        .get("y")
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .ok_or_else(|| anyhow::anyhow!("simulation action y coordinate is missing"))?;
    anyhow::ensure!(
        x < BOARD_WIDTH && y < BOARD_HEIGHT,
        "simulation action coordinate is outside the board"
    );
    Ok((x, y))
}

fn render_state(request: &InprocInvocation) -> anyhow::Result<Value> {
    let state = state_from_input(&request.input)?;
    let status = simulation_status(&state);
    Ok(serde_json::json!({
        "kind": "modular_simulation_render_state",
        "package_id": request.provider_package_id,
        "state": state,
        "status": status,
        "board": {"width": BOARD_WIDTH, "height": BOARD_HEIGHT},
        "legend": {
            "farm": "food production",
            "generator": "energy production",
            "habitat": "population and material support"
        },
        "inference_performed": false,
        "network_performed": false,
    }))
}

fn export_save(request: &InprocInvocation) -> anyhow::Result<Value> {
    let state = state_from_input(&request.input)?;
    let bytes = plurora_work::canonical_json_bytes(&state)
        .map_err(|error| anyhow::anyhow!(error.code.as_str()))?;
    let digest = plurora_core::world_bundle_sha256_digest(&bytes);
    Ok(serde_json::json!({
        "kind": "modular_simulation_portable_save",
        "package_id": request.provider_package_id,
        "schema": STATE_SCHEMA,
        "digest": digest,
        "state": state,
        "portable": true,
        "inference_performed": false,
        "network_performed": false,
    }))
}

fn migrate_save(request: &InprocInvocation) -> anyhow::Result<Value> {
    let value = request.input.get("save").unwrap_or(&request.input);
    if value.get("schema").and_then(Value::as_str) == Some(STATE_SCHEMA) {
        let state: SimulationState = serde_json::from_value(value.clone())?;
        validate_state(&state)?;
        return Ok(serde_json::json!({
            "kind": "modular_simulation_save_migrated",
            "package_id": request.provider_package_id,
            "from_schema": STATE_SCHEMA,
            "to_schema": STATE_SCHEMA,
            "migrated": false,
            "state": state,
        }));
    }
    anyhow::ensure!(
        value.get("schema").and_then(Value::as_str) == Some(LEGACY_STATE_SCHEMA),
        "save migration supports only modular-simulation v0 or v1"
    );
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Migrated Colony");
    let mut state = initial_state(
        title,
        "review the migrated colony and choose a new objective",
    );
    state.turn = value.get("round").and_then(Value::as_u64).unwrap_or(0);
    for key in ["energy", "food", "materials"] {
        if let Some(amount) = value.get(key).and_then(Value::as_i64) {
            state.resources.insert(key.to_string(), amount);
        }
    }
    if let Some(structures) = value.get("structures").and_then(Value::as_array) {
        for structure in structures {
            let Some(kind) = structure.get("kind").and_then(Value::as_str) else {
                continue;
            };
            let Some(x) = structure
                .get("x")
                .and_then(Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            else {
                continue;
            };
            let Some(y) = structure
                .get("y")
                .and_then(Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            else {
                continue;
            };
            if x < BOARD_WIDTH
                && y < BOARD_HEIGHT
                && matches!(kind, "farm" | "generator" | "habitat")
            {
                if let Some(cell) = state
                    .cells
                    .iter_mut()
                    .find(|cell| cell.x == x && cell.y == y)
                {
                    cell.structure = Some(kind.to_string());
                }
            }
        }
    }
    state.revision = 1;
    state.log.push("migrated portable save from v0".to_string());
    validate_state(&state)?;
    Ok(serde_json::json!({
        "kind": "modular_simulation_save_migrated",
        "package_id": request.provider_package_id,
        "from_schema": LEGACY_STATE_SCHEMA,
        "to_schema": STATE_SCHEMA,
        "migrated": true,
        "state": state,
    }))
}

fn inspect_state(request: &InprocInvocation) -> anyhow::Result<Value> {
    let state = state_from_input(&request.input)?;
    let occupied = state
        .cells
        .iter()
        .filter(|cell| cell.structure.is_some())
        .count();
    let assigned = state
        .cells
        .iter()
        .map(|cell| u32::from(cell.workers))
        .sum::<u32>();
    Ok(serde_json::json!({
        "kind": "modular_simulation_inspection",
        "package_id": request.provider_package_id,
        "schema": state.schema,
        "revision": state.revision,
        "turn": state.turn,
        "status": simulation_status(&state),
        "population": state.population,
        "assigned_workers": assigned,
        "occupied_cells": occupied,
        "available_cells": usize::from(BOARD_WIDTH) * usize::from(BOARD_HEIGHT) - occupied,
        "resources": state.resources,
        "objectives": state.objectives,
        "inference_performed": false,
        "network_performed": false,
    }))
}

async fn request_ai_move(request: &InprocInvocation) -> anyhow::Result<Value> {
    let state = state_from_input(&request.input)?;
    let consumer_port =
        PortId::parse("ai-advisor").map_err(|error| anyhow::anyhow!(error.code.as_str()))?;
    let invocation = CapabilityInvocationRequest {
        handle: None,
        capability_id: Some("example/ai-advisor/suggest".to_string()),
        caller_package_id: None,
        provider_package_id: None,
        version: Some("^1.0".to_string()),
        session_id: request.session_id.clone(),
        input: serde_json::json!({
            "state": state,
            "allowed_actions": ["build", "assign", "advance"],
        }),
    };
    match invoke_capability_from_inproc_port(&consumer_port, invocation).await {
        Ok(result) => {
            let inference_performed = result
                .output
                .get("inference_performed")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let network_performed = result
                .output
                .get("network_performed")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Ok(serde_json::json!({
                "kind": "modular_simulation_ai_move",
                "available": true,
                "provider_package_id": result.provider_package_id,
                "provider_component_id": result.provider_component_id,
                "provider_component_digest": result.provider_component_digest,
                "provider_behavior_digest": result.provider_behavior_digest,
                "suggestion": result.output,
                "inference_performed": inference_performed,
                "network_performed": network_performed,
            }))
        }
        Err(_) => Ok(serde_json::json!({
            "kind": "modular_simulation_ai_move_unavailable",
            "available": false,
            "reason_code": "binding_unavailable",
            "next_step": "select an explicit compatible provider for the ai-advisor Runtime Port",
            "inference_performed": false,
            "network_performed": false,
        })),
    }
}

fn suggest_move(request: &InprocInvocation) -> anyhow::Result<Value> {
    let state = state_from_input(&request.input)?;
    let empty = state.cells.iter().find(|cell| cell.structure.is_none());
    let materials = state
        .resources
        .get("materials")
        .copied()
        .unwrap_or_default();
    let food = state.resources.get("food").copied().unwrap_or_default();
    let action = if food < i64::from(state.population) * 2 && materials >= 2 {
        empty
            .map(|cell| {
                serde_json::json!({
                    "kind": "build",
                    "x": cell.x,
                    "y": cell.y,
                    "structure": "farm"
                })
            })
            .unwrap_or_else(|| serde_json::json!({"kind": "advance"}))
    } else {
        serde_json::json!({"kind": "advance"})
    };
    Ok(serde_json::json!({
        "kind": "modular_simulation_advice",
        "provider_package_id": request.provider_package_id,
        "policy": "deterministic_local_v1",
        "action": action,
        "reason": "preserve food resilience before expanding the colony",
        "inference_performed": false,
        "network_performed": false,
    }))
}

fn server_tick(request: &InprocInvocation) -> anyhow::Result<Value> {
    let mut state = state_from_input(&request.input)?;
    apply_advance(&mut state);
    state.revision = state.revision.saturating_add(1);
    validate_state(&state)?;
    Ok(serde_json::json!({
        "kind": "modular_simulation_server_tick",
        "package_id": request.provider_package_id,
        "state": state,
        "status": simulation_status(&state),
        "authoritative": true,
        "inference_performed": false,
        "network_performed": false,
    }))
}

fn state_from_input(input: &Value) -> anyhow::Result<SimulationState> {
    let value = input.get("state").unwrap_or(input);
    let state: SimulationState = serde_json::from_value(value.clone())
        .context("simulation state does not match the portable schema")?;
    validate_state(&state)?;
    Ok(state)
}

fn validate_state(state: &SimulationState) -> anyhow::Result<()> {
    anyhow::ensure!(
        state.schema == STATE_SCHEMA,
        "simulation state schema is unsupported"
    );
    anyhow::ensure!(!state.title.trim().is_empty(), "simulation title is empty");
    let resource_keys = state
        .resources
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        resource_keys == BTreeSet::from(["energy", "food", "materials"]),
        "simulation resources are incomplete"
    );
    anyhow::ensure!(
        state.cells.len() == usize::from(BOARD_WIDTH) * usize::from(BOARD_HEIGHT),
        "simulation board cell set is incomplete"
    );
    let mut coordinates = BTreeSet::new();
    let mut assigned = 0_u32;
    for cell in &state.cells {
        anyhow::ensure!(
            cell.x < BOARD_WIDTH && cell.y < BOARD_HEIGHT && coordinates.insert((cell.x, cell.y)),
            "simulation board contains an invalid or duplicate coordinate"
        );
        anyhow::ensure!(
            cell.structure
                .as_deref()
                .is_none_or(|value| matches!(value, "farm" | "generator" | "habitat")),
            "simulation board contains an unsupported structure"
        );
        anyhow::ensure!(
            cell.structure.is_some() || cell.workers == 0,
            "simulation workers require a structure"
        );
        assigned = assigned.saturating_add(u32::from(cell.workers));
    }
    anyhow::ensure!(
        assigned <= state.population,
        "simulation assigned workers exceed population"
    );
    Ok(())
}

fn simulation_status(state: &SimulationState) -> &'static str {
    if state.resources.values().any(|value| *value < 0) {
        "strained"
    } else if state.population >= 12 {
        "objective_reached"
    } else {
        "stable"
    }
}

fn state_result(kind: &str, request: &InprocInvocation, state: SimulationState) -> Value {
    serde_json::json!({
        "kind": kind,
        "package_id": request.provider_package_id,
        "status": simulation_status(&state),
        "state": state,
        "inference_performed": false,
        "network_performed": false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(capability: &str, input: Value) -> InprocInvocation {
        InprocInvocation {
            capability_id: format!("{FIRST_PARTY_PACKAGE_ID}/{capability}"),
            provider_package_id: FIRST_PARTY_PACKAGE_ID.to_string(),
            session_id: None,
            input,
        }
    }

    async fn invoke(capability: &str, input: Value) -> Value {
        try_handle(&request(capability, input))
            .await
            .expect("known capability")
            .expect("capability succeeds")
    }

    #[tokio::test]
    async fn reducer_build_assign_advance_and_save_are_deterministic() {
        let created = invoke("create_state", json!({"title": "Test Colony"})).await;
        let built = invoke(
            "apply_input",
            json!({
                "state": created["state"],
                "action": {"kind": "build", "x": 1, "y": 2, "structure": "farm"}
            }),
        )
        .await;
        let assigned = invoke(
            "apply_input",
            json!({
                "state": built["state"],
                "action": {"kind": "assign", "x": 1, "y": 2, "workers": 2}
            }),
        )
        .await;
        let advanced = invoke(
            "apply_input",
            json!({"state": assigned["state"], "action": {"kind": "advance"}}),
        )
        .await;
        assert_eq!(advanced["state"]["turn"], json!(1));
        assert_eq!(advanced["state"]["resources"]["food"], json!(12));
        let first = invoke("export_save", json!({"state": advanced["state"]})).await;
        let second = invoke("export_save", json!({"state": advanced["state"]})).await;
        assert_eq!(first["digest"], second["digest"]);
        assert_eq!(first["portable"], json!(true));
    }

    #[tokio::test]
    async fn legacy_save_migrates_and_current_save_is_idempotent() {
        let migrated = invoke(
            "migrate_save",
            json!({
                "save": {
                    "schema": LEGACY_STATE_SCHEMA,
                    "title": "Old Colony",
                    "round": 7,
                    "energy": 9,
                    "food": 8,
                    "materials": 6,
                    "structures": [{"kind": "generator", "x": 0, "y": 1}]
                }
            }),
        )
        .await;
        assert_eq!(migrated["migrated"], json!(true));
        assert_eq!(migrated["state"]["turn"], json!(7));
        let replay = invoke("migrate_save", json!({"save": migrated["state"]})).await;
        assert_eq!(replay["migrated"], json!(false));
        assert_eq!(replay["state"], migrated["state"]);
    }

    #[tokio::test]
    async fn optional_ai_is_unavailable_without_an_explicit_run_binding() {
        let created = invoke("create_state", json!({})).await;
        let result = invoke("request_ai_move", json!({"state": created["state"]})).await;
        assert_eq!(result["available"], json!(false));
        assert_eq!(result["reason_code"], json!("binding_unavailable"));
        assert_eq!(result["inference_performed"], json!(false));
    }

    #[tokio::test]
    async fn community_provider_uses_the_same_behavior_without_first_party_dispatch() {
        let mut value = request("create_state", json!({"title": "Community Colony"}));
        value.provider_package_id = COMMUNITY_PACKAGE_ID.to_string();
        let output = try_handle(&value).await.unwrap().unwrap();
        assert_eq!(output["package_id"], json!(COMMUNITY_PACKAGE_ID));
        assert_eq!(output["state"]["title"], json!("Community Colony"));
    }

    #[tokio::test]
    async fn optional_advisor_is_an_ordinary_deterministic_provider() {
        let created = invoke("create_state", json!({})).await;
        let mut value = request("suggest", json!({"state": created["state"]}));
        value.capability_id = format!("{ADVISOR_PACKAGE_ID}/suggest");
        value.provider_package_id = ADVISOR_PACKAGE_ID.to_string();
        let output = try_handle(&value).await.unwrap().unwrap();
        assert_eq!(output["provider_package_id"], json!(ADVISOR_PACKAGE_ID));
        assert_eq!(output["policy"], json!("deterministic_local_v1"));
        assert_eq!(output["inference_performed"], json!(false));
        assert_eq!(output["network_performed"], json!(false));
    }

    #[tokio::test]
    async fn raw_secret_like_input_is_rejected_without_echo() {
        let secret = "RawSecretExample1234567890abcdefABCDEF123456";
        let output = invoke("create_state", json!({"token": secret})).await;
        assert_eq!(output["reason_code"], json!("raw_secret"));
        assert!(!serde_json::to_string(&output).unwrap().contains(secret));
    }
}
