//! Small forms of the attach menu: a poll, a location and a contact. Fields are
//! plain text lines; every check runs before anything is sent.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::actions::Then;
use crate::app::{App, LineEdit};
use crate::text::{truncate, width};
use crate::theme as c;

/// Poll options offered (WhatsApp allows more; this many fit one screen).
const POLL_OPTIONS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FormKind {
    Poll,
    Location,
    Contact,
}

impl FormKind {
    pub fn title(self) -> &'static str {
        match self {
            FormKind::Poll => "New poll",
            FormKind::Location => "Send location",
            FormKind::Contact => "Send contact",
        }
    }
}

pub struct Field {
    pub label: String,
    pub hint: String,
    pub edit: LineEdit,
}

impl Field {
    fn new(label: &str, hint: &str) -> Self {
        Self { label: label.into(), hint: hint.into(), edit: LineEdit::default() }
    }
}

pub struct Form {
    pub kind: FormKind,
    pub fields: Vec<Field>,
    pub focus: usize,
    /// Poll: several answers allowed.
    pub multi: bool,
    pub error: Option<String>,
    pub sending: bool,
    // Set by the last draw, for the mouse:
    pub rect: Rect,
    pub field_hits: Vec<(Rect, usize)>,
    pub multi_btn: Rect,
    pub send_btn: Rect,
    pub cancel_btn: Rect,
}

/// What a valid form turns into.
#[derive(Debug, PartialEq)]
pub enum Submission {
    Poll { question: String, options: Vec<String>, multi: bool },
    Location { lat: f64, lon: f64, name: Option<String>, address: Option<String> },
    Contact { name: String, phone: String },
}

impl Form {
    pub fn new(kind: FormKind) -> Self {
        let fields = match kind {
            FormKind::Poll => {
                let mut f = vec![Field::new("Question", "Ask something")];
                f.extend((1..=POLL_OPTIONS).map(|i| Field::new(&format!("Option {i}"), if i <= 2 { "required" } else { "optional" })));
                f
            }
            FormKind::Location => vec![
                Field::new("Latitude", "-6.2088"),
                Field::new("Longitude", "106.8456"),
                Field::new("Name", "optional"),
                Field::new("Address", "optional"),
            ],
            FormKind::Contact => vec![Field::new("Name", "Display name"), Field::new("Phone", "+62 812 3456 7890")],
        };
        Self {
            kind,
            fields,
            focus: 0,
            multi: false,
            error: None,
            sending: false,
            rect: Rect::default(),
            field_hits: Vec::new(),
            multi_btn: Rect::default(),
            send_btn: Rect::default(),
            cancel_btn: Rect::default(),
        }
    }

    fn text(&self, i: usize) -> String {
        self.fields.get(i).map(|f| f.edit.text.trim().to_string()).unwrap_or_default()
    }

    /// Check the fields; the message says what to fix.
    pub fn validate(&self) -> Result<Submission, String> {
        let opt = |i: usize| Some(self.text(i)).filter(|t| !t.is_empty());
        match self.kind {
            FormKind::Poll => {
                let question = self.text(0);
                if question.is_empty() {
                    return Err("Write the question".into());
                }
                let options: Vec<String> = (1..self.fields.len()).map(|i| self.text(i)).filter(|t| !t.is_empty()).collect();
                if options.len() < 2 {
                    return Err("A poll needs at least two options".into());
                }
                let mut seen = std::collections::HashSet::new();
                if let Some(dup) = options.iter().find(|o| !seen.insert(o.to_lowercase())) {
                    return Err(format!("\"{}\" is listed twice", truncate(dup, 20)));
                }
                Ok(Submission::Poll { question, options, multi: self.multi })
            }
            FormKind::Location => {
                let num = |i: usize, label: &str, lim: f64| -> Result<f64, String> {
                    let v: f64 = self.text(i).replace(',', ".").parse().map_err(|_| format!("{label} must be a number"))?;
                    if v.abs() > lim {
                        return Err(format!("{label} must be between -{lim} and {lim}"));
                    }
                    Ok(v)
                };
                Ok(Submission::Location { lat: num(0, "Latitude", 90.0)?, lon: num(1, "Longitude", 180.0)?, name: opt(2), address: opt(3) })
            }
            FormKind::Contact => {
                let name = self.text(0);
                let phone: String = self.text(1).chars().filter(|c| c.is_ascii_digit() || *c == '+').collect();
                if name.is_empty() {
                    return Err("Write the contact's name".into());
                }
                if phone.chars().filter(char::is_ascii_digit).count() < 6 {
                    return Err("Write a phone number".into());
                }
                Ok(Submission::Contact { name, phone })
            }
        }
    }
}

impl App {
    pub fn open_form(&mut self, kind: FormKind) {
        if self.open.is_none() {
            self.notice("Open a chat first");
            return;
        }
        self.form = Some(Form::new(kind));
    }

