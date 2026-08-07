use super::*;
use crate::runtime::effects::{principal_identity, EffectReceiptRequest};
use crate::DEFAULT_CONTRACT_PROFILE;
use plurora_core::{ArtifactDescriptor, EffectScope, EffectTerminalStatus, PrincipalIdentity};

const WEBSOCKET_METHOD: &str = "WEBSOCKET";

#[derive(Debug, Clone)]
struct WebSocketEffectContext {
    principal: PrincipalIdentity,
    package_id: String,
    capability_id: String,
    destination_host: String,
    connection_id: String,
    executor_kind: String,
    network_performed: bool,
    session_id: Option<String>,
    trace_id: String,
    parent_receipts: Vec<String>,
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    // --- Outbound ---

    pub(crate) async fn dispatch_outbound_audit(&self, params: &Value) -> anyhow::Result<Value> {
        let package_id = params
            .get("package_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("host.outbound.audit requires package_id"))?
            .to_string();
        Ok(serde_json::to_value(
            self.list_outbound_audit(&package_id).await?,
        )?)
    }

    pub(crate) async fn dispatch_outbound_execute(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        // --- L3: public outbound/secret boundary ---
        // Determine package_id from the protocol context principal.
        // The caller CANNOT self-assert a different package_id.
        let package_id = match &context.principal {
            ProtocolPrincipal::Package { package_id } => package_id.clone(),
            ProtocolPrincipal::HostAdmin | ProtocolPrincipal::HostDev => {
                // Host principals may supply package_id in params for testing.
                params
                    .get("package_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| "host/test".to_string())
            }
            other => {
                anyhow::bail!(
                    "host.outbound.execute requires package or host principal, got {:?}",
                    other
                )
            }
        };

        let capability_id = params
            .get("capability_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("host.outbound.execute requires capability_id"))?
            .to_string();
        if !capability_id.starts_with(&format!("{package_id}/")) {
            anyhow::bail!(
                "host.outbound.execute capability_id must belong to the caller package namespace"
            );
        }
        let destination_host = params
            .get("destination_host")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("host.outbound.execute requires destination_host"))?
            .to_string();
        let method = params
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("host.outbound.execute requires method"))?
            .to_string();
        let path: Option<String> = params
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_string);
        let purpose: Option<String> = params
            .get("purpose")
            .and_then(Value::as_str)
            .map(str::to_string);
        let secret_refs: Vec<String> = params
            .get("secret_refs")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let metadata = params.get("metadata").cloned().unwrap_or(Value::Null);
        let body_shape = params.get("body_shape").cloned();

        // L4: Parse secret_headers from params for host-side injection.
        // Format: { "Authorization": {"secret_ref": "...", "scheme": "bearer"} }
        let secret_headers_spec = parse_secret_headers(&params)?;

        // L5: Parse static_headers from params for safe non-secret header injection.
        // Format: { "anthropic-version": "2023-06-01" }
        // Only allowlisted header names are accepted; secret-bearing names are rejected.
        let static_headers = parse_static_headers(&params)?;

        // Collect secret_refs from both top-level and secret_headers
        let mut all_secret_refs = secret_refs.clone();
        for spec in &secret_headers_spec {
            if !all_secret_refs.contains(&spec.secret_ref) {
                all_secret_refs.push(spec.secret_ref.clone());
            }
        }

        // Y2: enforce manifest declarations for secret refs.
        // Any secret_ref used in top-level `secret_refs` or in
        // `secret_headers` must be declared in the caller package's
        // `permissions.secret_refs`. Undeclared refs → fail-closed.
        if !all_secret_refs.is_empty() {
            if self.is_contract_none_package(&package_id).await {
                anyhow::bail!(
                    "Foreign Capsule package '{}' cannot resolve kernel secret references",
                    package_id
                );
            }
            let manifest = self.packages.manifest(&package_id).await.ok_or_else(|| {
                anyhow::anyhow!(
                    "host.outbound.execute package '{}' is not loaded",
                    package_id
                )
            })?;
            let declared: std::collections::HashSet<&str> = manifest
                .permissions
                .secret_refs
                .iter()
                .map(|s| s.as_str())
                .collect();
            for secret_ref in &all_secret_refs {
                if !declared.contains(secret_ref.as_str()) {
                    anyhow::bail!(
                        "secret_ref '{}' is not declared in package manifest permissions.secret_refs",
                        secret_ref
                    );
                }
            }
        }

        let policy_request = crate::runtime::OutboundRequest {
            principal: context.principal.clone(),
            package_id: package_id.clone(),
            capability_id: capability_id.clone(),
            destination_host: destination_host.clone(),
            method: method.clone(),
            purpose: purpose.clone(),
            secret_refs_used: all_secret_refs.clone(),
            correlation_id: context.correlation_id,
        };

        // L4: Resolve secret_headers into resolved_secret_headers
        let mut resolved_secret_headers = Vec::new();
        for spec in &secret_headers_spec {
            HeaderName::from_bytes(spec.header_name.as_bytes()).map_err(|_| {
                anyhow::anyhow!("host.outbound.execute secret header name is invalid")
            })?;
            let raw_value = self
                .resolve_secret_ref_with_session(&spec.secret_ref, context.session_id.as_deref())
                .await
                .map_err(|_| {
                    anyhow::anyhow!("host.outbound.execute secret header is unavailable")
                })?;
            let header_value = match spec.scheme.to_lowercase().as_str() {
                "bearer" => format!("Bearer {}", raw_value),
                "basic" => format!("Basic {}", raw_value),
                "raw" | "" => raw_value,
                other => format!("{} {}", other, raw_value),
            };
            HeaderValue::from_str(&header_value).map_err(|_| {
                anyhow::anyhow!("host.outbound.execute secret header value is invalid")
            })?;
            resolved_secret_headers.push(crate::runtime::outbound::ResolvedSecretHeader {
                header_name: spec.header_name.clone(),
                value: crate::runtime::outbound::RedactedHeaderValue(header_value),
            });
        }

        let executor_request = crate::runtime::OutboundExecutorRequest {
            package_id: package_id.clone(),
            capability_id: capability_id.clone(),
            destination_host: destination_host.clone(),
            method: method.clone(),
            path,
            purpose,
            secret_refs: all_secret_refs.clone(),
            redaction_state: None,
            timeout_ms: params.get("timeout_ms").and_then(Value::as_u64),
            metadata,
            body_shape,
            secret_headers: secret_headers_spec,
            resolved_secret_headers,
            static_headers,
        };

        let response = self
            .execute_outbound_with_policy(policy_request, executor_request)
            .await?;

        // Strip any raw-secret-like fields from the response before
        // returning it to the caller. The OutboundExecutorResponse
        // struct is already content-free by design, but we do an
        // extra sweep to ensure conformance: no raw_secret,
        // api_key, Bearer, sk- patterns in the serialized output.
        let mut response_value = serde_json::to_value(&response)?;
        strip_raw_secrets_from_value(&mut response_value);
        Ok(response_value)
    }

    /// Y3: Dispatch `host.outbound.stream`.
    ///
    /// Performs the same permission checks as `host.outbound.execute`,
    /// then starts a platform stream and spawns the executor's `stream`
    /// method to emit frames asynchronously. Returns a stream_id
    /// that the caller subscribes to via the existing event stream.
    pub(crate) async fn dispatch_outbound_stream(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        // --- Same auth/permission checks as dispatch_outbound_execute ---
        let package_id = match &context.principal {
            ProtocolPrincipal::Package { package_id } => package_id.clone(),
            ProtocolPrincipal::HostAdmin | ProtocolPrincipal::HostDev => params
                .get("package_id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| "host/test".to_string()),
            other => {
                anyhow::bail!(
                    "host.outbound.stream requires package or host principal, got {:?}",
                    other
                )
            }
        };

        let capability_id = params
            .get("capability_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("host.outbound.stream requires capability_id"))?
            .to_string();
        if !capability_id.starts_with(&format!("{package_id}/")) {
            anyhow::bail!(
                "host.outbound.stream capability_id must belong to the caller package namespace"
            );
        }
        let destination_host = params
            .get("destination_host")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("host.outbound.stream requires destination_host"))?
            .to_string();
        let method = params
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("host.outbound.stream requires method"))?
            .to_string();
        let path: Option<String> = params
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_string);
        let purpose: Option<String> = params
            .get("purpose")
            .and_then(Value::as_str)
            .map(str::to_string);
        let secret_refs: Vec<String> = params
            .get("secret_refs")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let metadata = params.get("metadata").cloned().unwrap_or(Value::Null);
        let body_shape = params.get("body_shape").cloned();

