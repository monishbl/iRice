//! Processes and timers for the sharing services: what Quickshell's Process,
//! StdioCollector, SplitParser, KeptProcess and Timer give the QML services,
//! in a form the host's loop can use. Nothing here blocks the loop: output
//! and exits come back as `M` messages (lib.rs), handled by `Sharing::message`.
//!
//! Children that live a while are started on the loop's thread (so
//! PR_SET_PDEATHSIG ties them to the shell as `setpriv --pdeathsig TERM` and
//! `Tools.watcher` do in the QML: they end with it) and watched from threads.

use crate::M;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

pub type Send_ = Box<dyn Fn(Box<dyn std::any::Any + Send>) + Send>;

/// One long-lived child: its generation (messages from an older one are
/// ignored), its pid, its stdin (when it takes lines)
#[derive(Default)]
pub struct Kept {
    pub gen: u64,
    pub pid: i32,
    pub stdin: Option<ChildStdin>,
    pub running: bool,
}

impl Kept {
    /// A line to its stdin (Process.write); false when it isn't running
    pub fn write(&mut self, line: &str) -> bool {
        match self.stdin.as_mut() {
            Some(i) => i.write_all(line.as_bytes()).and_then(|_| i.flush()).is_ok(),
            None => false,
        }
    }
    /// Process.running = false: stdin closed (helpers end with it), then SIGTERM
    pub fn stop(&mut self) {
        self.stdin = None;
        if self.running && self.pid > 0 {
            unsafe {
                libc::kill(self.pid, libc::SIGTERM);
            }
        }
    }
    /// Process.signal(n)
    pub fn signal(&self, sig: i32) {
        if self.running && self.pid > 0 {
            unsafe {
                libc::kill(self.pid, sig);
            }
        }
    }
    /// It has ended (its Exit arrived)
    pub fn ended(&mut self) {
        self.running = false;
        self.stdin = None;
        self.pid = 0;
    }
}

fn command(cmd: &[String], stdin: Stdio, die_with_us: bool) -> Option<Command> {
    let (prog, args) = cmd.split_first()?;
    let mut c = Command::new(prog);
    c.args(args).stdin(stdin).stdout(Stdio::piped()).stderr(Stdio::piped());
    unsafe {
        c.pre_exec(move || {
            if die_with_us {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            }
            Ok(())
        });
    }
    Some(c)
}

fn tail(lines: &mut Vec<String>, line: String) {
    lines.push(line);
    if lines.len() > 20 {
        lines.remove(0);
    }
}

/// Start a child that stays: its stdout lines come back as `M::Line(name,
/// gen, line)`, its end as `M::Exit(name, gen, code, last stderr lines)`.
/// None when it can't start (a missing program: Quickshell emits no
/// exited() then either; callers treat that as an end).
pub fn spawn(send: impl Fn(Box<dyn std::any::Any + Send>) + Send + Clone + 'static, name: &'static str, gen: u64, cmd: &[String], env: &[(&str, &str)], with_stdin: bool, die_with_us: bool) -> Option<Kept> {
    let mut c = command(cmd, if with_stdin { Stdio::piped() } else { Stdio::null() }, die_with_us)?;
    for (k, v) in env {
        c.env(k, v);
    }
    let mut child: Child = c.spawn().ok()?;
    let pid = child.id() as i32;
    let stdin = child.stdin.take();
    let out = child.stdout.take();
    let err = child.stderr.take();
    let s2 = send.clone();
    std::thread::spawn(move || {
        let errs = err.map(|e| {
            std::thread::spawn(move || {
                let mut last = Vec::new();
                for l in BufReader::new(e).split(b'\n').flatten() {
                    tail(&mut last, String::from_utf8_lossy(&l).into_owned());
                }
                last
            })
        });
        if let Some(o) = out {
            for l in BufReader::new(o).split(b'\n').flatten() {
                s2(Box::new(M::Line(name, gen, String::from_utf8_lossy(&l).into_owned())));
            }
        }
        let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
        let last = errs.and_then(|j| j.join().ok()).unwrap_or_default();
        s2(Box::new(M::Exit(name, gen, code, last.join("\n"))));
    });
    Some(Kept { gen, pid, stdin, running: true })
}

/// A command whose whole output is wanted (StdioCollector): the answer comes
/// back as `M::Out(job, stdout, stderr, code)`. `input` goes to its stdin.
pub fn run(send: impl Fn(Box<dyn std::any::Any + Send>) + Send + Clone + 'static, job: crate::Job, cmd: &[String], input: Option<Vec<u8>>) {
    let Some(mut c) = command(cmd, if input.is_some() { Stdio::piped() } else { Stdio::null() }, false) else {
        send(Box::new(M::Out(job, String::new(), String::new(), -1)));
        return;
    };
    match c.spawn() {
        Ok(mut child) => {
            let stdin = child.stdin.take();
            std::thread::spawn(move || {
                if let (Some(mut i), Some(bytes)) = (stdin, input) {
                    let _ = i.write_all(&bytes);
                }
                let (so, se, code) = match child.wait_with_output() {
                    Ok(o) => (String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned(), o.status.code().unwrap_or(-1)),
                    Err(_) => (String::new(), String::new(), -1),
                };
                send(Box::new(M::Out(job, so, se, code)));
            });
        }
        Err(_) => send(Box::new(M::Out(job, String::new(), String::new(), -1))),
    }
}

