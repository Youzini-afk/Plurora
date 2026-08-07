use std::collections::BTreeSet;
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use plurora_core::{DependencySource, PackageManifest, ProjectDescriptor};
use serde_json::Value;

use super::types::{PackageDescriptor, PlannedRequirement, SourceDescriptor};

pub(super) fn parse_root_descriptor(root_url: &str, root_ref: &str) -> Result<PackageDescriptor> {
    if let Some(path) = root_url.strip_prefix("file://") {
        return Ok(PackageDescriptor {
            source: SourceDescriptor::Local {
                path: PathBuf::from(path),
            },
        });
    }
    if let Some(path) = root_url.strip_prefix("local:") {
        return Ok(PackageDescriptor {
            source: SourceDescriptor::Local {
                path: PathBuf::from(path),
            },
        });
    }
    let path = PathBuf::from(root_url);
    if path.exists() || root_url.starts_with('/') || root_url.starts_with('.') {
        return Ok(PackageDescriptor {
            source: SourceDescriptor::Local { path },
        });
    }
    let parsed = url::Url::parse(root_url)?;
    let mut url = parsed.clone();
    url.set_fragment(None);
    let ref_name = if root_ref.trim().is_empty() {
        parsed.fragment().unwrap_or("HEAD").to_string()
    } else {
        root_ref.to_string()
    };
    Ok(PackageDescriptor {
        source: SourceDescriptor::Git {
            url: url.to_string(),
            ref_name,
        },
    })
}

pub(super) fn resolve_dep(req: &PlannedRequirement) -> Result<PackageDescriptor> {
    let source: DependencySource = serde_json::from_value(req.source.clone())?;
    let source = match source {
        DependencySource::Internal => SourceDescriptor::Internal,
        DependencySource::Git { url, r#ref } => SourceDescriptor::Git {
            url,
            ref_name: r#ref,
        },
        DependencySource::Local { path } => SourceDescriptor::Local {
            path: PathBuf::from(path),
        },
    };
    Ok(PackageDescriptor { source })
}

pub(super) fn parse_manifest_at(path: &Path) -> Result<PackageManifest> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest = match path.extension().and_then(|ext| ext.to_str()) {
        Some("json") => serde_json::from_str(&raw)?,
        _ => serde_yaml::from_str(&raw)?,
    };
    Ok(manifest)
}

pub(super) fn parse_project_descriptor_at(path: &Path) -> Result<ProjectDescriptor> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read project descriptor {}", path.display()))?;
    let descriptor: ProjectDescriptor = serde_yaml::from_str(&raw)
        .map_err(|error| anyhow::anyhow!("invalid project.yaml: {error}"))?;
    descriptor.validate()?;
    Ok(descriptor)
}

pub(super) fn manifest_path_in(dir: &Path) -> Result<PathBuf> {
    for name in ["manifest.yaml", "manifest.yml", "manifest.json"] {
        let path = dir.join(name);
        if path.is_file() {
            return Ok(path);
        }
    }
    anyhow::bail!("no manifest.yaml or manifest.json in {}", dir.display())
}

pub(super) fn value_str<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing string field '{key}'"))
}

pub(super) fn sorted_vec(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn block_on_current<F>(future: F) -> F::Output
where
    F: Future,
{
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
}
