//! Local backend lifecycle.
//!
//! The desktop app needs the Go backend. This module reconnects to the one
//! recorded in `<data>/run/core.json` (after validating it, not just "some port
//! is open") or starts the bundled `wa-bot-backend` through the supervisor with
//! an ephemeral port, with its data in a fixed folder — `~/.wa-bot/{database,
//! media}` — so the login session survives wherever the binaries live (AppImage
//! mount, build tree, ...).

pub mod process;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use wabot_supervisor::{SpawnOptions, Supervisor};

use self::process::BACKEND_PROCESS_NAME;
use crate::TOKIO_RT;

/// Logical identity written to `core.json`.
pub const CORE_NAME: &str = "aorus22.wabot-core";
const CORE_SCHEMA_VERSION: u32 = 1;

/// The spawned backend and its registry record, kept alive for the process
/// lifetime.
static BACKEND: Mutex<Option<(Supervisor, CoreRecord)>> = Mutex::new(None);

/// Whether this run manages the backend through `core.json`. Explicit URL/port
/// overrides never do, and never touch the file.
static USE_REGISTRY: AtomicBool = AtomicBool::new(false);

/// API base URL the HTTP/WebSocket clients should currently use.
static API_URL: Mutex<String> = Mutex::new(String::new());

pub fn set_api_url(url: &str) {
    *API_URL.lock() = url.to_string();
}

pub fn api_url() -> String {
    API_URL.lock().clone()
}

/// WebSocket URL for the current backend; the WS client asks again on every
/// reconnect, so a restarted backend on a new port is followed automatically.
pub fn ws_url() -> String {
    let api = api_url();
    let base = api.trim_end_matches("/api");
    match base.strip_prefix("https://") {
        Some(rest) => format!("wss://{rest}/ws"),
        None => format!("{}/ws", base.replace("http://", "ws://")),
    }
}

/// What `<data>/run/core.json` holds. No secrets: it only says where the
/// backend this app started is listening.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreRecord {
    pub schema_version: u32,
    pub name: String,
    /// PID of the Go backend, not of this app.
    pub pid: u32,
    pub port: u16,
    /// Random per spawn, so an old instance never deletes a newer one's record.
    pub instance_id: String,
    pub started_at: String,
}

impl CoreRecord {
    fn new(pid: u32, port: u16) -> Self {
        Self {
            schema_version: CORE_SCHEMA_VERSION,
            name: CORE_NAME.to_string(),
            pid,
            port,
            instance_id: new_instance_id(),
            started_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        }
    }

    fn api_url(&self) -> String {
        format!("http://127.0.0.1:{}/api", self.port)
    }
}

/// UUID-shaped random id (v4 layout). Not a credential, only a spawn marker.
fn new_instance_id() -> String {
    use std::hash::{BuildHasher, Hasher};

    let word = || {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        );
        hasher.write_u32(std::process::id());
        hasher.finish()
    };
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&word().to_le_bytes());
    bytes[8..].copy_from_slice(&word().to_le_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

/// `<data_dir>/run/core.json`.
pub fn core_file() -> PathBuf {
    data_dir().join("run").join("core.json")
}

/// The record in `path`; `None` when it is missing, unreadable, corrupt, or of
/// another schema version.
pub fn read_core(path: &Path) -> Option<CoreRecord> {
    let record: CoreRecord = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    (record.schema_version == CORE_SCHEMA_VERSION && record.name == CORE_NAME).then_some(record)
}

/// Write the record atomically (temp file in the same folder, then rename).
pub fn write_core(path: &Path, record: &CoreRecord) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("core.json.{}.tmp", std::process::id()));
    let json = serde_json::to_vec_pretty(record).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Delete `path` only while it still describes `record` (same instance, pid and
/// port). Returns whether it was deleted.
pub fn remove_core_if(path: &Path, record: &CoreRecord) -> bool {
    match read_core(path) {
        Some(current)
            if current.instance_id == record.instance_id
                && current.pid == record.pid
                && current.port == record.port =>
        {
            std::fs::remove_file(path).is_ok()
        }
        _ => false,
    }
}

/// What to do about the backend `core.json` points at.
#[derive(Debug, PartialEq)]
enum Existing {
    /// Alive, owns its port, answers like WA Bot: reconnect.
    Reuse,
    /// Gone: the record is leftover and a new backend may start.
    Stale,
    /// Cannot be told apart safely from a live backend on our data.
    Ambiguous(String),
}

/// Facts gathered about a recorded backend, kept apart from the decision so it
/// can be tested without real processes.
struct Probe {
    /// Executable name of the recorded pid, `None` when no such process.
    pid_name: Option<String>,
    /// Pid and executable name of whatever listens on the recorded port.
    listener: Option<(u32, Option<String>)>,
    responds: bool,
}

fn classify(record: &CoreRecord, probe: &Probe) -> Existing {
    let is_backend = |name: &Option<String>| name.as_deref() == Some(BACKEND_PROCESS_NAME);
    if is_backend(&probe.pid_name) {
        return match &probe.listener {
            Some((pid, _)) if *pid == record.pid && probe.responds => Existing::Reuse,
            _ => Existing::Ambiguous(format!(
                "backend pid {} from {CORE_NAME} is running but does not serve port {} as expected",
                record.pid, record.port
            )),
        };
    }
    match &probe.listener {
        Some((pid, name)) if is_backend(name) => Existing::Ambiguous(format!(
            "another {BACKEND_PROCESS_NAME} (pid {pid}) listens on port {}, not the recorded pid {}",
            record.port, record.pid
        )),
        // Gone, or its port now belongs to an unrelated process: never adopted.
        _ => Existing::Stale,
    }
}

fn inspect(record: &CoreRecord) -> Existing {
    let listener = crate::backend_process::pid_listening_on(record.port)
        .map(|pid| (pid, crate::backend_process::process_name(pid)));
    let probe = Probe {
        pid_name: crate::backend_process::process_name(record.pid),
        responds: listener.is_some() && is_wabot_backend(record.port),
        listener,
    };
    classify(record, &probe)
}

/// Fixed data folder: `$WABOT_HOME`, else `~/.wa-bot`.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("WABOT_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".wa-bot"))
        .unwrap_or_else(|| PathBuf::from(".wa-bot"))
}

