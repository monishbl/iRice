//! Health (Rust only): how Tessera has been doing, from its own record
//! (tessera_core::health: a sample each 5 minutes, and the crashes, restarts
//! and fallbacks as they happened). At the top one line says how it's
//! going; then memory and processor over 24 hours or 7 days, the slow
//! moments, and the recent events in plain words, each with the one thing
//! worth doing about it (save its crash report, use the faster GPU).
//! Read off the UI thread when the page opens; nothing is sent anywhere.

use super::super::super::model::{btn, Ctx, Opt, Page, Row};
use serde_json::{json, Value};
use tessera_core::health::{self, Sample};

/// The record, read off the UI thread: its samples (with the bucket still
/// being counted, and the memory now), its events, the crash reports kept
fn load(live: Option<Sample>) -> Value {
    let (mut samples, events) = health::read_all();
    let (rss, pss) = health::memory();
    let recording = live.is_some();
    if let Some(mut l) = live {
        (l.rss, l.pss) = (rss, pss);
        samples.push(l);
    }
    let reports: Vec<String> = tessera_core::crash::list().iter().filter_map(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).collect();
    json!({
        "now": tessera_core::util::now_ms() / 1000,
        "mem": if pss > 0 { pss } else { rss },
        "recording": recording,
        "samples": samples.iter().map(|s| json!([s.t, s.d, s.mem(), s.cpu_ms, s.s16, s.s33, s.worst, s.crashes, s.frames, s.missed])).collect::<Vec<_>>(),
        "events": events,
        "reports": reports,
    })
}

fn samples_of(v: &Value) -> Vec<Sample> {
    let n = |a: &Value, i: usize| a[i].as_u64().unwrap_or(0);
    v["samples"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|s| Sample { t: s[0].as_i64().unwrap_or(0), d: s[1].as_i64().unwrap_or(300).max(1), pss: n(s, 2), cpu_ms: n(s, 3), s16: n(s, 4), s33: n(s, 5), worst: n(s, 6), crashes: n(s, 7), frames: n(s, 8), missed: n(s, 9), ..Default::default() })
                .collect()
        })
        .unwrap_or_default()
}

/// "142 MB" from KiB
pub fn mb(kib: u64) -> String {
    let m = kib as f64 / 1024.0;
    if m >= 1024.0 {
        format!("{:.1} GB", m / 1024.0)
    } else {
        format!("{} MB", m.round())
    }
}

/// "0.8%", "12%"
pub fn pct(p: f64) -> String {
    if p < 10.0 {
        format!("{p:.1}%")
    } else {
        format!("{}%", p.round())
    }
}

/// "1 restart", "2 restarts"
fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

/// When, in words: "Today 14:02", "Yesterday 09:10", "Mon 16 Nov 14:02"
pub fn when(t: i64, now: i64) -> String {
    let day = tessera_core::util::local_day;
    let hm = tessera_core::util::strftime("%H:%M", t);
    match day(now) - day(t) {
        0 => format!("Today {hm}"),
        1 => format!("Yesterday {hm}"),
        _ => tessera_core::util::strftime("%a %e %b %H:%M", t).split_whitespace().collect::<Vec<_>>().join(" "),
    }
}

/// "14:02", "yesterday 09:10", "Mon 16 Nov"
pub fn since(t: i64, now: i64) -> String {
    let day = tessera_core::util::local_day;
    match day(now) - day(t) {
        0 => tessera_core::util::strftime("%H:%M", t),
        1 => format!("yesterday {}", tessera_core::util::strftime("%H:%M", t)),
        _ => tessera_core::util::strftime("%a %e %b", t).split_whitespace().collect::<Vec<_>>().join(" "),
    }
}

/// A part's name in a sentence ("the dock")
fn part_name(part: &str) -> String {
    let n = tessera_core::contain::display_name(part);
    match n.strip_prefix("The ") {
        Some(rest) => format!("the {rest}"),
        None => n,
    }
}

