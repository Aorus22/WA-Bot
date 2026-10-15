use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;
use std::time::Duration;

use parking_lot::Mutex;
use zbus::blocking::Connection;
use zbus::zvariant::Value;

use super::Notice;

// GNOME removes an application's notification source when this bus sender
// disconnects. Share one connection until the process exits, including after
// Notify returns and after individual notifications are dismissed.
static CONNECTION: LazyLock<Mutex<Option<Connection>>> = LazyLock::new(|| Mutex::new(None));

// How long the popup stays before hiding itself. GNOME Shell ignores the
// timeout an application asks for (and keeps a banner that arrived while the
// user was idle until they move), so the popup is also closed explicitly.
const EXPIRE_MS: i32 = 6_000;

// The id the daemon gave the last notification of each chat, with a generation
// that tells a later message from the one a closing timer was started for.
// Passing the id back as `replaces_id` makes the next message of that chat
// overwrite it instead of stacking; a dismissed one is shown as a new one.
static LAST_ID: LazyLock<Mutex<HashMap<String, (u32, u64)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static GENERATION: AtomicU64 = AtomicU64::new(0);

pub(super) fn show(notice: &Notice, avatar: Option<&Path>, chat_id: &str) -> Result<u32, zbus::Error> {
    let replaces_id = LAST_ID.lock().get(chat_id).map_or(0, |(id, _)| *id);
    let id = notify(notice, avatar, replaces_id)?;
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed);
    LAST_ID.lock().insert(chat_id.to_owned(), (id, generation));

    let chat_id = chat_id.to_owned();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(EXPIRE_MS as u64));
        // A newer message of this chat took over the popup and has its own timer.
        let mut last = LAST_ID.lock();
        if last.get(&chat_id) == Some(&(id, generation)) {
            last.remove(&chat_id);
            drop(last);
            if let Err(error) = close(id) {
                eprintln!("[notifications] could not close notification {id}: {error}");
            }
        }
    });
    Ok(id)
}

fn close(id: u32) -> Result<(), zbus::Error> {
    let connection = CONNECTION.lock();
    let Some(connection) = connection.as_ref() else {
        return Ok(());
    };
    connection.call_method(
        Some("org.freedesktop.Notifications"),
        "/org/freedesktop/Notifications",
        Some("org.freedesktop.Notifications"),
        "CloseNotification",
        &(id,),
    )?;
    Ok(())
}

fn notify(notice: &Notice, avatar: Option<&Path>, replaces_id: u32) -> Result<u32, zbus::Error> {
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
                replaces_id,
                icon,
                notice.title.as_str(),
                notice.body.as_str(),
                Vec::<&str>::new(),
                hints,
                EXPIRE_MS,
            ),
        )
        .and_then(|reply| reply.body().deserialize::<u32>());
    if result.is_err() {
        *connection = None;
    }
    result
}
