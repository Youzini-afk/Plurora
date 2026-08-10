use super::*;
use plurora_core::PackageEntry;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Component;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
struct ResolvedSurfaceBundle {
    surface_id: String,
    bundle_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle_fingerprint: Option<String>,
    export_name: String,
    stylesheets: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wrapper_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    package_id: Option<String>,
    source: SurfaceBundleSource,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum SurfaceBundleSource {
    Package,
    DevPath,
}

fn surface_prefix_matches(surface_id: &str, prefix: &str) -> bool {
    surface_id == prefix
        || surface_id
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn default_surface_export_name(surface_id: &str) -> String {
    if surface_id.starts_with("ydltavern/") {
        match surface_id {
            "ydltavern/play" | "ydltavern/surface" => "mountTavernPlaySurface".to_string(),
            "ydltavern/settings" => "mountTavernSettingsSurface".to_string(),
            "ydltavern/extensions" => "mountTavernExtensionsSurface".to_string(),
            "ydltavern/character" => "mountTavernCharactersSurface".to_string(),
            "ydltavern/world-info" => "mountTavernWorldInfoSurface".to_string(),
            "ydltavern/persona" => "mountTavernPersonaSurface".to_string(),
            "ydltavern/ai-response-config" => "mountTavernAIResponseConfigSurface".to_string(),
            "ydltavern/user-settings" => "mountTavernUserSettingsSurface".to_string(),
            "ydltavern/backgrounds" => "mountTavernBackgroundsSurface".to_string(),
            _ => "mountTavernPlaySurface".to_string(),
        }
    } else {
        "mountSurface".to_string()
    }
}

fn default_surface_stylesheets(prefix: &str) -> Vec<String> {
    if prefix == "ydltavern" {
        vec![
            "/surface-bundles/ydltavern/styles/surface.css".to_string(),
            "/surface-bundles/ydltavern/styles/mobile.css".to_string(),
        ]
    } else {
        Vec::new()
    }
}

fn cache_busted_url(path: &str, fingerprint: Option<&str>) -> String {
    match fingerprint {
        Some(value) => format!("{path}?v={value}"),
        None => path.to_string(),
    }
}

fn bundle_fingerprint(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let digest = Sha256::digest(&bytes);
    Some(
        digest[..8]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    // --- Surface ---

    pub(crate) async fn dispatch_surface_resolve_bundle(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        if !context.allows_host_action("observe") {
            anyhow::bail!(
                "host.surface.bundle.resolve permission denied: authenticated authority lacks observe"
            );
        }

        let surface_id = params
            .get("surface_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("surface_id required"))?;

        if context.allows_all_host_resources("host", "installation") {
            if let Some(bundle) = self.try_resolve_via_dev_path(surface_id)? {
                return Ok(serde_json::to_value(bundle)?);
            }
            if let Some(bundle) = self.try_resolve_via_package(surface_id).await? {
                return Ok(serde_json::to_value(bundle)?);
            }
        }

        anyhow::bail!("surface_not_found: {surface_id}")
    }

    async fn try_resolve_via_package(
        &self,
        surface_id: &str,
    ) -> anyhow::Result<Option<ResolvedSurfaceBundle>> {
        let mut matches = Vec::new();
        for package in self.list_packages().await {
            if !package
                .manifest
                .contributes
                .surfaces
                .iter()
                .any(|surface| surface.id == surface_id)
            {
                continue;
            }
            let PackageEntry::SurfaceBundle { bundle } = &package.manifest.entry.kind else {
                continue;
            };
            let Some(package_root) = self.config.package_roots.get(&package.id) else {
                continue;
            };
            let Some((bundle_file, bundle_relative)) =
                contained_package_bundle(package_root, bundle)
            else {
                continue;
            };
            matches.push((package.id, bundle_file, bundle_relative));
        }
        anyhow::ensure!(
            matches.len() <= 1,
            "surface bundle resolution is ambiguous for the requested surface"
        );
        let Some((package_id, bundle_file, bundle_relative)) = matches.pop() else {
            return Ok(None);
        };
        let fingerprint = bundle_fingerprint(&bundle_file);
        let bundle_path = format!(
            "/surface-bundles/packages/{}/{}",
            package_id,
            path_for_url(&bundle_relative)
        );
        Ok(Some(ResolvedSurfaceBundle {
            surface_id: surface_id.to_string(),
            bundle_url: cache_busted_url(&bundle_path, fingerprint.as_deref()),
            bundle_fingerprint: fingerprint,
            export_name: default_surface_export_name(surface_id),
            stylesheets: Vec::new(),
            wrapper_class: Some(surface_wrapper_class(surface_id)),
            package_id: Some(package_id),
            source: SurfaceBundleSource::Package,
        }))
    }

    fn try_resolve_via_dev_path(
        &self,
        surface_id: &str,
    ) -> anyhow::Result<Option<ResolvedSurfaceBundle>> {
        let Some((prefix, path)) = self
            .config
            .surface_dev_paths
            .iter()
            .filter(|(prefix, _)| surface_prefix_matches(surface_id, prefix))
            .max_by_key(|(prefix, _)| prefix.len())
        else {
            return Ok(None);
        };

        let bundle_path = PathBuf::from(path).join("bundle.mjs");
        let fingerprint = bundle_fingerprint(&bundle_path);
        let bundle_url = format!("/surface-bundles/{prefix}/bundle.mjs");

        Ok(Some(ResolvedSurfaceBundle {
            surface_id: surface_id.to_string(),
            bundle_url: cache_busted_url(&bundle_url, fingerprint.as_deref()),
            bundle_fingerprint: fingerprint,
            export_name: default_surface_export_name(surface_id),
            stylesheets: default_surface_stylesheets(prefix),
            wrapper_class: Some(format!("{}-surface", prefix.replace(['/', '_'], "-"))),
            package_id: None,
            source: SurfaceBundleSource::DevPath,
        }))
    }

    pub(crate) async fn dispatch_surface_list(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        self.ensure_surface_catalog_access(context, "shell.contribution.list")?;
        let slot = params
            .get("slot")
            .and_then(Value::as_str)
            .map(str::to_string);
        Ok(self.list_surface_contributions(slot).await)
    }

    pub(crate) async fn dispatch_surface_describe(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        self.ensure_surface_catalog_access(context, "shell.contribution.describe")?;
        let surface_id = params
            .get("surface_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("shell.contribution.describe requires surface_id"))?;
        self.describe_surface_contribution(surface_id).await
    }

    fn ensure_surface_catalog_access(
        &self,
        context: &ProtocolContext,
        method: &str,
    ) -> anyhow::Result<()> {
        if !context.allows_host_action("observe") {
            anyhow::bail!("{method} permission denied: authenticated authority lacks observe");
        }
        // Contributions are currently installed at Host scope and do not carry
        // Installation ownership. Until that relationship is explicit, an exact-
        // Installation device cannot enumerate the global
        // contribution catalogue.
        if !context.allows_all_host_resources("host", "installation") {
            anyhow::bail!(
                "{method} permission denied: global surface catalogue requires all-installation authority"
            );
        }
        Ok(())
    }
}

fn surface_wrapper_class(surface_id: &str) -> String {
    let prefix = surface_id.split('/').next().unwrap_or(surface_id);
    format!("{}-surface", prefix.replace(['/', '_'], "-"))
}

fn contained_package_bundle(package_root: &Path, bundle: &str) -> Option<(PathBuf, PathBuf)> {
    let mut relative = PathBuf::new();
    for component in Path::new(bundle).components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if relative.as_os_str().is_empty() {
        return None;
    }
    let root = fs::canonicalize(package_root).ok()?;
    let bundle_file = fs::canonicalize(root.join(&relative)).ok()?;
    if !bundle_file.is_file() || !bundle_file.starts_with(&root) {
        return None;
    }
    Some((bundle_file, relative))
}

fn path_for_url(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_bundle_resolution_is_contained_by_the_configured_root() -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join("dist"))?;
        fs::write(root.path().join("dist/bundle.mjs"), "export {}")?;

        let resolved = contained_package_bundle(root.path(), "dist/bundle.mjs")
            .expect("contained bundle resolves");
        assert!(resolved.0.starts_with(fs::canonicalize(root.path())?));
        assert_eq!(resolved.1, PathBuf::from("dist/bundle.mjs"));
        assert!(contained_package_bundle(root.path(), "../bundle.mjs").is_none());
        assert!(contained_package_bundle(root.path(), "/bundle.mjs").is_none());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn package_bundle_resolution_rejects_a_symlink_escape() -> anyhow::Result<()> {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        fs::write(outside.path().join("bundle.mjs"), "export {}")?;
        symlink(
            outside.path().join("bundle.mjs"),
            root.path().join("bundle.mjs"),
        )?;

        assert!(contained_package_bundle(root.path(), "bundle.mjs").is_none());
        Ok(())
    }
}