    /// Check and send. Problems are shown inside the form.
    fn submit_form(&mut self) {
        let Some(chat) = self.open.clone() else { return };
        let Some(form) = self.form.as_mut() else { return };
        if form.sending {
            return;
        }
        let submission = match form.validate() {
            Ok(s) => s,
            Err(e) => {
                form.error = Some(e);
                return;
            }
        };
        form.error = None;
        form.sending = true;
        match submission {
            Submission::Poll { question, options, multi } => self.spawn_call("Poll sent", Then::CloseForm, move |h| async move {
                h.send_poll(&chat, &question, &options, multi).await.map(|_| ()).map_err(|e| e.to_string())
            }),
            Submission::Location { lat, lon, name, address } => {
                self.spawn_call("Location sent", Then::CloseForm, move |h| async move {
                    h.send_location(&chat, lat, lon, name.as_deref(), address.as_deref(), None).await.map(|_| ()).map_err(|e| e.to_string())
                })
            }
            Submission::Contact { name, phone } => self.spawn_call("Contact sent", Then::CloseForm, move |h| async move {
                h.send_contact(&chat, &name, &phone, None).await.map(|_| ()).map_err(|e| e.to_string())
            }),
        }
    }

    pub fn cancel_form(&mut self) {
        // An upload in flight cannot be taken back.
        if self.form.as_ref().is_some_and(|f| f.sending) {
            return;
        }
        self.form = None;
    }

    pub fn key_form(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Some(f) = self.form.as_mut() else { return };
        if f.sending {
            return;
        }
        let n = f.fields.len();
        match key.code {
            KeyCode::Esc => self.cancel_form(),
            KeyCode::Char('s') if ctrl => self.submit_form(),
            KeyCode::Char('t') if ctrl => f.multi = !f.multi,
            KeyCode::Tab | KeyCode::Down => f.focus = (f.focus + 1) % n,
            KeyCode::BackTab | KeyCode::Up => f.focus = (f.focus + n - 1) % n,
            KeyCode::Enter => {
                if f.focus + 1 < n {
                    f.focus += 1;
                } else {
                    self.submit_form();
                }
            }
            KeyCode::Char('v') if ctrl => self.paste_clipboard(),
            _ => {
                f.error = None;
                f.fields[f.focus].edit.key(key);
            }
        }
    }

    pub fn mouse_form(&mut self, m: MouseEvent) {
        if m.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }
        let Some(f) = self.form.as_mut() else { return };
        if f.sending {
            return;
        }
        let at = |r: Rect| r.width > 0 && m.column >= r.x && m.column < r.x + r.width && m.row >= r.y && m.row < r.y + r.height;
        if at(f.send_btn) {
            self.submit_form();
        } else if at(f.cancel_btn) {
            self.cancel_form();
        } else if at(f.multi_btn) {
            f.multi = !f.multi;
        } else if let Some(i) = f.field_hits.iter().find(|(r, _)| at(*r)).map(|(_, i)| *i) {
            f.focus = i;
        }
    }
}

