//! The health record: what Settings → Health shows, so problems are seen,
//! not remembered. Local only: nothing here is ever sent.
//!
//! `$XDG_STATE_HOME/tessera/health.jsonl`, one JSON object a line:
//!
//! - **Samples**, one per 5 minutes on the wall clock (`BUCKET_SECS`):
//!   `{"t":<bucket start>,"d":<seconds covered>,"rss":<KiB>,"pss":<KiB>,"cpu":<ms>}`
//!   and, only when not zero, `s16`/`s33` (loop turns that held the UI
//!   thread over 16 / 33 ms), `w` (the worst, ms), `fr`/`mf` (animation
//!   frames on screen / missed, when wp_presentation was asked: the wake
//!   log or the slow-GPU watch) and `c` (parts that panicked and were made
//!   again). Memory is read from /proc/self/smaps_rollup, CPU time from
//!   getrusage (the whole process, its threads too).
//! - **Events**, when they happen (rare): `{"t":…,"ev":"start","why":…}`
//!   (why: normal, crash, stopped, watchdog, retry; and the GPU),
//!   `stop` (an orderly end), `crash` (a part's panic, or the loop's:
//!   `fatal`, with the report's file name), `off` (a part turned off after
//!   3 crashes), `lock` (the screen locked again after a restart),
//!   `gpu-slow`; and from the scripts: `watchdog` (tty-shell.sh: it
//!   restarted a shell that stopped answering), `fallback`
//!   (launch-shell.sh: the QML shell started instead).
//!
//! Cheap by design: the loop turns are timed with two clock reads (the
//! poll's return, a calloop source with no file of its own: `Woke`, and the
//! turn's end), no timer of its own (a bucket is closed by the first turn
//! after its end, whatever woke the loop), the memory read and the write go
//! to the work pool. The file is a ring: past `CAP_BYTES` it keeps its
//! newest `KEEP_BYTES` (a week of samples is ~110 KB) and nothing older
//! than `KEEP_DAYS`.

use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A sample's length (s)
pub const BUCKET_SECS: i64 = 300;
/// The file's largest size…
pub const CAP_BYTES: u64 = 200 * 1024;
/// …and what's kept when it grows past it
pub const KEEP_BYTES: usize = 150 * 1024;
/// Nothing older is kept (days)
pub const KEEP_DAYS: i64 = 8;
/// A loop turn longer than this is a stall (ms): a 60 Hz frame
pub const STALL_MS: f64 = 16.0;
/// …and this, a long one: two frames
pub const LONG_STALL_MS: f64 = 33.0;

/// Where the record is
pub fn path() -> PathBuf {
    crate::util::xdg("XDG_STATE_HOME", ".local/state").join("tessera/health.jsonl")
}

/// What one bucket counted so far (the UI thread's)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Counts {
    pub s16: u32,
    pub s33: u32,
    pub worst_ms: f64,
    pub frames: u32,
    pub missed: u32,
    pub crashes: u32,
}

impl Counts {
    /// One loop turn took `ms`
    pub fn turn(&mut self, ms: f64) {
        if ms > STALL_MS {
            self.s16 += 1;
            if ms > LONG_STALL_MS {
                self.s33 += 1;
            }
            self.worst_ms = self.worst_ms.max(ms);
        }
    }
}

thread_local! {
    /// recording (the shell's loop, standalone: `start`)
    static ON: Cell<bool> = const { Cell::new(false) };
    /// when the loop's poll last returned (Woke)
    static WOKE: Cell<Option<Instant>> = const { Cell::new(None) };
    static NOW: RefCell<Counts> = RefCell::new(Counts::default());
    /// the bucket's wall-clock start (s) and when it ends (the loop's clock)
    static BUCKET: Cell<(i64, Option<Instant>)> = const { Cell::new((0, None)) };
    /// the process's CPU time (µs) at the bucket's start
    static CPU_AT: Cell<i64> = const { Cell::new(0) };
}

