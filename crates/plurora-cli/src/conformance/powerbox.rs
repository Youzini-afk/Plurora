//! Phase 5 public Exposure/Binding contract vectors.
//!
//! Stateful lifecycle and concurrency vectors live beside the production
//! `PowerboxRegistry`; these registry cases guard the public method/event wire
//! surface without inventing a second sidecar or private DTO family.

use std::collections::BTreeSet;
use std::sync::Arc;

use plurora_core::{
    EVENT_BINDING_EXPIRED, EVENT_BINDING_REVOKED, EVENT_BINDING_SELECTED, EVENT_EXPOSURE_CREATED,
    EVENT_EXPOSURE_EXPIRED, EVENT_EXPOSURE_REVOKED, PLATFORM_EVENT_KINDS,
};
use plurora_runtime::{ContractOwnerLayer, MethodStatus, PlatformMethod};
use plurora_runtime::{
    InMemoryEventStore, ProtocolContext, ProtocolResourceSelector, Runtime, RuntimeConfig,
};
use plurora_work::MAX_PROVIDER_CANDIDATES;
use serde_json::{json, Value};

use crate::schema_export::{event_schema, event_schemas, method_schema, method_schemas};

const METHODS: [PlatformMethod; 7] = [
    PlatformMethod::ExposureList,
    PlatformMethod::ExposureCreate,
    PlatformMethod::ExposureRevoke,
    PlatformMethod::BindingList,
    PlatformMethod::BindingCandidates,
    PlatformMethod::BindingSelect,
    PlatformMethod::BindingRevoke,
];

const EVENTS: [&str; 6] = [
    EVENT_EXPOSURE_CREATED,
    EVENT_EXPOSURE_REVOKED,
    EVENT_EXPOSURE_EXPIRED,
    EVENT_BINDING_SELECTED,
    EVENT_BINDING_REVOKED,
    EVENT_BINDING_EXPIRED,
];

