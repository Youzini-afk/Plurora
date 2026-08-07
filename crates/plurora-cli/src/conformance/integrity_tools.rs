//! Conformance tests for `plurora/integrity-lab`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use plurora_runtime::CapabilityInvocationRequest;
use sequoia_openpgp as openpgp;
use serde_json::json;
use tempfile::TempDir;

use openpgp::armor;
use openpgp::cert::prelude::*;
use openpgp::policy::StandardPolicy;
use openpgp::serialize::stream::{Armorer, Message, Signer};
use openpgp::serialize::SerializeInto;

use super::fixtures::*;
use crate::commands::manifest;

const PACKAGE_ID: &str = "plurora/integrity-lab";

async fn load_integrity_lab(
) -> anyhow::Result<plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>> {
    let (_store, runtime) = runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "packages/plurora/integrity-lab/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    Ok(runtime)
}

async fn invoke(
    runtime: &plurora_runtime::Runtime<plurora_runtime::InMemoryEventStore>,
    cap: &str,
    input: serde_json::Value,
) -> anyhow::Result<plurora_runtime::CapabilityInvocationResult> {
    runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(cap.to_string()),
            caller_package_id: None,
            provider_package_id: Some(PACKAGE_ID.to_string()),
            version: None,
            session_id: None,
            input,
        })
        .await
        .map_err(Into::into)
}

pub(crate) async fn tree_hash_deterministic() -> anyhow::Result<()> {
    let rt = load_integrity_lab().await?;
    let tmp = TempDir::new()?;
    write_file(&tmp.path().join("b.txt"), b"bravo")?;
    write_file(&tmp.path().join("nested/a.txt"), b"alpha")?;

    let first = invoke(
        &rt,
        "plurora/integrity-lab/compute_tree_hash",
        json!({ "dir": tmp.path().to_string_lossy() }),
    )
    .await?;
    let second = invoke(
        &rt,
        "plurora/integrity-lab/compute_tree_hash",
        json!({ "dir": tmp.path().to_string_lossy() }),
    )
    .await?;

    anyhow::ensure!(first.output["sha256"] == second.output["sha256"]);
    anyhow::ensure!(first.output["sha256"]
        .as_str()
        .unwrap_or("")
        .starts_with("sha256:"));
    anyhow::ensure!(first.output["files_hashed"] == json!(2));
    anyhow::ensure!(first.output["total_bytes"] == json!(10));
    Ok(())
}

pub(crate) async fn tree_hash_excludes_metadata() -> anyhow::Result<()> {
    let rt = load_integrity_lab().await?;
    let tmp = TempDir::new()?;
    write_file(&tmp.path().join("manifest.yaml"), b"id: pkg/example\n")?;

    let before = invoke(
        &rt,
        "plurora/integrity-lab/compute_tree_hash",
        json!({ "dir": tmp.path().to_string_lossy() }),
    )
    .await?;
    write_file(&tmp.path().join(".git/HEAD"), b"ref: refs/heads/main\n")?;
    write_file(&tmp.path().join(".gitignore"), b"target\n")?;
    write_file(&tmp.path().join("target/build.log"), b"ignored\n")?;
    let after = invoke(
        &rt,
        "plurora/integrity-lab/compute_tree_hash",
        json!({ "dir": tmp.path().to_string_lossy() }),
    )
    .await?;

    anyhow::ensure!(before.output["sha256"] == after.output["sha256"]);
    anyhow::ensure!(after.output["files_hashed"] == json!(1));
    Ok(())
}

pub(crate) async fn manifest_hash_yaml_json_equivalent() -> anyhow::Result<()> {
    let rt = load_integrity_lab().await?;
    let tmp = TempDir::new()?;
    let yaml = tmp.path().join("manifest.yaml");
    let json_path = tmp.path().join("manifest.json");
    write_file(
        &yaml,
        b"id: pkg/example\nversion: 0.1.0\nitems:\n  - a\n  - b\n",
    )?;
    write_file(
        &json_path,
        br#"{"version":"0.1.0","items":["a","b"],"id":"pkg/example"}"#,
    )?;

    let yaml_hash = invoke(
        &rt,
        "plurora/integrity-lab/compute_manifest_hash",
        json!({ "manifest_path": yaml.to_string_lossy() }),
    )
    .await?;
    let json_hash = invoke(
        &rt,
        "plurora/integrity-lab/compute_manifest_hash",
        json!({ "manifest_path": json_path.to_string_lossy() }),
    )
    .await?;

    anyhow::ensure!(yaml_hash.output["sha256"] == json_hash.output["sha256"]);
    anyhow::ensure!(yaml_hash.output["sha256"]
        .as_str()
        .unwrap_or("")
        .starts_with("sha256:"));
    Ok(())
}