        // Y3: Host-level stream policy mirrors outbound.execute fail-closed
        // profile behavior. The default profile has this disabled, so stream
        // requests are rejected before any stream is registered or executor is
        // called.
        let stream_policy = &self.config.outbound_execute_policy;
        if !stream_policy.enabled {
            anyhow::bail!("host policy has not enabled outbound.stream");
        }
        if stream_policy.allow_redirects {
            anyhow::bail!("outbound.stream redirects are disabled");
        }
        if stream_policy.allowed_hosts.is_empty()
            || !stream_policy
                .allowed_hosts
                .iter()
                .any(|allowed| outbound_host_matches(allowed, &destination_host))
        {
            anyhow::bail!(
                "host policy does not allow outbound.stream host '{}'",
                destination_host
            );
        }
        if stream_policy.https_only {
            validate_outbound_stream_https_only(
                &destination_host,
                path.as_deref(),
                &metadata,
                stream_policy.allow_insecure_loopback_for_tests,
            )?;
        } else {
            anyhow::bail!("host policy attempted to disable HTTPS-only outbound.stream");
        }

        // Parse secret_headers (same as execute)
        let secret_headers_spec = parse_secret_headers(&params)?;

        // Parse static_headers (same as execute)
        let static_headers = parse_static_headers(&params)?;

        // Collect all secret_refs
        let mut all_secret_refs = secret_refs.clone();
        for spec in &secret_headers_spec {
            if !all_secret_refs.contains(&spec.secret_ref) {
                all_secret_refs.push(spec.secret_ref.clone());
            }
        }

        // Y2: enforce manifest declarations for secret refs
        if !all_secret_refs.is_empty() {
            if self.is_contract_none_package(&package_id).await {
                anyhow::bail!(
                    "Foreign Capsule package '{}' cannot resolve kernel secret references",
                    package_id
                );
            }
            let manifest = self.packages.manifest(&package_id).await.ok_or_else(|| {
                anyhow::anyhow!(
                    "host.outbound.stream package '{}' is not loaded",
                    package_id
                )
            })?;
            let declared: std::collections::HashSet<&str> = manifest
                .permissions
                .secret_refs
                .iter()
                .map(|s| s.as_str())
                .collect();
            for secret_ref in &all_secret_refs {
                if !declared.contains(secret_ref.as_str()) {
                    anyhow::bail!(
                        "secret_ref '{}' is not declared in package manifest permissions.secret_refs",
                        secret_ref
                    );
                }
            }
        }

        // Y3: Parse stream_format
        let stream_format_str = params
            .get("stream_format")
            .and_then(Value::as_str)
            .unwrap_or("sse");
        let stream_format = match stream_format_str {
            "sse" => crate::runtime::outbound::StreamFormat::Sse,
            "ndjson" => crate::runtime::outbound::StreamFormat::Ndjson,
            "raw" => crate::runtime::outbound::StreamFormat::Raw,
            other => anyhow::bail!("host.outbound.stream unknown stream_format '{other}'"),
        };
        let max_frame_bytes = params
            .get("max_frame_bytes")
            .and_then(Value::as_u64)
            .map(|v| v as usize);
        let max_total_bytes = params
            .get("max_total_bytes")
            .and_then(Value::as_u64)
            .map(|v| v as usize);
        let max_duration_ms = params.get("max_duration_ms").and_then(Value::as_u64);

        // Build the policy request (same checks as execute)
        let policy_request = crate::runtime::OutboundRequest {
            principal: context.principal.clone(),
            package_id: package_id.clone(),
            capability_id: capability_id.clone(),
            destination_host: destination_host.clone(),
            method: method.clone(),
            purpose: purpose.clone(),
            secret_refs_used: all_secret_refs.clone(),
            correlation_id: context.correlation_id,
        };

        // Resolve secret headers
        let mut resolved_secret_headers = Vec::new();
        for spec in &secret_headers_spec {
            reqwest::header::HeaderName::from_bytes(spec.header_name.as_bytes()).map_err(|_| {
                anyhow::anyhow!("host.outbound.stream secret header name is invalid")
            })?;
            let raw_value = self
                .resolve_secret_ref_with_session(&spec.secret_ref, context.session_id.as_deref())
                .await
                .map_err(|_| {
                    anyhow::anyhow!("host.outbound.stream secret header is unavailable")
                })?;
            let header_value = match spec.scheme.to_lowercase().as_str() {
                "bearer" => format!("Bearer {}", raw_value),
                "basic" => format!("Basic {}", raw_value),
                "raw" | "" => raw_value,
                other => format!("{} {}", other, raw_value),
            };
            reqwest::header::HeaderValue::from_str(&header_value).map_err(|_| {
                anyhow::anyhow!("host.outbound.stream secret header value is invalid")
            })?;
            resolved_secret_headers.push(crate::runtime::outbound::ResolvedSecretHeader {
                header_name: spec.header_name.clone(),
                value: crate::runtime::outbound::RedactedHeaderValue(header_value),
            });
        }

        // Run the policy check
        let _audit_record = self.check_and_audit_outbound(policy_request).await?;

        // Build the executor request
        let executor_request = crate::runtime::OutboundExecutorRequest {
            package_id: package_id.clone(),
            capability_id: capability_id.clone(),
            destination_host: destination_host.clone(),
            method: method.clone(),
            path,
            purpose,
            secret_refs: all_secret_refs.clone(),
            redaction_state: None,
            timeout_ms: params.get("timeout_ms").and_then(Value::as_u64),
            metadata,
            body_shape,
            secret_headers: secret_headers_spec,
            resolved_secret_headers,
            static_headers,
        };

        // Start a platform stream via the existing streaming infrastructure.
        // We create a synthetic "capability" for the outbound stream.
        let outbound_capability_id = capability_id.clone();
        let session_id = format!("platform_outbound_stream_{}", package_id.replace('/', "_"));

        // Register the stream in the StreamRegistry
        let stream_record = self
            .streams
            .start_invocation(
                outbound_capability_id.clone(),
                package_id.clone(),
                session_id.clone(),
                serde_json::json!({
                    "destination_host": destination_host,
                    "method": method,
                    "stream_format": stream_format_str,
                }),
            )
            .await;

        // Emit capability/stream.started event
        let event_payload = json!({
            "invocation_id": stream_record.invocation_id,
            "stream_id": stream_record.stream_id,
            "capability_id": outbound_capability_id,
            "provider_package_id": package_id,
            "session_id": session_id,
        });
        self.append_platform_event(
            &session_id,
            plurora_core::EVENT_STREAM_STARTED,
            event_payload,
        )
        .await?;

        // Create cancel signal
        let (cancel_tx, cancel_rx) = crate::runtime::outbound::CancelSignal::new();

        // Store the cancel sender so that capability.cancel can set it
        let invocation_id = stream_record.invocation_id.clone();
        let stream_id = stream_record.stream_id.clone();

