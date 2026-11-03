//! System-tray icon: shows that the app keeps running in the background after
//! its window is closed, with a right-click menu to open the app or quit.
//!
//! Speaks StatusNotifierItem over D-Bus (what KDE, COSMIC, and GNOME with the
//! AppIndicator extension host). Menu clicks arrive on the tray's own thread
//! and are forwarded to the GPUI thread through a channel.

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::StandardItem;
use ksni::{Icon, MenuItem, OfflineReason, ToolTip, Tray};

use crate::tray::Command;

const ICON_PNG: &[u8] = include_bytes!("../../../../dist/icons/wa-bot-128.png");

struct WaBotTray {
    tx: flume::Sender<Command>,
}

impl Tray for WaBotTray {
    fn id(&self) -> String {
        "wa-bot".into()
    }

    fn title(&self) -> String {
        "WA Bot".into()
    }

    fn icon_name(&self) -> String {
        // Resolved from the installed hicolor theme; the pixmap below covers
        // runs where the icon is not installed (a bare AppImage, `cargo run`).
        "wa-bot".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        icon_pixmap().into_iter().collect()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "WA Bot".into(),
            description: "Running in the background".into(),
            ..Default::default()
        }
    }

    /// Left click.
    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(Command::Open);
    }

    /// The desktop's tray host appeared: the icon is showing now.
    fn watcher_online(&self) {
        let _ = self.tx.send(Command::Available(true));
    }

    /// The tray host is not there (yet). Keep waiting for it: the app autostarts at
    /// login a few seconds before GNOME Shell's AppIndicator extension is up.
    fn watcher_offline(&self, reason: OfflineReason) -> bool {
        eprintln!("[tray] waiting for the tray host: {reason:?}");
        let _ = self.tx.send(Command::Available(false));
        true
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Open app".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(Command::Open);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(Command::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// The bundled icon as the ARGB32 (network byte order) pixmap SNI expects.
fn icon_pixmap() -> Option<Icon> {
    let rgba = image::load_from_memory_with_format(ICON_PNG, image::ImageFormat::Png)
        .ok()?
        .into_rgba8();
    let (width, height) = rgba.dimensions();
    let data = rgba
        .pixels()
        .flat_map(|p| [p[3], p[0], p[1], p[2]])
        .collect();
    Some(Icon {
        width: width as i32,
        height: height as i32,
        data,
    })
}

/// Put the icon in the tray. `None` only when D-Bus itself is unusable.
///
/// A missing tray host is not an error: it is waited for, and the icon appears as
/// soon as the host does (`Command::Available` says when). Failing at once, as this
/// used to, left an app that autostarted at login with no tray for its whole life.
/// The caller must not hide the app until `Available(true)`, since nothing else
/// could bring it back.
pub fn spawn() -> Option<flume::Receiver<Command>> {
    let (tx, rx) = flume::unbounded();
    let tray = WaBotTray { tx };
    match tray.assume_sni_available(true).spawn() {
        Ok(handle) => {
            // The icon lives as long as the process.
            let handle: Handle<WaBotTray> = handle;
            std::mem::forget(handle);
            Some(rx)
        }
        Err(e) => {
            eprintln!("[tray] unavailable: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_host_is_waited_for_and_reported() {
        let (tx, rx) = flume::unbounded();
        let tray = WaBotTray { tx };
        assert!(tray.watcher_offline(OfflineReason::No), "keep the service alive until the host comes");
        assert_eq!(rx.try_recv(), Ok(Command::Available(false)));
        tray.watcher_online();
        assert_eq!(rx.try_recv(), Ok(Command::Available(true)));
    }

    /// End to end, on a private session bus (never run by default):
    /// the harness starts a stand-in tray host a few seconds after this test begins.
    ///
    /// `dbus-run-session -- bash -c '(sleep 4; python3 fakewatcher.py) & cargo test -p wabot
    /// registers_when_the_host_appears_late -- --ignored --nocapture'`
    #[test]
    #[ignore]
    fn registers_when_the_host_appears_late() {
        use std::time::Duration;
        let rx = spawn().expect("D-Bus is usable");
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(3)),
            Ok(Command::Available(false)),
            "no host yet: reported, and the app must not hide into a tray that is not there"
        );
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(30)),
            Ok(Command::Available(true)),
            "the icon registers by itself once the host appears"
        );
    }

    #[test]
    fn pixmap_is_argb_of_the_bundled_icon() {
        let icon = icon_pixmap().expect("bundled icon decodes");
        assert!(icon.width > 0 && icon.height > 0);
        assert_eq!(icon.data.len(), (icon.width * icon.height * 4) as usize);
    }
}
