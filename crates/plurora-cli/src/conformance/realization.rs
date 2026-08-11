//! Phase 6 public Realization contract vectors.
//!
//! Stateful planner/executor/recovery tests live with the production service
//! registry. These cases lock the public method, event, authority, approval,
//! and replay-safe wire boundary.

use std::collections::BTreeSet;
use std::sync::Arc;

use plurora_core::{
    EVENT_REALIZATION_ACTIVE, EVENT_REALIZATION_APPLYING, EVENT_REALIZATION_FAILED,
    EVENT_REALIZATION_PLANNED, EVENT_REALIZATION_RECONCILED, EVENT_REALIZATION_ROLLED_BACK,
    EVENT_REALIZATION_STOPPED, PLATFORM_EVENT_KINDS,
};
use plurora_runtime::{ContractOwnerLayer, MethodStatus, PlatformMethod};
use plurora_runtime::{
    InMemoryEventStore, ProtocolContext, ProtocolResourceSelector, Runtime, RuntimeConfig,
};
use serde_json::{json, Value};

use crate::schema_export::{event_schema, event_schemas, method_schema, method_schemas};

const METHODS: [PlatformMethod; 7] = [
    PlatformMethod::RealizationPlan,
    PlatformMethod::RealizationApply,
    PlatformMethod::RealizationGet,
    PlatformMethod::RealizationList,
    PlatformMethod::RealizationStop,
    PlatformMethod::RealizationRollback,
    PlatformMethod::RealizationReconcile,
];

const EVENTS: [&str; 7] = [
    EVENT_REALIZATION_PLANNED,
    EVENT_REALIZATION_APPLYING,
    EVENT_REALIZATION_ACTIVE,
    EVENT_REALIZATION_STOPPED,
    EVENT_REALIZATION_FAILED,
    EVENT_REALIZATION_ROLLED_BACK,
    EVENT_REALIZATION_RECONCILED,
];

async fn method_identity_owner_status_and_typed_dtos() -> anyhow::Result<()> {
    let expected = BTreeSet::from([
        "host.realization.plan",
        "host.realization.apply",
        "host.realization.get",
        "host.realization.list",
        "host.realization.stop",
        "host.realization.rollback",
        "host.realization.reconcile",
    ]);
    anyhow::ensure!(
        METHODS
            .iter()
            .map(PlatformMethod::id)
            .collect::<BTreeSet<_>>()
            == expected
    );
    let exported = method_schemas();
    anyhow::ensure!(exported.len() == 99);
    for method in METHODS {
        anyhow::ensure!(method.status() == MethodStatus::Implemented);
        anyhow::ensure!(method.is_dispatched() && !method.streaming());
        anyhow::ensure!(method.contract().owner_layer == ContractOwnerLayer::Host);
        let (_, params, result) = exported
            .iter()
            .find(|(candidate, _, _)| *candidate == method)
            .ok_or_else(|| anyhow::anyhow!("{} is missing from schema export", method.id()))?;
        let document = method_schema(method, params.clone(), result.clone());
        anyhow::ensure!(document.pointer("/$defs/Params").is_some());
        anyhow::ensure!(document.pointer("/$defs/Result").is_some());
        anyhow::ensure!(
            document.pointer("/x-plurora-contract/owner_layer")
                == Some(&Value::String("host".into()))
        );
        anyhow::ensure!(
            document.pointer("/x-plurora-contract/implementation_status")
                == Some(&Value::String("implemented".into()))
        );
    }
    Ok(())
}

async fn methods_fail_before_controller_without_public_action() -> anyhow::Result<()> {
    let runtime = Runtime::new(
        Arc::new(InMemoryEventStore::default()),
        RuntimeConfig::default(),
    );
    let installation = "11111111-1111-4111-8111-111111111111";
    let realization = "22222222-2222-4222-8222-222222222222";
    let historic = "33333333-3333-4333-8333-333333333333";
    let denied = ProtocolContext::host_device(
        "realization-action-denied",
        Vec::new(),
        [
            ("installation", installation),
            ("target", "local"),
            ("realization", realization),
            ("realization", historic),
        ]
        .into_iter()
        .map(|(kind, id)| ProtocolResourceSelector {
            owner: "host".into(),
            kind: kind.into(),
            id: Some(id.into()),
        })
        .collect(),
        Vec::new(),
        "conformance",
    );
    let descriptor = json!({
        "artifact_type_uri": "urn:plurora:realization-plan:v1",
        "media_type": "application/json",
        "digest": format!("sha256:{}", "a".repeat(64)),
        "size_bytes": 1,
    });
    let approval = json!({
        "plan_digest": descriptor["digest"],
        "decision": "approved",
        "accepted_risks": [],
        "decided_at": "2026-08-12T00:00:00Z",
    });
    let vectors = [
        ("host.realization.list", "observe", json!({})),
        (
            "host.realization.get",
            "observe",
            json!({"installation_id": installation, "realization_id": realization}),
        ),
        (
            "host.realization.plan",
            "realization.plan",
            json!({
                "installation_id": installation,
                "expected_installation_revision": 1,
                "target_id": "local",
                "backends": [{
                    "kind": "oci_image",
                    "workload_id": "server",
                    "execution_class": "oci-container.v1",
                    "image": format!("example/app@sha256:{}", "b".repeat(64)),
                    "container_port": 8080,
                    "port_name": "http",
                    "route_id": "server-http",
                }],
                "idempotency_key": "realization-plan-action",
            }),
        ),
        (
            "host.realization.apply",
            "realization.apply",
            json!({"installation_id": installation, "target_id": "local", "realization_id": realization, "expected_revision": 1, "plan_ref": descriptor, "approval": approval, "idempotency_key": "realization-apply-action"}),
        ),
        (
            "host.realization.stop",
            "realization.apply",
            json!({"installation_id": installation, "target_id": "local", "realization_id": realization, "expected_revision": 2, "idempotency_key": "realization-stop-action"}),
        ),
        (
            "host.realization.rollback",
            "realization.apply",
            json!({"installation_id": installation, "target_id": "local", "realization_id": realization, "expected_revision": 2, "rollback_to_realization_id": historic, "approval": approval, "idempotency_key": "realization-rollback-action"}),
        ),
        (
            "host.realization.reconcile",
            "realization.apply",
            json!({"installation_id": installation, "target_id": "local", "realization_id": realization, "expected_revision": 2, "idempotency_key": "realization-reconcile-action"}),
        ),
    ];
    for (method, action, params) in vectors {
        let error = runtime
            .call_protocol(&denied, method, params)
            .await
            .expect_err("missing action must fail before controller access");
        anyhow::ensure!(
            error.message.contains(action),
            "{method} did not require {action}: {}",
            error.message
        );
    }
    Ok(())
}

