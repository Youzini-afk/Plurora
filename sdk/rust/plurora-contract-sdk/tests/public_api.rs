use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use yg_kernel_sdk::{
    AppendEventRequest, AssetGetParams, ContractDiagnostic, ContractOwnerLayer, ContractSelection,
    ContractVersionRequirement, EmptyParams, KernelClient, KernelTransport, ProtocolDescriptor,
    ProtocolSelection, KERNEL_V1_ASSET_PUT,
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
impl KernelTransport for RecordingTransport {
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

fn generated_method_is_available(client: &KernelClient, params: AssetGetParams) {
    let _future = client.asset_get(params);
}

fn canonical_and_legacy_methods_are_available(client: &KernelClient, params: EmptyParams) {
    let _canonical_host = client.host_info(params.clone());
    let _legacy_host = client.legacy_kernel_v1_host_info(params.clone());
    let _canonical_target = client.target_list(params.clone());
    let _legacy_target = client.legacy_kernel_v1_target_list(params);
}

#[test]
fn generated_modules_are_exported_from_the_crate_root() {
    assert_eq!(KERNEL_V1_ASSET_PUT, "kernel/v1/asset.put");
    let _ = std::mem::size_of::<AppendEventRequest>();
    let _ = std::mem::size_of::<ContractSelection>();
    let _ = std::mem::size_of::<ProtocolSelection>();
    let _ = std::mem::size_of::<ProtocolDescriptor>();
    let _ = std::mem::size_of::<ContractDiagnostic>();
    let _ = generated_method_is_available;
    let _ = canonical_and_legacy_methods_are_available;
}

#[test]
fn negotiated_client_never_drops_the_contract_selection() {
    futures::executor::block_on(async {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut client = KernelClient::new(Box::new(RecordingTransport {
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
        client.host_info(params.clone()).await.unwrap();
        client.legacy_kernel_v1_host_info(params).await.unwrap();

        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                RecordedCall {
                    method: "host.info".to_string(),
                    contract: Some(selection.clone()),
                },
                RecordedCall {
                    method: "host.info".to_string(),
                    contract: Some(selection.clone()),
                },
                RecordedCall {
                    method: "kernel.v1.host.info".to_string(),
                    contract: Some(selection),
                },
            ]
        );
    });
}
