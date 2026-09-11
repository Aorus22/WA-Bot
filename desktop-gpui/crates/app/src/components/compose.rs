//! Compose-area helpers for the chat view: the attach (+) menu, its popovers
//! (emoji / sticker), and the poll, location and contact dialogs.
//!
//! Mirrors the web client's `ChatArea` plus-menu and `ChatComposeDialogs`,
//! keeping the state and validation logic out of the (already large) view so
//! it can be unit tested.

use wabot_backend_client::dto::StickerFavorite;

/// Which panel the attach (+) button currently shows below it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AttachPanel {
    /// Nothing open.
    #[default]
    None,
    /// The attach menu itself (Emoji, Sticker, Media, …).
    Menu,
    /// Emoji picker grid.
    Emoji,
    /// Sticker picker grid.
    Sticker,
}

impl AttachPanel {
    /// Toggle the attach area: closes whatever panel is open, otherwise opens
    /// the menu itself.
    pub fn toggle_menu(&mut self) {
        *self = if *self == AttachPanel::None {
            AttachPanel::Menu
        } else {
            AttachPanel::None
        };
    }

    pub fn is_open(&self) -> bool {
        *self != AttachPanel::None
    }
}

/// Modal dialogs opened from the attach menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ComposeDialog {
    Poll,
    Location,
    Contact,
}

/// Attachment flavours offered by the menu, mapped onto the `/send-media`
/// `type` field accepted by the backend.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MediaKind {
    /// Images, videos and audio (`image/*,video/*,audio/*` on the web).
    Media,
    Document,
    Audio,
    Gif,
}

impl MediaKind {
    /// Title shown in the native file prompt.
    pub fn prompt(self) -> &'static str {
        match self {
            MediaKind::Media => "Pilih media (gambar, video, audio)",
            MediaKind::Document => "Pilih dokumen",
            MediaKind::Audio => "Pilih audio",
            MediaKind::Gif => "Pilih GIF",
        }
    }

    /// Wire media type for `/send-media`. The backend treats anything that is
    /// not image/video/gif/audio as a document.
    pub fn media_type(self, file_name: &str) -> &'static str {
        match self {
            MediaKind::Document => "document",
            MediaKind::Audio => "audio",
            MediaKind::Gif => "gif",
            MediaKind::Media => {
                if is_audio_file(file_name) {
                    "audio"
                } else if is_video_file(file_name) {
                    "video"
                } else {
                    "image"
                }
            }
        }
    }

    /// Message `type` used by the optimistic local bubble (matches the web's
    /// `effectiveType`).
    pub fn optimistic_type(self, media_type: &str) -> &'static str {
        match media_type {
            "audio" => "audio",
            "video" => "video",
            "gif" => "gif",
            "document" => "document",
            _ => "image",
        }
    }

    /// Placeholder content for the optimistic bubble (web sends `[Image]`,
    /// `[Video]`, `[Audio]` and the file name for documents).
    pub fn optimistic_content(self, file_name: &str, media_type: &str) -> String {
        match media_type {
            "image" => "[Image]".to_string(),
            "video" => "[Video]".to_string(),
            "audio" => "[Audio]".to_string(),
            "gif" => "[GIF]".to_string(),
            _ => file_name.to_string(),
        }
    }
}

/// Lower-cased extension of a file name (without the dot).
pub fn extension_lower(file_name: &str) -> String {
    file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Audio extension / MIME recognition, mirroring the web's `isAudioFile`.
pub fn is_audio_file(file_name: &str) -> bool {
    matches!(
        extension_lower(file_name).as_str(),
        "mp3" | "ogg" | "oga" | "opus" | "m4a" | "wav" | "aac" | "amr" | "flac" | "weba"
    )
}

/// Whether a message body is a bare document file name (web's document
/// attachments carry the file name as their content). Requires a single
/// token so ordinary text ending in ".pdf" is not mistaken for a file.
pub fn looks_like_document_name(content: &str) -> bool {
    let name = content.trim();
    if name.is_empty() || name.split_whitespace().count() != 1 {
        return false;
    }
    matches!(
        extension_lower(name).as_str(),
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "csv" | "rtf" | "odt"
            | "ods" | "zip" | "rar" | "7z" | "json" | "xml" | "apk" | "epub"
    )
}

/// Video extension recognition.
pub fn is_video_file(file_name: &str) -> bool {
    matches!(
        extension_lower(file_name).as_str(),
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "3gp" | "m4v" | "mpeg" | "mpg"
    )
}

// ---------------------------------------------------------------------------
// Markdown mode (web's `encodeMarkdown`: `{{md:<base64>}}`)
// ---------------------------------------------------------------------------

pub const MD_PREFIX: &str = "{{md:";
pub const MD_SUFFIX: &str = "}}";

/// Encode compose text as the wire format the web/desktop renderers decode.
pub fn encode_markdown(text: &str) -> String {
    format!("{MD_PREFIX}{}{MD_SUFFIX}", base64_encode(text.as_bytes()))
}

/// Standard base64 encoding (no line breaks, `=` padded).
pub fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);

        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Text to send over the wire, applying markdown encoding when the mode is on.
