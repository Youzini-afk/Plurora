use plurora_cli::commands::install::{InstallArgs, OutputFormat};
use plurora_core::project::{
    ExternalSourceKind, ExternalWorkspaceOwnership, ProjectDescriptor, ProjectType,
};
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread")]
async fn install_local_fixture() {
    let tmp = tempdir().unwrap();
    let data_dir = tmp.path().join("plurora");
    plurora_cli::commands::install::run(InstallArgs {
        source: fixture_path(),
        profile: "test".to_string(),
        data_dir: Some(data_dir.clone()),
        require_signed: false,
        strict: false,
        yes: true,
        format: OutputFormat::Human,
        wrap_as_adapter: false,
        workspace_only: false,
        link_local: false,
    })
    .await
    .unwrap();

    assert!(data_dir.join("profiles/test.lock.toml").exists());
    let lockfile = std::fs::read_to_string(data_dir.join("profiles/test.lock.toml")).unwrap();
    assert!(lockfile.contains("fixture/pkg-local"));
    assert!(data_dir.join("store").is_dir());
}

#[tokio::test(flavor = "multi_thread")]
async fn list_lockfile_and_uninstall_local_fixture() {
    let tmp = tempdir().unwrap();
    let data_dir = tmp.path().join("plurora");
    plurora_cli::commands::install::run(InstallArgs {
        source: fixture_path(),
        profile: "test".to_string(),
        data_dir: Some(data_dir.clone()),
        require_signed: false,
        strict: false,
        yes: true,
        format: OutputFormat::Human,
        wrap_as_adapter: false,
        workspace_only: false,
        link_local: false,
    })
    .await
    .unwrap();

    plurora_cli::commands::list_installed::run(
        plurora_cli::commands::list_installed::ListInstalledArgs {
            profile: "test".to_string(),
            data_dir: Some(data_dir.clone()),
            format: OutputFormat::Json,
        },
    )
    .await
    .unwrap();

    plurora_cli::commands::lockfile::run(plurora_cli::commands::lockfile::LockfileArgs {
        profile: "test".to_string(),
        data_dir: Some(data_dir.clone()),
        check: true,
    })
    .await
    .unwrap();

    plurora_cli::commands::uninstall::run(plurora_cli::commands::uninstall::UninstallArgs {
        package_id: "fixture/pkg-local".to_string(),
        profile: "test".to_string(),
        data_dir: Some(data_dir.clone()),
        keep_data: false,
        delete_data: false,
    })
    .await
    .unwrap();

    let lockfile = std::fs::read_to_string(data_dir.join("profiles/test.lock.toml")).unwrap();
    assert!(!lockfile.contains("fixture/pkg-local"));
}

fn fixture_path() -> String {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/conformance/fixtures/install/pkg-local")
        .to_string_lossy()
        .to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn install_bare_external_project_into_isolated_managed_workspace() {
    let tmp = tempdir().unwrap();
    let source = tmp.path().join("source-project");
    let data_dir = tmp.path().join("plurora");
    std::fs::create_dir_all(source.join("src")).unwrap();
    std::fs::write(source.join("README.md"), "external fixture\n").unwrap();
    std::fs::write(source.join("src/main.txt"), "hello\n").unwrap();

    let install = || InstallArgs {
        source: source.to_string_lossy().to_string(),
        profile: "test".to_string(),
        data_dir: Some(data_dir.clone()),
        require_signed: false,
        strict: false,
        yes: true,
        format: OutputFormat::Human,
        wrap_as_adapter: false,
        workspace_only: true,
        link_local: false,
    };
    plurora_cli::commands::install::run(install())
        .await
        .unwrap();

    let descriptor_path = only_project_descriptor(&data_dir);
    let first_yaml = std::fs::read_to_string(&descriptor_path).unwrap();
    let descriptor: ProjectDescriptor = serde_yaml::from_str(&first_yaml).unwrap();
    assert_eq!(
        descriptor.project.project_type,
        ProjectType::ExternalWorkspace
    );
    let external = descriptor.project.external.as_ref().unwrap();
    assert_eq!(external.source_kind, Some(ExternalSourceKind::Local));
    assert_eq!(
        external.workspace_ownership,
        Some(ExternalWorkspaceOwnership::Managed)
    );
    let workspace = std::path::PathBuf::from(external.workspace_root.as_ref().unwrap());
    let canonical_data_dir = std::fs::canonicalize(&data_dir).unwrap();
    assert!(workspace.starts_with(canonical_data_dir.join("workspaces/external")));
    assert_ne!(workspace, source);
    assert_eq!(
        std::fs::read_to_string(workspace.join("src/main.txt")).unwrap(),
        "hello\n"
    );

    plurora_cli::commands::install::run(install())
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&descriptor_path).unwrap(),
        first_yaml
    );
    assert_eq!(active_project_count(&data_dir), 1);

    plurora_cli::commands::uninstall::run(plurora_cli::commands::uninstall::UninstallArgs {
        package_id: descriptor.project.id.as_str().to_string(),
        profile: "test".to_string(),
        data_dir: Some(data_dir.clone()),
        keep_data: true,
        delete_data: false,
    })
    .await
    .unwrap();
    assert!(source.join("src/main.txt").is_file());
    assert!(data_dir
        .join("projects/.archived")
        .join(descriptor.project.id.as_str())
        .join("project.yaml")
        .is_file());
    assert!(data_dir
        .join("workspaces/.archived/external")
        .join(descriptor.project.id.as_str())
        .is_dir());
}

#[tokio::test(flavor = "multi_thread")]
async fn linked_local_external_project_never_deletes_the_source() {
    let tmp = tempdir().unwrap();
    let source = tmp.path().join("linked-project");
    let data_dir = tmp.path().join("plurora");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("keep.txt"), "owned by user\n").unwrap();

    plurora_cli::commands::install::run(InstallArgs {
        source: source.to_string_lossy().to_string(),
        profile: "test".to_string(),
        data_dir: Some(data_dir.clone()),
        require_signed: false,
        strict: false,
        yes: true,
        format: OutputFormat::Human,
        wrap_as_adapter: false,
        workspace_only: true,
        link_local: true,
    })
    .await
    .unwrap();

    let descriptor: ProjectDescriptor =
        serde_yaml::from_str(&std::fs::read_to_string(only_project_descriptor(&data_dir)).unwrap())
            .unwrap();
    assert_eq!(
        descriptor
            .project
            .external
            .as_ref()
            .unwrap()
            .workspace_ownership,
        Some(ExternalWorkspaceOwnership::LinkedLocal)
    );

    plurora_cli::commands::uninstall::run(plurora_cli::commands::uninstall::UninstallArgs {
        package_id: descriptor.project.id.as_str().to_string(),
        profile: "test".to_string(),
        data_dir: Some(data_dir),
        keep_data: false,
        delete_data: true,
    })
    .await
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(source.join("keep.txt")).unwrap(),
        "owned by user\n"
    );
}

fn only_project_descriptor(data_dir: &std::path::Path) -> std::path::PathBuf {
    std::fs::read_dir(data_dir.join("projects"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("project.yaml"))
        .find(|path| path.is_file())
        .expect("one active project descriptor")
}

fn active_project_count(data_dir: &std::path::Path) -> usize {
    std::fs::read_dir(data_dir.join("projects"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("project.yaml").is_file())
        .count()
}
