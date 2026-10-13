//! Local backend lifecycle.
//!
//! The desktop app needs the Go backend. This module finds one that is already
//! running (validated, not just "some port is open") or starts the bundled
//! `wa-bot-backend` through the supervisor, with its data in a fixed folder —
//! `~/.wa-bot/{database,media}` — so the login session survives wherever the
//! binaries live (AppImage mount, build tree, ...).

use std::path::PathBuf;
use std::time::Duration;

use parking_lot::Mutex;
use wabot_supervisor::{SpawnOptions, Supervisor};

use crate::TOKIO_RT;

/// Ports an externally started backend is looked for on: `.env` default,
/// compiled default, external default.
pub const PROBE_PORTS: [u16; 3] = [3090, 3000, 8080];

/// The spawned backend, kept alive for the process lifetime.
static BACKEND: Mutex<Option<Supervisor>> = Mutex::new(None);

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

/// A running, validated backend: the standard ports, then the URL remembered
/// from the last session (a backend that outlived its app is adopted, never
/// duplicated — two backends on one database would fight over the session).
pub fn find_running(last_backend_url: Option<&str>) -> Option<String> {
    PROBE_PORTS
        .iter()
        .copied()
        .chain(last_backend_url.and_then(loopback_port))
        .find(|port| is_wabot_backend(*port))
        .map(|port| format!("http://127.0.0.1:{port}/api"))
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
    *BACKEND.lock() = Some(supervisor);
    Ok(format!("{}/api", info.base_url))
}

/// Restart the backend and return its API base URL, which is `api_url` again
/// whenever its port could be reused.
///
/// A backend this process started is stopped through its supervisor. One it
/// merely adopted (started by the resident daemon, say) is found by its port
/// and stopped from here. Either way this process then owns the new one.
pub async fn restart(api_url: &str) -> Result<String, String> {
    let port = loopback_port(api_url);
    let owned = BACKEND.lock().take();
    if let Some(mut supervisor) = owned {
        let _ = supervisor.stop(Duration::from_secs(5)).await;
    }
    if let Some(port) = port {
        if TOKIO_RT.spawn_blocking(move || is_wabot_backend(port)).await.unwrap_or(false) {
            crate::backend_process::stop_listener(port, Duration::from_secs(5), move || is_wabot_backend(port)).await?;
        }
    }

    match spawn_bundled(port).await {
        Ok(url) => Ok(url),
        // The port can linger for a moment; any free one beats staying down.
        Err(_) if port.is_some() => spawn_bundled(None).await,
        Err(e) => Err(e),
    }
}

/// Stop the backend this process started, if any.
pub async fn shutdown() {
    let supervisor = BACKEND.lock().take();
    if let Some(mut supervisor) = supervisor {
        let _ = supervisor.stop(Duration::from_secs(3)).await;
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
}
