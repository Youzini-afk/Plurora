//! Network policy checker and outbound audit helpers.
//!
//! This module provides the generic network policy enforcement for
//! Ygg-provided network/request helpers. It does NOT intercept
//! arbitrary subprocess OS calls — it checks whether a package's
//! manifest and the host policy permit a given outbound request.
//!
//! The checker is called before the runtime records an outbound
//! audit event and before the request is forwarded.

use chrono::Utc;
use plurora_core::{
    new_id, ArtifactDescriptor, CapabilityId, EffectScope, EffectTerminalStatus,
    NetworkDeclaration, NetworkPermissions, OutboundAuditRecord, PackageId, PrincipalIdentity,
    RedactionState, EVENT_OUTBOUND_DENIED, EVENT_OUTBOUND_EXECUTE_COMPLETED,
    EVENT_OUTBOUND_REQUEST, EVENT_OUTBOUND_STREAM_COMPLETED, EVENT_OUTBOUND_WEBSOCKET_COMPLETED,
};
use serde_json::{json, Value};
use uuid::Uuid;

use super::effects::{principal_identity, EffectReceiptRequest};
use super::Runtime;
use crate::{EventStore, ProtocolPrincipal, DEFAULT_CONTRACT_PROFILE};

/// Result of a network policy check.
#[derive(Debug, Clone)]
pub struct NetworkPolicyDecision {
    /// Whether the request is allowed.
    pub allowed: bool,
    /// Reason for denial, if not allowed.
    pub denial_reason: Option<String>,
    /// The matching declaration, if found.
    pub matched_declaration: Option<NetworkDeclaration>,
}

/// Check whether a package's manifest and host policy allow an outbound
/// request to the given destination.
///
/// This function:
/// 1. Looks up the package's `network` permissions from the manifest.
/// 2. Checks structured `declarations` first, falling back to the
///    flat `hosts` list for backward compatibility.
/// 3. Matches host (exact or glob-style with `*` prefix).
/// 4. Matches method if the declaration specifies methods.
/// 5. Official packages have no bypass — they must declare network
///    permissions like any other package.
///
/// This is designed for the Ygg-provided network/request helper path,
/// not for intercepting arbitrary subprocess OS calls.
pub fn check_network_policy(
    permissions: &NetworkPermissions,
    destination_host: &str,
    method: &str,
) -> NetworkPolicyDecision {
    // Check structured declarations first
    for decl in &permissions.declarations {
        if host_matches(&decl.host, destination_host) {
            if decl.methods.is_empty() || method_matches(&decl.methods, method) {
                return NetworkPolicyDecision {
                    allowed: true,
                    denial_reason: None,
                    matched_declaration: Some(decl.clone()),
                };
            }
        }
    }

    // Fall back to flat hosts list (backward compat)
    for host in &permissions.hosts {
        if host_matches(host, destination_host) {
            return NetworkPolicyDecision {
                allowed: true,
                denial_reason: None,
                matched_declaration: Some(NetworkDeclaration {
                    host: host.clone(),
                    methods: Vec::new(),
                    purpose: None,
                }),
            };
        }
    }

    // No matching declaration — denied
    NetworkPolicyDecision {
        allowed: false,
        denial_reason: Some(format!(
            "package has no network permission for host '{}' method '{}'",
            destination_host, method
        )),
        matched_declaration: None,
    }
}

/// Check whether a host pattern matches a destination host.
///
/// Supports:
/// - Exact match: `"api.example.com"` matches `"api.example.com"`
/// - Wildcard prefix: `"*.example.com"` matches `"api.example.com"`
/// - Port-specific: `"api.example.com:443"` matches `"api.example.com:443"`
fn host_matches(pattern: &str, destination: &str) -> bool {
    if pattern == destination {
        return true;
    }
    // Wildcard prefix: *.example.com matches sub.example.com
    if let Some(suffix) = pattern.strip_prefix("*.") {
        // destination must end with .suffix and have at least one char before it
        if let Some(dest_domain) = destination.strip_prefix("*.") {
            // destination is also a wildcard — exact match
            return dest_domain == suffix;
        }
        if destination.ends_with(&format!(".{}", suffix)) {
            // Ensure there's a subdomain part before the suffix
            let prefix = &destination[..destination.len() - suffix.len() - 1];
            return !prefix.is_empty();
        }
        // Also match the base domain itself for *.example.com
        if destination == suffix {
            return true;
        }
    }
    false
}

