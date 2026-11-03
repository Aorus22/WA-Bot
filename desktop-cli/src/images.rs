//! Pictures in the terminal: a small cache of decoded images (thumbnails in
//! bubbles, the viewer, profile photos) on top of `ratatui-image`.

use std::collections::{HashMap, VecDeque};

use image::DynamicImage;
use ratatui::layout::Size;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::sliced::SlicedProtocol;
use ratatui_image::{FilterType, Resize};
use wabot_backend_client::Message;

/// Which cache an image belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Message pictures: big, few kept.
    Media,
    /// Profile photos: tiny, many kept.
    Avatar,
    /// Sticker picker: small, many kept.
    Sticker,
}

/// Where a picture is shown. Each place gets its own protocol: one protocol keeps
/// one encoded size, so sharing it between a 4x2 avatar and a 12x6 profile photo
/// makes the terminal show both at the first size.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Slot {
    /// Thumbnail inside a message bubble.
    Bubble,
    /// Full-screen viewer.
    Viewer,
    /// Small thumbnail in the info drawer's media list.
    Row,
    /// Profile photo in the chat list.
    Avatar,
    /// Profile photo in the info drawer.
    Profile,
    /// Profile photo in the conversation header (shown next to the list's own).
    Header,
    /// A sticker in the picker's grid.
    Sticker,
}

impl Slot {
    /// Longest side of the copy this place needs (None: as decoded).
    fn max_side(self) -> Option<u32> {
        match self {
            Slot::Bubble => Some(640),
            Slot::Row => Some(160),
            Slot::Viewer | Slot::Avatar | Slot::Profile | Slot::Header | Slot::Sticker => None,
        }
    }

    /// Small pictures are enlarged to their box; bubble thumbnails keep their size.
    fn resize(self) -> Resize {
        match self {
            Slot::Bubble | Slot::Viewer => Resize::Fit(Some(FilterType::Triangle)),
            Slot::Row | Slot::Avatar | Slot::Profile | Slot::Header | Slot::Sticker => Resize::Scale(Some(FilterType::Triangle)),
        }
    }
}

pub struct Ready {
    img: DynamicImage,
    /// Pixel size of the decoded image.
    pub w: u32,
    pub h: u32,
    protos: HashMap<Slot, StatefulProtocol>,
    /// Row-sliced copies, for pictures that scroll half out of view.
    sliced: HashMap<Slot, (Size, SlicedProtocol)>,
}

impl Ready {
    pub fn new(img: DynamicImage) -> Self {
        Self { w: img.width(), h: img.height(), img, protos: HashMap::new(), sliced: HashMap::new() }
    }

    fn source_for(&self, slot: Slot) -> DynamicImage {
        match slot.max_side() {
            Some(m) if self.img.width() > m || self.img.height() > m => self.img.thumbnail(m, m),
            _ => self.img.clone(),
        }
    }

    /// Whether `sliced(slot, size)` is already made (so it costs nothing).
    pub fn is_built(&self, slot: Slot, size: Size) -> bool {
        self.sliced.get(&slot).is_some_and(|(s, _)| *s == size)
    }

    /// The row-sliced protocol for `slot` at exactly `size` cells, rebuilt when the
    /// size changes. This is what lets a picture show partly while scrolling.
    pub fn sliced(&mut self, picker: &Picker, slot: Slot, size: Size) -> Option<&SlicedProtocol> {
        if self.sliced.get(&slot).is_none_or(|(s, _)| *s != size) {
            let built = SlicedProtocol::new_with_resize(picker, self.source_for(slot), size, slot.resize()).ok()?;
            self.sliced.insert(slot, (size, built));
        }
        self.sliced.get(&slot).map(|(_, p)| p)
    }

    /// The protocol for `slot`, made on first use.
    pub fn proto(&mut self, picker: &Picker, slot: Slot) -> &mut StatefulProtocol {
        let img = &self.img;
        self.protos.entry(slot).or_insert_with(|| {
            let source = match slot.max_side() {
                Some(m) if img.width() > m || img.height() > m => img.thumbnail(m, m),
                _ => img.clone(),
            };
            picker.new_resize_protocol(source)
        })
    }
}

pub enum Img {
    Loading,
    Failed,
    Ready(Ready),
}

pub struct ImageCache {
    cap: usize,
    pub max_dim: u32,
    map: HashMap<String, Img>,
    order: VecDeque<String>,
}

impl ImageCache {
    pub fn new(cap: usize, max_dim: u32) -> Self {
        Self { cap, max_dim, map: HashMap::new(), order: VecDeque::new() }
    }

    pub fn get(&self, url: &str) -> Option<&Img> {
        self.map.get(url)
    }

    pub fn get_mut(&mut self, url: &str) -> Option<&mut Img> {
        self.map.get_mut(url)
    }

    /// Mark `url` as being fetched. False when it is already known (loading,
    /// loaded or failed), so the caller must not fetch it again.
    pub fn begin(&mut self, url: &str) -> bool {
        if self.map.contains_key(url) {
            return false;
        }
        self.map.insert(url.to_string(), Img::Loading);
        self.order.push_back(url.to_string());
        while self.order.len() > self.cap {
            // Oldest first; never drop one that is still on its way.
            let Some(pos) = self.order.iter().position(|u| !matches!(self.map.get(u), Some(Img::Loading))) else {
                break;
            };
            if let Some(old) = self.order.remove(pos) {
                self.map.remove(&old);
            }
        }
        true
    }

    pub fn finish(&mut self, url: &str, ready: Option<Ready>) {
        if let Some(slot) = self.map.get_mut(url) {
            *slot = ready.map_or(Img::Failed, Img::Ready);
        }
    }

