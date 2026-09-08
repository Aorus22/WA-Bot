use wabot_backend_client::dto::{
    CallDirection, CallLog, CallSource, CallState, CallStatus, CallType, MediaMode,
};

#[derive(Clone, Debug, Default)]
pub struct CallManager {
    pub active_call: Option<CallState>,
    pub incoming_call: Option<CallState>,
    pub mic_muted: bool,
    pub speaker_muted: bool,
    pub video_upgrade_requested: bool,
    pub call_history: Vec<CallLog>,
}

impl CallManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initiate_call(&mut self, target: &str, call_type: CallType, group_jid: Option<String>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

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
            video_enabled: false,
            remote_video_enabled: false,
        };

        self.active_call = Some(call);
        self.video_upgrade_requested = false;
    }

    pub fn receive_incoming(&mut self, call: CallState) {
        if self.active_call.is_none() {
            self.incoming_call = Some(call);
        }
    }

    pub fn accept_incoming(&mut self) -> Option<CallState> {
        if let Some(mut call) = self.incoming_call.take() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
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
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            call.status = CallStatus::Connected;
            if call.answered_at.is_none() {
                call.answered_at = Some(now);
            }
        }
    }

    pub fn hangup(&mut self) -> Option<CallLog> {
        let call = self.active_call.take()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let duration_ms = call.answered_at.map(|ans| (now - ans) * 1000).unwrap_or(0);

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

    #[test]
    fn test_call_lifecycle_and_history() {
        let mut mgr = CallManager::new();
        assert!(mgr.active_call.is_none());

        // Initiate call
        mgr.initiate_call("+628123456789", CallType::Audio, None);
        assert!(mgr.active_call.is_some());
        assert_eq!(mgr.active_call.as_ref().unwrap().status, CallStatus::Initiating);

        // Connect
        mgr.connect_active();
        assert_eq!(mgr.active_call.as_ref().unwrap().status, CallStatus::Connected);

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
        let incoming = CallState {
            id: "inc-1".to_string(),
            status: CallStatus::Ringing,
            call_type: CallType::Audio,
            direction: CallDirection::Incoming,
            source: CallSource::Incoming,
            media_mode: MediaMode::Live,
            target: "+628999".to_string(),
            group_jid: None,
            participants: None,
            started_at: 1000,
            answered_at: None,
            video_enabled: false,
            remote_video_enabled: false,
        };

        mgr.receive_incoming(incoming);
        assert!(mgr.incoming_call.is_some());

        // Accept
        let accepted = mgr.accept_incoming().unwrap();
        assert_eq!(accepted.status, CallStatus::Connected);
        assert!(mgr.incoming_call.is_none());
        assert!(mgr.active_call.is_some());

        // Request and accept video upgrade
        mgr.request_video_upgrade();
        assert!(mgr.video_upgrade_requested);
        mgr.accept_video_upgrade();
        assert!(!mgr.video_upgrade_requested);
        assert_eq!(mgr.active_call.as_ref().unwrap().call_type, CallType::Video);
        assert!(mgr.active_call.as_ref().unwrap().video_enabled);
    }
}