/// Locate the bundled backend binary: `$WABOT_BACKEND`, then next to the
/// executable (AppImage `usr/bin`, `compiled/gpui-linux`), `be/`, and the repo
/// root of a dev build (`desktop-gpui/target/release/wabot` → repo root).
pub fn backend_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("WABOT_BACKEND").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = format!("wa-bot-backend{}", wabot_supervisor::BACKEND_EXE_SUFFIX);
    [
        dir.join(&name),
        dir.join("be").join(&name),
        dir.join("../../..").join(&name),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

/// True when `port` answers `GET /api/status` like the WA Bot backend does
/// (`{"isLoggedIn": ...}`). An open port alone is not enough: other local
/// servers (a Caddy on 8080, say) accept connections and answer with HTML.
pub fn is_wabot_backend(port: u16) -> bool {
    use std::io::{Read, Write};

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(100))
    else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(400)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(400)));
    let request = format!(
        "GET /api/status HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = Vec::new();
    let _ = stream.take(8192).read_to_end(&mut response);
    looks_like_wabot_status(&String::from_utf8_lossy(&response))
}

pub fn looks_like_wabot_status(response: &str) -> bool {
    response.starts_with("HTTP/1.1 200") && response.contains("\"isLoggedIn\"")
}

/// Port of a loopback `http://127.0.0.1:<port>/...` URL.
pub fn loopback_port(url: &str) -> Option<u16> {
    let rest = url.strip_prefix("http://")?;
    let authority = rest.split('/').next()?;
    let (host, port) = authority.rsplit_once(':')?;
    matches!(host, "127.0.0.1" | "localhost").then(|| port.parse().ok())?
}

/// Reconnect to the backend recorded in `core.json`, or start the bundled one.
/// Returns the API base URL. Refuses (with a clear error) when a backend that
/// cannot be verified may already be using the data folder, rather than risk
/// two backends on one SQLite session.
pub fn connect_or_start() -> Result<String, String> {
    USE_REGISTRY.store(true, Ordering::SeqCst);
    let path = core_file();
    if let Some(record) = read_core(&path) {
        match inspect(&record) {
            Existing::Reuse => {
                eprintln!("[backend] reconnecting to pid {} on port {}", record.pid, record.port);
                return Ok(record.api_url());
            }
            Existing::Ambiguous(why) => return Err(why),
            Existing::Stale => {
                remove_core_if(&path, &record);
            }
        }
    }
    let dir = data_dir();
    let unknown = crate::backend_process::backend_pids_using(&dir);
    if !unknown.is_empty() {
        return Err(format!(
            "wa-bot-backend (pid {}) is already using {dir:?} but is not registered in {path:?}; stop it first",
            unknown.iter().map(u32::to_string).collect::<Vec<_>>().join(", ")
        ));
    }
    start_bundled()
}

