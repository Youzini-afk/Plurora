//! Handler for `official/secret-store-lab` capabilities.

use anyhow::Result;
use plurora_core::project::ProjectId;
use serde::Deserialize;
use serde_json::Value;

use crate::secret_store::{
    current_key_source, load_store, resolve_master_key, save_store, validate_secret_name,
    validate_secret_value,
};

use super::InprocInvocation;

const PACKAGE_ID: &str = "official/secret-store-lab";

#[derive(Debug, Deserialize)]
struct NameInput {
    name: String,
}

#[derive(Debug, Deserialize)]
struct PutSecretInput {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct ProjectNameInput {
    project_id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct PutProjectSecretInput {
    project_id: String,
    name: String,
    value: String,
}

pub fn try_handle(request: &InprocInvocation) -> Option<Result<Value>> {
    if request.provider_package_id != PACKAGE_ID {
        return None;
    }

    match request.capability_id.as_str() {
        "secret-store.put_secret" | "official/secret-store-lab/put_secret" => {
            Some(put_secret(request.input.clone()))
        }
        "secret-store.has_secret" | "official/secret-store-lab/has_secret" => {
            Some(has_secret(request.input.clone()))
        }
        "secret-store.list_secrets" | "official/secret-store-lab/list_secrets" => {
            Some(list_secrets())
        }
        "secret-store.delete_secret" | "official/secret-store-lab/delete_secret" => {
            Some(delete_secret(request.input.clone()))
        }
        "secret-store.put_project_secret" | "official/secret-store-lab/put_project_secret" => {
            Some(put_project_secret(request.input.clone()))
        }
        "secret-store.has_project_secret" | "official/secret-store-lab/has_project_secret" => {
            Some(has_project_secret(request.input.clone()))
        }
        "secret-store.list_project_secrets" | "official/secret-store-lab/list_project_secrets" => {
            Some(list_project_secrets(request.input.clone()))
        }
        "secret-store.delete_project_secret"
        | "official/secret-store-lab/delete_project_secret" => {
            Some(delete_project_secret(request.input.clone()))
        }
        "secret-store.health" | "official/secret-store-lab/health" => Some(health()),
        _ => None,
    }
}

fn store_path() -> Result<std::path::PathBuf> {
    plurora_core::paths::secret_store_path()
}

fn put_secret(input: Value) -> Result<Value> {
    let input: PutSecretInput = serde_json::from_value(input)?;
    validate_secret_name(&input.name)?;
    validate_secret_value(&input.value)?;

    let (identity, _) = resolve_master_key()?;
    let recipient = identity.to_public();
    let path = store_path()?;
    let mut store = load_store(&path, &identity)?;
    let created = !store.secrets.contains_key(&input.name);
    store.secrets.insert(input.name.clone(), input.value);
    save_store(&path, &store, &recipient)?;

    Ok(serde_json::json!({
        "name": input.name,
        "stored": true,
        "created": created,
    }))
}

fn has_secret(input: Value) -> Result<Value> {
    let input: NameInput = serde_json::from_value(input)?;
    validate_secret_name(&input.name)?;

    let (identity, _) = resolve_master_key()?;
    let path = store_path()?;
    let store = load_store(&path, &identity)?;
    Ok(serde_json::json!({
        "name": input.name,
        "exists": store.secrets.contains_key(&input.name),
    }))
}

fn list_secrets() -> Result<Value> {
    let (identity, _) = resolve_master_key()?;
    let path = store_path()?;
    let store = load_store(&path, &identity)?;
    let names: Vec<String> = store.secrets.keys().cloned().collect();
    Ok(serde_json::json!({ "names": names }))
}

fn delete_secret(input: Value) -> Result<Value> {
    let input: NameInput = serde_json::from_value(input)?;
    validate_secret_name(&input.name)?;

    let (identity, _) = resolve_master_key()?;
    let recipient = identity.to_public();
    let path = store_path()?;
    let mut store = load_store(&path, &identity)?;
    let removed = store.secrets.remove(&input.name).is_some();
    save_store(&path, &store, &recipient)?;

    Ok(serde_json::json!({
        "name": input.name,
        "removed": removed,
    }))
}

fn project_store_path(project_id: &ProjectId) -> Result<std::path::PathBuf> {
    plurora_core::paths::project_secret_store_path(project_id)
}

fn put_project_secret(input: Value) -> Result<Value> {
    let input: PutProjectSecretInput = serde_json::from_value(input)?;
    let project_id = ProjectId::new(input.project_id)?;
    validate_secret_name(&input.name)?;
    validate_secret_value(&input.value)?;

    plurora_core::paths::ensure_project_initialized(&project_id)?;
    let (identity, _) = resolve_master_key()?;
    let recipient = identity.to_public();
    let path = project_store_path(&project_id)?;
    let mut store = load_store(&path, &identity)?;
    let created = !store.secrets.contains_key(&input.name);
    store.secrets.insert(input.name.clone(), input.value);
    save_store(&path, &store, &recipient)?;

    Ok(serde_json::json!({
        "project_id": project_id.as_str(),
        "name": input.name,
        "stored": true,
        "created": created,
    }))
}

fn has_project_secret(input: Value) -> Result<Value> {
    let input: ProjectNameInput = serde_json::from_value(input)?;
    let project_id = ProjectId::new(input.project_id)?;
    validate_secret_name(&input.name)?;

    let (identity, _) = resolve_master_key()?;
    let path = project_store_path(&project_id)?;
    let store = load_store(&path, &identity)?;
    Ok(serde_json::json!({
        "project_id": project_id.as_str(),
        "name": input.name,
        "exists": store.secrets.contains_key(&input.name),
    }))
}

fn list_project_secrets(input: Value) -> Result<Value> {
    #[derive(Debug, Deserialize)]
    struct ListProjectSecretsInput {
        project_id: String,
    }

    let input: ListProjectSecretsInput = serde_json::from_value(input)?;
    let project_id = ProjectId::new(input.project_id)?;
    let (identity, _) = resolve_master_key()?;
    let path = project_store_path(&project_id)?;
    let store = load_store(&path, &identity)?;
    let names: Vec<String> = store.secrets.keys().cloned().collect();
    Ok(serde_json::json!({
        "project_id": project_id.as_str(),
        "names": names,
    }))
}

fn delete_project_secret(input: Value) -> Result<Value> {
    let input: ProjectNameInput = serde_json::from_value(input)?;
    let project_id = ProjectId::new(input.project_id)?;
    validate_secret_name(&input.name)?;

    plurora_core::paths::ensure_project_initialized(&project_id)?;
    let (identity, _) = resolve_master_key()?;
    let recipient = identity.to_public();
    let path = project_store_path(&project_id)?;
    let mut store = load_store(&path, &identity)?;
    let removed = store.secrets.remove(&input.name).is_some();
    save_store(&path, &store, &recipient)?;

    Ok(serde_json::json!({
        "project_id": project_id.as_str(),
        "name": input.name,
        "removed": removed,
    }))
}

fn health() -> Result<Value> {
    let path = store_path()?;
    let exists = path.exists();
    let key_source = current_key_source()?;
    let secret_count = if exists {
        let (identity, _) = resolve_master_key()?;
        load_store(&path, &identity)?.secrets.len()
    } else {
        0
    };

    Ok(serde_json::json!({
        "store_path": path.display().to_string(),
        "exists": exists,
        "secret_count": secret_count,
        "key_source": key_source.as_str(),
    }))
}