/// Recording on this thread (the shell's loop): the start event (its reason
/// worked out from the record's end, off this thread) and the first bucket
pub fn start() {
    ON.with(|o| o.set(true));
    ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
    begin_bucket(crate::util::now_ms() / 1000);
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default();
    let gpu = gpu_now();
    crate::work::spawn(move || {
        let path = path();
        let tail = read_tail(&path, 16 * 1024);
        let (why, report) = start_reason(&parse_lines(&tail), &sig);
        let mut e = json!({"t": crate::util::now_ms() / 1000, "ev": "start", "why": why, "s": sig});
        if !gpu.is_empty() {
            e["gpu"] = json!(gpu);
        }
        if let Some(r) = report {
            e["report"] = json!(r);
        }
        append(&path, &e.to_string());
    });
}

/// Recording, for events from any thread
static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Is it recording (on this thread)?
pub fn on() -> bool {
    ON.with(Cell::get)
}

/// What the shell draws on, in a few words ("Intel", or the renderer's name)
fn gpu_now() -> String {
    if let Some(f) = crate::gpu::drawing_family() {
        return crate::gpu::family_label(f).to_string();
    }
    let r = crate::egl::renderer();
    r.chars().take(60).collect()
}

fn begin_bucket(now_secs: i64) {
    let start = now_secs - now_secs.rem_euclid(BUCKET_SECS);
    let left = (start + BUCKET_SECS - now_secs).max(1) as u64;
    BUCKET.with(|b| b.set((start, Some(Instant::now() + Duration::from_secs(left)))));
    CPU_AT.with(|c| c.set(cpu_us()));
}

/// The process's processor time so far (µs, every thread)
fn cpu_us() -> i64 {
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) } != 0 {
        return 0;
    }
    let us = |t: libc::timeval| t.tv_sec * 1_000_000 + t.tv_usec;
    us(ru.ru_utime) + us(ru.ru_stime)
}

/// The loop's poll returned (Woke's before_handle_events)
#[inline]
pub fn woke() {
    if on() {
        WOKE.with(|w| w.set(Some(Instant::now())));
    }
}

/// The loop's turn ended: counted when it held the thread over a frame;
/// the bucket closed when its time is up
#[inline]
pub fn turn_end() {
    let Some(w) = WOKE.with(Cell::take) else { return };
    let now = Instant::now();
    let ms = now.duration_since(w).as_secs_f64() * 1000.0;
    if ms > STALL_MS {
        NOW.with(|c| c.borrow_mut().turn(ms));
    }
    if BUCKET.with(|b| b.get().1.is_some_and(|end| now >= end)) {
        close_bucket(false);
    }
}

/// wp_presentation: animation frames on screen, and how many were missed
pub fn frames(shown: u32, missed: u32) {
    if on() {
        NOW.with(|c| {
            let mut c = c.borrow_mut();
            c.frames += shown;
            c.missed += missed;
        });
    }
}

/// A part panicked (and is made again: contain.rs)
pub fn part_crashed() {
    if on() {
        NOW.with(|c| c.borrow_mut().crashes += 1);
    }
}

/// The bucket written (its memory read off this thread; `now` at the end:
/// on this one), the next begun
fn close_bucket(now: bool) {
    let (start, _) = BUCKET.with(Cell::get);
    let counts = NOW.with(|c| std::mem::take(&mut *c.borrow_mut()));
    let cpu_ms = (cpu_us() - CPU_AT.with(Cell::get)).max(0) / 1000;
    let end = crate::util::now_ms() / 1000;
    begin_bucket(end);
    let job = move || {
        let (rss, pss) = memory();
        append(&path(), &sample_line(start, (end - start).max(1), rss, pss, cpu_ms, &counts));
    };
    if now {
        job();
    } else {
        crate::work::spawn(job);
    }
}

/// The shell ends in order: the bucket so far and a `stop` (on this thread)
pub fn stop() {
    if !on() {
        return;
    }
    close_bucket(true);
    append(&path(), &json!({"t": crate::util::now_ms() / 1000, "ev": "stop"}).to_string());
    ON.with(|o| o.set(false));
    ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
}

