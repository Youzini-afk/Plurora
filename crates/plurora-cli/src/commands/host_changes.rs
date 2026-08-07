use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use anyhow::Context;
use plurora_core::ProjectId;
use plurora_service::DevelopmentDraftRequest;
use reqwest::Method;
use serde_json::{json, Value};

use super::host_access::{print_json, request};

const MAX_DRAFT_REQUEST_BYTES: u64 = 20 * 1024 * 1024;

pub async fn list(endpoint: &str, access_token: &str, project_id: &str) -> anyhow::Result<()> {
    let path = changes_path(project_id)?;
    print_json(request(endpoint, access_token, Method::GET, &path, None).await?)
}

pub async fn get(
    endpoint: &str,
    access_token: &str,
    project_id: &str,
    change_set_id: &str,
) -> anyhow::Result<()> {
    let path = change_path(project_id, change_set_id)?;
    print_json(request(endpoint, access_token, Method::GET, &path, None).await?)
}

pub async fn draft(
    endpoint: &str,
    access_token: &str,
    project_id: &str,
    request_path: &Path,
) -> anyhow::Result<()> {
    let body = read_draft_request(request_path)?;
    let path = changes_path(project_id)?;
    print_json(request(endpoint, access_token, Method::POST, &path, Some(body)).await?)
}

pub async fn decide(
    endpoint: &str,
    access_token: &str,
    project_id: &str,
    change_set_id: &str,
    approved: bool,
    reason: Option<String>,
) -> anyhow::Result<()> {
    let path = format!("{}/approve", change_path(project_id, change_set_id)?);
    let mut body = json!({"approved": approved});
    if let Some(reason) = reason {
        body["reason"] = json!(reason);
    }
    print_json(request(endpoint, access_token, Method::POST, &path, Some(body)).await?)
}

pub async fn execute(
    endpoint: &str,
    access_token: &str,
    project_id: &str,
    change_set_id: &str,
) -> anyhow::Result<()> {
    post_empty_action(endpoint, access_token, project_id, change_set_id, "execute").await
}

pub async fn recover(
    endpoint: &str,
    access_token: &str,
    project_id: &str,
    change_set_id: &str,
) -> anyhow::Result<()> {
    post_empty_action(endpoint, access_token, project_id, change_set_id, "recover").await
}

pub async fn bundle(
    endpoint: &str,
    access_token: &str,
    project_id: &str,
    change_set_id: &str,
) -> anyhow::Result<()> {
    let path = format!("{}/bundle", change_path(project_id, change_set_id)?);
    print_json(request(endpoint, access_token, Method::GET, &path, None).await?)
}

async fn post_empty_action(
    endpoint: &str,
    access_token: &str,
    project_id: &str,
    change_set_id: &str,
    action: &str,
) -> anyhow::Result<()> {
    let path = format!("{}/{}", change_path(project_id, change_set_id)?, action);
    print_json(request(endpoint, access_token, Method::POST, &path, Some(json!({}))).await?)
}

fn changes_path(project_id: &str) -> anyhow::Result<String> {
    let project_id = ProjectId::new(project_id.trim())?;
    Ok(format!(
        "/host/v1/projects/{}/changes",
        encode_segment(project_id.as_str())
    ))
}

fn change_path(project_id: &str, change_set_id: &str) -> anyhow::Result<String> {
    let change_set_id = change_set_id.trim();
    anyhow::ensure!(
        !change_set_id.is_empty()
            && change_set_id.len() <= 256
            && change_set_id.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
            })
            && !matches!(change_set_id, "." | ".."),
        "change_set_id is invalid"
    );
    Ok(format!(
        "{}/{}",
        changes_path(project_id)?,
        encode_segment(change_set_id)
    ))
}

fn encode_segment(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn read_draft_request(path: &Path) -> anyhow::Result<Value> {
    let bytes = if path == Path::new("-") {
        read_bounded(io::stdin().lock()).context("failed to read ChangeSet request from stdin")?
    } else {
        read_bounded(
            File::open(path)
                .with_context(|| format!("failed to open ChangeSet request {}", path.display()))?,
        )
        .with_context(|| format!("failed to read ChangeSet request {}", path.display()))?
    };
    parse_draft_request(&bytes)
}

fn read_bounded(reader: impl Read) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_DRAFT_REQUEST_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        u64::try_from(bytes.len()).unwrap_or(u64::MAX) <= MAX_DRAFT_REQUEST_BYTES,
        "ChangeSet request exceeds {MAX_DRAFT_REQUEST_BYTES} bytes"
    );
    Ok(bytes)
}

fn parse_draft_request(bytes: &[u8]) -> anyhow::Result<Value> {
    let request: DevelopmentDraftRequest =
        serde_json::from_slice(bytes).context("ChangeSet request is not valid typed JSON")?;
    serde_json::to_value(request).context("failed to encode ChangeSet request")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_paths_and_draft_requests_are_strict() -> anyhow::Result<()> {
        assert_eq!(
            change_path("project-1", "chg-0123456789abcdef")?,
            "/host/v1/projects/project-1/changes/chg-0123456789abcdef"
        );
        assert!(change_path("project-1", "../other").is_err());
        assert!(parse_draft_request(
            br#"{"goal":"update source","operations":[{"op":"file_write","path":"src/app.ts","content":"export {};\n"}],"verification":{"kind":"static_validation"}}"#
        )
        .is_ok());
        assert!(
            parse_draft_request(br#"{"goal":"update source","operations":[],"unknown":true}"#)
                .is_err()
        );
        Ok(())
    }
}
