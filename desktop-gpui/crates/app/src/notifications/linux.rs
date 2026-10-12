use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use parking_lot::Mutex;
use zbus::blocking::Connection;
use zbus::zvariant::Value;

use super::Notice;

// GNOME removes an application's notification source when this bus sender
// disconnects. Share one connection until the process exits, including after
// Notify returns and after individual notifications are dismissed.
static CONNECTION: LazyLock<Mutex<Option<Connection>>> = LazyLock::new(|| Mutex::new(None));

pub(super) fn show(notice: &Notice, avatar: Option<&Path>) -> Result<u32, zbus::Error> {
    let mut connection = CONNECTION.lock();
    if connection.is_none() {
        *connection = Some(Connection::session()?);
    }
    let icon = avatar.and_then(Path::to_str).unwrap_or("wa-bot");
    let mut hints = HashMap::from([
        ("desktop-entry", Value::from("wa-bot")),
        ("category", Value::from("im.received")),
        ("urgency", Value::from(1u8)),
        ("sound-name", Value::from("message-new-instant")),
    ]);
    if avatar.is_some() {
        hints.insert("image-path", Value::from(icon));
    }
    let result = connection
        .as_ref()
        .unwrap()
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                "WA Bot",
                0u32,
                icon,
                notice.title.as_str(),
                notice.body.as_str(),
                Vec::<&str>::new(),
                hints,
                -1i32,
            ),
        )
        .and_then(|reply| reply.body().deserialize::<u32>());
    if result.is_err() {
        *connection = None;
    }
    result
}
