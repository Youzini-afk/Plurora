use std::fs;
use std::process::Command;

fn plurora() -> String {
    env!("CARGO_BIN_EXE_plurora").to_string()
}

fn repo_root() -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.to_str().expect("utf8 repo root").to_string()
}

#[test]
fn echo_rust_inproc_passes_package_conformance() {
    let output = Command::new(plurora())
        .args([
            "conformance",
            "package",
            "--path",
            "examples/packages/echo-rust-inproc",
            "--format",
            "json",
        ])
        .current_dir(repo_root())
        .output()
        .expect("run plurora conformance package");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json report");
    assert_eq!(report["summary"]["failed"], 0);
    assert_eq!(report["checks"][0]["status"], "Pass");
    assert!(report["package_envelope"]["artifact"]["digest"]
        .as_str()
        .unwrap_or_default()
        .starts_with("sha256:"));
    assert_eq!(report["components"][0]["claim_status"], "legacy_adapted");
    assert_eq!(report["components"][0]["trust_class"], "trusted_native");
    assert_eq!(report["components"][0]["composability_guaranteed"], true);
    assert_eq!(report["components"][0]["portability_guaranteed"], false);
}

#[test]
fn path_b_self_contained_passes_package_conformance() {
    let output = Command::new(plurora())
        .args([
            "conformance",
            "package",
            "--path",
            "examples/packages/path-b-self-contained",
            "--format",
            "json",
        ])
        .current_dir(repo_root())
        .output()
        .expect("run plurora conformance package");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json report");
    assert_eq!(report["package_id"], "examples/path-b-app");
    assert_eq!(report["summary"]["failed"], 0);
    assert_eq!(report["components"][0]["claim_status"], "foreign_capsule");
    assert_eq!(report["components"][0]["trust_class"], "foreign_capsule");
    assert_eq!(report["components"][0]["composability_guaranteed"], false);
    assert_eq!(report["components"][0]["portability_guaranteed"], false);
    assert!(report["components"][0]["guarantee"]
        .as_str()
        .unwrap_or_default()
        .contains("not guaranteed"));
}

#[test]
fn broken_manifest_fails_schema_check_without_crashing() {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::write(
        dir.path().join("manifest.yaml"),
        "schema_version: 1\nid: broken/package\nversion: 0.1.0\n",
    )
    .expect("write manifest");
    let output = Command::new(plurora())
        .args([
            "conformance",
            "package",
            "--path",
            dir.path().to_str().expect("utf8 temp path"),
            "--format",
            "json",
        ])
        .current_dir(repo_root())
        .output()
        .expect("run plurora conformance package");
    assert!(!output.status.success(), "broken manifest should fail");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json report");
    assert_eq!(report["checks"][0]["id"], "manifest.schema_valid");
    assert_eq!(report["checks"][0]["status"], "Fail");
}
