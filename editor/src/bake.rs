//! The bake cache: everything Beverly works out from the user's files (impostor atlases, virtual-geometry meshes,
//! baked light) is kept here, never in the project. An entry is named by a hash of its source and its bake settings
//! (`key`), so a changed model or setting simply misses; a missing entry is built again in the background
//! (`request`). The cache lives in the user's settings folder; an exported game carries the entries its level used in
//! `bakes/` (read first-hand, never written) and builds any that are missing the same way.
use bevy::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// The name of a bake: a hash of what it is (`kind`, such as "impostor") and everything it comes from: the source's
/// bytes and the bake settings, each as a part.
pub fn key(kind: &str, parts: &[&[u8]]) -> String {
    let mut h = Sha256::new();
    h.update(kind.as_bytes());
    for p in parts {
        h.update((p.len() as u64).to_le_bytes());
        h.update(p);
    }
    h.finalize().iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// How far a bake has got, for the island: the bake calls `set` with a fraction.
#[derive(Clone, Default)]
pub struct Progress(Arc<AtomicU32>);

impl Progress {
    pub fn set(&self, fraction: f32) { self.0.store((fraction.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::Relaxed); }
    fn get(&self) -> f32 { self.0.load(Ordering::Relaxed) as f32 / 1000.0 }
}

type Outcome = Arc<Mutex<Option<Result<Vec<u8>, String>>>>;

struct Job { kind: String, key: String, label: String, progress: Progress, outcome: Outcome }

struct Inner {
    root: PathBuf,
    /// An exported game's bakes (read only).
    packed: Option<PathBuf>,
    running: Vec<Job>,
    /// What this run asked for, so an export packs just that.
    used: BTreeSet<(String, String)>,
}

/// A bake finished (or failed): whoever asked can `get` it now.
#[derive(Message, Clone, Debug)]
pub struct BakeDone { pub kind: String, pub label: String, pub error: Option<String> }

#[derive(Resource, Clone)]
pub struct Bakes(Arc<Mutex<Inner>>);

impl Bakes {
    pub fn new(root: PathBuf, packed: Option<PathBuf>) -> Self { Bakes(Arc::new(Mutex::new(Inner { root, packed, running: vec![], used: default() }))) }

    /// The editor's and the player's cache: the user's settings folder.
    pub fn default_root() -> PathBuf { crate::editor::mcp::user_settings_dir().join("cache") }

    fn inner(&self) -> std::sync::MutexGuard<'_, Inner> { self.0.lock().unwrap_or_else(|e| e.into_inner()) }

    /// A finished bake, or None (then `request` it).
    pub fn get(&self, kind: &str, key: &str) -> Option<Vec<u8>> {
        let mut i = self.inner();
        let found = std::iter::once(&i.root).chain(i.packed.iter()).find_map(|d| std::fs::read(d.join(kind).join(key)).ok());
        if found.is_some() { i.used.insert((kind.into(), key.into())); }
        found
    }

    /// Builds a bake in the background unless it is cached or already being built; `label` is what the island says
    /// ("Impostor for Tree"). True when it started. The work reports its progress and returns the bytes to keep.
    pub fn request(&self, kind: &str, key: &str, label: &str, work: impl FnOnce(&Progress) -> Result<Vec<u8>, String> + Send + 'static) -> bool {
        if self.get(kind, key).is_some() { return false; }
        let mut i = self.inner();
        i.used.insert((kind.into(), key.into()));
        if i.running.iter().any(|j| j.kind == kind && j.key == key) { return false; }
        let (progress, outcome) = (Progress::default(), Outcome::default());
        let (p, o) = (progress.clone(), outcome.clone());
        std::thread::spawn(move || {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(&p))).unwrap_or_else(|_| Err("the bake crashed".into()));
            *o.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
        });
        i.running.push(Job { kind: kind.into(), key: key.into(), label: label.into(), progress, outcome });
        true
    }

    /// Keeps bytes made another way (a picture taken of the 3D scene, say) as a finished bake.
    pub fn put(&self, kind: &str, key: &str, bytes: &[u8]) -> std::io::Result<()> {
        let root = self.inner().root.clone();
        self.inner().used.insert((kind.into(), key.into()));
        write_atomic(&root.join(kind), key, bytes)
    }

    /// What is being built, with how far each has got (0 to 1).
    pub fn running(&self) -> Vec<(String, f32)> { self.inner().running.iter().map(|j| (j.label.clone(), j.progress.get())).collect() }

    /// Saves what finished and says so (called each frame by the plugin).
    pub fn poll(&self) -> Vec<BakeDone> {
        let mut i = self.inner();
        let root = i.root.clone();
        let mut done = vec![];
        i.running.retain(|j| {
            let Some(r) = j.outcome.lock().unwrap_or_else(|e| e.into_inner()).take() else { return true };
            let error = r.and_then(|bytes| write_atomic(&root.join(&j.kind), &j.key, &bytes).map_err(|e| e.to_string())).err();
            done.push(BakeDone { kind: j.kind.clone(), label: j.label.clone(), error });
            false
        });
        done
    }

    /// How many entries the cache holds and their total size.
    pub fn size(&self) -> (usize, u64) {
        let root = self.inner().root.clone();
        let files: Vec<_> = std::fs::read_dir(&root).into_iter().flatten().flatten().flat_map(|k| std::fs::read_dir(k.path()).into_iter().flatten().flatten()).filter_map(|f| f.metadata().ok()).filter(|m| m.is_file()).collect();
        (files.len(), files.iter().map(|m| m.len()).sum())
    }

    /// Deletes every entry (they are built again when asked for). Returns how many.
    pub fn clear(&self) -> usize {
        let n = self.size().0;
        let _ = std::fs::remove_dir_all(&self.inner().root);
        n
    }

    /// For an export: the entries this run used, as the files to put in the game's `bakes/` folder.
    pub fn pack(&self) -> Vec<(String, Vec<u8>)> {
        let used: Vec<_> = self.inner().used.iter().cloned().collect();
        used.into_iter().filter_map(|(kind, key)| self.get(&kind, &key).map(|b| (format!("bakes/{kind}/{key}"), b))).collect()
    }
}

