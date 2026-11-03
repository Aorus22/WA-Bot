//! System tray API with implementation selected for the current platform.

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Open,
    Quit,
    /// The tray icon is now showing (`true`) or the desktop's tray host went away
    /// or has not started yet (`false`). Sent as it changes, so an app that
    /// autostarts before the host is up learns when the icon appears.
    Available(bool),
}

/// Whether closing the last window must leave the app running: always for the
/// resident popover, otherwise only while the tray icon is there to bring it back.
pub fn keep_running_without_window(popover_mode: bool, tray_up: bool) -> bool {
    popover_mode || tray_up
}

#[cfg(test)]
mod tests {
    use super::keep_running_without_window as keep;

    #[test]
    fn the_app_only_hides_into_a_tray_that_exists() {
        assert!(keep(true, false), "the popover is always resident");
        assert!(keep(false, true), "a normal launch hides while the tray icon shows");
        assert!(!keep(false, false), "no tray: closing the last window quits, nothing could bring it back");
    }
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod unsupported;

#[cfg(target_os = "linux")]
pub use linux::spawn;
#[cfg(not(target_os = "linux"))]
pub use unsupported::spawn;
