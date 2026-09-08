//! WA Bot backend supervisor — spawn, port handshake, readiness, lifecycle.
//!
//! Framework-free (tokio only, no GPUI) so the spawn/handshake/readiness/kill
//! logic is testable headless in CI.
//!
//! Exact port of the sidecar contract in `desktop/main.js`:
//! - spawn `wa-bot-backend` with `PORT=:0` (ephemeral), `ALLOWED_ORIGINS=*`,
//!   `DB_PATH`, `MEDIA_PATH`, `TZ=Asia/Jakarta` via **env only** (never argv)
//! - parse `BACKEND_PORT:<port>` from child stdout
//! - readiness probe `GET {base}/api/settings`
//! - kill cleanly on shutdown so no orphan backend survives
//!
//! Anti-pattern note (ARCHITECTURE.md): wa-bot has **no encryption key** —
//! unlike web-term's `WEBTERM_ENCRYPTION_KEY`, nothing secret is passed here.
//! `DB_PATH`/`MEDIA_PATH` are plain filesystem paths, not secrets, but they
//! still travel via env (never argv) to keep the process listing clean.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::io::AsyncBufReadExt;
use tokio::process::Child;

/// Maximum characters stored in the stderr ring buffer (newest-wins).
pub const MAX_STDERR_TAIL_CHARS: usize = 2000;

/// Default external-backend port, mirroring `DEFAULT_EXTERNAL_BACKEND_PORT` in main.js.
pub const DEFAULT_EXTERNAL_BACKEND_PORT: u16 = 8080;

/// Executable suffix for the backend binary on this platform.
#[cfg(windows)]
pub const BACKEND_EXE_SUFFIX: &str = ".exe";
#[cfg(not(windows))]
pub const BACKEND_EXE_SUFFIX: &str = "";

/// Lifecycle status of the managed backend process.
#[derive(Debug, Clone, PartialEq)]
pub enum BackendStatus {
    /// Process launched; waiting for the `BACKEND_PORT` handshake.
    Starting,
    /// Handshake received and readiness probe passing; backend serving.
    Ready,
    /// Startup failed before readiness (invalid path, early exit, probe timeout).
    /// `stderr_tail` is the newest-wins tail of child stderr, capped at 2000 chars.
    Failed {
        reason: String,
        stderr_tail: String,
    },
    /// Backend exited unexpectedly after readiness.
    Crashed { exit_code: Option<i32> },
}

/// Errors originating from the supervisor.
#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    #[error("startup failed: {reason} (stderr: {stderr_tail})")]
    Failed {
        reason: String,
        stderr_tail: String,
    },
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("invalid CLI port: {0}")]
    InvalidCliPort(String),
    #[error("process IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Options controlling backend spawn.
#[derive(Debug, Clone)]
pub struct SpawnOptions {
    /// Path to the backend executable (bundled binary, or a dev override).
    pub backend_path: PathBuf,
    /// Absolute database directory path (paritas main.js `userData/database`).
    pub db_path: PathBuf,
    /// Absolute media directory path (paritas main.js `userData/media`).
    pub media_path: PathBuf,
    /// Value for ALLOWED_ORIGINS (desktop default: "*").
    pub allowed_origins: String,
    /// Extra argv for the child (non-secret only; secrets travel via env).
    /// Used by tests to drive `sh -c` / python fake backends; the real
    /// backend is spawned with no arguments, exactly like main.js.
    pub args: Vec<String>,
    /// Working directory for the child. The Go backend resolves its SQLite
    /// files as `database/*.db` **relative to cwd** (it ignores DB_PATH for
    /// opening), so real spawns must set this to the userData-style base dir
    /// that contains `database/` and `media/` — mirroring main.js
    /// `cwd: userDataPath` and desktop-gtk `cmd.Dir = m.userData`.
    /// Fake backends ignore it. `None` inherits the parent cwd.
    pub work_dir: Option<PathBuf>,
    /// Max time to wait for readiness after the handshake (default 15s).
    pub readiness_timeout: Duration,
    /// Max time to wait for the BACKEND_PORT handshake (default 15s).
    pub handshake_timeout: Duration,
}