        // Determine executor kind for the response
        let executor = self.outbound_executor();
        let executor_kind = match &self.config.outbound_executor {
            crate::runtime::outbound::OutboundExecutorConfig::DenyAll => {
                crate::runtime::outbound::ExecutorKind::DenyAll
            }
            crate::runtime::outbound::OutboundExecutorConfig::Custom(_) => {
                crate::runtime::outbound::ExecutorKind::Fake
            }
            crate::runtime::outbound::OutboundExecutorConfig::LiveHttp(_) => {
                crate::runtime::outbound::ExecutorKind::Real
            }
        };
        let network_performed =
            matches!(executor_kind, crate::runtime::outbound::ExecutorKind::Real);

        // Create a StreamEmitter that feeds into the platform stream lifecycle
        let emitter = Arc::new(StreamEmitterAdapter {
            streams: self.streams.clone(),
            store: self.store.clone(),
            session_id: session_id.clone(),
            invocation_id: invocation_id.clone(),
            stream_id: stream_id.clone(),
            cancel_tx,
        });

        // Spawn the executor stream in a background task
        let executor_for_task = executor.clone();
        let invocation_id_for_end = invocation_id.clone();
        let session_id_for_end = session_id.clone();
        let streams_for_end = self.streams.clone();
        let store_for_end = self.store.clone();
        let runtime_for_end = self.clone();
        let stream_id_for_end = stream_id.clone();
        let context_principal = context.principal.clone();
        let pkg_id_for_end = package_id.clone();
        let cap_id_for_end = outbound_capability_id.clone();
        let host_for_end = destination_host.clone();
        let method_for_end = method.clone();
        let format_str_for_end = stream_format_str.to_string();
        let secret_refs_for_end = all_secret_refs.clone();
        let correlation_id_for_end = context.correlation_id;
        let completion_id_for_end = plurora_core::new_id("obc");
        let started_for_end = Instant::now();
        let executor_kind_for_error = executor_kind;
        let network_performed_for_error = network_performed;

