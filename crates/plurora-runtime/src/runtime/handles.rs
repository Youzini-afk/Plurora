use std::collections::HashMap;
use std::sync::Arc;

use plurora_core::{CapHandle, CapHandleId, HandleProvenance, PackageId};
use serde_json::Value;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct HandleTable {
    handles: Arc<RwLock<HashMap<CapHandleId, CapHandle>>>,
    by_holder: Arc<RwLock<HashMap<PackageId, Vec<CapHandleId>>>>,
}

impl HandleTable {
    pub async fn mint(&self, handle: CapHandle) -> CapHandleId {
        let id = handle.id;
        let holder = handle.scope.holder_package_id.clone();
        self.handles.write().await.insert(id, handle);
        self.by_holder
            .write()
            .await
            .entry(holder)
            .or_default()
            .push(id);
        id
    }

    pub async fn lookup(&self, id: CapHandleId) -> Option<CapHandle> {
        self.handles.read().await.get(&id).cloned()
    }

    pub async fn revoke(&self, id: CapHandleId) -> anyhow::Result<()> {
        let mut handles = self.handles.write().await;
        let handle = handles
            .get_mut(&id)
            .ok_or_else(|| anyhow::anyhow!("capability handle not found"))?;
        handle.revoked = true;
        Ok(())
    }

    pub async fn attenuate(
        &self,
        parent: CapHandleId,
        constraints: Value,
    ) -> anyhow::Result<CapHandleId> {
        let parent_handle = self
            .lookup(parent)
            .await
            .ok_or_else(|| anyhow::anyhow!("parent capability handle not found"))?;
        if parent_handle.revoked {
            anyhow::bail!("parent capability handle is revoked");
        }
        let mut child = parent_handle.clone();
        child.id = CapHandleId::new();
        child.constraints = constraints;
        child.parent = Some(parent);
        child.revoked = false;
        child.lease.invocations_used = 0;
        child.provenance = HandleProvenance {
            granted_at: chrono::Utc::now(),
            granted_by_package_id: parent_handle.scope.holder_package_id.clone(),
            via_method: "attenuate".to_string(),
        };
        Ok(self.mint(child).await)
    }

    pub async fn list_for(&self, holder: &PackageId) -> Vec<CapHandle> {
        let ids = self
            .by_holder
            .read()
            .await
            .get(holder)
            .cloned()
            .unwrap_or_default();
        let handles = self.handles.read().await;
        ids.into_iter()
            .filter_map(|id| handles.get(&id).cloned())
            .collect()
    }

    pub async fn record_invocation(&self, id: CapHandleId) -> anyhow::Result<()> {
        let mut handles = self.handles.write().await;
        let handle = handles
            .get_mut(&id)
            .ok_or_else(|| anyhow::anyhow!("capability handle not found"))?;
        if handle.revoked {
            anyhow::bail!("capability handle is revoked");
        }
        handle.lease.invocations_used = handle.lease.invocations_used.saturating_add(1);
        if let Some(max) = handle.lease.max_invocations {
            if handle.lease.invocations_used >= max {
                handle.revoked = true;
            }
        }
        Ok(())
    }
}