fn write_atomic(dir: &std::path::Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{name}.part"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, dir.join(name))
}

pub struct BakePlugin(pub Bakes);

impl Plugin for BakePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.0.clone()).add_message::<BakeDone>().add_systems(First, |bakes: Res<Bakes>, mut done: MessageWriter<BakeDone>| {
            for d in bakes.poll() { done.write(d); }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("beverly-bakes-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn wait(b: &Bakes) -> Vec<BakeDone> {
        loop {
            let d = b.poll();
            if !d.is_empty() { return d; }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn a_bake_is_named_by_its_source_and_settings() {
        let a = key("impostor", &[b"tree bytes", b"views 12"]);
        assert_eq!(a, key("impostor", &[b"tree bytes", b"views 12"]));
        assert_ne!(a, key("impostor", &[b"tree bytes", b"views 16"]));
        assert_ne!(a, key("impostor", &[b"tree bytes2", b"views 12"]));
        assert_ne!(a, key("lightprobe", &[b"tree bytes", b"views 12"]));
        // where one part ends matters
        assert_ne!(key("k", &[b"ab", b"c"]), key("k", &[b"a", b"bc"]));
    }

    #[test]
    fn a_missing_bake_is_built_once_in_the_background_and_kept() {
        let b = Bakes::new(temp("build"), None);
        assert!(b.get("k", "a").is_none());
        assert!(b.request("k", "a", "Thing", |p| { p.set(0.5); Ok(b"baked".to_vec()) }));
        assert!(!b.request("k", "a", "Thing", |_| Ok(vec![])), "already being built");
        let d = wait(&b);
        assert!(d[0].error.is_none() && d[0].label == "Thing");
        assert_eq!(b.get("k", "a").as_deref(), Some(&b"baked"[..]));
        assert!(!b.request("k", "a", "Thing", |_| Ok(vec![])), "cached");
        assert_eq!(b.size(), (1, 5));
        assert_eq!(b.clear(), 1);
        assert!(b.get("k", "a").is_none(), "built again when asked for");
    }

    #[test]
    fn a_failed_bake_says_why_and_leaves_nothing() {
        let b = Bakes::new(temp("fail"), None);
        b.request("k", "x", "Bad", |_| Err("no triangles".into()));
        b.request("k", "y", "Worse", |_| panic!("boom"));
        let mut errors = vec![];
        while errors.len() < 2 { errors.extend(wait(&b).into_iter().filter_map(|d| d.error)); }
        errors.sort();
        assert_eq!(errors, ["no triangles", "the bake crashed"]);
        assert_eq!(b.size().0, 0);
    }

    #[test]
    fn an_export_packs_what_was_used_and_a_game_reads_it_first_hand() {
        let b = Bakes::new(temp("pack"), None);
        b.request("k", "a", "A", |_| Ok(b"one".to_vec()));
        b.request("k", "b", "B", |_| Ok(b"two".to_vec()));
        let mut n = 0;
        while n < 2 { n += wait(&b).len(); }
        let fresh = Bakes::new(b.inner().root.clone(), None);
        fresh.get("k", "a");
        let files = fresh.pack();
        assert_eq!(files, [("bakes/k/a".to_string(), b"one".to_vec())], "only what this run used");
        // the game: its own empty cache, the packed folder beside it
        let game = temp("game");
        std::fs::create_dir_all(game.join("bakes/k")).unwrap();
        std::fs::write(game.join("bakes/k/a"), b"one").unwrap();
        let g = Bakes::new(temp("gamecache"), Some(game.join("bakes")));
        assert_eq!(g.get("k", "a").as_deref(), Some(&b"one"[..]));
        assert!(g.get("k", "b").is_none());
    }
}