        tokio::spawn(async move {
            let result = executor_for_task
                .stream(
                    executor_request,
                    stream_format,
                    emitter.clone(),
                    cancel_rx,
                    max_frame_bytes,
                    max_total_bytes,
                    max_duration_ms,
                )
                .await;

            // Helper closure for appending platform events
            let append_event = |kind: &'static str, payload: Value| {
                let store = store_for_end.clone();
                let session_id = session_id_for_end.clone();
                async move {
                    use plurora_core::{new_id, EventEnvelope, PLATFORM_RUNTIME_ID};
                    let seq = store.next_sequence(&session_id).await.unwrap_or(0);
                    let event = EventEnvelope {
                        id: new_id("evt"),
                        session_id,
                        sequence: seq,
                        timestamp: chrono::Utc::now(),
                        writer_package_id: PLATFORM_RUNTIME_ID.to_string(),
                        kind: kind.to_string(),
                        schema_version: 1,
                        payload,
                        metadata: json!({}),
                    };
                    let _ = store.append(event).await;
                }
            };

            // On stream end, write the terminal state and audit
            match result {
                Ok(summary) => {
                    let final_termination = match summary.status.as_str() {
                        "cancelled" => "cancelled",
                        "timeout" => "timeout",
                        "error" => "error",
                        _ => "ended",
                    };
                    let duration_ms = started_for_end.elapsed().as_millis() as u64;
                    let receipt = runtime_for_end
                        .emit_outbound_stream_completed(
                            &session_id_for_end,
                            crate::runtime::OutboundStreamCompletion {
                                id: &completion_id_for_end,
                                package_id: &pkg_id_for_end,
                                capability_id: &cap_id_for_end,
                                destination_host: &host_for_end,
                                method: &method_for_end,
                                stream_format: &format_str_for_end,
                                status: &summary.status,
                                total_chunks: summary.frame_count,
                                total_bytes: summary.bytes_received,
                                duration_ms,
                                final_termination,
                                executor_kind: crate::runtime::outbound::executor_kind_str(
                                    summary.executor_kind,
                                ),
                                network_performed: summary.network_performed,
                                redaction_state: summary.redaction_state,
                                secret_refs_used: &secret_refs_for_end,
                                correlation_id: correlation_id_for_end,
                            },
                        )
                        .await
                        .ok();
                    let terminal_transition = if receipt.is_some() {
                        match final_termination {
                            "cancelled" => {
                                streams_for_end
                                    .cancel_invocation(&invocation_id_for_end)
                                    .await
                            }
                            "timeout" => {
                                streams_for_end
                                    .timeout_invocation(&invocation_id_for_end)
                                    .await
                            }
                            "error" => {
                                streams_for_end
                                    .error_invocation(
                                        &invocation_id_for_end,
                                        "outbound stream failed",
                                    )
                                    .await
                            }
                            _ => streams_for_end.end_invocation(&invocation_id_for_end).await,
                        }
                    } else {
                        Err(anyhow::anyhow!("outbound stream receipt was not recorded"))
                    };

                    if terminal_transition.is_ok() {
                        let stream_event_kind = match final_termination {
                            "cancelled" => plurora_core::EVENT_STREAM_CANCELLED,
                            "timeout" => plurora_core::EVENT_STREAM_TIMEOUT,
                            "error" => plurora_core::EVENT_STREAM_ERROR,
                            _ => plurora_core::EVENT_STREAM_ENDED,
                        };
                        append_event(
                            stream_event_kind,
                            json!({
                                "invocation_id": invocation_id_for_end,
                                "stream_id": stream_id_for_end,
                                "status": summary.status,
                                "frame_count": summary.frame_count,
                                "bytes_received": summary.bytes_received,
                                "executor_kind": summary.executor_kind,
                                "receipt": receipt,
                            }),
                        )
                        .await;
                    }

                    // Emit outbound audit record
                    append_event(plurora_core::EVENT_OUTBOUND_REQUEST, json!({
                        "principal": context_principal,
                        "package_id": pkg_id_for_end,
                        "capability_id": cap_id_for_end,
                        "destination_host": host_for_end,
                        "method": method_for_end,
                        "status": summary.status,
                        "stream_format": format_str_for_end,
                        "frame_count": summary.frame_count,
                        "bytes_received": summary.bytes_received,
                        "redaction_state": serde_json::to_value(summary.redaction_state).unwrap_or(Value::Null),
                        "executor_kind": summary.executor_kind,
                        "network_performed": summary.network_performed,
                    })).await;
                }
                Err(_error) => {
                    let duration_ms = started_for_end.elapsed().as_millis() as u64;
                    let receipt = runtime_for_end
                        .emit_outbound_stream_completed(
                            &session_id_for_end,
                            crate::runtime::OutboundStreamCompletion {
                                id: &completion_id_for_end,
                                package_id: &pkg_id_for_end,
                                capability_id: &cap_id_for_end,
                                destination_host: &host_for_end,
                                method: &method_for_end,
                                stream_format: &format_str_for_end,
                                status: "error",
                                total_chunks: 0,
                                total_bytes: 0,
                                duration_ms,
                                final_termination: "error",
                                executor_kind: crate::runtime::outbound::executor_kind_str(
                                    executor_kind_for_error,
                                ),
                                network_performed: network_performed_for_error,
                                redaction_state: plurora_core::RedactionState::Redacted,
                                secret_refs_used: &secret_refs_for_end,
                                correlation_id: correlation_id_for_end,
                            },
                        )
                        .await
                        .ok();
                    let terminal_transition = if receipt.is_some() {
                        streams_for_end
                            .error_invocation(
                                &invocation_id_for_end,
                                "outbound stream executor failed",
                            )
                            .await
                    } else {
                        Err(anyhow::anyhow!("outbound stream receipt was not recorded"))
                    };

                    if terminal_transition.is_ok() {
                        append_event(
                            plurora_core::EVENT_STREAM_ERROR,
                            json!({
                                "invocation_id": invocation_id_for_end,
                                "stream_id": stream_id_for_end,
                                "error_code": "executor_failed",
                                "error_present": true,
                                "receipt": receipt,
                            }),
                        )
                        .await;
                    }
                }
            }
        });

        // Return the stream response immediately
        let response = crate::runtime::outbound::OutboundStreamResponse {
            stream_id: stream_record.stream_id.clone(),
            status: crate::runtime::outbound::StreamStartStatus::Ok,
            redaction_state: RedactionState::Redacted,
            network_performed,
            executor_kind,
        };

        let mut response_value = serde_json::to_value(&response)?;
        strip_raw_secrets_from_value(&mut response_value);
        Ok(response_value)
    }

    pub(crate) async fn dispatch_outbound_websocket_open(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let package_id = match &context.principal {
            ProtocolPrincipal::Package { package_id } => package_id.clone(),
            ProtocolPrincipal::HostAdmin | ProtocolPrincipal::HostDev => params
                .get("package_id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| "host/test".to_string()),
            other => anyhow::bail!(
                "host.outbound.websocket.open requires package or host principal, got {:?}",
                other
            ),
        };

        let capability_id = params
            .get("capability_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("host.outbound.websocket.open requires capability_id"))?
            .to_string();
        if !capability_id.starts_with(&format!("{package_id}/")) {
            anyhow::bail!(
                "host.outbound.websocket.open capability_id must belong to the caller package namespace"
            );
        }
        let destination_host = params
            .get("destination_host")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                anyhow::anyhow!("host.outbound.websocket.open requires destination_host")
            })?
            .to_string();
        let path = params
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_string);
        let purpose = params
            .get("purpose")
            .and_then(Value::as_str)
            .map(str::to_string);
        let mut metadata = params.get("metadata").cloned().unwrap_or(Value::Null);
        let subprotocols: Vec<String> = params
            .get("subprotocols")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let secret_refs: Vec<String> = params
            .get("secret_refs")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let secret_headers_spec = parse_secret_headers(&params)?;
        let static_headers_spec = parse_static_headers(&params)?;
        let mut all_secret_refs = secret_refs.clone();
        for spec in &secret_headers_spec {
            if !all_secret_refs.contains(&spec.secret_ref) {
                all_secret_refs.push(spec.secret_ref.clone());
            }
        }

        if !all_secret_refs.is_empty() {
            if self.is_contract_none_package(&package_id).await {
                anyhow::bail!(
                    "Foreign Capsule package '{}' cannot resolve kernel secret references",
                    package_id
                );
            }
            let manifest = self.packages.manifest(&package_id).await.ok_or_else(|| {
                anyhow::anyhow!(
                    "host.outbound.websocket.open package '{}' is not loaded",
                    package_id
                )
            })?;
            let declared: std::collections::HashSet<&str> = manifest
                .permissions
                .secret_refs
                .iter()
                .map(|s| s.as_str())
                .collect();
            for secret_ref in &all_secret_refs {
                if !declared.contains(secret_ref.as_str()) {
                    anyhow::bail!(
                        "secret_ref '{}' is not declared in package manifest permissions.secret_refs",
                        secret_ref
                    );
                }
            }
        }

        let policy_request = crate::runtime::OutboundRequest {
            principal: context.principal.clone(),
            package_id: package_id.clone(),
            capability_id: capability_id.clone(),
            destination_host: destination_host.clone(),
            method: WEBSOCKET_METHOD.to_string(),
            purpose: purpose.clone(),
            secret_refs_used: all_secret_refs.clone(),
            correlation_id: context.correlation_id,
        };
        let _audit_record = self.check_and_audit_outbound(policy_request).await?;

        let mut secret_headers = HashMap::new();
        for spec in &secret_headers_spec {
            HeaderName::from_bytes(spec.header_name.as_bytes()).map_err(|_| {
                anyhow::anyhow!("host.outbound.websocket.open secret header name is invalid")
            })?;
            let raw_value = self
                .resolve_secret_ref_with_session(&spec.secret_ref, context.session_id.as_deref())
                .await
                .map_err(|_| {
                    anyhow::anyhow!("host.outbound.websocket.open secret header is unavailable")
                })?;
            let header_value = match spec.scheme.to_lowercase().as_str() {
                "bearer" => format!("Bearer {}", raw_value),
                "basic" => format!("Basic {}", raw_value),
                "raw" | "" => raw_value,
                other => format!("{} {}", other, raw_value),
            };
            HeaderValue::from_str(&header_value).map_err(|_| {
                anyhow::anyhow!("host.outbound.websocket.open secret header value is invalid")
            })?;
            secret_headers.insert(spec.header_name.clone(), header_value);
        }
        let static_headers = static_headers_spec
            .into_iter()
            .map(|hdr| (hdr.name, hdr.value))
            .collect::<HashMap<_, _>>();

        let session_id = format!(
            "platform_outbound_websocket_{}",
            package_id.replace('/', "_")
        );
        let stream_record = self
            .streams
            .start_invocation(
                capability_id.clone(),
                package_id.clone(),
                session_id.clone(),
                json!({
                    "destination_host": destination_host,
                    "method": WEBSOCKET_METHOD,
                }),
            )
            .await;
        self.append_platform_event(
            &session_id,
            plurora_core::EVENT_STREAM_STARTED,
            json!({
                "invocation_id": stream_record.invocation_id,
                "stream_id": stream_record.stream_id,
                "capability_id": capability_id,
                "provider_package_id": package_id,
                "session_id": session_id,
            }),
        )
        .await?;

        metadata = match metadata {
            Value::Object(mut map) => {
                map.insert(
                    "connection_id".to_string(),
                    Value::String(stream_record.stream_id.clone()),
                );
                Value::Object(map)
            }
            other => json!({"connection_id": stream_record.stream_id, "request_metadata": other}),
        };

        let subprotocol_count = subprotocols.len();
        let req = crate::runtime::OutboundWebSocketOpenRequest {
            capability_id: capability_id.clone(),
            package_id: package_id.clone(),
            destination_host: destination_host.clone(),
            path,
            purpose,
            subprotocols,
            secret_refs: all_secret_refs.clone(),
            metadata,
            static_headers,
            secret_headers,
            max_frame_bytes: params
                .get("max_frame_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(65_536) as usize,
            max_total_bytes_inbound: params
                .get("max_total_bytes_inbound")
                .and_then(Value::as_u64)
                .unwrap_or(10 * 1024 * 1024) as usize,
            max_total_bytes_outbound: params
                .get("max_total_bytes_outbound")
                .and_then(Value::as_u64)
                .unwrap_or(10 * 1024 * 1024) as usize,
            max_idle_ms: params
                .get("max_idle_ms")
                .and_then(Value::as_u64)
                .unwrap_or(60_000),
            max_duration_ms: params
                .get("max_duration_ms")
                .and_then(Value::as_u64)
                .unwrap_or(1_800_000),
        };
        let open_started = Instant::now();
        let open_started_at = chrono::Utc::now();
        let session = match self.outbound_websocket_executor().open(req).await {
            Ok(session) => session,
            Err(error) => {
                let denied = error.to_string().to_ascii_lowercase().contains("denied");
                let code = if denied { 1008 } else { 1011 };
                let reason = if denied { "open_denied" } else { "open_failed" };
                let no_parents: Vec<String> = Vec::new();
                let completion_id = plurora_core::new_id("obc");
                let receipt = self
                    .emit_outbound_websocket_completed(
                        &session_id,
                        crate::runtime::OutboundWebSocketCompletion {
                            id: &completion_id,
                            package_id: &package_id,
                            capability_id: &capability_id,
                            destination_host: &destination_host,
                            connection_id: &stream_record.stream_id,
                            code,
                            reason,
                            total_frames_in: 0,
                            total_frames_out: 0,
                            total_bytes_in: 0,
                            total_bytes_out: 0,
                            duration_ms: open_started.elapsed().as_millis() as u64,
                            executor_kind: "configured",
                            network_performed: false,
                            redaction_state: RedactionState::NotCaptured,
                            secret_refs_used: &all_secret_refs,
                            correlation_id: context.correlation_id,
                            parent_receipts: &no_parents,
                        },
                    )
                    .await?;
                let _ = self
                    .streams
                    .error_invocation(&stream_record.invocation_id, reason)
                    .await;
                self.append_platform_event(
                    &session_id,
                    plurora_core::EVENT_OUTBOUND_WEBSOCKET_ERROR,
                    json!({
                        "connection_id": stream_record.stream_id,
                        "error_code": reason,
                        "message_redacted": reason,
                        "receipt": receipt.clone(),
                    }),
                )
                .await?;
                self.append_platform_event(
                    &session_id,
                    plurora_core::EVENT_STREAM_ERROR,
                    json!({
                        "invocation_id": stream_record.invocation_id,
                        "stream_id": stream_record.stream_id,
                        "error": reason,
                        "receipt": receipt,
                    }),
                )
                .await?;
                return Err(error);
            }
        };
        let open_effect_context = WebSocketEffectContext {
            principal: principal_identity(&context.principal),
            package_id: package_id.clone(),
            capability_id: capability_id.clone(),
            destination_host: destination_host.clone(),
            connection_id: session.connection_id.clone(),
            executor_kind: format!("{:?}", session.executor_kind).to_ascii_lowercase(),
            network_performed: session.network_performed,
            session_id: context.session_id.clone(),
            trace_id: context.effective_correlation_id().to_string(),
            parent_receipts: Vec::new(),
        };
        let open_receipt = self
            .record_websocket_operation_effect(
                &open_effect_context,
                "outbound.websocket.open",
                EffectTerminalStatus::Succeeded,
                open_started_at,
                open_started.elapsed().as_millis() as u64,
                json!({
                    "operation": "open",
                    "subprotocol_count": subprotocol_count,
                    "secret_refs_used": all_secret_refs,
                }),
                json!({
                    "connection_id": session.connection_id,
                    "subprotocol_negotiated": session.subprotocol_negotiated,
                    "network_performed": session.network_performed,
                    "executor_kind": session.executor_kind,
                }),
            )
            .await?;
        self.streams
            .set_metadata_field(
                &stream_record.invocation_id,
                "open_receipt",
                Value::String(open_receipt.digest.clone()),
            )
            .await?;
        self.streams
            .set_metadata_field(
                &stream_record.invocation_id,
                "executor_kind",
                Value::String(open_effect_context.executor_kind.clone()),
            )
            .await?;
        self.streams
            .set_metadata_field(
                &stream_record.invocation_id,
                "network_performed",
                Value::Bool(session.network_performed),
            )
            .await?;
        let response = json!({
            "connection_id": session.connection_id,
            "status": "ok",
            "subprotocol_negotiated": session.subprotocol_negotiated,
            "redaction_state": session.redaction_state,
            "network_performed": session.network_performed,
            "executor_kind": session.executor_kind,
            "receipt": open_receipt.clone(),
        });
        let store = self.store.clone();
        let streams = self.streams.clone();
        let session_id_for_task = session_id.clone();
        let invocation_id_for_task = stream_record.invocation_id.clone();
        let stream_id_for_task = stream_record.stream_id.clone();
        let completion_id_for_task = plurora_core::new_id("obc");
        let correlation_id_for_task = context.correlation_id;
        let pkg_id_for_task = package_id.clone();
        let cap_id_for_task = capability_id.clone();
        let host_for_task = destination_host.clone();
        let executor_kind_for_task = session.executor_kind;
        let network_performed_for_task = session.network_performed;
        let redaction_state_for_task = session.redaction_state;
        let secret_refs_for_task = all_secret_refs.clone();
        let open_receipt_for_task = open_receipt.clone();
        let runtime_for_task = self.clone();
        let mut events = session.events;
        tokio::spawn(async move {
            let append_event = |kind: &'static str, payload: Value| {
                let store = store.clone();
                let session_id = session_id_for_task.clone();
                async move {
                    use plurora_core::{new_id, EventEnvelope, PLATFORM_RUNTIME_ID};
                    let seq = store.next_sequence(&session_id).await.unwrap_or(0);
                    let _ = store
                        .append(EventEnvelope {
                            id: new_id("evt"),
                            session_id,
                            sequence: seq,
                            timestamp: chrono::Utc::now(),
                            writer_package_id: PLATFORM_RUNTIME_ID.to_string(),
                            kind: kind.to_string(),
                            schema_version: 1,
                            payload,
                            metadata: json!({}),
                        })
                        .await;
                }
            };
            let parent_receipts = vec![open_receipt_for_task.digest.clone()];
            let mut terminal_seen = false;
            while let Some(event) = events.recv().await {
                match event {
                    WebSocketEvent::Closed {
                        connection_id,
                        code,
                        reason,
                        total_frames_in,
                        total_frames_out,
                        total_bytes_in,
                        total_bytes_out,
                        duration_ms,
                    } => {
                        let receipt = runtime_for_task
                            .emit_outbound_websocket_completed(
                                &session_id_for_task,
                                crate::runtime::OutboundWebSocketCompletion {
                                    id: &completion_id_for_task,
                                    package_id: &pkg_id_for_task,
                                    capability_id: &cap_id_for_task,
                                    destination_host: &host_for_task,
                                    connection_id: &connection_id,
                                    code,
                                    reason: &reason,
                                    total_frames_in,
                                    total_frames_out,
                                    total_bytes_in,
                                    total_bytes_out,
                                    duration_ms,
                                    executor_kind: crate::runtime::outbound::executor_kind_str(
                                        executor_kind_for_task,
                                    ),
                                    network_performed: network_performed_for_task,
                                    redaction_state: redaction_state_for_task,
                                    secret_refs_used: &secret_refs_for_task,
                                    correlation_id: correlation_id_for_task,
                                    parent_receipts: &parent_receipts,
                                },
                            )
                            .await
                            .ok();
                        let terminal_transition = if receipt.is_some() {
                            match code {
                                1000 => streams.end_invocation(&invocation_id_for_task).await,
                                1001 => streams.cancel_invocation(&invocation_id_for_task).await,
                                1013 => streams.timeout_invocation(&invocation_id_for_task).await,
                                _ => {
                                    streams
                                        .error_invocation(
                                            &invocation_id_for_task,
                                            "websocket terminated with an error",
                                        )
                                        .await
                                }
                            }
                        } else {
                            Err(anyhow::anyhow!("websocket receipt was not recorded"))
                        };
                        if terminal_transition.is_ok() {
                            let stream_event_kind = match code {
                                1000 => plurora_core::EVENT_STREAM_ENDED,
                                1001 => plurora_core::EVENT_STREAM_CANCELLED,
                                1013 => plurora_core::EVENT_STREAM_TIMEOUT,
                                _ => plurora_core::EVENT_STREAM_ERROR,
                            };
                            append_event(
                                stream_event_kind,
                                json!({
                                    "invocation_id": invocation_id_for_task,
                                    "stream_id": stream_id_for_task,
                                    "receipt": receipt,
                                }),
                            )
                            .await;
                        }
                        terminal_seen = true;
                        break;
                    }
                    other => {
                        let (kind, mut payload, _) = websocket_event_to_platform_event(other);
                        if kind == plurora_core::EVENT_OUTBOUND_WEBSOCKET_OPENED {
                            if let Value::Object(map) = &mut payload {
                                map.insert(
                                    "receipt".to_string(),
                                    serde_json::to_value(&open_receipt_for_task)
                                        .unwrap_or(Value::Null),
                                );
                            }
                        }
                        append_event(kind, payload).await;
                    }
                }
            }
            if !terminal_seen {
                let receipt = runtime_for_task
                    .emit_outbound_websocket_completed(
                        &session_id_for_task,
                        crate::runtime::OutboundWebSocketCompletion {
                            id: &completion_id_for_task,
                            package_id: &pkg_id_for_task,
                            capability_id: &cap_id_for_task,
                            destination_host: &host_for_task,
                            connection_id: &stream_id_for_task,
                            code: 1011,
                            reason: "event_channel_ended",
                            total_frames_in: 0,
                            total_frames_out: 0,
                            total_bytes_in: 0,
                            total_bytes_out: 0,
                            duration_ms: 1,
                            executor_kind: crate::runtime::outbound::executor_kind_str(
                                executor_kind_for_task,
                            ),
                            network_performed: network_performed_for_task,
                            redaction_state: redaction_state_for_task,
                            secret_refs_used: &secret_refs_for_task,
                            correlation_id: correlation_id_for_task,
                            parent_receipts: &parent_receipts,
                        },
                    )
                    .await
                    .ok();
                let terminal_transition = if receipt.is_some() {
                    streams
                        .error_invocation(&invocation_id_for_task, "websocket event channel ended")
                        .await
                } else {
                    Err(anyhow::anyhow!("websocket receipt was not recorded"))
                };
                if terminal_transition.is_ok() {
                    append_event(
                        plurora_core::EVENT_STREAM_ERROR,
                        json!({
                            "invocation_id": invocation_id_for_task,
                            "stream_id": stream_id_for_task,
                            "error_code": "websocket_event_channel_ended",
                            "error_present": true,
                            "receipt": receipt,
                        }),
                    )
                    .await;
                }
            }
        });
        let mut response_value = response;
        strip_raw_secrets_from_value(&mut response_value);
        Ok(response_value)
    }

    async fn record_websocket_operation_effect(
        &self,
        context: &WebSocketEffectContext,
        effect_kind: &str,
        status: EffectTerminalStatus,
        started_at: chrono::DateTime<chrono::Utc>,
        duration_ms: u64,
        input: Value,
        output: Value,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let mut request = EffectReceiptRequest::live(
            effect_kind,
            context.principal.clone(),
            json!({
                "kind": "outbound_websocket_executor",
                "executor_kind": context.executor_kind,
                "capability_id": context.capability_id,
                "provider_package_id": context.package_id,
            }),
            status,
            started_at,
            duration_ms.max(1),
            context.trace_id.clone(),
        );
        request.protocol_profiles = vec![DEFAULT_CONTRACT_PROFILE.to_string()];
        request.inputs = vec![input.clone()];
        request.outputs = vec![output.clone()];
        request.external_effects = context
            .network_performed
            .then(|| {
                json!({
                    "kind": "network_websocket_operation",
                    "destination_host": context.destination_host,
                    "connection_id": context.connection_id,
                })
            })
            .into_iter()
            .collect();
        request.authority = Some(json!({
            "package_id": context.package_id,
            "capability_id": context.capability_id,
        }));
        request.policy_decision = Some(json!({
            "outcome": if status == EffectTerminalStatus::Denied {
                "denied"
            } else {
                "allowed"
            },
        }));
        request.parent_receipts = context.parent_receipts.clone();
        request.scope = EffectScope {
            session_id: context.session_id.clone(),
            branch_id: None,
        };
        request.planned = input;
        request.actual = output;
        self.record_effect_receipt(request).await
    }

    pub(crate) async fn dispatch_outbound_websocket_send(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let connection_id = params
            .get("connection_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                anyhow::anyhow!("host.outbound.websocket.send requires connection_id")
            })?;
        let frame = parse_websocket_frame(params)?;
        let (frame_kind, frame_bytes) = match &frame {
            OutboundWebSocketFrame::Text(text) => ("text", text.len()),
            OutboundWebSocketFrame::Binary(bytes) => ("binary", bytes.len()),
        };
        let effect_context = websocket_effect_context(self, context, connection_id).await;
        let started = Instant::now();
        let started_at = chrono::Utc::now();
        let status = match self
            .outbound_websocket_executor()
            .send(connection_id, frame)
            .await
        {
            Ok(status) => status,
            Err(error) => {
                let receipt = self
                    .record_websocket_operation_effect(
                        &effect_context,
                        "outbound.websocket.send",
                        EffectTerminalStatus::Failed,
                        started_at,
                        started.elapsed().as_millis() as u64,
                        json!({
                            "operation": "send",
                            "connection_id": connection_id,
                            "frame_kind": frame_kind,
                            "frame_bytes": frame_bytes,
                        }),
                        json!({"status": "failed"}),
                    )
                    .await?;
                if let Some(record) = self
                    .streams
                    .get_invocation_by_stream_id(connection_id)
                    .await
                {
                    self.append_platform_event(
                        &record.session_id,
                        plurora_core::EVENT_OUTBOUND_WEBSOCKET_ERROR,
                        json!({
                            "connection_id": connection_id,
                            "error_code": "send_failed",
                            "message_redacted": "send_failed",
                            "receipt": receipt,
                        }),
                    )
                    .await?;
                }
                return Err(error);
            }
        };
        let terminal_status = match status {
            crate::runtime::SendStatus::Ok => EffectTerminalStatus::Succeeded,
            crate::runtime::SendStatus::BufferFull => EffectTerminalStatus::Partial,
            crate::runtime::SendStatus::ConnectionClosed => EffectTerminalStatus::Cancelled,
            crate::runtime::SendStatus::ConnectionNotFound => EffectTerminalStatus::Failed,
        };
        let receipt = self
            .record_websocket_operation_effect(
                &effect_context,
                "outbound.websocket.send",
                terminal_status,
                started_at,
                started.elapsed().as_millis() as u64,
                json!({
                    "operation": "send",
                    "connection_id": connection_id,
                    "frame_kind": frame_kind,
                    "frame_bytes": frame_bytes,
                }),
                json!({"status": status}),
            )
            .await?;
        Ok(json!({"status": status, "receipt": receipt}))
    }

    pub(crate) async fn dispatch_outbound_websocket_close(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let connection_id = params
            .get("connection_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                anyhow::anyhow!("host.outbound.websocket.close requires connection_id")
            })?;
        let code = params.get("code").and_then(Value::as_u64).unwrap_or(1000) as u16;
        let reason = params
            .get("reason")
            .and_then(Value::as_str)
            .map(str::to_string);
        let reason_present = reason.is_some();
        let effect_context = websocket_effect_context(self, context, connection_id).await;
        let started = Instant::now();
        let started_at = chrono::Utc::now();
        match self
            .outbound_websocket_executor()
            .close(connection_id, code, reason)
            .await
        {
            Ok(()) => {
                let receipt = self
                    .record_websocket_operation_effect(
                        &effect_context,
                        "outbound.websocket.close",
                        EffectTerminalStatus::Succeeded,
                        started_at,
                        started.elapsed().as_millis() as u64,
                        json!({
                            "operation": "close",
                            "connection_id": connection_id,
                            "code": code,
                            "reason_present": reason_present,
                        }),
                        json!({"status": "accepted"}),
                    )
                    .await?;
                Ok(json!({"status": "ok", "receipt": receipt}))
            }
            Err(error) => {
                let receipt = self
                    .record_websocket_operation_effect(
                        &effect_context,
                        "outbound.websocket.close",
                        EffectTerminalStatus::Failed,
                        started_at,
                        started.elapsed().as_millis() as u64,
                        json!({
                            "operation": "close",
                            "connection_id": connection_id,
                            "code": code,
                            "reason_present": reason_present,
                        }),
                        json!({"status": "failed"}),
                    )
                    .await?;
                if let Some(record) = self
                    .streams
                    .get_invocation_by_stream_id(connection_id)
                    .await
                {
                    self.append_platform_event(
                        &record.session_id,
                        plurora_core::EVENT_OUTBOUND_WEBSOCKET_ERROR,
                        json!({
                            "connection_id": connection_id,
                            "error_code": "close_failed",
                            "message_redacted": "close_failed",
                            "receipt": receipt,
                        }),
                    )
                    .await?;
                }
                Err(error)
            }
        }
    }
}

