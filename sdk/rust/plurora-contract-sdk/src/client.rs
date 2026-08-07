use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;

use crate::types::{ContractSelection, HostInfo};

#[async_trait]
pub trait PluroraTransport: Send + Sync {
    async fn invoke(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value>;

    async fn invoke_with_contract(
        &self,
        method: &str,
        params: serde_json::Value,
        contract: &ContractSelection,
    ) -> Result<serde_json::Value> {
        let _ = (method, params, contract);
        anyhow::bail!("Plurora transport does not support explicit contract selection")
    }

    fn invoke_stream(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Box<dyn Stream<Item = Result<serde_json::Value>> + Unpin + Send>;
}

pub struct PluroraClient {
    pub transport: Box<dyn PluroraTransport>,
    contract: Option<ContractSelection>,
}

impl PluroraClient {
    pub fn new(transport: Box<dyn PluroraTransport>) -> Self {
        Self {
            transport,
            contract: None,
        }
    }

    pub async fn invoke(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        match &self.contract {
            Some(contract) => {
                self.transport
                    .invoke_with_contract(method, params, contract)
                    .await
            }
            None => self.transport.invoke(method, params).await,
        }
    }

    pub async fn negotiate_host(&mut self, selection: ContractSelection) -> Result<HostInfo> {
        let raw = self
            .transport
            .invoke_with_contract("host.info", serde_json::json!({}), &selection)
            .await?;
        let info = serde_json::from_value(raw)?;
        self.contract = Some(selection);
        Ok(info)
    }

    pub fn clear_contract_selection(&mut self) {
        self.contract = None;
    }
}