fn cap(t: &str) -> String {
    let mut c = t.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// A start that wasn't asked for (the shell before ended badly)
fn is_restart(e: &Value) -> bool {
    e["ev"] == "start" && matches!(e["why"].as_str(), Some("crash" | "stopped" | "watchdog"))
}

/// The line at the top: (what, more, all well)
pub fn status(events: &[Value], samples: &[Sample], now: i64) -> (String, String, bool) {
    let day = tessera_core::util::local_day;
    let today = |e: &&Value| e["t"].as_i64().is_some_and(|t| day(t) == day(now));
    let week = |e: &&Value| e["t"].as_i64().is_some_and(|t| now - t < 7 * 86400);
    let restarts = events.iter().filter(today).filter(|e| is_restart(e)).count();
    let fallbacks = events.iter().filter(today).filter(|e| e["ev"] == "fallback").count();
    let parts = events.iter().filter(today).filter(|e| e["ev"] == "crash" && e["fatal"] != true).count();
    let offs: Vec<String> = events.iter().filter(today).filter(|e| e["ev"] == "off").filter_map(|e| e["part"].as_str()).map(part_name).collect();
    if restarts > 0 {
        let kinds: Vec<&str> = events.iter().filter(today).filter(|e| is_restart(e)).filter_map(|e| e["why"].as_str()).collect();
        let why = if kinds.iter().all(|k| *k == "watchdog") {
            "It stopped responding and was started again."
        } else if kinds.iter().all(|k| *k == "crash") {
            "It stopped after an error and was started again."
        } else {
            "It stopped and was started again."
        };
        return (format!("{} today", count(restarts, "restart", "restarts")), format!("{why} The events below say when and why."), false);
    }
    if fallbacks > 0 {
        return ("Switched to the basic shell today".into(), "Tessera stopped twice in a row, so the basic shell stood in until it was started again.".into(), false);
    }
    if let Some(first) = offs.first() {
        return (format!("{} was turned off today", cap(first)), "It stopped 3 times in 2 minutes. It comes back when you log in again.".into(), false);
    }
    if parts > 0 {
        return (format!("{} recovered today", count(parts, "part", "parts")), "A part of Tessera stopped after an error and was started again. Everything else kept working.".into(), false);
    }
    let day_ago = now - 86400;
    let long: u64 = samples.iter().filter(|s| s.t >= day_ago).map(|s| s.s33).sum();
    let worst = samples.iter().filter(|s| s.t >= day_ago).map(|s| s.worst).max().unwrap_or(0);
    if long >= 20 {
        return ("Some slow moments today".into(), format!("Tessera paused for over two frames {long} times in the last 24 hours (the longest {worst} ms)."), false);
    }
    let earlier = events.iter().filter(week).filter(|e| is_restart(e) || e["ev"] == "fallback" || e["ev"] == "off").count();
    let more = if earlier == 0 { "No crashes or restarts in the last 7 days.".to_string() } else { format!("No crashes or restarts today; {} in the last 7 days.", count(earlier, "problem", "problems")) };
    ("Running smoothly".into(), more, true)
}

/// What one event was, for people: (title, explanation, what to offer)
pub enum Offer {
    None,
    Report(String),
    OtherGpu,
}

pub fn explain(e: &Value) -> Option<(String, String, Offer)> {
    let report = || e["report"].as_str().filter(|r| !r.is_empty()).map(|r| Offer::Report(r.to_string())).unwrap_or(Offer::None);
    let part = e["part"].as_str().unwrap_or("");
    Some(match e["ev"].as_str()? {
        "start" => match e["why"].as_str()? {
            "crash" => ("Restarted after a crash".into(), "Tessera stopped after an error and was started again. The crash report has the details.".into(), report()),
            "stopped" => ("Restarted after it stopped".into(), "Tessera ended without saying why (the computer may have run out of memory) and was started again.".into(), Offer::None),
            "watchdog" => ("Restarted because it stopped responding".into(), "Tessera didn't answer for over a minute, so it was started again.".into(), Offer::None),
            "retry" => ("Back from the basic shell".into(), "Tessera started again after the basic shell had stood in for it.".into(), Offer::None),
            _ => return None,
        },
        "crash" if e["fatal"] != true => (format!("{} stopped and recovered", cap(&part_name(part))), "An error was caught and this part was started again. Everything else kept working.".into(), report()),
        "off" => (format!("{} was turned off", cap(&part_name(part))), "It stopped 3 times in 2 minutes, so it stays off until you log in again.".into(), Offer::None),
        "fallback" => {
            let why = e["why"].as_str().unwrap_or("").trim();
            ("Switched to the basic shell".into(), if why.is_empty() { "Tessera stopped twice in a row, so the basic shell stood in.".to_string() } else { format!("{why}. The basic shell stood in until Tessera was started again.") }, Offer::None)
        }
        "lock" => ("Screen locked again after a restart".into(), "Tessera ended while your screen was locked; it locked again as it started, so nothing was left open.".into(), Offer::None),
        "gpu-slow" => {
            let share = e["share"].as_f64().unwrap_or(0.0) * 100.0;
            let on = e["on"].as_str().unwrap_or("low-power");
            (format!("Animations were slow on the {on} graphics"), format!("{}% of animation frames were missed. The other GPU is faster but uses more power.", share.round()), Offer::OtherGpu)
        }
        "gpu-fallback" => {
            let (from, to) = (e["from"].as_str().unwrap_or("chosen"), e["to"].as_str().unwrap_or("desktop's"));
            (format!("Couldn't draw on {from}: back on {to}"), format!("Tessera couldn't start on the {from} GPU with this desktop, so Graphics processor was set to Same as the desktop."), Offer::None)
        }
        _ => return None,
    })
}

/// A scale's top for `max` (a round number above it)
pub fn nice_top(max: f64, floor: f64) -> f64 {
    let m = max.max(floor);
    let p = 10f64.powf(m.log10().floor());
    for k in [1.0, 2.0, 2.5, 5.0, 10.0] {
        if k * p >= m {
            return k * p;
        }
    }
    10.0 * p
}

/// A bar chart row (settings-system.slint's "chart"): one bar a slot
fn chart(title: &str, now: String, top: String, bars: Vec<Opt>, low: &str, summary: String, first: bool, last: bool) -> Row {
    let mut r = Row::custom("system", "chart").data(bars);
    r.label = title.into();
    r.value = now;
    r.text = top;
    r.low = low.into();
    r.high = "Now".into();
    r.detail = summary;
    r.from = if first { 1.0 } else { 0.0 };
    r.to = if last { 1.0 } else { 0.0 };
    r
}

/// A bar: its height (0..1), what hovering it says, and its style (0 the
/// accent, 1 a warning, 3 no record)
fn bar(frac: f64, says: String, style: i32) -> Opt {
    Opt::new("").num(frac.clamp(0.0, 1.0) as f32).detail(says).style(style)
}

pub fn page(cx: &mut Ctx, p: &mut Page) {
    p.title = String::new();
    let live = health::live();
    let v = cx.once("health.data", move || load(live));
    let footer = "Kept on this computer only, for up to a week (about 200 KB). Nothing is sent.";
    if v.is_null() {
        p.group("").row(Row::new("Reading Tessera's record…").spinner());
        return;
    }
    let now = v["now"].as_i64().unwrap_or(0);
    let samples = samples_of(&v);
    let events: Vec<Value> = v["events"].as_array().cloned().unwrap_or_default();
    if samples.is_empty() && events.is_empty() {
        let (label, detail) = if v["recording"] == true {
            ("No history yet", "Tessera notes its memory, processor time and slow moments every 5 minutes, and any crash or restart as it happens. Check back in a few minutes.")
        } else {
            ("Not recorded here", "Tessera keeps this record when it runs as the whole shell. Started beside the basic shell, it doesn't.")
        };
        p.group("").footer(footer).row(Row::new(label).detail(detail).icon("heartbeat"));
        return;
    }

    // ── How it's going ──
    let (label, detail, ok) = status(&events, &samples, now);
    let up = events.iter().rev().find(|e| e["ev"] == "start").and_then(|e| e["t"].as_i64());
    let detail = match up {
        Some(t) if v["recording"] == true => format!("{detail} Running since {}.", since(t, now)),
        _ => detail,
    };
    p.group("").row(Row::new(label).detail(detail).icon(if ok { "check-circle" } else { "warning-circle" }));

    // ── Memory and processor ──
    let week = cx.local_s("health.range") == "week";
    let (secs, n, low) = if week { (6 * 3600, 28usize, "7 days ago") } else { (3600, 24usize, "24 hours ago") };
    let from = (now / secs) * secs - (n as i64 - 1) * secs;
    let slots = health::slots(&samples, from, secs, n);
    let label_of = |t: i64| if week { tessera_core::util::strftime("%a %H:00", t) } else { tessera_core::util::strftime("%H:00", t) };
    let range = cx.on(|cx, a| cx.local_set("health.range", if a.i == 1 { "week" } else { "day" }));
    let mem_top = nice_top(slots.iter().map(|s| s.mem_max as f64).fold(0.0, f64::max), 1024.0);
    let mem_bars: Vec<Opt> = slots
        .iter()
        .map(|s| if s.n == 0 { bar(0.0, format!("{} · no record", label_of(s.start)), 3) } else { bar(s.mem_avg as f64 / mem_top, format!("{} · {}", label_of(s.start), mb(s.mem_avg)), 0) })
        .collect();
    let recorded: Vec<&health::Slot> = slots.iter().filter(|s| s.n > 0).collect();
    let mem_lo = recorded.iter().map(|s| s.mem_avg).filter(|m| *m > 0).min().unwrap_or(0);
    let mem_hi = recorded.iter().map(|s| s.mem_max).max().unwrap_or(0);
    let mem_now = v["mem"].as_u64().unwrap_or(0);
    let cpu_top = nice_top(slots.iter().map(|s| s.cpu_pct).fold(0.0, f64::max), 1.0);
    let cpu_bars: Vec<Opt> = slots
        .iter()
        .map(|s| if s.n == 0 { bar(0.0, format!("{} · no record", label_of(s.start)), 3) } else { bar(s.cpu_pct / cpu_top, format!("{} · {}", label_of(s.start), pct(s.cpu_pct)), 0) })
        .collect();
    let (cpu_ms, cpu_s): (u64, i64) = samples.iter().filter(|s| s.t >= from).fold((0, 0), |a, s| (a.0 + s.cpu_ms, a.1 + s.d));
    let cpu_avg = if cpu_s > 0 { cpu_ms as f64 / (cpu_s as f64 * 10.0) } else { 0.0 };
    let span = if week { "the last 7 days" } else { "the last 24 hours" };
    p.group("Memory and processor")
        .footer("Memory is Tessera's fair share of what it uses (shared parts split with other apps). Processor is the share of one core, averaged.")
        .row(Row::new("Show").segmented(&["24 hours", "7 days"], if week { 1 } else { 0 }, range))
        .row(chart("Memory", if mem_now > 0 { format!("{} now", mb(mem_now)) } else { String::new() }, mb(mem_top as u64), mem_bars, low, format!("Memory over {span}: {} to {}", mb(mem_lo), mb(mem_hi)), false, false))
        .row(chart("Processor", format!("{} average", pct(cpu_avg)), pct(cpu_top), cpu_bars, low, format!("Processor over {span}: {} on average", pct(cpu_avg)), false, true));

    // ── Slow moments ──
    let stall_top = nice_top(slots.iter().map(|s| s.stalls_per_hour).fold(0.0, f64::max), 5.0);
    let stall_bars: Vec<Opt> = slots
        .iter()
        .map(|s| {
            if s.n == 0 {
                bar(0.0, format!("{} · no record", label_of(s.start)), 3)
            } else {
                let says = if s.stalls == 0 { format!("{} · none", label_of(s.start)) } else { format!("{} · {}, the longest {} ms", label_of(s.start), count(s.stalls as usize, "slow moment", "slow moments"), s.worst) };
                bar(s.stalls_per_hour / stall_top, says, if s.long_stalls > 0 { 1 } else { 0 })
            }
        })
        .collect();
    let total: u64 = slots.iter().map(|s| s.stalls).sum();
    let worst = slots.iter().map(|s| s.worst).max().unwrap_or(0);
    let (frames, missed): (u64, u64) = samples.iter().filter(|s| s.t >= from).fold((0, 0), |a, s| (a.0 + s.frames, a.1 + s.missed));
    let g = p.group("Slow moments");
    g.footer("A slow moment is when Tessera was busy for longer than a frame (16 ms), which can show as a stutter; over 33 ms (in orange) you'd likely notice it.");
    g.row(chart(
        "Per hour",
        if total == 0 { "None".into() } else { format!("{}, the longest {worst} ms", count(total as usize, "in all", "in all")) },
        format!("{}", stall_top.round()),
        stall_bars,
        low,
        format!("Slow moments over {span}: {total}, the longest {worst} ms"),
        true,
        frames + missed == 0,
    ));
    if frames + missed > 0 {
        let share = missed as f64 / (frames + missed) as f64 * 100.0;
        g.row(Row::new("Animation frames missed").detail("Measured while the timing log or the graphics check was on").shows(pct(share)));
    }

    // ── Recent events ──
    let reports: Vec<String> = v["reports"].as_array().map(|a| a.iter().filter_map(|r| r.as_str().map(String::from)).collect()).unwrap_or_default();
    let mut shown: Vec<(String, String, Offer)> = events
        .iter()
        .rev()
        .filter(|e| e["t"].as_i64().is_some_and(|t| now - t < 7 * 86400))
        .filter_map(|e| explain(e).map(|(t, d, o)| (t, format!("{} · {d}", when(e["t"].as_i64().unwrap_or(0), now)), o)))
        .take(20)
        .collect();
    let g = p.group("Recent events");
    g.footer(footer);
    if shown.is_empty() {
        g.row(Row::new("No crashes or restarts in the last 7 days").tone(1));
    }
    let other_gpu = tessera_core::gpu::faster_family();
    for (title, detail, offer) in shown.drain(..) {
        let row = Row::new(title).detail(detail);
        let row = match offer {
            Offer::Report(name) => {
                let kept = reports.contains(&name);
                let n = name.clone();
                let act = cx.on(move |cx, _| match tessera_core::crash::save_one(&n) {
                    Ok(path) => cx.toast(format!("Saved to Downloads as {}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()), None),
                    Err(e) => cx.toast(format!("Couldn't save the report ({e}). Check that your Downloads folder exists and has space, then try again."), None),
                });
                row.buttons(vec![btn(if kept { "Save report" } else { "Report deleted" }).icon("download-simple").off(!kept)], act)
            }
            Offer::OtherGpu if other_gpu.is_some() => {
                let act = cx.on(|cx, _| {
                    if !tessera_core::gpu::use_faster() {
                        cx.toast("Couldn't switch. Choose the GPU in Battery & Power → Cool-down and performance.", None);
                    }
                });
                row.buttons(vec![btn("Use the other GPU").icon("arrow-clockwise")], act)
            }
            _ => row,
        };
        g.row(row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str) -> Vec<Value> {
        health::parse_lines(s)
    }

    #[test]
    fn the_top_line_says_how_it_is_going() {
        let now = tessera_core::util::now_ms() / 1000;
        let (l, d, ok) = status(&[], &[], now);
        assert!(ok && l == "Running smoothly" && d.contains("No crashes"), "{l} {d}");
        let two = ev(&format!("{{\"t\":{now},\"ev\":\"start\",\"why\":\"crash\"}}\n{{\"t\":{now},\"ev\":\"start\",\"why\":\"watchdog\"}}\n{{\"t\":{now},\"ev\":\"start\",\"why\":\"normal\"}}"));
        let (l, _, ok) = status(&two, &[], now);
        assert_eq!((l.as_str(), ok), ("2 restarts today", false));
        let part = ev(&format!("{{\"t\":{now},\"ev\":\"crash\",\"part\":\"dock\"}}"));
        assert_eq!(status(&part, &[], now).0, "1 part recovered today");
        let off = ev(&format!("{{\"t\":{now},\"ev\":\"off\",\"part\":\"dock\"}}"));
        assert_eq!(status(&off, &[], now).0, "The dock was turned off today");
        let slow = vec![Sample { t: now - 600, s33: 25, worst: 140, ..Default::default() }];
        assert_eq!(status(&[], &slow, now).0, "Some slow moments today");
        // (a crash last week, nothing today: smooth, and it says so)
        let old = ev(&format!("{{\"t\":{},\"ev\":\"start\",\"why\":\"crash\"}}", now - 3 * 86400));
        let (l, d, ok) = status(&old, &[], now);
        assert!(ok && l == "Running smoothly" && d.contains("1 problem in the last 7 days"), "{d}");
    }

    #[test]
    fn events_in_plain_words() {
        let e = ev("{\"t\":1,\"ev\":\"crash\",\"part\":\"dock\",\"report\":\"20261116-140233-dock.txt\"}\n{\"t\":2,\"ev\":\"start\",\"why\":\"normal\"}\n{\"t\":3,\"ev\":\"stop\"}\n{\"t\":4,\"ev\":\"gpu-slow\",\"share\":0.18,\"on\":\"Intel\",\"to\":\"NVIDIA\"}\n{\"t\":5,\"ev\":\"crash\",\"part\":\"shell\",\"fatal\":true}");
        let (t, _, o) = explain(&e[0]).unwrap();
        assert_eq!(t, "The dock stopped and recovered");
        assert!(matches!(o, Offer::Report(ref r) if r == "20261116-140233-dock.txt"));
        // (ordinary starts and stops, and the loop's own crash — its restart says it — aren't listed)
        assert!(explain(&e[1]).is_none() && explain(&e[2]).is_none() && explain(&e[4]).is_none());
        let (t, d, o) = explain(&e[3]).unwrap();
        assert_eq!(t, "Animations were slow on the Intel graphics");
        assert!(d.starts_with("18% of animation frames"));
        assert!(matches!(o, Offer::OtherGpu));
    }

    #[test]
    fn numbers_read_well() {
        assert_eq!(mb(145_000), "142 MB");
        assert_eq!(mb(3 * 1024 * 1024 / 2), "1.5 GB");
        assert_eq!(pct(0.84), "0.8%");
        assert_eq!(pct(12.4), "12%");
        assert_eq!(nice_top(0.0, 5.0), 5.0);
        assert_eq!(nice_top(7.0, 5.0), 10.0);
        assert_eq!(nice_top(130_000.0, 1024.0), 200_000.0);
        assert_eq!(nice_top(0.3, 1.0), 1.0);
        let now = tessera_core::util::now_ms() / 1000;
        assert!(when(now, now).starts_with("Today "));
    }
}
