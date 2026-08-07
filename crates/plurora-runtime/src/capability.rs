use std::collections::HashMap;

use plurora_core::{
    ArtifactDescriptor, CapHandleId, CapabilityDescriptor, CapabilityId, ComponentBoundaryClaims,
    ComponentClaimStatus, ComponentDescriptor, ComponentTrustClass, EffectReplayMode,
    HookSubscription, PackageId,
};
use schemars::{
    gen::SchemaGenerator,
    schema::{InstanceType, Metadata, Schema, SchemaObject, SingleOrVec},
    JsonSchema,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RegisteredCapability {
    pub descriptor: CapabilityDescriptor,
    pub provider_package_id: PackageId,
    #[serde(default)]
    pub provider_component_id: String,
    #[serde(default)]
    pub provider_component_digest: String,
    #[serde(default)]
    pub provider_behavior_digest: String,
    #[serde(default)]
    pub provider_trust_class: ComponentTrustClass,
    #[serde(default)]
    pub provider_claim_status: ComponentClaimStatus,
    #[serde(default)]
    pub provider_enforced_boundaries: ComponentBoundaryClaims,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CapabilityInvocationRequest {
    /// New preferred path: invoke by kernel-minted handle.
    #[serde(default)]
    pub handle: Option<CapHandleId>,
    /// Legacy path. Runtime auto-mints a one-use transient handle when no
    /// handle is supplied.
    #[serde(default)]
    pub capability_id: Option<CapabilityId>,
    #[serde(default)]
    pub caller_package_id: Option<PackageId>,
    #[serde(default)]
    pub provider_package_id: Option<PackageId>,
    #[serde(default)]
    pub version: Option<String>,
    /// Kernel session id this invocation is part of, if any.
    /// Propagated from ProtocolContext for downstream scope resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub input: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CapabilityInvocationResult {
    pub capability_id: CapabilityId,
    pub provider_package_id: PackageId,
    #[serde(default)]
    pub provider_component_id: String,
    #[serde(default)]
    pub provider_component_digest: String,
    #[serde(default)]
    pub provider_behavior_digest: String,
    #[serde(default)]
    pub provider_trust_class: ComponentTrustClass,
    pub output: Value,
    pub duration_ms: u64,
    #[schemars(schema_with = "uuid_schema")]
    pub correlation_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_mode: Option<EffectReplayMode>,
}

fn uuid_schema(_gen: &mut SchemaGenerator) -> Schema {
    let mut schema = SchemaObject::default();
    schema.instance_type = Some(SingleOrVec::Single(Box::new(InstanceType::String)));
    schema.format = Some("uuid".to_string());
    schema.metadata = Some(Box::new(Metadata::default()));
    Schema::Object(schema)
}

#[derive(Default)]
pub struct CapabilityFabric {
    providers: RwLock<HashMap<CapabilityId, Vec<RegisteredCapability>>>,
}

impl CapabilityFabric {
    pub async fn register_component(
        &self,
        package_id: &PackageId,
        component: &ComponentDescriptor,
        descriptors: &[CapabilityDescriptor],
    ) -> anyhow::Result<()> {
        for descriptor in descriptors {
            anyhow::ensure!(
                component.capability_ids.contains(&descriptor.id),
                "component '{}' does not claim capability '{}'",
                component.component_id,
                descriptor.id
            );
        }
        let mut providers = self.providers.write().await;
        for descriptor in descriptors {
            providers
                .entry(descriptor.id.clone())
                .or_default()
                .push(RegisteredCapability {
                    descriptor: descriptor.clone(),
                    provider_package_id: package_id.clone(),
                    provider_component_id: component.component_id.clone(),
                    provider_component_digest: component.artifact.digest.clone(),
                    provider_behavior_digest: component.behavior.digest.clone(),
                    provider_trust_class: component.trust_class,
                    provider_claim_status: component.claim_status,
                    provider_enforced_boundaries: component.enforced_boundaries.clone(),
                });
        }
        Ok(())
    }

    /// Compatibility adapter for embedders that still register raw package
    /// capabilities. Such providers are deliberately marked as foreign until
    /// they supply a component descriptor.
    pub async fn register_package(
        &self,
        package_id: &PackageId,
        descriptors: &[CapabilityDescriptor],
    ) {
        let mut providers = self.providers.write().await;
        for descriptor in descriptors {
            providers
                .entry(descriptor.id.clone())
                .or_default()
                .push(RegisteredCapability {
                    descriptor: descriptor.clone(),
                    provider_package_id: package_id.clone(),
                    provider_component_id: format!("{package_id}/component/default"),
                    provider_component_digest: format!("sha256:{}", "0".repeat(64)),
                    provider_behavior_digest: format!("sha256:{}", "0".repeat(64)),
                    provider_trust_class: ComponentTrustClass::ForeignCapsule,
                    provider_claim_status: ComponentClaimStatus::ForeignCapsule,
                    provider_enforced_boundaries: ComponentBoundaryClaims::default(),
                });
        }
    }

    pub async fn unregister_package(&self, package_id: &PackageId) {
        let mut providers = self.providers.write().await;
        providers.retain(|_, registered| {
            registered.retain(|capability| &capability.provider_package_id != package_id);
            !registered.is_empty()
        });
    }

    pub async fn discover(&self) -> Vec<RegisteredCapability> {
        let mut values: Vec<_> = self
            .providers
            .read()
            .await
            .values()
            .flat_map(|providers| providers.iter().cloned())
            .collect();
        values.sort_by(|a, b| {
            a.descriptor
                .id
                .cmp(&b.descriptor.id)
                .then(a.provider_package_id.cmp(&b.provider_package_id))
                .then(a.provider_component_id.cmp(&b.provider_component_id))
        });
        values
    }

    pub async fn describe(&self, capability_id: &CapabilityId) -> Vec<RegisteredCapability> {
        self.providers
            .read()
            .await
            .get(capability_id)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn resolve(
        &self,
        capability_id: &CapabilityId,
        provider_package_id: Option<&PackageId>,
        version: Option<&str>,
    ) -> anyhow::Result<RegisteredCapability> {
        let mut providers = self.describe(capability_id).await;
        if let Some(provider_package_id) = provider_package_id {
            providers.retain(|provider| &provider.provider_package_id == provider_package_id);
        }
        if let Some(version) = version {
            providers.retain(|provider| version_matches(version, &provider.descriptor.version));
        }
        match providers.as_slice() {
            [] => anyhow::bail!("capability '{}' has no provider", capability_id),
            [provider] => Ok(provider.clone()),
            _ => anyhow::bail!(
                "capability '{}' has ambiguous providers; specify provider_package_id",
                capability_id
            ),
        }
    }
}

fn version_matches(requirement: &str, provided: &str) -> bool {
    if requirement == "*" || requirement == provided {
        return true;
    }
    if let Some(major) = requirement
        .strip_prefix('^')
        .and_then(|req| req.split('.').next())
    {
        return provided.split('.').next() == Some(major);
    }
    false
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RegisteredHook {
    pub subscriber_package_id: PackageId,
    pub subscription: HookSubscription,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionDispatchResult {
    pub extension_point: String,
    pub invoked: Vec<RegisteredHook>,
    pub vetoed_by: Option<PackageId>,
    pub payload: Value,
}

#[derive(Default)]
pub struct ExtensionRegistry {
    hooks: RwLock<HashMap<String, Vec<RegisteredHook>>>,
}

impl ExtensionRegistry {
    pub async fn register_package(&self, package_id: &PackageId, hooks: &[HookSubscription]) {
        let mut registry = self.hooks.write().await;
        for hook in hooks {
            registry
                .entry(hook.extension_point.clone())
                .or_default()
                .push(RegisteredHook {
                    subscriber_package_id: package_id.clone(),
                    subscription: hook.clone(),
                });
        }
        for hooks in registry.values_mut() {
            hooks.sort_by(|a, b| {
                a.subscription
                    .precedence
                    .cmp(&b.subscription.precedence)
                    .then(a.subscriber_package_id.cmp(&b.subscriber_package_id))
                    .then(a.subscription.handler.cmp(&b.subscription.handler))
            });
        }
    }

    pub async fn unregister_package(&self, package_id: &PackageId) {
        let mut registry = self.hooks.write().await;
        registry.retain(|_, hooks| {
            hooks.retain(|hook| &hook.subscriber_package_id != package_id);
            !hooks.is_empty()
        });
    }

    pub async fn list_hooks(&self, extension_point: &str) -> Vec<RegisteredHook> {
        self.hooks
            .read()
            .await
            .get(extension_point)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn list_all_hooks(&self) -> Vec<RegisteredHook> {
        self.hooks
            .read()
            .await
            .values()
            .flat_map(|hooks| hooks.iter().cloned())
            .collect()
    }

    pub async fn dispatch(&self, extension_point: &str, payload: Value) -> ExtensionDispatchResult {
        let invoked = self.list_hooks(extension_point).await;
        let vetoed_by = invoked
            .iter()
            .find(|hook| hook.subscription.handler == "veto")
            .map(|hook| hook.subscriber_package_id.clone());
        let mut payload = payload;
        for hook in &invoked {
            if hook.subscription.handler == "metadata_trace" {
                if let Some(object) = payload.as_object_mut() {
                    let metadata = object
                        .entry("metadata")
                        .or_insert_with(|| Value::Object(Default::default()));
                    if let Some(metadata) = metadata.as_object_mut() {
                        metadata.insert(
                            "hook_trace".to_string(),
                            Value::String(hook.subscriber_package_id.clone()),
                        );
                    }
                }
            }
        }
        ExtensionDispatchResult {
            extension_point: extension_point.to_string(),
            invoked,
            vetoed_by,
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use plurora_core::{CapabilityDescriptor, HookSubscription, HookTiming};
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn ambiguous_capability_is_rejected() {
        let fabric = CapabilityFabric::default();
        let descriptor = CapabilityDescriptor {
            id: "example/echo/echo".to_string(),
            version: "0.1.0".to_string(),
            input_schema: Value::Null,
            output_schema: Value::Null,
            streaming: false,
            side_effects: Vec::new(),
            description: None,
        };
        fabric
            .register_package(&"example/a".to_string(), &[descriptor.clone()])
            .await;
        fabric
            .register_package(&"example/b".to_string(), &[descriptor])
            .await;

        let result = fabric
            .resolve(&"example/echo/echo".to_string(), None, None)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn explicit_provider_resolves_conflict() -> anyhow::Result<()> {
        let fabric = CapabilityFabric::default();
        let descriptor = CapabilityDescriptor {
            id: "example/echo/echo".to_string(),
            version: "0.1.0".to_string(),
            input_schema: Value::Null,
            output_schema: Value::Null,
            streaming: false,
            side_effects: Vec::new(),
            description: None,
        };
        fabric
            .register_package(&"example/a".to_string(), &[descriptor.clone()])
            .await;
        fabric
            .register_package(&"example/b".to_string(), &[descriptor])
            .await;

        let result = fabric
            .resolve(
                &"example/echo/echo".to_string(),
                Some(&"example/b".to_string()),
                Some("^0.1"),
            )
            .await?;
        assert_eq!(result.provider_package_id, "example/b");
        Ok(())
    }

    #[tokio::test]
    async fn hook_dispatch_reports_veto() {
        let registry = ExtensionRegistry::default();
        registry
            .register_package(
                &"example/veto".to_string(),
                &[HookSubscription {
                    extension_point: "context/before_open".to_string(),
                    handler: "veto".to_string(),
                    timing: HookTiming::Sync,
                    precedence: 0,
                }],
            )
            .await;
        let result = registry.dispatch("context/before_open", json!({})).await;
        assert_eq!(result.vetoed_by, Some("example/veto".to_string()));
    }
}