/// Start the bundled backend with its data under [`data_dir`]. Returns the API
/// base URL (`http://127.0.0.1:<port>/api`).
pub fn start_bundled() -> Result<String, String> {
    TOKIO_RT.block_on(spawn_bundled(None))
}

async fn spawn_bundled(preferred_port: Option<u16>) -> Result<String, String> {
    let binary = backend_binary().ok_or_else(|| {
        "wa-bot-backend not found next to the app (set WABOT_BACKEND to its path)".to_string()
    })?;
    let base = data_dir();
    std::fs::create_dir_all(&base).map_err(|e| format!("create {base:?}: {e}"))?;
    let (db_dir, media_dir) =
        wabot_supervisor::ensure_data_dirs(&base).map_err(|e| format!("data dirs: {e}"))?;

    let mut opts = SpawnOptions::new(&binary, db_dir, media_dir)
        .map_err(|e| e.to_string())?
        // The Go backend opens `database/*.db` relative to its cwd.
        .with_work_dir(&base);
    if let Some(port) = preferred_port {
        opts = opts.with_preferred_port(port);
    }

    let mut supervisor = Supervisor::new();
    let info = supervisor.spawn(opts).await.map_err(|e| e.to_string())?;
    eprintln!(
        "[backend] started {binary:?} on port {} (data: {base:?})",
        info.port
    );
    let record = CoreRecord::new(info.pid, info.port);
    if USE_REGISTRY.load(Ordering::SeqCst) {
        if let Err(e) = write_core(&core_file(), &record) {
            eprintln!("[backend] could not write {:?}: {e}", core_file());
        }
    }
    *BACKEND.lock() = Some((supervisor, record));
    Ok(format!("{}/api", info.base_url))
}

/// Restart the backend and return its API base URL, which is `api_url` again
/// whenever its port could be reused. The registry record and the URL the
/// HTTP/WebSocket clients follow are updated to the new backend.
///
/// A backend this process started is stopped through its supervisor. One it
/// merely adopted (started by the resident daemon, say) is found by its port
/// and stopped from here. Either way this process then owns the new one.
pub async fn restart(api_url: &str) -> Result<String, String> {
    let port = loopback_port(api_url);
    let path = core_file();
    let owned = BACKEND.lock().take();
    let mut old_record = owned.as_ref().map(|(_, record)| record.clone());
    if let Some((mut supervisor, _)) = owned {
        let _ = supervisor.stop(Duration::from_secs(5)).await;
    }
    if let Some(port) = port {
        if TOKIO_RT.spawn_blocking(move || is_wabot_backend(port)).await.unwrap_or(false) {
            if old_record.is_none() {
                old_record = read_core(&path).filter(|record| record.port == port);
            }
            crate::backend_process::stop_listener(port, Duration::from_secs(5), move || is_wabot_backend(port)).await?;
        }
    }
    if USE_REGISTRY.load(Ordering::SeqCst) {
        if let Some(record) = &old_record {
            remove_core_if(&path, record);
        }
    }

    let url = match spawn_bundled(port).await {
        Ok(url) => url,
        // The port can linger for a moment; any free one beats staying down.
        Err(_) if port.is_some() => spawn_bundled(None).await?,
        Err(e) => return Err(e),
    };
    set_api_url(&url);
    Ok(url)
}

