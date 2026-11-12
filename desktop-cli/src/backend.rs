//! Finding the local backend.

use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::Deserialize;

/// The part of `<data>/run/core.json` we need (written by the desktop app).
#[derive(Deserialize)]
struct CoreRecord {
    port: u16,
}

fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("WABOT_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".wa-bot"))
        .unwrap_or_else(|| PathBuf::from(".wa-bot"))
}

/// The value after `name` in the arguments, if both are there.
pub fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// `--url <base>`, `--port <n>`, else the backend the desktop app recorded.
pub fn resolve_api_url(args: impl Iterator<Item = String>) -> Result<String> {
    let args: Vec<String> = args.collect();
    let value = |name: &str| {
        args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
    };
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("wabot-cli [--url http://host:port] [--port N] [--images auto|off|halfblocks|kitty|sixel|iterm2] [--ticks auto|nerd|unicode|ascii] [--scroll LINES] [--theme {}]\n\nWithout --theme the terminal's own background and palette are used (`auto`).\nWithout flags it uses the backend recorded in ~/.wa-bot/run/core.json.", crate::theme::names());
        std::process::exit(0);
    }
    if let Some(url) = value("--url") {
        return Ok(url);
    }
    if let Some(port) = value("--port") {
        let port: u16 = port.parse().map_err(|_| anyhow::anyhow!("invalid port: {port}"))?;
        return Ok(format!("http://127.0.0.1:{port}/api"));
    }
    let path = data_dir().join("run").join("core.json");
    match std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<CoreRecord>(&b).ok()) {
        Some(core) => Ok(format!("http://127.0.0.1:{}/api", core.port)),
        None => bail!(
            "no backend found ({} missing). Start the WA Bot app first, or pass --port / --url.",
            path.display()
        ),
    }
}

/// WebSocket URL for an API base URL.
pub fn ws_url(api: &str) -> String {
    let base = api.trim_end_matches('/').trim_end_matches("/api");
    match base.strip_prefix("https://") {
        Some(rest) => format!("wss://{rest}/ws"),
        None => format!("{}/ws", base.replace("http://", "ws://")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_url_follows_scheme() {
        assert_eq!(ws_url("http://127.0.0.1:8080/api"), "ws://127.0.0.1:8080/ws");
        assert_eq!(ws_url("https://x.y/api/"), "wss://x.y/ws");
    }
}
