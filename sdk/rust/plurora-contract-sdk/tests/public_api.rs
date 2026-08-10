use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use plurora_contract_sdk::{
    AppendEventRequest, ContractOwnerLayer, ContractSelection, ContractVersionRequirement,
    EmptyParams, InstallationRecordSchemaVersion, ObjectGetRequest, ObjectGetResponse,
    PluroraClient, PluroraTransport, ProtocolDescriptor, ProtocolSelection, WorkId, WorkRevision,
    WorkRevisionSchema, OBJECT_PUT,
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

fn work_revision_identity_and_title_remain_typed(value: &WorkRevision) {
    let _: &String = &value.title;
    let _: &WorkId = &value.work_id;
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
    let _ = work_revision_identity_and_title_remain_typed;
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
