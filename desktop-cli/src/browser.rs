//! The file chooser behind the attach menu: a small browser in the terminal that
//! works with the mouse (click a folder to enter it, a file to pick it) and the keyboard.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, Event};
use crate::attach::{self, AttachMode};
use crate::actions::Pasted;
use crate::text::{truncate, width};
use crate::theme::{self as c, local};

/// More than this in one folder is cut off (the list is for picking one file).
const MAX_ENTRIES: usize = 5000;

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Btn {
    Up,
    Home,
    Hidden,
    Cancel,
}

pub struct Browser {
    pub mode: AttachMode,
    pub dir: PathBuf,
    pub entries: Vec<Entry>,
    pub sel: usize,
    pub offset: usize,
    pub hidden: bool,
    pub error: Option<String>,
    // Set by the last draw, for the mouse:
    pub rect: Rect,
    pub list: Rect,
    pub buttons: Vec<(Rect, Btn)>,
}

impl Browser {
    pub fn new(mode: AttachMode, dir: PathBuf) -> Self {
        let mut b = Self {
            mode,
            dir: PathBuf::new(),
            entries: Vec::new(),
            sel: 0,
            offset: 0,
            hidden: false,
            error: None,
            rect: Rect::default(),
            list: Rect::default(),
            buttons: Vec::new(),
        };
        b.cd(dir);
        b
    }

    /// Go to `dir`. When it cannot be read the old folder stays and the reason is shown.
    pub fn cd(&mut self, dir: PathBuf) {
        match list_dir(&dir, self.mode, self.hidden) {
            Ok(entries) => {
                self.dir = dir;
                self.entries = entries;
                self.sel = 0;
                self.offset = 0;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub fn reload(&mut self) {
        let dir = self.dir.clone();
        self.cd(dir);
    }

    pub fn step(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        self.sel = (self.sel as isize + delta).clamp(0, self.entries.len() as isize - 1) as usize;
    }
}

/// Folders first, then the files this mode accepts; the parent first of all.
pub fn list_dir(dir: &Path, mode: AttachMode, hidden: bool) -> Result<Vec<Entry>, String> {
    let read = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for item in read.flatten().take(MAX_ENTRIES) {
        let name = item.file_name().to_string_lossy().into_owned();
        if !hidden && name.starts_with('.') {
            continue;
        }
        // `metadata` follows symlinks, so a link to a folder is a folder.
        let Ok(meta) = std::fs::metadata(item.path()) else { continue };
        let entry = Entry { name, path: item.path(), is_dir: meta.is_dir(), size: meta.len(), modified: meta.modified().ok() };
        if entry.is_dir {
            dirs.push(entry);
        } else if mode.accepts(&entry.name) {
            files.push(entry);
        }
    }
    let key = |e: &Entry| e.name.to_lowercase();
    dirs.sort_by_key(key);
    files.sort_by_key(key);
    let mut out = Vec::with_capacity(dirs.len() + files.len() + 1);
    if let Some(parent) = dir.parent() {
        out.push(Entry { name: "..".into(), path: parent.to_path_buf(), is_dir: true, size: 0, modified: None });
    }
    out.extend(dirs);
    out.extend(files);
    Ok(out)
}

pub fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < KB * KB {
        format!("{:.0} KB", b / KB)
    } else if b < KB * KB * KB {
        format!("{:.1} MB", b / KB / KB)
    } else {
        format!("{:.1} GB", b / KB / KB / KB)
    }
}

impl App {
    pub fn open_browser(&mut self, mode: AttachMode) {
        if self.open.is_none() {
            self.notice("Open a chat first");
            return;
        }
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
        let dir = self.last_dir.clone().filter(|d| d.is_dir()).unwrap_or(home);
        self.browser = Some(Browser::new(mode, dir));
    }

    /// Enter the highlighted folder, or pick the highlighted file.
    fn browser_open_selected(&mut self) {
        let Some(b) = self.browser.as_mut() else { return };
        let Some(entry) = b.entries.get(b.sel).cloned() else { return };
        if entry.is_dir {
            b.cd(entry.path);
            self.last_dir = Some(b.dir.clone());
            return;
        }
        let mode = b.mode;
        self.last_dir = Some(b.dir.clone());
        self.browser = None;
        // Reading a big file takes a moment: off the UI thread.
        let tx = self.tx.clone();
        self.rt.spawn_blocking(move || {
            let result = attach::pick_from_file(&entry.path, mode).map(Pasted::File);
            let _ = tx.send(Event::Pasted(result));
        });
    }

    fn browser_parent(&mut self) {
        if let Some(b) = self.browser.as_mut() {
            if let Some(parent) = b.dir.parent().map(Path::to_path_buf) {
                b.cd(parent);
                self.last_dir = Some(b.dir.clone());
            }
        }
    }

    fn browser_button(&mut self, btn: Btn) {
        match btn {
            Btn::Cancel => self.browser = None,
            Btn::Up => self.browser_parent(),
            Btn::Home => {
                if let (Some(b), Some(home)) = (self.browser.as_mut(), std::env::var_os("HOME")) {
                    b.cd(PathBuf::from(home));
                }
            }
            Btn::Hidden => {
                if let Some(b) = self.browser.as_mut() {
                    b.hidden = !b.hidden;
                    b.reload();
                }
            }
        }
    }

    pub fn key_browser(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Some(b) = self.browser.as_mut() else { return };
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.browser = None,
            KeyCode::Char('c') if ctrl => self.browser = None,
            KeyCode::Char('j') | KeyCode::Down => b.step(1),
            KeyCode::Char('k') | KeyCode::Up => b.step(-1),
            KeyCode::Char('d') if ctrl => b.step(10),
            KeyCode::Char('u') if ctrl => b.step(-10),
            KeyCode::PageDown => b.step(10),
            KeyCode::PageUp => b.step(-10),
            KeyCode::Char('g') | KeyCode::Home => b.sel = 0,
            KeyCode::Char('G') | KeyCode::End => b.sel = b.entries.len().saturating_sub(1),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => self.browser_open_selected(),
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => self.browser_parent(),
            KeyCode::Char('~') => self.browser_button(Btn::Home),
            KeyCode::Char('.') => self.browser_button(Btn::Hidden),
            _ => {}
        }
    }

    pub fn mouse_browser(&mut self, m: MouseEvent) {
        let Some(b) = self.browser.as_mut() else { return };
        let at = |r: Rect| m.column >= r.x && m.column < r.x + r.width && m.row >= r.y && m.row < r.y + r.height;
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(btn) = b.buttons.iter().find(|(r, _)| at(*r)).map(|(_, k)| *k) {
                    self.browser_button(btn);
                } else if at(b.list) {
                    let idx = b.offset + (m.row - b.list.y) as usize;
                    if idx < b.entries.len() {
                        b.sel = idx;
                        self.browser_open_selected();
                    }
                }
            }
            MouseEventKind::ScrollUp => b.offset = b.offset.saturating_sub(3),
            MouseEventKind::ScrollDown => {
                let room = b.list.height as usize;
                b.offset = (b.offset + 3).min(b.entries.len().saturating_sub(room));
            }
            _ => {}
        }
    }
}

