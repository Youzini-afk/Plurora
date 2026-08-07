use plurora_core::{SessionId, EVENT_PROJECTION_UPDATED};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::Runtime;
use crate::EventStore;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionDefinition {
    pub id: String,
    pub session_id: SessionId,
    #[serde(default)]
    pub source_kind_prefix: Option<String>,
    #[serde(default)]
    pub state: Value,
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub async fn projection_register(
        &self,
        definition: ProjectionDefinition,
    ) -> anyhow::Result<ProjectionDefinition> {
        self.projections
            .write()
            .await
            .insert(definition.id.clone(), definition.clone());
        Ok(definition)
    }

    pub async fn projection_rebuild(
        &self,
        projection_id: &str,
    ) -> anyhow::Result<ProjectionDefinition> {
        let mut projections = self.projections.write().await;
        let projection = projections
            .get_mut(projection_id)
            .ok_or_else(|| anyhow::anyhow!("projection '{projection_id}' not found"))?;
        let events = self
            .list_events_range(&super::EventListRequest {
                session_id: projection.session_id.clone(),
                after_sequence: None,
                limit: None,
                kind_prefix: projection.source_kind_prefix.clone(),
                writer_package_id: None,
            })
            .await?;
        projection.state = json!({"event_count": events.len(), "last_sequence": events.last().map(|event| event.sequence)});
        let projection = projection.clone();
        drop(projections);
        self.append_kernel_event(
            &format!("kernel_projection_{}", projection.id.replace('/', "_")),
            EVENT_PROJECTION_UPDATED,
            serde_json::to_value(&projection)?,
        )
        .await?;
        Ok(projection)
    }

    pub async fn projection_get(
        &self,
        projection_id: &str,
    ) -> anyhow::Result<ProjectionDefinition> {
        self.projections
            .read()
            .await
            .get(projection_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("projection '{projection_id}' not found"))
    }

    pub async fn projection_list(&self) -> Vec<ProjectionDefinition> {
        let mut projections: Vec<_> = self.projections.read().await.values().cloned().collect();
        projections.sort_by(|a, b| a.id.cmp(&b.id));
        projections
    }
}
