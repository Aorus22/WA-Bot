//! Manual update check against GitHub Releases + AppImage self-replace.
//!
//! Everything here is blocking: call it from a background thread. There is no
//! periodic polling; it only runs when the user asks in Settings.
//!
//! Channels: the stable release is the repository's `latest` release (a
//! `vX.Y.Z` tag built from `main`). The rolling `beta` release is a prerelease,
//! which GitHub leaves out of `/releases/latest`, so it is never offered here.

use std::path::PathBuf;
use std::time::Duration;

const REPO: &str = "Aorus22/WA-Bot";
/// Test hook: point the check at a mock server (full URL of the "latest release" JSON).
const API_OVERRIDE_ENV: &str = "WABOT_UPDATE_API";

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub page_url: String,
    pub appimage_url: Option<String>,
    pub sums_url: Option<String>,
    pub appimage_name: Option<String>,
}

/// Where this process was installed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallKind {
    /// Running from an AppImage at this path: it can replace itself.
    AppImage(PathBuf),
    /// Package manager, `cargo run` or any other build.
    Other,
}

fn client(timeout: Duration) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(format!("wa-bot-desktop/{CURRENT_VERSION}"))
        .timeout(timeout)
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))
}

fn parse_version(tag: &str) -> Option<semver::Version> {
    semver::Version::parse(tag.trim().trim_start_matches('v')).ok()
}

/// Parse the GitHub "latest release" JSON. `Ok(None)` means already up to date.
pub fn parse_release(json: &str, current: &str) -> Result<Option<UpdateInfo>, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("Unexpected response: {e}"))?;
    let tag = v["tag_name"].as_str().ok_or("Release has no tag_name")?;
    let latest = parse_version(tag).ok_or_else(|| format!("Unrecognised version tag: {tag}"))?;
    let current = parse_version(current).ok_or("Invalid current version")?;
    if latest <= current {
        return Ok(None);
    }
    let version = latest.to_string();
    let wanted = format!("wa-bot_{version}_x86_64.AppImage");
    let mut appimage_url = None;
    let mut sums_url = None;
    let mut appimage_name = None;
    for a in v["assets"].as_array().into_iter().flatten() {
        let (Some(name), Some(url)) = (a["name"].as_str(), a["browser_download_url"].as_str())
        else {
            continue;
        };
        if name == wanted {
            appimage_url = Some(url.to_string());
            appimage_name = Some(name.to_string());
        } else if name == "SHA256SUMS.txt" {
            sums_url = Some(url.to_string());
        }
    }
    Ok(Some(UpdateInfo {
        version,
        notes: v["body"].as_str().unwrap_or("").to_string(),
        page_url: v["html_url"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| format!("https://github.com/{REPO}/releases/latest")),
        appimage_url,
        sums_url,
        appimage_name,
    }))
}

/// Ask GitHub for the latest stable release. `Ok(None)` = up to date.
pub fn check() -> Result<Option<UpdateInfo>, String> {
    let url = std::env::var(API_OVERRIDE_ENV)
        .unwrap_or_else(|_| format!("https://api.github.com/repos/{REPO}/releases/latest"));
    let resp = client(Duration::from_secs(10))?
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("Couldn't reach GitHub: {e}"))?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None); // no published stable release yet
    }
    if !resp.status().is_success() {
        return Err(format!("GitHub returned {}", resp.status()));
    }
    let body = resp.text().map_err(|e| format!("Read error: {e}"))?;
    parse_release(&body, CURRENT_VERSION)
}

/// Find `name` in a `sha256sum` file (`<hex>  <name>` per line).
pub fn find_checksum(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let hash = it.next()?;
        let file = it.next()?.trim_start_matches('*');
        (file == name).then(|| hash.to_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{
      "tag_name": "v0.2.0", "html_url": "https://example/r", "body": "notes",
      "assets": [
        {"name": "wa-bot_0.2.0_x86_64.AppImage", "browser_download_url": "https://example/a"},
        {"name": "SHA256SUMS.txt", "browser_download_url": "https://example/s"}
      ]}"#;

    #[test]
    fn newer_release_is_offered() {
        let u = parse_release(JSON, "0.1.0").unwrap().unwrap();
        assert_eq!(u.version, "0.2.0");
        assert_eq!(u.appimage_url.as_deref(), Some("https://example/a"));
        assert_eq!(u.sums_url.as_deref(), Some("https://example/s"));
    }

    #[test]
    fn same_or_older_is_up_to_date() {
        assert!(parse_release(JSON, "0.2.0").unwrap().is_none());
        assert!(parse_release(JSON, "0.3.0").unwrap().is_none());
    }

    #[test]
    fn beta_builds_get_the_stable_release() {
        assert!(parse_release(JSON, "0.2.0-beta.7").unwrap().is_some());
        assert!(parse_release(JSON, "0.1.0-beta.7").unwrap().is_some());
    }

    #[test]
    fn bad_tag_is_error() {
        assert!(parse_release(r#"{"tag_name":"latest"}"#, "0.1.0").is_err());
    }

    #[test]
    fn missing_assets_are_reported_as_none() {
        let u = parse_release(r#"{"tag_name":"v0.2.0","assets":[]}"#, "0.1.0")
            .unwrap()
            .unwrap();
        assert!(u.appimage_url.is_none() && u.sums_url.is_none());
    }

    #[test]
    fn checksum_lookup() {
        let sums = "ABCDEF  wa-bot_0.2.0_x86_64.AppImage\n123  other\n";
        assert_eq!(
            find_checksum(sums, "wa-bot_0.2.0_x86_64.AppImage").as_deref(),
            Some("abcdef")
        );
        assert!(find_checksum(sums, "missing").is_none());
    }
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod unsupported;

#[cfg(target_os = "linux")]
pub use linux::{download_and_install, install_kind, spawn_restart};
#[cfg(not(target_os = "linux"))]
pub use unsupported::{download_and_install, install_kind, spawn_restart};
