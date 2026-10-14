//! What the "Chat media" section shows: the kinds of media a download can be
//! limited to and the plain-language reading of its progress.

use wabot_backend_client::dto::HistoryMediaStatus;

use super::history_sync::format_duration;

/// A kind of media the user can choose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediaKind {
    /// What the backend calls it.
    pub id: &'static str,
    pub label: &'static str,
    /// Chosen from the start. Videos and documents are big, so they are not.
    pub by_default: bool,
}

pub const MEDIA_KINDS: [MediaKind; 5] = [
    MediaKind { id: "image", label: "Photos", by_default: true },
    MediaKind { id: "sticker", label: "Stickers", by_default: true },
    MediaKind { id: "audio", label: "Audio & voice notes", by_default: true },
    MediaKind { id: "video", label: "Videos", by_default: false },
    MediaKind { id: "document", label: "Documents", by_default: false },
];

pub fn default_kinds() -> Vec<String> {
    MEDIA_KINDS.iter().filter(|k| k.by_default).map(|k| k.id.to_string()).collect()
}

/// True while a download is running or being stopped.
pub fn is_active(state: &str) -> bool {
    matches!(state, "running" | "cancelling")
}

pub fn state_label(state: &str) -> &'static str {
    match state {
        "running" => "Downloading",
        "cancelling" => "Stopping…",
        "completed" => "Done",
        "partial" => "Done, with failures",
        "cancelled" => "Stopped",
        "interrupted" => "Paused",
        "failed" => "Failed",
        _ => "Not run yet",
    }
}

/// "812 B", "34.5 MB", "1.2 GB".
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let units = ["KB", "MB", "GB", "TB"];
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value >= 100.0 {
        format!("{} {}", value.round() as u64, units[unit])
    } else {
        format!("{value:.1} {}", units[unit])
    }
}

/// Shares of the bar (downloaded, gone from the server, failed), each 0..=1.
pub fn progress_shares(st: &HistoryMediaStatus) -> (f32, f32, f32) {
    if st.total == 0 {
        return (0.0, 0.0, 0.0);
    }
    let share = |n: u64| (n as f32 / st.total as f32).clamp(0.0, 1.0);
    (share(st.done), share(st.unavailable), share(st.failed))
}

/// "Downloading · 130 of 9800 files (1%)".
pub fn headline(st: &HistoryMediaStatus) -> String {
    let mut parts = vec![state_label(&st.state).to_string()];
    if st.total > 0 {
        let handled = st.done + st.unavailable + st.failed;
        parts.push(format!("{handled} of {} files ({}%)", st.total, (handled * 100 + st.total / 2) / st.total));
    }
    parts.join(" · ")
}

/// How long it ran and, while running, how long is left.
pub fn timing_text(st: &HistoryMediaStatus) -> Option<String> {
    st.started_at?;
    let active = is_active(&st.state);
    let mut parts = vec![format!(
        "{} {}",
        if active { "running for" } else { "ran for" },
        format_duration(st.elapsed_ms / 1000)
    )];
    if active {
        parts.push(match st.eta_ms {
            Some(ms) => format!("about {} left", format_duration(ms / 1000)),
            None => "estimating time…".to_string(),
        });
    }
    Some(parts.join(" · "))
}

pub fn result_text(st: &HistoryMediaStatus) -> String {
    let mut parts = vec![format!("Downloaded {} files ({})", st.done, format_bytes(st.bytes))];
    if st.unavailable > 0 {
        parts.push(format!("{} no longer on WhatsApp's servers", st.unavailable));
    }
    if st.failed > 0 {
        parts.push(format!("{} failed", st.failed));
    }
    parts.join(" · ")
}

/// Why a download stopped before the end, and what resuming does; `None` unless
/// it can be resumed.
pub fn paused_text(st: &HistoryMediaStatus) -> Option<String> {
    if !st.resumable {
        return None;
    }
    let why = match st.state.as_str() {
        "interrupted" => "The app or backend was restarted while downloading.",
        "cancelled" => "The download was stopped.",
        _ => "The download stopped early.",
    };
    Some(format!(
        "{why} What was downloaded is kept, and {} files are left. Resume continues where it left off.",
        st.remaining
    ))
}

/// The last problem worth showing, with how many there were.
pub fn error_text(st: &HistoryMediaStatus) -> Option<String> {
    let last = st.errors.last()?;
    Some(if st.errors.len() > 1 {
        format!("{} (and {} more)", last.message, st.errors.len() - 1)
    } else {
        last.message.clone()
    })
}

