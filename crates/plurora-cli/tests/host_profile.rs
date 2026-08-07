use plurora_cli::cli::HostProfile;
use plurora_cli::commands::host::runtime_config_from_profile;
use plurora_runtime::{ExecStatusKind, LocalExecExecutorConfig, LocalExecStartRequest};
use tempfile::TempDir;
use tokio::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

struct DataDirGuard {
    previous: Option<String>,
    _tmp: TempDir,
}

impl DataDirGuard {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("create temp data dir");
        let previous = std::env::var("PLURORA_DATA_DIR").ok();
        std::env::set_var("PLURORA_DATA_DIR", tmp.path().display().to_string());
        Self {
            previous,
            _tmp: tmp,
        }
    }
}

impl Drop for DataDirGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("PLURORA_DATA_DIR", value),
            None => std::env::remove_var("PLURORA_DATA_DIR"),
        }
    }
}

struct EnvVarGuard(String);

impl EnvVarGuard {
    fn set(name: &str, value: &str) -> Self {
        std::env::set_var(name, value);
        Self(name.to_string())
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        std::env::remove_var(&self.0);
    }
}

fn profile_from_yaml(yaml: &str) -> HostProfile {
    serde_yaml::from_str(yaml).expect("parse host profile")
}

fn first_existing(paths: &[&str]) -> Option<std::path::PathBuf> {
    paths
        .iter()
        .map(std::path::PathBuf::from)
        .find(|path| path.exists())
}

#[tokio::test]
async fn profile_default_installs_composite_resolver() {
    let _lock = ENV_LOCK.lock().await;
    let _data_dir = DataDirGuard::new();
    let profile = profile_from_yaml("title: test\n");

    let config = runtime_config_from_profile(&profile).expect("build runtime config");
    let err = config
        .secret_resolver
        .resolver
        .resolve("secret_ref:store:NONEXISTENT")
        .await
        .expect_err("missing store secret should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("not in store"),
        "default profile should install store resolver, got: {msg}"
    );
    assert!(
        !msg.contains("no secret resolver configured"),
        "default profile must not leave DenyAll resolver installed: {msg}"
    );
}

#[tokio::test]
async fn profile_with_env_allowlist_resolves_listed_var() {
    let _lock = ENV_LOCK.lock().await;
    let _data_dir = DataDirGuard::new();
    let name = format!("PLURORA_TEST_RESOLVER_PROBE_{}", std::process::id());
    let _env = EnvVarGuard::set(&name, "synthetic-env-secret-value");
    let profile = profile_from_yaml(&format!(
        "title: test\nsecret_resolver:\n  env_allowlist:\n    - {name}\n"
    ));

    let config = runtime_config_from_profile(&profile).expect("build runtime config");
    let resolved = config
        .secret_resolver
        .resolver
        .resolve(&format!("secret_ref:env:{name}"))
        .await
        .expect("listed env var should resolve");
    assert_eq!(resolved, "synthetic-env-secret-value");
}

#[tokio::test]
async fn profile_with_store_disabled_denies_store_refs() {
    let _lock = ENV_LOCK.lock().await;
    let _data_dir = DataDirGuard::new();
    let profile = profile_from_yaml(
        "title: test\nsecret_resolver:\n  store_enabled: false\n  env_allowlist: []\n",
    );

    let config = runtime_config_from_profile(&profile).expect("build runtime config");
    let err = config
        .secret_resolver
        .resolver
        .resolve("secret_ref:store:ANY")
        .await
        .expect_err("store refs should be denied when all resolvers disabled");
    assert!(
        err.to_string().contains("no secret resolver configured"),
        "store-disabled empty profile should fail closed with DenyAll: {err}"
    );
}

#[test]
fn host_profile_default_local_exec_yields_deny_all() {
    let profile = profile_from_yaml("title: test\n");
    let config = runtime_config_from_profile(&profile).expect("build runtime config");
    assert!(matches!(
        config.local_exec_executor,
        LocalExecExecutorConfig::DenyAll
    ));
}

#[test]
fn host_profile_live_local_exec_rejects_empty_allowlists() {
    let profile =
        profile_from_yaml("title: test\nlocal_exec:\n  enabled: true\n  executor: live\n");
    let err = match runtime_config_from_profile(&profile) {
        Ok(_) => panic!("live local exec without allowlists should fail closed"),
        Err(err) => err,
    };
    let msg = err.to_string();
    assert!(msg.contains("allowed_programs") || msg.contains("allowed_working_dirs"));
}

#[tokio::test]
async fn host_profile_live_local_exec_runs_allowed_tiny_command() -> anyhow::Result<()> {
    let Some(echo) = first_existing(&["/bin/echo", "/usr/bin/echo"]) else {
        eprintln!("skipping: echo binary not found");
        return Ok(());
    };
    let tmp = tempfile::tempdir()?;
    let yaml = format!(
        "title: test\nlocal_exec:\n  enabled: true\n  executor: live\n  allowed_programs:\n    - {}\n  allowed_working_dirs:\n    - {}\n  max_duration_ms: 5000\n  max_log_bytes: 4096\n",
        echo.display(),
        tmp.path().display()
    );
    let profile = profile_from_yaml(&yaml);
    let config = runtime_config_from_profile(&profile).expect("build runtime config");
    let executor = config.local_exec_executor.executor();
    let response = executor
        .start(LocalExecStartRequest {
            target_id: "local".to_string(),
            command: plurora_runtime::ExecCommand {
                program: echo.display().to_string(),
                args: vec!["host-profile-local-exec".to_string()],
            },
            cwd: Some(tmp.path().to_path_buf()),
            env: Default::default(),
            lifecycle: plurora_runtime::ExecLifecyclePolicy::StopOnSessionClose,
            resource_limits: plurora_runtime::ExecResourceLimits::default(),
            readiness_probe: plurora_runtime::ReadinessProbe::default(),
            port_names: Vec::new(),
        })
        .await?;
    assert!(response.exec_id.is_some());
    assert_eq!(response.status.kind, ExecStatusKind::Running);
    Ok(())
}
