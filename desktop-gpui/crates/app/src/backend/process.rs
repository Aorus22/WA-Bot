//! Stopping a backend this process did not start.
//!
//! A backend can outlive the app that launched it (the resident daemon, or a
//! previous session) and is adopted by whoever starts next. Restarting it then
//! means finding the process that listens on its port and stopping it from here.
//! Only a process that is named like the WA Bot backend is ever touched.

use std::process::Command;
use std::time::Duration;

/// Executable name of the backend (without `.exe`).
pub const BACKEND_PROCESS_NAME: &str = "wa-bot-backend";

/// Inode of the socket listening on `port`, from the text of `/proc/net/tcp`
/// or `/proc/net/tcp6`.
pub fn listening_inode(table: &str, port: u16) -> Option<u64> {
    const LISTEN: &str = "0A";
    table.lines().skip(1).find_map(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let local_port = u16::from_str_radix(fields.get(1)?.rsplit(':').next()?, 16).ok()?;
        (local_port == port && *fields.get(3)? == LISTEN)
            .then(|| fields.get(9)?.parse().ok())
            .flatten()
    })
}

/// The process listening on loopback `port`, if one can be found.
pub fn pid_listening_on(port: u16) -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        let inode = ["/proc/net/tcp", "/proc/net/tcp6"]
            .iter()
            .filter_map(|path| std::fs::read_to_string(path).ok())
            .find_map(|table| listening_inode(&table, port))?;
        let needle = format!("socket:[{inode}]");
        std::fs::read_dir("/proc").ok()?.flatten().find_map(|entry| {
            let pid: u32 = entry.file_name().to_str()?.parse().ok()?;
            std::fs::read_dir(entry.path().join("fd"))
                .ok()?
                .flatten()
                .any(|fd| std::fs::read_link(fd.path()).is_ok_and(|target| target.to_string_lossy() == needle))
                .then_some(pid)
        })
    }
    #[cfg(target_os = "macos")]
    {
        let out = Command::new("lsof")
            .args(["-nP", "-t", &format!("-iTCP:{port}"), "-sTCP:LISTEN"])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).lines().next()?.trim().parse().ok()
    }
    #[cfg(windows)]
    {
        let out = Command::new("netstat").args(["-ano", "-p", "tcp"]).output().ok()?;
        String::from_utf8_lossy(&out.stdout).lines().find_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let local = fields.get(1)?;
            (fields.get(3) == Some(&"LISTENING") && local.rsplit(':').next()? == port.to_string())
                .then(|| fields.get(4)?.parse().ok())
                .flatten()
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = port;
        None
    }
}

/// Name of the executable behind `pid`, lower-cased and without a `.exe` suffix.
pub fn process_name(pid: u32) -> Option<String> {
    #[cfg(target_os = "linux")]
    let name = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    #[cfg(target_os = "macos")]
    let name = {
        let out = Command::new("ps").args(["-p", &pid.to_string(), "-o", "comm="]).output().ok()?;
        String::from_utf8_lossy(&out.stdout).trim().rsplit('/').next()?.to_string()
    };
    #[cfg(windows)]
    let name = {
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).split(',').next()?.trim_matches('"').to_string()
    };
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    let name = {
        let _ = pid;
        return None;
    };
    Some(name.trim().trim_end_matches(".exe").to_lowercase())
}

/// Pids of running backends whose working directory is `dir` — the data folder
/// the app starts its backend in. Only Linux can tell (via `/proc`); elsewhere
/// the answer is always empty.
pub fn backend_pids_using(dir: &std::path::Path) -> Vec<u32> {
    #[cfg(target_os = "linux")]
    {
        let Ok(dir) = dir.canonicalize() else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|entry| {
                let pid: u32 = entry.file_name().to_str()?.parse().ok()?;
                (process_name(pid)? == BACKEND_PROCESS_NAME).then_some(())?;
                let cwd = std::fs::read_link(entry.path().join("cwd")).ok()?;
                (cwd == dir).then_some(pid)
            })
            .collect()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = dir;
        Vec::new()
    }
}