/// "4826 files to download" for the chosen kinds.
pub fn chosen_text(st: Option<&HistoryMediaStatus>, kinds: &[String]) -> String {
    if kinds.is_empty() {
        return "Choose at least one kind of media.".to_string();
    }
    let count: u64 = kinds
        .iter()
        .map(|k| st.and_then(|st| st.available.get(k)).copied().unwrap_or(0))
        .sum();
    format!("{count} files to download. Videos and documents can be large, so they are off by default.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use wabot_backend_client::dto::HistorySyncError;

    fn status(state: &str, total: u64, done: u64) -> HistoryMediaStatus {
        HistoryMediaStatus {
            state: state.into(),
            total,
            done,
            started_at: Some(1_000),
            ..Default::default()
        }
    }

    #[test]
    fn photos_stickers_and_audio_are_chosen_from_the_start() {
        assert_eq!(default_kinds(), ["image", "sticker", "audio"]);
        assert_eq!(MEDIA_KINDS.len(), 5);
    }

    #[test]
    fn states_and_sizes_read_naturally() {
        assert!(is_active("running") && is_active("cancelling"));
        for stopped in ["completed", "partial", "failed", "cancelled", "interrupted", "idle"] {
            assert!(!is_active(stopped), "{stopped}");
        }
        assert_eq!(state_label("interrupted"), "Paused");
        assert_eq!(format_bytes(812), "812 B");
        assert_eq!(format_bytes(35_651_584), "34.0 MB");
        assert_eq!(format_bytes(1_288_490_189), "1.2 GB");
    }

    #[test]
    fn the_bar_and_headline_count_every_handled_file() {
        let mut st = status("running", 100, 30);
        st.unavailable = 10;
        st.failed = 5;
        let (done, gone, failed) = progress_shares(&st);
        assert!((done - 0.30).abs() < 1e-6 && (gone - 0.10).abs() < 1e-6 && (failed - 0.05).abs() < 1e-6);
        assert_eq!(headline(&st), "Downloading · 45 of 100 files (45%)");
        assert_eq!(progress_shares(&status("idle", 0, 0)), (0.0, 0.0, 0.0));
        assert_eq!(headline(&status("idle", 0, 0)), "Not run yet");
    }

    #[test]
    fn timing_shows_time_left_only_while_running() {
        let mut st = status("running", 100, 10);
        st.elapsed_ms = 125_000;
        assert_eq!(timing_text(&st).unwrap(), "running for 2 min · estimating time…");
        st.eta_ms = Some(900_000);
        assert_eq!(timing_text(&st).unwrap(), "running for 2 min · about 15 min left");
        st.state = "cancelled".into();
        assert_eq!(timing_text(&st).unwrap(), "ran for 2 min");
        st.started_at = None;
        assert_eq!(timing_text(&st), None);
    }

    #[test]
    fn a_stopped_download_says_why_and_what_is_left() {
        let mut st = status("interrupted", 100, 30);
        st.remaining = 70;
        assert_eq!(paused_text(&st), None);
        st.resumable = true;
        let text = paused_text(&st).unwrap();
        assert!(text.contains("restarted") && text.contains("70 files are left") && text.contains("Resume"), "{text}");
        st.state = "cancelled".into();
        assert!(paused_text(&st).unwrap().contains("was stopped"));
    }

    #[test]
    fn results_errors_and_the_chosen_count() {
        let mut st = status("completed", 10, 7);
        st.bytes = 5 * 1024 * 1024;
        assert_eq!(result_text(&st), "Downloaded 7 files (5.0 MB)");
        st.unavailable = 2;
        st.failed = 1;
        let text = result_text(&st);
        assert!(text.contains("2 no longer on WhatsApp's servers") && text.contains("1 failed"), "{text}");

        assert_eq!(error_text(&st), None);
        st.errors = vec![
            HistorySyncError { chat_id: None, message: "first".into() },
            HistorySyncError { chat_id: None, message: "second".into() },
        ];
        assert_eq!(error_text(&st).unwrap(), "second (and 1 more)");

        st.available.insert("image".into(), 4826);
        st.available.insert("video".into(), 228);
        let kinds = vec!["image".to_string(), "sticker".to_string()];
        assert!(chosen_text(Some(&st), &kinds).starts_with("4826 files"));
        assert!(chosen_text(None, &kinds).starts_with("0 files"));
        assert_eq!(chosen_text(Some(&st), &[]), "Choose at least one kind of media.");
    }
}
