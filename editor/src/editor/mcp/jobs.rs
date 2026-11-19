//! Work that takes longer than one tool call may wait (scripted tests, exports): started by `run_job`, run on a
//! thread, checked with `get_job`. Each job works on the saved project files, so the editor stays responsive.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// How many finished jobs are kept.
const KEEP: usize = 20;

#[derive(Default)]
struct State {
    done: bool,
    ok: bool,
    /// How long it took, once done.
    took: f32,
    /// What happened so far, a line per step (a test's report, the export's file).
    lines: Vec<String>,
}

pub struct Job {
    pub id: u32,
    pub what: String,
    started: Instant,
    state: Arc<Mutex<State>>,
}

impl Job {
    pub fn describe(&self, full: bool) -> Value {
        let s = self.state.lock().unwrap();
        let mut v = json!({
            "id": self.id, "what": self.what,
            "status": if !s.done { "running" } else if s.ok { "passed" } else { "failed" },
            "seconds": crate::settings::num(((if s.done { s.took } else { self.started.elapsed().as_secs_f32() }) * 10.0).round() / 10.0),
        });
        if full { v["output"] = json!(s.lines.join("\n")); }
        v
    }
}

#[derive(Default)]
pub struct Jobs {
    list: Vec<Job>,
    next: u32,
}

impl Jobs {
    pub fn get(&self, id: u32) -> Option<&Job> { self.list.iter().find(|j| j.id == id) }
    pub fn all(&self) -> &[Job] { &self.list }

    /// Start `work` on a thread; it adds lines as it goes and returns whether it went well.
    fn start(&mut self, what: String, work: impl FnOnce(&mut dyn FnMut(String)) -> bool + Send + 'static) -> u32 {
        self.next += 1;
        let state = Arc::new(Mutex::new(State::default()));
        let s = state.clone();
        let started = Instant::now();
        std::thread::spawn(move || {
            let ok = work(&mut |line| s.lock().unwrap().lines.push(line));
            let mut s = s.lock().unwrap();
            s.ok = ok;
            s.took = started.elapsed().as_secs_f32();
            s.done = true;
        });
        self.list.push(Job { id: self.next, what, started, state });
        // forget the oldest finished ones
        while self.list.len() > KEEP {
            match self.list.iter().position(|j| j.state.lock().unwrap().done) { Some(i) => { self.list.remove(i); } None => break }
        }
        self.next
    }

    /// Run scripted tests (all of them, or one by name) with the game player, one after another.
    pub fn tests(&mut self, player: PathBuf, dir: PathBuf, names: Vec<String>) -> u32 {
        let what = if names.len() == 1 { format!("test {}", names[0]) } else { format!("{} tests", names.len()) };
        self.start(what, move |say| {
            let mut ok = true;
            for n in &names {
                let file = Path::new("tests").join(format!("{n}.ron"));
                let out = std::process::Command::new(&player).arg(&dir).arg("--test").arg(&file).output();
                let report = std::fs::read_to_string(dir.join("test-output").join(n).join("report.txt")).unwrap_or_default();
                match out {
                    Ok(o) => {
                        let code = o.status.code().unwrap_or(-1);
                        ok &= code == 0;
                        say(format!("{n}: {}", match code { 0 => "passed".into(), 1 => "FAILED".into(), 2 => "bad test file".into(), c => format!("stopped ({c})") }));
                        if code != 0 || report.is_empty() { say(String::from_utf8_lossy(&o.stderr).lines().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n")); }
                        say(report);
                    }
                    Err(e) => {
                        ok = false;
                        say(format!("{n}: couldn't start the game player: {e}"));
                    }
                }
            }
            ok
        })
    }

    /// Export the Windows game zip to `out`.
    pub fn export(&mut self, player: PathBuf, dir: PathBuf, name: String, bakes: Vec<(String, Vec<u8>)>, out: PathBuf) -> u32 {
        self.start(format!("export {}", out.display()), move |say| match crate::level::export_zip(&dir, &name, &player, &bakes, &out) {
            Ok(n) => {
                say(format!("Wrote {} ({n} files): unzip and run {name}.exe", out.display()));
                true
            }
            Err(e) => {
                say(format!("Couldn't export: {e}"));
                false
            }
        })
    }
}

/// The scripted tests in a project: tests/*.ron, by name.
pub fn tests_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir.join("tests")).into_iter().flatten().flatten()
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".ron")).map(str::to_string))
        .collect();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_runs_on_its_own_thread_and_keeps_its_output_and_time() {
        let mut jobs = Jobs::default();
        let id = jobs.start("sum".into(), |say| { say("1 + 1 = 2".into()); true });
        let bad = jobs.start("fail".into(), |_| false);
        while jobs.all().iter().any(|j| j.describe(false)["status"] == "running") { std::thread::sleep(std::time::Duration::from_millis(5)); }
        let d = jobs.get(id).unwrap().describe(true);
        assert_eq!((d["status"].as_str(), d["output"].as_str()), (Some("passed"), Some("1 + 1 = 2")));
        assert_eq!(jobs.get(bad).unwrap().describe(false)["status"], "failed");
        let t = d["seconds"].clone();
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert_eq!(jobs.get(id).unwrap().describe(false)["seconds"], t, "the clock stops when it's done");
    }
}