async fn method_identity_owner_status_and_typed_dtos() -> anyhow::Result<()> {
    let expected = BTreeSet::from([
        "host.exposure.list",
        "host.exposure.create",
        "host.exposure.revoke",
        "host.binding.list",
        "host.binding.candidates",
        "host.binding.select",
        "host.binding.revoke",
    ]);
    let actual = METHODS
        .iter()
        .map(PlatformMethod::id)
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(actual == expected);

    let exported = method_schemas();
    anyhow::ensure!(exported.len() == 92);
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

async fn every_method_enforces_its_public_action_before_controller_effects() -> anyhow::Result<()> {
    let runtime = Runtime::new(
        Arc::new(InMemoryEventStore::default()),
        RuntimeConfig::default(),
    );
    let resources = ["installation", "run", "port", "exposure", "binding"]
        .into_iter()
        .map(|kind| ProtocolResourceSelector {
            owner: "host".into(),
            kind: kind.into(),
            id: None,
        })
        .collect();
    let denied = ProtocolContext::host_device(
        "powerbox-action-denied",
        Vec::new(),
        resources,
        Vec::new(),
        "conformance",
    );
    let installation = "11111111-1111-4111-8111-111111111111";
    let provider = "22222222-2222-4222-8222-222222222222";
    let run = "33333333-3333-4333-8333-333333333333";
    let exposure = "44444444-4444-4444-8444-444444444444";
    let binding = "55555555-5555-4555-8555-555555555555";
    let digest = format!("sha256:{}", "a".repeat(64));
    let vectors = [
        ("host.exposure.list", "observe", json!({})),
        (
            "host.exposure.create",
            "exposure.manage",
            json!({
                "installation_id": installation,
                "expected_installation_revision": 1,
                "run_id": run,
                "expected_run_revision": 1,
                "export_port": "save",
                "audience": [{"kind": "installation", "id": installation}],
                "expires_at": "2999-01-01T00:00:00Z",
                "idempotency_key": "exposure-create-action",
            }),
        ),
        (
            "host.exposure.revoke",
            "exposure.manage",
            json!({
                "installation_id": installation,
                "expected_installation_revision": 1,
                "run_id": run,
                "expected_run_revision": 1,
                "export_port": "save",
                "exposure_id": exposure,
                "expected_exposure_revision": 1,
                "idempotency_key": "exposure-revoke-action",
            }),
        ),
        ("host.binding.list", "observe", json!({})),
        (
            "host.binding.candidates",
            "observe",
            json!({
                "consumer_installation_id": installation,
                "expected_consumer_installation_revision": 1,
                "phase": "launch",
                "import_port": "save",
            }),
        ),
        (
            "host.binding.select",
            "binding.manage",
            json!({
                "consumer_installation_id": installation,
                "expected_consumer_installation_revision": 1,
                "phase": "launch",
                "import_port": "save",
                "exposure_id": exposure,
                "expected_exposure_revision": 1,
                "provider_installation_id": provider,
                "expected_provider_installation_revision": 1,
                "candidate_digest": digest,
                "idempotency_key": "binding-select-action",
            }),
        ),
        (
            "host.binding.revoke",
            "binding.manage",
            json!({
                "consumer_installation_id": installation,
                "expected_consumer_installation_revision": 1,
                "import_port": "save",
                "exposure_id": exposure,
                "binding_id": binding,
                "expected_binding_revision": 1,
                "idempotency_key": "binding-revoke-action",
            }),
        ),
    ];

    for (method, action, params) in vectors {
        let error = runtime
            .call_protocol(&denied, method, params)
            .await
            .expect_err("missing public action must fail before controller access");
        anyhow::ensure!(
            error.message.contains(action),
            "{method} did not report its required {action} action: {}",
            error.message
        );
    }
    Ok(())
}

async fn event_identity_and_public_payload_schema() -> anyhow::Result<()> {
    let exported = event_schemas();
    anyhow::ensure!(exported.len() == 69);
    for kind in EVENTS {
        anyhow::ensure!(PLATFORM_EVENT_KINDS.contains(&kind));
        let (_, payload) = exported
            .iter()
            .find(|(candidate, _)| *candidate == kind)
            .ok_or_else(|| anyhow::anyhow!("{kind} is missing from schema export"))?;
        let document = event_schema(kind, payload.clone());
        let wrapper = if kind.starts_with("host/exposure.") {
            "exposure"
        } else {
            "binding"
        };
        anyhow::ensure!(document
            .pointer(&format!("/$defs/Payload/properties/{wrapper}"))
            .is_some());
        anyhow::ensure!(
            document.pointer("/properties/kind/const") == Some(&Value::String(kind.into()))
        );
    }
    Ok(())
}

async fn public_contract_contains_no_private_authority_or_handle() -> anyhow::Result<()> {
    for (method, params, result) in method_schemas()
        .into_iter()
        .filter(|(method, _, _)| METHODS.contains(method))
    {
        let wire = serde_json::to_string(&method_schema(method, params, result))?;
        for private in [
            "authority_handle_id",
            "authority_basis",
            "grant_reference",
            "raw_handle",
            "token",
            "host_path",
            "secret_value",
            "stderr",
        ] {
            anyhow::ensure!(!wire.contains(private), "{} leaked {private}", method.id());
        }
    }
    Ok(())
}

async fn candidates_are_effect_free_and_never_encode_selection() -> anyhow::Result<()> {
    let exported = method_schemas();
    let (_, params, result) = exported
        .iter()
        .find(|(method, _, _)| *method == PlatformMethod::BindingCandidates)
        .ok_or_else(|| anyhow::anyhow!("binding candidates schema is missing"))?;
    let params = serde_json::to_string(params)?;
    let result = serde_json::to_string(result)?;
    anyhow::ensure!(!params.contains("idempotency_key"));
    anyhow::ensure!(result.contains("candidates") && result.contains("gaps"));
    for public_fact in [
        "exposure",
        "audience",
        "consumer_port",
        "provider_port",
        "protocol_id",
        "interface_id",
        "profiles",
        "interaction",
        "accepted_effects",
        "effect_class",
        "transport",
        "latest_binding_phase",
        "availability",
        "multiplicity",
        "provider_work",
        "provider_installation",
        "source",
        "provider_component",
        "trust_class",
        "claim_status",
        "enforced_boundaries",
        "component_artifact",
        "behavior",
        "protocol_implementations",
    ] {
        anyhow::ensure!(
            result.contains(public_fact),
            "candidate disclosure omitted {public_fact}"
        );
    }
    anyhow::ensure!(!result.contains("data_risk") && !result.contains("effect_risk"));
    anyhow::ensure!(!result.contains("BindingMutationResult"));
    anyhow::ensure!(!result.contains("selected_binding"));
    anyhow::ensure!(MAX_PROVIDER_CANDIDATES > 1);
    Ok(())
}

async fn launch_and_runtime_wire_pins_are_structurally_distinct() -> anyhow::Result<()> {
    let exported = method_schemas();
    for method in [
        PlatformMethod::BindingCandidates,
        PlatformMethod::BindingSelect,
    ] {
        let (_, params, _) = exported
            .iter()
            .find(|(candidate, _, _)| *candidate == method)
            .ok_or_else(|| anyhow::anyhow!("{} schema is missing", method.id()))?;
        let wire = serde_json::to_string(params)?;
        let required = params
            .get("required")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("{} params have no required set", method.id()))?;
        anyhow::ensure!(required.contains(&Value::String("phase".into())));
        anyhow::ensure!(wire.contains("BindingPhase"));
        anyhow::ensure!(wire.contains("consumer_run"));
        anyhow::ensure!(wire.contains("run_id"));
        anyhow::ensure!(wire.contains("run_revision"));
        anyhow::ensure!(wire.contains("context_id"));
        anyhow::ensure!(!wire.contains("consumer_run_id"));
    }
    let (_, revoke, _) = exported
        .iter()
        .find(|(candidate, _, _)| *candidate == PlatformMethod::BindingRevoke)
        .ok_or_else(|| anyhow::anyhow!("binding revoke schema is missing"))?;
    let revoke = serde_json::to_string(revoke)?;
    anyhow::ensure!(revoke.contains("consumer_run") && revoke.contains("run_revision"));
    Ok(())
}