/// One sample's line
pub fn sample_line(t: i64, d: i64, rss: u64, pss: u64, cpu_ms: i64, c: &Counts) -> String {
    let mut s = format!("{{\"t\":{t},\"d\":{d},\"rss\":{rss},\"pss\":{pss},\"cpu\":{cpu_ms}");
    for (k, v) in [("s16", c.s16), ("s33", c.s33), ("w", c.worst_ms.round() as u32), ("fr", c.frames), ("mf", c.missed), ("c", c.crashes)] {
        if v > 0 {
            s.push_str(&format!(",\"{k}\":{v}"));
        }
    }
    s.push('}');
    s
}

/// This process's memory (KiB): resident, and its fair share of what's
/// shared (proportional)
pub fn memory() -> (u64, u64) {
    let t = std::fs::read_to_string("/proc/self/smaps_rollup").unwrap_or_default();
    rollup(&t)
}

/// Rss and Pss of a smaps_rollup text (KiB)
pub fn rollup(text: &str) -> (u64, u64) {
    let field = |name: &str| {
        text.lines().find_map(|l| l.strip_prefix(name)).and_then(|r| r.trim().trim_end_matches("kB").trim().parse::<u64>().ok()).unwrap_or(0)
    };
    (field("Rss:"), field("Pss:"))
}

/// An event, written now (on the work pool: any thread may call it)
pub fn event(kind: &str, mut fields: Value) {
    if !ACTIVE.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    if !fields.is_object() {
        fields = json!({});
    }
    fields["t"] = json!(crate::util::now_ms() / 1000);
    fields["ev"] = json!(kind);
    let line = fields.to_string();
    crate::work::spawn(move || append(&path(), &line));
}

/// The same, written before this returns (a fatal panic: the process ends next)
pub fn event_now(kind: &str, mut fields: Value) {
    fields["t"] = json!(crate::util::now_ms() / 1000);
    fields["ev"] = json!(kind);
    append(&path(), &fields.to_string());
}

static WRITE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A line added; past the cap the file is cut to its newest lines
pub fn append(path: &Path, line: &str) {
    let _g = WRITE.lock();
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) else { return };
    let _ = f.write_all(format!("{line}\n").as_bytes());
    if f.metadata().is_ok_and(|m| m.len() > CAP_BYTES) {
        drop(f);
        trim(path, crate::util::now_ms() / 1000);
    }
}

/// Cut to the newest `KEEP_BYTES` of whole lines, none older than `KEEP_DAYS`
fn trim(path: &Path, now_secs: i64) {
    let Ok(text) = std::fs::read_to_string(path) else { return };
    let kept = ring(&text, KEEP_BYTES, now_secs - KEEP_DAYS * 86400);
    let tmp = path.with_extension("jsonl.tmp");
    if std::fs::write(&tmp, kept).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// The newest whole lines of `text` within `keep` bytes, none older than `since`
pub fn ring(text: &str, keep: usize, since: i64) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut size = 0;
    for l in text.lines().rev() {
        if l.trim().is_empty() {
            continue;
        }
        if size + l.len() + 1 > keep {
            break;
        }
        // (an old line: those before it are older still)
        if line_time(l).is_some_and(|t| t < since) {
            break;
        }
        size += l.len() + 1;
        out.push(l);
    }
    out.reverse();
    let mut s = out.join("\n");
    if !s.is_empty() {
        s.push('\n');
    }
    s
}

fn line_time(l: &str) -> Option<i64> {
    let i = l.find("\"t\":")? + 4;
    let rest = &l[i..];
    let end = rest.find(|c: char| !c.is_ascii_digit() && c != '-').unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// The file's last `n` bytes, from its first whole line
fn read_tail(path: &Path, n: u64) -> String {
    let Ok(mut f) = std::fs::File::open(path) else { return String::new() };
    let len = f.metadata().map_or(0, |m| m.len());
    let from = len.saturating_sub(n);
    if f.seek(SeekFrom::Start(from)).is_err() {
        return String::new();
    }
    let mut buf = Vec::new();
    let _ = f.read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf).into_owned();
    if from > 0 {
        text.split_once('\n').map(|x| x.1.to_string()).unwrap_or_default()
    } else {
        text
    }
}

