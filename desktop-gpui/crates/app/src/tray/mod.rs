//! System tray API with implementation selected for the current platform.

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Open,
    Quit,
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod unsupported;

#[cfg(target_os = "linux")]
pub use linux::spawn;
#[cfg(not(target_os = "linux"))]
pub use unsupported::spawn;