async fn websocket_effect_context<S: EventStore>(
    runtime: &Runtime<S>,
    context: &ProtocolContext,
    connection_id: &str,
) -> WebSocketEffectContext {
    if let Some(record) = runtime
        .streams
        .get_invocation_by_stream_id(connection_id)
        .await
    {
        let destination_host = record
            .metadata
            .get("destination_host")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let executor_kind = record
            .metadata
            .get("executor_kind")
            .and_then(Value::as_str)
            .unwrap_or("configured")
            .to_string();
        let network_performed = record
            .metadata
            .get("network_performed")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let parent_receipts = record
            .metadata
            .get("open_receipt")
            .and_then(Value::as_str)
            .map(|digest| vec![digest.to_string()])
            .unwrap_or_default();
        return WebSocketEffectContext {
            principal: principal_identity(&context.principal),
            package_id: record.provider_package_id,
            capability_id: record.capability_id,
            destination_host,
            connection_id: connection_id.to_string(),
            executor_kind,
            network_performed,
            session_id: Some(record.session_id),
            trace_id: context.effective_correlation_id().to_string(),
            parent_receipts,
        };
    }

    let package_id = match &context.principal {
        ProtocolPrincipal::Package { package_id } => package_id.clone(),
        _ => "host/unknown".to_string(),
    };
    WebSocketEffectContext {
        principal: principal_identity(&context.principal),
        package_id,
        capability_id: "unknown".to_string(),
        destination_host: "unknown".to_string(),
        connection_id: connection_id.to_string(),
        executor_kind: "configured".to_string(),
        network_performed: false,
        session_id: context.session_id.clone(),
        trace_id: context.effective_correlation_id().to_string(),
        parent_receipts: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Y3: StreamEmitterAdapter — bridges OutboundStreamFrame to platform stream lifecycle
// ---------------------------------------------------------------------------

/// Adapter that implements `StreamEmitter` and feeds frames into
/// the platform stream registry.
///
/// Each emitted `OutboundStreamFrame` is converted to a chunk in
/// the `StreamRegistry` and records `capability/stream.chunk` events.
/// The spawned task's completion handler emits terminal events.
struct StreamEmitterAdapter<S: EventStore> {
    streams: Arc<StreamRegistry>,
    store: Arc<S>,
    session_id: String,
    invocation_id: String,
    stream_id: String,
    cancel_tx: tokio::sync::watch::Sender<bool>,
}

#[async_trait::async_trait]
impl<S> StreamEmitter for StreamEmitterAdapter<S>
where
    S: EventStore,
{
    async fn emit(&self, frame: OutboundStreamFrame) -> anyhow::Result<()> {
        // Check if the invocation is still active
        let record = self.streams.get_invocation(&self.invocation_id).await;
        if let Some(rec) = &record {
            if rec.is_terminal() {
                // Invocation is already terminal (cancelled/timed out/etc).
                // Signal the cancel so the executor stops.
                let _ = self.cancel_tx.send(true);
                return Ok(());
            }
        }

        // If this is a Done frame, end the invocation instead of appending a chunk
        if frame.kind == OutboundFrameKind::Done {
            // Signal cancel to stop any further emissions
            let _ = self.cancel_tx.send(true);
            return Ok(());
        }

        // Build the chunk payload from the outbound frame
        let payload = json!({
            "invocation_id": self.invocation_id,
            "stream_id": self.stream_id,
            "seq": frame.seq,
            "kind": frame.kind,
            "data_shape": frame.data_shape,
            "bytes_received": frame.bytes_received,
        });

        // Append a chunk frame to the platform stream
        let _platform_frame = self
            .streams
            .append_chunk(
                &self.invocation_id,
                payload.clone(),
                RedactionState::Redacted,
            )
            .await?;

        use plurora_core::{new_id, EventEnvelope, EVENT_STREAM_CHUNK, PLATFORM_RUNTIME_ID};
        let seq = self.store.next_sequence(&self.session_id).await?;
        self.store
            .append(EventEnvelope {
                id: new_id("evt"),
                session_id: self.session_id.clone(),
                sequence: seq,
                timestamp: chrono::Utc::now(),
                writer_package_id: PLATFORM_RUNTIME_ID.to_string(),
                kind: EVENT_STREAM_CHUNK.to_string(),
                schema_version: 1,
                payload: json!({
                    "invocation_id": self.invocation_id,
                    "stream_id": self.stream_id,
                    "outbound_seq": frame.seq,
                    "redaction_state": serde_json::to_value(RedactionState::Redacted)?,
                    "data": payload,
                }),
                metadata: json!({}),
            })
            .await?;

        Ok(())
    }
}

/// Recursively strip raw-secret-like field values from a JSON value
/// before returning it to a protocol caller.
///
/// This is a defense-in-depth sweep: replaces values of known secret
/// field names with `"[redacted]"`. Does not touch non-secret fields.
fn strip_raw_secrets_from_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                if plurora_core::is_secret_field_name(k) {
                    *v = Value::String("[redacted]".to_string());
                } else {
                    strip_raw_secrets_from_value(v);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                strip_raw_secrets_from_value(item);
            }
        }
        _ => {}
    }
}