impl SpawnOptions {
    /// Create spawn options. `db_path` and `media_path` must be absolute so
    /// the child behaves identically regardless of the parent cwd.
    pub fn new(
        backend_path: impl Into<PathBuf>,
        db_path: impl Into<PathBuf>,
        media_path: impl Into<PathBuf>,
    ) -> Result<Self, SupervisorError> {
        let db_path = db_path.into();
        let media_path = media_path.into();
        if !db_path.is_absolute() {
            return Err(SupervisorError::InvalidPath(format!(
                "db_path must be absolute, got '{}'",
                db_path.display()
            )));
        }
        if !media_path.is_absolute() {
            return Err(SupervisorError::InvalidPath(format!(
                "media_path must be absolute, got '{}'",
                media_path.display()
            )));
        }
        Ok(Self {
            backend_path: backend_path.into(),
            db_path,
            media_path,
            allowed_origins: "*".to_string(),
            args: Vec::new(),
            work_dir: None,
            readiness_timeout: Duration::from_secs(15),
            handshake_timeout: Duration::from_secs(15),
        })
    }

    pub fn with_allowed_origins(mut self, origins: impl Into<String>) -> Self {
        self.allowed_origins = origins.into();
        self
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_work_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.work_dir = Some(dir.into());
        self
    }

    pub fn with_readiness_timeout(mut self, timeout: Duration) -> Self {
        self.readiness_timeout = timeout;
        self
    }

    pub fn with_handshake_timeout(mut self, timeout: Duration) -> Self {
        self.handshake_timeout = timeout;
        self
    }

    /// Return the environment variable pairs injected into the child process.
    ///
    /// Exactly the five pairs from `desktop/main.js`; explicitly **no**
    /// encryption key (wa-bot has none — see module docs).
    pub fn env_vars(&self) -> Vec<(&'static str, String)> {
        vec![
            ("PORT", ":0".to_string()),
            ("ALLOWED_ORIGINS", self.allowed_origins.clone()),
            ("DB_PATH", self.db_path.to_string_lossy().to_string()),
            ("MEDIA_PATH", self.media_path.to_string_lossy().to_string()),
            ("TZ", "Asia/Jakarta".to_string()),
        ]
    }

    /// Construct the child Command ensuring paths travel via env (never argv).
    pub fn build_command(&self) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new(&self.backend_path);
        cmd.args(&self.args);
        if let Some(dir) = &self.work_dir {
            cmd.current_dir(dir);
        }
        for (k, v) in self.env_vars() {
            cmd.env(k, v);
        }
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.kill_on_drop(true);
        cmd
    }
}

/// Handle information for a running backend child process.
#[derive(Debug, Clone, PartialEq)]
pub struct BackendInfo {
    /// Loopback base URL derived from the handshake, e.g. "http://127.0.0.1:51234".
    pub base_url: String,
    pub pid: u32,
    pub port: u16,
}

/// Parse the first occurrence of `BACKEND_PORT:<port>` from a line.
///
/// Tolerant of surrounding log noise (prefix/suffix); requires at least one
/// ASCII digit after the marker and a valid u16 value.
pub fn parse_handshake_line(line: &str) -> Option<u16> {
    let idx = line.find("BACKEND_PORT:")?;
    let rest = &line[idx + "BACKEND_PORT:".len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u16>().ok()
}

// ---------------------------------------------------------------------------
// CLI parity (desktop/main.js): --no-backend --port X
// ---------------------------------------------------------------------------

/// External backend selected via `--no-backend --port <port>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalBackend {
    pub port: u16,
}

/// Mirror of main.js `hasCliFlag`: matches `--name` or `-name` exactly.
pub fn has_cli_flag(args: &[String], name: &str) -> bool {
    let long = format!("--{name}");
    let short = format!("-{name}");
    args.iter().any(|a| a == &long || a == &short)
}

/// Mirror of main.js `getCliValue`: supports `--name=value`, `-name=value`,
/// and `--name value` forms. Returns `None` when the flag is absent.
pub fn get_cli_value(args: &[String], name: &str) -> Option<String> {
    let long = format!("--{name}");
    let short = format!("-{name}");
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if let Some(v) = arg.strip_prefix(&format!("{long}=")) {
            return Some(v.to_string());
        }
        if let Some(v) = arg.strip_prefix(&format!("{short}=")) {
            return Some(v.to_string());
        }
        if arg == &long || arg == &short {
            return Some(iter.next().cloned().unwrap_or_default());
        }
    }
    None
}