/// Check whether a method is in the allowed methods list (case-insensitive).
fn method_matches(allowed: &[String], method: &str) -> bool {
    allowed.iter().any(|m| m.eq_ignore_ascii_case(method))
}

/// Request to check and potentially record an outbound network request.
#[derive(Debug, Clone)]
pub struct OutboundRequest {
    /// The principal that initiated the request.
    pub principal: ProtocolPrincipal,
    /// Package that owns the outbound request.
    pub package_id: PackageId,
    /// Capability through which the request was made.
    pub capability_id: CapabilityId,
    /// Destination host.
    pub destination_host: String,
    /// HTTP method (GET, POST, etc).
    pub method: String,
    /// Declared purpose (overrides manifest purpose if set).
    pub purpose: Option<String>,
    /// Secret references used (not raw secrets).
    pub secret_refs_used: Vec<String>,
    #[allow(dead_code)]
    pub correlation_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct OutboundExecuteCompletion<'a> {
    pub id: &'a str,
    pub package_id: &'a str,
    pub capability_id: &'a str,
    pub destination_host: &'a str,
    pub method: &'a str,
    pub status: &'a str,
    pub executor_kind: &'a str,
    pub status_code: Option<u16>,
    pub total_bytes_request: u64,
    pub total_bytes_response: u64,
    pub duration_ms: u64,
    pub network_performed: bool,
    pub redaction_state: RedactionState,
    pub secret_refs_used: &'a [String],
    pub correlation_id: Option<Uuid>,
    pub usage: Value,
    pub cost: Value,
}