fn signal(pid: u32, force: bool) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .arg(if force { "-KILL" } else { "-TERM" })
            .arg(pid.to_string())
            .status();
    }
    #[cfg(windows)]
    {
        let mut command = Command::new("taskkill");
        command.args(["/PID", &pid.to_string()]);
        if force {
            command.arg("/F");
        }
        let _ = command.status();
    }
}

/// Stop the backend listening on `port`: ask it to terminate, and force it if
/// it is still there after `grace`. `still_up` says whether it still answers.
pub async fn stop_listener(
    port: u16,
    grace: Duration,
    still_up: impl Fn() -> bool + Send + Sync + 'static,
) -> Result<(), String> {
    let still_up = std::sync::Arc::new(still_up);
    let pid = tokio::task::spawn_blocking(move || pid_listening_on(port))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("could not tell which process is listening on port {port}"))?;
    let name = process_name(pid).unwrap_or_default();
    if name != BACKEND_PROCESS_NAME {
        return Err(format!(
            "port {port} is held by '{name}' (pid {pid}), not by {BACKEND_PROCESS_NAME}; leaving it alone"
        ));
    }

    let gone = |within: Duration| {
        let still_up = still_up.clone();
        async move {
            let deadline = std::time::Instant::now() + within;
            while std::time::Instant::now() < deadline {
                let up = still_up.clone();
                if !tokio::task::spawn_blocking(move || up()).await.unwrap_or(true) {
                    return true;
                }
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
            false
        }
    };

    signal(pid, false);
    if gone(grace).await {
        return Ok(());
    }
    signal(pid, true);
    if gone(Duration::from_secs(3)).await {
        Ok(())
    } else {
        Err(format!("pid {pid} would not stop"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
   0: 0100007F:0BB8 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 424242 1 0000000000000000 100 0 0 10 0\n\
   1: 0100007F:0BB9 0100007F:C350 01 00000000:00000000 00:00000000 00000000  1000        0 515151 1 0000000000000000 100 0 0 10 0\n\
   2: 00000000:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 636363 1 0000000000000000 100 0 0 10 0\n";

    #[test]
    fn finds_the_listening_socket_for_a_port() {
        assert_eq!(listening_inode(TABLE, 3000), Some(424242));
        assert_eq!(listening_inode(TABLE, 8080), Some(636363));
    }

    #[test]
    fn ignores_connections_that_are_not_listening() {
        // 0x0BB9 = 3001 is an established connection, not a listener.
        assert_eq!(listening_inode(TABLE, 3001), None);
        assert_eq!(listening_inode(TABLE, 9999), None);
    }

    /// Needs a real backend binary: `WABOT_TEST_BACKEND=/path/wa-bot-backend cargo test -- --ignored`.
    /// Runs it in a scratch directory, then stops it by port.
    #[test]
    #[ignore]
    fn stops_a_real_backend_found_by_its_port() {
        let binary = std::env::var("WABOT_TEST_BACKEND").expect("WABOT_TEST_BACKEND");
        let dir = std::env::temp_dir().join(format!("wabot-stop-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("database")).unwrap();
        std::fs::create_dir_all(dir.join("media")).unwrap();
        let port = 39555u16;
        let mut child = Command::new(binary)
            .current_dir(&dir)
            .env("PORT", format!(":{port}"))
            .env("DB_PATH", dir.join("database"))
            .env("MEDIA_PATH", dir.join("media"))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        for _ in 0..50 {
            if crate::backend::is_wabot_backend(port) {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        assert!(crate::backend::is_wabot_backend(port), "backend did not come up");
        assert_eq!(pid_listening_on(port), Some(child.id()));

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(stop_listener(port, Duration::from_secs(5), move || crate::backend::is_wabot_backend(port)))
            .unwrap();
        assert!(!crate::backend::is_wabot_backend(port));
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(dir);
    }
}
