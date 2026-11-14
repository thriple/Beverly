//! Help > Check for updates: the newest release is downloaded from GitHub Releases, checked against the SHA-256
//! published with it (update.json), and its programs (Beverly.exe, beverly-player.exe) put in place of these,
//! which keep running. The new build runs from the next start. A program written by Beverly itself is not
//! marked as downloaded, so Windows doesn't ask to "Run anyway" again. Adapted from Prism 3D's updater.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The newest release's files (GitHub sends "latest/download" to the release marked Latest).
pub const RELEASES: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases/latest/download");

/// This build's number, set by the release workflow (none for a build made by hand).
pub const BUILD: Option<&str> = option_env!("BEVERLY_BUILD");

pub fn this_build() -> Option<u32> { BUILD.and_then(|b| b.trim().parse().ok()) }

/// What the release workflow publishes beside the zip.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Manifest {
    pub build: u32,
    /// Each zip's name and SHA-256, in hex.
    pub files: BTreeMap<String, String>,
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Manifest, String> { serde_json::from_str(text).map_err(|e| format!("The update list could not be read ({e}).")) }
}

pub fn sha256_hex(bytes: &[u8]) -> String { Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect() }

/// Whether `bytes` have the published checksum.
pub fn checks_out(bytes: &[u8], expected: &str) -> bool { sha256_hex(bytes).eq_ignore_ascii_case(expected.trim()) }

/// Rename, trying again for a moment if the file is busy (on Windows a virus scanner often holds a new file).
fn rename(from: &Path, to: &Path) -> io::Result<()> {
    let mut tries = 0;
    loop {
        match std::fs::rename(from, to) {
            Err(e) if tries < 5 && from.exists() && e.kind() != io::ErrorKind::NotFound => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            r => return r,
        }
    }
}

/// Put `new` where `current` is, moving `current` to `aside` first (a running program can be renamed, even on
/// Windows). If `new` cannot be moved in, `current` is put back.
pub fn swap(current: &Path, new: &Path, aside: &Path) -> io::Result<()> {
    if aside.exists() { std::fs::remove_file(aside)?; }
    if current.exists() { rename(current, aside)?; }
    if let Err(e) = rename(new, current) {
        let _ = rename(aside, current);
        return Err(e);
    }
    Ok(())
}

fn aside(exe: &Path) -> PathBuf { exe.with_extension("old.exe") }

/// Put the zip's programs in the folder `dir` (the zip's own folder name is ignored). On failure the old ones
/// stay.
pub fn install(zip: &[u8], dir: &Path) -> Result<(), String> {
    let why = |e: io::Error| {
        if e.kind() == io::ErrorKind::PermissionDenied {
            format!("Beverly may not write to {}. Move the Beverly folder somewhere you own, or download the new build from the Releases page.", dir.display())
        } else {
            format!("The update could not be installed in {} ({e}).", dir.display())
        }
    };
    let mut archive = zip::ZipArchive::new(io::Cursor::new(zip)).map_err(|e| format!("The download is not a zip ({e})."))?;
    // every program in it, by file name (Windows' zip tool may write backslashes)
    let mut programs = vec![];
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().replace('\\', "/");
        let file = name.rsplit('/').next().unwrap_or("").to_string();
        if file.to_ascii_lowercase().ends_with(".exe") {
            let mut bytes = vec![];
            entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            programs.push((file, bytes));
        }
    }
    if programs.is_empty() { return Err("The download has no programs in it.".into()); }
    // write them all first, then swap them in
    let news: Vec<(PathBuf, PathBuf)> = programs.iter().map(|(f, _)| (dir.join(f), dir.join(f).with_extension("new.exe"))).collect();
    for ((_, bytes), (_, new)) in programs.iter().zip(&news) {
        if let Err(e) = std::fs::write(new, bytes) {
            for (_, n) in &news { let _ = std::fs::remove_file(n); }
            return Err(why(e));
        }
    }
    for (exe, new) in &news {
        if let Err(e) = swap(exe, new, &aside(exe)) {
            for (_, n) in &news { let _ = std::fs::remove_file(n); }
            return Err(why(e));
        }
    }
    Ok(())
}

/// Delete what an earlier update left beside the programs in `dir` (at start).
pub fn clean_up_in(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        if name.ends_with(".old.exe") || name.ends_with(".new.exe") { let _ = std::fs::remove_file(e.path()); }
    }
}

pub fn clean_up() {
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) { clean_up_in(&dir); }
}

/// Where the check or the download is.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Idle,
    Checking,
    /// A build made by hand: nothing to compare with.
    Local,
    /// Only the Windows build is published.
    Unsupported,
    /// A game's own copy of the editor: it updates with the game.
    GameEditor,
    UpToDate(u32),
    Available { latest: u32, sha: String },
    Downloading { latest: u32, done: u64, total: Option<u64> },
    Installed(u32),
    Failed(String),
}

