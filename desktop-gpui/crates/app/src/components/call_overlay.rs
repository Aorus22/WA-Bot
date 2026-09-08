use crate::state::call::CallManager;

pub struct CallOverlayHelper;

impl CallOverlayHelper {
    pub fn format_duration(seconds: u64) -> String {
        let mins = seconds / 60;
        let secs = seconds % 60;
        format!("{:02}:{:02}", mins, secs)
    }

    pub fn is_overlay_needed(mgr: &CallManager) -> bool {
        mgr.active_call.is_some() || mgr.incoming_call.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wabot_backend_client::dto::{CallDirection, CallSource, CallState, CallStatus, CallType, MediaMode};

    #[test]
    fn test_call_overlay_helper() {
        let mut mgr = CallManager::new();
        assert!(!CallOverlayHelper::is_overlay_needed(&mgr));
        assert_eq!(CallOverlayHelper::format_duration(65), "01:05");

        mgr.incoming_call = Some(CallState {
            id: "call-1".to_string(),
            status: CallStatus::Ringing,
            call_type: CallType::Audio,
            direction: CallDirection::Incoming,
            source: CallSource::Incoming,
            media_mode: MediaMode::Live,
            target: "+62812".to_string(),
            group_jid: None,
            participants: None,
            started_at: 0,
            answered_at: None,
            video_enabled: false,
            remote_video_enabled: false,
        });

        assert!(CallOverlayHelper::is_overlay_needed(&mgr));
    }
}
