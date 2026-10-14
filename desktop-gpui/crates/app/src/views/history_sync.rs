//! What the "Chat history" card shows: the depth choices a user picks from and
//! the plain-language reading of a sync's progress.

use wabot_backend_client::dto::{HistorySyncRequest, HistorySyncStatus};

/// How far back a sync goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncDepth {
    PerChat50,
    PerChat200,
    PerChat1000,
    Days30,
    Days365,
    Everything,
}

impl SyncDepth {
    pub const ALL: [SyncDepth; 6] = [
        SyncDepth::PerChat50,
        SyncDepth::PerChat200,
        SyncDepth::PerChat1000,
        SyncDepth::Days30,
        SyncDepth::Days365,
        SyncDepth::Everything,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SyncDepth::PerChat50 => "50 / chat",
            SyncDepth::PerChat200 => "200 / chat",
            SyncDepth::PerChat1000 => "1000 / chat",
            SyncDepth::Days30 => "Last 30 days",
            SyncDepth::Days365 => "Last year",
            SyncDepth::Everything => "Everything",
        }
    }

    /// One sentence on what the choice does and what it costs.
    pub fn hint(self) -> &'static str {
        match self {
            SyncDepth::PerChat50 => "Quick: up to 50 older messages in every chat.",
            SyncDepth::PerChat200 => "Up to 200 older messages in every chat.",
            SyncDepth::PerChat1000 => "Up to 1000 older messages in every chat. Takes a while on many chats.",
            SyncDepth::Days30 => "Every message from the last 30 days, however many that is.",
            SyncDepth::Days365 => "Every message from the last year. Can take a long time.",
            SyncDepth::Everything => {
                "Everything your phone will give, back to the first message of each chat. \
                 Keep the phone online; this can take hours."
            }
        }
    }

    pub fn request(self) -> HistorySyncRequest {
        let mut req = HistorySyncRequest::default();
        match self {
            SyncDepth::PerChat50 => req.per_chat = 50,
            SyncDepth::PerChat200 => req.per_chat = 200,
            SyncDepth::PerChat1000 => req.per_chat = 1000,
            SyncDepth::Days30 => req.since_days = 30,
            SyncDepth::Days365 => req.since_days = 365,
            SyncDepth::Everything => req.all = true,
        }
        req
    }
}

/// True while a sync is running or being stopped.
pub fn is_active(state: &str) -> bool {
    matches!(state, "running" | "cancelling")
}

pub fn state_label(state: &str) -> &'static str {
    match state {
        "running" => "Syncing",
        "cancelling" => "Stopping…",
        "completed" => "Done",
        "partial" => "Done, with warnings",
        "cancelled" => "Stopped",
        "interrupted" => "Paused",
        "failed" => "Failed",
        _ => "Not run yet",
    }
}

/// Share of chats finished, 0..=1.
pub fn progress_fraction(st: &HistorySyncStatus) -> f32 {
    if st.chats_total == 0 {
        return 0.0;
    }
    (st.chats_processed as f32 / st.chats_total as f32).clamp(0.0, 1.0)
}

/// "3 min", "1 h 20 min", "45 s".
pub fn format_duration(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs} s"),
        60..=3599 => format!("{} min", (secs + 30) / 60),
        _ => {
            let mins = (secs + 30) / 60;
            if mins % 60 == 0 {
                format!("{} h", mins / 60)
            } else {
                format!("{} h {} min", mins / 60, mins % 60)
            }
        }
    }
}

/// Share of chats being worked on right now (drawn after the finished part of
/// the bar), 0..=1.
pub fn in_progress_fraction(st: &HistorySyncStatus) -> f32 {
    if st.chats_total == 0 || !is_active(&st.state) {
        return 0.0;
    }
    let room = st.chats_total.saturating_sub(st.chats_processed) as usize;
    room.min(st.current.len()) as f32 / st.chats_total as f32
}

/// "running for 3 min" / "ran for 12 min": the time the sync actually ran, not
/// counting the time it was stopped.
pub fn elapsed_text(st: &HistorySyncStatus) -> Option<String> {
    st.started_at?;
    let word = if is_active(&st.state) { "running for" } else { "ran for" };
    Some(format!("{word} {}", format_duration(st.elapsed_ms / 1000)))
}

/// "about 20 min left" while running, "estimating time…" until the backend can
/// tell.
pub fn eta_text(st: &HistorySyncStatus) -> Option<String> {
    if !is_active(&st.state) || st.chats_total == 0 {
        return None;
    }
    Some(match st.eta_ms {
        Some(ms) => format!("about {} left", format_duration(ms / 1000)),
        None => "estimating time…".to_string(),
    })
}