pub fn wire_text(text: &str, markdown_mode: bool) -> String {
    if markdown_mode {
        encode_markdown(text)
    } else {
        text.to_string()
    }
}

// ---------------------------------------------------------------------------
// Dialog drafts
// ---------------------------------------------------------------------------

/// Poll composer state (question, 2..=12 options, multi-select flag).
#[derive(Clone, Debug, PartialEq)]
pub struct PollDraft {
    pub question: String,
    pub options: Vec<String>,
    pub multi_select: bool,
}

pub const POLL_MAX_OPTIONS: usize = 12;
pub const POLL_MIN_OPTIONS: usize = 2;

impl Default for PollDraft {
    fn default() -> Self {
        Self {
            question: String::new(),
            options: vec![String::new(), String::new()],
            multi_select: false,
        }
    }
}

impl PollDraft {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an empty option, capped like the web client (12).
    pub fn add_option(&mut self) -> bool {
        if self.options.len() >= POLL_MAX_OPTIONS {
            return false;
        }
        self.options.push(String::new());
        true
    }

    /// Remove an option, keeping at least two.
    pub fn remove_option(&mut self, index: usize) {
        if self.options.len() > POLL_MIN_OPTIONS && index < self.options.len() {
            self.options.remove(index);
        }
    }

    /// `(question, options, multi_select)` once valid.
    pub fn validated(&self) -> Result<(String, Vec<String>, bool), &'static str> {
        let question = self.question.trim().to_string();
        if question.is_empty() {
            return Err("Poll perlu pertanyaan");
        }
        let options: Vec<String> = self
            .options
            .iter()
            .map(|o| o.trim().to_string())
            .filter(|o| !o.is_empty())
            .collect();
        if options.len() < POLL_MIN_OPTIONS {
            return Err("Poll minimal 2 opsi");
        }
        Ok((question, options, self.multi_select))
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Location composer state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocationDraft {
    pub latitude: String,
    pub longitude: String,
    pub name: String,
    pub address: String,
    pub live: bool,
}

impl LocationDraft {
    pub fn new() -> Self {
        Self::default()
    }

    /// `(lat, lng, name, address, live)` once valid.
    pub fn validated(&self) -> Result<(f64, f64, String, String, bool), &'static str> {
        let latitude: f64 = self
            .latitude
            .trim()
            .parse()
            .map_err(|_| "Latitude tidak valid")?;
        let longitude: f64 = self
            .longitude
            .trim()
            .parse()
            .map_err(|_| "Longitude tidak valid")?;
        if !latitude.is_finite() || !longitude.is_finite() {
            return Err("Koordinat tidak valid");
        }
        Ok((
            latitude,
            longitude,
            self.name.trim().to_string(),
            self.address.trim().to_string(),
            self.live,
        ))
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Contact composer state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContactDraft {
    pub name: String,
    pub phone: String,
}

impl ContactDraft {
    pub fn new() -> Self {
        Self::default()
    }

    /// `(name, phone)` once valid.
    pub fn validated(&self) -> Result<(String, String), &'static str> {
        let name = self.name.trim().to_string();
        let phone = self.phone.trim().to_string();
        if name.is_empty() || phone.is_empty() {
            return Err("Nama dan nomor telepon wajib diisi");
        }
        Ok((name, phone))
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Favourite stickers shown by the sticker picker.
#[derive(Clone, Default, Debug)]
pub struct StickerPickerState {
    pub is_loading: bool,
    /// Set once a fetch finished so the picker does not refetch every open.
    pub loaded: bool,
    pub favorites: Vec<StickerFavorite>,
}

impl StickerPickerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_loading(&mut self) {
        self.is_loading = true;
    }

    pub fn set_favorites(&mut self, favorites: Vec<StickerFavorite>) {
        self.favorites = favorites;
        self.is_loading = false;
        self.loaded = true;
    }

