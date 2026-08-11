use super::*;
use crate::{
    BindingCandidatesRequest, BindingListRequest, BindingRevokeRequest, BindingSelectRequest,
    ExposureCreateRequest, ExposureListRequest, ExposureRevokeRequest, PowerboxAuthoritySubject,
    PowerboxMutationAuthority, PowerboxQueryContext,
};
use plurora_work::{
    installation_port_resource_id, InstallationId, PortId, ResourceSelector, RunId,
};

impl<S> Runtime<S>
where
    S: EventStore,
{
    fn ensure_powerbox_action(
        context: &ProtocolContext,
        action: &str,
        method: &str,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            context.allows_host_action(action),
            "{method} permission denied: authenticated authority lacks '{action}'"
        );
        Ok(())
    }

    fn allows_installation(context: &ProtocolContext, installation_id: &InstallationId) -> bool {
        context.allows_host_resource("host", "installation", installation_id.as_str())
    }

    fn allows_run(context: &ProtocolContext, run_id: &RunId) -> bool {
        context.allows_host_resource("host", "run", run_id.as_str())
    }

    fn allows_port(
        context: &ProtocolContext,
        installation_id: &InstallationId,
        port_id: &PortId,
    ) -> bool {
        context.allows_host_resource(
            "host",
            "port",
            &installation_port_resource_id(installation_id, port_id),
        )
    }

    fn query_context(context: &ProtocolContext) -> PowerboxQueryContext {
        let unrestricted = matches!(
            context.principal,
            ProtocolPrincipal::HostAdmin | ProtocolPrincipal::HostDev
        );
        let visible_resources = context
            .authority
            .as_ref()
            .into_iter()
            .flat_map(|authority| authority.resources.iter())
            .filter(|selector| selector.owner == "host")
            .map(|selector| ResourceSelector {
                kind: selector.kind.clone(),
                id: selector.id.clone().unwrap_or_else(|| "*".to_string()),
            })
            .collect::<Vec<_>>();
        let mut audience_principals = Vec::new();
        if let Some(grant_id) = context.host_device_grant_id() {
            audience_principals.push(ResourceSelector {
                kind: "grant".to_string(),
                id: grant_id.to_string(),
            });
        }
        PowerboxQueryContext {
            unrestricted,
            visible_resources,
            audience_principals,
        }
    }

    fn ensure_optional_run(
        context: &ProtocolContext,
        run: Option<&crate::RunRevisionPin>,
        method: &str,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            run.is_none_or(|pin| Self::allows_run(context, &pin.run_id)),
            "{method} permission denied: exact consumer Run authority is required when a Run pin is supplied"
        );
        Ok(())
    }

    async fn close_affected_bindings(
        &self,
        binding_ids: &[plurora_work::BindingId],
        reason_code: &str,
    ) {
        for binding_id in binding_ids {
            self.run_bindings
                .close_binding(binding_id, reason_code)
                .await;
        }
    }

    pub(crate) async fn dispatch_exposure_list(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        Self::ensure_powerbox_action(context, "observe", "host.exposure.list")?;
        let request: ExposureListRequest = serde_json::from_value(params)?;
        let views = self
            .config
            .powerbox_control
            .exposure_list(request)
            .await?
            .into_iter()
            .filter(|view| {
                Self::allows_installation(context, &view.record.installation_id)
                    && view
                        .record
                        .run_id
                        .as_ref()
                        .is_none_or(|run_id| Self::allows_run(context, run_id))
                    && Self::allows_port(
                        context,
                        &view.record.installation_id,
                        &view.record.export_port,
                    )
                    && context.allows_host_resource(
                        "host",
                        "exposure",
                        view.record.exposure_id.as_str(),
                    )
            })
            .collect::<Vec<_>>();
        Ok(serde_json::to_value(views)?)
    }

    pub(crate) async fn dispatch_exposure_create(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let mut request: ExposureCreateRequest = serde_json::from_value(params)?;
        request.validate()?;
        Self::ensure_powerbox_action(context, "exposure.manage", "host.exposure.create")?;
        anyhow::ensure!(
            Self::allows_installation(context, &request.installation_id)
                && Self::allows_run(context, &request.run_id)
                && Self::allows_port(context, &request.installation_id, &request.export_port),
            "host.exposure.create permission denied: exact Installation, Run, and export Port authority is required"
        );
        request.authority = Some(PowerboxMutationAuthority::verified(
            PowerboxAuthoritySubject::ExposureCreate {
                installation_id: request.installation_id.clone(),
                run_id: request.run_id.clone(),
                export_port: request.export_port.clone(),
            },
            context.host_device_grant_id().map(str::to_owned),
            context.verified_authority_expiry_ms(),
            context.powerbox_authority_refresh(),
        )?);
        let result = self
            .config
            .powerbox_control
            .exposure_create(request)
            .await?;
        self.close_affected_bindings(&result.affected_binding_ids, "exposure_changed")
            .await;
        Ok(serde_json::to_value(result)?)
    }

    pub(crate) async fn dispatch_exposure_revoke(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let mut request: ExposureRevokeRequest = serde_json::from_value(params)?;
        request.validate()?;
        Self::ensure_powerbox_action(context, "exposure.manage", "host.exposure.revoke")?;
        anyhow::ensure!(
            Self::allows_installation(context, &request.installation_id)
                && Self::allows_run(context, &request.run_id)
                && Self::allows_port(context, &request.installation_id, &request.export_port),
            "host.exposure.revoke permission denied: exact Installation, Run, and export Port authority is required"
        );
        // The durable controller validates that the referenced Exposure still
        // identifies the export Port covered at creation and that every parent
        // revision is exact.
        request.authority = Some(PowerboxMutationAuthority::verified(
            PowerboxAuthoritySubject::ExposureRevoke {
                installation_id: request.installation_id.clone(),
                run_id: request.run_id.clone(),
                export_port: request.export_port.clone(),
                exposure_id: request.exposure_id.clone(),
            },
            context.host_device_grant_id().map(str::to_owned),
            context.verified_authority_expiry_ms(),
            context.powerbox_authority_refresh(),
        )?);
        let result = self
            .config
            .powerbox_control
            .exposure_revoke(request)
            .await?;
        self.close_affected_bindings(&result.affected_binding_ids, "exposure_revoked")
            .await;
        Ok(serde_json::to_value(result)?)
    }

    pub(crate) async fn dispatch_binding_list(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        Self::ensure_powerbox_action(context, "observe", "host.binding.list")?;
        let mut request: BindingListRequest = serde_json::from_value(params)?;
        request.query = Some(Self::query_context(context));
        let views = self
            .config
            .powerbox_control
            .binding_list(request)
            .await?
            .into_iter()
            .filter(|view| {
                Self::allows_installation(
                    context,
                    &view.record.consumer.installation.installation_id,
                ) && Self::allows_port(
                    context,
                    &view.record.consumer.installation.installation_id,
                    &view.record.consumer.port.root_port,
                )
            })
            .collect::<Vec<_>>();
        Ok(serde_json::to_value(views)?)
    }

    pub(crate) async fn dispatch_binding_candidates(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        Self::ensure_powerbox_action(context, "observe", "host.binding.candidates")?;
        let mut request: BindingCandidatesRequest = serde_json::from_value(params)?;
        anyhow::ensure!(
            Self::allows_installation(context, &request.consumer_installation_id)
                && Self::allows_port(
                    context,
                    &request.consumer_installation_id,
                    &request.import_port,
                ),
            "host.binding.candidates permission denied: exact consumer Installation and import Port authority is required"
        );
        Self::ensure_optional_run(
            context,
            request.consumer_run.as_ref(),
            "host.binding.candidates",
        )?;
        request.query = Some(Self::query_context(context));
        request.validate()?;
        let result = self
            .config
            .powerbox_control
            .binding_candidates(request)
            .await?
            .stable()?;
        Ok(serde_json::to_value(result)?)
    }

    pub(crate) async fn dispatch_binding_select(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let mut request: BindingSelectRequest = serde_json::from_value(params)?;
        Self::ensure_powerbox_action(context, "binding.manage", "host.binding.select")?;
        anyhow::ensure!(
            Self::allows_installation(context, &request.consumer_installation_id)
                && Self::allows_port(
                    context,
                    &request.consumer_installation_id,
                    &request.import_port,
                )
                && context.allows_host_resource(
                    "host",
                    "exposure",
                    request.exposure_id.as_str(),
                ),
            "host.binding.select permission denied: exact consumer Installation, import Port, and Exposure authority is required"
        );
        Self::ensure_optional_run(
            context,
            request.consumer_run.as_ref(),
            "host.binding.select",
        )?;
        request.query = Some(Self::query_context(context));
        request.validate()?;
        request.authority = Some(PowerboxMutationAuthority::verified(
            PowerboxAuthoritySubject::BindingSelect {
                consumer_installation_id: request.consumer_installation_id.clone(),
                consumer_run: request.consumer_run.clone(),
                import_port: request.import_port.clone(),
                exposure_id: request.exposure_id.clone(),
            },
            context.host_device_grant_id().map(str::to_owned),
            context.verified_authority_expiry_ms(),
            context.powerbox_authority_refresh(),
        )?);
        let replay_request = request.clone();
        let mut result = self.config.powerbox_control.binding_select(request).await?;
        self.close_affected_bindings(&result.affected_binding_ids, "binding_replaced")
            .await;
        if result.binding.record.phase == plurora_work::BindingPhase::Runtime
            && result.binding.record.status == crate::BindingDecisionStatus::Selected
        {
            match self
                .attach_runtime_binding(result.binding.record.clone())
                .await
            {
                Ok(_) => {
                    result.binding.effective_status = crate::BindingEffectiveStatus::Active;
                }
                Err(error) => {
                    if error.is_definitive_drift() {
                        self.config
                            .powerbox_control
                            .binding_drifted(
                                &result.binding.record.binding_id,
                                error.reason_code(),
                            )
                            .await
                            .map_err(|terminal_error| anyhow::anyhow!(
                                "binding drift was detected after selection but its durable close obligation could not complete: {terminal_error}"
                            ))?;
                    }
                    // Validation may have durably terminalized a selection whose
                    // Run or provider drifted after its first successful call.
                    // Re-read the durable claim without minting another handle.
                    let mut terminal_replay = false;
                    if result.idempotent || error.is_definitive_drift() {
                        let replay = self
                            .config
                            .powerbox_control
                            .binding_select(replay_request)
                            .await?;
                        if replay.binding.record.status != crate::BindingDecisionStatus::Selected {
                            result = replay;
                            terminal_replay = true;
                        }
                    }
                    if !terminal_replay {
                        return Err(error.into());
                    }
                }
            }
        }
        Ok(serde_json::to_value(result)?)
    }

    pub(crate) async fn dispatch_binding_revoke(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let mut request: BindingRevokeRequest = serde_json::from_value(params)?;
        request.validate()?;
        Self::ensure_powerbox_action(context, "binding.manage", "host.binding.revoke")?;
        anyhow::ensure!(
            Self::allows_installation(context, &request.consumer_installation_id)
                && Self::allows_port(
                    context,
                    &request.consumer_installation_id,
                    &request.import_port,
                )
                && context.allows_host_resource(
                    "host",
                    "exposure",
                    request.exposure_id.as_str(),
                ),
            "host.binding.revoke permission denied: exact consumer Installation, import Port, and Exposure authority is required"
        );
        Self::ensure_optional_run(
            context,
            request.consumer_run.as_ref(),
            "host.binding.revoke",
        )?;
        request.authority = Some(PowerboxMutationAuthority::verified(
            PowerboxAuthoritySubject::BindingRevoke {
                consumer_installation_id: request.consumer_installation_id.clone(),
                consumer_run: request.consumer_run.clone(),
                import_port: request.import_port.clone(),
                exposure_id: request.exposure_id.clone(),
                binding_id: request.binding_id.clone(),
            },
            context.host_device_grant_id().map(str::to_owned),
            context.verified_authority_expiry_ms(),
            context.powerbox_authority_refresh(),
        )?);
        let result = self.config.powerbox_control.binding_revoke(request).await?;
        self.close_affected_bindings(&result.affected_binding_ids, "binding_revoked")
            .await;
        Ok(serde_json::to_value(result)?)
    }
}