/// The lines that are JSON objects
pub fn parse_lines(text: &str) -> Vec<Value> {
    text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(Value::is_object).collect()
}

/// Why this start: what the record says after the last start (`sig`: this
/// Hyprland session's). normal (the last run ended in order, or with its
/// session), crash (the loop panicked: its report), watchdog (it stopped
/// answering and was restarted), retry (the QML shell stood in since),
/// stopped (it ended without a word: killed, out of memory, a crash in a
/// library)
pub fn start_reason(lines: &[Value], sig: &str) -> (&'static str, Option<String>) {
    let Some(last) = lines.iter().rposition(|v| v["ev"] == "start") else { return ("normal", None) };
    let after = &lines[last + 1..];
    let has = |ev: &str| after.iter().any(|v| v["ev"] == ev);
    if has("watchdog") {
        return ("watchdog", None);
    }
    if has("fallback") {
        return ("retry", None);
    }
    if let Some(c) = after.iter().rev().find(|v| v["ev"] == "crash" && v["fatal"] == true) {
        return ("crash", c["report"].as_str().map(String::from));
    }
    if has("stop") {
        return ("normal", None);
    }
    // (a run of another session that never said stop: the computer was
    // turned off or the session ended under it, not a crash)
    let before = lines[last]["s"].as_str().unwrap_or("");
    if before != sig {
        return ("normal", None);
    }
    ("stopped", None)
}

// ── Reading it back (Settings → Health) ─────────────────────────────────

/// One sample read back
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sample {
    pub t: i64,
    pub d: i64,
    /// KiB
    pub rss: u64,
    pub pss: u64,
    pub cpu_ms: u64,
    pub s16: u64,
    pub s33: u64,
    pub worst: u64,
    pub frames: u64,
    pub missed: u64,
    pub crashes: u64,
}

impl Sample {
    pub fn from(v: &Value) -> Option<Sample> {
        if !v["ev"].is_null() {
            return None;
        }
        let n = |k: &str| v[k].as_u64().unwrap_or(0);
        Some(Sample {
            t: v["t"].as_i64()?,
            d: v["d"].as_i64().unwrap_or(BUCKET_SECS).max(1),
            rss: n("rss"),
            pss: n("pss"),
            cpu_ms: n("cpu"),
            s16: n("s16"),
            s33: n("s33"),
            worst: n("w"),
            frames: n("fr"),
            missed: n("mf"),
            crashes: n("c"),
        })
    }
    /// Memory (KiB): the fair share when it was read, else resident
    pub fn mem(&self) -> u64 {
        if self.pss > 0 { self.pss } else { self.rss }
    }
}

/// The record read: its samples and its events, oldest first
pub fn read_all() -> (Vec<Sample>, Vec<Value>) {
    split(&parse_lines(&std::fs::read_to_string(path()).unwrap_or_default()))
}

pub fn split(lines: &[Value]) -> (Vec<Sample>, Vec<Value>) {
    let mut samples: Vec<Sample> = lines.iter().filter_map(Sample::from).collect();
    let mut events: Vec<Value> = lines.iter().filter(|v| v["ev"].is_string() && v["t"].is_i64()).cloned().collect();
    samples.sort_by_key(|s| s.t);
    events.sort_by_key(|v| v["t"].as_i64().unwrap_or(0));
    (samples, events)
}

