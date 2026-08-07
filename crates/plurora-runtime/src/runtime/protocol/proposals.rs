use super::*;

impl<S> Runtime<S>
where
    S: EventStore,
{
    async fn ensure_host_device_proposal_access(
        &self,
        context: &ProtocolContext,
        action: &str,
        proposal: &crate::runtime::ProposalRecord,
    ) -> anyhow::Result<()> {
        if !context.is_host_device() {
            return Ok(());
        }
        if !context.allows_host_action(action) {
            anyhow::bail!("Host device authority does not include action '{action}'");
        }
        if let Some(session_id) = proposal.target_session_id.as_deref() {
            self.ensure_host_session_access(context, action, session_id)
                .await
        } else if context.allows_all_host_resources("host", "project") {
            Ok(())
        } else {
            anyhow::bail!("project-scoped Host device cannot access an unbound proposal")
        }
    }

    // --- Proposal ---

    pub(crate) async fn dispatch_proposal_create(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let proposal: crate::runtime::ProposalRecord = serde_json::from_value(params.clone())?;
        self.ensure_host_device_proposal_access(context, "develop_propose", &proposal)
            .await?;
        Ok(serde_json::to_value(
            self.create_proposal(context, proposal).await?,
        )?)
    }

    pub(crate) async fn dispatch_proposal_get(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let proposal_id = params
            .get("proposal_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.proposal.get requires proposal_id"))?;
        let proposal = self.get_proposal(proposal_id).await?;
        self.ensure_host_device_proposal_access(context, "observe", &proposal)
            .await?;
        Ok(serde_json::to_value(proposal)?)
    }

    pub(crate) async fn dispatch_proposal_list(
        &self,
        context: &ProtocolContext,
    ) -> anyhow::Result<Value> {
        let mut visible = Vec::new();
        for proposal in self.list_proposals().await {
            if self
                .ensure_host_device_proposal_access(context, "observe", &proposal)
                .await
                .is_ok()
            {
                visible.push(proposal);
            }
        }
        Ok(serde_json::to_value(visible)?)
    }

    pub(crate) async fn dispatch_proposal_approve(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let proposal_id = params
            .get("proposal_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.proposal.approve requires proposal_id"))?;
        let reason = params
            .get("reason")
            .and_then(Value::as_str)
            .map(str::to_string);
        let proposal = self.get_proposal(proposal_id).await?;
        self.ensure_host_device_proposal_access(context, "develop_approve", &proposal)
            .await?;
        Ok(serde_json::to_value(
            self.approve_proposal(context, proposal_id, reason).await?,
        )?)
    }

    pub(crate) async fn dispatch_proposal_reject(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let proposal_id = params
            .get("proposal_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.proposal.reject requires proposal_id"))?;
        let reason = params
            .get("reason")
            .and_then(Value::as_str)
            .map(str::to_string);
        let proposal = self.get_proposal(proposal_id).await?;
        self.ensure_host_device_proposal_access(context, "develop_approve", &proposal)
            .await?;
        Ok(serde_json::to_value(
            self.reject_proposal(context, proposal_id, reason).await?,
        )?)
    }

    pub(crate) async fn dispatch_proposal_apply(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let proposal_id = params
            .get("proposal_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("kernel.v1.proposal.apply requires proposal_id"))?;
        let proposal = self.get_proposal(proposal_id).await?;
        self.ensure_host_device_proposal_access(context, "develop_execute", &proposal)
            .await?;
        Ok(serde_json::to_value(
            self.apply_proposal(context, proposal_id).await?,
        )?)
    }
}
