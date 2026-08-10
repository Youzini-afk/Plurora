use std::collections::BTreeSet;
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::Result;
use plurora_core::{DependencySource, PackageDependency, PackageManifest};
use serde_json::Value;

#[derive(Debug, Clone)]
pub(super) enum SourceDescriptor {
    Git { url: String, ref_name: String },
    Local { path: PathBuf },
    Internal,
}

pub(super) fn parse_root_descriptor(root_url: &str, root_ref: &str) -> Result<SourceDescriptor> {
    if root_url.to_ascii_lowercase().starts_with("file://") {
        let parsed =
            url::Url::parse(root_url).map_err(|_| anyhow::anyhow!("source file URL is invalid"))?;
        let path = parsed
            .to_file_path()
            .map_err(|_| anyhow::anyhow!("source file URL is invalid"))?;
        return Ok(SourceDescriptor::Local { path });
    }
    if let Some(path) = root_url.strip_prefix("local:") {
        return Ok(SourceDescriptor::Local {
            path: PathBuf::from(path),
        });
    }
    let path = PathBuf::from(root_url);
    if path.exists() || path.is_absolute() || root_url.starts_with('.') {
        return Ok(SourceDescriptor::Local { path });
    }
    let parsed = url::Url::parse(root_url)
        .map_err(|_| anyhow::anyhow!("source must be a local path or an absolute URL"))?;
    let mut url = parsed.clone();
    url.set_fragment(None);
    let ref_name = if root_ref.trim().is_empty() {
        parsed.fragment().unwrap_or("HEAD").to_string()
    } else {
        root_ref.to_string()
    };
    Ok(SourceDescriptor::Git {
        url: url.to_string(),
        ref_name,
    })
}

pub(super) fn dependency_source(
    dependency: &PackageDependency,
    base_dir: &Path,
) -> Result<SourceDescriptor> {
    Ok(match &dependency.source {
        DependencySource::Internal => SourceDescriptor::Internal,
        DependencySource::Git { url, r#ref } => SourceDescriptor::Git {
            url: url.clone(),
            ref_name: r#ref.clone(),
        },
        DependencySource::Local { path } => {
            let path = PathBuf::from(path);
            SourceDescriptor::Local {
                path: if path.is_absolute() {
                    path
                } else {
                    base_dir.join(path)
                },
            }
        }
    })
}

pub(super) fn parse_manifest_at(path: &Path) -> Result<PackageManifest> {
    let raw = fs::read_to_string(path)
        .map_err(|_| anyhow::anyhow!("package manifest could not be read"))?;
    let manifest: PackageManifest = match path.extension().and_then(|extension| extension.to_str())
    {
        Some("json") => serde_json::from_str(&raw)
            .map_err(|_| anyhow::anyhow!("package manifest is malformed"))?,
        _ => serde_yaml::from_str(&raw)
            .map_err(|_| anyhow::anyhow!("package manifest is malformed"))?,
    };
    Ok(manifest)
}

pub(super) fn manifest_path_in(directory: &Path) -> Result<PathBuf> {
    for name in ["manifest.yaml", "manifest.json"] {
        let path = directory.join(name);
        if path.is_file() {
            return Ok(path);
        }
    }
    anyhow::bail!("package source does not contain manifest.yaml or manifest.json")
}

pub(super) fn value_str<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("helper response is missing required field '{key}'"))
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