fn outbound_host_matches(pattern: &str, destination: &str) -> bool {
    if pattern.eq_ignore_ascii_case(destination) {
        return true;
    }
    let pattern = pattern.to_ascii_lowercase();
    let destination = destination.to_ascii_lowercase();
    if let Some(suffix) = pattern.strip_prefix("*.") {
        return destination == suffix
            || destination
                .strip_suffix(suffix)
                .is_some_and(|prefix| prefix.ends_with('.') && prefix.len() > 1);
    }
    false
}

fn validate_outbound_stream_https_only(
    destination_host: &str,
    path: Option<&str>,
    metadata: &Value,
    allow_insecure_loopback_for_tests: bool,
) -> anyhow::Result<()> {
    let base_url = metadata.get("base_url").and_then(Value::as_str);
    let url_str = if let Some(base) = base_url {
        let mut url = base.trim_end_matches('/').to_string();
        if let Some(path) = path {
            if !path.starts_with('/') {
                url.push('/');
            }
            url.push_str(path);
        }
        url
    } else {
        let scheme = metadata
            .get("scheme")
            .and_then(Value::as_str)
            .unwrap_or("https");
        let raw_path = path.unwrap_or("/");
        let path = if raw_path.starts_with('/') {
            raw_path.to_string()
        } else {
            format!("/{raw_path}")
        };
        format!("{scheme}://{destination_host}{path}")
    };

    let url = reqwest::Url::parse(&url_str)
        .map_err(|e| anyhow::anyhow!("invalid outbound.stream URL '{}': {e}", url_str))?;
    let actual_host = url.host_str().unwrap_or("");
    if !actual_host.eq_ignore_ascii_case(destination_host) {
        anyhow::bail!(
            "outbound.stream URL host '{}' does not match destination_host '{}'",
            actual_host,
            destination_host
        );
    }
    if url.scheme() != "https" {
        let is_loopback =
            actual_host == "127.0.0.1" || actual_host == "localhost" || actual_host == "[::1]";
        if !allow_insecure_loopback_for_tests || !is_loopback {
            anyhow::bail!("outbound.stream rejects non-HTTPS URL: {}", url_str);
        }
    }
    Ok(())
}