#[derive(Debug, Clone)]
pub struct OutboundStreamCompletion<'a> {
    pub id: &'a str,
    pub package_id: &'a str,
    pub capability_id: &'a str,
    pub destination_host: &'a str,
    pub method: &'a str,
    pub stream_format: &'a str,
    pub status: &'a str,
    pub total_chunks: u64,
    pub total_bytes: u64,
    pub duration_ms: u64,
    pub final_termination: &'a str,
    pub executor_kind: &'a str,
    pub network_performed: bool,
    pub redaction_state: RedactionState,
    pub secret_refs_used: &'a [String],
    pub correlation_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct OutboundWebSocketCompletion<'a> {
    pub id: &'a str,
    pub package_id: &'a str,
    pub capability_id: &'a str,
    pub destination_host: &'a str,
    pub connection_id: &'a str,
    pub code: u16,
    pub reason: &'a str,
    pub total_frames_in: u64,
    pub total_frames_out: u64,
    pub total_bytes_in: u64,
    pub total_bytes_out: u64,
    pub duration_ms: u64,
    pub executor_kind: &'a str,
    pub network_performed: bool,
    pub redaction_state: RedactionState,
    pub secret_refs_used: &'a [String],
    pub correlation_id: Option<Uuid>,
    pub parent_receipts: &'a [String],
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    /// Check network policy for an outbound request and, if allowed,
    /// record an outbound audit event with redacted state.
    ///
    /// If denied, records an outbound denial audit event and returns an error.
    /// This is the primary entry point for Ygg-provided network helpers.
    pub async fn check_and_audit_outbound(
        &self,
        request: OutboundRequest,
    ) -> anyhow::Result<OutboundAuditRecord> {
        let contract_none = self.is_contract_none_package(&request.package_id).await;
        let permissions = self.packages.permissions(&request.package_id).await;
        let permissions = permissions.unwrap_or_default();

        let decision = if contract_none {
            NetworkPolicyDecision {
                allowed: false,
                denial_reason: Some(
                    "Foreign Capsule is self-contained and receives no kernel network authority"
                        .to_string(),
                ),
                matched_declaration: None,
            }
        } else {
            check_network_policy(
                &permissions.network,
                &request.destination_host,
                &request.method,
            )
        };

        let principal_str =
            serde_json::to_string(&request.principal).unwrap_or_else(|_| "\"unknown\"".to_string());

        if !decision.allowed {
            let record = OutboundAuditRecord {
                id: new_id("ob"),
                principal: principal_str,
                package_id: request.package_id.clone(),
                capability_id: request.capability_id.clone(),
                destination_host: request.destination_host.clone(),
                method: request.method.clone(),
                purpose: request.purpose.clone(),
                redaction_state: RedactionState::NotCaptured,
                secret_refs_used: request.secret_refs_used.clone(),
                usage: json!({}),
                cost: json!({}),
                status: "denied".to_string(),
                error: decision.denial_reason.clone(),
            };
            let session_id = format!("kernel_outbound_{}", request.package_id.replace('/', "_"));
            let receipt = self
                .record_outbound_policy_denial(&request, &record)
                .await?;
            let mut payload = serde_json::to_value(&record)?;
            payload
                .as_object_mut()
                .expect("outbound audit record serializes as an object")
                .insert("receipt".to_string(), serde_json::to_value(receipt)?);
            self.append_kernel_event(&session_id, EVENT_OUTBOUND_DENIED, payload)
                .await?;
            anyhow::bail!(
                "outbound request denied: {}",
                decision.denial_reason.unwrap_or_default()
            );
        }

        let purpose = request.purpose.or_else(|| {
            decision
                .matched_declaration
                .as_ref()
                .and_then(|d| d.purpose.clone())
        });

        let record = OutboundAuditRecord {
            id: new_id("ob"),
            principal: principal_str,
            package_id: request.package_id.clone(),
            capability_id: request.capability_id.clone(),
            destination_host: request.destination_host.clone(),
            method: request.method.clone(),
            purpose,
            redaction_state: RedactionState::Redacted,
            secret_refs_used: request.secret_refs_used.clone(),
            usage: json!({}),
            cost: json!({}),
            status: "allowed".to_string(),
            error: None,
        };
        let session_id = format!("kernel_outbound_{}", request.package_id.replace('/', "_"));
        self.append_kernel_event(
            &session_id,
            EVENT_OUTBOUND_REQUEST,
            serde_json::to_value(&record)?,
        )
        .await?;

        Ok(record)
    }

    pub async fn emit_outbound_execute_completed(
        &self,
        completion: OutboundExecuteCompletion<'_>,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let session_id = format!(
            "kernel_outbound_{}",
            completion.package_id.replace('/', "_")
        );
        let payload = json!({
            "id": completion.id,
            "package_id": completion.package_id,
            "capability_id": completion.capability_id,
            "destination_host": completion.destination_host,
            "method": completion.method,
            "status": completion.status,
            "executor_kind": completion.executor_kind,
            "status_code": completion.status_code,
            "total_bytes_request": completion.total_bytes_request,
            "total_bytes_response": completion.total_bytes_response,
            "duration_ms": completion.duration_ms,
            "network_performed": completion.network_performed,
            "redaction_state": completion.redaction_state,
            "secret_refs_used": completion.secret_refs_used,
            "correlation_id": completion.correlation_id,
        });
        let receipt = self
            .record_outbound_execute_effect(&completion, &payload)
            .await?;
        let mut payload = payload;
        payload
            .as_object_mut()
            .expect("outbound completion payload is an object")
            .insert("receipt".to_string(), serde_json::to_value(&receipt)?);
        self.append_kernel_event(&session_id, EVENT_OUTBOUND_EXECUTE_COMPLETED, payload)
            .await?;
        Ok(receipt)
    }

    pub async fn emit_outbound_stream_completed(
        &self,
        session_id: &str,
        completion: OutboundStreamCompletion<'_>,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let payload = json!({
            "id": completion.id,
            "package_id": completion.package_id,
            "capability_id": completion.capability_id,
            "destination_host": completion.destination_host,
            "method": completion.method,
            "stream_format": completion.stream_format,
            "status": completion.status,
            "total_chunks": completion.total_chunks,
            "total_bytes": completion.total_bytes,
            "duration_ms": completion.duration_ms,
            "final_termination": completion.final_termination,
            "executor_kind": completion.executor_kind,
            "network_performed": completion.network_performed,
            "redaction_state": completion.redaction_state,
            "secret_refs_used": completion.secret_refs_used,
            "correlation_id": completion.correlation_id,
        });
        let receipt = self
            .record_outbound_stream_effect(&completion, &payload)
            .await?;
        let mut payload = payload;
        payload
            .as_object_mut()
            .expect("outbound stream completion payload is an object")
            .insert("receipt".to_string(), serde_json::to_value(&receipt)?);
        append_outbound_completion_event(
            self.store.clone(),
            session_id.to_string(),
            EVENT_OUTBOUND_STREAM_COMPLETED,
            payload,
        )
        .await?;
        Ok(receipt)
    }

    pub async fn emit_outbound_websocket_completed(
        &self,
        session_id: &str,
        completion: OutboundWebSocketCompletion<'_>,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let payload = websocket_completed_payload(&completion);
        let receipt = self
            .record_outbound_websocket_effect(&completion, &payload)
            .await?;
        let mut payload = payload;
        payload
            .as_object_mut()
            .expect("outbound websocket completion payload is an object")
            .insert("receipt".to_string(), serde_json::to_value(&receipt)?);
        append_outbound_completion_event(
            self.store.clone(),
            session_id.to_string(),
            EVENT_OUTBOUND_WEBSOCKET_COMPLETED,
            payload,
        )
        .await?;
        Ok(receipt)
    }

    /// List outbound audit events for a given package (both allowed and denied).
    pub async fn list_outbound_audit(
        &self,
        package_id: &PackageId,
    ) -> anyhow::Result<Vec<plurora_core::EventEnvelope>> {
        let session_id = format!("kernel_outbound_{}", package_id.replace('/', "_"));
        // Use session+kind-prefix pushdown instead of list_session + full filter.
        let request_events = self
            .store
            .list_session_kind_prefix(&session_id, "kernel/v1/outbound")
            .await?;
        Ok(request_events)
    }

    async fn record_outbound_execute_effect(
        &self,
        completion: &OutboundExecuteCompletion<'_>,
        payload: &Value,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let status = outbound_terminal_status(completion.status);
        let mut request = EffectReceiptRequest::live(
            "outbound.execute",
            PrincipalIdentity::Package {
                package_id: completion.package_id.to_string(),
            },
            json!({
                "kind": "outbound_executor",
                "executor_kind": completion.executor_kind,
                "capability_id": completion.capability_id,
            }),
            status,
            Utc::now() - chrono::Duration::milliseconds(completion.duration_ms as i64),
            completion.duration_ms.max(1),
            completion
                .correlation_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| completion.id.to_string()),
        );
        request.protocol_profiles = vec![DEFAULT_CONTRACT_PROFILE.to_string()];
        request.inputs = vec![json!({
            "destination_host": completion.destination_host,
            "method": completion.method,
            "total_bytes_request": completion.total_bytes_request,
            "secret_refs_used": completion.secret_refs_used,
        })];
        request.outputs = vec![payload.clone()];
        request.external_effects = completion
            .network_performed
            .then(|| {
                json!({
                    "kind": "network_io",
                    "destination_host": completion.destination_host,
                    "status_code": completion.status_code,
                    "bytes_sent": completion.total_bytes_request,
                    "bytes_received": completion.total_bytes_response,
                })
            })
            .into_iter()
            .collect();
        request.authority = Some(json!({
            "package_id": completion.package_id,
            "capability_id": completion.capability_id,
            "secret_refs_used": completion.secret_refs_used,
        }));
        request.policy_decision = Some(json!({
            "outcome": if status == EffectTerminalStatus::Denied { "denied" } else { "allowed" },
            "network_performed": completion.network_performed,
        }));
        request.usage = completion.usage.clone();
        request.cost = completion.cost.clone();
        request.scope = EffectScope::default();
        request.planned = json!({
            "destination_host": completion.destination_host,
            "method": completion.method,
        });
        request.actual = payload.clone();
        self.record_effect_receipt(request).await
    }

    async fn record_outbound_policy_denial(
        &self,
        outbound: &OutboundRequest,
        audit: &OutboundAuditRecord,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let mut request = EffectReceiptRequest::live(
            "outbound.policy",
            principal_identity(&outbound.principal),
            json!({
                "kind": "network_policy",
                "capability_id": outbound.capability_id,
            }),
            EffectTerminalStatus::Denied,
            Utc::now(),
            1,
            outbound
                .correlation_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| audit.id.clone()),
        );
        request.protocol_profiles = vec![DEFAULT_CONTRACT_PROFILE.to_string()];
        request.inputs = vec![json!({
            "package_id": outbound.package_id,
            "capability_id": outbound.capability_id,
            "destination_host": outbound.destination_host,
            "method": outbound.method,
            "purpose_present": outbound.purpose.is_some(),
            "secret_refs_used": outbound.secret_refs_used,
        })];
        request.outputs = vec![json!({
            "status": "denied",
            "network_performed": false,
        })];
        request.authority = Some(json!({
            "package_id": outbound.package_id,
            "capability_id": outbound.capability_id,
        }));
        request.policy_decision = Some(json!({
            "outcome": "denied",
            "matched_declaration": false,
        }));
        request.planned = json!({
            "destination_host": outbound.destination_host,
            "method": outbound.method,
        });
        request.actual = json!({
            "status": "denied",
            "network_performed": false,
        });
        self.record_effect_receipt(request).await
    }

    async fn record_outbound_stream_effect(
        &self,
        completion: &OutboundStreamCompletion<'_>,
        payload: &Value,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let status = outbound_terminal_status(completion.status);
        let mut request = EffectReceiptRequest::live(
            "outbound.stream",
            PrincipalIdentity::Package {
                package_id: completion.package_id.to_string(),
            },
            json!({
                "kind": "outbound_stream_executor",
                "executor_kind": completion.executor_kind,
                "capability_id": completion.capability_id,
            }),
            status,
            Utc::now() - chrono::Duration::milliseconds(completion.duration_ms as i64),
            completion.duration_ms.max(1),
            completion
                .correlation_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| completion.id.to_string()),
        );
        request.protocol_profiles = vec![DEFAULT_CONTRACT_PROFILE.to_string()];
        request.inputs = vec![json!({
            "destination_host": completion.destination_host,
            "method": completion.method,
            "stream_format": completion.stream_format,
            "secret_refs_used": completion.secret_refs_used,
        })];
        request.outputs = vec![payload.clone()];
        request.external_effects = completion
            .network_performed
            .then(|| {
                json!({
                    "kind": "network_stream",
                    "destination_host": completion.destination_host,
                    "chunks": completion.total_chunks,
                    "bytes_received": completion.total_bytes,
                    "termination": completion.final_termination,
                })
            })
            .into_iter()
            .collect();
        request.authority = Some(json!({
            "package_id": completion.package_id,
            "capability_id": completion.capability_id,
            "secret_refs_used": completion.secret_refs_used,
        }));
        request.policy_decision = Some(json!({
            "outcome": if status == EffectTerminalStatus::Denied { "denied" } else { "allowed" },
        }));
        request.planned = json!({
            "destination_host": completion.destination_host,
            "method": completion.method,
            "stream_format": completion.stream_format,
        });
        request.actual = payload.clone();
        self.record_effect_receipt(request).await
    }

    async fn record_outbound_websocket_effect(
        &self,
        completion: &OutboundWebSocketCompletion<'_>,
        payload: &Value,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let status = match completion.code {
            1000 => EffectTerminalStatus::Succeeded,
            1001 => EffectTerminalStatus::Cancelled,
            1008 => EffectTerminalStatus::Denied,
            1013 => EffectTerminalStatus::TimedOut,
            _ => EffectTerminalStatus::Failed,
        };
        let mut request = EffectReceiptRequest::live(
            "outbound.websocket",
            PrincipalIdentity::Package {
                package_id: completion.package_id.to_string(),
            },
            json!({
                "kind": "outbound_websocket_executor",
                "executor_kind": completion.executor_kind,
                "capability_id": completion.capability_id,
            }),
            status,
            Utc::now() - chrono::Duration::milliseconds(completion.duration_ms as i64),
            completion.duration_ms.max(1),
            completion
                .correlation_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| completion.id.to_string()),
        );
        request.protocol_profiles = vec![DEFAULT_CONTRACT_PROFILE.to_string()];
        request.inputs = vec![json!({
            "destination_host": completion.destination_host,
            "connection_id": completion.connection_id,
            "secret_refs_used": completion.secret_refs_used,
        })];
        request.outputs = vec![payload.clone()];
        request.external_effects = completion
            .network_performed
            .then(|| {
                json!({
                    "kind": "network_websocket",
                    "destination_host": completion.destination_host,
                    "frames_in": completion.total_frames_in,
                    "frames_out": completion.total_frames_out,
                    "bytes_in": completion.total_bytes_in,
                    "bytes_out": completion.total_bytes_out,
                })
            })
            .into_iter()
            .collect();
        request.authority = Some(json!({
            "package_id": completion.package_id,
            "capability_id": completion.capability_id,
            "secret_refs_used": completion.secret_refs_used,
        }));
        request.policy_decision = Some(json!({
            "outcome": if status == EffectTerminalStatus::Denied {
                "denied"
            } else {
                "allowed"
            },
        }));
        request.parent_receipts = completion.parent_receipts.to_vec();
        request.planned = json!({
            "destination_host": completion.destination_host,
            "connection_id": completion.connection_id,
        });
        request.actual = payload.clone();
        self.record_effect_receipt(request).await
    }
}

