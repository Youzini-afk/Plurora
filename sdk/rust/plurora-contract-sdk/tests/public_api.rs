use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use plurora_contract_sdk::{
    AppendEventRequest, BindingCandidate, BindingCandidatesRequest, BindingComponentDisclosure,
    BindingInstallationDisclosure, BindingListRequest, BindingPhase, BindingRevokeRequest,
    BindingSelectRequest, BindingView, BindingWorkDisclosure, ContractOwnerLayer,
    ContractSelection, ContractVersionRequirement, EmptyParams, ExposureCreateRequest,
    ExposureListRequest, ExposureRevokeRequest, ExposureView, HostBindingExpiredPayload,
    HostBindingRevokedPayload, HostBindingSelectedPayload, HostExposureCreatedPayload,
    HostExposureExpiredPayload, HostExposureRevokedPayload, InstallationRecordSchemaVersion,
    ObjectGetRequest, ObjectGetResponse, PluroraClient, PluroraTransport, PortDescriptor,
    ProtocolDescriptor, ProtocolSelection, WorkId, WorkRevision, WorkRevisionSchema,
    HOST_BINDING_EXPIRED, HOST_BINDING_REVOKED, HOST_BINDING_SELECTED, HOST_EXPOSURE_CREATED,
    HOST_EXPOSURE_EXPIRED, HOST_EXPOSURE_REVOKED, OBJECT_PUT,
};

#[derive(Clone, Debug, PartialEq)]
struct RecordedCall {
    method: String,
    contract: Option<ContractSelection>,
}

struct RecordingTransport {
    calls: Arc<Mutex<Vec<RecordedCall>>>,
}

#[async_trait]
impl PluroraTransport for RecordingTransport {
    async fn invoke(&self, method: &str, _params: serde_json::Value) -> Result<serde_json::Value> {
        self.calls.lock().unwrap().push(RecordedCall {
            method: method.to_string(),
            contract: None,
        });
        Ok(host_info_json())
    }

    async fn invoke_with_contract(
        &self,
        method: &str,
        _params: serde_json::Value,
        contract: &ContractSelection,
    ) -> Result<serde_json::Value> {
        self.calls.lock().unwrap().push(RecordedCall {
            method: method.to_string(),
            contract: Some(contract.clone()),
        });
        Ok(host_info_json())
    }

    fn invoke_stream(
        &self,
        _method: &str,
        _params: serde_json::Value,
    ) -> Box<dyn Stream<Item = Result<serde_json::Value>> + Unpin + Send> {
        Box::new(futures::stream::empty())
    }
}

fn host_info_json() -> serde_json::Value {
    serde_json::json!({
        "protocol_version": "0.1.0",
        "methods": [],
        "supported_transports": ["test"]
    })
}

fn generated_method_is_available(client: &PluroraClient, params: ObjectGetRequest) {
    let _future = client.object_get(params);
}

fn canonical_methods_are_available(client: &PluroraClient, params: EmptyParams) {
    let _host = client.host_info(params.clone());
    let _target = client.host_target_list(params);
}

fn powerbox_methods_are_available(
    client: &PluroraClient,
    exposure_list: ExposureListRequest,
    exposure_create: ExposureCreateRequest,
    exposure_revoke: ExposureRevokeRequest,
    binding_list: BindingListRequest,
    candidates: BindingCandidatesRequest,
    select: BindingSelectRequest,
    revoke: BindingRevokeRequest,
) {
    let _ = client.host_exposure_list(exposure_list);
    let _ = client.host_exposure_create(exposure_create);
    let _ = client.host_exposure_revoke(exposure_revoke);
    let _ = client.host_binding_list(binding_list);
    let _ = client.host_binding_candidates(candidates);
    let _ = client.host_binding_select(select);
    let _ = client.host_binding_revoke(revoke);
}

fn work_revision_identity_and_title_remain_typed(value: &WorkRevision) {
    let _: &String = &value.title;
    let _: &WorkId = &value.work_id;
}

fn powerbox_candidate_disclosure_remains_typed(
    candidate: &BindingCandidate,
    candidates: &BindingCandidatesRequest,
    select: &BindingSelectRequest,
) {
    let _: &BindingPhase = &candidates.phase;
    let _: &BindingPhase = &select.phase;
    let _: &ExposureView = &candidate.exposure;
    let _: &PortDescriptor = &candidate.consumer_port;
    let _: &PortDescriptor = &candidate.provider_port;
    let _: &BindingWorkDisclosure = &candidate.provider_work;
    let _: &BindingInstallationDisclosure = &candidate.provider_installation;
    let _: &BindingComponentDisclosure = &candidate.provider_component;
}