    /// Forget failures so they are tried again (the `r` key).
    pub fn clear_failed(&mut self) {
        self.map.retain(|_, v| !matches!(v, Img::Failed));
        let map = &self.map;
        self.order.retain(|u| map.contains_key(u));
    }
}

/// Decode and shrink to `max_dim` on the longest side (never enlarges).
pub fn decode(bytes: &[u8], max_dim: u32) -> Result<DynamicImage, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    Ok(if img.width() > max_dim || img.height() > max_dim { img.thumbnail(max_dim, max_dim) } else { img })
}

/// The picture a message shows: the media itself, or a video's preview.
pub fn media_url(m: &Message) -> Option<String> {
    let url = match m.message_type.as_str() {
        "image" | "sticker" | "gif" => m.media_url.clone(),
        "video" => m.extra.as_ref().and_then(|e| e.thumbnail.clone()),
        _ => None,
    };
    url.filter(|u| !u.is_empty())
}

/// Spend one picture encoding from this frame's budget. False (and `pending` set, so
/// the next frame comes quickly) when it is used up.
pub fn take_budget(budget: &mut u8, pending: &mut bool) -> bool {
    if *budget == 0 {
        *pending = true;
        false
    } else {
        *budget -= 1;
        true
    }
}

/// Terminal graphics, or `None` when switched off. Call after entering the
/// alternate screen and before reading input events (it talks to the terminal).
pub fn make_picker(mode: &str) -> Option<Picker> {
    if mode == "off" {
        return None;
    }
    let mut picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    match mode {
        "halfblocks" => picker.set_protocol_type(ProtocolType::Halfblocks),
        "kitty" => picker.set_protocol_type(ProtocolType::Kitty),
        "sixel" => picker.set_protocol_type(ProtocolType::Sixel),
        "iterm2" => picker.set_protocol_type(ProtocolType::Iterm2),
        _ => {}
    }
    Some(picker)
}

/// Cells an `w`x`h` px image takes inside a `max_cols`x`max_rows` box at one
/// font size. With `enlarge` off a small image keeps its natural size.
pub fn fit(w: u32, h: u32, font: (u16, u16), max_cols: u16, max_rows: u16, enlarge: bool) -> (u16, u16) {
    let (fw, fh) = (u32::from(font.0.max(1)), u32::from(font.1.max(1)));
    let (w, h) = (w.max(1), h.max(1));
    let (box_w, box_h) = (u32::from(max_cols) * fw, u32::from(max_rows) * fh);
    let mut pw = if enlarge { box_w } else { w.min(box_w) };
    let mut ph = h * pw / w;
    if ph > box_h {
        ph = box_h;
        pw = w * ph / h;
    }
    let cols = pw.div_ceil(fw).clamp(1, u32::from(max_cols));
    let rows = ph.div_ceil(fh).clamp(1, u32::from(max_rows));
    (cols as u16, rows as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_keeps_aspect_and_never_exceeds_the_box() {
        // 800x400 into 40x14 cells of 10x20 px: width bound, 400x200 px -> 40x10.
        assert_eq!(fit(800, 400, (10, 20), 40, 14, false), (40, 10));
        // A tall image is cut by the height: 14 rows = 280 px, so 140 px wide = 14 cols.
        assert_eq!(fit(500, 1000, (10, 20), 40, 14, false), (14, 14));
        // A small image is not enlarged ...
        assert_eq!(fit(100, 100, (10, 20), 40, 14, false), (10, 5));
        // ... unless asked to (the viewer).
        assert_eq!(fit(100, 100, (10, 20), 40, 14, true), (28, 14));
    }

    #[test]
    fn cache_evicts_oldest_but_not_loading() {
        let mut c = ImageCache::new(2, 100);
        assert!(c.begin("a"));
        assert!(!c.begin("a"));
        c.finish("a", None);
        assert!(c.begin("b"));
        assert!(c.begin("c"));
        // "a" (failed, oldest) made room; "b" and "c" are still loading.
        assert!(c.get("a").is_none());
        assert!(c.get("b").is_some() && c.get("c").is_some());
        c.clear_failed();
        assert!(c.begin("a"));
    }

    #[test]
    fn media_urls_per_type() {
        let mut m = Message { media_url: Some("/media/a.jpg".into()), message_type: "image".into(), ..Default::default() };
        assert_eq!(media_url(&m).as_deref(), Some("/media/a.jpg"));
        m.message_type = "document".into();
        assert!(media_url(&m).is_none());
        m.message_type = "video".into();
        assert!(media_url(&m).is_none());
    }

    /// Real files from `$WABOT_SAMPLES` (a folder): every one must decode.
    /// `WABOT_SAMPLES=/some/dir cargo test decode_samples -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn decode_samples() {
        let Some(dir) = std::env::var_os("WABOT_SAMPLES") else { return };
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let bytes = std::fs::read(&path).unwrap();
            let img = decode(&bytes, 1280).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            println!("{}: {}x{}", path.display(), img.width(), img.height());
        }
    }

    #[test]
    fn each_place_gets_its_own_protocol() {
        let picker = Picker::halfblocks();
        let mut r = Ready::new(DynamicImage::new_rgba8(200, 100));
        let avatar = r.proto(&picker, Slot::Avatar) as *mut StatefulProtocol as usize;
        let profile = r.proto(&picker, Slot::Profile) as *mut StatefulProtocol as usize;
        let avatar_again = r.proto(&picker, Slot::Avatar) as *mut StatefulProtocol as usize;
        assert_ne!(avatar, profile, "a 4x2 avatar and a 12x6 profile photo must not share one");
        assert_eq!(avatar, avatar_again, "the same place keeps its protocol between frames");
    }
}
