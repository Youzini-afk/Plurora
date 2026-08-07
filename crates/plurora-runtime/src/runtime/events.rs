use plurora_core::{
    EventEnvelope, EventKind, EventSequence, PackageId, SessionId, PLATFORM_RUNTIME_ID,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::broadcast;

use super::Runtime;
use crate::{validate_json_schema_subset, EventStore, ProtocolContext, ProtocolPrincipal};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AppendEventRequest {
    pub session_id: SessionId,
    pub writer_package_id: PackageId,
    pub kind: EventKind,
    pub payload: Value,
    pub metadata: Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct EventListRequest {
    pub session_id: SessionId,
    #[serde(default)]
    pub after_sequence: Option<EventSequence>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub kind_prefix: Option<String>,
    #[serde(default)]
    pub writer_package_id: Option<PackageId>,
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub async fn append_event(&self, request: AppendEventRequest) -> anyhow::Result<EventEnvelope> {
        match self.sessions.read().await.get(&request.session_id) {
            Some(session) if session.status == plurora_core::SessionStatus::Open => {}
            Some(_) => anyhow::bail!("session '{}' is closed", request.session_id),
            None => anyhow::bail!("session '{}' is not open", request.session_id),
        }

        if request.writer_package_id != PLATFORM_RUNTIME_ID {
            match (
                self.is_contract_none_package(&request.writer_package_id)
                    .await,
                self.packages.permissions(&request.writer_package_id).await,
            ) {
                (false, Some(permissions)) if permissions.events.append => {}
                _ => {
                    self.audit_permission_denied(
                        &request.session_id,
                        &request.writer_package_id,
                        "events.append",
                    )
                    .await?;
                    anyhow::bail!(
                        "package '{}' is not allowed to append events",
                        request.writer_package_id
                    );
                }
            }
        }

        let mut request = request;
        let before = self
            .dispatch_extension_handlers(
                "journal/before_append",
                json!({
                    "session_id": request.session_id,
                    "writer_package_id": request.writer_package_id,
                    "kind": request.kind,
                    "payload": request.payload,
                    "metadata": request.metadata,
                }),
            )
            .await;
        if let Some(vetoed_by) = before.vetoed_by {
            anyhow::bail!("event append vetoed by hook package '{vetoed_by}'");
        }
        request.metadata = before
            .payload
            .get("metadata")
            .cloned()
            .unwrap_or(request.metadata);
        let event = self.append_event_unchecked(request).await?;
        let _ = self
            .dispatch_extension_handlers(
                "journal/after_append",
                serde_json::to_value(&event).unwrap_or_else(|_| json!({})),
            )
            .await;
        Ok(event)
    }

    pub async fn append_event_with_context(
        &self,
        context: &ProtocolContext,
        mut request: AppendEventRequest,
    ) -> anyhow::Result<EventEnvelope> {
        match &context.principal {
            ProtocolPrincipal::Anonymous if context.is_host_device() => {
                self.ensure_host_session_access(context, "access_manage", &request.session_id)
                    .await?;
                self.append_event(request).await
            }
            ProtocolPrincipal::HostAdmin | ProtocolPrincipal::HostDev => {
                self.append_event(request).await
            }
            ProtocolPrincipal::Package { package_id } => {
                request.writer_package_id = package_id.clone();
                self.append_event(request).await
            }
            ProtocolPrincipal::Human { .. }
            | ProtocolPrincipal::Assistant { .. }
            | ProtocolPrincipal::Anonymous => {
                anyhow::bail!("principal is not allowed to append events directly")
            }
        }
    }

    pub(crate) async fn append_event_unchecked(
        &self,
        request: AppendEventRequest,
    ) -> anyhow::Result<EventEnvelope> {
        // Build a preliminary envelope to check writer_owns_kind before
        // allocating a sequence number. This prevents hook vetoes or
        // schema validation failures from consuming a sequence.
        let prelim = EventEnvelope {
            id: String::new(), // placeholder; real ID assigned atomically
            session_id: request.session_id.clone(),
            sequence: 0, // placeholder
            timestamp: chrono::Utc::now(),
            writer_package_id: request.writer_package_id.clone(),
            kind: request.kind.clone(),
            schema_version: 1,
            payload: request.payload.clone(),
            metadata: request.metadata.clone(),
        };

        if !prelim.writer_owns_kind() {
            anyhow::bail!(
                "package '{}' cannot write event kind '{}'",
                prelim.writer_package_id,
                prelim.kind
            );
        }

        if prelim.writer_package_id != PLATFORM_RUNTIME_ID {
            if let Some(manifest) = self.packages.manifest(&prelim.writer_package_id).await {
                if let Some(schema) = manifest
                    .contributes
                    .schemas
                    .iter()
                    .find(|schema| schema.id == prelim.kind)
                    .map(|schema| &schema.schema)
                {
                    validate_json_schema_subset(schema, &prelim.payload)?;
                }
            }
        }

        // All pre-checks passed — use store-level atomic append so
        // the sequence is allocated atomically with the insert.
        let event = self
            .store
            .append_with_sequence(
                request.session_id,
                request.writer_package_id,
                request.kind,
                prelim.schema_version,
                request.payload,
                request.metadata,
            )
            .await?;
        Ok(event)
    }

    pub async fn list_events(&self, session_id: &SessionId) -> anyhow::Result<Vec<EventEnvelope>> {
        self.store.list_session(session_id).await
    }

    pub async fn list_events_range(
        &self,
        request: &EventListRequest,
    ) -> anyhow::Result<Vec<EventEnvelope>> {
        let mut events = self
            .store
            .list_session_range(&request.session_id, request.after_sequence, request.limit)
            .await?;
        if let Some(kind_prefix) = &request.kind_prefix {
            events.retain(|event| event.kind.starts_with(kind_prefix));
        }
        if let Some(writer_package_id) = &request.writer_package_id {
            events.retain(|event| &event.writer_package_id == writer_package_id);
        }
        Ok(events)
    }

    pub async fn list_events_for(
        &self,
        session_id: &SessionId,
        caller_package_id: Option<&PackageId>,
    ) -> anyhow::Result<Vec<EventEnvelope>> {
        if let Some(caller) = caller_package_id {
            match (
                self.is_contract_none_package(caller).await,
                self.packages.permissions(caller).await,
            ) {
                (false, Some(permissions)) if permissions.events.read => {}
                _ => {
                    self.audit_permission_denied(session_id, caller, "events.read")
                        .await?;
                    anyhow::bail!("package '{caller}' is not allowed to read events");
                }
            }
        }
        self.list_events(session_id).await
    }

    pub async fn list_events_range_for(
        &self,
        request: &EventListRequest,
        caller_package_id: Option<&PackageId>,
    ) -> anyhow::Result<Vec<EventEnvelope>> {
        if let Some(caller) = caller_package_id {
            match (
                self.is_contract_none_package(caller).await,
                self.packages.permissions(caller).await,
            ) {
                (false, Some(permissions)) if permissions.events.read => {}
                _ => {
                    self.audit_permission_denied(&request.session_id, caller, "events.read")
                        .await?;
                    anyhow::bail!("package '{caller}' is not allowed to read events");
                }
            }
        }
        self.list_events_range(request).await
    }

    pub async fn list_events_with_context(
        &self,
        context: &ProtocolContext,
        session_id: &SessionId,
    ) -> anyhow::Result<Vec<EventEnvelope>> {
        match &context.principal {
            ProtocolPrincipal::Anonymous if context.is_host_device() => {
                self.ensure_host_session_access(context, "observe", session_id)
                    .await?;
                self.list_events(session_id).await
            }
            ProtocolPrincipal::HostAdmin | ProtocolPrincipal::HostDev => {
                self.list_events(session_id).await
            }
            ProtocolPrincipal::Package { package_id } => {
                self.list_events_for(session_id, Some(package_id)).await
            }
            ProtocolPrincipal::Human { .. }
            | ProtocolPrincipal::Assistant { .. }
            | ProtocolPrincipal::Anonymous => {
                if self
                    .principal_has_grant(&context.principal, "events.read", Some(session_id))
                    .await
                {
                    self.list_events(session_id).await
                } else {
                    anyhow::bail!("principal is not allowed to read events")
                }
            }
        }
    }

    pub async fn list_events_range_with_context(
        &self,
        context: &ProtocolContext,
        request: &EventListRequest,
    ) -> anyhow::Result<Vec<EventEnvelope>> {
        match &context.principal {
            ProtocolPrincipal::Anonymous if context.is_host_device() => {
                self.ensure_host_session_access(context, "observe", &request.session_id)
                    .await?;
                self.list_events_range(request).await
            }
            ProtocolPrincipal::HostAdmin | ProtocolPrincipal::HostDev => {
                self.list_events_range(request).await
            }
            ProtocolPrincipal::Package { package_id } => {
                self.list_events_range_for(request, Some(package_id)).await
            }
            ProtocolPrincipal::Human { .. }
            | ProtocolPrincipal::Assistant { .. }
            | ProtocolPrincipal::Anonymous => {
                if self
                    .principal_has_grant(
                        &context.principal,
                        "events.read",
                        Some(&request.session_id),
                    )
                    .await
                {
                    self.list_events_range(request).await
                } else {
                    anyhow::bail!("principal is not allowed to read events")
                }
            }
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<EventEnvelope> {
        self.store.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use plurora_core::{
        EntryDescriptor, PackageContributions, PackageEntry, PermissionSet, SandboxPolicy,
        EVENT_PERMISSION_DENIED,
    };
    use serde_json::json;

    use super::*;
    use crate::{InMemoryEventStore, RuntimeConfig};

    #[tokio::test]
    async fn package_cannot_write_another_namespace() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(store, RuntimeConfig::default());
        let session = runtime
            .open_session(super::super::OpenSessionRequest::default())
            .await?;

        let result = runtime
            .append_event(AppendEventRequest {
                session_id: session.id,
                writer_package_id: "org/a".to_string(),
                kind: "org/b/event".to_string(),
                payload: json!({}),
                metadata: json!({}),
            })
            .await;

        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn package_can_write_its_own_namespace() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(store.clone(), RuntimeConfig::default());
        let session = runtime
            .open_session(super::super::OpenSessionRequest::default())
            .await?;
        runtime
            .load_package(plurora_core::PackageManifest {
                schema_version: 1,
                id: "org/a".to_string(),
                version: "0.1.0".to_string(),
                display_name: None,
                description: None,
                author: None,
                license: None,
                entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                    crate_ref: "org-a".to_string(),
                    symbol: "register".to_string(),
                    abi_version: 1,
                }),
                provides: Vec::new(),
                consumes: Vec::new(),
                requires: Vec::new(),
                contributes: PackageContributions::default(),
                permissions: PermissionSet {
                    events: plurora_core::EventPermissions {
                        read: false,
                        append: true,
                    },
                    ..PermissionSet::default()
                },
                sandbox_policy: SandboxPolicy::default(),
            })
            .await?;

        let event = runtime
            .append_event(AppendEventRequest {
                session_id: session.id.clone(),
                writer_package_id: "org/a".to_string(),
                kind: "org/a/event".to_string(),
                payload: json!({"ok": true}),
                metadata: json!({}),
            })
            .await?;

        assert_eq!(event.sequence, 1);
        let events = store.list_session(&session.id).await?;
        assert_eq!(events.len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn denied_event_append_records_audit_event() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(store.clone(), RuntimeConfig::default());
        let session = runtime
            .open_session(super::super::OpenSessionRequest::default())
            .await?;

        let denied = runtime
            .append_event(AppendEventRequest {
                session_id: session.id.clone(),
                writer_package_id: "org/unauthorized".to_string(),
                kind: "org/unauthorized/event".to_string(),
                payload: json!({}),
                metadata: json!({}),
            })
            .await;
        assert!(denied.is_err());

        let events = store.list_session(&session.id).await?;
        assert_eq!(
            events.last().expect("audit event").kind,
            EVENT_PERMISSION_DENIED
        );
        Ok(())
    }

    #[tokio::test]
    async fn package_context_overrides_spoofed_event_writer() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(store, RuntimeConfig::default());
        let session = runtime
            .open_session(super::super::OpenSessionRequest::default())
            .await?;
        runtime
            .load_package(plurora_core::PackageManifest {
                schema_version: 1,
                id: "example/caller".to_string(),
                version: "0.1.0".to_string(),
                display_name: None,
                description: None,
                author: None,
                license: None,
                entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                    crate_ref: "example-caller".to_string(),
                    symbol: "register".to_string(),
                    abi_version: 1,
                }),
                provides: Vec::new(),
                consumes: Vec::new(),
                requires: Vec::new(),
                contributes: PackageContributions::default(),
                permissions: PermissionSet {
                    events: plurora_core::EventPermissions {
                        read: false,
                        append: true,
                    },
                    ..PermissionSet::default()
                },
                sandbox_policy: SandboxPolicy::default(),
            })
            .await?;

        let event = runtime
            .append_event_with_context(
                &ProtocolContext::package("example/caller", "test"),
                AppendEventRequest {
                    session_id: session.id,
                    writer_package_id: "example/spoofed".to_string(),
                    kind: "example/caller/event".to_string(),
                    payload: json!({}),
                    metadata: json!({}),
                },
            )
            .await?;

        assert_eq!(event.writer_package_id, "example/caller");
        Ok(())
    }
}