/// "368 of 999 chats (37%)" for the headline.
pub fn chats_text(st: &HistorySyncStatus) -> Option<String> {
    (st.chats_total > 0).then(|| {
        format!(
            "{} of {} chats ({}%)",
            st.chats_processed,
            st.chats_total,
            (progress_fraction(st) * 100.0).round() as u32
        )
    })
}

/// What the "retry finished chats" toggle is about, with how many chats it concerns.
pub fn finished_chats_text(st: Option<&HistorySyncStatus>) -> String {
    match st.map(|st| st.chats_finished).unwrap_or(0) {
        0 => "Chats that already reached the start of their history are skipped unless this is on.".to_string(),
        n => format!("{n} chats already reached the start of their history. They are skipped unless this is on."),
    }
}

/// Why a sync stopped before the end, and what resuming does; `None` unless it
/// can be resumed.
pub fn paused_text(st: &HistorySyncStatus) -> Option<String> {
    if !st.resumable {
        return None;
    }
    let left = st.chats_total.saturating_sub(st.chats_processed);
    let remaining = if left > 0 { format!("{left} chats are left") } else { "some chats are left".to_string() };
    let why = match st.state.as_str() {
        "interrupted" => "The app or backend was restarted while syncing.",
        "cancelled" => "The sync was stopped.",
        _ => "The sync stopped early.",
    };
    Some(format!(
        "{why} Nothing imported so far is lost, and {remaining}. Resume continues where it left off."
    ))
}

/// What a chat in progress is doing, for the "Now:" lines.
pub fn current_text(c: &wabot_backend_client::dto::HistorySyncCurrent) -> String {
    let name = if c.name.trim().is_empty() { c.chat_id.as_str() } else { c.name.as_str() };
    let what = match c.phase.as_str() {
        "asking-phone" => "asking your phone for older messages",
        _ => "importing",
    };
    if c.added > 0 {
        format!("{name} — {what}, +{} so far", c.added)
    } else {
        format!("{name} — {what}")
    }
}

/// The result line: what was added and what did not need work.
pub fn result_text(st: &HistorySyncStatus) -> String {
    let mut parts = vec![format!(
        "Added {} messages in {} chats",
        st.messages_added, st.chats_with_new
    )];
    if st.chats_exhausted > 0 {
        parts.push(format!("{} chats had nothing older", st.chats_exhausted));
    }
    if st.chats_skipped > 0 {
        parts.push(format!("{} skipped (already at their start)", st.chats_skipped));
    }
    if st.timeouts > 0 {
        parts.push(format!("{} requests got no answer from the phone", st.timeouts));
    }
    parts.join(" · ")
}

/// A note about messages that were received but not imported, or `None`.
pub fn staged_text(st: &HistorySyncStatus) -> Option<String> {
    (st.pending_messages > 0).then(|| {
        format!(
            "{} older messages in {} chats are downloaded but not imported yet — choose a deeper option to import them.",
            st.pending_messages, st.pending_chats
        )
    })
}

