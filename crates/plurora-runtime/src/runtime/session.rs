use chrono::Utc;
use plurora_core::{
    new_id, KernelSession, SessionId, SessionStatus, EVENT_SESSION_CLOSED, EVENT_SESSION_OPENED,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Runtime;
use crate::EventStore;

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct OpenSessionRequest {
    pub labels: Vec<String>,
    pub active_package_set: Vec<plurora_core::PackageId>,
    pub metadata: Value,
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub async fn open_session(
        &self,
        mut request: OpenSessionRequest,
    ) -> anyhow::Result<KernelSession> {
        if request.labels.is_empty() {
            request.labels = self.config.default_labels.clone();
        }

        let now = Utc::now();
        let session = KernelSession {
            id: new_id("ses"),
            labels: request.labels,
            active_package_set: request.active_package_set,
            principal_scope: None,
            status: SessionStatus::Open,
            created_at: now,
            updated_at: now,
            metadata: request.metadata,
        };

        self.sessions
            .write()
            .await
            .insert(session.id.clone(), session.clone());

        self.append_kernel_event(
            &session.id,
            EVENT_SESSION_OPENED,
            serde_json::json!({
                "labels": session.labels,
                "active_package_set": session.active_package_set,
                "principal_scope": session.principal_scope,
            }),
        )
        .await?;

        Ok(session)
    }

    pub async fn close_session(
        &self,
        session_id: SessionId,
    ) -> anyhow::Result<plurora_core::EventEnvelope> {
        let mut sessions = self.sessions.write().await;
        match sessions.get_mut(&session_id) {
            Some(session) if session.status == SessionStatus::Open => {
                session.status = SessionStatus::Closed;
                session.updated_at = Utc::now();
            }
            Some(_) => anyhow::bail!("session '{session_id}' is already closed"),
            None => anyhow::bail!("session '{session_id}' is not open"),
        }
        drop(sessions);
        self.append_kernel_event(&session_id, EVENT_SESSION_CLOSED, serde_json::json!({}))
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{InMemoryEventStore, RuntimeConfig};
    use plurora_core::KERNEL_PACKAGE_ID;

    #[tokio::test]
    async fn session_open_records_kernel_event() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(store.clone(), RuntimeConfig::default());

        let session = runtime.open_session(OpenSessionRequest::default()).await?;
        let events = store.list_session(&session.id).await?;

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 0);
        assert_eq!(events[0].writer_package_id, KERNEL_PACKAGE_ID);
        assert_eq!(events[0].kind, EVENT_SESSION_OPENED);
        assert!(events[0].is_kernel_event());

        Ok(())
    }
}
