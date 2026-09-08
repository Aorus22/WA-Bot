//! Supervisor lifecycle tests — hermetic (temp dirs, ephemeral ports only).

use std::path::PathBuf;
use std::time::Duration;
use wabot_supervisor::{
    ensure_data_dirs, BackendStatus, SpawnOptions, Supervisor,
};

/// Fake backend: prints `BACKEND_PORT:<ephemeral>` then serves HTTP 200 for
/// every request (satisfies the readiness probe). No race: the OS assigns the
/// port via bind(0) before it is announced.
const FAKE_BACKEND_PY: &str = r#"
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

class H(BaseHTTPRequestHandler):
    def do_GET(self):
        body = b'{}'
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *args):
        pass

print('fake backend booting (noise before handshake)', flush=True)
server = HTTPServer(('127.0.0.1', 0), H)
print(f"BACKEND_PORT:{server.server_address[1]}", flush=True)
server.serve_forever()
"#;

/// Resolve a python interpreter, or `None` when neither exists (test skips).
fn resolve_python() -> Option<PathBuf> {
    for cand in ["python3", "python"] {
        // Search PATH manually (no extra deps).
        if let Some(paths) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&paths) {
                #[cfg(windows)]
                let file = dir.join(format!("{cand}.exe"));
                #[cfg(not(windows))]
                let file = dir.join(cand);
                if file.is_file() {
                    return Some(file);
                }
            }
        }
    }
    None
}

fn fake_opts(python: &PathBuf, dir: &std::path::Path) -> SpawnOptions {
    let script = dir.join("fake_backend.py");
    std::fs::write(&script, FAKE_BACKEND_PY).unwrap();
    let (db_dir, media_dir) = ensure_data_dirs(dir).unwrap();
    SpawnOptions::new(python, db_dir, media_dir)
        .unwrap()
        .with_args(vec![script.to_string_lossy().to_string()])
        .with_handshake_timeout(Duration::from_secs(20))
        .with_readiness_timeout(Duration::from_secs(20))
}

fn is_orphan_serving(base_url: &str) -> bool {
    // Synchronous probe: any TCP accept means something still serves.
    let url = base_url.trim_start_matches("http://");
    std::net::TcpStream::connect_timeout(
        &url.parse().expect("base_url must be socket addr"),
        Duration::from_secs(2),
    )
    .is_ok()
}

