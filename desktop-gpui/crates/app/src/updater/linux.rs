//! AppImage update installation for Linux.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::{client, find_checksum, InstallKind, UpdateInfo};

pub fn install_kind() -> InstallKind {
    match std::env::var_os("APPIMAGE") {
        Some(p) if !p.is_empty() && PathBuf::from(&p).is_file() => {
            InstallKind::AppImage(PathBuf::from(p))
        }
        _ => InstallKind::Other,
    }
}

/// Download the new AppImage next to the running one, verify its SHA256 and
/// atomically replace it. `progress` receives the bytes downloaded, `total`
/// the size (0 when unknown).
pub fn download_and_install(
    info: &UpdateInfo,
    progress: &AtomicU64,
    total: &AtomicU64,
) -> Result<(), String> {
    let InstallKind::AppImage(target) = install_kind() else {
        return Err("Not running from an AppImage. Download the release instead.".into());
    };
    let (Some(url), Some(sums_url), Some(name)) =
        (&info.appimage_url, &info.sums_url, &info.appimage_name)
    else {
        return Err("This release has no AppImage or SHA256SUMS.txt asset.".into());
    };

    let http = client(Duration::from_secs(600))?;
    let sums = http
        .get(sums_url)
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.text())
        .map_err(|e| format!("Couldn't fetch checksums: {e}"))?;
    let expected =
        find_checksum(&sums, name).ok_or_else(|| format!("No checksum listed for {name}"))?;

    let dir = target.parent().ok_or("Invalid AppImage path")?;
    let tmp = dir.join(format!(
        ".{}.update",
        target.file_name().and_then(|n| n.to_str()).unwrap_or("app")
    ));
    let result = (|| -> Result<(), String> {
        let mut resp = http
            .get(url)
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("Download failed: {e}"))?;
        total.store(resp.content_length().unwrap_or(0), Ordering::Relaxed);
        progress.store(0, Ordering::Relaxed);
        let mut file = fs::File::create(&tmp)
            .map_err(|e| format!("Can't write next to the AppImage ({}): {e}", dir.display()))?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut done = 0u64;
        loop {
            let n = resp
                .read(&mut buf)
                .map_err(|e| format!("Download failed: {e}"))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n])
                .map_err(|e| format!("Write failed: {e}"))?;
            done += n as u64;
            progress.store(done, Ordering::Relaxed);
        }
        file.sync_all().map_err(|e| format!("Write failed: {e}"))?;
        drop(file);
        if hex(&hasher.finalize()) != expected {
            return Err("Checksum mismatch. Download discarded, current version kept.".into());
        }
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("chmod failed: {e}"))?;
        // Atomic on one filesystem; fine even though the old file is running.
        fs::rename(&tmp, &target).map_err(|e| format!("Couldn't replace the AppImage: {e}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Start the (already replaced) AppImage as the resident daemon once this
/// process has exited. The delay lets the single-instance socket and the old
/// backend go first, so the new shell starts its own, updated backend.
pub fn spawn_restart() -> Result<(), String> {
    let InstallKind::AppImage(path) = install_kind() else {
        return Err("Not running from an AppImage".into());
    };
    std::process::Command::new("sh")
        .arg("-c")
        .arg("sleep 3; exec \"$0\" --popover")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Couldn't restart: {e}"))
}