/// The bucket still being counted (the UI thread's), as a sample with no
/// memory: Settings shows it with the memory it reads itself
pub fn live() -> Option<Sample> {
    if !on() {
        return None;
    }
    let (start, _) = BUCKET.with(Cell::get);
    let c = NOW.with(|c| c.borrow().clone());
    let now = crate::util::now_ms() / 1000;
    Some(Sample {
        t: start,
        d: (now - start).max(1),
        cpu_ms: ((cpu_us() - CPU_AT.with(Cell::get)).max(0) / 1000) as u64,
        s16: c.s16 as u64,
        s33: c.s33 as u64,
        worst: c.worst_ms.round() as u64,
        frames: c.frames as u64,
        missed: c.missed as u64,
        crashes: c.crashes as u64,
        ..Default::default()
    })
}

/// One slot of a chart
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Slot {
    pub start: i64,
    /// the samples in it
    pub n: u32,
    /// memory: the average and the highest (KiB)
    pub mem_avg: u64,
    pub mem_max: u64,
    /// processor: % of one core over the time recorded
    pub cpu_pct: f64,
    /// stalls over 16 ms, per hour recorded
    pub stalls_per_hour: f64,
    pub stalls: u64,
    pub long_stalls: u64,
    pub worst: u64,
}

/// The samples from `from` laid in `count` slots of `secs` each
pub fn slots(samples: &[Sample], from: i64, secs: i64, count: usize) -> Vec<Slot> {
    let mut out: Vec<Slot> = (0..count).map(|i| Slot { start: from + i as i64 * secs, ..Default::default() }).collect();
    let mut acc: Vec<(u64, u64, i64, u64)> = vec![(0, 0, 0, 0); count]; // mem sum, mem samples, seconds, cpu ms
    for s in samples {
        if s.t < from {
            continue;
        }
        let i = ((s.t - from) / secs) as usize;
        let Some(o) = out.get_mut(i) else { continue };
        o.n += 1;
        let m = s.mem();
        if m > 0 {
            acc[i].0 += m;
            acc[i].1 += 1;
            o.mem_max = o.mem_max.max(m);
        }
        acc[i].2 += s.d;
        acc[i].3 += s.cpu_ms;
        o.stalls += s.s16;
        o.long_stalls += s.s33;
        o.worst = o.worst.max(s.worst);
    }
    for (o, a) in out.iter_mut().zip(acc) {
        if a.1 > 0 {
            o.mem_avg = a.0 / a.1;
        }
        if a.2 > 0 {
            o.cpu_pct = a.3 as f64 / (a.2 as f64 * 1000.0) * 100.0;
            o.stalls_per_hour = o.stalls as f64 * 3600.0 / a.2 as f64;
        }
    }
    out
}

/// The loop's poll returning, timed for `turn_end`: a calloop source with no
/// file of its own, only the lifecycle call every dispatch makes
pub struct Woke;

impl calloop::EventSource for Woke {
    type Event = ();
    type Metadata = ();
    type Ret = ();
    type Error = std::io::Error;
    const NEEDS_EXTRA_LIFECYCLE_EVENTS: bool = true;