/// Stop the backend this process started, if any, and drop its registry record.
pub async fn shutdown() {
    let backend = BACKEND.lock().take();
    if let Some((mut supervisor, record)) = backend {
        let _ = supervisor.stop(Duration::from_secs(3)).await;
        if USE_REGISTRY.load(Ordering::SeqCst) {
            remove_core_if(&core_file(), &record);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_backend_status_json() {
        assert!(looks_like_wabot_status(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"isLoggedIn\":false}"
        ));
    }

    #[test]
    fn rejects_other_servers() {
        // Caddy serving an SPA answers 200 with HTML for every path.
        assert!(!looks_like_wabot_status("HTTP/1.1 200 OK\r\nServer: Caddy\r\n\r\n<!doctype html>"));
        assert!(!looks_like_wabot_status("HTTP/1.1 404 Not Found\r\n\r\n{\"isLoggedIn\":false}"));
        assert!(!looks_like_wabot_status(""));
    }

    #[test]
    fn extracts_loopback_ports_only() {
        assert_eq!(loopback_port("http://127.0.0.1:51234/api"), Some(51234));
        assert_eq!(loopback_port("http://localhost:3090/api"), Some(3090));
        assert_eq!(loopback_port("https://example.com:443/api"), None);
        assert_eq!(loopback_port("http://10.0.0.5:3090/api"), None);
        assert_eq!(loopback_port("http://127.0.0.1/api"), None);
    }

    #[test]
    fn data_dir_honours_override() {
        // SAFETY of env in tests: single assertion, restored immediately.
        std::env::set_var("WABOT_HOME", "/tmp/wabot-test-home");
        assert_eq!(data_dir(), PathBuf::from("/tmp/wabot-test-home"));
        std::env::remove_var("WABOT_HOME");
        assert!(data_dir().ends_with(".wa-bot"));
    }

    fn record(pid: u32, port: u16) -> CoreRecord {
        CoreRecord::new(pid, port)
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wabot-core-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn record_round_trips_through_an_atomic_write() {
        let path = scratch("roundtrip").join("run").join("core.json");
        let rec = record(4242, 45321);
        write_core(&path, &rec).unwrap();
        assert_eq!(read_core(&path), Some(rec));
        // No temp file is left behind.
        assert_eq!(std::fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn missing_corrupt_or_foreign_records_read_as_none() {
        let dir = scratch("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("core.json");
        assert_eq!(read_core(&path), None);
        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(read_core(&path), None);
        let mut other = record(1, 2);
        other.schema_version = 99;
        write_core(&path, &other).unwrap();
        assert_eq!(read_core(&path), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn only_the_matching_record_is_removed() {
        let dir = scratch("remove");
        let path = dir.join("core.json");
        let current = record(10, 3000);
        write_core(&path, &current).unwrap();

        // An older instance (other id) must not delete the newer record.
        assert!(!remove_core_if(&path, &record(10, 3000)));
        let mut moved = current.clone();
        moved.port = 3001;
        assert!(!remove_core_if(&path, &moved));
        assert!(path.exists());

        assert!(remove_core_if(&path, &current));
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn instance_ids_are_uuid_shaped_and_unique() {
        let a = new_instance_id();
        assert_eq!(a.len(), 36);
        assert_eq!(a.matches('-').count(), 4);
        assert_eq!(&a[14..15], "4");
        assert_ne!(a, new_instance_id());
    }

    fn probe(pid_name: Option<&str>, listener: Option<(u32, &str)>, responds: bool) -> Probe {
        Probe {
            pid_name: pid_name.map(str::to_string),
            listener: listener.map(|(pid, name)| (pid, Some(name.to_string()))),
            responds,
        }
    }

    #[test]
    fn a_live_backend_owning_its_port_is_reused() {
        let rec = record(10, 3000);
        let p = probe(Some("wa-bot-backend"), Some((10, "wa-bot-backend")), true);
        assert_eq!(classify(&rec, &p), Existing::Reuse);
    }

    #[test]
    fn a_dead_backend_is_stale() {
        let rec = record(10, 3000);
        assert_eq!(classify(&rec, &probe(None, None, false)), Existing::Stale);
        // The pid was recycled by some other program.
        assert_eq!(classify(&rec, &probe(Some("firefox"), None, false)), Existing::Stale);
    }

    #[test]
    fn a_port_taken_by_another_program_is_never_adopted() {
        let rec = record(10, 3000);
        let p = probe(None, Some((77, "caddy")), true);
        assert_eq!(classify(&rec, &p), Existing::Stale);
    }

    #[test]
    fn unverifiable_backends_are_ambiguous() {
        let rec = record(10, 3000);
        // Recorded pid is a backend but is not listening on the recorded port.
        let p = probe(Some("wa-bot-backend"), None, false);
        assert!(matches!(classify(&rec, &p), Existing::Ambiguous(_)));
        // Listening, but not answering like WA Bot.
        let p = probe(Some("wa-bot-backend"), Some((10, "wa-bot-backend")), false);
        assert!(matches!(classify(&rec, &p), Existing::Ambiguous(_)));
        // Recorded pid is gone but a different backend sits on the port.
        let p = probe(None, Some((99, "wa-bot-backend")), true);
        assert!(matches!(classify(&rec, &p), Existing::Ambiguous(_)));
    }

    #[test]
    fn websocket_url_follows_the_api_url() {
        set_api_url("http://127.0.0.1:45321/api");
        assert_eq!(ws_url(), "ws://127.0.0.1:45321/ws");
        set_api_url("https://bot.example.com/api");
        assert_eq!(ws_url(), "wss://bot.example.com/ws");
    }
}