async fn event_identity_and_public_payload_schema() -> anyhow::Result<()> {
    let exported = event_schemas();
    anyhow::ensure!(exported.len() == 76);
    for kind in EVENTS {
        anyhow::ensure!(PLATFORM_EVENT_KINDS.contains(&kind));
        let (_, payload) = exported
            .iter()
            .find(|(candidate, _)| *candidate == kind)
            .ok_or_else(|| anyhow::anyhow!("{kind} is missing from schema export"))?;
        let document = event_schema(kind, payload.clone());
        anyhow::ensure!(document
            .pointer("/$defs/Payload/properties/realization")
            .is_some());
        anyhow::ensure!(document
            .pointer("/$defs/Payload/properties/target_id")
            .is_some());
        anyhow::ensure!(
            document.pointer("/properties/kind/const") == Some(&Value::String(kind.into()))
        );
    }
    Ok(())
}

async fn plan_is_effect_free_and_apply_binds_exact_approval() -> anyhow::Result<()> {
    let exported = method_schemas();
    let (_, plan_params, plan_result) = exported
        .iter()
        .find(|(method, _, _)| *method == PlatformMethod::RealizationPlan)
        .ok_or_else(|| anyhow::anyhow!("realization plan schema is missing"))?;
    let plan_params = serde_json::to_string(plan_params)?;
    let plan_result_text = serde_json::to_string(plan_result)?;
    anyhow::ensure!(plan_params.contains("expected_installation_revision"));
    anyhow::ensure!(plan_params.contains("target_id") && plan_params.contains("backends"));
    anyhow::ensure!(!plan_params.contains("approval"));
    anyhow::ensure!(plan_result_text.contains("plan_ref") && plan_result_text.contains("gaps"));
    anyhow::ensure!(
        plan_result
            .pointer("/properties/actual_resources")
            .is_none(),
        "plan result must not expose effect-owned resources at its top level"
    );

    let (_, apply_params, _) = exported
        .iter()
        .find(|(method, _, _)| *method == PlatformMethod::RealizationApply)
        .ok_or_else(|| anyhow::anyhow!("realization apply schema is missing"))?;
    let apply = serde_json::to_string(apply_params)?;
    for field in [
        "plan_ref",
        "plan_digest",
        "decision",
        "accepted_risks",
        "decided_at",
        "expected_revision",
        "idempotency_key",
    ] {
        anyhow::ensure!(apply.contains(field), "apply omitted {field}");
    }
    Ok(())
}

async fn rollback_wire_replays_persisted_plan_without_workspace() -> anyhow::Result<()> {
    let exported = method_schemas();
    let (_, params, result) = exported
        .iter()
        .find(|(method, _, _)| *method == PlatformMethod::RealizationRollback)
        .ok_or_else(|| anyhow::anyhow!("realization rollback schema is missing"))?;
    let wire = format!(
        "{}{}",
        serde_json::to_string(params)?,
        serde_json::to_string(result)?
    );
    for required in [
        "realization_id",
        "rollback_to_realization_id",
        "expected_revision",
        "approval",
        "idempotency_key",
    ] {
        anyhow::ensure!(wire.contains(required), "rollback omitted {required}");
    }
    for forbidden in [
        "workspace_path",
        "source_url",
        "git_ref",
        "raw_secret",
        "stderr",
    ] {
        anyhow::ensure!(
            !wire.contains(forbidden),
            "rollback leaked or required {forbidden}"
        );
    }
    Ok(())
}

pub(crate) fn realization_cases() -> Vec<super::runner::ConformanceCase> {
    macro_rules! case {
        ($id:expr, [$($tag:expr),*], $func:path) => {
            super::registry::case($id, &[$($tag),*], || Box::pin($func()))
        };
    }
    vec![
        case!(
            "realization.public_method_identity_owner_typed_dto",
            ["phase6", "protocol", "realization", "contract"],
            method_identity_owner_status_and_typed_dtos
        ),
        case!(
            "realization.public_actions_no_effect_on_denial",
            [
                "phase6",
                "protocol",
                "realization",
                "authority",
                "no_effect"
            ],
            methods_fail_before_controller_without_public_action
        ),
        case!(
            "realization.public_event_identity_payload",
            ["phase6", "protocol", "realization", "event"],
            event_identity_and_public_payload_schema
        ),
        case!(
            "realization.plan_effect_free_apply_exact_approval",
            ["phase6", "realization", "plan", "approval", "no_effect"],
            plan_is_effect_free_and_apply_binds_exact_approval
        ),
        case!(
            "realization.rollback_persisted_plan_no_workspace",
            ["phase6", "realization", "rollback", "replay", "negative"],
            rollback_wire_replays_persisted_plan_without_workspace
        ),
    ]
}
