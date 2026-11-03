//! What can be attached: pictures, videos, GIFs, audio and any other file, with the
//! `/send-media` type and MIME the backend expects (same table as the gpui app).

use std::path::Path;

use image::DynamicImage;

/// Largest file we read into memory and upload.
pub const MAX_FILE: u64 = 64 * 1024 * 1024;
/// Longest side kept of a picture's preview.
const PREVIEW_SIDE: u32 = 1600;

/// The `type` of `/send-media`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AttachKind {
    Image,
    Video,
    Gif,
    Audio,
    Document,
}

impl AttachKind {
    pub fn media_type(self) -> &'static str {
        match self {
            AttachKind::Image => "image",
            AttachKind::Video => "video",
            AttachKind::Gif => "gif",
            AttachKind::Audio => "audio",
            AttachKind::Document => "document",
        }
    }

    /// How it is named to the user ("Send picture to …").
    pub fn label(self) -> &'static str {
        match self {
            AttachKind::Image => "picture",
            AttachKind::Video => "video",
            AttachKind::Gif => "GIF",
            AttachKind::Audio => "audio",
            AttachKind::Document => "document",
        }
    }

    /// WhatsApp has no caption for audio.
    pub fn allows_caption(self) -> bool {
        self != AttachKind::Audio
    }
}

/// The menu entry an attachment came from: it decides which files the browser lists.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AttachMode {
    /// Pictures, videos and audio, told apart by file name.
    Media,
    /// Anything, always sent as a document.
    Document,
    Audio,
    Gif,
}

impl AttachMode {
    pub fn prompt(self) -> &'static str {
        match self {
            AttachMode::Media => "a photo, video or audio file",
            AttachMode::Document => "a document",
            AttachMode::Audio => "an audio file",
            AttachMode::Gif => "a GIF",
        }
    }

    pub fn accepts(self, name: &str) -> bool {
        match self {
            AttachMode::Media => kind_of(name) != AttachKind::Document,
            AttachMode::Document => true,
            AttachMode::Audio => kind_of(name) == AttachKind::Audio,
            AttachMode::Gif => kind_of(name) == AttachKind::Gif,
        }
    }
}

pub fn ext_of(name: &str) -> String {
    name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

/// What a file is, from its name.
pub fn kind_of(name: &str) -> AttachKind {
    match ext_of(name).as_str() {
        "gif" => AttachKind::Gif,
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "heic" | "heif" | "avif" | "tif" | "tiff" => AttachKind::Image,
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "3gp" | "m4v" | "mpeg" | "mpg" => AttachKind::Video,
        "mp3" | "ogg" | "oga" | "opus" | "m4a" | "wav" | "aac" | "amr" | "flac" | "weba" => AttachKind::Audio,
        _ => AttachKind::Document,
    }
}

pub fn mime_of(name: &str) -> &'static str {
    match ext_of(name).as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "3gp" => "video/3gpp",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "aac" => "audio/aac",
        "flac" => "audio/flac",
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "txt" | "log" | "md" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "xml" => "application/xml",
        "zip" => "application/zip",
        "rar" => "application/vnd.rar",
        "7z" => "application/x-7z-compressed",
        _ => "application/octet-stream",
    }
}

/// A file ready to send.
#[derive(Debug)]
pub struct Picked {
    pub bytes: Vec<u8>,
    pub name: String,
    pub mime: &'static str,
    pub kind: AttachKind,
    /// Decoded picture for the preview (pictures and GIFs only).
    pub preview: Option<DynamicImage>,
}

impl Picked {
    /// Pixel size of the preview, when there is one.
    pub fn size(&self) -> Option<(u32, u32)> {
        self.preview.as_ref().map(|p| (p.width(), p.height()))
    }
}

pub fn shrink(img: DynamicImage) -> DynamicImage {
    if img.width() > PREVIEW_SIDE || img.height() > PREVIEW_SIDE {
        img.thumbnail(PREVIEW_SIDE, PREVIEW_SIDE)
    } else {
        img
    }
}

