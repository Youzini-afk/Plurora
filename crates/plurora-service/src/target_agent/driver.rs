use plurora_runtime::{ExecutionTarget, ExecutionTargetReachability};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TargetDriverKind {
    Local,
    Agent,
}

pub(super) fn resolve_target_driver(target: &ExecutionTarget) -> TargetDriverKind {
    match target.reachability {
        ExecutionTargetReachability::LocalHost => TargetDriverKind::Local,
        ExecutionTargetReachability::Direct | ExecutionTargetReachability::ReverseTunnel => {
            TargetDriverKind::Agent
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plurora_runtime::{ExecutionTargetCapability, ExecutionTargetStatusKind};
    use std::collections::BTreeMap;

    fn target(id: &str, reachability: ExecutionTargetReachability) -> ExecutionTarget {
        ExecutionTarget {
            id: id.to_string(),
            name: id.to_string(),
            reachability,
            declared_capabilities: vec![ExecutionTargetCapability::HealthProbe],
            capabilities: vec![ExecutionTargetCapability::HealthProbe],
            status: ExecutionTargetStatusKind::Available,
            protocol_versions: vec!["target-agent.v1".to_string()],
            selected_protocol_version: Some("target-agent.v1".to_string()),
            identity_ref: Some(format!("target-agent:{id}")),
            labels: BTreeMap::new(),
            observed: None,
            last_seen_at_ms: None,
            heartbeat_expires_at_ms: None,
            enrolled_at_ms: None,
            revoked_at_ms: None,
            lease_epoch: 1,
            policy_epoch: 1,
        }
    }

    #[test]
    fn routes_local_and_agent_targets_without_address_fallback() {
        assert_eq!(
            resolve_target_driver(&target("local", ExecutionTargetReachability::LocalHost)),
            TargetDriverKind::Local
        );
        assert_eq!(
            resolve_target_driver(&target(
                "remote-1",
                ExecutionTargetReachability::ReverseTunnel
            )),
            TargetDriverKind::Agent
        );
        assert_eq!(
            resolve_target_driver(&target("remote-2", ExecutionTargetReachability::Direct)),
            TargetDriverKind::Agent
        );
    }
}