fn parse_websocket_frame(params: &Value) -> anyhow::Result<OutboundWebSocketFrame> {
    let kind = params.get("kind").and_then(Value::as_str).unwrap_or("text");
    match kind {
        "text" => Ok(OutboundWebSocketFrame::Text(
            params
                .get("text")
                .or_else(|| params.get("data"))
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("host.outbound.websocket.send requires text data"))?
                .to_string(),
        )),
        "binary" => {
            let arr = params
                .get("bytes")
                .or_else(|| params.get("data"))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    anyhow::anyhow!("host.outbound.websocket.send requires binary bytes array")
                })?;
            let mut bytes = Vec::with_capacity(arr.len());
            for value in arr {
                let byte = value.as_u64().ok_or_else(|| {
                    anyhow::anyhow!("host.outbound.websocket.send binary bytes must be integers")
                })?;
                if byte > u8::MAX as u64 {
                    anyhow::bail!("host.outbound.websocket.send binary byte out of range");
                }
                bytes.push(byte as u8);
            }
            Ok(OutboundWebSocketFrame::Binary(Bytes::from(bytes)))
        }
        other => anyhow::bail!("host.outbound.websocket.send unknown frame kind '{other}'"),
    }
}

fn websocket_event_to_platform_event(event: WebSocketEvent) -> (&'static str, Value, bool) {
    match event {
        WebSocketEvent::Opened {
            connection_id,
            subprotocol,
        } => (
            plurora_core::EVENT_OUTBOUND_WEBSOCKET_OPENED,
            json!({"connection_id": connection_id, "subprotocol": subprotocol}),
            false,
        ),
        WebSocketEvent::Frame {
            connection_id,
            direction,
            kind,
            bytes,
            seq,
            payload,
        } => {
            let payload_shape = match payload {
                crate::runtime::WebSocketFramePayload::Text(text) => {
                    json!({"kind": "text", "bytes": text.len()})
                }
                crate::runtime::WebSocketFramePayload::Binary(bytes) => {
                    json!({"kind": "binary", "bytes": bytes.len()})
                }
            };
            (
                plurora_core::EVENT_OUTBOUND_WEBSOCKET_FRAME,
                json!({
                    "connection_id": connection_id,
                    "direction": direction,
                    "frame_kind": kind,
                    "bytes": bytes,
                    "seq": seq,
                    "payload_shape": payload_shape,
                }),
                false,
            )
        }
        WebSocketEvent::Error {
            connection_id,
            code,
            message_redacted,
        } => (
            plurora_core::EVENT_OUTBOUND_WEBSOCKET_ERROR,
            json!({"connection_id": connection_id, "error_code": code, "message_redacted": message_redacted}),
            false,
        ),
        WebSocketEvent::Closed {
            connection_id,
            code,
            reason,
            total_frames_in,
            total_frames_out,
            total_bytes_in,
            total_bytes_out,
            duration_ms,
        } => (
            plurora_core::EVENT_OUTBOUND_WEBSOCKET_COMPLETED,
            json!({
                "connection_id": connection_id,
                "code": code,
                "reason": reason,
                "total_frames_in": total_frames_in,
                "total_frames_out": total_frames_out,
                "total_bytes_in": total_bytes_in,
                "total_bytes_out": total_bytes_out,
                "duration_ms": duration_ms,
            }),
            true,
        ),
    }
}

