use super::*;
use crate::{
    InstallationCreateRequest, InstallationGetRequest, InstallationListRequest,
    InstallationMutationAuthority, InstallationRemoveRequest, InstallationUpdateRequest,
};
use plurora_work::InstallationId;

impl<S> Runtime<S>
where
    S: EventStore,
{
    fn ensure_installation_action(
        context: &ProtocolContext,
        action: &str,
        installation_id: &InstallationId,
        method: &str,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            context.allows_host_action(action)
                && context.allows_host_resource(
                    "host",
                    "installation",
                    installation_id.as_str()
                ),
            "{method} permission denied: authenticated authority lacks '{action}' for the exact installation"
        );
        Ok(())
    }

    pub(crate) async fn dispatch_installation_list(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        anyhow::ensure!(
            context.allows_host_action("observe"),
            "host.installation.list permission denied: authenticated authority lacks observe"
        );
        let request: InstallationListRequest = serde_json::from_value(params)?;
        let installations = self
            .config
            .installation_control
            .list(request)
            .await?
            .into_iter()
            .filter(|view| {
                context.allows_host_resource(
                    "host",
                    "installation",
                    view.record.installation_id.as_str(),
                )
            })
            .collect::<Vec<_>>();
        Ok(serde_json::to_value(installations)?)
    }

    pub(crate) async fn dispatch_installation_get(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: InstallationGetRequest = serde_json::from_value(params)?;
        Self::ensure_installation_action(
            context,
            "observe",
            &request.installation_id,
            "host.installation.get",
        )?;
        let view = self
            .config
            .installation_control
            .get(&request.installation_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("installation not found"))?;
        Ok(serde_json::to_value(view)?)
    }

    pub(crate) async fn dispatch_installation_create(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let mut request: InstallationCreateRequest = serde_json::from_value(params)?;
        request.validate()?;
        anyhow::ensure!(
            context.allows_host_action("installation.manage"),
            "host.installation.create permission denied: authenticated authority lacks installation.manage"
        );
        anyhow::ensure!(
            context.allows_host_resource("host", "work", request.work_id.as_str()),
            "host.installation.create permission denied: authenticated authority lacks the exact Work"
        );
        request.authority = Some(InstallationMutationAuthority::verified_for_work(
            request.work_id.clone(),
            context.host_device_grant_id().map(str::to_owned),
            context.verified_authority_expiry_ms(),
            context.installation_authority_refresh(),
        )?);
        Ok(serde_json::to_value(
            self.config.installation_control.create(request).await?,
        )?)
    }

    pub(crate) async fn dispatch_installation_update(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let mut request: InstallationUpdateRequest = serde_json::from_value(params)?;
        request.validate()?;
        Self::ensure_installation_action(
            context,
            "installation.manage",
            &request.installation_id,
            "host.installation.update",
        )?;
        request.authority = Some(InstallationMutationAuthority::verified_for_installation(
            request.installation_id.clone(),
            context.host_device_grant_id().map(str::to_owned),
            context.verified_authority_expiry_ms(),
            context.installation_authority_refresh(),
        )?);
        let result = self.config.installation_control.update(request).await?;
        let invalidated = self
            .config
            .powerbox_control
            .installation_changed(
                &result.installation.record.installation_id,
                Some(result.installation.revision),
            )
            .await?;
        for binding_id in invalidated.affected_binding_ids {
            self.run_bindings
                .close_binding(&binding_id, "installation_updated")
                .await;
        }
        Ok(serde_json::to_value(result)?)
    }

    pub(crate) async fn dispatch_installation_remove(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let mut request: InstallationRemoveRequest = serde_json::from_value(params)?;
        request.validate()?;
        Self::ensure_installation_action(
            context,
            "installation.manage",
            &request.installation_id,
            "host.installation.remove",
        )?;
        request.authority = Some(InstallationMutationAuthority::verified_for_installation(
            request.installation_id.clone(),
            context.host_device_grant_id().map(str::to_owned),
            context.verified_authority_expiry_ms(),
            context.installation_authority_refresh(),
        )?);
        let result = self.config.installation_control.remove(request).await?;
        let invalidated = self
            .config
            .powerbox_control
            .installation_changed(&result.installation.record.installation_id, None)
            .await?;
        for binding_id in invalidated.affected_binding_ids {
            self.run_bindings
                .close_binding(&binding_id, "installation_removed")
                .await;
        }
        Ok(serde_json::to_value(result)?)
    }
}
