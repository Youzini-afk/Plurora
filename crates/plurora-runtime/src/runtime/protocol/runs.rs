use super::*;
use crate::{
    RunGetRequest, RunListRequest, RunMutationAuthority, RunStartRequest, RunStatusRequest,
    RunStopRequest,
};
use plurora_work::InstallationId;

impl<S> Runtime<S>
where
    S: EventStore,
{
    fn ensure_run_action(
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
                    installation_id.as_str(),
                ),
            "{method} permission denied: authenticated authority lacks '{action}' for the exact Installation"
        );
        Ok(())
    }

    fn ensure_exact_run_resource(
        context: &ProtocolContext,
        installation_id: &InstallationId,
        run_id: &plurora_work::RunId,
        method: &str,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            context.allows_host_resource("host", "run", run_id.as_str())
                || context.allows_host_resource(
                    "host",
                    "installation",
                    installation_id.as_str(),
                ),
            "{method} permission denied: authenticated authority lacks the exact Run or its exact parent Installation"
        );
        Ok(())
    }

    pub(crate) async fn dispatch_run_list(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        anyhow::ensure!(
            context.allows_host_action("observe"),
            "host.run.list permission denied: authenticated authority lacks observe"
        );
        let request: RunListRequest = serde_json::from_value(params)?;
        if let Some(installation_id) = request.installation_id.as_ref() {
            Self::ensure_run_action(context, "observe", installation_id, "host.run.list")?;
        }
        let runs = self
            .config
            .run_control
            .list(request)
            .await?
            .into_iter()
            .filter(|view| {
                context.allows_host_resource(
                    "host",
                    "installation",
                    view.record.installation_id.as_str(),
                ) && (context.allows_host_resource("host", "run", view.record.run_id.as_str())
                    || context.allows_host_resource(
                        "host",
                        "installation",
                        view.record.installation_id.as_str(),
                    ))
            })
            .collect::<Vec<_>>();
        Ok(serde_json::to_value(runs)?)
    }

    pub(crate) async fn dispatch_run_get(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: RunGetRequest = serde_json::from_value(params)?;
        Self::ensure_run_action(context, "observe", &request.installation_id, "host.run.get")?;
        Self::ensure_exact_run_resource(
            context,
            &request.installation_id,
            &request.run_id,
            "host.run.get",
        )?;
        let run = self
            .config
            .run_control
            .get(request)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Run not found"))?;
        Ok(serde_json::to_value(run)?)
    }

    pub(crate) async fn dispatch_run_status(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: RunStatusRequest = serde_json::from_value(params)?;
        Self::ensure_run_action(
            context,
            "observe",
            &request.installation_id,
            "host.run.status",
        )?;
        Ok(serde_json::to_value(
            self.config.run_control.status(request).await?,
        )?)
    }

    pub(crate) async fn dispatch_run_start(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let mut request: RunStartRequest = serde_json::from_value(params)?;
        request.validate()?;
        Self::ensure_run_action(context, "run", &request.installation_id, "host.run.start")?;
        // RunId is generated only after successful preflight. Creation authority
        // is therefore narrowly derived from the exact parent Installation;
        // clients never need authority for an unknown future child id.
        request.authority = Some(RunMutationAuthority::verified_for_installation(
            request.installation_id.clone(),
            context.host_device_grant_id().map(str::to_owned),
            context.verified_authority_expiry_ms(),
            context.run_authority_refresh(),
        )?);
        Ok(serde_json::to_value(
            self.config.run_control.start(request).await?,
        )?)
    }

    pub(crate) async fn dispatch_run_stop(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let mut request: RunStopRequest = serde_json::from_value(params)?;
        request.validate()?;
        Self::ensure_run_action(context, "run", &request.installation_id, "host.run.stop")?;
        Self::ensure_exact_run_resource(
            context,
            &request.installation_id,
            &request.run_id,
            "host.run.stop",
        )?;
        request.authority = Some(RunMutationAuthority::verified_for_run(
            request.installation_id.clone(),
            request.run_id.clone(),
            context.host_device_grant_id().map(str::to_owned),
            context.verified_authority_expiry_ms(),
            context.run_authority_refresh(),
        )?);
        Ok(serde_json::to_value(
            self.config.run_control.stop(request).await?,
        )?)
    }
}
