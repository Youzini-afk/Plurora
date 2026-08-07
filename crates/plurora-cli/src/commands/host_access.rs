use anyhow::Context;
use reqwest::redirect::Policy;
use reqwest::{Client, Method, StatusCode};
use serde_json::{json, Value};
use url::Host;

const VALID_SCOPES: &[&str] = &[
    "observe",
    "project_operate",
    "deploy",
    "develop_propose",
    "develop_approve",
    "develop_execute",
    "access_manage",
];

pub async fn me(endpoint: &str, access_token: &str) -> anyhow::Result<()> {
    print_json(
        request(
            endpoint,
            access_token,
            Method::GET,
            "/host/v1/access/me",
            None,
        )
        .await?,
    )
}

pub async fn list(endpoint: &str, access_token: &str) -> anyhow::Result<()> {
    print_json(request(endpoint, access_token, Method::GET, "/host/v1/access", None).await?)
}

pub async fn pair(
    endpoint: &str,
    access_token: &str,
    device_name: String,
    mut scopes: Vec<String>,
    projects: Vec<String>,
    targets: Vec<String>,
    grant_days: u64,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !device_name.trim().is_empty(),
        "device_name cannot be empty"
    );
    anyhow::ensure!(
        (1..=365).contains(&grant_days),
        "grant_days must be between 1 and 365"
    );
    scopes.sort();
    scopes.dedup();
    anyhow::ensure!(
        scopes
            .iter()
            .all(|scope| VALID_SCOPES.contains(&scope.as_str())),
        "unknown Host access scope; valid values: {}",
        VALID_SCOPES.join(", ")
    );
    if !scopes.iter().any(|scope| scope == "observe") {
        scopes.push("observe".to_string());
    }
    let mut resources = Vec::new();
    if projects.is_empty() {
        resources.push(json!({"kind": "project"}));
    } else {
        for id in normalized_ids(projects, "project")? {
            resources.push(json!({"kind": "project", "id": id}));
        }
    }
    if targets.is_empty() {
        resources.push(json!({"kind": "target"}));
    } else {
        for id in normalized_ids(targets, "target")? {
            resources.push(json!({"kind": "target", "id": id}));
        }
    }
    let body = json!({
        "device_name": device_name.trim(),
        "scopes": scopes,
        "resources": resources,
        "pairing_ttl_secs": 600,
        "grant_ttl_secs": grant_days.saturating_mul(24 * 60 * 60),
    });
    print_json(
        request(
            endpoint,
            access_token,
            Method::POST,
            "/host/v1/access/pairings",
            Some(body),
        )
        .await?,
    )
}

pub async fn revoke(endpoint: &str, access_token: &str, grant_id: &str) -> anyhow::Result<()> {
    anyhow::ensure!(!grant_id.trim().is_empty(), "grant_id cannot be empty");
    let path = format!(
        "/host/v1/access/grants/{}/revoke",
        url::form_urlencoded::byte_serialize(grant_id.trim().as_bytes()).collect::<String>()
    );
    print_json(request(endpoint, access_token, Method::POST, &path, Some(json!({}))).await?)
}

pub async fn bulk_revoke(
    endpoint: &str,
    access_token: &str,
    grant_ids: Vec<String>,
) -> anyhow::Result<()> {
    let grant_ids = normalized_ids(grant_ids, "grant")?;
    anyhow::ensure!(!grant_ids.is_empty(), "at least one grant id is required");
    print_json(
        request(
            endpoint,
            access_token,
            Method::POST,
            "/host/v1/access/grants/revoke",
            Some(json!({"grant_ids": grant_ids})),
        )
        .await?,
    )
}

pub async fn projects(endpoint: &str, access_token: &str) -> anyhow::Result<()> {
    print_json(rpc(endpoint, access_token, "host.project.list", json!({})).await?)
}

pub async fn targets(endpoint: &str, access_token: &str) -> anyhow::Result<()> {
    print_json(rpc(endpoint, access_token, "host.target.list", json!({})).await?)
}

pub async fn project_status(
    endpoint: &str,
    access_token: &str,
    project_id: &str,
) -> anyhow::Result<()> {
    print_json(
        rpc(
            endpoint,
            access_token,
            "host.project.status",
            json!({"project_id": project_id}),
        )
        .await?,
    )
}

pub async fn target_status(
    endpoint: &str,
    access_token: &str,
    target_id: &str,
) -> anyhow::Result<()> {
    print_json(
        rpc(
            endpoint,
            access_token,
            "host.target.status",
            json!({"target_id": target_id}),
        )
        .await?,
    )
}

