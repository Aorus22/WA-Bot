//! wabot-cli: terminal chat client for WA Bot.
//!
//! Talks to the same local Go backend as the gpui and web clients. It does not
//! start the backend: run the desktop app (or `wa-bot-backend`) first, or point
//! it at one with `--url`/`--port`.

mod actions;
mod attach;
mod browser;
mod form;
mod app;
mod clip;
mod backend;
mod images;
mod menu;
mod overlay;
mod panel;
mod smooth;
mod stickers;
mod text;
mod ticks;
mod theme;
mod ui;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use app::{App, Event};
use ratatui::crossterm::event::{self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture};
use ratatui::crossterm::execute;
use wabot_backend_client::{HttpClient, WsClient};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let api = backend::resolve_api_url(args.iter().cloned())?;
    let image_mode = backend::flag_value(&args, "--images").unwrap_or_else(|| "auto".to_string());
    ticks::init(&backend::flag_value(&args, "--ticks").unwrap_or_else(|| "auto".to_string()));
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("tokio runtime")?;

    let (tx, rx) = flume::unbounded::<Event>();
    let http = Arc::new(HttpClient::new(api.clone()));

    // WebSocket events -> the same channel as keys and HTTP answers.
    let ws = WsClient::from_static_url(backend::ws_url(&api), None);
    let ws_rx = ws.subscribe();
    {
        let _guard = rt.enter();
        ws.start();
    }
    let ws_tx = tx.clone();
    std::thread::spawn(move || {
        while let Ok(ev) = ws_rx.recv() {
            if ws_tx.send(Event::Ws(ev)).is_err() {
                break;
            }
        }
    });

    // The picker talks to the terminal, so it runs after the alternate screen is
    // up and before the input thread starts reading events.
    let mut terminal = ratatui::init();
    let picker = images::make_picker(&image_mode);
    execute!(std::io::stdout(), EnableMouseCapture, EnableBracketedPaste).ok();
    // Where the terminal can tell Ctrl+Shift+C from Ctrl+C, ask it to (kitty protocol).
    let enhanced = ratatui::crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false);
    if enhanced {
        execute!(
            std::io::stdout(),
            ratatui::crossterm::event::PushKeyboardEnhancementFlags(
                ratatui::crossterm::event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
            )
        )
        .ok();
    }

    // Terminal input thread.
    let key_tx = tx.clone();
    std::thread::spawn(move || loop {
        match event::read() {
            Ok(ev) => {
                if key_tx.send(Event::Input(ev)).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    });

    let mut app = App::new(http, rt.handle().clone(), tx.clone(), picker);
    if let Some(n) = backend::flag_value(&args, "--scroll").and_then(|v| v.parse::<isize>().ok()) {
        app.wheel_step = n.clamp(1, 20);
    }
    app.reload_chats();

    let result = run(&mut terminal, &mut app, rx);
    if enhanced {
        execute!(std::io::stdout(), ratatui::crossterm::event::PopKeyboardEnhancementFlags).ok();
    }
    execute!(std::io::stdout(), DisableBracketedPaste, DisableMouseCapture).ok();
    ratatui::restore();
    ws.stop();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App, rx: flume::Receiver<Event>) -> Result<()> {
    while !app.quit {
        terminal.draw(|f| ui::draw(f, app))?;
        app.after_draw();

        // Block for the next event (or a tick for typing expiry), then drain the
        // rest so a burst of events costs one redraw.
        // While a scroll is gliding, wake up every frame instead of every half second.
        let wait = if app.animating() { Duration::from_millis(16) } else { Duration::from_millis(500) };
        match rx.recv_timeout(wait) {
            Ok(ev) => app.handle(ev),
            Err(flume::RecvTimeoutError::Timeout) => app.handle(Event::Tick),
            Err(flume::RecvTimeoutError::Disconnected) => break,
        }
        while let Ok(ev) = rx.try_recv() {
            app.handle(ev);
        }
        app.animate();
    }
    Ok(())
}