#[tokio::test]
async fn tracer_spawn_handshake_ready_stop_no_orphan() {
    let Some(python) = resolve_python() else {
        eprintln!("SKIP: no python3/python on PATH for fake backend");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let mut sup = Supervisor::new();
    let info = sup
        .spawn(fake_opts(&python, tmp.path()))
        .await
        .expect("spawn→handshake→ready must succeed against fake backend");
    assert!(info.base_url.starts_with("http://127.0.0.1:"));
    assert!(info.pid > 0);
    assert_eq!(sup.status(), BackendStatus::Ready);
    assert!(sup.child_pid().is_some());

    // Status channel observed Ready.
    let rx = sup.subscribe();
    assert_eq!(*rx.borrow(), BackendStatus::Ready);

    let base_url = info.base_url.clone();
    sup.stop(Duration::from_secs(5)).await.unwrap();
    assert!(sup.child_pid().is_none());
    assert!(sup.info().is_none());
    // No orphan: port no longer accepts connections, adopt finds nothing.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!is_orphan_serving(&base_url), "backend port must be closed after stop");
    assert!(Supervisor::adopt_or_clear(Some(base_url)).await.is_none());
}

#[tokio::test]
async fn crash_restart_reaches_ready_on_fresh_ephemeral_port() {
    let Some(python) = resolve_python() else {
        eprintln!("SKIP: no python3/python on PATH for fake backend");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();

    // First incarnation.
    let mut first = Supervisor::new();
    let info1 = first
        .spawn(fake_opts(&python, tmp.path()))
        .await
        .expect("first spawn must reach ready");
    // Force-kill without graceful stop (SIGKILL-level crash simulation).
    first.kill_force().await.unwrap();
    assert!(first.child_pid().is_none());

    // Second incarnation must handshake cleanly — the supervisor always
    // re-picks an ephemeral port, so no address-in-use is possible.
    let mut second = Supervisor::new();
    let info2 = second
        .spawn(fake_opts(&python, tmp.path()))
        .await
        .expect("respawn after crash must reach ready");
    assert_eq!(second.status(), BackendStatus::Ready);
    let _ = info1; // first backend is gone; only assert the restart works.
    second.stop(Duration::from_secs(5)).await.unwrap();
    assert!(!is_orphan_serving(&info2.base_url));
}

#[tokio::test]
async fn invalid_path_reports_failed_with_tail() {
    let tmp = tempfile::tempdir().unwrap();
    let (db_dir, media_dir) = ensure_data_dirs(tmp.path()).unwrap();
    let opts = SpawnOptions::new("nonexistent_backend_xyz_123", db_dir, media_dir).unwrap();
    let mut sup = Supervisor::new();
    let err = sup.spawn(opts).await.expect_err("invalid path must fail");
    match sup.status() {
        BackendStatus::Failed { reason, stderr_tail } => {
            assert!(!reason.is_empty());
            assert!(!stderr_tail.is_empty());
        }
        other => panic!("expected Failed, got {other:?}"),
    }
    let _ = err;
}

#[test]
fn data_dirs_idempotent_parity_with_main_js() {
    let tmp = tempfile::tempdir().unwrap();
    let (db1, media1) = ensure_data_dirs(tmp.path()).unwrap();
    assert_eq!(db1, tmp.path().join("database"));
    assert_eq!(media1, tmp.path().join("media"));
    // Second call with pre-existing dirs succeeds.
    let (db2, media2) = ensure_data_dirs(tmp.path()).unwrap();
    assert_eq!((db1, media1), (db2, media2));
}

/// Live-backend test: builds the real Go backend when `go` exists, drives a
/// full spawn→probe→stop cycle, asserts no orphan. Skips with a reason when
/// Go or the repo source is unavailable (e.g. CI without Go).
#[tokio::test]
async fn live_backend_if_go_available() {
    if std::process::Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP: `go` not on PATH");
        return;
    }
    // Locate repo root (crates/supervisor -> crates -> desktop-gpui -> repo).
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest
        .ancestors()
        .nth(3)
        .expect("repo root above desktop-gpui")
        .to_path_buf();
    if !repo_root.join("cmd/api").is_dir() {
        eprintln!("SKIP: cmd/api not found under {}", repo_root.display());
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    #[cfg(windows)]
    let bin = tmp.path().join("wa-bot-backend.exe");
    #[cfg(not(windows))]
    let bin = tmp.path().join("wa-bot-backend");
    let build = std::process::Command::new("go")
        .args(["build", "-o"])
        .arg(&bin)
        .arg("./cmd/api")
        .current_dir(&repo_root)
        .output();
    let Ok(out) = build else {
        eprintln!("SKIP: `go build` could not run");
        return;
    };
    if !out.status.success() {
        eprintln!(
            "SKIP: `go build ./cmd/api` failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }
    let (db_dir, media_dir) = ensure_data_dirs(tmp.path()).unwrap();
    // The Go backend opens `database/*.db` relative to cwd (ignoring DB_PATH
    // for open), so work_dir must be the base dir — main.js `cwd: userDataPath`.
    let opts = SpawnOptions::new(&bin, db_dir, media_dir)
        .unwrap()
        .with_work_dir(tmp.path())
        .with_handshake_timeout(Duration::from_secs(30))
        .with_readiness_timeout(Duration::from_secs(30));
    let mut sup = Supervisor::new();
    let info = sup.spawn(opts).await.expect("live backend must reach ready");
    assert_eq!(sup.status(), BackendStatus::Ready);
    let base = info.base_url.clone();
    sup.stop(Duration::from_secs(10)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!is_orphan_serving(&base), "live backend must not orphan");
}