impl Status {
    pub fn busy(&self) -> bool { matches!(self, Status::Checking | Status::Downloading { .. }) }
}

/// Where to look, and what to replace.
#[derive(Clone, Debug)]
pub struct Config {
    /// The folder holding update.json and the zip.
    pub base: String,
    pub current: Option<u32>,
    pub asset: Option<&'static str>,
    /// The folder the programs are in.
    pub dir: PathBuf,
    /// This is Beverly's own editor (not a game's copy of it).
    pub own: bool,
}

impl Config {
    pub fn this_app() -> Config {
        let exe = std::env::current_exe().unwrap_or_default();
        Config {
            base: RELEASES.to_owned(),
            current: this_build(),
            asset: cfg!(windows).then_some("Beverly-windows.zip"),
            dir: exe.parent().map(Path::to_path_buf).unwrap_or_default(),
            own: exe.file_stem().is_some_and(|s| s.eq_ignore_ascii_case("beverly")),
        }
    }
}

/// Checks and downloads on a thread; the UI reads [`Updater::status`].
#[derive(Clone)]
pub struct Updater { config: Config, status: Arc<Mutex<Status>> }

impl Default for Updater {
    fn default() -> Self { Self::new(Config::this_app()) }
}

impl Updater {
    pub fn new(config: Config) -> Self { Self { config, status: Arc::new(Mutex::new(Status::Idle)) } }

    pub fn status(&self) -> Status { self.status.lock().map(|s| s.clone()).unwrap_or(Status::Idle) }

    fn set(&self, s: Status) {
        if let Ok(mut now) = self.status.lock() { *now = s; }
    }

    /// Look for a newer build.
    pub fn check(&self) {
        if self.status().busy() { return; }
        if !self.config.own { return self.set(Status::GameEditor); }
        let Some(current) = self.config.current else { return self.set(Status::Local) };
        let Some(asset) = self.config.asset else { return self.set(Status::Unsupported) };
        self.set(Status::Checking);
        let me = self.clone();
        std::thread::spawn(move || {
            let url = format!("{}/update.json", me.config.base);
            let result = net::get(&url, |_, _| {}).and_then(|b| Manifest::parse(&String::from_utf8_lossy(&b)));
            me.set(match result {
                Err(e) => Status::Failed(e),
                Ok(m) if m.build <= current => Status::UpToDate(current),
                Ok(m) => match m.files.get(asset) {
                    Some(sha) => Status::Available { latest: m.build, sha: sha.clone() },
                    None => Status::Failed(format!("Build {} has no {asset}.", m.build)),
                },
            });
        });
    }

    /// Download the build found by [`Updater::check`], check it and put it in place.
    pub fn install(&self) {
        let Status::Available { latest, sha } = self.status() else { return };
        let Some(asset) = self.config.asset else { return };
        self.set(Status::Downloading { latest, done: 0, total: None });
        let me = self.clone();
        std::thread::spawn(move || {
            let url = format!("{}/{asset}", me.config.base);
            let result = net::get(&url, |done, total| me.set(Status::Downloading { latest, done, total })).and_then(|zip| {
                if !checks_out(&zip, &sha) {
                    return Err("The download doesn't match its published checksum, so nothing was changed. Try again later.".to_owned());
                }
                install(&zip, &me.config.dir)
            });
            me.set(match result {
                Ok(()) => Status::Installed(latest),
                Err(e) => Status::Failed(e),
            });
        });
    }
}

mod net {
    use std::io::Read;
    use std::time::Duration;
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

    /// The largest download accepted.
    const MAX_DOWNLOAD: u64 = 1 << 30;

