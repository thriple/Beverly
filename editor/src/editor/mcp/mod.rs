//! The local MCP server (ported from Prism 3D's): off by default; when on, an AI assistant on this computer can
//! read the project and run Beverly's actions. The protocol is in [`protocol`], the socket in `server`, and the
//! tools run on the main thread in [`host`], once a frame.

pub mod host;
pub mod jobs;
pub mod protocol;
mod server;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::path::PathBuf;

/// The default port: unassigned, and next to Prism's (47615) so both can run.
pub const DEFAULT_PORT: u16 = 47616;
/// How many recent calls the window lists.
const LOG_LINES: usize = 6;

/// What the user chose. Kept in the user's settings (`mcp.json`), not in projects.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub port: u16,
    pub token: String,
}

impl Default for Settings {
    fn default() -> Self { Settings { enabled: false, port: DEFAULT_PORT, token: new_token() } }
}

impl Settings {
    pub fn url(&self) -> String { format!("http://127.0.0.1:{}/mcp", self.port) }

    /// The command that adds Beverly to Claude Code.
    pub fn claude_command(&self) -> String {
        format!("claude mcp add --transport http beverly {} --header \"Authorization: Bearer {}\"", self.url(), self.token)
    }

    fn file() -> PathBuf { user_settings_dir().join("mcp.json") }

    /// The saved settings, or new ones (with a fresh token) saved straight away.
    pub fn load() -> Settings {
        if let Some(s) = std::fs::read_to_string(Self::file()).ok().and_then(|t| serde_json::from_str(&t).ok()) { return s; }
        let s = Settings::default();
        s.save();
        s
    }

    pub fn save(&self) {
        let f = Self::file();
        if let Some(d) = f.parent() { let _ = std::fs::create_dir_all(d); }
        let _ = std::fs::write(f, serde_json::to_string_pretty(self).unwrap_or_default());
    }
}

/// Where Beverly keeps the user's own settings: %APPDATA%\Beverly on Windows, ~/.config/beverly elsewhere.
pub fn user_settings_dir() -> PathBuf {
    if let Some(a) = std::env::var_os("APPDATA") { return PathBuf::from(a).join("Beverly"); }
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") { return PathBuf::from(x).join("beverly"); }
    std::env::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".config").join("beverly")
}

/// A fresh token: 32 hex characters from the system's random seed (each `RandomState` is keyed from it).
pub fn new_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    (0..2u64)
        .map(|i| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(i);
            h.write_u128(nanos);
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// Whether the server is listening.
#[derive(Clone, Debug, PartialEq)]
pub enum Listen {
    Off,
    On(u16),
    Failed(String),
}

/// A tool call waiting for the main thread.
pub struct Call {
    pub name: String,
    pub args: Value,
    pub reply: std::sync::mpsc::Sender<Result<protocol::Output, String>>,
}

/// The server's state, kept in the editor.
pub struct Mcp {
    pub settings: Settings,
    pub listen: Listen,
    /// The last few calls, newest last.
    pub log: VecDeque<String>,
    pub jobs: jobs::Jobs,
    server: Option<server::Server>,
}

impl Mcp {
    /// Not listening, with unsaved default settings (until `start`).
    pub fn off() -> Mcp { Mcp { settings: Settings::default(), listen: Listen::Off, log: VecDeque::new(), jobs: jobs::Jobs::default(), server: None } }

    /// The saved settings, listening straight away if the user left it on.
    pub fn start() -> Mcp {
        let mut m = Mcp { settings: Settings::load(), listen: Listen::Off, log: VecDeque::new(), jobs: jobs::Jobs::default(), server: None };
        m.apply();
        m
    }

    /// Start or stop the server to match the settings (and restart it on a new token); saves the settings.
    pub fn apply(&mut self) {
        self.settings.save();
        if let Some(s) = self.server.take() { s.stop(); }
        self.listen = Listen::Off;
        if !self.settings.enabled { return; }
        match server::Server::start(self.settings.port, self.settings.token.clone()) {
            Ok(s) => {
                self.listen = Listen::On(s.port());
                self.server = Some(s);
            }
            Err(e) => self.listen = Listen::Failed(e),
        }
    }

    /// Calls waiting for the main thread.
    pub fn take_calls(&mut self) -> Vec<Call> {
        match &self.server {
            Some(s) => s.calls.lock().map(|c| c.try_iter().collect()).unwrap_or_default(),
            None => vec![],
        }
    }

    /// Note a call for the window.
    pub fn note(&mut self, line: String) {
        self.log.push_back(line);
        while self.log.len() > LOG_LINES { self.log.pop_front(); }
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        if let Some(s) = self.server.take() { s.stop(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_32_hex_characters_and_differ() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn settings_read_back_and_default_off() {
        let s = Settings::default();
        assert!(!s.enabled);
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), s);
        let partial: Settings = serde_json::from_str(r#"{"enabled": true}"#).unwrap();
        assert!(partial.enabled && partial.port == DEFAULT_PORT && partial.token.len() == 32);
        assert!(s.claude_command().contains(&format!("Bearer {}", s.token)));
        assert!(s.claude_command().contains("http://127.0.0.1:47616/mcp"));
    }
}
