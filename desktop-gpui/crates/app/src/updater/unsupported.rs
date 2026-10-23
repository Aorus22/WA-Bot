use std::sync::atomic::AtomicU64;

use super::{InstallKind, UpdateInfo};

/// Self replacement is only supported for Linux AppImage builds.
pub fn install_kind() -> InstallKind {
    InstallKind::Other
}

pub fn download_and_install(
    _info: &UpdateInfo,
    _progress: &AtomicU64,
    _total: &AtomicU64,
) -> Result<(), String> {
    Err("Self-installing updates are only supported for Linux AppImages.".into())
}

pub fn spawn_restart() -> Result<(), String> {
    Err("Restarting an updated AppImage is only supported on Linux.".into())
}
