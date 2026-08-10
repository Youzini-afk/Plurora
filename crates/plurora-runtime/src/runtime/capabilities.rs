use std::time::Instant;

use plurora_core::{
    ArtifactDescriptor, CapHandle, CapHandleId, CapabilityId, EffectReplayMode, EffectScope,
    EffectTerminalStatus, HandleLease, HandleProvenance, HandleScope, PackageEntry,
    PrincipalIdentity, EVENT_CAPABILITY_COMPLETED, EVENT_CAPABILITY_FAILED,
    EVENT_CAPABILITY_INVOKED, PLATFORM_RUNTIME_ID,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use super::branches::BranchRecord;
use super::effects::{principal_identity, request_principal, EffectReceiptRequest};
use super::Runtime;
use crate::{
    sha256_digest, validate_json_schema_subset, CapabilityInvocationRequest,
    CapabilityInvocationResult, EventStore, InprocInvocation, ProtocolContext, ProtocolPrincipal,
    RegisteredCapability, DEFAULT_CONTRACT_PROFILE,
};

#[derive(Debug, Clone)]
struct CapabilityEffectContext {
    principal: PrincipalIdentity,
    parent_invocation_id: Option<Uuid>,
    parent_receipts: Vec<String>,
    replay_mode: EffectReplayMode,
    branch_id: Option<String>,
}

#[derive(Debug, Clone)]
struct CapabilityReceiptSource {
    caller_package_id: Option<String>,
    provider_package_id: Option<String>,
    version: Option<String>,
    session_id: Option<String>,
    input: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CapabilityReexecutionResult {
    pub branch: BranchRecord,
    pub invocation: CapabilityInvocationResult,
}

impl CapabilityReceiptSource {
    fn capture(request: &CapabilityInvocationRequest) -> Self {
        Self {
            caller_package_id: request.caller_package_id.clone(),
            provider_package_id: request.provider_package_id.clone(),
            version: request.version.clone(),
            session_id: request.session_id.clone(),
            input: request.input.clone(),
        }
    }
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub async fn discover_capabilities(&self) -> Vec<crate::RegisteredCapability> {
        self.capabilities.discover().await
    }

    pub async fn invoke_capability(
        &self,
        request: CapabilityInvocationRequest,
    ) -> anyhow::Result<CapabilityInvocationResult> {
        let effect_context = CapabilityEffectContext {
            principal: request_principal(request.caller_package_id.as_deref()),
            parent_invocation_id: None,
            parent_receipts: Vec::new(),
            replay_mode: EffectReplayMode::Live,
            branch_id: None,
        };
        self.invoke_capability_authorized(request, Uuid::new_v4(), effect_context)
            .await
    }

    async fn invoke_capability_authorized(
        &self,
        request: CapabilityInvocationRequest,
        correlation_id: Uuid,
        effect_context: CapabilityEffectContext,
    ) -> anyhow::Result<CapabilityInvocationResult> {
        let started = Instant::now();
        let started_at = chrono::Utc::now();
        let receipt_source = CapabilityReceiptSource::capture(&request);

        let prepared = self.prepare_capability_invocation(&request).await;
        let (capability_id, version, active_handle) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                let capability_id = request.capability_id.clone().unwrap_or_else(|| {
                    format!(
                        "handle:{}",
                        request
                            .handle
                            .map_or_else(|| "unknown".to_string(), |handle| handle.0.to_string())
                    )
                });
                let duration_ms = elapsed_ms(started);
                let status = classify_capability_error(&error);
                let receipt = self
                    .record_capability_effect(
                        receipt_source,
                        effect_context,
                        &capability_id,
                        request.provider_package_id.as_deref(),
                        request.handle,
                        status,
                        started_at,
                        duration_ms,
                        correlation_id,
                        None,
                        Some(error.to_string()),
                        None,
                    )
                    .await?;
                self.emit_capability_failed(
                    correlation_id,
                    &capability_id,
                    request.handle,
                    duration_ms,
                    "prepare_failed",
                    &error.to_string(),
                    Some(&receipt),
                )
                .await?;
                return Err(error);
            }
        };

        self.append_platform_event(
            &capability_event_session_id(&capability_id),
            EVENT_CAPABILITY_INVOKED,
            json!({
                "correlation_id": correlation_id,
                "capability_id": capability_id,
                "caller_package_id": request.caller_package_id,
                "handle_id": active_handle,
                "started_at": started_at,
            }),
        )
        .await?;

        let invoke_result = self
            .invoke_capability_prepared(
                request,
                capability_id.clone(),
                version,
                active_handle,
                correlation_id,
                started,
            )
            .await;

        match invoke_result {
            Ok(mut result) => {
                let receipt = self
                    .record_capability_effect(
                        receipt_source,
                        effect_context.clone(),
                        &capability_id,
                        Some(&result.provider_package_id),
                        Some(active_handle),
                        EffectTerminalStatus::Succeeded,
                        started_at,
                        result.duration_ms,
                        correlation_id,
                        Some(result.output.clone()),
                        None,
                        Some(&result),
                    )
                    .await?;
                result.receipt = Some(receipt.clone());
                result.replay_mode = Some(effect_context.replay_mode);
                self.append_platform_event(
                    &capability_event_session_id(&capability_id),
                    EVENT_CAPABILITY_COMPLETED,
                    json!({
                        "correlation_id": correlation_id,
                        "capability_id": capability_id,
                        "provider_package_id": result.provider_package_id,
                        "provider_component_id": result.provider_component_id,
                        "provider_component_digest": result.provider_component_digest,
                        "provider_behavior_digest": result.provider_behavior_digest,
                        "provider_trust_class": result.provider_trust_class,
                        "duration_ms": result.duration_ms,
                        "completed_at": chrono::Utc::now(),
                        "receipt": receipt,
                    }),
                )
                .await?;
                Ok(result)
            }
            Err(error) => {
                let duration_ms = elapsed_ms(started);
                let receipt = self
                    .record_capability_effect(
                        receipt_source,
                        effect_context,
                        &capability_id,
                        None,
                        Some(active_handle),
                        classify_capability_error(&error),
                        started_at,
                        duration_ms,
                        correlation_id,
                        None,
                        Some(error.to_string()),
                        None,
                    )
                    .await?;
                self.emit_capability_failed(
                    correlation_id,
                    &capability_id,
                    Some(active_handle),
                    duration_ms,
                    "invoke_failed",
                    &error.to_string(),
                    Some(&receipt),
                )
                .await?;
                Err(error)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_capability_effect(
        &self,
        source: CapabilityReceiptSource,
        effect_context: CapabilityEffectContext,
        capability_id: &str,
        provider_package_id: Option<&str>,
        handle_id: Option<CapHandleId>,
        status: EffectTerminalStatus,
        started_at: chrono::DateTime<chrono::Utc>,
        duration_ms: u64,
        correlation_id: Uuid,
        output: Option<Value>,
        error: Option<String>,
        provider_identity: Option<&CapabilityInvocationResult>,
    ) -> anyhow::Result<ArtifactDescriptor> {
        let resolved_provider = provider_package_id
            .map(str::to_string)
            .or_else(|| source.provider_package_id.clone());
        let decision = if status == EffectTerminalStatus::Denied {
            "deny"
        } else {
            "allow"
        };
        let mut request = EffectReceiptRequest::live(
            "capability.invoke",
            effect_context.principal,
            json!({
                "kind": "capability_provider",
                "capability_id": capability_id,
                "provider_package_id": resolved_provider,
                "requested_version": source.version,
            }),
            status,
            started_at,
            duration_ms,
            correlation_id.to_string(),
        );
        request.protocol_profiles = vec![DEFAULT_CONTRACT_PROFILE.to_string()];
        request.inputs = vec![source.input];
        request.outputs = output.into_iter().collect();
        request.authority = Some(json!({
            "caller_package_id": source.caller_package_id,
            "handle_id": handle_id,
            "parent_invocation_id": effect_context.parent_invocation_id,
        }));
        request.policy_decision = Some(json!({
            "outcome": decision,
            "basis": if status == EffectTerminalStatus::Denied {
                "capability_authorization"
            } else {
                "capability_runtime"
            },
        }));
        request.parent_receipts = effect_context.parent_receipts;
        request.replay_mode = effect_context.replay_mode;
        request.scope = EffectScope {
            session_id: source.session_id,
            branch_id: effect_context.branch_id,
        };
        request.planned = json!({
            "capability_id": capability_id,
            "provider_package_id": resolved_provider,
        });
        request.actual = json!({
            "capability_id": capability_id,
            "provider_package_id": resolved_provider,
            "provider_component_id": provider_identity.map(|result| &result.provider_component_id),
            "provider_component_digest": provider_identity.map(|result| &result.provider_component_digest),
            "provider_behavior_digest": provider_identity.map(|result| &result.provider_behavior_digest),
            "provider_trust_class": provider_identity.map(|result| result.provider_trust_class),
            "status": status,
            "error_present": error.is_some(),
        });
        self.record_effect_receipt(request).await
    }

    async fn prepare_capability_invocation(
        &self,
        request: &CapabilityInvocationRequest,
    ) -> anyhow::Result<(CapabilityId, Option<String>, CapHandleId)> {
        if let Some(caller) = &request.caller_package_id {
            if self.is_contract_none_package(caller).await {
                let capability_id = request.capability_id.clone().unwrap_or_else(|| {
                    request.handle.map_or_else(
                        || "unknown".to_string(),
                        |handle| format!("handle:{}", handle.0),
                    )
                });
                self.audit_permission_denied(
                    &capability_event_session_id(&capability_id),
                    caller,
                    "capabilities.invoke",
                )
                .await?;
                anyhow::bail!(
                    "Foreign Capsule package '{caller}' is self-contained and cannot invoke platform capabilities"
                );
            }
        }
        if let Some(handle_id) = request.handle {
            let handle = self
                .handles
                .lookup(handle_id)
                .await
                .ok_or_else(|| anyhow::anyhow!("capability handle not found"))?;
            if let Some(caller) = &request.caller_package_id {
                if handle.scope.holder_package_id != *caller {
                    anyhow::bail!("capability handle is not held by caller package");
                }
            }
            validate_handle_lease(&handle)?;
            let version = if handle.cap_version == "1" {
                None
            } else {
                Some(handle.cap_version.clone())
            };
            return Ok((handle.cap_type, version, handle_id));
        }

        let capability_id = request
            .capability_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("capability invoke requires handle or capability_id"))?;
        if let Some(caller) = &request.caller_package_id {
            let allowed = self
                .packages
                .permissions(caller)
                .await
                .map(|permissions| {
                    permissions.capabilities.invoke.iter().any(|pattern| {
                        pattern == "*"
                            || pattern == &capability_id
                            || capability_id.starts_with(pattern.trim_end_matches('*'))
                    })
                })
                .unwrap_or(false);
            if !allowed {
                self.audit_permission_denied(
                    &capability_event_session_id(&capability_id),
                    caller,
                    "capabilities.invoke",
                )
                .await?;
                anyhow::bail!(
                    "package '{caller}' is not allowed to invoke '{}'",
                    capability_id
                );
            }
        }
        let holder = request
            .caller_package_id
            .clone()
            .unwrap_or_else(|| PLATFORM_RUNTIME_ID.to_string());
        let provider = self
            .capabilities
            .resolve(
                &capability_id,
                request.provider_package_id.as_ref(),
                request.version.as_deref(),
            )
            .await?;
        let handle_id = self
            .handles
            .mint(CapHandle {
                id: CapHandleId::new(),
                cap_type: capability_id.clone(),
                cap_version: provider.descriptor.version,
                scope: HandleScope {
                    holder_package_id: holder,
                    session_id: request.session_id.clone(),
                },
                constraints: json!({}),
                lease: HandleLease {
                    expires_at: None,
                    max_invocations: Some(1),
                    invocations_used: 0,
                },
                provenance: HandleProvenance {
                    granted_at: chrono::Utc::now(),
                    granted_by_package_id: PLATFORM_RUNTIME_ID.to_string(),
                    via_method: "auto_mint".to_string(),
                },
                parent: None,
                revoked: false,
            })
            .await;
        Ok((capability_id, request.version.clone(), handle_id))
    }

    async fn invoke_capability_prepared(
        &self,
        mut request: CapabilityInvocationRequest,
        capability_id: CapabilityId,
        version: Option<String>,
        active_handle: CapHandleId,
        correlation_id: Uuid,
        started: Instant,
    ) -> anyhow::Result<CapabilityInvocationResult> {
        let mut before_payload = serde_json::Map::with_capacity(3);
        before_payload.insert(
            "capability_id".to_string(),
            Value::String(capability_id.clone()),
        );
        before_payload.insert(
            "caller_package_id".to_string(),
            request
                .caller_package_id
                .take()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        before_payload.insert("input".to_string(), std::mem::take(&mut request.input));
        let mut before = self
            .dispatch_extension_handlers("capability/before_invoke", Value::Object(before_payload))
            .await;
        if let Some(vetoed_by) = before.vetoed_by.as_deref() {
            anyhow::bail!("capability invoke vetoed by hook package '{vetoed_by}'");
        }
        request.input = before
            .payload
            .get_mut("input")
            .map(Value::take)
            .ok_or_else(|| anyhow::anyhow!("before-invoke hook payload lost capability input"))?;

        let provider = self
            .capabilities
            .resolve(
                &capability_id,
                request.provider_package_id.as_ref(),
                version.as_deref(),
            )
            .await?;
        validate_json_schema_subset(&provider.descriptor.input_schema, &request.input)?;
        let output = self
            .execute_registered_capability(
                &provider,
                &capability_id,
                request.session_id.clone(),
                request.input,
            )
            .await?;
        self.handles.record_invocation(active_handle).await?;
        let result = CapabilityInvocationResult {
            capability_id: provider.descriptor.id,
            provider_package_id: provider.provider_package_id,
            provider_component_id: provider.provider_component_id,
            provider_component_digest: provider.provider_component_digest,
            provider_behavior_digest: provider.provider_behavior_digest,
            provider_trust_class: provider.provider_trust_class,
            output,
            duration_ms: elapsed_ms(started),
            correlation_id,
            receipt: None,
            replay_mode: None,
        };
        let _ = self
            .dispatch_extension_handlers(
                "capability/after_invoke",
                serde_json::to_value(&result).unwrap_or_else(|_| json!({})),
            )
            .await;
        Ok(result)
    }

    async fn emit_capability_failed(
        &self,
        correlation_id: Uuid,
        capability_id: &str,
        handle_id: Option<CapHandleId>,
        duration_ms: u64,
        error_kind: &str,
        error_message: &str,
        receipt: Option<&ArtifactDescriptor>,
    ) -> anyhow::Result<()> {
        self.append_platform_event(
            &capability_event_session_id(capability_id),
            EVENT_CAPABILITY_FAILED,
            json!({
                "correlation_id": correlation_id,
                "capability_id": capability_id,
                "handle_id": handle_id,
                "duration_ms": duration_ms,
                "error_kind": error_kind,
                "error_message": safe_capability_error_message(error_kind),
                "error_message_present": !error_message.is_empty(),
                "error_fingerprint": sha256_digest(error_message.as_bytes()),
                "failed_at": chrono::Utc::now(),
                "receipt": receipt,
            }),
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn execute_registered_capability(
        &self,
        provider: &RegisteredCapability,
        capability_id: &str,
        session_id: Option<String>,
        input: Value,
    ) -> anyhow::Result<Value> {
        let output = match self.package_status(&provider.provider_package_id).await {
            Some(record) => match record.manifest.entry.kind {
                PackageEntry::RustInproc {
                    crate_ref, symbol, ..
                } => {
                    let package = self
                        .config
                        .inproc_packages
                        .lookup(&crate_ref, &symbol)
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "rust_inproc entry '{crate_ref}::{symbol}' is not available"
                            )
                        })?;
                    let invocation = InprocInvocation {
                        capability_id: capability_id.to_string(),
                        provider_package_id: provider.provider_package_id.clone(),
                        session_id: session_id.clone(),
                        input,
                    };
                    crate::inproc::with_runtime_invoker(
                        self.clone(),
                        session_id.clone(),
                        package.invoke(invocation),
                    )
                    .await?
                }
                PackageEntry::Subprocess { .. } => match self
                    .subprocesses
                    .invoke(
                        self.clone(),
                        &provider.provider_package_id,
                        capability_id,
                        session_id,
                        input,
                    )
                    .await
                {
                    Ok(output) => output,
                    Err(error) => return Err(error),
                },
                other => anyhow::bail!(
                    "entry kind '{}' cannot execute capabilities yet",
                    crate::entry_kind(&other)
                ),
            },
            None => anyhow::bail!(
                "provider package '{}' is not loaded",
                provider.provider_package_id
            ),
        };
        validate_json_schema_subset(&provider.descriptor.output_schema, &output)?;
        Ok(output)
    }

    pub async fn invoke_capability_with_context(
        &self,
        context: &ProtocolContext,
        request: CapabilityInvocationRequest,
    ) -> anyhow::Result<CapabilityInvocationResult> {
        self.invoke_capability_with_effect_context(
            context,
            request,
            EffectReplayMode::Live,
            Vec::new(),
            None,
        )
        .await
    }

    async fn invoke_capability_with_effect_context(
        &self,
        context: &ProtocolContext,
        mut request: CapabilityInvocationRequest,
        replay_mode: EffectReplayMode,
        parent_receipts: Vec<String>,
        branch_id: Option<String>,
    ) -> anyhow::Result<CapabilityInvocationResult> {
        let effect_context = || CapabilityEffectContext {
            principal: principal_identity(&context.principal),
            parent_invocation_id: context.parent_invocation_id,
            parent_receipts: parent_receipts.clone(),
            replay_mode,
            branch_id: branch_id.clone(),
        };
        match &context.principal {
            ProtocolPrincipal::Anonymous if context.is_host_device() => {
                if context.host_operation.is_some() {
                    anyhow::ensure!(
                        context.host_operation_is_authorized(),
                        "Host device capability invocation carries an invalid Host operation context"
                    );
                    request.caller_package_id = None;
                    return self
                        .invoke_capability_authorized(
                            request,
                            context.effective_correlation_id(),
                            effect_context(),
                        )
                        .await;
                }
                let session_id = context
                    .session_id
                    .as_deref()
                    .or(request.session_id.as_deref())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Host device capability invocation requires a bound session"
                        )
                    })?;
                self.ensure_host_session_access(context, "access_manage", session_id)
                    .await?;
                if context.session_id.is_some() {
                    request.session_id = context.session_id.clone();
                }
                request.caller_package_id = None;
                self.invoke_capability_authorized(
                    request,
                    context.effective_correlation_id(),
                    effect_context(),
                )
                .await
            }
            ProtocolPrincipal::HostAdmin | ProtocolPrincipal::HostDev => {
                if context.session_id.is_some() {
                    request.session_id = context.session_id.clone();
                }
                request.caller_package_id = None;
                self.invoke_capability_authorized(
                    request,
                    context.effective_correlation_id(),
                    effect_context(),
                )
                .await
            }
            ProtocolPrincipal::Package { package_id } => {
                request.session_id = context.session_id.clone();
                request.caller_package_id = Some(package_id.clone());
                self.invoke_capability_authorized(
                    request,
                    context.effective_correlation_id(),
                    effect_context(),
                )
                .await
            }
            ProtocolPrincipal::Human { .. }
            | ProtocolPrincipal::Assistant { .. }
            | ProtocolPrincipal::Anonymous => {
                if self
                    .principal_has_grant(
                        &context.principal,
                        "capabilities.invoke",
                        request.capability_id.as_deref(),
                    )
                    .await
                {
                    if context.session_id.is_some() {
                        request.session_id = context.session_id.clone();
                    }
                    request.caller_package_id = None;
                    self.invoke_capability_authorized(
                        request,
                        context.effective_correlation_id(),
                        effect_context(),
                    )
                    .await
                } else {
                    if context.session_id.is_some() {
                        request.session_id = context.session_id.clone();
                    }
                    request.caller_package_id = None;
                    let correlation_id = context.effective_correlation_id();
                    let started_at = chrono::Utc::now();
                    let capability_id = request.capability_id.clone().unwrap_or_else(|| {
                        format!(
                            "handle:{}",
                            request.handle.map_or_else(
                                || "unknown".to_string(),
                                |handle| handle.0.to_string()
                            )
                        )
                    });
                    let source = CapabilityReceiptSource::capture(&request);
                    let error = "principal is not allowed to invoke capabilities";
                    let receipt = self
                        .record_capability_effect(
                            source,
                            effect_context(),
                            &capability_id,
                            request.provider_package_id.as_deref(),
                            request.handle,
                            EffectTerminalStatus::Denied,
                            started_at,
                            1,
                            correlation_id,
                            None,
                            Some(error.to_string()),
                            None,
                        )
                        .await?;
                    self.emit_capability_failed(
                        correlation_id,
                        &capability_id,
                        request.handle,
                        1,
                        "authorization_denied",
                        error,
                        Some(&receipt),
                    )
                    .await?;
                    anyhow::bail!(error)
                }
            }
        }
    }

    pub async fn replay_capability_receipt(
        &self,
        receipt_digest: &str,
    ) -> anyhow::Result<CapabilityInvocationResult> {
        let replay = self.replay_effect_receipt(receipt_digest).await?;
        anyhow::ensure!(
            replay.receipt.effect_kind == "capability.invoke",
            "effect receipt '{}' is not a capability invocation",
            receipt_digest
        );
        anyhow::ensure!(
            replay.receipt.status == EffectTerminalStatus::Succeeded,
            "recorded capability invocation did not succeed"
        );
        anyhow::ensure!(
            replay.outputs.len() == 1,
            "recorded capability invocation must contain exactly one output"
        );
        let capability_id = replay
            .receipt
            .actual
            .get("capability_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("recorded capability id is missing"))?
            .to_string();
        let provider_package_id = replay
            .receipt
            .actual
            .get("provider_package_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("recorded capability provider is missing"))?
            .to_string();
        let provider_component_id = replay
            .receipt
            .actual
            .get("provider_component_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("{provider_package_id}/component/default"));
        let provider_component_digest = replay
            .receipt
            .actual
            .get("provider_component_digest")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let provider_behavior_digest = replay
            .receipt
            .actual
            .get("provider_behavior_digest")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let provider_trust_class = replay
            .receipt
            .actual
            .get("provider_trust_class")
            .cloned()
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or_default();
        let correlation_id = Uuid::parse_str(&replay.receipt.trace_id)
            .map_err(|error| anyhow::anyhow!("recorded capability trace id is invalid: {error}"))?;
        Ok(CapabilityInvocationResult {
            capability_id,
            provider_package_id,
            provider_component_id,
            provider_component_digest,
            provider_behavior_digest,
            provider_trust_class,
            output: replay.outputs.into_iter().next().unwrap_or(Value::Null),
            duration_ms: replay.receipt.latency_ms,
            correlation_id,
            receipt: Some(replay.receipt_ref),
            replay_mode: Some(EffectReplayMode::Historical),
        })
    }

    pub async fn reexecute_capability_receipt(
        &self,
        context: &ProtocolContext,
        receipt_digest: &str,
    ) -> anyhow::Result<CapabilityReexecutionResult> {
        let replay = self.replay_effect_receipt(receipt_digest).await?;
        anyhow::ensure!(
            replay.receipt.effect_kind == "capability.invoke",
            "effect receipt '{}' is not a capability invocation",
            receipt_digest
        );
        anyhow::ensure!(
            replay.receipt.input_refs.len() == 1,
            "recorded capability invocation must contain exactly one input"
        );
        let capability_id = replay
            .receipt
            .actual
            .get("capability_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("recorded capability id is missing"))?
            .to_string();
        let provider_package_id = replay
            .receipt
            .actual
            .get("provider_package_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        let parent_session_id = replay.receipt.scope.session_id.clone().ok_or_else(|| {
            anyhow::anyhow!("recorded capability invocation has no session scope")
        })?;
        let mut inputs = self
            .read_effect_values(&replay.receipt.input_refs, "input")
            .await?;
        let input = inputs.pop().unwrap_or(Value::Null);
        let next_sequence = self.store.next_sequence(&parent_session_id).await?;
        let branch = self
            .fork_session(
                parent_session_id,
                next_sequence.saturating_sub(1),
                json!({
                    "kind": "effect_reexecution",
                    "source_receipt": receipt_digest,
                }),
            )
            .await?;
        let request = CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(capability_id),
            caller_package_id: None,
            provider_package_id,
            version: None,
            session_id: Some(branch.child_session_id.clone()),
            input,
        };
        let invocation = self
            .invoke_capability_with_effect_context(
                context,
                request,
                EffectReplayMode::Reexecute,
                vec![receipt_digest.to_string()],
                Some(branch.id.clone()),
            )
            .await?;
        Ok(CapabilityReexecutionResult { branch, invocation })
    }
}

fn safe_capability_error_message(error_kind: &str) -> &'static str {
    match error_kind {
        "prepare_failed" => "capability invocation preparation failed",
        "invoke_failed" => "capability invocation failed",
        "authorization_denied" => "capability invocation authorization denied",
        _ => "capability invocation failed",
    }
}