pub fn draw_form(f: &mut Frame, form: &mut Form, area: Rect) {
    let label_w = 11u16;
    let rows = form.fields.len() as u16 + u16::from(form.kind == FormKind::Poll) + 5;
    let w = 64.min(area.width);
    let h = (rows + 2).min(area.height);
    let rect = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    form.rect = rect;
    f.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(c::GREEN))
        .title(Span::styled(format!(" {} ", form.kind.title()), Style::new().fg(c::FG).add_modifier(Modifier::BOLD)))
        .style(Style::new().bg(c::SURFACE));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    form.field_hits.clear();
    let mut y = inner.y + 1;
    let mut cursor = None;
    for (i, fld) in form.fields.iter().enumerate() {
        if y >= inner.y + inner.height {
            break;
        }
        let focused = i == form.focus && !form.sending;
        let input_x = inner.x + 1 + label_w;
        let input_w = inner.width.saturating_sub(label_w + 2);
        let bg = if focused { c::SELECT } else { c::BG_DARK };
        let (text, style) = if fld.edit.text.is_empty() {
            (fld.hint.clone(), Style::new().fg(c::DIM))
        } else {
            (fld.edit.text.clone(), Style::new().fg(c::FG))
        };
        // Keep the end of long text in view while typing.
        let shown = if focused && !fld.edit.text.is_empty() && width(&text) + 2 > input_w as usize {
            let tail: String = text.chars().rev().take(input_w as usize - 3).collect::<Vec<_>>().into_iter().rev().collect();
            format!("…{tail}")
        } else {
            truncate(&text, input_w.saturating_sub(1) as usize)
        };
        f.render_widget(
            Paragraph::new(Line::styled(format!(" {:<width$}", fld.label, width = label_w as usize - 1), Style::new().fg(c::DIM))),
            Rect::new(inner.x, y, label_w + 1, 1),
        );
        let pad = (input_w as usize).saturating_sub(width(&shown) + 1);
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(format!(" {shown}"), style), Span::raw(" ".repeat(pad))]).style(Style::new().bg(bg))),
            Rect::new(input_x, y, input_w, 1),
        );
        form.field_hits.push((Rect::new(inner.x, y, inner.width, 1), i));
        if focused {
            let col = if fld.edit.text.is_empty() { 0 } else { fld.edit.cursor_col().min(input_w as usize - 3) };
            cursor = Some((input_x + 1 + col as u16, y));
        }
        y += 1;
    }

    if form.kind == FormKind::Poll && y < inner.y + inner.height {
        let line = format!(" [{}] Allow several answers", if form.multi { "x" } else { " " });
        f.render_widget(Paragraph::new(Line::styled(line.clone(), Style::new().fg(c::FG))), Rect::new(inner.x, y + 1, inner.width, 1));
        form.multi_btn = Rect::new(inner.x, y + 1, width(&line) as u16 + 1, 1);
    } else {
        form.multi_btn = Rect::default();
    }

    // Error line, then the buttons.
    let bottom = inner.y + inner.height;
    if let Some(e) = &form.error {
        f.render_widget(Paragraph::new(Line::styled(format!(" {}", truncate(e, inner.width as usize - 2)), Style::new().fg(c::RED))), Rect::new(inner.x, bottom - 2, inner.width, 1));
    }
    let by = bottom - 1;
    if form.sending {
        form.send_btn = Rect::default();
        form.cancel_btn = Rect::default();
        f.render_widget(Paragraph::new(Line::styled(" Sending…", Style::new().fg(c::YELLOW))), Rect::new(inner.x, by, inner.width, 1));
    } else {
        let (send, cancel) = (" ⏎ Send ", " Cancel ");
        let (sw, cw) = (width(send) as u16, width(cancel) as u16);
        let send_r = Rect::new(inner.x + 1, by, sw, 1);
        let cancel_r = Rect::new(send_r.x + sw + 2, by, cw, 1);
        f.render_widget(Paragraph::new(Span::styled(send, Style::new().fg(c::BG).bg(c::GREEN).add_modifier(Modifier::BOLD))), send_r);
        f.render_widget(Paragraph::new(Span::styled(cancel, Style::new().fg(c::FG).bg(c::SELECT))), cancel_r);
        f.render_widget(
            Paragraph::new(Span::styled("Tab next field · Enter on the last one sends", Style::new().fg(c::DIM))),
            Rect::new(cancel_r.x + cw + 2, by, inner.width.saturating_sub(sw + cw + 5), 1),
        );
        form.send_btn = send_r;
        form.cancel_btn = cancel_r;
        if let Some(pos) = cursor {
            f.set_cursor_position(pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(kind: FormKind, values: &[&str]) -> Form {
        let mut f = Form::new(kind);
        for (i, v) in values.iter().enumerate() {
            f.fields[i].edit.text = (*v).to_string();
        }
        f
    }

    #[test]
    fn a_poll_needs_a_question_and_two_different_options() {
        assert_eq!(filled(FormKind::Poll, &[]).validate().unwrap_err(), "Write the question");
        assert!(filled(FormKind::Poll, &["Makan?", "Soto"]).validate().unwrap_err().contains("two options"));
        assert!(filled(FormKind::Poll, &["Makan?", "Soto", "soto"]).validate().unwrap_err().contains("twice"));
        let mut f = filled(FormKind::Poll, &["Makan?", "Soto", "", "Bakso"]);
        f.multi = true;
        assert_eq!(
            f.validate().unwrap(),
            Submission::Poll { question: "Makan?".into(), options: vec!["Soto".into(), "Bakso".into()], multi: true },
            "blank options are skipped"
        );
    }

    #[test]
    fn a_location_needs_real_coordinates() {
        assert!(filled(FormKind::Location, &["abc", "106"]).validate().unwrap_err().contains("Latitude must be a number"));
        assert!(filled(FormKind::Location, &["95", "106"]).validate().unwrap_err().contains("between"));
        assert!(filled(FormKind::Location, &["-6,2", "200"]).validate().unwrap_err().contains("Longitude"));
        assert_eq!(
            filled(FormKind::Location, &["-6,2088", "106.8456", "Monas", ""]).validate().unwrap(),
            Submission::Location { lat: -6.2088, lon: 106.8456, name: Some("Monas".into()), address: None },
            "a decimal comma is accepted"
        );
    }

    #[test]
    fn a_contact_needs_a_name_and_a_number() {
        assert!(filled(FormKind::Contact, &["", "0812345678"]).validate().is_err());
        assert!(filled(FormKind::Contact, &["Budi", "12"]).validate().unwrap_err().contains("phone"));
        assert_eq!(
            filled(FormKind::Contact, &["Budi", "+62 812-3456-7890"]).validate().unwrap(),
            Submission::Contact { name: "Budi".into(), phone: "+6281234567890".into() }
        );
    }
}
