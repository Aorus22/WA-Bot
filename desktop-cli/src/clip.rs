//! Clipboard and dropped files: what `Ctrl-v` attaches and what "Copy" writes.
//!
//! The terminal cannot paste a picture (bracketed paste is text only), so the
//! clipboard is read directly: `arboard` first, then `wl-paste` / `xclip`.

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use image::{DynamicImage, ImageFormat, RgbaImage};

use crate::attach::{self, kind_of, AttachKind, AttachMode, Picked};

/// Longest side we keep of a pasted screenshot; WhatsApp recompresses anyway.
const MAX_SIDE: u32 = 2560;

/// Keeps the clipboard owner alive: on Linux what we copy vanishes with it.
static HELD: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);

fn with_held<T>(f: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>) -> Result<T, String> {
    let mut held = HELD.lock().map_err(|_| "clipboard is busy".to_string())?;
    if held.is_none() {
        *held = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
    }
    f(held.as_mut().expect("just set")).map_err(|e| e.to_string())
}

/// Run `cmd args`, return stdout when it succeeded and printed something.
fn capture(cmd: &str, args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new(cmd).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    (out.status.success() && !out.stdout.is_empty()).then_some(out.stdout)
}

/// Run `cmd args` with `data` on stdin; true when it exited cleanly.
fn feed(cmd: &str, args: &[&str], data: &[u8]) -> bool {
    let Ok(mut child) = Command::new(cmd).args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(data).is_err() {
            return false;
        }
    }
    child.wait().is_ok_and(|s| s.success())
}

/// The picture on the clipboard, encoded as PNG.
pub fn pick_from_clipboard() -> Result<Picked, String> {
    let img = read_clipboard_image()?;
    let img = shrink(img);
    Ok(Picked {
        bytes: encode_png(&img)?,
        name: "pasted-image.png".into(),
        mime: "image/png",
        kind: AttachKind::Image,
        preview: Some(img),
    })
}

fn read_clipboard_image() -> Result<DynamicImage, String> {
    if let Ok(data) = with_held(|cb| cb.get_image()) {
        let (w, h) = (data.width as u32, data.height as u32);
        if let Some(buf) = RgbaImage::from_raw(w, h, data.bytes.into_owned()) {
            return Ok(DynamicImage::ImageRgba8(buf));
        }
    }
    let tools: [(&str, &[&str]); 2] = [
        ("wl-paste", &["--no-newline", "--type", "image/png"]),
        ("xclip", &["-selection", "clipboard", "-t", "image/png", "-o"]),
    ];
    for (cmd, args) in tools {
        if let Some(bytes) = capture(cmd, args) {
            if let Ok(img) = image::load_from_memory(&bytes) {
                return Ok(img);
            }
        }
    }
    Err("No picture on the clipboard".into())
}

fn shrink(img: DynamicImage) -> DynamicImage {
    if img.width() > MAX_SIDE || img.height() > MAX_SIDE {
        img.thumbnail(MAX_SIDE, MAX_SIDE)
    } else {
        img
    }
}

fn encode_png(img: &DynamicImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}

/// A file dropped onto the terminal: pictures, videos and audio go as media,
/// everything else as a document.
pub fn pick_from_path(path: &Path) -> Result<Picked, String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mode = if kind_of(&name) == AttachKind::Document { AttachMode::Document } else { AttachMode::Media };
    attach::pick_from_file(path, mode)
}

/// Pasted or dropped text that names one existing file: the path.
/// Handles quotes, `file://`, `%20` and backslash-escaped spaces.
pub fn file_path_in(text: &str) -> Option<PathBuf> {
    let t = text.trim();
    if t.is_empty() || t.contains('\n') {
        return None;
    }
    let t = t.trim_matches(|c| c == '\'' || c == '"');
    let t = t.strip_prefix("file://").unwrap_or(t);
    let t = t.replace("%20", " ").replace("\\ ", " ");
    let path = PathBuf::from(t);
    (path.is_absolute() && path.is_file()).then_some(path)
}

pub fn read_text() -> Result<String, String> {
    if let Ok(t) = with_held(|cb| cb.get_text()) {
        if !t.is_empty() {
            return Ok(t);
        }
    }
    let tools: [(&str, &[&str]); 2] =
        [("wl-paste", &["--no-newline"]), ("xclip", &["-selection", "clipboard", "-o"])];
    tools
        .iter()
        .find_map(|(cmd, args)| capture(cmd, args))
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .ok_or_else(|| "The clipboard is empty".to_string())
}

pub fn write_text(text: &str) -> Result<(), String> {
    // wl-copy / xclip hand the text to a background owner that outlives us.
    if feed("wl-copy", &[], text.as_bytes()) || feed("xclip", &["-selection", "clipboard", "-i"], text.as_bytes()) {
        return Ok(());
    }
    with_held(|cb| cb.set_text(text.to_string()))
}

/// Hand an address to the desktop (`xdg-open`, `open` on a Mac); does not wait for it.
pub fn open_url(url: &str) -> Result<(), String> {
    let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    Command::new(opener)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|mut child| {
            // Reap it when it ends, so it does not linger as a zombie.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        })
        .map_err(|e| format!("{opener}: {e}"))
}

pub fn write_image(img: &DynamicImage) -> Result<(), String> {
    let png = encode_png(img)?;
    if feed("wl-copy", &["--type", "image/png"], &png) || feed("xclip", &["-selection", "clipboard", "-t", "image/png", "-i"], &png) {
        return Ok(());
    }
    let rgba = img.to_rgba8();
    let data = arboard::ImageData {
        width: rgba.width() as usize,
        height: rgba.height() as usize,
        bytes: std::borrow::Cow::Owned(rgba.into_raw()),
    };
    with_held(|cb| cb.set_image(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_dropped_file_paths() {
        let dir = std::env::temp_dir().join(format!("wabot-clip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a b.png");
        image::RgbaImage::from_pixel(4, 3, image::Rgba([9, 9, 9, 255])).save(&file).unwrap();
        let pdf = dir.join("laporan akhir.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();

        let plain = file.to_string_lossy().into_owned();
        assert_eq!(file_path_in(&plain), Some(file.clone()));
        assert_eq!(file_path_in(&format!("'{plain}'")), Some(file.clone()));
        assert_eq!(file_path_in(&format!("file://{}", plain.replace(' ', "%20"))), Some(file.clone()));
        assert_eq!(file_path_in(&plain.replace(' ', "\\ ")), Some(file.clone()));
        assert!(file_path_in("just some text").is_none());
        assert!(file_path_in(&format!("{plain}\nsecond line")).is_none());
        assert!(file_path_in("/does/not/exist.png").is_none());
        assert_eq!(file_path_in(&pdf.to_string_lossy()), Some(pdf.clone()), "documents can be dropped too");

        let picked = pick_from_path(&file).unwrap();
        assert_eq!((picked.mime, picked.size(), picked.kind), ("image/png", Some((4, 3)), AttachKind::Image));
        assert_eq!(picked.name, "a b.png");
        let doc = pick_from_path(&pdf).unwrap();
        assert_eq!((doc.kind, doc.mime), (AttachKind::Document, "application/pdf"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn big_pictures_are_shrunk_and_encoded() {
        let big = DynamicImage::ImageRgba8(RgbaImage::from_pixel(3000, 1000, image::Rgba([1, 2, 3, 255])));
        let small = shrink(big);
        assert_eq!(small.width(), MAX_SIDE);
        assert!(encode_png(&small).unwrap().starts_with(&[0x89, b'P', b'N', b'G']));
    }
}
