use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use plurora_core::PackageManifest;

pub(crate) async fn read_manifest(path: PathBuf) -> Result<PackageManifest> {
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read package manifest {}", path.display()))?;
    let manifest = match path.extension().and_then(|ext| ext.to_str()) {
        Some("yaml") | Some("yml") => serde_yaml::from_str(&raw)
            .with_context(|| format!("failed to parse YAML package manifest {}", path.display()))?,
        _ => serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse JSON package manifest {}", path.display()))?,
    };
    Ok(manifest)
}

pub(crate) async fn validate_manifest(path: PathBuf) -> Result<()> {
    let manifest = read_manifest(path).await?;
    manifest.validate_basic()?;
    println!("valid manifest: {}@{}", manifest.id, manifest.version);
    Ok(())
}
