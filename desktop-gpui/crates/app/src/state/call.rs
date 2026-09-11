use gpui::Global;
use wabot_backend_client::dto::{
    CallDirection, CallLog, CallSource, CallState, CallStatus, CallType, MediaMode,
};
use wabot_backend_client::ws::WsEvent;

/// Terminal statuses: once the backend reports one of these the call is over and
/// the overlay must disappear (mirrors Web `terminalStatuses`).
pub fn is_terminal(status: &CallStatus) -> bool {
    matches!(
        status,
        CallStatus::Ended
            | CallStatus::Rejected
            | CallStatus::Missed
            | CallStatus::Busy
            | CallStatus::Failed
            | CallStatus::Interrupted
    )
}

/// Current wall clock in Unix milliseconds (backend call timestamps use ms).
pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Normalise a backend timestamp to milliseconds. Some call states still carry
/// Unix seconds, so anything below the ms epoch threshold is scaled up.
fn to_millis(ts: i64) -> i64 {
    if ts > 0 && ts < 100_000_000_000 {
        ts * 1000
    } else {
        ts
    }
}

/// `mm:ss` duration label shared by the overlay and its helpers.
pub fn format_duration(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// Global call state shared by every route: who is calling, what the backend is
/// doing, and the local control toggles.
#[derive(Clone, Debug, Default)]
pub struct CallManager {
    pub active_call: Option<CallState>,
    pub incoming_call: Option<CallState>,
    pub mic_muted: bool,
    pub speaker_muted: bool,
    pub video_upgrade_requested: bool,
    pub call_history: Vec<CallLog>,
}

impl Global for CallManager {}

impl CallManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn global(cx: &gpui::App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut gpui::App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    pub fn initiate_call(&mut self, target: &str, call_type: CallType, group_jid: Option<String>) {
        let now = now_millis();
        let video_enabled = matches!(call_type, CallType::Video | CallType::GroupVideo);

        let call = CallState {
            id: format!("call-{}", now),
            status: CallStatus::Initiating,
            call_type,
            direction: CallDirection::Outgoing,
            source: CallSource::Ui,
            media_mode: MediaMode::Live,
            target: target.to_string(),
            group_jid,
            participants: None,
            started_at: now,
            answered_at: None,
            video_enabled,
            remote_video_enabled: false,
        };

        self.active_call = Some(call);
        self.incoming_call = None;
        self.video_upgrade_requested = false;
    }


    /// Replace the tracked call with the authoritative state returned by the API
    /// or pushed over the WebSocket while preserving local control toggles.
    pub fn adopt(&mut self, call: CallState) {
        self.active_call = Some(call);
        self.incoming_call = None;
    }

    pub fn receive_incoming(&mut self, call: CallState) {
        if self.active_call.is_none() {
            self.incoming_call = Some(call);
        }
    }

    pub fn accept_incoming(&mut self) -> Option<CallState> {
        if let Some(mut call) = self.incoming_call.take() {
            let now = now_millis();
            call.status = CallStatus::Connected;
            call.answered_at = Some(now);
            self.active_call = Some(call.clone());
            Some(call)
        } else {
            None
        }
    }

    pub fn reject_incoming(&mut self) {
        self.incoming_call = None;
    }

    pub fn connect_active(&mut self) {
        if let Some(call) = &mut self.active_call {
            let now = now_millis();
            call.status = CallStatus::Connected;
            if call.answered_at.is_none() {
                call.answered_at = Some(now);
            }
        }
        self.incoming_call = None;
    }

    pub fn hangup(&mut self) -> Option<CallLog> {
        let call = self.active_call.take()?;
        let now = now_millis();

        let duration_ms = call.answered_at.map(|ans| (now - to_millis(ans)).max(0)).unwrap_or(0);

        let log = CallLog {
            id: call.id.clone(),
            meow_call_id: call.id.clone(),
            direction: call.direction,
            call_type: call.call_type,
            target: call.target,
            group_jid: call.group_jid,
            participants: call.participants,
            source: call.source,
            media_mode: call.media_mode,
            status: CallStatus::Ended,
            error_message: None,
            api_key_id: None,
            started_at: call.started_at,
            answered_at: call.answered_at,
            ended_at: Some(now),
            duration_ms: Some(duration_ms),
            created_at: now,
        };

        self.call_history.insert(0, log.clone());
        Some(log)
    }

    pub fn toggle_mic(&mut self) {
        self.mic_muted = !self.mic_muted;
    }

    pub fn toggle_speaker(&mut self) {
        self.speaker_muted = !self.speaker_muted;
    }

    pub fn request_video_upgrade(&mut self) {
        self.video_upgrade_requested = true;
    }

    pub fn accept_video_upgrade(&mut self) {
        if let Some(call) = &mut self.active_call {
            call.video_enabled = true;
            call.remote_video_enabled = true;
            call.call_type = CallType::Video;
        }
        self.video_upgrade_requested = false;
    }

    // --- Overlay state ---

    /// True while a call should cover the whole app (any non-terminal status).
    pub fn is_overlay_visible(&self) -> bool {
        self.active_call
            .as_ref()
            .map(|c| !is_terminal(&c.status))
            .unwrap_or(false)
    }

    /// True while an inbound call is still ringing and not already answered.
    pub fn is_incoming_visible(&self) -> bool {
        let Some(incoming) = self.incoming_call.as_ref() else {
            return false;
        };
        if incoming.status != CallStatus::Ringing || incoming.direction != CallDirection::Incoming {
            return false;
        }
        // The full-screen call view takes over once the same call connects.
        match self.active_call.as_ref() {
            Some(active)
                if active.id == incoming.id
                    && matches!(active.status, CallStatus::Connected | CallStatus::Connecting) =>
            {
                false
            }
            _ => true,
        }
    }

    pub fn is_video_call(&self) -> bool {
        self.active_call
            .as_ref()
            .map(|c| matches!(c.call_type, CallType::Video | CallType::GroupVideo))
            .unwrap_or(false)
    }

    pub fn is_group_call(&self) -> bool {
        self.active_call
            .as_ref()
            .map(|c| matches!(c.call_type, CallType::GroupAudio | CallType::GroupVideo))
            .unwrap_or(false)
    }

    /// Remote party (JID or number) of the tracked call.
    pub fn peer(&self) -> String {
        self.active_call
            .as_ref()
            .map(|c| c.group_jid.clone().unwrap_or_else(|| c.target.clone()))
            .or_else(|| self.incoming_call.as_ref().map(|c| c.target.clone()))
            .unwrap_or_default()
    }

    pub fn is_connected(&self) -> bool {
        self.active_call
            .as_ref()
            .map(|c| matches!(c.status, CallStatus::Connected | CallStatus::Connecting))
            .unwrap_or(false)
    }

    /// Elapsed connected time in seconds (0 until the call is answered).
    pub fn duration_seconds(&self, now_ms: i64) -> u64 {
        let Some(call) = self.active_call.as_ref() else {
            return 0;
        };
        let started = call.answered_at.unwrap_or(call.started_at);
        if started <= 0 {
            return 0;
        }
        (((now_ms - to_millis(started)).max(0)) / 1000) as u64
    }

    /// Status line rendered under the peer name, matching Web `statusLabel`.
    pub fn status_label(&self, now_ms: i64) -> String {
        let Some(call) = self.active_call.as_ref() else {
            return String::new();
        };
        match call.status {
            CallStatus::Preparing => "Preparing…".to_string(),
            CallStatus::Initiating => "Calling…".to_string(),
            CallStatus::Ringing => "Ringing…".to_string(),
            CallStatus::Connecting => "Connecting…".to_string(),
            CallStatus::Connected => format_duration(self.duration_seconds(now_ms)),
            CallStatus::Ending => "Ending…".to_string(),
            _ => String::new(),
        }
    }

    /// Patch the video flags of the tracked call from a `call.video_state` envelope.
    pub fn patch_video(&mut self, id: &str, video_enabled: bool, remote_video_enabled: bool) -> bool {
        if let Some(call) = self.active_call.as_mut() {
            if call.id == id {
                let changed = call.video_enabled != video_enabled
                    || call.remote_video_enabled != remote_video_enabled;
                call.video_enabled = video_enabled;
                call.remote_video_enabled = remote_video_enabled;
                return changed;
            }
        }
        false
    }

    pub fn patch_participants(&mut self, id: &str, participants: Vec<String>) -> bool {
        if let Some(call) = self.active_call.as_mut() {
            if call.id == id && call.participants.as_ref() != Some(&participants) {
                call.participants = Some(participants);
                return true;
            }
        }
        false
    }

    /// Drop the tracked call locally (the backend push or a local hangup is the
    /// source of truth; this only clears UI state).
    pub fn end_local(&mut self, id: Option<&str>) -> bool {
        let matches_id = |call: &CallState| id.map(|want| call.id == want).unwrap_or(true);

        let mut changed = false;
        if self.active_call.as_ref().map(matches_id).unwrap_or(false) {
            self.active_call = None;
            changed = true;
        }
        if self.incoming_call.as_ref().map(matches_id).unwrap_or(false) {
            self.incoming_call = None;
            changed = true;
        }
        if changed {
            self.video_upgrade_requested = false;
        }
        changed
    }

    /// Apply an authoritative call state, clearing the call on terminal statuses.
    fn apply_state(&mut self, state: CallState) -> bool {
        // Guard against the backend's zero-value broadcasts (no active call).
        if state.id.is_empty()
            || (state.status == CallStatus::Unknown && state.target.is_empty())
        {
            return false;
        }

        if is_terminal(&state.status) {
            return self.end_local(Some(&state.id));
        }

        if state.status == CallStatus::Ringing && state.direction == CallDirection::Incoming {
            self.incoming_call = Some(state.clone());
        } else if state.direction == CallDirection::Outgoing
            || matches!(state.status, CallStatus::Connected | CallStatus::Connecting)
        {
            if self
                .incoming_call
                .as_ref()
                .map(|inc| inc.id == state.id)
                .unwrap_or(false)
            {
                self.incoming_call = None;
            }
        }

        self.active_call = Some(state);
        true
    }

    fn parse_state(payload: &serde_json::Value) -> Option<CallState> {
        if payload.is_null() {
            return None;
        }
        serde_json::from_value::<CallState>(payload.clone()).ok()
    }

    /// Fold a backend WebSocket event into the call state. Returns true when the
    /// UI needs to be repainted.
    pub fn handle_ws_event(&mut self, event: &WsEvent) -> bool {
        let WsEvent::CallEvent { event_type, payload } = event else {
            return false;
        };

        match event_type.as_str() {
            "call.incoming" => match Self::parse_state(payload) {
                Some(state) => {
                    self.incoming_call = Some(state.clone());
                    self.active_call = Some(state);
                    true
                }
                None => false,
            },
            "call.state" | "call.peer_accepted" | "call.ready" => match Self::parse_state(payload) {
                Some(state) => self.apply_state(state),
                None => false,
            },
            "call.ended" => {
                let id = payload
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                self.end_local(id.as_deref())
            }
            "call.video_state" | "call.video_upgrade_requested" => {
                let id = payload
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let local = payload
                    .get("video_enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let remote = payload
                    .get("remote_video_enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let mut changed = self.patch_video(&id, local, remote);
                if event_type == "call.video_upgrade_requested" && !self.video_upgrade_requested {
                    self.video_upgrade_requested = true;
                    changed = true;
                }
                changed
            }
            "call.group_state" => {
                let id = payload
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();

                let mut changed = false;
                if let Some(list) = payload.get("participants").and_then(|v| v.as_array()) {
                    let participants: Vec<String> = list
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    changed = self.patch_participants(&id, participants);
                }
                if let Some(state) = payload
                    .get("state")
                    .and_then(|v| Self::parse_state(v))
                {
                    if self.active_call.as_ref().map(|c| c.id == state.id).unwrap_or(false) {
                        changed = self.apply_state(state) || changed;
                    }
                }
                changed
            }
            "call.participant_join" => {
                let id = payload
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let target = payload
                    .get("target")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                if let (Some(target), Some(call)) = (target, self.active_call.as_mut()) {
                    if call.id == id {
                        let participants = call.participants.get_or_insert_with(Vec::new);
                        if !participants.iter().any(|p| p == &target) {
                            participants.push(target);
                            return true;
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }

    pub fn filtered_history<'a>(
        &'a self,
        direction: Option<&str>,
        call_type: Option<&str>,
        status: Option<&str>,
    ) -> Vec<&'a CallLog> {
        self.call_history
            .iter()
            .filter(|log| {
                if let Some(d) = direction {
                    match d {
                        "incoming" => log.direction == CallDirection::Incoming,
                        "outgoing" => log.direction == CallDirection::Outgoing,
                        _ => true,
                    }
                } else {
                    true
                }
            })
            .filter(|log| {
                if let Some(t) = call_type {
                    match t {
                        "audio" => log.call_type == CallType::Audio,
                        "video" => log.call_type == CallType::Video,
                        "group_audio" => log.call_type == CallType::GroupAudio,
                        "group_video" => log.call_type == CallType::GroupVideo,
                        _ => true,
                    }
                } else {
                    true
                }
            })
            .filter(|log| {
                if let Some(s) = status {
                    match s {
                        "connected" => log.status == CallStatus::Connected || log.status == CallStatus::Ended,
                        "missed" => log.status == CallStatus::Missed,
                        "rejected" => log.status == CallStatus::Rejected,
                        _ => true,
                    }
                } else {
                    true
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state(id: &str, status: CallStatus, direction: CallDirection) -> CallState {
        CallState {
            id: id.to_string(),
            status,
            call_type: CallType::Audio,
            direction,
            source: CallSource::Ui,
            media_mode: MediaMode::Live,
            target: "628123456789@s.whatsapp.net".to_string(),
            group_jid: None,
            participants: None,
            started_at: now_millis() - 5_000,
            answered_at: None,
            video_enabled: false,
            remote_video_enabled: false,
        }
    }

    #[test]
    fn test_call_lifecycle_and_history() {
        let mut mgr = CallManager::new();
        assert!(mgr.active_call.is_none());

        // Initiate call
        mgr.initiate_call("+628123456789", CallType::Audio, None);
        assert!(mgr.active_call.is_some());
        assert_eq!(mgr.active_call.as_ref().unwrap().status, CallStatus::Initiating);
        assert!(mgr.is_overlay_visible());

        // Connect
        mgr.connect_active();
        assert_eq!(mgr.active_call.as_ref().unwrap().status, CallStatus::Connected);
        assert_eq!(mgr.status_label(now_millis() + 65_000), "01:05");

        // Hangup
        let log = mgr.hangup().unwrap();
        assert!(mgr.active_call.is_none());
        assert_eq!(log.status, CallStatus::Ended);
        assert_eq!(mgr.call_history.len(), 1);

        // Filter history
        let filtered = mgr.filtered_history(Some("outgoing"), Some("audio"), None);
        assert_eq!(filtered.len(), 1);

        let non_matching = mgr.filtered_history(Some("incoming"), None, None);
        assert_eq!(non_matching.len(), 0);
    }

    #[test]
    fn test_incoming_call_and_video_upgrade() {
        let mut mgr = CallManager::new();
        let incoming = sample_state("inc-1", CallStatus::Ringing, CallDirection::Incoming);

        mgr.receive_incoming(incoming);
        assert!(mgr.incoming_call.is_some());
        assert!(mgr.is_incoming_visible());

        // Accept
        let accepted = mgr.accept_incoming().unwrap();
        assert_eq!(accepted.status, CallStatus::Connected);
        assert!(mgr.incoming_call.is_none());
        assert!(mgr.active_call.is_some());
        assert!(!mgr.is_incoming_visible());

        // Request and accept video upgrade
        mgr.request_video_upgrade();
        assert!(mgr.video_upgrade_requested);
        mgr.accept_video_upgrade();
        assert!(!mgr.video_upgrade_requested);
        assert_eq!(mgr.active_call.as_ref().unwrap().call_type, CallType::Video);
        assert!(mgr.active_call.as_ref().unwrap().video_enabled);
    }

    #[test]
    fn test_ws_call_state_drives_overlay() {
        let mut mgr = CallManager::new();

        // Backend pushes the call we just started (echoed as call.state).
        let started = sample_state("call-1", CallStatus::Initiating, CallDirection::Outgoing);
        assert!(mgr.handle_ws_event(&WsEvent::CallEvent {
            event_type: "call.state".to_string(),
            payload: serde_json::to_value(&started).unwrap(),
        }));
        assert!(mgr.is_overlay_visible());

        // Connected state keeps the overlay up and starts the timer.
        let connected = sample_state("call-1", CallStatus::Connected, CallDirection::Outgoing);
        mgr.handle_ws_event(&WsEvent::CallEvent {
            event_type: "call.ready".to_string(),
            payload: serde_json::to_value(&connected).unwrap(),
        });
        assert!(mgr.is_connected());
        // Connected calls tick up from `answered_at`, falling back to `started_at`.
        assert_eq!(mgr.duration_seconds(now_millis()), 5);
        assert_eq!(mgr.status_label(now_millis()), "00:05");

        // Terminal state tears it down.
        let ended = sample_state("call-1", CallStatus::Ended, CallDirection::Outgoing);
        assert!(mgr.handle_ws_event(&WsEvent::CallEvent {
            event_type: "call.ended".to_string(),
            payload: serde_json::json!({ "id": "call-1", "status": "ended", "state": ended }),
        }));
        assert!(!mgr.is_overlay_visible());
    }

    #[test]
    fn test_ws_incoming_and_zero_value_broadcast() {
        let mut mgr = CallManager::new();

        let incoming = sample_state("inc-9", CallStatus::Ringing, CallDirection::Incoming);
        mgr.handle_ws_event(&WsEvent::CallEvent {
            event_type: "call.incoming".to_string(),
            payload: serde_json::to_value(&incoming).unwrap(),
        });
        assert!(mgr.is_incoming_visible());

        // The backend also broadcasts empty states when nothing is active; they
        // must not clobber the ringing call.
        assert!(!mgr.handle_ws_event(&WsEvent::CallEvent {
            event_type: "call.state".to_string(),
            payload: serde_json::json!({
                "id": "",
                "status": "",
                "type": "",
                "direction": "",
                "source": "",
                "media_mode": "",
                "target": "",
                "started_at": 0,
                "answered_at": null
            }),
        }));
        assert!(mgr.is_incoming_visible());
    }

    #[test]
    fn test_overlay_helper_follows_call_state() {
        // The overlay component's helper must agree with the global state.
        use crate::components::call_overlay::CallOverlayHelper;

        let mut mgr = CallManager::new();
        assert!(!CallOverlayHelper::is_overlay_needed(&mgr));
        assert_eq!(CallOverlayHelper::format_duration(65), "01:05");

        mgr.receive_incoming(sample_state(
            "inc-1",
            CallStatus::Ringing,
            CallDirection::Incoming,
        ));
        assert!(CallOverlayHelper::is_overlay_needed(&mgr));
    }

    #[test]
    fn test_ws_video_state_patches_flags() {
        let mut mgr = CallManager::new();
        mgr.adopt(sample_state("call-7", CallStatus::Connected, CallDirection::Outgoing));

        assert!(mgr.handle_ws_event(&WsEvent::CallEvent {
            event_type: "call.video_state".to_string(),
            payload: serde_json::json!({
                "id": "call-7",
                "video_enabled": true,
                "remote_video_enabled": true
            }),
        }));

        let call = mgr.active_call.as_ref().unwrap();
        assert!(call.video_enabled);
        assert!(call.remote_video_enabled);
        // A voice call upgraded to video keeps its original type until the
        // backend reports otherwise.
        assert!(!mgr.is_video_call());
    }
}
