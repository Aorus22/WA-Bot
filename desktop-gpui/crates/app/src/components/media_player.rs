#[derive(Clone, Debug, PartialEq)]
pub struct AudioPlayerState {
    pub is_playing: bool,
    pub duration_secs: f32,
    pub current_pos_secs: f32,
    pub volume: f32,
    pub waveform_samples: Vec<f32>,
}

impl AudioPlayerState {
    pub fn new(duration_secs: f32) -> Self {
        Self {
            is_playing: false,
            duration_secs,
            current_pos_secs: 0.0,
            volume: 1.0,
            waveform_samples: vec![0.2, 0.4, 0.7, 0.9, 0.5, 0.3, 0.8, 0.6, 0.4, 0.2],
        }
    }

    pub fn toggle_playback(&mut self) {
        self.is_playing = !self.is_playing;
    }

    pub fn play(&mut self) {
        self.is_playing = true;
    }

    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    pub fn seek(&mut self, pos_secs: f32) {
        self.current_pos_secs = pos_secs.clamp(0.0, self.duration_secs);
    }

    pub fn progress_pct(&self) -> f32 {
        if self.duration_secs <= 0.0 {
            0.0
        } else {
            (self.current_pos_secs / self.duration_secs * 100.0).clamp(0.0, 100.0)
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaViewerState {
    pub is_open: bool,
    pub media_url: Option<String>,
    pub media_type: String, // "image", "video", "document", "audio"
    pub caption: Option<String>,
    pub file_name: Option<String>,
}

impl MediaViewerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self, url: String, media_type: String, caption: Option<String>, file_name: Option<String>) {
        self.is_open = true;
        self.media_url = Some(url);
        self.media_type = media_type;
        self.caption = caption;
        self.file_name = file_name;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.media_url = None;
        self.caption = None;
        self.file_name = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_player_state_transitions() {
        let mut player = AudioPlayerState::new(30.0);
        assert!(!player.is_playing);
        assert_eq!(player.progress_pct(), 0.0);

        player.toggle_playback();
        assert!(player.is_playing);

        player.seek(15.0);
        assert_eq!(player.current_pos_secs, 15.0);
        assert_eq!(player.progress_pct(), 50.0);

        player.seek(40.0); // beyond duration clamp
        assert_eq!(player.current_pos_secs, 30.0);
        assert_eq!(player.progress_pct(), 100.0);
    }

    #[test]
    fn test_media_viewer_open_close() {
        let mut viewer = MediaViewerState::new();
        assert!(!viewer.is_open);

        viewer.open("https://media.test/img.png".to_string(), "image".to_string(), Some("Sunset".to_string()), Some("sunset.png".to_string()));
        assert!(viewer.is_open);
        assert_eq!(viewer.media_url.as_deref(), Some("https://media.test/img.png"));
        assert_eq!(viewer.caption.as_deref(), Some("Sunset"));

        viewer.close();
        assert!(!viewer.is_open);
        assert_eq!(viewer.media_url, None);
    }
}