pub(crate) fn powerbox_cases() -> Vec<super::runner::ConformanceCase> {
    macro_rules! case {
        ($id:expr, [$($tag:expr),*], $func:path) => {
            super::registry::case($id, &[$($tag),*], || Box::pin($func()))
        };
    }
    vec![
        case!(
            "powerbox.public_method_identity_owner_typed_dto",
            ["phase5", "protocol", "exposure", "binding", "contract"],
            method_identity_owner_status_and_typed_dtos
        ),
        case!(
            "powerbox.public_method_actions_no_effect_on_denial",
            ["phase5", "protocol", "authority", "permission", "no_effect"],
            every_method_enforces_its_public_action_before_controller_effects
        ),
        case!(
            "powerbox.public_event_identity_payload",
            ["phase5", "protocol", "event", "exposure", "binding"],
            event_identity_and_public_payload_schema
        ),
        case!(
            "powerbox.public_wire_no_private_authority",
            ["phase5", "protocol", "authority", "privacy", "negative"],
            public_contract_contains_no_private_authority_or_handle
        ),
        case!(
            "powerbox.candidates_effect_free_no_auto_select",
            ["phase5", "protocol", "binding", "no_effect", "negative"],
            candidates_are_effect_free_and_never_encode_selection
        ),
        case!(
            "powerbox.launch_runtime_exact_run_pin",
            ["phase5", "protocol", "binding", "run", "cas"],
            launch_and_runtime_wire_pins_are_structurally_distinct
        ),
    ]
}
