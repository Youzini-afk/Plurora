use super::*;
use sha2::{Digest, Sha256};
use std::fs;
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
    project_id: Option<String>,
    source: SurfaceBundleSource,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum SurfaceBundleSource {
    InstalledProject,
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

        for entry in self.config.project_registry.list() {
            if !context.allows_host_resource(
                "host",
                "project",
                entry.descriptor.project.id.as_str(),
            ) {
                continue;
            }
            if let Some(bundle) = self.try_resolve_via_project(&entry, surface_id)? {
                return Ok(serde_json::to_value(bundle)?);
            }
        }

        if context.allows_all_host_resources("host", "project") {
            if let Some(bundle) = self.try_resolve_via_dev_path(surface_id)? {
                return Ok(serde_json::to_value(bundle)?);
            }
        }

        anyhow::bail!("surface_not_found: {surface_id}")
    }

    fn try_resolve_via_project(
        &self,
        entry: &crate::ProjectEntry,
        surface_id: &str,
    ) -> anyhow::Result<Option<ResolvedSurfaceBundle>> {
        if entry.descriptor.project.entry_surface_id.as_deref() != Some(surface_id) {
            return Ok(None);
        }

        let project_id = entry.descriptor.project.id.as_str();
        let dist_dir = recoverable_project_dist_dir(&entry.descriptor.project.id);
        let bundle_path = dist_dir.as_ref().map(|path| path.join("bundle.mjs"));
        let fingerprint = bundle_path.as_deref().and_then(bundle_fingerprint);
        let bundle_path = format!("/surface-bundles/projects/{project_id}/bundle.mjs");
        let stylesheets = dist_dir
            .as_deref()
            .map(|path| discover_project_stylesheets(project_id, path))
            .unwrap_or_default();
        let wrapper_class = (!stylesheets.is_empty()).then(|| surface_wrapper_class(surface_id));
        Ok(Some(ResolvedSurfaceBundle {
            surface_id: surface_id.to_string(),
            bundle_url: cache_busted_url(&bundle_path, fingerprint.as_deref()),
            bundle_fingerprint: fingerprint,
            export_name: default_surface_export_name(surface_id),
            stylesheets,
            wrapper_class,
            project_id: Some(project_id.to_string()),
            source: SurfaceBundleSource::InstalledProject,
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
            project_id: None,
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
        // project ownership. Until that relationship is explicit, an exact-project
        // device may resolve its project's bundle but cannot enumerate the global
        // contribution catalogue.
        if !context.allows_all_host_resources("host", "project") {
            anyhow::bail!(
                "{method} permission denied: global surface catalogue requires all-project authority"
            );
        }
        Ok(())
    }
}

fn discover_project_stylesheets(project_id: &str, dist_dir: &Path) -> Vec<String> {
    let styles_dir = dist_dir.join("styles");
    let mut entries = match fs::read_dir(styles_dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("css") {
                    return None;
                }
                let name = path.file_name()?.to_str()?;
                if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
                    return None;
                }
                Some((stylesheet_order(name), name.to_string(), path))
            })
            .collect::<Vec<_>>(),
        Err(_) => return Vec::new(),
    };
    entries.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    entries
        .into_iter()
        .map(|(_, name, path)| {
            let url = format!("/surface-bundles/projects/{project_id}/styles/{name}");
            cache_busted_url(&url, bundle_fingerprint(&path).as_deref())
        })
        .collect()
}

fn stylesheet_order(name: &str) -> u8 {
    match name {
        "surface.css" => 0,
        "mobile.css" => 1,
        _ => 2,
    }
}

fn surface_wrapper_class(surface_id: &str) -> String {
    let prefix = surface_id.split('/').next().unwrap_or(surface_id);
    format!("{}-surface", prefix.replace(['/', '_'], "-"))
}

fn recoverable_project_dist_dir(project_id: &ProjectId) -> Option<PathBuf> {
    let project_dir = plurora_core::paths::project_dir(project_id).ok()?;
    let dist = project_dir.join("dist");
    if dist.is_dir() {
        return Some(dist);
    }
    latest_dist_backup(&project_dir)
}

fn latest_dist_backup(project_dir: &Path) -> Option<PathBuf> {
    let mut candidates = fs::read_dir(project_dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            if !name.starts_with(".dist.bak-") || !path.is_dir() {
                return None;
            }
            let modified = entry.metadata().and_then(|meta| meta.modified()).ok();
            Some((modified, path))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(modified, _)| *modified);
    candidates.pop().map(|(_, path)| path)
}