/// L4: Parse `secret_headers` from `host.outbound.execute` params.
///
/// Expected format:
/// ```json
/// {
///   "Authorization": {"secret_ref": "secret_ref:env:DEEPSEEK_API_KEY", "scheme": "bearer"},
///   "x-api-key": {"secret_ref": "secret_ref:env:MY_KEY"}
/// }
/// ```
///
/// Each entry declares a header to be injected from a secret_ref, with
/// an optional scheme prefix (e.g. "bearer" → "Bearer <value>").
/// The host resolves these at execution time; raw values are never
/// returned to the caller, persisted in audit, or echoed in errors.
fn parse_secret_headers(
    params: &Value,
) -> anyhow::Result<Vec<crate::runtime::outbound::SecretHeaderSpec>> {
    let secret_headers_value = match params.get("secret_headers") {
        Some(v) => v,
        None => return Ok(Vec::new()),
    };

    let headers_obj = secret_headers_value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("host.outbound.execute secret_headers must be an object"))?;

    let mut specs = Vec::new();
    for (header_name, header_spec) in headers_obj {
        let secret_ref = header_spec
            .get("secret_ref")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                anyhow::anyhow!("host.outbound.execute secret header requires secret_ref")
            })?
            .to_string();

        if !plurora_core::SecretRef::is_valid_ref(&secret_ref) {
            anyhow::bail!("host.outbound.execute secret header secret_ref is invalid");
        }

        let scheme = header_spec
            .get("scheme")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        specs.push(crate::runtime::outbound::SecretHeaderSpec {
            header_name: header_name.clone(),
            secret_ref,
            scheme,
        });
    }

    Ok(specs)
}

/// L5: Parse `static_headers` from `host.outbound.execute` params.
///
/// Expected format:
/// ```json
/// {
///   "anthropic-version": "2023-06-01",
///   "accept": "application/json"
/// }
/// ```
///
/// Each entry declares a safe non-secret header to be injected into the
/// request. Only header names on the `STATIC_HEADER_ALLOWLIST` are
/// permitted; secret-bearing header names (Authorization, x-api-key,
/// Cookie, etc.) are rejected with an error. Values must be plain
/// strings that do not look like raw secrets.
///
/// This allows provider-specific version headers (e.g. Anthropic's
/// `anthropic-version`) without requiring secret resolution, while
/// preventing `static_headers` from becoming a secret bypass path.
fn parse_static_headers(
    params: &Value,
) -> anyhow::Result<Vec<crate::runtime::outbound::StaticHeader>> {
    let static_headers_value = match params.get("static_headers") {
        Some(v) => v,
        None => return Ok(Vec::new()),
    };

    let headers_obj = static_headers_value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("host.outbound.execute static_headers must be an object"))?;

    let mut headers = Vec::new();
    for (header_name, header_value) in headers_obj {
        // Defense-in-depth: reject known secret-bearing header names
        if crate::runtime::outbound::is_secret_header_name(header_name) {
            anyhow::bail!(
                "host.outbound.execute static_headers rejected: '{}' is a secret-bearing header; use secret_headers with secret_ref instead",
                header_name
            );
        }

        // Only allowlisted header names are permitted
        if !crate::runtime::outbound::is_static_header_allowed(header_name) {
            anyhow::bail!(
                "host.outbound.execute static_headers rejected: '{}' is not on the safe header allowlist",
                header_name
            );
        }

        let value = header_value
            .as_str()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "host.outbound.execute static_headers value for '{}' must be a string",
                    header_name
                )
            })?
            .to_string();

        // Reject values that look like raw secrets
        if looks_like_raw_secret_value(&value) {
            anyhow::bail!(
                "host.outbound.execute static_headers rejected: value for '{}' looks like a raw secret; use secret_headers with secret_ref instead",
                header_name
            );
        }

        headers.push(crate::runtime::outbound::StaticHeader {
            name: header_name.clone(),
            value,
        });
    }

    Ok(headers)
}

/// Check if a static header value looks like a raw secret.
/// This is a lightweight defense-in-depth check — not a full secret scanner.
fn looks_like_raw_secret_value(value: &str) -> bool {
    if value.starts_with("sk-") || value.starts_with("sk_") {
        return true;
    }
    if value.starts_with("key-") || value.starts_with("key_") {
        return true;
    }
    if value.starts_with("Bearer ") || value.starts_with("bearer ") {
        return true;
    }
    if value.starts_with("AIza") {
        return true;
    }
    // High-entropy alphanumeric strings of length >= 32
    if value.len() >= 32
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        let has_upper = value.chars().any(|c| c.is_uppercase());
        let has_lower = value.chars().any(|c| c.is_lowercase());
        let has_digit = value.chars().any(|c| c.is_ascii_digit());
        if has_upper && has_lower && has_digit {
            return true;
        }
    }
    false
}