#[test]
fn generated_modules_are_exported_from_the_crate_root() {
    assert_eq!(OBJECT_PUT, "object/put");
    let _ = std::mem::size_of::<AppendEventRequest>();
    let _ = std::mem::size_of::<ContractSelection>();
    let _ = std::mem::size_of::<ProtocolSelection>();
    let _ = std::mem::size_of::<ProtocolDescriptor>();
    let _ = generated_method_is_available;
    let _ = canonical_methods_are_available;
    let _ = powerbox_methods_are_available;
    let _ = work_revision_identity_and_title_remain_typed;
    let _ = powerbox_candidate_disclosure_remains_typed;

    assert_eq!(HOST_EXPOSURE_CREATED, "host/exposure.created");
    assert_eq!(HOST_EXPOSURE_REVOKED, "host/exposure.revoked");
    assert_eq!(HOST_EXPOSURE_EXPIRED, "host/exposure.expired");
    assert_eq!(HOST_BINDING_SELECTED, "host/binding.selected");
    assert_eq!(HOST_BINDING_REVOKED, "host/binding.revoked");
    assert_eq!(HOST_BINDING_EXPIRED, "host/binding.expired");
    let _ = std::mem::size_of::<ExposureView>();
    let _ = std::mem::size_of::<BindingView>();
    let _ = std::mem::size_of::<HostExposureCreatedPayload>();
    let _ = std::mem::size_of::<HostExposureRevokedPayload>();
    let _ = std::mem::size_of::<HostExposureExpiredPayload>();
    let _ = std::mem::size_of::<HostBindingSelectedPayload>();
    let _ = std::mem::size_of::<HostBindingRevokedPayload>();
    let _ = std::mem::size_of::<HostBindingExpiredPayload>();
    let generated_types = include_str!("../src/types.rs");
    for type_name in [
        "BindingCandidate",
        "BindingComponentDisclosure",
        "BindingInstallationDisclosure",
        "BindingWorkDisclosure",
    ] {
        let marker = format!("pub struct {type_name} {{");
        let start = generated_types
            .find(&marker)
            .expect("generated public type");
        let body = &generated_types[start..];
        let end = body.find("\n}\n").expect("generated public type body") + 3;
        for private in [
            "authority_handle_id",
            "authority_basis",
            "grant_reference",
            "raw_handle",
            "host_path",
            "secret_value",
            "stderr",
        ] {
            assert!(
                !body[..end].contains(private),
                "{type_name} leaked {private}"
            );
        }
    }
}

#[test]
fn generated_work_discriminators_reject_unknown_values() {
    assert!(serde_json::from_value::<WorkRevisionSchema>(serde_json::json!("wrong")).is_err());
    assert!(
        serde_json::from_value::<InstallationRecordSchemaVersion>(serde_json::json!(2)).is_err()
    );
    assert!(
        serde_json::from_value::<WorkRevisionSchema>(serde_json::json!("plurora.work-revision.v1"))
            .is_ok()
    );
    assert!(
        serde_json::from_value::<InstallationRecordSchemaVersion>(serde_json::json!(1)).is_ok()
    );
}

#[test]
fn generated_object_get_preserves_asset_wire_and_accepts_state_audit_shape() {
    let asset_request = serde_json::json!({"asset_id": "ast_ordinary"});
    let parsed: ObjectGetRequest = serde_json::from_value(asset_request.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), asset_request);
    assert!(serde_json::from_value::<ObjectGetRequest>(
        serde_json::json!({"kind": "asset", "asset_id": "ast_ordinary"})
    )
    .is_err());

    let state_request = serde_json::json!({
        "installation_id": "00000000-0000-4000-8000-000000000007",
        "installation_state_artifact": {
            "artifact_type_uri": "urn:plurora:installation-state-reset-receipt:v1",
            "media_type": "application/vnd.plurora.installation-state-receipt+json",
            "digest": format!("sha256:{}", "a".repeat(64)),
            "size_bytes": 1
        }
    });
    let parsed: ObjectGetRequest = serde_json::from_value(state_request.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), state_request);

    let asset_response = serde_json::json!({
        "record": {
            "id": "ast_ordinary",
            "origin_package_id": "plurora/platform-runtime",
            "mime": "text/plain",
            "hash": format!("sha256:{}", "b".repeat(64)),
            "size_bytes": 7,
            "created_at": "2026-08-10T00:00:00Z",
            "metadata": null
        },
        "content": "ordinary"
    });
    let parsed: ObjectGetResponse = serde_json::from_value(asset_response.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), asset_response);
}

#[test]
fn negotiated_client_never_drops_the_contract_selection() {
    futures::executor::block_on(async {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut client = PluroraClient::new(Box::new(RecordingTransport {
            calls: calls.clone(),
        }));
        let selection = ContractSelection {
            profile: "plurora.contract.default/v1".to_string(),
            versions: vec![ContractVersionRequirement {
                layer: ContractOwnerLayer::Host,
                version: "0.1.0".to_string(),
            }],
            protocols: vec![ProtocolSelection {
                protocol_id: "plurora.change".to_string(),
                version: "1.0.0".to_string(),
                profile: Some("plurora.change/default/v1".to_string()),
            }],
        };
        client.negotiate_host(selection.clone()).await.unwrap();
        let params: EmptyParams = serde_json::from_value(serde_json::json!({})).unwrap();
        client.host_info(params).await.unwrap();

        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                RecordedCall {
                    method: "host.info".to_string(),
                    contract: Some(selection.clone()),
                },
                RecordedCall {
                    method: "host.info".to_string(),
                    contract: Some(selection),
                },
            ]
        );
    });
}