pub(crate) async fn gpg_verify_valid_signature() -> anyhow::Result<()> {
    let rt = load_integrity_lab().await?;
    let fixture = gpg_fixture("Alice Fixture <alice.integrity@example.test>")?;
    let result = invoke(
        &rt,
        "plurora/integrity-lab/verify_gpg_signature",
        fixture.input_with_key(&fixture.public_key),
    )
    .await?;

    anyhow::ensure!(result.output["verified"] == json!(true));
    anyhow::ensure!(result.output["key_fingerprint"] == json!(fixture.signing_fingerprint));
    anyhow::ensure!(result.output["signing_time"].is_string());
    anyhow::ensure!(result.output["error"].is_null());
    Ok(())
}

pub(crate) async fn gpg_verify_wrong_key_fails() -> anyhow::Result<()> {
    let rt = load_integrity_lab().await?;
    let signed_by_a = gpg_fixture("Alice Fixture <alice.integrity@example.test>")?;
    let key_b = gpg_fixture("Bob Fixture <bob.integrity@example.test>")?;

    let result = invoke(
        &rt,
        "plurora/integrity-lab/verify_gpg_signature",
        signed_by_a.input_with_key(&key_b.public_key),
    )
    .await?;

    anyhow::ensure!(result.output["verified"] == json!(false));
    anyhow::ensure!(result.output["error"].is_string());
    Ok(())
}

pub(crate) async fn gpg_verify_invalid_signature_no_panic() -> anyhow::Result<()> {
    let rt = load_integrity_lab().await?;
    let fixture = gpg_fixture("Alice Fixture <alice.integrity@example.test>")?;
    let result = invoke(
        &rt,
        "plurora/integrity-lab/verify_gpg_signature",
        json!({
            "data": BASE64.encode(&fixture.data),
            "signature": "-----BEGIN PGP SIGNATURE-----\ncorrupt\n-----END PGP SIGNATURE-----\n",
            "public_keys": [fixture.public_key],
        }),
    )
    .await?;

    anyhow::ensure!(result.output["verified"] == json!(false));
    anyhow::ensure!(result.output["error"]
        .as_str()
        .unwrap_or("")
        .contains("invalid signature format"));
    Ok(())
}

pub(crate) async fn fingerprint_extraction_consistent() -> anyhow::Result<()> {
    let rt = load_integrity_lab().await?;
    let fixture = gpg_fixture("Alice Fixture <alice.integrity@example.test>")?;
    let result = invoke(
        &rt,
        "plurora/integrity-lab/fingerprint_public_key",
        json!({ "public_key": fixture.public_key }),
    )
    .await?;

    anyhow::ensure!(result.output["fingerprint"] == json!(fixture.fingerprint));
    let user_ids = result.output["user_ids"]
        .as_array()
        .context("missing user_ids")?;
    anyhow::ensure!(user_ids
        .iter()
        .any(|id| id == "Alice Fixture <alice.integrity@example.test>"));
    Ok(())
}

fn write_file(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

struct GpgFixture {
    data: Vec<u8>,
    signature: String,
    public_key: String,
    fingerprint: String,
    signing_fingerprint: String,
}

impl GpgFixture {
    fn input_with_key(&self, public_key: &str) -> serde_json::Value {
        json!({
            "data": BASE64.encode(&self.data),
            "signature": self.signature,
            "public_keys": [public_key],
        })
    }
}

fn gpg_fixture(user_id: &str) -> anyhow::Result<GpgFixture> {
    let data = b"integrity-lab conformance signed payload".to_vec();
    let (cert, _) = CertBuilder::general_purpose(None, Some(user_id)).generate()?;
    let policy = StandardPolicy::new();
    let signing_keypair = cert
        .keys()
        .secret()
        .with_policy(&policy, None)
        .supported()
        .alive()
        .revoked(false)
        .for_signing()
        .next()
        .context("missing signing key")?
        .key()
        .clone()
        .into_keypair()?;

    let mut signature = Vec::new();
    let signing_fingerprint = signing_keypair.public().fingerprint().to_string();
    {
        let message = Message::new(&mut signature);
        let message = Armorer::new(message).kind(armor::Kind::Signature).build()?;
        let mut signer = Signer::new(message, signing_keypair).detached().build()?;
        signer.write_all(&data)?;
        signer.finalize()?;
    }

    let public_key = String::from_utf8(cert.armored().to_vec()?)?;
    let fingerprint = cert.fingerprint().to_string();
    Ok(GpgFixture {
        data,
        signature: String::from_utf8(signature)?,
        public_key,
        fingerprint,
        signing_fingerprint,
    })
}
