use super::Command;

/// Tray integration is unavailable on this platform.
pub fn spawn() -> Option<flume::Receiver<Command>> {
    None
}