async fn rpc(
    endpoint: &str,
    access_token: &str,
    method: &str,
    params: Value,
) -> anyhow::Result<Value> {
    let response = request(
        endpoint,
        access_token,
        Method::POST,
        "/rpc",
        Some(json!({"id": "cli", "method": method, "params": params})),
    )
    .await?;
    if let Some(error) = response.get("error") {
        anyhow::bail!("Host RPC returned an error: {error}");
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Host RPC response is missing result"))
}

fn normalized_ids(ids: Vec<String>, kind: &str) -> anyhow::Result<Vec<String>> {
    let mut normalized = ids
        .into_iter()
        .map(|id| id.trim().to_string())
        .collect::<Vec<_>>();
    anyhow::ensure!(
        normalized.iter().all(|id| !id.is_empty()),
        "{kind} ids cannot be empty"
    );
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

pub(crate) async fn request(
    endpoint: &str,
    access_token: &str,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> anyhow::Result<Value> {
    let url = host_url(endpoint, path)?;
    // Host control APIs do not use redirects. Refusing them also prevents a
    // bearer credential from ever being replayed to a redirected origin.
    let client = Client::builder().redirect(Policy::none()).build()?;
    let mut request = client
        .request(method, url)
        .bearer_auth(access_token)
        .header("accept", "application/json");
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.context("Host access request failed")?;
    let status = response.status();
    let bytes = response.bytes().await?;
    if !status.is_success() {
        anyhow::bail!(
            "Host access request returned {}: {}",
            status,
            String::from_utf8_lossy(&bytes)
        );
    }
    if status == StatusCode::NO_CONTENT || bytes.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(&bytes).context("Host returned invalid JSON")
}

pub(crate) fn normalize_host_endpoint(endpoint: &str) -> anyhow::Result<String> {
    let url = url::Url::parse(endpoint.trim())
        .with_context(|| format!("invalid Host endpoint '{}'", endpoint.trim()))?;
    anyhow::ensure!(
        matches!(url.scheme(), "http" | "https"),
        "Host endpoint must use http or https"
    );
    anyhow::ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Host endpoint must not contain userinfo, query, or fragment components"
    );
    anyhow::ensure!(
        url.path() == "/",
        "Host endpoint must be an origin without a path"
    );
    anyhow::ensure!(url.port() != Some(0), "Host endpoint port must be non-zero");
    if url.scheme() == "http" {
        let loopback = match url.host() {
            Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
            Some(Host::Ipv4(address)) => address.is_loopback(),
            Some(Host::Ipv6(address)) => address.is_loopback(),
            None => false,
        };
        anyhow::ensure!(
            loopback,
            "remote Host access requires https; http is allowed only for loopback"
        );
    }
    Ok(url.origin().ascii_serialization())
}

pub(crate) fn host_url(endpoint: &str, path: &str) -> anyhow::Result<url::Url> {
    anyhow::ensure!(
        path.starts_with('/') && !path.starts_with("//"),
        "Host API path must be origin-relative"
    );
    let endpoint = normalize_host_endpoint(endpoint)?;
    url::Url::parse(&endpoint)?
        .join(path)
        .context("failed to resolve Host API URL")
}

pub(crate) fn print_json(value: Value) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{host_url, normalize_host_endpoint};

    #[test]
    fn host_access_http_is_loopback_only() {
        assert!(host_url("http://127.0.0.1:8787", "/host/v1/access/me").is_ok());
        assert!(host_url("http://[::1]:8787", "/host/v1/access/me").is_ok());
        assert!(host_url("http://localhost:8787", "/host/v1/access/me").is_ok());
        assert!(host_url("http://example.test:8787", "/host/v1/access/me").is_err());
        assert!(host_url("https://example.test", "/host/v1/access/me").is_ok());
        assert!(host_url("https://user:secret@example.test", "/host/v1/access/me").is_err());
        assert!(host_url("https://example.test?token=secret", "/host/v1/access/me").is_err());
        assert!(host_url("https://example.test/api", "/host/v1/access/me").is_err());
        assert!(host_url("https://example.test:0", "/host/v1/access/me").is_err());
        assert!(host_url("https://example.test", "//attacker.test/path").is_err());
        assert_eq!(
            normalize_host_endpoint("https://example.test:443/").unwrap(),
            "https://example.test"
        );
    }
}