fn outbound_terminal_status(status: &str) -> EffectTerminalStatus {
    match status.to_ascii_lowercase().as_str() {
        "ok" | "success" | "succeeded" | "completed" => EffectTerminalStatus::Succeeded,
        "denied" => EffectTerminalStatus::Denied,
        "cancelled" | "canceled" => EffectTerminalStatus::Cancelled,
        "timeout" | "timed_out" => EffectTerminalStatus::TimedOut,
        "partial" => EffectTerminalStatus::Partial,
        _ => EffectTerminalStatus::Failed,
    }
}

pub(crate) fn websocket_completed_payload(completion: &OutboundWebSocketCompletion<'_>) -> Value {
    json!({
        "id": completion.id,
        "package_id": completion.package_id,
        "capability_id": completion.capability_id,
        "destination_host": completion.destination_host,
        "connection_id": completion.connection_id,
        "code": completion.code,
        "reason": safe_websocket_reason(completion.reason),
        "total_frames_in": completion.total_frames_in,
        "total_frames_out": completion.total_frames_out,
        "total_bytes_in": completion.total_bytes_in,
        "total_bytes_out": completion.total_bytes_out,
        "duration_ms": completion.duration_ms,
        "executor_kind": completion.executor_kind,
        "network_performed": completion.network_performed,
        "redaction_state": completion.redaction_state,
        "secret_refs_used": completion.secret_refs_used,
        "correlation_id": completion.correlation_id,
    })
}

