//! Per-package effect audit: aggregate actual capability/network/secret use
//! from the event store, compare it to manifest declarations, and suggest
//! tightening unused authority.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use plurora_core::{
    CapabilityId, PackageId, EVENT_ASSET_PUT, EVENT_CAPABILITY_INVOKED,
    EVENT_OUTBOUND_EXECUTE_COMPLETED, EVENT_OUTBOUND_STREAM_COMPLETED,
    EVENT_OUTBOUND_WEBSOCKET_COMPLETED,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Runtime;
use crate::EventStore;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AuditPackageParams {
    pub package_id: PackageId,
    #[serde(default)]
    pub since: Option<DateTime<Utc>>,
    #[serde(default)]
    pub until: Option<DateTime<Utc>>,
}

impl AuditPackageParams {
    pub fn window(&self) -> (DateTime<Utc>, DateTime<Utc>) {
        let until = self.until.unwrap_or_else(Utc::now);
        let since = self.since.unwrap_or_else(|| until - Duration::days(7));
        (since, until)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PackageAuditReport {
    pub package_id: PackageId,
    pub since: DateTime<Utc>,
    pub until: DateTime<Utc>,
    pub declared: DeclaredAuthority,
    pub used: UsedAuthority,
    pub unused: UnusedAuthority,
    pub suggestions: Vec<TighteningSuggestion>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeclaredAuthority {
    pub capabilities_invoke: Vec<CapabilityId>,
    pub network_hosts: Vec<String>,
    pub secret_refs: Vec<String>,
    pub events_read: bool,
    pub events_append: bool,
    pub assets_read: bool,
    pub assets_write: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct UsedAuthority {
    pub capabilities_invoked: HashMap<CapabilityId, u64>,
    pub network_hosts_used: HashMap<String, u64>,
    pub secret_refs_used: HashMap<String, u64>,
    pub events_read_count: u64,
    pub events_append_count: u64,
    pub assets_read_count: u64,
    pub assets_write_count: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct UnusedAuthority {
    pub capabilities_unused: Vec<CapabilityId>,
    pub network_hosts_unused: Vec<String>,
    pub secret_refs_unused: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TighteningSuggestion {
    pub kind: String,
    pub target: String,
    pub rationale: String,
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub async fn audit_package(
        &self,
        package_id: &PackageId,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> anyhow::Result<PackageAuditReport> {
        let manifest = self
            .packages
            .manifest(package_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("package '{package_id}' is not loaded"))?;

        let declared = DeclaredAuthority {
            capabilities_invoke: sorted_unique(manifest.permissions.capabilities.invoke.clone()),
            network_hosts: sorted_unique(
                manifest
                    .permissions
                    .network
                    .hosts
                    .iter()
                    .cloned()
                    .chain(
                        manifest
                            .permissions
                            .network
                            .declarations
                            .iter()
                            .map(|declaration| declaration.host.clone()),
                    )
                    .collect(),
            ),
            secret_refs: sorted_unique(manifest.permissions.secret_refs.clone()),
            events_read: manifest.permissions.events.read,
            events_append: manifest.permissions.events.append,
            assets_read: manifest.permissions.assets.read,
            assets_write: manifest.permissions.assets.write,
        };

        let mut used = UsedAuthority::default();
        let events = self.store.list_all().await?;
        for event in events {
            if event.timestamp < since || event.timestamp > until {
                continue;
            }

            match event.kind.as_str() {
                EVENT_CAPABILITY_INVOKED => {
                    if event
                        .payload
                        .get("caller_package_id")
                        .and_then(|value| value.as_str())
                        == Some(package_id.as_str())
                    {
                        if let Some(capability_id) = event
                            .payload
                            .get("capability_id")
                            .and_then(|value| value.as_str())
                        {
                            increment(&mut used.capabilities_invoked, capability_id.to_string());
                        }
                    }
                }
                EVENT_OUTBOUND_EXECUTE_COMPLETED
                | EVENT_OUTBOUND_STREAM_COMPLETED
                | EVENT_OUTBOUND_WEBSOCKET_COMPLETED => {
                    if event
                        .payload
                        .get("package_id")
                        .and_then(|value| value.as_str())
                        == Some(package_id.as_str())
                    {
                        if let Some(host) = event
                            .payload
                            .get("destination_host")
                            .and_then(|value| value.as_str())
                        {
                            increment(&mut used.network_hosts_used, host.to_string());
                        }
                        for secret_ref in event
                            .payload
                            .get("secret_refs_used")
                            .and_then(|value| value.as_array())
                            .into_iter()
                            .flatten()
                            .filter_map(|value| value.as_str())
                        {
                            increment(&mut used.secret_refs_used, secret_ref.to_string());
                        }
                    }
                }
                EVENT_ASSET_PUT => {
                    if event
                        .payload
                        .get("origin_package_id")
                        .and_then(|value| value.as_str())
                        == Some(package_id.as_str())
                    {
                        used.assets_write_count += 1;
                    }
                    if event.writer_package_id == *package_id {
                        used.events_append_count += 1;
                    }
                }
                _ => {
                    if event.writer_package_id == *package_id {
                        used.events_append_count += 1;
                    }
                }
            }
        }

        let used_caps: HashSet<&str> = used
            .capabilities_invoked
            .keys()
            .map(String::as_str)
            .collect();
        let used_hosts: HashSet<&str> =
            used.network_hosts_used.keys().map(String::as_str).collect();
        let used_secrets: HashSet<&str> =
            used.secret_refs_used.keys().map(String::as_str).collect();

        let capabilities_unused = declared
            .capabilities_invoke
            .iter()
            .filter(|capability_id| !used_caps.contains(capability_id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let network_hosts_unused = declared
            .network_hosts
            .iter()
            .filter(|host| !used_hosts.contains(host.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let secret_refs_unused = declared
            .secret_refs
            .iter()
            .filter(|secret_ref| !used_secrets.contains(secret_ref.as_str()))
            .cloned()
            .collect::<Vec<_>>();

        let unused = UnusedAuthority {
            capabilities_unused,
            network_hosts_unused,
            secret_refs_unused,
        };
        let suggestions = tightening_suggestions(&unused, since, until);

        Ok(PackageAuditReport {
            package_id: package_id.clone(),
            since,
            until,
            declared,
            used,
            unused,
            suggestions,
        })
    }
}

fn increment(map: &mut HashMap<String, u64>, key: String) {
    *map.entry(key).or_insert(0) += 1;
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn tightening_suggestions(
    unused: &UnusedAuthority,
    since: DateTime<Utc>,
    until: DateTime<Utc>,
) -> Vec<TighteningSuggestion> {
    let days = (until - since).num_days().max(1);
    let mut suggestions = Vec::new();

    for capability_id in &unused.capabilities_unused {
        suggestions.push(TighteningSuggestion {
            kind: "remove_capability".to_string(),
            target: capability_id.clone(),
            rationale: format!(
                "Remove \"{capability_id}\" from permissions.capabilities.invoke (no calls in {days} days)"
            ),
        });
    }
    for host in &unused.network_hosts_unused {
        suggestions.push(TighteningSuggestion {
            kind: "remove_host".to_string(),
            target: host.clone(),
            rationale: format!(
                "Remove \"{host}\" from permissions.network declarations/hosts (no calls in {days} days)"
            ),
        });
    }
    for secret_ref in &unused.secret_refs_unused {
        suggestions.push(TighteningSuggestion {
            kind: "remove_secret_ref".to_string(),
            target: secret_ref.clone(),
            rationale: format!(
                "Remove \"{secret_ref}\" from permissions.secret_refs (no use in {days} days)"
            ),
        });
    }

    suggestions
}