    fn process_events<F>(&mut self, _: calloop::Readiness, _: calloop::Token, _: F) -> Result<calloop::PostAction, Self::Error>
    where
        F: FnMut((), &mut ()),
    {
        Ok(calloop::PostAction::Continue)
    }
    fn register(&mut self, _: &mut calloop::Poll, _: &mut calloop::TokenFactory) -> calloop::Result<()> {
        Ok(())
    }
    fn reregister(&mut self, _: &mut calloop::Poll, _: &mut calloop::TokenFactory) -> calloop::Result<()> {
        Ok(())
    }
    fn unregister(&mut self, _: &mut calloop::Poll) -> calloop::Result<()> {
        Ok(())
    }
    fn before_handle_events(&mut self, _: calloop::EventIterator) {
        woke();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_count_as_stalls_over_a_frame() {
        let mut c = Counts::default();
        for ms in [3.0, 15.9, 16.5, 20.0, 40.0, 120.4] {
            c.turn(ms);
        }
        assert_eq!((c.s16, c.s33), (4, 2));
        assert_eq!(c.worst_ms, 120.4);
    }

    #[test]
    fn samples_are_short_lines_that_read_back() {
        let quiet = sample_line(1_763_380_800, 300, 61234, 50123, 912, &Counts::default());
        assert_eq!(quiet, r#"{"t":1763380800,"d":300,"rss":61234,"pss":50123,"cpu":912}"#);
        let c = Counts { s16: 3, s33: 1, worst_ms: 41.6, frames: 120, missed: 4, crashes: 1 };
        let busy = sample_line(1_763_381_100, 300, 1, 2, 3, &c);
        let s = Sample::from(&serde_json::from_str(&busy).unwrap()).unwrap();
        assert_eq!((s.s16, s.s33, s.worst, s.frames, s.missed, s.crashes, s.mem()), (3, 1, 42, 120, 4, 1, 2));
        // (a week of quiet samples fits the cap with room)
        assert!(quiet.len() * 7 * 288 < KEEP_BYTES);
        // (an event isn't a sample)
        assert!(Sample::from(&json!({"t": 1, "ev": "start"})).is_none());
    }

    #[test]
    fn buckets_lie_on_the_wall_clock() {
        // (the first bucket of a start mid-way ends at the next 5-minute mark)
        let now: i64 = 1_763_380_800 + 137;
        assert_eq!(now - now.rem_euclid(BUCKET_SECS), 1_763_380_800);
        let samples = vec![
            Sample { t: 1000 * 300, d: 300, pss: 100, cpu_ms: 3000, s16: 2, ..Default::default() },
            Sample { t: 1001 * 300, d: 300, pss: 300, cpu_ms: 0, s16: 1, s33: 1, worst: 50, ..Default::default() },
            Sample { t: 1012 * 300, d: 300, rss: 900, ..Default::default() },
            // (before the first slot, after the last: left out)
            Sample { t: 999 * 300, d: 300, pss: 5, ..Default::default() },
            Sample { t: 1030 * 300, d: 300, pss: 5, ..Default::default() },
        ];
        let s = slots(&samples, 1000 * 300, 3600, 2);
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].n, s[0].mem_avg, s[0].mem_max, s[0].stalls, s[0].long_stalls, s[0].worst), (2, 200, 300, 3, 1, 50));
        assert!((s[0].cpu_pct - 0.5).abs() < 1e-9, "{}", s[0].cpu_pct);
        assert!((s[0].stalls_per_hour - 18.0).abs() < 1e-9);
        assert_eq!((s[1].n, s[1].mem_avg, s[1].start), (1, 900, 1000 * 300 + 3600));
    }

    #[test]
    fn the_ring_keeps_the_newest_lines() {
        let text: String = (0..1000).map(|i| format!("{{\"t\":{},\"d\":300,\"rss\":1,\"pss\":1,\"cpu\":1}}\n", 10_000 + i)).collect();
        let kept = ring(&text, 4096, 0);
        assert!(kept.len() <= 4096 && kept.len() > 4000, "{}", kept.len());
        assert!(kept.ends_with("\"t\":10999,\"d\":300,\"rss\":1,\"pss\":1,\"cpu\":1}\n"));
        assert!(kept.lines().all(|l| serde_json::from_str::<Value>(l).is_ok()));
        // (nothing older than the cut-off)
        let recent = ring(&text, 1 << 20, 10_990);
        assert_eq!(recent.lines().count(), 10);
        // (the file itself: past the cap, cut)
        let d = std::env::temp_dir().join(format!("tessera-health-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let p = d.join("health.jsonl");
        let now = crate::util::now_ms() / 1000;
        let line = format!("{{\"t\":{now},\"d\":300,\"rss\":61234,\"pss\":50123,\"cpu\":912}}");
        for _ in 0..(CAP_BYTES as usize / line.len() + 50) {
            append(&p, &line);
        }
        let size = std::fs::metadata(&p).unwrap().len();
        assert!(size <= CAP_BYTES, "{size}");
        assert!(size >= (KEEP_BYTES - line.len()) as u64 / 2, "{size}");
        assert!(std::fs::read_to_string(&p).unwrap().lines().all(|l| l == line));
        // (the tail read at start begins at a whole line)
        let tail = read_tail(&p, 1000);
        assert!(tail.lines().all(|l| l == line) && !tail.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn rollup_reads_rss_and_pss() {
        let t = "55d0-7ff [rollup]\nRss:              123456 kB\nPss:               98765 kB\nPss_Anon:          50000 kB\n";
        assert_eq!(rollup(t), (123456, 98765));
        assert_eq!(rollup(""), (0, 0));
    }

    #[test]
    fn why_it_started() {
        let l = |s: &str| parse_lines(s);
        assert_eq!(start_reason(&[], "a"), ("normal", None));
        // (ended in order)
        assert_eq!(start_reason(&l("{\"t\":1,\"ev\":\"start\",\"s\":\"a\"}\n{\"t\":2,\"d\":300}\n{\"t\":3,\"ev\":\"stop\"}"), "a").0, "normal");
        // (no stop, same session: it stopped without a word)
        assert_eq!(start_reason(&l("{\"t\":1,\"ev\":\"start\",\"s\":\"a\"}\n{\"t\":2,\"d\":300}"), "a").0, "stopped");
        // (no stop, another session: the computer went off under it)
        assert_eq!(start_reason(&l("{\"t\":1,\"ev\":\"start\",\"s\":\"a\"}"), "b").0, "normal");
        // (the loop panicked: its report)
        let crash = l("{\"t\":1,\"ev\":\"start\",\"s\":\"a\"}\n{\"t\":2,\"ev\":\"crash\",\"part\":\"dock\",\"report\":\"x-dock.txt\"}\n{\"t\":3,\"ev\":\"crash\",\"part\":\"shell\",\"fatal\":true,\"report\":\"y-shell.txt\"}");
        assert_eq!(start_reason(&crash, "a"), ("crash", Some("y-shell.txt".into())));
        // (a part's crash alone isn't the shell's end)
        assert_eq!(start_reason(&l("{\"t\":1,\"ev\":\"start\",\"s\":\"a\"}\n{\"t\":2,\"ev\":\"crash\",\"part\":\"dock\"}\n{\"t\":3,\"ev\":\"stop\"}"), "a").0, "normal");
        // (the watchdog restarted it, even if it then ended in order)
        assert_eq!(start_reason(&l("{\"t\":1,\"ev\":\"start\",\"s\":\"a\"}\n{\"t\":2,\"ev\":\"watchdog\"}\n{\"t\":3,\"ev\":\"stop\"}"), "a").0, "watchdog");
        // (the QML shell stood in)
        assert_eq!(start_reason(&l("{\"t\":1,\"ev\":\"start\",\"s\":\"a\"}\n{\"t\":2,\"ev\":\"fallback\",\"why\":\"x\"}"), "a").0, "retry");
        // (only what came after the last start counts)
        assert_eq!(start_reason(&l("{\"t\":1,\"ev\":\"start\",\"s\":\"a\"}\n{\"t\":2,\"ev\":\"watchdog\"}\n{\"t\":3,\"ev\":\"start\",\"s\":\"a\"}\n{\"t\":4,\"ev\":\"stop\"}"), "a").0, "normal");
        // (a broken line is skipped)
        assert_eq!(parse_lines("{\"t\":1}\nnot json\n{\"t\":\n[1]\n").len(), 1);
    }

    #[test]
    fn events_and_samples_part() {
        let (s, e) = split(&parse_lines("{\"t\":5,\"ev\":\"stop\"}\n{\"t\":600,\"d\":300,\"pss\":1}\n{\"t\":300,\"d\":300,\"pss\":2}\n{\"t\":2,\"ev\":\"start\"}"));
        assert_eq!(s.iter().map(|s| s.t).collect::<Vec<_>>(), vec![300, 600]);
        assert_eq!(e.iter().map(|v| v["t"].as_i64().unwrap()).collect::<Vec<_>>(), vec![2, 5]);
    }
}
