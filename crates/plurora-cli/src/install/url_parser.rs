use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

pub struct InstallUrl {
    pub source: InstallSource,
}

pub enum InstallSource {
    Git { url: String, r#ref: Option<String> },
    Local { path: PathBuf },
}

impl InstallUrl {
    pub fn url_for_resolver(&self) -> String {
        match &self.source {
            InstallSource::Git { url, .. } => url.clone(),
            InstallSource::Local { path } => path.to_string_lossy().to_string(),
        }
    }

    pub fn ref_or_default(&self) -> String {
        match &self.source {
            InstallSource::Git { r#ref, .. } => r#ref.clone().unwrap_or_else(|| "HEAD".to_string()),
            InstallSource::Local { .. } => String::new(),
        }
    }
}

pub fn parse_install_url(input: &str) -> Result<InstallUrl> {
    let input = input.trim();
    if input.is_empty() {
        anyhow::bail!("install source cannot be empty");
    }

    if is_rejected_scheme(input) {
        anyhow::bail!("unsupported install URL scheme; use HTTPS or a local path");
    }

    if is_local_path(input) {
        return Ok(InstallUrl {
            source: InstallSource::Local {
                path: expand_home(input)?,
            },
        });
    }

    if input.starts_with("http://") {
        anyhow::bail!("install URL must use https://");
    }

    let (raw_url, ref_name) = split_ref(input);
    let url = if raw_url.starts_with("https://") {
        normalize_https_url(raw_url)?
    } else if looks_like_git_short_form(raw_url) {
        normalize_https_url(&format!("https://{raw_url}"))?
    } else {
        return Err(anyhow!(
            "unsupported install source '{input}'; expected local path, github.com/user/repo, or https:// URL"
        ));
    };

    Ok(InstallUrl {
        source: InstallSource::Git {
            url,
            r#ref: ref_name.map(ToOwned::to_owned),
        },
    })
}

fn is_local_path(input: &str) -> bool {
    Path::new(input).exists()
        || Path::new(input).is_absolute()
        || input.starts_with("./")
        || input.starts_with("../")
        || input.starts_with("~/")
}

fn expand_home(input: &str) -> Result<PathBuf> {
    if let Some(rest) = input.strip_prefix("~/") {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("home dir unavailable for ~/ path"))?;
        Ok(home.join(rest))
    } else {
        Ok(PathBuf::from(input))
    }
}

fn split_ref(input: &str) -> (&str, Option<&str>) {
    match input.split_once('#') {
        Some((url, ref_name)) if !ref_name.is_empty() => (url, Some(ref_name)),
        Some((url, _)) => (url, None),
        None => (input, None),
    }
}

fn is_rejected_scheme(input: &str) -> bool {
    ["ssh://", "git://", "file://"]
        .iter()
        .any(|scheme| input.starts_with(scheme))
}

fn looks_like_git_short_form(input: &str) -> bool {
    let mut parts = input.split('/');
    let Some(host) = parts.next() else {
        return false;
    };
    host.contains('.') && parts.next().is_some() && parts.next().is_some()
}

fn normalize_https_url(raw: &str) -> Result<String> {
    let parsed = url::Url::parse(raw)?;
    if parsed.scheme() != "https" {
        anyhow::bail!("install URL must use https://");
    }
    if parsed.has_host() && !parsed.username().is_empty() || parsed.password().is_some() {
        anyhow::bail!("install URL must not include userinfo");
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("install URL must include a host"))?;
    let path = parsed.path().trim_end_matches('/');
    if path.is_empty() || path == "/" {
        anyhow::bail!("install URL must include a repository path");
    }
    let path = path.strip_suffix(".git").unwrap_or(path);
    Ok(format!("https://{host}{path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_short_form() {
        let parsed = parse_install_url("github.com/user/repo").unwrap();
        match parsed.source {
            InstallSource::Git { url, r#ref } => {
                assert_eq!(url, "https://github.com/user/repo");
                assert_eq!(r#ref, None);
            }
            InstallSource::Local { .. } => panic!("expected git source"),
        }
    }

    #[test]
    fn parses_github_short_form_with_ref() {
        let parsed = parse_install_url("github.com/user/repo#v1.0").unwrap();
        match parsed.source {
            InstallSource::Git { url, r#ref } => {
                assert_eq!(url, "https://github.com/user/repo");
                assert_eq!(r#ref.as_deref(), Some("v1.0"));
            }
            InstallSource::Local { .. } => panic!("expected git source"),
        }
    }

    #[test]
    fn strips_git_suffix() {
        let parsed = parse_install_url("https://github.com/user/repo.git").unwrap();
        match parsed.source {
            InstallSource::Git { url, r#ref } => {
                assert_eq!(url, "https://github.com/user/repo");
                assert_eq!(r#ref, None);
            }
            InstallSource::Local { .. } => panic!("expected git source"),
        }
    }

    #[test]
    fn parses_relative_local_path() {
        let parsed = parse_install_url("./packages/my-pkg").unwrap();
        match parsed.source {
            InstallSource::Local { path } => assert_eq!(path, PathBuf::from("./packages/my-pkg")),
            InstallSource::Git { .. } => panic!("expected local source"),
        }
    }

    #[test]
    fn parses_absolute_local_path() {
        let path = std::env::current_dir().unwrap().join("packages/my-pkg");
        let parsed = parse_install_url(&path.to_string_lossy()).unwrap();
        match parsed.source {
            InstallSource::Local { path: parsed_path } => assert_eq!(parsed_path, path),
            InstallSource::Git { .. } => panic!("expected local source"),
        }
    }

    #[test]
    fn expands_home_local_path() {
        let parsed = parse_install_url("~/path").unwrap();
        match parsed.source {
            InstallSource::Local { path } => {
                assert!(path.ends_with("path"));
                assert!(path.is_absolute());
            }
            InstallSource::Git { .. } => panic!("expected local source"),
        }
    }

    #[test]
    fn rejects_unsafe_schemes() {
        for input in [
            "ssh://github.com/user/repo",
            "git://github.com/user/repo",
            "file:///tmp/pkg",
        ] {
            assert!(
                parse_install_url(input).is_err(),
                "{input} should be rejected"
            );
        }
    }
}