fn safe_websocket_reason(reason: &str) -> &'static str {
    match reason {
        "closed" => "closed",
        "remote_closed" => "remote_closed",
        "idle_timeout" => "idle_timeout",
        "max_duration_ms" => "max_duration_ms",
        "event_channel_ended" => "event_channel_ended",
        "open_failed" => "open_failed",
        "inbound_limit" => "inbound_limit",
        _ => "redacted",
    }
}

pub(crate) async fn append_outbound_completion_event<S: EventStore>(
    store: std::sync::Arc<S>,
    session_id: String,
    kind: &'static str,
    payload: Value,
) -> anyhow::Result<()> {
    use plurora_core::{EventEnvelope, KERNEL_PACKAGE_ID};

    let seq = store.next_sequence(&session_id).await?;
    store
        .append(EventEnvelope {
            id: new_id("evt"),
            session_id,
            sequence: seq,
            timestamp: chrono::Utc::now(),
            writer_package_id: KERNEL_PACKAGE_ID.to_string(),
            kind: kind.to_string(),
            schema_version: 1,
            payload,
            metadata: json!({}),
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use plurora_core::NetworkPermissions;

    fn make_perms(declarations: Vec<NetworkDeclaration>, hosts: Vec<String>) -> NetworkPermissions {
        NetworkPermissions {
            hosts,
            declarations,
        }
    }

    #[test]
    fn no_network_permission_denies() {
        let perms = make_perms(vec![], vec![]);
        let decision = check_network_policy(&perms, "api.example.com", "GET");
        assert!(!decision.allowed);
    }

    #[test]
    fn flat_hosts_allows_any_method() {
        let perms = make_perms(vec![], vec!["api.example.com".to_string()]);
        let decision = check_network_policy(&perms, "api.example.com", "POST");
        assert!(decision.allowed);
    }

    #[test]
    fn structured_declaration_allows_matching_host() {
        let perms = make_perms(
            vec![NetworkDeclaration {
                host: "api.example.com".to_string(),
                methods: vec!["GET".to_string(), "POST".to_string()],
                purpose: Some("model inference".to_string()),
            }],
            vec![],
        );
        let decision = check_network_policy(&perms, "api.example.com", "GET");
        assert!(decision.allowed);
        assert_eq!(
            decision.matched_declaration.unwrap().purpose,
            Some("model inference".to_string())
        );
    }

    #[test]
    fn structured_declaration_denies_non_matching_method() {
        let perms = make_perms(
            vec![NetworkDeclaration {
                host: "api.example.com".to_string(),
                methods: vec!["GET".to_string()],
                purpose: None,
            }],
            vec![],
        );
        let decision = check_network_policy(&perms, "api.example.com", "DELETE");
        assert!(!decision.allowed);
    }

    #[test]
    fn structured_declaration_empty_methods_allows_any() {
        let perms = make_perms(
            vec![NetworkDeclaration {
                host: "api.example.com".to_string(),
                methods: vec![],
                purpose: None,
            }],
            vec![],
        );
        let decision = check_network_policy(&perms, "api.example.com", "DELETE");
        assert!(decision.allowed);
    }

    #[test]
    fn wildcard_host_matches_subdomain() {
        let perms = make_perms(
            vec![NetworkDeclaration {
                host: "*.example.com".to_string(),
                methods: vec![],
                purpose: None,
            }],
            vec![],
        );
        let decision = check_network_policy(&perms, "api.example.com", "GET");
        assert!(decision.allowed);
    }

    #[test]
    fn wildcard_host_matches_base_domain() {
        let perms = make_perms(
            vec![NetworkDeclaration {
                host: "*.example.com".to_string(),
                methods: vec![],
                purpose: None,
            }],
            vec![],
        );
        let decision = check_network_policy(&perms, "example.com", "GET");
        assert!(decision.allowed);
    }

    #[test]
    fn wildcard_host_does_not_match_other_domain() {
        let perms = make_perms(
            vec![NetworkDeclaration {
                host: "*.example.com".to_string(),
                methods: vec![],
                purpose: None,
            }],
            vec![],
        );
        let decision = check_network_policy(&perms, "api.other.com", "GET");
        assert!(!decision.allowed);
    }

    #[test]
    fn structured_declarations_take_priority_over_flat_hosts() {
        let perms = make_perms(
            vec![NetworkDeclaration {
                host: "api.example.com".to_string(),
                methods: vec!["GET".to_string()],
                purpose: Some("structured".to_string()),
            }],
            vec!["api.example.com".to_string()],
        );
        // GET matches the structured declaration
        let decision = check_network_policy(&perms, "api.example.com", "GET");
        assert!(decision.allowed);
        assert_eq!(
            decision.matched_declaration.unwrap().purpose,
            Some("structured".to_string())
        );
    }

    #[test]
    fn method_matching_is_case_insensitive() {
        let perms = make_perms(
            vec![NetworkDeclaration {
                host: "api.example.com".to_string(),
                methods: vec!["get".to_string()],
                purpose: None,
            }],
            vec![],
        );
        let decision = check_network_policy(&perms, "api.example.com", "GET");
        assert!(decision.allowed);
    }

    #[test]
    fn network_policy_matches_websocket_when_declared() {
        let perms = make_perms(
            vec![NetworkDeclaration {
                host: "api.openai.com".to_string(),
                methods: vec!["WEBSOCKET".to_string()],
                purpose: Some("OpenAI Realtime".to_string()),
            }],
            vec![],
        );
        let decision = check_network_policy(&perms, "api.openai.com", "WEBSOCKET");
        assert!(decision.allowed);
        assert_eq!(
            decision.matched_declaration.unwrap().purpose,
            Some("OpenAI Realtime".to_string())
        );
    }

    #[test]
    fn network_policy_rejects_websocket_when_not_declared() {
        let perms = make_perms(
            vec![NetworkDeclaration {
                host: "api.openai.com".to_string(),
                methods: vec!["POST".to_string()],
                purpose: None,
            }],
            vec![],
        );
        let decision = check_network_policy(&perms, "api.openai.com", "WEBSOCKET");
        assert!(!decision.allowed);
    }

    #[test]
    fn exact_host_match() {
        assert!(host_matches("api.example.com", "api.example.com"));
        assert!(!host_matches("api.example.com", "other.example.com"));
    }

    #[test]
    fn port_specific_match() {
        assert!(host_matches("api.example.com:443", "api.example.com:443"));
        assert!(!host_matches("api.example.com:443", "api.example.com:80"));
    }
}
