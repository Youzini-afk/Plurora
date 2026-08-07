use anyhow::{Context, Result};
use plurora_core::lockfile::Lockfile;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::IsTerminal;

pub fn approve_all(plan: &Value) -> Value {
    let summary = plan
        .pointer("/permissions_summary")
        .cloned()
        .unwrap_or(json!({}));
    let new_caps = summary
        .pointer("/new_capabilities")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let new_hosts = summary
        .pointer("/new_network_hosts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let new_secrets = summary
        .pointer("/new_secret_refs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    json!({
        "approved_capabilities": new_caps,
        "approved_network_hosts": new_hosts,
        "approved_secret_refs": new_secrets,
    })
}

pub fn prompt_for_consent(plan: &Value, existing_lockfile: Option<&str>) -> Result<Value> {
    let plan_perms = aggregate_plan_permissions(plan);
    let existing_perms = if let Some(lockfile_toml) = existing_lockfile {
        read_existing_grants(lockfile_toml).unwrap_or_default()
    } else {
        AggregatedPermissions::default()
    };
    let diff = diff_permissions(&plan_perms, &existing_perms);

    if diff.is_empty() {
        return Ok(approve_all(plan));
    }

    if !std::io::stdin().is_terminal() {
        anyhow::bail!("no TTY available; use --yes for non-interactive consent");
    }

    let summary = format_diff_summary(&diff);
    let prompt_text = format!("Install will request: {summary}. Continue?");
    let confirmed = dialoguer::Confirm::new()
        .with_prompt(prompt_text)
        .default(false)
        .interact()?;

    if !confirmed {
        anyhow::bail!("install declined by user");
    }

    Ok(json!({
        "approved_capabilities": diff.new_capabilities.iter().cloned().collect::<Vec<_>>(),
        "approved_network_hosts": diff.new_network_hosts.iter().cloned().collect::<Vec<_>>(),
        "approved_secret_refs": diff.new_secret_refs.iter().cloned().collect::<Vec<_>>(),
    }))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct AggregatedPermissions {
    capabilities: BTreeSet<String>,
    network_hosts: BTreeSet<String>,
    secret_refs: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PermissionDiff {
    new_capabilities: BTreeSet<String>,
    new_network_hosts: BTreeSet<String>,
    new_secret_refs: BTreeSet<String>,
    already_granted: AggregatedPermissions,
}

impl PermissionDiff {
    fn is_empty(&self) -> bool {
        self.new_capabilities.is_empty()
            && self.new_network_hosts.is_empty()
            && self.new_secret_refs.is_empty()
    }
}

fn aggregate_plan_permissions(plan: &Value) -> AggregatedPermissions {
    let mut permissions = AggregatedPermissions::default();
    if let Some(packages) = plan.pointer("/packages").and_then(Value::as_array) {
        for pkg in packages {
            collect_strings(
                pkg.pointer("/permissions/capabilities_invoke"),
                &mut permissions.capabilities,
            );
            collect_strings(
                pkg.pointer("/permissions/network_hosts"),
                &mut permissions.network_hosts,
            );
            collect_strings(
                pkg.pointer("/permissions/secret_refs"),
                &mut permissions.secret_refs,
            );
        }
    }
    permissions
}

fn collect_strings(value: Option<&Value>, output: &mut BTreeSet<String>) {
    if let Some(values) = value.and_then(Value::as_array) {
        output.extend(values.iter().filter_map(Value::as_str).map(str::to_string));
    }
}

fn read_existing_grants(lockfile_toml: &str) -> Result<AggregatedPermissions> {
    let lockfile: Lockfile =
        toml::from_str(lockfile_toml).context("failed to parse lockfile TOML")?;
    lockfile.validate()?;

    let mut permissions = AggregatedPermissions::default();
    for entry in lockfile.package {
        permissions.capabilities.extend(entry.granted_capabilities);
        permissions.network_hosts.extend(entry.granted_network);
        permissions.secret_refs.extend(entry.granted_secrets);
    }
    Ok(permissions)
}

fn diff_permissions(
    plan: &AggregatedPermissions,
    existing: &AggregatedPermissions,
) -> PermissionDiff {
    PermissionDiff {
        new_capabilities: plan
            .capabilities
            .difference(&existing.capabilities)
            .cloned()
            .collect(),
        new_network_hosts: plan
            .network_hosts
            .difference(&existing.network_hosts)
            .cloned()
            .collect(),
        new_secret_refs: plan
            .secret_refs
            .difference(&existing.secret_refs)
            .cloned()
            .collect(),
        already_granted: AggregatedPermissions {
            capabilities: plan
                .capabilities
                .intersection(&existing.capabilities)
                .cloned()
                .collect(),
            network_hosts: plan
                .network_hosts
                .intersection(&existing.network_hosts)
                .cloned()
                .collect(),
            secret_refs: plan
                .secret_refs
                .intersection(&existing.secret_refs)
                .cloned()
                .collect(),
        },
    }
}

fn format_diff_summary(diff: &PermissionDiff) -> String {
    let mut parts = Vec::new();
    if !diff.new_capabilities.is_empty() {
        parts.push(format!(
            "{} capability invocation(s)",
            diff.new_capabilities.len()
        ));
    }
    if !diff.new_network_hosts.is_empty() {
        parts.push(format!(
            "{} network host(s): {}",
            diff.new_network_hosts.len(),
            diff.new_network_hosts
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !diff.new_secret_refs.is_empty() {
        parts.push(format!(
            "{} secret reference(s)",
            diff.new_secret_refs.len()
        ));
    }
    if parts.is_empty() {
        return "no new permissions".to_string();
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approve_all_extracts_all_new_permissions() {
        let plan = json!({
            "permissions_summary": {
                "new_capabilities": ["model.live_call", "vector.search"],
                "new_network_hosts": ["api.openai.com"],
                "new_secret_refs": ["OPENAI_API_KEY"]
            }
        });

        let consent = approve_all(&plan);

        assert_eq!(
            consent["approved_capabilities"],
            json!(["model.live_call", "vector.search"])
        );
        assert_eq!(consent["approved_network_hosts"], json!(["api.openai.com"]));
        assert_eq!(consent["approved_secret_refs"], json!(["OPENAI_API_KEY"]));
    }

    #[test]
    fn diff_detects_new_capabilities() {
        let plan = AggregatedPermissions {
            capabilities: ["a", "b", "c"].into_iter().map(String::from).collect(),
            network_hosts: BTreeSet::default(),
            secret_refs: BTreeSet::default(),
        };
        let existing = AggregatedPermissions {
            capabilities: ["a", "b"].into_iter().map(String::from).collect(),
            network_hosts: BTreeSet::default(),
            secret_refs: BTreeSet::default(),
        };

        let diff = diff_permissions(&plan, &existing);

        assert_eq!(
            diff.new_capabilities,
            ["c"].into_iter().map(String::from).collect()
        );
        assert_eq!(
            diff.already_granted.capabilities,
            ["a", "b"].into_iter().map(String::from).collect()
        );
    }

    #[test]
    fn aggregate_plan_permissions_collects_all_package_permissions() {
        let plan = json!({
            "packages": [
                {
                    "permissions": {
                        "capabilities_invoke": ["model.live_call"],
                        "network_hosts": ["api.openai.com"],
                        "secret_refs": ["secret_ref:env:OPENAI_API_KEY"]
                    }
                },
                {
                    "permissions": {
                        "capabilities_invoke": ["vector.search", "model.live_call"],
                        "network_hosts": ["example.com"],
                        "secret_refs": []
                    }
                }
            ]
        });

        let permissions = aggregate_plan_permissions(&plan);

        assert_eq!(
            permissions.capabilities,
            ["model.live_call", "vector.search"]
                .into_iter()
                .map(String::from)
                .collect()
        );
        assert_eq!(
            permissions.network_hosts,
            ["api.openai.com", "example.com"]
                .into_iter()
                .map(String::from)
                .collect()
        );
        assert_eq!(
            permissions.secret_refs,
            ["secret_ref:env:OPENAI_API_KEY"]
                .into_iter()
                .map(String::from)
                .collect()
        );
    }

    #[test]
    fn diff_summary_renders_single_line() {
        let plan = AggregatedPermissions {
            capabilities: ["model.live_call", "vector.search"]
                .into_iter()
                .map(String::from)
                .collect(),
            network_hosts: ["api.openai.com"].into_iter().map(String::from).collect(),
            secret_refs: ["secret_ref:env:OPENAI_API_KEY"]
                .into_iter()
                .map(String::from)
                .collect(),
        };
        let diff = diff_permissions(&plan, &AggregatedPermissions::default());

        assert_eq!(
            format_diff_summary(&diff),
            "2 capability invocation(s), 1 network host(s): api.openai.com, 1 secret reference(s)"
        );
    }
}