/// Quickshell.execDetached: started, not waited on by anyone (a thread reaps it)
pub fn detach(cmd: &[&str]) {
    let Some((prog, args)) = cmd.split_first() else { return };
    let mut c = Command::new(prog);
    c.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    if let Ok(mut child) = c.spawn() {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

pub fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// Named deadlines, one slint::Timer for the nearest (the QML services have
/// a Timer each; here they share one, so an idle shell wakes for none)
#[derive(Default)]
pub struct Sched {
    due: Vec<(&'static str, Instant)>,
    timer: slint::Timer,
}

impl Sched {
    /// Due in `ms` (replacing an earlier setting of the same name)
    pub fn set(&mut self, name: &'static str, ms: u64) {
        self.due.retain(|(n, _)| *n != name);
        self.due.push((name, Instant::now() + Duration::from_millis(ms)));
    }
    /// Due in `ms`, unless it is due sooner already
    pub fn set_if_later(&mut self, name: &'static str, ms: u64) {
        if !self.has(name) {
            self.set(name, ms);
        }
    }
    pub fn cancel(&mut self, name: &'static str) {
        self.due.retain(|(n, _)| *n != name);
    }
    pub fn has(&self, name: &'static str) -> bool {
        self.due.iter().any(|(n, _)| *n == name)
    }
    /// The names whose time has come (removed)
    pub fn take(&mut self) -> Vec<&'static str> {
        let now = Instant::now();
        let mut out = Vec::new();
        self.due.retain(|(n, t)| {
            if *t <= now {
                out.push(*n);
                false
            } else {
                true
            }
        });
        out
    }
    /// Set the timer for the nearest deadline (after any change)
    pub fn arm(&mut self, send: impl Fn(Box<dyn std::any::Any + Send>) + 'static) {
        let Some(next) = self.due.iter().map(|(_, t)| *t).min() else {
            self.timer.stop();
            return;
        };
        let wait = next.saturating_duration_since(Instant::now()).max(Duration::from_millis(1));
        self.timer.start(slint::TimerMode::SingleShot, wait, move || send(Box::new(M::Due)));
    }
}

/// KeptProcess's rest between a run's end and the next start: firstWait
/// doubling for each quick end (a run of a minute or more starts over), up
/// to maxWait
#[derive(Default)]
pub struct Backoff {
    pub quick_ends: u32,
    pub started: Option<Instant>,
}

impl Backoff {
    pub fn began(&mut self) {
        self.started = Some(Instant::now());
    }
    /// The wait after an end, in ms
    pub fn ended(&mut self, first_wait: u64, max_wait: u64) -> u64 {
        if self.started.is_some_and(|t| t.elapsed() >= Duration::from_secs(60)) {
            self.quick_ends = 0;
        }
        let ms = (first_wait.saturating_mul(1u64 << self.quick_ends.min(20))).min(max_wait);
        self.quick_ends += 1;
        ms
    }
    pub fn reset(&mut self) {
        self.quick_ends = 0;
    }
}

/// A string of JSON per line out of a line of a child's stdout
pub fn json_line(line: &str) -> Option<serde_json::Value> {
    serde_json::from_str(line.trim()).ok()
}

/// Read a whole stream to a string (small files, a pipe)
pub fn read_all(mut r: impl Read) -> String {
    let mut s = String::new();
    let _ = r.read_to_string(&mut s);
    s
}

static GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
/// A number for each child started, so messages from an older one are ignored
pub fn next_gen() -> u64 {
    GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// A KeptProcess: a process kept running while `wanted`, started again after
/// it ends (or fails to start), waiting longer each time it ends quickly
/// (`first_wait` doubling up to 5 min; a run of a minute or more starts over).
pub struct Keep {
    pub kept: Kept,
    pub backoff: Backoff,
    /// the message name its output comes under
    pub name: &'static str,
    /// the deadline name of its rest
    pub rest: &'static str,
}

impl Keep {
    pub fn new(name: &'static str, rest: &'static str) -> Self {
        Self { kept: Kept::default(), backoff: Backoff::default(), name, rest }
    }
    /// Make it so (call every turn)
    pub fn sync(&mut self, sched: &mut Sched, send: impl Fn(Box<dyn std::any::Any + Send>) + Send + Clone + 'static, wanted: bool, cmd: &[String], first_wait: u64, with_stdin: bool) {
        if wanted && !self.kept.running && !sched.has(self.rest) {
            let gen = next_gen();
            match spawn(send, self.name, gen, cmd, &[], with_stdin, true) {
                Some(k) => {
                    self.kept = k;
                    self.backoff.began();
                }
                None => {
                    // (a missing binary emits no exited(): the rest starts all the same)
                    self.kept = Kept { gen, ..Default::default() };
                    let ms = self.backoff.ended(first_wait, 300_000);
                    sched.set(self.rest, ms);
                }
            }
        } else if !wanted && self.kept.running {
            self.kept.stop();
        }
        if !wanted {
            sched.cancel(self.rest);
            self.backoff.reset();
        }
    }
    /// Its Exit arrived (false: an older one's, ignore)
    pub fn ended(&mut self, sched: &mut Sched, gen: u64, wanted: bool, first_wait: u64) -> bool {
        if gen != self.kept.gen {
            return false;
        }
        self.kept.ended();
        if wanted {
            let ms = self.backoff.ended(first_wait, 300_000);
            sched.set(self.rest, ms);
        }
        true
    }
    pub fn is(&self, gen: u64) -> bool {
        gen == self.kept.gen
    }
}
