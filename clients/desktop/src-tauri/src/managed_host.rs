use std::{
    collections::HashSet,
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

use tauri::{async_runtime::Receiver, App, AppHandle, Manager};
use tauri_plugin_shell::{
    process::{CommandChild, CommandEvent},
    ShellExt,
};
use uuid::Uuid;

const LISTEN_PREFIX: &str = "PLURORA_HOST_LISTEN_ADDR=";
const DESKTOP_PROFILE_NAME: &str = "desktop";
const FIRST_PARTY_MANIFESTS: [(&str, &str); 7] = [
    ("git-tools-lab", "git-tools-lab/manifest.yaml"),
    ("integrity-lab", "integrity-lab/manifest.yaml"),
    ("install-lab", "install-lab/manifest.yaml"),
    ("docker-runtime-lab", "docker-runtime-lab/manifest.yaml"),
    ("source-intake-lab", "source-intake-lab/manifest.yaml"),
    ("secret-store-lab", "secret-store-lab/manifest.yaml"),
    ("workspace-lab", "workspace-lab/manifest.yaml"),
];

#[derive(Default)]
pub struct ManagedHostState {
    child: Mutex<Option<CommandChild>>,
    stopping: AtomicBool,
}

impl ManagedHostState {
    fn install(&self, child: CommandChild) {
        self.stopping.store(false, Ordering::Release);
        *self
            .child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(child);
    }

    fn forget_finished(&self) {
        self.child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
    }

    pub fn stop(&self) {
        self.stopping.store(true, Ordering::Release);
        let child = self
            .child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(child) = child {
            if let Err(error) = child.kill() {
                eprintln!("failed to stop managed Host process: {error}");
            }
        }
    }

    fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::Acquire)
    }
}

pub fn start(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    plurora_core::paths::ensure_initialized()?;
    let data_dir = plurora_core::paths::data_dir()?;
    let static_dir = resolve_static_dir(app)?;
    let profile_path = ensure_desktop_profile(app, &data_dir)?;
    if !static_dir.join("index.html").is_file() {
        return Err(format!(
            "managed Host static directory has no index.html: {}",
            static_dir.display()
        )
        .into());
    }

    let access_token = generate_secret();
    let bootstrap_nonce = generate_secret();
    let args = build_sidecar_args(&static_dir, &data_dir, &profile_path);
    let (events, child) = app
        .shell()
        .sidecar("plurora-host")?
        .args(args)
        .env("PLURORA_HTTP_ACCESS_TOKEN", &access_token)
        .env("PLURORA_HTTP_BOOTSTRAP_TOKEN", &bootstrap_nonce)
        .spawn()?;

    app.state::<ManagedHostState>().install(child);
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = supervise(app_handle.clone(), events, bootstrap_nonce).await {
            eprintln!("managed Plurora Host failed: {error}");
            app_handle.state::<ManagedHostState>().stop();
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.show();
            }
        }
    });
    Ok(())
}

pub fn stop(app: &AppHandle) {
    app.state::<ManagedHostState>().stop();
}

async fn supervise(
    app: AppHandle,
    mut events: Receiver<CommandEvent>,
    bootstrap_nonce: String,
) -> Result<(), String> {
    let listen_addr = wait_for_listen_address(&mut events).await?;
    wait_for_health_while_draining(listen_addr, &mut events).await?;

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is unavailable".to_string())?;
    let url = bootstrap_url(listen_addr, &bootstrap_nonce)?;
    window.navigate(url).map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;

    while let Some(event) = events.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => log_sidecar_line("host", &bytes),
            CommandEvent::Stderr(bytes) => log_sidecar_line("host stderr", &bytes),
            CommandEvent::Error(error) => eprintln!("managed Host output error: {error}"),
            CommandEvent::Terminated(payload) => {
                let state = app.state::<ManagedHostState>();
                state.forget_finished();
                if !state.is_stopping() {
                    eprintln!(
                        "managed Host terminated unexpectedly (code={:?}, signal={:?})",
                        payload.code, payload.signal
                    );
                    app.exit(1);
                }
                return Ok(());
            }
            _ => {}
        }
    }

    Err("managed Host event channel closed".to_string())
}

async fn wait_for_health_while_draining(
    addr: SocketAddr,
    events: &mut Receiver<CommandEvent>,
) -> Result<(), String> {
    let health = wait_for_health(addr);
    tokio::pin!(health);

    loop {
        tokio::select! {
            result = &mut health => return result,
            event = events.recv() => match event {
                Some(CommandEvent::Stdout(bytes)) => log_sidecar_line("host", &bytes),
                Some(CommandEvent::Stderr(bytes)) => log_sidecar_line("host stderr", &bytes),
                Some(CommandEvent::Error(error)) => return Err(format!("Host process output error: {error}")),
                Some(CommandEvent::Terminated(payload)) => {
                    return Err(format!("Host exited before health readiness (code={:?}, signal={:?})", payload.code, payload.signal));
                }
                Some(_) => {}
                None => return Err("Host event channel closed before health readiness".to_string()),
            }
        }
    }
}