    pub fn failed(&mut self) {
        self.is_loading = false;
        self.loaded = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::message_bubble::MessageBubbleHelper;

    #[test]
    fn attach_panel_toggle_closes_popovers() {
        let mut panel = AttachPanel::None;
        assert!(!panel.is_open());
        panel.toggle_menu();
        assert_eq!(panel, AttachPanel::Menu);
        panel.toggle_menu();
        assert_eq!(panel, AttachPanel::None);

        // Any open panel closes on the next toggle.
        panel = AttachPanel::Emoji;
        panel.toggle_menu();
        assert_eq!(panel, AttachPanel::None);
    }

    #[test]
    fn media_type_detection_matches_web() {
        assert_eq!(MediaKind::Media.media_type("photo.PNG"), "image");
        assert_eq!(MediaKind::Media.media_type("clip.webm"), "video");
        assert_eq!(MediaKind::Media.media_type("note.mp3"), "audio");
        assert_eq!(MediaKind::Media.media_type("no-extension"), "image");
        assert_eq!(MediaKind::Document.media_type("report.pdf"), "document");
        assert_eq!(MediaKind::Audio.media_type("voice.ogg"), "audio");
        assert_eq!(MediaKind::Gif.media_type("loop.gif"), "gif");

        assert!(is_audio_file("a.M4A"));
        assert!(!is_audio_file("a.mp4"));
        assert!(is_video_file("a.mkv"));
        assert!(!is_video_file("a.mkvz"));
    }

    #[test]
    fn document_name_detection_is_conservative() {
        assert!(looks_like_document_name("kosongan.docx"));
        assert!(looks_like_document_name(" data.JSON "));
        // Only a bare file name counts; captions and sentences are left alone
        // (real document messages carry `type == "document"` anyway).
        assert!(!looks_like_document_name("Laporan Q3.pdf"));
        assert!(!looks_like_document_name("lihat file.pdf"));
        assert!(!looks_like_document_name("halo"));
        assert!(!looks_like_document_name(""));
    }

    #[test]
    fn optimistic_content_matches_web_placeholders() {
        assert_eq!(MediaKind::Media.optimistic_content("x.png", "image"), "[Image]");
        assert_eq!(MediaKind::Media.optimistic_content("x.mp4", "video"), "[Video]");
        assert_eq!(MediaKind::Media.optimistic_content("x.mp3", "audio"), "[Audio]");
        assert_eq!(MediaKind::Document.optimistic_content("report.pdf", "document"), "report.pdf");
        assert_eq!(MediaKind::Gif.optimistic_content("x.gif", "gif"), "[GIF]");
        assert_eq!(MediaKind::Media.optimistic_type("image"), "image");
        assert_eq!(MediaKind::Media.optimistic_type("document"), "document");
    }

    #[test]
    fn markdown_round_trips_through_renderer() {
        let encoded = wire_text("Halo *dunia* 🎉", true);
        assert!(encoded.starts_with(MD_PREFIX));
        assert!(encoded.ends_with(MD_SUFFIX));
        assert_eq!(
            MessageBubbleHelper::decode_content(&encoded),
            "Halo *dunia* 🎉"
        );
        // Markdown mode off sends the raw text untouched.
        assert_eq!(wire_text("plain", false), "plain");
    }

    #[test]
    fn poll_draft_validates_and_caps_options() {
        let mut poll = PollDraft::new();
        assert!(poll.validated().is_err());

        poll.question = "Lunch?".into();
        poll.options[0] = "Nasi".into();
        assert!(poll.validated().is_err(), "needs two options");

        poll.options[1] = "Mie".into();
        let (q, opts, multi) = poll.validated().unwrap();
        assert_eq!(q, "Lunch?");
        assert_eq!(opts, vec!["Nasi".to_string(), "Mie".to_string()]);
        assert!(!multi);

        // Blank options are dropped.
        poll.options.push("   ".into());
        assert_eq!(poll.validated().unwrap().1.len(), 2);

        for _ in 0..20 {
            poll.add_option();
        }
        assert_eq!(poll.options.len(), POLL_MAX_OPTIONS);

        while poll.options.len() > 2 {
            poll.remove_option(0);
        }
        assert_eq!(poll.options.len(), 2);
        poll.remove_option(0);
        assert_eq!(poll.options.len(), 2, "never below the minimum");
    }

    #[test]
    fn location_draft_validates_coordinates() {
        let mut loc = LocationDraft::new();
        assert!(loc.validated().is_err());

        loc.latitude = "-6.2088".into();
        loc.longitude = "106.8456".into();
        loc.name = " Jakarta ".into();
        loc.live = true;
        let (lat, lng, name, address, live) = loc.validated().unwrap();
        assert!((lat + 6.2088).abs() < f64::EPSILON);
        assert!((lng - 106.8456).abs() < f64::EPSILON);
        assert_eq!(name, "Jakarta");
        assert_eq!(address, "");
        assert!(live);

        loc.latitude = "abc".into();
        assert!(loc.validated().is_err());
    }

    #[test]
    fn contact_draft_requires_name_and_phone() {
        let mut contact = ContactDraft::new();
        assert!(contact.validated().is_err());
        contact.name = "Budi".into();
        assert!(contact.validated().is_err());
        contact.phone = "+62812".into();
        assert_eq!(
            contact.validated().unwrap(),
            ("Budi".to_string(), "+62812".to_string())
        );
    }

    #[test]
    fn sticker_picker_tracks_loading_state() {
        let mut picker = StickerPickerState::new();
        picker.begin_loading();
        assert!(picker.is_loading);
        picker.failed();
        assert!(!picker.is_loading);
        assert!(picker.loaded);
    }
}