fn capability_event_session_id(capability_id: &str) -> String {
    format!("platform_capability_{}", capability_id.replace('/', "_"))
}

fn validate_handle_lease(handle: &CapHandle) -> anyhow::Result<()> {
    if handle.revoked {
        anyhow::bail!("capability handle is revoked");
    }
    if let Some(expires_at) = handle.lease.expires_at {
        if expires_at <= chrono::Utc::now() {
            anyhow::bail!("capability handle lease expired");
        }
    }
    if let Some(max_invocations) = handle.lease.max_invocations {
        if handle.lease.invocations_used >= max_invocations {
            anyhow::bail!("capability handle lease exhausted");
        }
    }
    Ok(())
}

fn elapsed_ms(started: Instant) -> u64 {
    (started.elapsed().as_millis() as u64).max(1)
}

fn classify_capability_error(error: &anyhow::Error) -> EffectTerminalStatus {
    let message = error.to_string().to_ascii_lowercase();
    if [
        "not allowed",
        "denied",
        "revoked",
        "expired",
        "exhausted",
        "vetoed",
    ]
    .iter()
    .any(|needle| message.contains(needle))
    {
        EffectTerminalStatus::Denied
    } else {
        EffectTerminalStatus::Failed
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
    use serde_json::Value;

    use super::*;
    use crate::{CapabilityInvocationRequest, InMemoryEventStore, RuntimeConfig};

    #[tokio::test]
    async fn loaded_package_registers_capability() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(store, RuntimeConfig::default());
        runtime
            .load_package(plurora_core::PackageManifest {
                schema_version: 1,
                id: "example/echo".to_string(),
                version: "0.1.0".to_string(),
                display_name: None,
                description: None,
                author: None,
                license: None,
                entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                    crate_ref: "example-echo-rust-inproc".to_string(),
                    symbol: "register".to_string(),
                    abi_version: 1,
                }),
                provides: vec![plurora_core::CapabilityDescriptor {
                    id: "example/echo/echo".to_string(),
                    version: "0.1.0".to_string(),
                    input_schema: Value::Null,
                    output_schema: Value::Null,
                    streaming: false,
                    side_effects: Vec::new(),
                    description: None,
                }],
                consumes: Vec::new(),
                requires: Vec::new(),
                contributes: PackageContributions::default(),
                permissions: PermissionSet::default(),
                sandbox_policy: SandboxPolicy::default(),
            })
            .await?;

        let discovered = runtime.discover_capabilities().await;
        assert_eq!(discovered.len(), 1);
        assert_eq!(
            discovered[0].provider_component_id,
            "example/echo/component/default"
        );
        assert!(discovered[0]
            .provider_component_digest
            .starts_with("sha256:"));
        assert_eq!(
            discovered[0].provider_trust_class,
            plurora_core::ComponentTrustClass::TrustedNative
        );

        let result = runtime
            .invoke_capability(CapabilityInvocationRequest {
                handle: None,
                capability_id: Some("example/echo/echo".to_string()),
                caller_package_id: None,
                provider_package_id: None,
                version: None,
                session_id: None,
                input: json!({"ping": true}),
            })
            .await?;
        assert_eq!(result.output, json!({"ping": true}));
        assert_eq!(
            result.provider_component_id,
            "example/echo/component/default"
        );
        assert_eq!(
            result.provider_component_digest,
            discovered[0].provider_component_digest
        );
        assert_eq!(
            result.provider_behavior_digest,
            discovered[0].provider_behavior_digest
        );
        Ok(())
    }

    #[tokio::test]
    async fn denied_capability_invoke_records_audit_event() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(store.clone(), RuntimeConfig::default());
        runtime
            .load_package(plurora_core::PackageManifest {
                schema_version: 1,
                id: "example/echo".to_string(),
                version: "0.1.0".to_string(),
                display_name: None,
                description: None,
                author: None,
                license: None,
                entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                    crate_ref: "example-echo-rust-inproc".to_string(),
                    symbol: "register".to_string(),
                    abi_version: 1,
                }),
                provides: vec![plurora_core::CapabilityDescriptor {
                    id: "example/echo/echo".to_string(),
                    version: "0.1.0".to_string(),
                    input_schema: Value::Null,
                    output_schema: Value::Null,
                    streaming: false,
                    side_effects: Vec::new(),
                    description: None,
                }],
                consumes: Vec::new(),
                requires: Vec::new(),
                contributes: PackageContributions::default(),
                permissions: PermissionSet::default(),
                sandbox_policy: SandboxPolicy::default(),
            })
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
                permissions: PermissionSet::default(),
                sandbox_policy: SandboxPolicy::default(),
            })
            .await?;

        let denied = runtime
            .invoke_capability(CapabilityInvocationRequest {
                handle: None,
                capability_id: Some("example/echo/echo".to_string()),
                caller_package_id: Some("example/caller".to_string()),
                provider_package_id: None,
                version: None,
                session_id: None,
                input: json!({}),
            })
            .await;
        assert!(denied.is_err());

        let events = store
            .list_session(&"platform_capability_example_echo_echo".to_string())
            .await?;
        assert!(events
            .iter()
            .any(|event| event.kind == EVENT_PERMISSION_DENIED));
        Ok(())
    }

    #[tokio::test]
    async fn package_context_overrides_spoofed_capability_caller() -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(store, RuntimeConfig::default());
        runtime
            .load_package(plurora_core::PackageManifest {
                schema_version: 1,
                id: "example/echo".to_string(),
                version: "0.1.0".to_string(),
                display_name: None,
                description: None,
                author: None,
                license: None,
                entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                    crate_ref: "example-echo-rust-inproc".to_string(),
                    symbol: "register".to_string(),
                    abi_version: 1,
                }),
                provides: vec![plurora_core::CapabilityDescriptor {
                    id: "example/echo/echo".to_string(),
                    version: "0.1.0".to_string(),
                    input_schema: Value::Null,
                    output_schema: Value::Null,
                    streaming: false,
                    side_effects: Vec::new(),
                    description: None,
                }],
                consumes: Vec::new(),
                requires: Vec::new(),
                contributes: PackageContributions::default(),
                permissions: PermissionSet::default(),
                sandbox_policy: SandboxPolicy::default(),
            })
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
                permissions: PermissionSet::default(),
                sandbox_policy: SandboxPolicy::default(),
            })
            .await?;

        let denied = runtime
            .invoke_capability_with_context(
                &ProtocolContext::package("example/caller", "test"),
                CapabilityInvocationRequest {
                    handle: None,
                    capability_id: Some("example/echo/echo".to_string()),
                    caller_package_id: None,
                    provider_package_id: None,
                    version: None,
                    session_id: None,
                    input: json!({}),
                },
            )
            .await;

        assert!(denied.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn capability_receipt_replays_without_provider_and_reexecutes_on_new_branch(
    ) -> anyhow::Result<()> {
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(store, RuntimeConfig::default());
        runtime.load_package(echo_manifest()).await?;
        let session = runtime
            .open_session(crate::runtime::OpenSessionRequest {
                labels: vec!["receipt-test".to_string()],
                active_package_set: vec!["example/echo".to_string()],
                metadata: json!({}),
            })
            .await?;
        let original = runtime
            .invoke_capability(CapabilityInvocationRequest {
                handle: None,
                capability_id: Some("example/echo/echo".to_string()),
                caller_package_id: None,
                provider_package_id: Some("example/echo".to_string()),
                version: None,
                session_id: Some(session.id.clone()),
                input: json!({"value": 7}),
            })
            .await?;
        let original_receipt = original
            .receipt
            .clone()
            .ok_or_else(|| anyhow::anyhow!("capability receipt missing"))?;
        assert_eq!(original.replay_mode, Some(EffectReplayMode::Live));
        assert!(!original.provider_component_id.is_empty());
        assert!(original.provider_component_digest.starts_with("sha256:"));

        runtime.unload_package(&"example/echo".to_string()).await?;
        let historical = runtime
            .replay_capability_receipt(&original_receipt.digest)
            .await?;
        assert_eq!(historical.output, json!({"value": 7}));
        assert_eq!(
            historical.provider_component_id,
            original.provider_component_id
        );
        assert_eq!(
            historical.provider_component_digest,
            original.provider_component_digest
        );
        assert_eq!(
            historical.provider_behavior_digest,
            original.provider_behavior_digest
        );
        assert_eq!(
            historical.provider_trust_class,
            original.provider_trust_class
        );
        assert_eq!(historical.replay_mode, Some(EffectReplayMode::Historical));
        assert_eq!(
            historical.receipt.as_ref().map(|item| item.digest.as_str()),
            Some(original_receipt.digest.as_str())
        );

        let missing = runtime
            .replay_effect_receipt(
                "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            )
            .await
            .expect_err("missing receipt object must be explicit");
        assert!(missing.to_string().contains("incomplete history"));

        runtime.load_package(echo_manifest()).await?;
        let reexecuted = runtime
            .reexecute_capability_receipt(
                &ProtocolContext::host_dev("test"),
                &original_receipt.digest,
            )
            .await?;
        assert_eq!(reexecuted.branch.parent_session_id, session.id);
        assert_eq!(reexecuted.invocation.output, json!({"value": 7}));
        assert_eq!(
            reexecuted.invocation.replay_mode,
            Some(EffectReplayMode::Reexecute)
        );
        let reexecuted_receipt = reexecuted
            .invocation
            .receipt
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("reexecution receipt missing"))?;
        assert_ne!(reexecuted_receipt.digest, original_receipt.digest);
        let replay = runtime
            .replay_effect_receipt(&reexecuted_receipt.digest)
            .await?;
        assert_eq!(
            replay.receipt.parent_receipts,
            vec![original_receipt.digest]
        );
        assert_eq!(replay.receipt.replay_mode, EffectReplayMode::Reexecute);
        assert_eq!(
            replay.receipt.scope.branch_id.as_deref(),
            Some(reexecuted.branch.id.as_str())
        );
        Ok(())
    }

    fn echo_manifest() -> plurora_core::PackageManifest {
        plurora_core::PackageManifest {
            schema_version: 1,
            id: "example/echo".to_string(),
            version: "0.1.0".to_string(),
            display_name: None,
            description: None,
            author: None,
            license: None,
            entry: EntryDescriptor::v1(PackageEntry::RustInproc {
                crate_ref: "example-echo-rust-inproc".to_string(),
                symbol: "register".to_string(),
                abi_version: 1,
            }),
            provides: vec![plurora_core::CapabilityDescriptor {
                id: "example/echo/echo".to_string(),
                version: "0.1.0".to_string(),
                input_schema: Value::Null,
                output_schema: Value::Null,
                streaming: false,
                side_effects: Vec::new(),
                description: None,
            }],
            consumes: Vec::new(),
            requires: Vec::new(),
            contributes: PackageContributions::default(),
            permissions: PermissionSet::default(),
            sandbox_policy: SandboxPolicy::default(),
        }
    }
}