/// The last problem worth showing, with how many there were.
pub fn error_text(st: &HistorySyncStatus) -> Option<String> {
    let last = st.errors.last()?;
    let message = match &last.chat_id {
        Some(chat) if !chat.is_empty() => format!("{chat}: {}", last.message),
        _ => last.message.clone(),
    };
    Some(if st.errors.len() > 1 {
        format!("{message} (and {} more)", st.errors.len() - 1)
    } else {
        message
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wabot_backend_client::dto::{HistorySyncCurrent, HistorySyncError};

    fn status(state: &str, total: u64, processed: u64) -> HistorySyncStatus {
        HistorySyncStatus {
            state: state.into(),
            chats_total: total,
            chats_processed: processed,
            started_at: Some(1_000_000),
            ..Default::default()
        }
    }

    #[test]
    fn every_depth_maps_to_one_request() {
        assert_eq!(SyncDepth::PerChat50.request().per_chat, 50);
        assert_eq!(SyncDepth::PerChat1000.request().per_chat, 1000);
        assert_eq!(SyncDepth::Days30.request().since_days, 30);
        assert_eq!(SyncDepth::Days365.request().since_days, 365);
        let all = SyncDepth::Everything.request();
        assert!(all.all && all.per_chat == 0 && all.since_days == 0);
        for depth in SyncDepth::ALL {
            assert!(!depth.label().is_empty() && !depth.hint().is_empty());
        }
    }

    #[test]
    fn a_dead_run_is_not_shown_as_active() {
        assert!(is_active("running") && is_active("cancelling"));
        for done in ["completed", "partial", "failed", "cancelled", "interrupted", "idle"] {
            assert!(!is_active(done), "{done}");
        }
    }

    #[test]
    fn fraction_and_durations() {
        assert_eq!(progress_fraction(&status("running", 0, 0)), 0.0);
        assert!((progress_fraction(&status("running", 200, 50)) - 0.25).abs() < 1e-6);
        assert_eq!(progress_fraction(&status("running", 10, 99)), 1.0);
        assert_eq!(format_duration(45), "45 s");
        assert_eq!(format_duration(125), "2 min");
        assert_eq!(format_duration(3600), "1 h");
        assert_eq!(format_duration(4800), "1 h 20 min");
    }

    #[test]
    fn the_headline_pieces_follow_the_backend() {
        let mut st = status("running", 100, 10);
        st.elapsed_ms = 100_000;
        assert_eq!(chats_text(&st).unwrap(), "10 of 100 chats (10%)");
        assert_eq!(chats_text(&status("running", 0, 0)), None);
        assert_eq!(elapsed_text(&st).unwrap(), "running for 2 min");
        // No estimate yet: say so, then show the backend's.
        assert_eq!(eta_text(&st).unwrap(), "estimating time…");
        st.eta_ms = Some(900_000);
        assert_eq!(eta_text(&st).unwrap(), "about 15 min left");
        // A stopped sync has no time left to estimate.
        st.state = "cancelled".into();
        assert_eq!(eta_text(&st), None);
        assert_eq!(elapsed_text(&st).unwrap(), "ran for 2 min");
    }

    #[test]
    fn working_chats_are_drawn_after_the_finished_ones() {
        let mut st = status("running", 100, 90);
        st.current = vec![HistorySyncCurrent::default(); 3];
        assert!((in_progress_fraction(&st) - 0.03).abs() < 1e-6);
        // Never past the end of the bar.
        let mut nearly = status("running", 10, 9);
        nearly.current = vec![HistorySyncCurrent::default(); 3];
        assert!((in_progress_fraction(&nearly) - 0.1).abs() < 1e-6);
        st.state = "interrupted".into();
        assert_eq!(in_progress_fraction(&st), 0.0);
    }

    #[test]
    fn the_retry_toggle_says_how_many_chats_it_is_about() {
        assert!(finished_chats_text(None).starts_with("Chats that already reached"));
        let mut st = status("idle", 0, 0);
        assert!(finished_chats_text(Some(&st)).starts_with("Chats that already reached"));
        st.chats_finished = 412;
        assert!(finished_chats_text(Some(&st)).starts_with("412 chats already reached"));
    }

    #[test]
    fn a_stopped_sync_says_why_and_offers_to_continue() {
        let mut st = status("interrupted", 999, 368);
        assert_eq!(state_label(&st.state), "Paused");
        assert_eq!(paused_text(&st), None); // not resumable: nothing to say
        st.resumable = true;
        let text = paused_text(&st).unwrap();
        assert!(text.contains("restarted") && text.contains("631 chats are left") && text.contains("Resume"), "{text}");
        st.state = "cancelled".into();
        assert!(paused_text(&st).unwrap().contains("was stopped"));
    }

    #[test]
    fn texts_name_what_is_happening() {
        let c = HistorySyncCurrent { chat_id: "1@s.whatsapp.net".into(), name: "Budi".into(), added: 120, phase: "asking-phone".into() };
        assert_eq!(current_text(&c), "Budi — asking your phone for older messages, +120 so far");
        let unnamed = HistorySyncCurrent { chat_id: "1@s.whatsapp.net".into(), name: " ".into(), added: 0, phase: "importing".into() };
        assert_eq!(current_text(&unnamed), "1@s.whatsapp.net — importing");

        let mut st = status("completed", 10, 10);
        st.messages_added = 42;
        st.chats_with_new = 3;
        assert_eq!(result_text(&st), "Added 42 messages in 3 chats");
        st.chats_exhausted = 2;
        st.chats_skipped = 4;
        st.timeouts = 1;
        assert!(result_text(&st).contains("2 chats had nothing older"));
        assert!(result_text(&st).contains("4 skipped"));
        assert!(result_text(&st).contains("1 requests got no answer"));

        assert_eq!(staged_text(&st), None);
        st.pending_messages = 22748;
        st.pending_chats = 24;
        assert!(staged_text(&st).unwrap().contains("22748 older messages in 24 chats"));

        assert_eq!(error_text(&st), None);
        st.errors = vec![
            HistorySyncError { chat_id: None, message: "first".into() },
            HistorySyncError { chat_id: Some("x@g.us".into()), message: "second".into() },
        ];
        assert_eq!(error_text(&st).unwrap(), "x@g.us: second (and 1 more)");
    }
}
