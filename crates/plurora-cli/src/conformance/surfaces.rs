use std::path::PathBuf;

use plurora_runtime::ProtocolContext;
use serde_json::json;

use crate::commands::manifest;

pub(crate) async fn contribution_list() -> anyhow::Result<()> {
    let (_store, runtime) = super::fixtures::runtime();
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/echo-rust-inproc/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    runtime
        .load_package(
            manifest::read_manifest(PathBuf::from(
                "examples/packages/thirdparty-surface-fixture/manifest.yaml",
            ))
            .await?,
        )
        .await?;
    let all = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.list",
            json!({}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        all.as_array().map(|items| items.len()).unwrap_or(0) >= 7,
        "surface contributions were not listed"
    );
    let entries = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.list",
            json!({"slot": "experience_entry"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        entries[0]["package_id"] == json!("thirdparty/surface-fixture"),
        "third-party entry surface missing"
    );
    anyhow::ensure!(
        entries[0]["surface"]["activation"]["launch_capability_id"]
            == json!("thirdparty/surface-fixture/start"),
        "entry launch capability missing"
    );
    anyhow::ensure!(
        entries[0]["surface"]["allowed_capability_ids"]
            == json!(["thirdparty/surface-fixture/inspect"]),
        "typed surface allowed capability ids missing"
    );
    let described = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.describe",
            json!({"surface_id": "thirdparty/surface-fixture/assist"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        described["surface"]["approval_policy"] == json!("fork_then_approve"),
        "assistant action policy missing"
    );
    let quick_actions = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.list",
            json!({"slot": "quick_action"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        quick_actions
            .as_array()
            .map(|items| items.len())
            .unwrap_or(0)
            == 1,
        "quick_action slot filter should return exactly one fixture contribution"
    );
    anyhow::ensure!(
        quick_actions[0]["package_id"] == json!("thirdparty/surface-fixture"),
        "quick_action slot filter returned wrong package"
    );
    anyhow::ensure!(
        quick_actions[0]["surface"]["slot"] == json!("quick_action"),
        "quick_action slot filter returned wrong slot"
    );
    let workshop_cards = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "shell.contribution.list",
            json!({"slot": "workshop_card"}),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    anyhow::ensure!(
        workshop_cards
            .as_array()
            .map(|items| items.len())
            .unwrap_or(0)
            == 1,
        "workshop_card slot filter should return exactly one fixture contribution"
    );
    anyhow::ensure!(
        workshop_cards[0]["surface"]["metadata"]["category"] == json!("tool"),
        "workshop_card metadata category missing"
    );
    Ok(())
}

pub(crate) async fn shell_descriptor_metadata_validation() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let invalid_path = temp.path().join("invalid.yaml");
    std::fs::write(
        &invalid_path,
        r#"schema_version: 1
id: thirdparty/invalid-shell
version: 0.1.0
entry:
  kind: rust_inproc
  crate_ref: example-echo-rust-inproc
  symbol: register
  abi_version: 1
provides:
  - id: thirdparty/invalid-shell/run
    version: 0.1.0
    input_schema: {}
    output_schema: {}
contributes:
  surfaces:
    - id: thirdparty/invalid-shell/quick
      version: 0.1.0
      slot: quick_action
      title: Invalid shell descriptor
      metadata:
        shell_schema_version: 1
        title: "not localized"
"#,
    )?;
    let check_err = crate::commands::package::package_check(invalid_path.clone())
        .await
        .expect_err("package check should reject invalid shell descriptor metadata");
    anyhow::ensure!(
        check_err.to_string().contains("title must be an object"),
        "invalid metadata package check failed for the wrong reason: {check_err}"
    );
    let manifest_err = crate::commands::manifest::validate_manifest(invalid_path)
        .await
        .expect_err("manifest validation should reject invalid shell descriptor metadata");
    anyhow::ensure!(
        manifest_err.to_string().contains("title must be an object"),
        "invalid metadata manifest validation failed for the wrong reason: {manifest_err}"
    );

    let cross_package_path = temp.path().join("cross-package.yaml");
    std::fs::write(
        &cross_package_path,
        r#"schema_version: 1
id: thirdparty/shell-owner
version: 0.1.0
entry:
  kind: rust_inproc
  crate_ref: example-echo-rust-inproc
  symbol: register
  abi_version: 1
provides:
  - id: thirdparty/shell-owner/run
    version: 0.1.0
    input_schema: {}
    output_schema: {}
contributes:
  surfaces:
    - id: thirdparty/shell-owner/quick
      version: 0.1.0
      slot: quick_action
      title: Cross package quick action
      capability_id: other/package/run
      metadata:
        shell_schema_version: 1
        title:
          en: Cross package
"#,
    )?;
    let ownership_err = crate::commands::manifest::validate_manifest(cross_package_path)
        .await
        .expect_err("manifest validation should reject cross-package capability_id");
    anyhow::ensure!(
        ownership_err
            .to_string()
            .contains("capability_id must belong to the package id"),
        "cross-package capability_id failed for the wrong reason: {ownership_err}"
    );

    let same_package_path = temp.path().join("same-package.yaml");
    std::fs::write(
        &same_package_path,
        r#"schema_version: 1
id: thirdparty/shell-owner
version: 0.1.0
entry:
  kind: rust_inproc
  crate_ref: example-echo-rust-inproc
  symbol: register
  abi_version: 1
provides:
  - id: thirdparty/shell-owner/run
    version: 0.1.0
    input_schema: {}
    output_schema: {}
contributes:
  surfaces:
    - id: thirdparty/shell-owner/quick
      version: 0.1.0
      slot: quick_action
      title: Same package quick action
      capability_id: thirdparty/shell-owner/run
      metadata:
        shell_schema_version: 1
        title:
          en: Same package
"#,
    )?;
    crate::commands::manifest::read_manifest(same_package_path)
        .await?
        .validate_basic()?;

    let cross_surface_path = temp.path().join("cross-surface.yaml");
    std::fs::write(
        &cross_surface_path,
        r#"schema_version: 1
id: thirdparty/shell-owner
version: 0.1.0
entry:
  kind: rust_inproc
  crate_ref: example-echo-rust-inproc
  symbol: register
  abi_version: 1
provides:
  - id: thirdparty/shell-owner/run
    version: 0.1.0
    input_schema: {}
    output_schema: {}
contributes:
  surfaces:
    - id: thirdparty/shell-owner/quick
      version: 0.1.0
      slot: quick_action
      title: Cross surface quick action
      metadata:
        shell_schema_version: 1
        title:
          en: Cross surface
        surface_id: other/package/surface
"#,
    )?;
    let surface_ownership_err = crate::commands::manifest::validate_manifest(cross_surface_path)
        .await
        .expect_err("manifest validation should reject cross-package metadata surface_id");
    anyhow::ensure!(
        surface_ownership_err
            .to_string()
            .contains("surface_id must belong to the package id"),
        "cross-package surface_id failed for the wrong reason: {surface_ownership_err}"
    );

    let long_title_path = temp.path().join("long-title.yaml");
    std::fs::write(
        &long_title_path,
        format!(
            r#"schema_version: 1
id: thirdparty/invalid-shell
version: 0.1.0
entry:
  kind: rust_inproc
  crate_ref: example-echo-rust-inproc
  symbol: register
  abi_version: 1
provides:
  - id: thirdparty/invalid-shell/run
    version: 0.1.0
    input_schema: {{}}
    output_schema: {{}}
contributes:
  surfaces:
    - id: thirdparty/invalid-shell/quick
      version: 0.1.0
      slot: quick_action
      title: Long title quick action
      metadata:
        shell_schema_version: 1
        title:
          en: {}
"#,
            "x".repeat(81)
        ),
    )?;
    let long_title_err = crate::commands::manifest::validate_manifest(long_title_path)
        .await
        .expect_err("manifest validation should reject overly long shell title");
    anyhow::ensure!(
        long_title_err
            .to_string()
            .contains("title.en must be non-empty text up to 80 characters"),
        "long shell title failed for the wrong reason: {long_title_err}"
    );

    let bad_icon_path = temp.path().join("bad-icon.yaml");
    std::fs::write(
        &bad_icon_path,
        r#"schema_version: 1
id: thirdparty/invalid-shell
version: 0.1.0
entry:
  kind: rust_inproc
  crate_ref: example-echo-rust-inproc
  symbol: register
  abi_version: 1
provides:
  - id: thirdparty/invalid-shell/run
    version: 0.1.0
    input_schema: {}
    output_schema: {}
contributes:
  surfaces:
    - id: thirdparty/invalid-shell/quick
      version: 0.1.0
      slot: quick_action
      title: Bad icon quick action
      metadata:
        shell_schema_version: 1
        title:
          en: Bad icon
        icon_hint: bad/icon
"#,
    )?;
    let bad_icon_err = crate::commands::manifest::validate_manifest(bad_icon_path)
        .await
        .expect_err("manifest validation should reject invalid shell icon hint");
    anyhow::ensure!(
        bad_icon_err.to_string().contains("icon_hint must match"),
        "bad shell icon hint failed for the wrong reason: {bad_icon_err}"
    );
    Ok(())
}