/// Read `path` for sending. The file is sent as it is (no re-encoding).
pub fn pick_from_file(path: &Path, mode: AttachMode) -> Result<Picked, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("That is not a file".into());
    }
    if meta.len() > MAX_FILE {
        return Err(format!("That file is larger than {} MB", MAX_FILE / 1024 / 1024));
    }
    let name = path.file_name().map_or_else(|| "file".to_string(), |n| n.to_string_lossy().into_owned());
    if !mode.accepts(&name) {
        return Err(format!("That is not {}", mode.prompt()));
    }
    let kind = match mode {
        AttachMode::Document => AttachKind::Document,
        AttachMode::Audio => AttachKind::Audio,
        AttachMode::Gif => AttachKind::Gif,
        AttachMode::Media => kind_of(&name),
    };
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let preview = match kind {
        AttachKind::Image | AttachKind::Gif => {
            Some(shrink(image::load_from_memory(&bytes).map_err(|e| format!("Cannot read that picture: {e}"))?))
        }
        _ => None,
    };
    Ok(Picked { bytes, mime: mime_of(&name), name, kind, preview })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_map_to_kinds_mimes_and_wire_types() {
        assert_eq!(kind_of("a.JPG"), AttachKind::Image);
        assert_eq!(kind_of("a.mp4"), AttachKind::Video);
        assert_eq!(kind_of("a.gif"), AttachKind::Gif);
        assert_eq!(kind_of("a.opus"), AttachKind::Audio);
        assert_eq!(kind_of("laporan.pdf"), AttachKind::Document);
        assert_eq!(kind_of("noext"), AttachKind::Document);
        assert_eq!(mime_of("x.docx"), "application/vnd.openxmlformats-officedocument.wordprocessingml.document");
        assert_eq!(mime_of("x.unknown"), "application/octet-stream");
        assert_eq!(AttachKind::Gif.media_type(), "gif");
        assert_eq!(AttachKind::Document.media_type(), "document");
        assert!(!AttachKind::Audio.allows_caption() && AttachKind::Video.allows_caption());
    }

    #[test]
    fn each_mode_lists_what_it_should() {
        assert!(AttachMode::Media.accepts("a.png") && AttachMode::Media.accepts("a.mp3") && !AttachMode::Media.accepts("a.pdf"));
        assert!(AttachMode::Document.accepts("a.pdf") && AttachMode::Document.accepts("a.png"));
        assert!(AttachMode::Audio.accepts("a.m4a") && !AttachMode::Audio.accepts("a.mp4"));
        assert!(AttachMode::Gif.accepts("a.gif") && !AttachMode::Gif.accepts("a.png"));
    }

    #[test]
    fn files_are_read_as_the_chosen_kind() {
        let dir = std::env::temp_dir().join(format!("wabot-attach-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("a.png");
        image::RgbaImage::from_pixel(8, 4, image::Rgba([1, 2, 3, 255])).save(&png).unwrap();
        let pdf = dir.join("laporan.pdf");
        std::fs::write(&pdf, b"%PDF-1.4 fake").unwrap();

        let p = pick_from_file(&png, AttachMode::Media).unwrap();
        assert_eq!((p.kind, p.mime, p.size()), (AttachKind::Image, "image/png", Some((8, 4))));
        // The same picture picked as a document goes out as a document, without a preview of its own.
        let d = pick_from_file(&png, AttachMode::Document).unwrap();
        assert_eq!(d.kind, AttachKind::Document);
        let p = pick_from_file(&pdf, AttachMode::Document).unwrap();
        assert_eq!((p.kind, p.mime, p.name.as_str(), p.size()), (AttachKind::Document, "application/pdf", "laporan.pdf", None));
        assert!(pick_from_file(&pdf, AttachMode::Media).unwrap_err().contains("not a photo"));
        assert!(pick_from_file(&dir, AttachMode::Document).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