    /// GET `url` with the system's TLS and certificates, following redirects, calling `progress(done, total)`
    /// as it arrives.
    pub fn get(url: &str, mut progress: impl FnMut(u64, Option<u64>)) -> Result<Vec<u8>, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build())
            .timeout_connect(Some(Duration::from_secs(20)))
            .user_agent("Beverly-updater")
            .build()
            .into();
        let mut response = agent.get(url).call().map_err(|e| format!("Couldn't reach the releases ({e})."))?;
        let total = response.body().content_length();
        let mut reader = response.body_mut().with_config().limit(MAX_DOWNLOAD).reader();
        let mut out = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf).map_err(|e| format!("The download stopped ({e})."))?;
            if n == 0 { break; }
            out.extend_from_slice(&buf[..n]);
            progress(out.len() as u64, total);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("beverly-update-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn zip_of(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        for (name, text) in entries {
            w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(text.as_bytes()).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn the_manifest_and_checksums_read() {
        let m = Manifest::parse(r#"{"build": 18, "files": {"Beverly-windows.zip": "00ff"}}"#).unwrap();
        assert_eq!(m.build, 18);
        assert!(Manifest::parse("nope").is_err());
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(checks_out(b"abc", &abc.to_uppercase()));
        assert!(!checks_out(b"abd", abc));
    }

    #[test]
    fn installing_replaces_both_programs_and_cleans_up_later() {
        let d = scratch("install");
        std::fs::write(d.join("Beverly.exe"), "build 1").unwrap();
        std::fs::write(d.join("beverly-player.exe"), "player 1").unwrap();
        install(&zip_of(&[("Beverly\\Beverly.exe", "build 2"), ("Beverly/beverly-player.exe", "player 2"), ("Beverly/README.txt", "x")]), &d).unwrap();
        assert_eq!(std::fs::read_to_string(d.join("Beverly.exe")).unwrap(), "build 2");
        assert_eq!(std::fs::read_to_string(d.join("beverly-player.exe")).unwrap(), "player 2");
        assert!(d.join("Beverly.old.exe").exists());
        assert!(!d.join("README.txt").exists(), "only the programs");
        clean_up_in(&d);
        assert!(!d.join("Beverly.old.exe").exists() && !d.join("beverly-player.old.exe").exists());
        assert!(install(&zip_of(&[("readme.txt", "x")]), &d).is_err());
        assert_eq!(std::fs::read_to_string(d.join("Beverly.exe")).unwrap(), "build 2");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Serve `files` over plain HTTP on a local port, one request each.
    fn serve(files: Vec<(&'static str, Vec<u8>)>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming().take(files.len()) {
                let Ok(mut s) = stream else { continue };
                let mut req = [0u8; 2048];
                let n = s.read(&mut req).unwrap_or(0);
                let line = String::from_utf8_lossy(&req[..n]);
                let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let body = files.iter().find(|(name, _)| path.ends_with(name)).map(|(_, b)| b.clone());
                let _ = match body {
                    Some(b) => s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", b.len()).as_bytes()).and_then(|_| s.write_all(&b)),
                    None => s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
                };
            }
        });
        base
    }

    fn wait(u: &Updater) -> Status {
        for _ in 0..500 {
            let s = u.status();
            if !s.busy() { return s; }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("still busy: {:?}", u.status());
    }

    #[test]
    fn a_newer_build_is_found_downloaded_checked_and_installed() {
        let d = scratch("flow");
        std::fs::write(d.join("Beverly.exe"), "build 7").unwrap();
        let zip = zip_of(&[("Beverly/Beverly.exe", "build 9")]);
        let manifest = format!(r#"{{"build": 9, "files": {{"Beverly-windows.zip": "{}"}}}}"#, sha256_hex(&zip));
        let base = serve(vec![("update.json", manifest.into_bytes()), ("Beverly-windows.zip", zip)]);
        let cfg = Config { base, current: Some(7), asset: Some("Beverly-windows.zip"), dir: d.clone(), own: true };
        let u = Updater::new(cfg.clone());
        u.check();
        assert!(matches!(wait(&u), Status::Available { latest: 9, .. }));
        u.install();
        assert_eq!(wait(&u), Status::Installed(9));
        assert_eq!(std::fs::read_to_string(d.join("Beverly.exe")).unwrap(), "build 9");
        // the same build is not offered; a hand-made build and a game's editor are not compared
        let base = serve(vec![("update.json", br#"{"build": 9, "files": {}}"#.to_vec())]);
        let u = Updater::new(Config { base, current: Some(9), ..cfg.clone() });
        u.check();
        assert_eq!(wait(&u), Status::UpToDate(9));
        let u = Updater::new(Config { current: None, ..cfg.clone() });
        u.check();
        assert_eq!(u.status(), Status::Local);
        let u = Updater::new(Config { own: false, ..cfg });
        u.check();
        assert_eq!(u.status(), Status::GameEditor);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_bad_download_changes_nothing() {
        let d = scratch("bad");
        std::fs::write(d.join("Beverly.exe"), "build 7").unwrap();
        let zip = zip_of(&[("Beverly/Beverly.exe", "build 9")]);
        let base = serve(vec![("update.json", br#"{"build": 9, "files": {"Beverly-windows.zip": "00"}}"#.to_vec()), ("Beverly-windows.zip", zip)]);
        let u = Updater::new(Config { base, current: Some(7), asset: Some("Beverly-windows.zip"), dir: d.clone(), own: true });
        u.check();
        assert!(matches!(wait(&u), Status::Available { .. }));
        u.install();
        assert!(matches!(wait(&u), Status::Failed(e) if e.contains("checksum")));
        assert_eq!(std::fs::read_to_string(d.join("Beverly.exe")).unwrap(), "build 7");
        let _ = std::fs::remove_dir_all(&d);
    }
}