pub fn draw_browser(f: &mut Frame, b: &mut Browser, area: Rect) {
    let w = (area.width * 85 / 100).clamp(40.min(area.width), 100);
    let h = (area.height * 85 / 100).clamp(14.min(area.height), 40);
    let rect = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    b.rect = rect;
    f.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(c::BLUE))
        .title(Span::styled(format!(" Choose {} ", b.mode.prompt()), Style::new().fg(c::FG).add_modifier(Modifier::BOLD)))
        .style(Style::new().bg(c::SURFACE));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let [path, tools, list, hint] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)]).areas(inner);

    // The folder, cut from the left so the end (the part that matters) stays.
    let shown = b.dir.display().to_string();
    let room = path.width.saturating_sub(2) as usize;
    let shown = if width(&shown) > room {
        let tail: String = shown.chars().rev().take(room.saturating_sub(1)).collect::<Vec<_>>().into_iter().rev().collect();
        format!("…{tail}")
    } else {
        shown
    };
    f.render_widget(Paragraph::new(Line::styled(format!(" {shown}"), Style::new().fg(c::YELLOW))), path);

    // Toolbar.
    let labels = [
        (Btn::Up, " ↑ Up ".to_string()),
        (Btn::Home, " ~ Home ".to_string()),
        (Btn::Hidden, format!(" . Hidden: {} ", if b.hidden { "on" } else { "off" })),
        (Btn::Cancel, " Cancel ".to_string()),
    ];
    let mut spans = vec![Span::raw(" ")];
    let mut x = tools.x + 1;
    b.buttons.clear();
    for (btn, label) in labels {
        let lw = width(&label) as u16;
        if x + lw > tools.x + tools.width {
            break;
        }
        spans.push(Span::styled(label, Style::new().fg(c::FG).bg(c::SELECT)));
        spans.push(Span::raw(" "));
        b.buttons.push((Rect::new(x, tools.y, lw, 1), btn));
        x += lw + 1;
    }
    f.render_widget(Paragraph::new(Line::from(spans)), tools);

    // The list; the highlighted row stays in view.
    b.list = list;
    let rows = list.height as usize;
    if b.sel < b.offset {
        b.offset = b.sel;
    } else if b.sel >= b.offset + rows {
        b.offset = b.sel + 1 - rows;
    }
    b.offset = b.offset.min(b.entries.len().saturating_sub(rows));
    let w = list.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    for (i, e) in b.entries.iter().enumerate().skip(b.offset).take(rows) {
        let bg = if i == b.sel { c::SELECT } else { c::SURFACE };
        let (name, style) = if e.is_dir {
            (format!("{}/", e.name), Style::new().fg(c::BLUE).add_modifier(Modifier::BOLD))
        } else {
            (e.name.clone(), Style::new().fg(c::FG))
        };
        let right = if e.is_dir || e.name == ".." {
            String::new()
        } else {
            let when = e.modified.and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).and_then(|d| local(d.as_millis() as i64));
            format!("{}  {}", when.map(|t| t.format("%-d %b %Y").to_string()).unwrap_or_default(), human_size(e.size))
        };
        let name = truncate(&name, w.saturating_sub(width(&right) + 4));
        let gap = w.saturating_sub(1 + width(&name) + width(&right) + 1);
        lines.push(
            Line::from(vec![
                Span::raw(" "),
                Span::styled(name, style),
                Span::raw(" ".repeat(gap)),
                Span::styled(right, Style::new().fg(c::DIM)),
                Span::raw(" "),
            ])
            .style(Style::new().bg(bg)),
        );
    }
    if b.entries.is_empty() {
        lines.push(Line::styled(" Nothing here you can send", Style::new().fg(c::DIM)));
    }
    f.render_widget(Paragraph::new(lines), list);

    let hint_text = match &b.error {
        Some(e) => Span::styled(format!(" {}", truncate(e, hint.width as usize - 2)), Style::new().fg(c::RED)),
        None => Span::styled(" click a folder to open it, a file to pick it · h back · esc close", Style::new().fg(c::DIM)),
    };
    f.render_widget(Paragraph::new(Line::from(hint_text)), hint);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        // Tests run in parallel: every call gets its own folder.
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("wabot-browser-{}-{n}", std::process::id()));
        std::fs::create_dir_all(dir.join("Zeta")).unwrap();
        std::fs::create_dir_all(dir.join("alpha")).unwrap();
        std::fs::write(dir.join("b.pdf"), b"x").unwrap();
        std::fs::write(dir.join("a.png"), b"x").unwrap();
        std::fs::write(dir.join("c.mp3"), b"x").unwrap();
        std::fs::write(dir.join(".secret.pdf"), b"x").unwrap();
        dir
    }

    fn names(v: &[Entry]) -> Vec<String> {
        v.iter().map(|e| e.name.clone()).collect()
    }

    #[test]
    fn lists_folders_first_and_only_what_the_mode_accepts() {
        let dir = scratch();
        let all = list_dir(&dir, AttachMode::Document, false).unwrap();
        assert_eq!(names(&all), ["..", "alpha", "Zeta", "a.png", "b.pdf", "c.mp3"], "hidden file left out, folders first");
        let media = list_dir(&dir, AttachMode::Media, false).unwrap();
        assert_eq!(names(&media), ["..", "alpha", "Zeta", "a.png", "c.mp3"]);
        let audio = list_dir(&dir, AttachMode::Audio, false).unwrap();
        assert_eq!(names(&audio), ["..", "alpha", "Zeta", "c.mp3"]);
        let shown = list_dir(&dir, AttachMode::Document, true).unwrap();
        assert!(names(&shown).contains(&".secret.pdf".to_string()));
        assert!(list_dir(&dir.join("nope"), AttachMode::Media, false).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_folder_that_cannot_be_read_leaves_the_browser_where_it_was() {
        let dir = scratch();
        let mut b = Browser::new(AttachMode::Document, dir.clone());
        b.cd(dir.join("missing"));
        assert_eq!(b.dir, dir);
        assert!(b.error.is_some());
        b.cd(dir.join("alpha"));
        assert!(b.error.is_none() && b.dir.ends_with("alpha"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sizes_are_human() {
        assert_eq!(human_size(900), "900 B");
        assert_eq!(human_size(2048), "2 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
    }
}
