//! Host-owned encrypted local secret store helpers.
//!
//! The store file is encrypted with age using a single x25519 identity. Public
//! capabilities can write and inspect names, while the host secret resolver reads
//! values directly in-process without exposing a general get capability.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

use age::secrecy::ExposeSecret;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const STORE_SCHEMA: &str = "plurora.secret-store.v1";
pub const MAX_SECRET_NAME_LEN: usize = 128;
pub const MAX_SECRET_VALUE_LEN: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeySource {
    Keyring,
    File,
    None,
}

impl KeySource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keyring => "keyring",
            Self::File => "file",
            Self::None => "none",
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct StoreFile {
    pub schema: String,
    pub secrets: BTreeMap<String, String>,
}

impl StoreFile {
    pub fn empty() -> Self {
        Self {
            schema: STORE_SCHEMA.to_string(),
            secrets: BTreeMap::new(),
        }
    }
}

pub fn validate_secret_name(name: &str) -> Result<()> {
    anyhow::ensure!(!name.is_empty(), "secret name must not be empty");
    anyhow::ensure!(
        name.len() <= MAX_SECRET_NAME_LEN,
        "secret name must be at most {MAX_SECRET_NAME_LEN} bytes"
    );
    anyhow::ensure!(name.is_ascii(), "secret name must be ASCII");
    anyhow::ensure!(
        name.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
        "secret name may only contain ASCII letters, digits, underscore, or dash"
    );
    Ok(())
}

pub fn validate_secret_value(value: &str) -> Result<()> {
    anyhow::ensure!(!value.is_empty(), "secret value must not be empty");
    anyhow::ensure!(
        value.len() <= MAX_SECRET_VALUE_LEN,
        "secret value must be at most {MAX_SECRET_VALUE_LEN} bytes"
    );
    Ok(())
}

pub fn load_store(path: &Path, key: &age::x25519::Identity) -> Result<StoreFile> {
    if !path.exists() {
        return Ok(StoreFile::empty());
    }

    let encrypted = std::fs::read(path)
        .with_context(|| format!("failed to read secret store at {}", path.display()))?;
    let decryptor = age::Decryptor::new(&encrypted[..])?;
    let mut decrypted = Vec::new();
    let mut reader = decryptor.decrypt(std::iter::once(key as &dyn age::Identity))?;
    reader.read_to_end(&mut decrypted)?;

    let store: StoreFile = serde_json::from_slice(&decrypted)?;
    anyhow::ensure!(
        store.schema == STORE_SCHEMA,
        "incompatible secret store schema: {}",
        store.schema
    );
    Ok(store)
}

pub fn save_store(path: &Path, store: &StoreFile, key: &age::x25519::Recipient) -> Result<()> {
    let json = serde_json::to_vec(store)?;
    let recipient = key.clone();
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))?;
    let mut encrypted = Vec::new();
    let mut writer = encryptor.wrap_output(&mut encrypted)?;
    writer.write_all(&json)?;
    writer.finish()?;

    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid store path"))?;
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".secrets.tmp.{}", std::process::id()));
    std::fs::write(&tmp, &encrypted)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&tmp)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&tmp, perms)?;
    }

    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn resolve_master_key() -> Result<(age::x25519::Identity, KeySource)> {
    let key_path = plurora_core::paths::secret_store_key_path()?;
    if key_path.exists() {
        let key_str = std::fs::read_to_string(&key_path)?;
        let identity = key_str
            .trim()
            .parse::<age::x25519::Identity>()
            .map_err(|e| anyhow::anyhow!("invalid key file: {e}"))?;
        return Ok((identity, KeySource::File));
    }

    let identity = age::x25519::Identity::generate();
    let key_secret = identity.to_string();
    let key_str = key_secret.expose_secret().to_string();

    plurora_core::paths::ensure_initialized()?;
    if let Some(parent) = key_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&key_path, &key_str)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&key_path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&key_path, perms)?;
    }
    Ok((identity, KeySource::File))
}

pub fn current_key_source() -> Result<KeySource> {
    let key_path = plurora_core::paths::secret_store_key_path()?;
    if key_path.exists() {
        return Ok(KeySource::File);
    }
    Ok(KeySource::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_secret_names() {
        assert!(validate_secret_name("OPENAI_API_KEY").is_ok());
        assert!(validate_secret_name("provider-key-1").is_ok());
        assert!(validate_secret_name("").is_err());
        assert!(validate_secret_name("has space").is_err());
        assert!(validate_secret_name("unicode-λ").is_err());
        assert!(validate_secret_name(&"A".repeat(129)).is_err());
    }

    #[test]
    fn validates_secret_values() {
        assert!(validate_secret_value("synthetic-test-value-12345").is_ok());
        assert!(validate_secret_value("").is_err());
        assert!(validate_secret_value(&"x".repeat(MAX_SECRET_VALUE_LEN + 1)).is_err());
    }

    #[test]
    fn store_encrypts_and_decrypts_roundtrip() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let path = tmp.path().join("secrets.dat");
        let identity = age::x25519::Identity::generate();
        let recipient = identity.to_public();
        let mut store = StoreFile::empty();
        store.secrets.insert(
            "ROUNDTRIP_KEY".to_string(),
            "synthetic-test-value-12345".to_string(),
        );

        save_store(&path, &store, &recipient)?;
        let encrypted = std::fs::read(&path)?;
        assert!(!String::from_utf8_lossy(&encrypted).contains("synthetic-test-value-12345"));
        let loaded = load_store(&path, &identity)?;
        assert_eq!(
            loaded.secrets.get("ROUNDTRIP_KEY").map(String::as_str),
            Some("synthetic-test-value-12345")
        );
        Ok(())
    }
}