/// Mirror of main.js `parseBackendPort`:
/// - `None` (flag absent) → default 8080
/// - digits within 1-65535 → that port
/// - non-digit or out-of-range → `Err` (never a silent default).
pub fn parse_backend_port(value: Option<&str>) -> Result<u16, SupervisorError> {
    match value {
        None => Ok(DEFAULT_EXTERNAL_BACKEND_PORT),
        Some(v) => {
            let digits_only = !v.is_empty() && v.chars().all(|c| c.is_ascii_digit());
            if !digits_only {
                return Err(SupervisorError::InvalidCliPort(format!(
                    "port must be digits 1-65535, got '{v}'"
                )));
            }
            match v.parse::<u32>() {
                Ok(p) if (1..=65535).contains(&p) => Ok(p as u16),
                _ => Err(SupervisorError::InvalidCliPort(format!(
                    "port out of range 1-65535, got '{v}'"
                ))),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Path resolution parity (desktop/main.js `startBackend`)
// ---------------------------------------------------------------------------

/// Packaged-layout candidate: `<resources>/be/wa-bot-backend[.exe]`.
/// Mirrors `path.join(process.resourcesPath, 'be', 'wa-bot-backend')`.
pub fn packaged_backend_path(resources_dir: &Path) -> PathBuf {
    resources_dir
        .join("be")
        .join(format!("wa-bot-backend{}", BACKEND_EXE_SUFFIX))
}

/// Dev-layout candidate: `<repo>/wa-bot-backend[.exe]`.
/// Mirrors `path.join(__dirname, '..', 'wa-bot-backend')` (repo root).
pub fn dev_backend_path(repo_root: &Path) -> PathBuf {
    repo_root.join(format!("wa-bot-backend{}", BACKEND_EXE_SUFFIX))
}

/// Two-level lookup with Electron parity: packaged first, dev fallback.
/// No other path guesses — exactly two levels.
pub fn resolve_backend_path(resources_dir: &Path, repo_root: &Path) -> PathBuf {
    let packaged = packaged_backend_path(resources_dir);
    if packaged.exists() {
        packaged
    } else {
        dev_backend_path(repo_root)
    }
}

/// Create `database/` and `media/` under `base` (paritas main.js userData
/// layout). Idempotent: succeeds when the directories already exist.
/// Returns `(db_dir, media_dir)`.
pub fn ensure_data_dirs(base: &Path) -> std::io::Result<(PathBuf, PathBuf)> {
    let db_dir = base.join("database");
    let media_dir = base.join("media");
    std::fs::create_dir_all(&db_dir)?;
    std::fs::create_dir_all(&media_dir)?;
    Ok((db_dir, media_dir))
}

// ---------------------------------------------------------------------------
// Supervisor
// ---------------------------------------------------------------------------

/// Process supervisor managing the Go backend child process lifecycle.
///
/// Startup kill policy (documented here for Phase 3 boot):
/// - Before spawning, the caller should `adopt_or_clear(last_backend_url)`:
///   if a previous backend still serves, reuse it instead of spawning.
/// - The supervisor only ever kills the child it spawned itself
///   (`kill_on_drop(true)` + explicit [`Supervisor::stop`]); it never kills
///   foreign PIDs, so a stale backend from a crashed session is adopted when
///   reachable and simply left alone when it is not (its port was ephemeral,
///   so a fresh spawn can never collide with it).
pub struct Supervisor {
    status_tx: tokio::sync::watch::Sender<BackendStatus>,
    status_rx: tokio::sync::watch::Receiver<BackendStatus>,
    info: Option<BackendInfo>,
    child: Arc<Mutex<Option<Child>>>,
    stderr_tail: Arc<Mutex<String>>,
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl Supervisor {
    /// Create a new Supervisor instance in Starting status.
    pub fn new() -> Self {
        let (status_tx, status_rx) = tokio::sync::watch::channel(BackendStatus::Starting);
        Self {
            status_tx,
            status_rx,
            info: None,
            child: Arc::new(Mutex::new(None)),
            stderr_tail: Arc::new(Mutex::new(String::new())),
        }
    }

    /// Read current status.
    pub fn status(&self) -> BackendStatus {
        self.status_rx.borrow().clone()
    }

    /// Subscribe to status change notifications.
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<BackendStatus> {
        self.status_rx.clone()
    }

    /// Return backend info if currently ready/running.
    pub fn info(&self) -> Option<&BackendInfo> {
        self.info.as_ref()
    }

    /// Return snapshot of newest stderr tail.
    pub fn stderr_tail(&self) -> String {
        self.stderr_tail.lock().clone()
    }

    /// Poll `GET {base_url}/api/settings` until success (any 2xx-4xx) or timeout.
    /// Used internally after the handshake and by Phase 3 for `--no-backend` mode.
    pub async fn wait_ready(
        base_url: &str,
        timeout: Duration,
    ) -> Result<(), SupervisorError> {
        let probe_url = format!("{}/api/settings", base_url.trim_end_matches('/'));
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(1000))
            .build()
            .unwrap_or_default();
        let start = tokio::time::Instant::now();
        loop {
            if let Ok(resp) = client.get(&probe_url).send().await {
                let code = resp.status().as_u16();
                if code < 500 {
                    return Ok(());
                }
            }
            if start.elapsed() > timeout {
                return Err(SupervisorError::Failed {
                    reason: format!("Readiness probe timed out after {timeout:?}"),
                    stderr_tail: String::new(),
                });
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    fn fail(&self, reason: String) -> SupervisorError {
        let stderr_tail = self.stderr_tail();
        let status = BackendStatus::Failed {
            reason: reason.clone(),
            stderr_tail: stderr_tail.clone(),
        };
        let _ = self.status_tx.send(status);
        SupervisorError::Failed {
            reason,
            stderr_tail,
        }
    }

    /// Spawn the backend process, perform port handshake and readiness polling.
    pub async fn spawn(&mut self, opts: SpawnOptions) -> Result<BackendInfo, SupervisorError> {
        let _ = self.status_tx.send(BackendStatus::Starting);
        self.stderr_tail.lock().clear();

        let mut cmd = opts.build_command();
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let reason = format!(
                    "Failed to spawn backend binary '{}': {e}",
                    opts.backend_path.display()
                );
                // No child exists to produce stderr — record the OS error as
                // the tail so Failed always carries diagnostic content.
                *self.stderr_tail.lock() = format!("Execution error: {e}");
                return Err(self.fail(reason));
            }
        };

        let pid = child.id().unwrap_or(0);
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // Spawn stderr collector into ring buffer (log metadata only —
        // never env secrets; see T-01-02c).
        let stderr_tail = self.stderr_tail.clone();
        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let mut reader = tokio::io::BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut lock = stderr_tail.lock();
                    lock.push_str(&line);
                    lock.push('\n');
                    let len = lock.chars().count();
                    if len > MAX_STDERR_TAIL_CHARS {
                        let to_drop = len - MAX_STDERR_TAIL_CHARS;
                        *lock = lock.chars().skip(to_drop).collect();
                    }
                }
            });
        }

        // Wait for BACKEND_PORT handshake, then keep draining stdout for the
        // child's whole lifetime: dropping the read end would SIGPIPE-kill
        // the Go backend on its next log write.
        let mut port_found: Option<u16> = None;
        if let Some(stdout) = stdout {
            let mut reader = tokio::io::BufReader::new(stdout).lines();
            let handshake_fut = async {
                while let Ok(Some(line)) = reader.next_line().await {
                    if let Some(port) = parse_handshake_line(&line) {
                        return Ok((port, reader));
                    }
                }
                Err("Stdout closed without BACKEND_PORT handshake")
            };

            match tokio::time::timeout(opts.handshake_timeout, handshake_fut).await {
                Ok(Ok((port, reader))) => {
                    port_found = Some(port);
                    tokio::spawn(async move {
                        let mut reader = reader;
                        while let Ok(Some(_)) = reader.next_line().await {}
                    });
                }
                Ok(Err(err_msg)) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    return Err(self.fail(format!("Backend handshake missing: {err_msg}")));
                }
                Err(_) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    return Err(self.fail(format!(
                        "Backend handshake timed out after {:?}",
                        opts.handshake_timeout
                    )));
                }
            }
        }

        let port = match port_found {
            Some(p) => p,
            None => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(self.fail("No stdout available to read handshake".to_string()));
            }
        };

        let base_url = format!("http://127.0.0.1:{port}");
        let info = BackendInfo {
            base_url: base_url.clone(),
            pid,
            port,
        };

        // Readiness probe with early-exit detection.
        let probe_url = format!("{base_url}/api/settings");
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(1000))
            .build()
            .unwrap_or_default();
        let start = tokio::time::Instant::now();
        loop {
            if let Ok(Some(exit_status)) = child.try_wait() {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(self.fail(format!(
                    "Backend child exited before readiness with status: {exit_status}"
                )));
            }
            if let Ok(resp) = client.get(&probe_url).send().await {
                if resp.status().as_u16() < 500 {
                    break;
                }
            }
            if start.elapsed() > opts.readiness_timeout {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(self.fail(format!(
                    "Readiness probe timed out after {:?}",
                    opts.readiness_timeout
                )));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        // Ready!
        let _ = self.status_tx.send(BackendStatus::Ready);
        self.info = Some(info.clone());
        *self.child.lock() = Some(child);

        // Spawn child exit monitor.
        let child_arc = Arc::clone(&self.child);
        let status_tx = self.status_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let mut lock = child_arc.lock();
                if let Some(child) = lock.as_mut() {
                    match child.try_wait() {
                        Ok(Some(exit_status)) => {
                            let code = exit_status.code();
                            let _ = status_tx.send(BackendStatus::Crashed { exit_code: code });
                            break;
                        }
                        Ok(None) => continue,
                        Err(_) => {
                            let _ = status_tx.send(BackendStatus::Crashed { exit_code: None });
                            break;
                        }
                    }
                } else {
                    // Stopped cleanly.
                    break;
                }
            }
        });

        Ok(info)
    }

    /// Return child PID if currently spawned.
    pub fn child_pid(&self) -> Option<u32> {
        self.child.lock().as_ref().and_then(|c| c.id())
    }

    /// Hard-kill the child without graceful shutdown (crash simulation /
    /// restart-loop recovery). Reaps the process so no zombie remains.
    pub async fn kill_force(&mut self) -> Result<(), SupervisorError> {
        let child_opt = self.child.lock().take();
        if let Some(mut child) = child_opt {
            // Child::kill is SIGKILL on unix; TerminateProcess on Windows.
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        self.info = None;
        let _ = self
            .status_tx
            .send(BackendStatus::Crashed { exit_code: None });
        Ok(())
    }

    /// Stop the backend process: graceful terminate with grace duration, then hard kill.
    pub async fn stop(&mut self, grace: Duration) -> Result<(), SupervisorError> {
        let child_opt = self.child.lock().take();
        if let Some(mut child) = child_opt {
            #[cfg(unix)]
            {
                if let Some(pid) = child.id() {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGTERM);
                    }
                }
            }

            let wait_fut = child.wait();
            match tokio::time::timeout(grace, wait_fut).await {
                Ok(Ok(_)) => {}
                _ => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                }
            }
        }
        self.info = None;
        Ok(())
    }

    /// Check if a previously recorded backend is still serving, or clear it.
    pub async fn adopt_or_clear(base_url: Option<String>) -> Option<BackendInfo> {
        let base_url = base_url?;
        let probe_url = format!("{}/api/settings", base_url.trim_end_matches('/'));
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(1))
            .build()
            .ok()?;

        if let Ok(resp) = client.get(&probe_url).send().await {
            if resp.status().is_success() {
                let port = probe_url
                    .split(':')
                    .next_back()?
                    .split('/')
                    .next()?
                    .parse::<u16>()
                    .unwrap_or(0);
                return Some(BackendInfo {
                    base_url,
                    pid: 0,
                    port,
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_parses_with_noise() {
        assert_eq!(parse_handshake_line("BACKEND_PORT:54321"), Some(54321));
        assert_eq!(
            parse_handshake_line("2026/09/08 Starting... BACKEND_PORT:8080 (ready)"),
            Some(8080)
        );
        assert_eq!(parse_handshake_line("BACKEND_PORT:0"), Some(0));
        assert_eq!(parse_handshake_line("BACKEND_PORT:notaport"), None);
        assert_eq!(parse_handshake_line("BACKEND_PORT:"), None);
        assert_eq!(parse_handshake_line("just a regular log line"), None);
        // First occurrence wins.
        assert_eq!(
            parse_handshake_line("BACKEND_PORT:1111 BACKEND_PORT:2222"),
            Some(1111)
        );
    }

    #[test]
    fn env_vars_are_exactly_main_js_five_without_secrets() {
        let base = PathBuf::from("/tmp/wabot-test");
        let opts = SpawnOptions::new(
            "/opt/wa-bot-backend",
            base.join("database"),
            base.join("media"),
        )
        .unwrap();
        let env = opts.env_vars();
        assert_eq!(env.len(), 5);
        let get = |k: &str| env.iter().find(|(key, _)| *key == k).map(|(_, v)| v.clone());
        assert_eq!(get("PORT").as_deref(), Some(":0"));
        assert_eq!(get("ALLOWED_ORIGINS").as_deref(), Some("*"));
        assert_eq!(
            get("DB_PATH").as_deref(),
            Some("/tmp/wabot-test/database")
        );
        assert_eq!(
            get("MEDIA_PATH").as_deref(),
            Some("/tmp/wabot-test/media")
        );
        assert_eq!(get("TZ").as_deref(), Some("Asia/Jakarta"));
        // No encryption-key material anywhere (wa-bot has none).
        for (k, v) in &env {
            assert!(!k.to_uppercase().contains("KEY"), "unexpected key-like env {k}");
            assert!(!v.to_uppercase().contains("ENCRYPT"), "unexpected secret {k}");
        }
        let cmd = opts.build_command();
        assert!(cmd.as_std().get_args().next().is_none());
        assert!(cmd.as_std().get_current_dir().is_none());
        let opts_wd = opts.clone().with_work_dir("/tmp/wabot-test");
        assert_eq!(
            opts_wd.build_command().as_std().get_current_dir(),
            Some(std::path::Path::new("/tmp/wabot-test"))
        );
    }

    #[test]
    fn spawn_options_rejects_relative_data_paths() {
        assert!(SpawnOptions::new("be", "relative/db", PathBuf::from("/abs/media")).is_err());
        assert!(SpawnOptions::new("be", PathBuf::from("/abs/db"), "relative/media").is_err());
    }

    #[test]
    fn cli_parity_with_main_js() {
        let args = vec!["--no-backend".to_string(), "--port".to_string(), "3090".to_string()];
        assert!(has_cli_flag(&args, "no-backend"));
        assert!(has_cli_flag(&args, "port"));
        assert_eq!(get_cli_value(&args, "port").as_deref(), Some("3090"));

        let eq = vec!["--port=4040".to_string()];
        assert_eq!(get_cli_value(&eq, "port").as_deref(), Some("4040"));
        let short = vec!["-port".to_string(), "5050".to_string()];
        assert!(has_cli_flag(&short, "port"));

        assert_eq!(parse_backend_port(None).unwrap(), 8080);
        assert_eq!(parse_backend_port(Some("3090")).unwrap(), 3090);
        assert_eq!(parse_backend_port(Some("1")).unwrap(), 1);
        assert_eq!(parse_backend_port(Some("65535")).unwrap(), 65535);
        assert!(parse_backend_port(Some("abc")).is_err());
        assert!(parse_backend_port(Some("0")).is_err());
        assert!(parse_backend_port(Some("65536")).is_err());
        assert!(parse_backend_port(Some("")).is_err());
        assert!(parse_backend_port(Some("12x34")).is_err());
    }

    #[test]
    fn backend_path_resolution_two_levels() {
        let tmp = tempfile::tempdir().unwrap();
        let resources = tmp.path().join("resources");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(resources.join("be")).unwrap();
        std::fs::create_dir_all(&repo).unwrap();
        // Nothing exists → dev fallback.
        assert_eq!(resolve_backend_path(&resources, &repo), dev_backend_path(&repo));
        // Packaged binary wins when present.
        std::fs::write(packaged_backend_path(&resources), b"x").unwrap();
        assert_eq!(
            resolve_backend_path(&resources, &repo),
            packaged_backend_path(&resources)
        );
    }
}
