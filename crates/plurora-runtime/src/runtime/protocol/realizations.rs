use super::*;
use crate::{
    RealizationApplyRequest, RealizationAuthoritySubject, RealizationGetRequest,
    RealizationListRequest, RealizationMutationAuthority, RealizationPlanRequest,
    RealizationReconcileRequest, RealizationRollbackRequest, RealizationStopRequest,
    REALIZATION_APPLY_ACTION, REALIZATION_PLAN_ACTION,
};
use plurora_work::{InstallationId, RealizationId};

impl<S> Runtime<S>
where
    S: EventStore,
{
    fn ensure_realization_resource(
        context: &ProtocolContext,
        installation_id: &InstallationId,
        target_id: &str,
        realization_ids: &[RealizationId],
        action: &str,
        method: &str,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            context.allows_host_action(action)
                && context.allows_host_resource(
                    "host",
                    "installation",
                    installation_id.as_str(),
                )
                && context.allows_host_resource("host", "target", target_id)
                && realization_ids.iter().all(|realization_id| context
                    .allows_host_resource("host", "realization", realization_id.as_str())),
            "{method} permission denied: {action} plus exact Installation, Target, and Realization resources are required"
        );
        Ok(())
    }

    fn mutation_authority(
        context: &ProtocolContext,
        subject: RealizationAuthoritySubject,
    ) -> anyhow::Result<RealizationMutationAuthority> {
        RealizationMutationAuthority::verified(
            subject,
            context.host_device_grant_id().map(str::to_owned),
            context.verified_authority_expiry_ms(),
            context.realization_authority_refresh(),
        )
    }

    pub(crate) async fn dispatch_realization_plan(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: RealizationPlanRequest = serde_json::from_value(params)?;
        let subject = RealizationAuthoritySubject::Plan {
            installation_id: request.installation_id.clone(),
            target_id: request.target_id.clone(),
        };
        anyhow::ensure!(
            context.allows_host_action(REALIZATION_PLAN_ACTION)
                && context.allows_host_resource(
                    "host",
                    "installation",
                    request.installation_id.as_str(),
                )
                && context.allows_host_resource("host", "target", &request.target_id),
            "host.realization.plan permission denied: exact Installation and Target are required"
        );
        Ok(serde_json::to_value(
            self.config
                .realization_control
                .plan(request, Self::mutation_authority(context, subject)?)
                .await?,
        )?)
    }

    pub(crate) async fn dispatch_realization_apply(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: RealizationApplyRequest = serde_json::from_value(params)?;
        let ids = vec![request.realization_id.clone()];
        Self::ensure_realization_resource(
            context,
            &request.installation_id,
            &request.target_id,
            &ids,
            REALIZATION_APPLY_ACTION,
            "host.realization.apply",
        )?;
        let subject = RealizationAuthoritySubject::Apply {
            installation_id: request.installation_id.clone(),
            target_id: request.target_id.clone(),
            realization_ids: ids,
        };
        Ok(serde_json::to_value(
            self.config
                .realization_control
                .apply(request, Self::mutation_authority(context, subject)?)
                .await?,
        )?)
    }

    pub(crate) async fn dispatch_realization_get(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: RealizationGetRequest = serde_json::from_value(params)?;
        anyhow::ensure!(
            context.allows_host_action("observe")
                && context.allows_host_resource(
                    "host",
                    "installation",
                    request.installation_id.as_str(),
                )
                && context.allows_host_resource(
                    "host",
                    "realization",
                    request.realization_id.as_str(),
                ),
            "host.realization.get permission denied: observe plus exact Installation and Realization are required"
        );
        let value = self
            .config
            .realization_control
            .get(request)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Realization not found"))?;
        Ok(serde_json::to_value(value)?)
    }

    pub(crate) async fn dispatch_realization_list(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        anyhow::ensure!(
            context.allows_host_action("observe"),
            "host.realization.list permission denied: observe is required"
        );
        let request: RealizationListRequest = serde_json::from_value(params)?;
        let values = self
            .config
            .realization_control
            .list(request)
            .await?
            .into_iter()
            .filter(|realization| {
                context.allows_host_resource(
                    "host",
                    "installation",
                    realization.installation_id.as_str(),
                ) && context.allows_host_resource(
                    "host",
                    "realization",
                    realization.realization_id.as_str(),
                )
            })
            .collect::<Vec<_>>();
        Ok(serde_json::to_value(values)?)
    }

    pub(crate) async fn dispatch_realization_stop(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: RealizationStopRequest = serde_json::from_value(params)?;
        let ids = vec![request.realization_id.clone()];
        Self::ensure_realization_resource(
            context,
            &request.installation_id,
            &request.target_id,
            &ids,
            REALIZATION_APPLY_ACTION,
            "host.realization.stop",
        )?;
        let subject = RealizationAuthoritySubject::Apply {
            installation_id: request.installation_id.clone(),
            target_id: request.target_id.clone(),
            realization_ids: ids,
        };
        Ok(serde_json::to_value(
            self.config
                .realization_control
                .stop(request, Self::mutation_authority(context, subject)?)
                .await?,
        )?)
    }

    pub(crate) async fn dispatch_realization_rollback(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: RealizationRollbackRequest = serde_json::from_value(params)?;
        let ids = vec![
            request.realization_id.clone(),
            request.rollback_to_realization_id.clone(),
        ];
        Self::ensure_realization_resource(
            context,
            &request.installation_id,
            &request.target_id,
            &ids,
            REALIZATION_APPLY_ACTION,
            "host.realization.rollback",
        )?;
        let subject = RealizationAuthoritySubject::Apply {
            installation_id: request.installation_id.clone(),
            target_id: request.target_id.clone(),
            realization_ids: ids,
        };
        Ok(serde_json::to_value(
            self.config
                .realization_control
                .rollback(request, Self::mutation_authority(context, subject)?)
                .await?,
        )?)
    }

    pub(crate) async fn dispatch_realization_reconcile(
        &self,
        context: &ProtocolContext,
        params: Value,
    ) -> anyhow::Result<Value> {
        let request: RealizationReconcileRequest = serde_json::from_value(params)?;
        let ids = vec![request.realization_id.clone()];
        Self::ensure_realization_resource(
            context,
            &request.installation_id,
            &request.target_id,
            &ids,
            REALIZATION_APPLY_ACTION,
            "host.realization.reconcile",
        )?;
        let subject = RealizationAuthoritySubject::Apply {
            installation_id: request.installation_id.clone(),
            target_id: request.target_id.clone(),
            realization_ids: ids,
        };
        Ok(serde_json::to_value(
            self.config
                .realization_control
                .reconcile(request, Self::mutation_authority(context, subject)?)
                .await?,
        )?)
    }
}