async fn wait_for_listen_address(
    events: &mut Receiver<CommandEvent>,
) -> Result<SocketAddr, String> {
    let deadline = tokio::time::sleep(Duration::from_secs(20));
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => return Err("timed out waiting for Host listen address".to_string()),
            event = events.recv() => match event {
                Some(CommandEvent::Stdout(bytes)) => {
                    log_sidecar_line("host", &bytes);
                    if let Some(addr) = parse_listen_addr_line(&bytes) {
                        return Ok(addr);
                    }
                }
                Some(CommandEvent::Stderr(bytes)) => log_sidecar_line("host stderr", &bytes),
                Some(CommandEvent::Error(error)) => return Err(format!("Host process output error: {error}")),
                Some(CommandEvent::Terminated(payload)) => {
                    return Err(format!("Host exited before readiness (code={:?}, signal={:?})", payload.code, payload.signal));
                }
                Some(_) => {}
                None => return Err("Host event channel closed before readiness".to_string()),
            }
        }
    }
}

async fn wait_for_health(addr: SocketAddr) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(1))
        .build()
        .map_err(|error| error.to_string())?;
    let endpoint = format!("http://{addr}/healthz");
    let deadline = Instant::now() + Duration::from_secs(20);

    while Instant::now() < deadline {
        if let Ok(response) = client.get(&endpoint).send().await {
            if response.status().is_success() {
                if let Ok(body) = response.text().await {
                    if body.trim() == "ok" {
                        return Ok(());
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    Err(format!("Host health check timed out at {endpoint}"))
}

fn resolve_static_dir(_app: &App) -> Result<PathBuf, Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");

    #[cfg(not(debug_assertions))]
    let path = _app.path().resource_dir()?.join("web");

    Ok(path.canonicalize()?)
}

fn build_sidecar_args(static_dir: &Path, data_dir: &Path, profile_path: &Path) -> Vec<String> {
    vec![
        "host".to_string(),
        "serve".to_string(),
        "--http".to_string(),
        "127.0.0.1:0".to_string(),
        "--static-dir".to_string(),
        static_dir.to_string_lossy().into_owned(),
        "--data-dir".to_string(),
        data_dir.to_string_lossy().into_owned(),
        "--profile".to_string(),
        profile_path.to_string_lossy().into_owned(),
    ]
}

fn ensure_desktop_profile(
    app: &App,
    data_dir: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let sources = resolve_first_party_manifest_sources(app)?;
    ensure_desktop_profile_from_sources(data_dir, &sources)
}

fn resolve_first_party_manifest_sources(
    _app: &App,
) -> Result<Vec<(String, PathBuf)>, Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../packages/plurora");

    #[cfg(not(debug_assertions))]
    let root = _app.path().resource_dir()?.join("plurora-packages");

    FIRST_PARTY_MANIFESTS
        .iter()
        .map(|(name, relative)| {
            let path = root.join(relative);
            if !path.is_file() {
                return Err(format!(
                    "managed Host package manifest is unavailable: {}",
                    path.display()
                )
                .into());
            }
            Ok(((*name).to_string(), path))
        })
        .collect()
}

fn ensure_desktop_profile_from_sources(
    data_dir: &Path,
    sources: &[(String, PathBuf)],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let managed_packages = data_dir.join("desktop").join("plurora-packages");
    fs::create_dir_all(&managed_packages)?;
    let mut autoload = Vec::with_capacity(sources.len());
    for (name, source) in sources {
        let destination = managed_packages.join(name).join("manifest.yaml");
        let bytes = fs::read(source)?;
        if fs::read(&destination).ok().as_deref() != Some(bytes.as_slice()) {
            atomic_replace_file(&destination, &bytes)?;
        }
        autoload.push(portable_path(&destination));
    }

    let profiles = data_dir.join("profiles");
    fs::create_dir_all(&profiles)?;
    let profile_path = profiles.join(format!("{DESKTOP_PROFILE_NAME}.yaml"));
    let mut profile = if profile_path.is_file() {
        serde_yaml::from_str::<serde_json::Value>(&fs::read_to_string(&profile_path)?)?
    } else {
        serde_json::json!({})
    };
    let profile_object = profile
        .as_object_mut()
        .ok_or("managed Desktop profile must be a YAML object")?;
    profile_object.insert(
        "title".to_string(),
        serde_json::Value::String("Plurora Desktop".to_string()),
    );
    profile_object.insert(
        "event_store".to_string(),
        serde_json::json!({ "kind": "sqlite", "path": "desktop-events.sqlite" }),
    );
    let existing = profile_object
        .get("autoload")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut seen = HashSet::new();
    let mut merged = Vec::new();
    for entry in autoload
        .into_iter()
        .map(serde_json::Value::String)
        .chain(existing.into_iter())
    {
        let Some(path) = entry.as_str() else {
            continue;
        };
        let normalized = portable_path(Path::new(path));
        if seen.insert(normalized.clone()) {
            merged.push(serde_json::Value::String(normalized));
        }
    }
    profile_object.insert("autoload".to_string(), serde_json::Value::Array(merged));
    atomic_replace_file(&profile_path, serde_yaml::to_string(&profile)?.as_bytes())?;
    Ok(profile_path)
}

fn portable_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn atomic_replace_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    let backup = path.with_extension(format!("bak-{}", Uuid::new_v4()));
    fs::write(&temporary, bytes)?;
    if path.exists() {
        fs::rename(path, &backup)?;
        if let Err(error) = fs::rename(&temporary, path) {
            let _ = fs::rename(&backup, path);
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        fs::remove_file(backup).ok();
    } else {
        fs::rename(temporary, path)?;
    }
    Ok(())
}

fn generate_secret() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn parse_listen_addr_line(bytes: &[u8]) -> Option<SocketAddr> {
    let line = std::str::from_utf8(bytes).ok()?.trim();
    let addr: SocketAddr = line.strip_prefix(LISTEN_PREFIX)?.parse().ok()?;
    (addr.ip() == IpAddr::V4(Ipv4Addr::LOCALHOST) && addr.port() != 0).then_some(addr)
}

fn bootstrap_url(addr: SocketAddr, nonce: &str) -> Result<tauri::Url, String> {
    let mut url = tauri::Url::parse(&format!("http://{addr}/host/bootstrap"))
        .map_err(|error| error.to_string())?;
    url.query_pairs_mut().append_pair("nonce", nonce);
    Ok(url)
}

fn log_sidecar_line(source: &str, bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    let line = text.trim();
    if !line.is_empty() {
        eprintln!("[{source}] {line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listen_handshake_accepts_only_loopback_nonzero_ports() {
        assert_eq!(
            parse_listen_addr_line(b"PLURORA_HOST_LISTEN_ADDR=127.0.0.1:43117"),
            Some("127.0.0.1:43117".parse().unwrap())
        );
        assert!(parse_listen_addr_line(b"PLURORA_HOST_LISTEN_ADDR=0.0.0.0:43117").is_none());
        assert!(parse_listen_addr_line(b"PLURORA_HOST_LISTEN_ADDR=127.0.0.1:0").is_none());
        assert!(parse_listen_addr_line(b"host serving 127.0.0.1:43117").is_none());
    }

    #[test]
    fn sidecar_secrets_are_url_safe_and_not_part_of_arguments() {
        let first = generate_secret();
        let second = generate_secret();
        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|character| character.is_ascii_hexdigit()));
        assert_ne!(first, second);

        let args = build_sidecar_args(
            Path::new("web"),
            Path::new("data"),
            Path::new("data/profiles/desktop.yaml"),
        );
        assert_eq!(&args[..4], ["host", "serve", "--http", "127.0.0.1:0"]);
        assert!(!args.iter().any(|arg| arg.contains(&first)));
        assert!(args
            .windows(2)
            .any(|pair| { pair == ["--profile", "data/profiles/desktop.yaml"] }));
    }

    #[test]
    fn bootstrap_url_contains_only_the_one_time_nonce() {
        let url = bootstrap_url("127.0.0.1:43117".parse().unwrap(), "secret").unwrap();
        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:43117/host/bootstrap?nonce=secret"
        );
        assert!(!url.as_str().contains("access_token="));
        assert!(!url.as_str().contains("plurora_token="));
    }

    #[test]
    fn stop_is_idempotent_without_a_child() {
        let state = ManagedHostState::default();
        state.stop();
        state.stop();
        assert!(state.is_stopping());
    }

    #[test]
    fn desktop_profile_is_sqlite_backed_and_preserves_custom_autoload_entries() {
        let temporary = tempfile::tempdir().unwrap();
        let data_dir = temporary.path().join("data");
        let source_root = temporary.path().join("resources");
        fs::create_dir_all(&source_root).unwrap();
        let first = source_root.join("first.yaml");
        let second = source_root.join("second.yaml");
        fs::write(&first, "id: plurora/first\n").unwrap();
        fs::write(&second, "id: plurora/second\n").unwrap();
        fs::create_dir_all(data_dir.join("profiles")).unwrap();
        fs::write(
            data_dir.join("profiles/desktop.yaml"),
            "autoload:\n  - C:/custom/manifest.yaml\n",
        )
        .unwrap();

        let profile = ensure_desktop_profile_from_sources(
            &data_dir,
            &[("first".to_string(), first), ("second".to_string(), second)],
        )
        .unwrap();
        let value: serde_json::Value =
            serde_yaml::from_str(&fs::read_to_string(profile).unwrap()).unwrap();
        assert_eq!(value["event_store"]["kind"], "sqlite");
        assert_eq!(value["event_store"]["path"], "desktop-events.sqlite");
        let entries = value["autoload"].as_array().unwrap();
        assert!(entries
            .iter()
            .any(|entry| entry == "C:/custom/manifest.yaml"));
        assert_eq!(entries.len(), 3);
        assert!(entries[0]
            .as_str()
            .unwrap()
            .ends_with("first/manifest.yaml"));
        assert!(entries[1]
            .as_str()
            .unwrap()
            .ends_with("second/manifest.yaml"));
    }
}
