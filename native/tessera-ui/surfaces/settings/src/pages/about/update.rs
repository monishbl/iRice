//! Software Update (Rust only since background updates; the frozen QML
//! shell keeps pages/update.qml). One status line with one button for what
//! matters now, then one group of two rows, Tessera and System, each with
//! its own Update; while one runs, its button gives way to a thin bar, one
//! line of what it's doing, Show details (its log, in a sheet that follows
//! the end) and Cancel. An error is one row under the status: what
//! happened, how to fix it, one action. Then two links: Options
//! (update/options: the channel, checking on its own, the AUR, unused
//! packages, snapshots, the build cache) and Go back to the previous
//! version (update/back, only when there's one). Hyprland's "keep back"
//! shows only when its update would break the glass plugin.
//!
//! Sheets take the page's place (a log, What's new, the packages, the
//! decisions before Tessera updates). The shell keeps which one is open
//! (`about.sheet`), so the island's Details opens one too.
//!
//! What it shows is the feed's `upd` (bridge_about.rs upd_feed); every
//! button asks the shell (`cx.ask("about", …)`).

use super::super::super::model::{btn, Ctx, Opt, Page, Row};
use super::{b, feed, n, plural, s, strings};
use serde_json::{json, Value};

/// The conflicts' sections in the decisions sheet, in order: (kind, title, footer)
const KINDS: [(&str, &str, &str); 5] = [
    ("setting", "Settings with new defaults", "You kept these at their defaults. Your value stays unless you choose the new one."),
    ("preset", "Your glass material", "Keeping your look saves it as a Custom material."),
    ("shortcut", "Shortcut clashes", "The update brings a shortcut on keys you already use."),
    ("removed", "Settings that change name or go away", "You changed these, and the update renames or no longer has them."),
    ("code", "Files you changed", "Keep mine drops the update's change to that file. Use the update's sets yours aside in the backup folder. Merge both is offered when the two fit together."),
];

fn upd<'a>(cx: &Ctx<'a>) -> &'a Value {
    &feed(cx)["upd"]
}

/// A button's act, run: a sheet ("sheet:notes"), a package to install
/// ("install:grub-btrfs"), else the shell's act by name ("tu.update")
fn run(cx: &mut Ctx, act: &str) {
    if let Some(k) = act.strip_prefix("sheet:") {
        cx.ask("about", "sheet", json!([k]));
    } else if let Some(pk) = act.strip_prefix("install:") {
        cx.ask("about", "needs.install", json!([pk]));
    } else {
        cx.ask("about", act, Value::Null);
    }
}

/// A version as the rows say it: the commit (8 characters), not the whole build line
pub fn short(v: &str) -> String {
    let first = v.split_whitespace().next().unwrap_or("");
    let tail = first.rsplit('+').next().unwrap_or(first);
    tail.chars().take(8).collect()
}

// ── The status: one line, one button ────────────────────────────────────

#[derive(Debug, PartialEq)]
pub struct Status {
    pub title: String,
    pub detail: String,
    pub icon: &'static str,
    /// (label, act)
    pub button: Option<(String, String)>,
    pub busy: bool,
    pub enabled: bool,
}

fn st(title: impl Into<String>, detail: impl Into<String>, icon: &'static str, button: Option<(&str, &str)>) -> Status {
    Status { title: title.into(), detail: detail.into(), icon, button: button.map(|(l, a)| (l.to_string(), a.to_string())), busy: false, enabled: true }
}

/// Tessera's update needs decisions first (the decisions sheet), and could go now
fn needs_decisions(u: &Value) -> bool {
    let t = &u["t"];
    b(&t["available"]) && n(&t["undecided"]) > 0 && !b(&u["running"]) && !b(&t["ready"]) && b(&u["allowed"])
}

/// What Install updates covers, in a few words
pub fn summary(u: &Value) -> String {
    let (t, sy) = (&u["t"], &u["s"]);
    let mut parts = Vec::new();
    if b(&t["available"]) {
        parts.push(if b(&t["downgrade"]) {
            format!("Tessera back to {}'s {}", s(&t["channelName"]), short(&s(&t["latest"])))
        } else {
            format!("Tessera {} → {}", short(&s(&t["current"])), short(&s(&t["latest"])))
        });
    }
    let count = n(&sy["count"]) + if b(&sy["includeAur"]) { n(&sy["aurCount"]) } else { 0 };
    if count > 0 {
        parts.push(plural(count, "system package"));
    }
    parts.join(" · ")
}

/// The status: the most important thing now, and its one button
pub fn status(u: &Value) -> Status {
    let (t, sy, snap) = (&u["t"], &u["s"], &u["snap"]);
    let checked = b(&u["checked"]);
    let at = s(&u["checkedAt"]);
    let last = if at.is_empty() { String::new() } else { format!("Last checked at {at}.") };
    let booted = s(&snap["booted"]);
    if !booted.is_empty() {
        let assistant = b(&snap["assistant"]);
        let mut x = st(
            format!("Started from snapshot {booted}"),
            "Your system is as it was then, until you restart. To keep it, restore this snapshot in Btrfs Assistant (Snapper, then Restore).",
            "clock-counter-clockwise",
            Some(if assistant { ("Open Btrfs Assistant", "bs.openAssistant") } else { ("Install Btrfs Assistant", "install:btrfs-assistant") }),
        );
        x.busy = !assistant && b(&u["installing"]);
        x.enabled = assistant || !b(&sy["running"]);
        return x;
    }
    if b(&t["restarting"]) {
        let mut x = st("Restarting Tessera", "A few seconds. Your windows stay open.", "arrow-clockwise", None);
        x.busy = true;
        return x;
    }
    if b(&t["ready"]) {
        let v = short(&s(&t["staged"]));
        return st(
            if v.is_empty() { "A new Tessera is ready".to_string() } else { format!("Tessera {v} is ready") },
            "Restart the shell now, or it restarts the next time you lock the screen or log in. Your windows stay open.",
            "arrow-clockwise",
            Some(("Restart shell", "tu.restart")),
        );
    }
    if b(&u["restartComputer"]) && !b(&sy["running"]) {
        return st("Restart to finish the system update", "The kernel, drivers or system libraries changed. Save your work first.", "arrow-clockwise", Some(("Restart computer", "reboot")));
    }
    if b(&sy["running"]) {
        return st("Updating system packages", "You can keep working. If the kernel or a driver changes, you restart when you choose.", "arrow-circle-up", None);
    }
    if s(&t["job"]["state"]) == "running" {
        return st("Updating Tessera", "You can keep working. The shell restarts only when you choose.", "arrow-circle-up", None);
    }
    if b(&u["checking"]) {
        let mut x = st("Checking for updates…", "Tessera, then your system's packages.", "arrow-circle-up", Some(("Check now", "su.check")));
        x.busy = true;
        return x;
    }
    if b(&t["offline"]) {
        return st("Can't reach the update server", "Check your internet connection, then try again.", "warning-circle", Some(("Try again", "su.check")));
    }
    if checked && b(&u["anything"]) {
        let decide = needs_decisions(u);
        let mut x = st("Updates are ready", summary(u), "arrow-circle-up", Some(("Install updates", if decide { "sheet:decide" } else { "su.updateAll" })));
        // (not while an update runs elsewhere: the error row under says so)
        x.enabled = s(&t["job"]["state"]) != "busy" && (decide || b(&t["canUpdate"]) || b(&sy["canUpdate"]));
        return x;
    }
    let preparing = n(&t["preparing"]);
    if checked && preparing > 0 {
        return st(
            "Preparing a new version of Tessera",
            format!("Offered in about {preparing} min, ready to install in seconds. {last}").trim_end().to_string(),
            "arrow-circle-up",
            Some(("Check now", "su.check")),
        );
    }
    if checked {
        return st("Everything is up to date", last, "check-circle", Some(("Check now", "su.check")));
    }
    st("Check for updates", "Tessera and your system's packages. Checking downloads only the package lists.", "arrow-circle-up", Some(("Check now", "su.check")))
}

// ── An error: one row, what happened, how to fix it, one action ─────────

#[derive(Debug, PartialEq)]
pub struct Problem {
    pub title: String,
    pub detail: String,
    pub action: (String, String),
}

fn problem_of(title: impl Into<String>, detail: impl Into<String>, label: &str, act: &str) -> Option<Problem> {
    Some(Problem { title: title.into(), detail: detail.into(), action: (label.into(), act.into()) })
}

/// What the update running elsewhere is doing (tessera-update.py's WHAT_WORDS)
fn holder_what(what: &str) -> &'static str {
    match what {
        "stage" | "resume" => "getting an update ready",
        "finish" => "installing an update",
        "restart" => "restarting Tessera into an update",
        "apply" | "update" => "updating Tessera",
        "recover" => "putting back an update that was cut off",
        _ => "",
    }
}

/// The error row, if something went wrong (the first that matters)
pub fn problem(u: &Value) -> Option<Problem> {
    let (t, sy) = (&u["t"], &u["s"]);
    let job = &t["job"];
    let sj = &sy["job"];
    if s(&job["state"]) == "busy" {
        // (never "another update" and a dead end: since when, where, what, and a way to see it)
        let h = &job["holder"];
        let at = s(&h["at"]);
        let place = match s(&h["where"]).as_str() {
            "this" => " in this session",
            "other" => " in another session",
            _ => "",
        };
        let what = holder_what(&s(&h["what"]));
        let detail = format!(
            "Started{}{place}{}. Wait for it to finish, then check again.",
            if at.is_empty() { String::new() } else { format!(" at {at}") },
            if what.is_empty() { String::new() } else { format!(", {what}") }
        );
        return if s(&h["log"]).is_empty() {
            problem_of("Software Update is already running", detail, "Check again", "su.check")
        } else {
            problem_of("Software Update is already running", detail, "Show details", "sheet:busy-log")
        };
    }
    if s(&job["state"]) == "error" && !b(&u["running"]) {
        let e = s(&job["error"]);
        let e = if e.is_empty() { "Something stopped it.".to_string() } else { e };
        return problem_of("Tessera's update didn't finish", format!("{} Nothing was changed. Try again; if it stops again, check your connection.", e.trim_end()), "Try again", "tu.update");
    }
    if b(&sy["interrupted"]) {
        return problem_of("The system update was cut off", "Update the packages again to finish: pacman picks up where it stopped.", "Update packages", "sys.update");
    }
    if s(&sj["state"]) == "error" && !b(&sy["running"]) {
        let title = s(&sj["text"]);
        let e = s(&sj["error"]);
        return problem_of(
            if title.is_empty() { "The system update didn't finish".to_string() } else { title },
            if e.is_empty() { "The details say where it stopped.".to_string() } else { e },
            "Show details",
            "sheet:system-log",
        );
    }
    let err = s(&t["error"]);
    if !err.is_empty() && !b(&t["offline"]) {
        return problem_of("Can't check for Tessera's updates", err, "Try again", "su.check");
    }
    if !b(&sy["canCheck"]) && b(&u["checked"]) {
        return problem_of("Can't check the system's packages", "checkupdates isn't installed. Install the pacman-contrib package, then check again.", "Check again", "su.check");
    }
    None
}

// ── The two rows ────────────────────────────────────────────────────────

/// A button on a part's row: (label, act, style (0 normal, 4 a text link), enabled)
pub type Btn = (String, String, i32, bool);

fn bt(label: &str, act: &str, style: i32, on: bool) -> Btn {
    (label.into(), act.into(), style, on)
}

/// A part's row: its line, its buttons, and while it runs its progress
/// (how far 0…1, -1 not known; the percent; what it's doing)
#[derive(Debug, Default, PartialEq)]
pub struct Part {
    pub detail: String,
    pub buttons: Vec<Btn>,
    pub progress: Option<(f32, String, String)>,
}

/// What Tessera's background job is doing, in one line
pub fn job_line(job: &Value) -> String {
    let mb = job["bytes"].as_f64().unwrap_or(0.0) / 1e6;
    match job["phase"].as_str().unwrap_or("") {
        "download" if job["how"] == "patch" => format!("Downloading a patch · {mb:.1} MB"),
        "download" => format!("Downloading the update · {mb:.1} MB"),
        "build" => "Building Tessera".into(),
        "parts" => {
            let t = s(&job["text"]);
            if t.is_empty() { "Building Tessera's parts".into() } else { t }
        }
        _ => {
            let t = s(&job["text"]);
            if t.is_empty() || t == "Preparing" { "Preparing the update".into() } else { t }
        }
    }
}

/// A build's bar text (the glass plugin's page): its percent, else the time so far
pub fn build_progress_text(fraction: f64, time: &str) -> String {
    if fraction >= 0.0 {
        format!("{}%", (fraction * 100.0).round())
    } else {
        time.split(" · ").next().unwrap_or("").to_string()
    }
}

fn percent(f: f64) -> String {
    if f >= 0.0 { format!("{}%", (f * 100.0).round()) } else { String::new() }
}

/// Tessera's row
pub fn tessera_part(u: &Value) -> Part {
    let t = &u["t"];
    let job = &t["job"];
    let state = s(&job["state"]);
    let cur = short(&if s(&t["current"]).is_empty() { s(&u["info"]["current"]) } else { s(&t["current"]) });
    let notes = b(&t["hasNotes"]);
    if state == "running" {
        let f = t["fraction"].as_f64().unwrap_or(-1.0);
        return Part {
            detail: String::new(),
            buttons: vec![bt("Show details", "sheet:tessera-log", 4, true), bt("Cancel", "tu.cancel", 0, true)],
            progress: Some((if f >= 0.0 { f.max(0.02) as f32 } else { -1.0 }, percent(f), job_line(job))),
        };
    }
    if b(&t["restarting"]) {
        return Part { detail: "Restarting…".into(), ..Default::default() };
    }
    if b(&t["ready"]) {
        let mut buttons = Vec::new();
        if notes {
            buttons.push(bt("What's new", "sheet:notes", 4, true));
        }
        buttons.push(bt("Discard", "tu.cancel", 0, true));
        return Part { detail: format!("{} is ready to restart", short(&s(&t["staged"]))), buttons, progress: None };
    }
    let result = &t["result"];
    if !result.is_null() && !b(&t["available"]) {
        let has = result["changelog"].as_array().is_some_and(|a| !a.is_empty()) || result["notes"].as_array().is_some_and(|a| !a.is_empty());
        return Part { detail: format!("Updated to {}", short(&s(&result["now"]))), buttons: if has { vec![bt("What's new", "sheet:result", 4, true)] } else { vec![] }, progress: None };
    }
    if !b(&u["allowed"]) {
        return Part { detail: format!("{}{}Off in the dev sandbox", cur, if cur.is_empty() { "" } else { " · " }), ..Default::default() };
    }
    if b(&t["available"]) {
        let mut detail = if b(&t["downgrade"]) {
            format!("{cur} → {} · back to {}", short(&s(&t["latest"])), s(&t["channelName"]))
        } else {
            format!("{cur} → {}", short(&s(&t["latest"])))
        };
        if state == "cancelled" {
            detail.push_str(" · Cancelled, nothing was changed");
        }
        let mut buttons = Vec::new();
        if notes {
            buttons.push(bt("What's new", "sheet:notes", 4, true));
        }
        let decide = needs_decisions(u);
        buttons.push(bt("Update", if decide { "sheet:decide" } else { "tu.update" }, 0, decide || b(&t["canUpdate"])));
        return Part { detail, buttons, progress: None };
    }
    let preparing = n(&t["preparing"]);
    let detail = if b(&u["checking"]) {
        "Checking…".to_string()
    } else if preparing > 0 {
        format!("{cur} · a new version is being prepared, about {preparing} min")
    } else if b(&u["checked"]) && s(&t["error"]).is_empty() {
        format!("{cur} · Up to date")
    } else {
        cur
    };
    Part { detail, ..Default::default() }
}

/// The system's row
pub fn system_part(u: &Value) -> Part {
    let sy = &u["s"];
    let job = &sy["job"];
    if b(&sy["running"]) {
        let p = sy["progress"].as_f64().unwrap_or(-1.0);
        let auth = s(&job["phase"]) == "auth";
        let text = s(&job["text"]);
        let pkg = s(&job["package"]);
        let line = if auth {
            "Enter your password in the dialog".to_string()
        } else if pkg.is_empty() {
            if text.is_empty() { "Updating".into() } else { text }
        } else {
            format!("{} · {pkg}", if text.is_empty() { "Updating".to_string() } else { text })
        };
        let mut buttons = vec![bt("Show details", "sheet:system-log", 4, true)];
        if b(&sy["cancellable"]) {
            buttons.push(bt("Cancel", "su.cancel", 0, true));
        }
        return Part { detail: String::new(), buttons, progress: Some((if auth { -1.0 } else { p.max(0.02) as f32 }, if auth { String::new() } else { percent(p) }, line)) };
    }
    if b(&u["checking"]) {
        return Part { detail: "Checking…".into(), ..Default::default() };
    }
    if !b(&u["checked"]) {
        return Part { detail: "Not checked yet".into(), ..Default::default() };
    }
    let count = n(&sy["count"]);
    let aur = n(&sy["aurCount"]);
    let with_aur = b(&sy["includeAur"]) && aur > 0;
    if count > 0 || with_aur || b(&sy["canUpdate"]) {
        let mut d = if count > 0 || with_aur { plural(count + if with_aur { aur } else { 0 }, "package") } else { "Simulated in the dev sandbox".to_string() };
        if with_aur {
            d.push_str(&format!(" · {aur} from the AUR"));
        } else if n(&sy["aurAll"]) > 0 {
            d.push_str(" · AUR updates not included");
        }
        let hold = b(&sy["hyprTooNew"]) && sy["hold"].as_i64() == Some(-1);
        if hold {
            d.push_str(" · choose what to do about Hyprland first");
        }
        return Part { detail: d, buttons: vec![bt("See packages", "sheet:packages", 4, true), bt("Update", "sys.update", 0, b(&sy["canUpdate"]))], progress: None };
    }
    if s(&job["state"]) == "done" {
        let k = n(&job["upgraded"]);
        return Part {
            detail: if s(&job["kind"]) == "undo" { "Back to the earlier versions".into() } else if k > 0 { format!("{} updated", plural(k, "package")) } else { "Up to date".into() },
            ..Default::default()
        };
    }
    Part { detail: "Up to date".into(), ..Default::default() }
}

/// A part as its row: the usual row with its buttons, or while it runs
/// the part view (settings-about.slint `part`: bar, line, buttons)
fn part_row(cx: &mut Ctx, label: &str, part: Part) -> Row {
    let acts: Vec<String> = part.buttons.iter().map(|x| x.1.clone()).collect();
    let id = cx.on(move |cx, a| {
        if let Some(w) = acts.get(a.i.max(0) as usize) {
            run(cx, w);
        }
    });
    let opts: Vec<Opt> = part.buttons.iter().map(|(l, _, style, on)| btn(l.as_str()).style(*style).off(!on)).collect();
    match part.progress {
        Some((f, pct, line)) => {
            let mut r = Row::custom("about", "part").id(id).num(f).text_value(line).value(pct);
            r.label = label.to_string();
            r.opts = opts;
            r
        }
        None if opts.is_empty() => Row::new(label).detail(part.detail),
        None => Row::new(label).detail(part.detail).buttons(opts, id),
    }
}

// ── The page ────────────────────────────────────────────────────────────

pub fn page(cx: &mut Ctx, p: &mut Page) {
    let u = upd(cx);
    let kind = s(&u["sheet"]);
    if !kind.is_empty() {
        sheet(cx, p, &kind);
        return;
    }

    // Status, and an error under it
    let mut u2 = u.clone();
    u2["installing"] = feed(cx)["needsInstalling"].clone();
    let x = status(&u2);
    let mut row = Row::new(x.title).icon(x.icon).detail(x.detail);
    if let Some((label, act)) = x.button {
        let id = cx.on(move |cx, _| run(cx, &act));
        row = row.button(label, id).primary().busy(x.busy).enabled(x.enabled && !x.busy);
    }
    let g = p.group("");
    if !b(&u["allowed"]) {
        g.footer("In the dev sandbox, Tessera's part is off and the system's is simulated: nothing is installed.");
    }
    g.row(row);
    if let Some(e) = problem(u) {
        let (label, act) = e.action;
        let id = cx.on(move |cx, _| run(cx, &act));
        g.row(Row::new(e.title).icon("warning-circle").tone(2).detail(e.detail).button(label, id));
    }

    // Tessera and System, each with its own Update
    let t = part_row(cx, "Tessera", tessera_part(u));
    let sys = part_row(cx, "System", system_part(u));
    p.group("").row(t).row(sys);

    // Hyprland, only when its update would break the glass plugin
    let sy = &u["s"];
    if b(&sy["hyprTooNew"]) && !b(&sy["running"]) {
        let hold = n(&sy["hold"]);
        let now = s(&sy["hyprNow"]);
        let keep = if now.is_empty() { "Keep this one".to_string() } else { format!("Keep {now}") };
        let pick = cx.on(|cx, a| cx.ask("about", "su.hold", json!([a.i])));
        p.group("")
            .footer(match hold {
                1 => "Hyprland stays as it is for this update; you're asked again next time.".to_string(),
                0 => "If the glass plugin can't be built for it, glass falls back to frosted until Tessera catches up.".to_string(),
                _ => format!("This Tessera is tested with Hyprland up to {}.", s(&sy["hyprMax"])),
            })
            .row(
                Row::new(format!("Hyprland {} may break the glass", s(&sy["hyprShort"])))
                    .detail("Choose before updating the system.")
                    .segmented(&[keep.as_str(), "Update anyway"], match hold { 1 => 0, 0 => 1, _ => -1 }, pick)
                    .width(280.0),
            );
    }

    // Options, and going back when there's something to go back to
    let back = &u["back"];
    let can_back = n(&back["count"]) > 0 || back["skipped"].as_array().is_some_and(|a| !a.is_empty());
    p.group("")
        .row(Row::link("Options", "update/options").icon("gear-six"))
        .row(Row::link("Go back to the previous version", "update/back").icon("clock-counter-clockwise").value(s(&back["day"])).visible(can_back && s(&u["snap"]["booted"]).is_empty()));
}

// ── Options (update/options) ───────────────────────────────────────────

/// The channel picker's line: the channel and the version this copy has
pub fn channel_line(picked: &str, info: &Value) -> String {
    if info.is_null() {
        return String::new();
    }
    if picked == "custom" {
        let branch = s(&info["branch"]);
        return format!("This copy follows {}, a development branch. Choose a channel to follow it instead.", if branch.is_empty() { "its own branch".to_string() } else { branch });
    }
    let date = s(&info["date"]);
    // ("On Stable: version X" read as Stable's own version: this copy's, said as such)
    format!("Following {}. This copy is version {}{}", s(&info["channelName"]), s(&info["current"]), if date.is_empty() { String::new() } else { format!(", from {date}") })
}

/// The build cache's size, when there is one
pub fn cache_words(bytes: Option<u64>) -> Option<String> {
    match bytes {
        Some(b) if b > 0 => Some(format!("{:.1} GB", b as f64 / 1e9)),
        _ => None,
    }
}

pub fn options(cx: &mut Ctx, p: &mut Page) {
    p.title = "Options".into();
    let u = upd(cx);
    let busy = b(&u["running"]) || b(&u["checking"]);

    // Which Tessera to follow, and checking on its own
    let info = &u["info"];
    let chosen = cx.s("update.channel");
    let picked = if chosen.is_empty() { s(&info["channel"]) } else { chosen };
    let pick = cx.on(|cx, a| {
        if let Some(ch) = ["stable", "beta"].get(a.i.max(0) as usize) {
            cx.set("update.channel", *ch);
            cx.ask("about", "su.channel", json!([ch]));
        }
    });
    let auto = cx.toggle("update.autoCheck");
    let t = &u["t"];
    let report = if s(&t["latest"]).is_empty() && !b(&t["available"]) { Value::Null } else { json!({"channelName": t["channelName"], "latest": t["latest"], "ui": ""}) };
    let latest = super::version::latest(b(&u["checked"]) && !b(&u["checking"]), b(&t["available"]), &report, info);
    let this_build = super::version::row(cx, "This build", &latest);
    p.group("Tessera")
        .footer("Stable changes only when a tested version is promoted. Beta gets every change first, so expect rough edges. Nothing installs until you choose Update.")
        .row(this_build)
        .row(Row::new("Follow")
            .detail(channel_line(&picked, info))
            .segmented(&["Stable", "Beta"], ["stable", "beta"].iter().position(|c| *c == picked).map_or(-1, |i| i as i32), pick)
            .enabled(!busy && b(&u["allowed"]))
            .width(200.0))
        .row(Row::new("Check automatically")
            .detail("Looks for a new Tessera every few hours and tells you. Nothing installs on its own.")
            .switch(cx.b("update.autoCheck"), auto));

    // The system's packages
    let sy = &u["s"];
    let include_aur = b(&sy["includeAur"]);
    let remove = b(&sy["removeDropped"]);
    let aur_t = cx.on(move |cx, _| cx.ask("about", "su.includeAur", json!([!include_aur])));
    let drop_t = cx.on(move |cx, _| cx.ask("about", "su.removeDropped", json!([!remove])));
    p.group("System packages")
        .row(Row::new("Include AUR updates")
            .visible(b(&u["aurHelper"]) || n(&sy["aurAll"]) > 0)
            .detail("AUR packages are built from scripts anyone can submit, without showing you what changed. Include them only if you trust them.")
            .switch(include_aur, aur_t)
            .enabled(!busy))
        .row(Row::new("Remove packages Tessera no longer uses").detail("Turn this off if you use them yourself.").switch(remove, drop_t).enabled(!busy));

    // Snapshots: one before each update, and the boot menu's
    let snap = &u["snap"];
    let tool = s(&snap["tool"]);
    let installing = b(&feed(cx)["needsInstalling"]);
    let g = p.group("Snapshots");
    g.footer("If the desktop doesn't start after an update, Tessera offers to go back on its own. From a text console (Ctrl+Alt+F3), run: tessera undo-update");
    g.row(Row::new("Snapshot before each update").detail(match tool.as_str() {
        "snapper" => "Snapper takes one before the system's packages change.".to_string(),
        "timeshift" => "Timeshift takes one before the system's packages change.".to_string(),
        _ => "No snapshot tool is set up (Snapper, or Timeshift on Btrfs). Updates can still go back to the earlier package versions.".to_string(),
    }).shows(match tool.as_str() { "snapper" => "Snapper", "timeshift" => "Timeshift", _ => "Off" }));
    if b(&snap["canMenu"]) {
        let grub = b(&snap["grubBtrfs"]);
        let menu_on = b(&snap["menuOn"]);
        let err = s(&snap["error"]);
        let detail = if !err.is_empty() {
            err.clone()
        } else if !grub {
            "To start from a snapshot when the desktop won't load, GRUB can list them (grub-btrfs). It asks for your password once.".into()
        } else {
            "GRUB lists your snapshots under “snapshots”: start from one to see your system as it was.".into()
        };
        let r = Row::new("Snapshots in the boot menu").detail(detail).tone(if err.is_empty() { 0 } else { 2 });
        if grub {
            let tg = cx.on(move |cx, _| cx.ask("about", "bs.setMenu", json!([!menu_on])));
            g.row(r.switch(menu_on, tg).enabled(!b(&snap["busy"])));
        } else {
            let i = cx.on(|cx, _| cx.ask("about", "needs.install", json!(["grub-btrfs"])));
            g.row(r.button(if installing { "Installing…" } else { "Install grub-btrfs" }, i).busy(installing).enabled(!b(&sy["running"])));
        }
    }

    // The build cache (only where Tessera is built on this computer)
    if let Some(size) = cache_words(u["cache"].as_u64()) {
        let err = s(&u["cacheError"]);
        let deleting = b(&u["cacheBusy"]);
        let del = cx.on(|cx, _| cx.ask("about", "cache.delete", Value::Null));
        p.group("Build cache")
            .footer("Kept so updates built on this computer take minutes. Deleting it frees the space; the next build takes longer.")
            .row(Row::new(format!("Build cache · {size}"))
                .detail(if err.is_empty() { "Tessera's compiled parts, kept between updates (at most about 4.5 GB).".to_string() } else { err.clone() })
                .tone(if err.is_empty() { 0 } else { 2 })
                .button(if deleting { "Deleting…" } else { "Delete cache" }, del)
                .danger()
                .busy(deleting)
                .enabled(!deleting && !b(&u["running"])));
    }
}

// ── Go back (update/back) ──────────────────────────────────────────────

pub fn back(cx: &mut Ctx, p: &mut Page) {
    p.title = "Go back".into();
    let u = upd(cx);
    let bk = &u["back"];
    let count = n(&bk["count"]);
    if count > 0 {
        let missing = strings(&bk["missing"]);
        let pk = strings(&bk["packages"]);
        let detail = if !missing.is_empty() {
            format!("Some earlier versions aren't on this computer any more ({}{}), so it can't go back.", missing.iter().take(4).cloned().collect::<Vec<_>>().join(", "), if missing.len() > 4 { "…" } else { "" })
        } else {
            format!(
                "The update on {} changed {}: {}{}. They go back to the versions before it; your files and settings stay. It asks for your password.",
                s(&bk["long"]),
                plural(count, "package"),
                pk.iter().take(3).cloned().collect::<Vec<_>>().join(", "),
                if count > 3 { "…" } else { "" }
            )
        };
        let go = cx.on(|cx, _| cx.ask("about", "su.goBack", Value::Null));
        p.group("System packages").row(
            Row::new(format!("Go back to before {}", s(&bk["day"])))
                .detail(detail)
                .tone(if missing.is_empty() { 0 } else { 1 })
                .button("Go back", go)
                .enabled(b(&bk["canGoBack"])),
        );
    }
    let include = strings(&bk["include"]);
    let skipped = bk["skipped"].as_array().cloned().unwrap_or_default();
    if !skipped.is_empty() {
        let g = p.group("Versions you went back from");
        g.footer("Skipped until you include them. A newer version installs as usual.");
        for x in &skipped {
            let name = s(&x["name"]);
            let on = include.contains(&name);
            let nm = name.clone();
            let tg = cx.on(move |cx, _| cx.ask("about", "su.includeSkip", json!([nm, !on])));
            g.row(Row::new(format!("{} {}", name, s(&x["to"]))).detail(if on { "Included in the next update" } else { "Skipped" }).switch(on, tg));
        }
    }
    if count == 0 && skipped.is_empty() {
        p.group("").row(Row::text("There's no update to go back from."));
    }
}

// ── Sheets ──────────────────────────────────────────────────────────────

/// The sheet's head: title, a line, its buttons (the last one closes it: Esc)
fn head(cx: &mut Ctx, p: &mut Page, title: &str, line: &str, buttons: Vec<(String, String, i32, bool)>) {
    let acts: Vec<String> = buttons.iter().map(|x| x.1.clone()).collect();
    let id = cx.on(move |cx, a| {
        if let Some(w) = acts.get(a.i.max(0) as usize) {
            match w.as_str() {
                "close" => cx.ask("about", "sheet", json!([""])),
                "copy" => {
                    cx.ask("about", "sheet.copy", Value::Null);
                    cx.toast("Log copied", None);
                }
                w => run(cx, w),
            }
        }
    });
    let data: Vec<Opt> = buttons.iter().map(|(l, _, style, on)| Opt::new(l.as_str()).style(*style).off(!on)).collect();
    let blk = p.custom("about", "sheet");
    blk.title(title).text(line).data(data).id(id);
}

fn done() -> (String, String, i32, bool) {
    ("Done".into(), "close".into(), 1, true)
}

fn sheet(cx: &mut Ctx, p: &mut Page, kind: &str) {
    let u = upd(cx);
    let (t, sy) = (&u["t"], &u["s"]);
    match kind {
        "tessera-log" | "system-log" | "busy-log" => {
            let (title, line) = match kind {
                "tessera-log" => ("Updating Tessera", if s(&t["job"]["state"]) == "running" { job_line(&t["job"]) } else { s(&t["job"]["text"]) }),
                "system-log" => ("Updating system packages", if b(&sy["running"]) { s(&sy["job"]["text"]) } else { s(&sy["job"]["error"]) }),
                _ => ("Software Update is already running", problem(u).map(|e| e.detail).unwrap_or_default()),
            };
            let log = s(&u["log"]);
            head(cx, p, title, &line, vec![("Copy log".into(), "copy".into(), 0, !log.is_empty()), done()]);
            let mut r = Row::custom("about", "log").text_value(if log.is_empty() { "Waiting for output…".to_string() } else { log }).num(440.0);
            r.on = true; // (follows the end, unless you scroll up to read)
            p.group("").row(r);
        }
        "notes" => {
            let from_to = if b(&t["downgrade"]) {
                format!("Back to {}'s {}", s(&t["channelName"]), short(&s(&t["latest"])))
            } else {
                format!("{} → {}", short(&s(&t["current"])), short(&s(&t["latest"])))
            };
            head(cx, p, "What's new in Tessera", &from_to, vec![done()]);
            let r = notes(cx, &t["changelog"], &t["notes"]);
            p.group("").row(r);
            let leaving = strings(&t["leaving"]);
            if b(&t["downgrade"]) && !leaving.is_empty() {
                let r = notes(cx, &json!([]), &t["leaving"]);
                p.group(format!("Going back to {} drops {}", s(&t["channelName"]), plural(n(&t["ahead"]), "change")))
                    .footer("Your settings and shortcuts are backed up first. Switch back to Beta to get these changes again.")
                    .row(r);
            }
        }
        "result" => {
            let r = &t["result"];
            let backup = s(&r["backup"]);
            let id = ("Done".to_string(), "tu.dismissResult".to_string(), 1, true);
            head(cx, p, "What's new", &format!("Tessera is now {}", short(&s(&r["now"]))), vec![id]);
            let nr = notes(cx, &r["changelog"], &r["notes"]);
            let g = p.group("");
            if !backup.is_empty() {
                g.footer(format!("Your settings and shortcuts were backed up to {backup}."));
            }
            g.row(nr);
        }
        "packages" => packages_sheet(cx, p, u),
        "decide" => decide_sheet(cx, p, u),
        _ => {
            head(cx, p, "Software Update", "", vec![done()]);
        }
    }
}

/// One package line (name, a badge, versions or a reason)
fn pkg(name: &str, from: &str, to: &str, badge: &str, dim: bool) -> Opt {
    let right = if from.is_empty() { to.to_string() } else { format!("{from}  →  {to}") };
    // (versions in mono, they line up; a reason in the normal font)
    let mono = !from.is_empty() || (to.chars().any(|c| c.is_ascii_digit()) && !to.contains(' '));
    Opt::new(name).detail(right).id(badge).off(dim).on(mono).style(1)
}

fn heading(text: String) -> Opt {
    Opt::new(text).style(0)
}

fn packages_sheet(cx: &mut Ctx, p: &mut Page, u: &Value) {
    let sy = &u["s"];
    let held = strings(&sy["held"]);
    let held_hypr = strings(&sy["heldHypr"]);
    let include_aur = b(&sy["includeAur"]);
    let mut lines = Vec::new();
    let list = sy["list"].as_array().cloned().unwrap_or_default();
    let aur = sy["aurList"].as_array().cloned().unwrap_or_default();
    if !list.is_empty() || !aur.is_empty() {
        lines.push(heading(format!("System · {}", plural((list.len() + aur.len()) as i64, "update"))));
        for (x, is_aur) in list.iter().map(|x| (x, false)).chain(aur.iter().map(|x| (x, true))) {
            let name = s(&x["name"]);
            let is_held = held.contains(&name);
            let badge = if is_held {
                if held_hypr.contains(&name) { "kept back" } else { "skipped" }
            } else if is_aur {
                if include_aur { "AUR" } else { "AUR · not included" }
            } else {
                ""
            };
            lines.push(pkg(&name, &s(&x["from"]), &s(&x["to"]), badge, is_held || (is_aur && !include_aur)));
        }
    }
    let missing = strings(&sy["missingList"]);
    if !missing.is_empty() {
        lines.push(heading("Tessera adds".into()));
        for m in &missing {
            lines.push(pkg(m, "", &s(&sy["reasons"][m]), "", false));
        }
    }
    let dropped = strings(&sy["droppedList"]);
    if !dropped.is_empty() {
        let rm = b(&sy["removeDropped"]);
        lines.push(heading("Tessera no longer uses".into()));
        for d in &dropped {
            lines.push(pkg(d, "", "", if rm { "removed" } else { "kept" }, !rm));
        }
    }
    let count = n(&sy["count"]) + if include_aur { n(&sy["aurCount"]) } else { 0 };
    head(cx, p, "System updates", &plural(count, "package"), vec![done()]);
    p.group("").row(Row::custom("about", "pkgs").data(lines));
}

/// A conflict's button label
pub fn choice_label(c: &Value, ch: &str) -> String {
    if c["kind"] != "code" {
        return c["labels"][ch].as_str().unwrap_or(ch).to_string();
    }
    let change = c["change"].as_str().unwrap_or("");
    match ch {
        "both" => "Merge both".into(),
        "mine" => if change == "deleted-mine" { "Keep it deleted" } else { "Keep mine" }.into(),
        _ => if change == "deleted-theirs" { "Delete it" } else if change == "deleted-mine" { "Bring it back" } else { "Use the update's" }.into(),
    }
}

fn decide_sheet(cx: &mut Ctx, p: &mut Page, u: &Value) {
    let t = &u["t"];
    let left = n(&t["undecided"]);
    head(
        cx,
        p,
        "Before Tessera updates",
        &if left > 0 { format!("{} left to decide. Nothing changes until you update.", plural(left, "thing")) } else { "Nothing changes until you update.".to_string() },
        vec![("Cancel".into(), "close".into(), 0, true), ("Update Tessera".into(), "tu.update".into(), 1, left == 0 && b(&t["canUpdate"]))],
    );
    let conflicts = t["conflicts"].as_array().cloned().unwrap_or_default();
    for (kind, title, foot) in KINDS {
        let mine: Vec<&Value> = conflicts.iter().filter(|c| c["kind"] == kind).collect();
        if mine.is_empty() {
            continue;
        }
        let mut rows = Vec::new();
        for c in mine {
            let detail = if kind == "code" && !s(&c["mine"]).is_empty() { format!("{}\n{} · {}", s(&c["detail"]), s(&c["mine"]), s(&c["theirs"])) } else { s(&c["detail"]) };
            let choices = strings(&c["choices"]);
            let labels: Vec<String> = choices.iter().map(|ch| choice_label(c, ch)).collect();
            let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
            // (-1 until chosen: nothing is picked for you)
            let cur = choices.iter().position(|x| *x == s(&t["choices"][s(&c["id"])])).map_or(-1, |i| i as i32);
            let id = s(&c["id"]);
            let ch2 = choices.clone();
            let pick = cx.on(move |cx, a| {
                if let Some(ch) = ch2.get(a.i.max(0) as usize) {
                    cx.ask("about", "tu.choose", json!([id, ch]));
                }
            });
            let w = (labels.iter().map(|l| l.chars().count() as f32 * 6.5 + 24.0).sum::<f32>() + 4.0).max(260.0);
            rows.push(Row::new(s(&c["title"])).detail(detail).segmented(&labels, cur, pick).width(w));
        }
        let g = p.group(title);
        g.footer(foot);
        for r in rows {
            g.row(r);
        }
    }
}

/// Update notes (UpdateNotes.qml) as a custom row: each changelog entry
/// (title, date, its Markdown), the commits as lines when there's none
fn notes(cx: &mut Ctx, changelog: &Value, fallback: &Value) -> Row {
    let entries: Vec<Opt> = changelog.as_array().map(|a| a.iter().map(|e| Opt::new(s(&e["title"])).detail(s(&e["date"])).id(plain(&s(&e["body"]))).styled(styled(&s(&e["body"])))).collect()).unwrap_or_default();
    let fb = if entries.is_empty() { strings(fallback).iter().map(|l| format!("•  {l}")).collect::<Vec<_>>().join("\n") } else { String::new() };
    // (a link in a note: Launch.openLink, web links only)
    let open = cx.on(|cx, a| {
        if a.s.starts_with("https://") || a.s.starts_with("http://") {
            cx.ask("about", "openLink", json!([a.s.clone()]));
        }
    });
    Row::custom("about", "notes").id(open).data(entries).text_value(fb)
}

/// A note's Markdown as Slint's StyledText reads it (UpdateNotes.qml's
/// Text.MarkdownText): headings become bold lines, quotes and rules and raw
/// HTML go (StyledText refuses them); anything it still can't read is shown
/// as plain text
pub fn styled(md: &str) -> slint::StyledText {
    slint::StyledText::from_markdown(&for_styled(md)).unwrap_or_else(|_| slint::StyledText::from_plain_text(&plain(md)))
}

/// The Markdown StyledText takes (see `styled`)
pub fn for_styled(md: &str) -> String {
    let mut out = Vec::new();
    for line in md.lines() {
        let t = line.trim_start();
        if t.starts_with('#') {
            let h = t.trim_start_matches('#').trim();
            if !h.is_empty() {
                out.push(format!("**{}**", h.replace("**", "")));
                out.push(String::new());
            }
            continue;
        }
        if let Some(q) = t.strip_prefix('>') {
            out.push(q.trim_start().to_string());
            continue;
        }
        // (a rule: ---, ***, ___ alone on a line)
        let bare: String = t.chars().filter(|c| !c.is_whitespace()).collect();
        if bare.len() >= 3 && (bare.chars().all(|c| c == '-') || bare.chars().all(|c| c == '*') || bare.chars().all(|c| c == '_')) {
            continue;
        }
        if t.starts_with('<') {
            continue;
        }
        out.push(line.to_string());
    }
    out.join("\n").trim().to_string()
}

/// Markdown as plain text: bold and code marks dropped, links as their
/// text, list items as bullets
pub fn plain(md: &str) -> String {
    let mut out = String::new();
    for line in md.lines() {
        let t = line.trim_start();
        let indent = line.len() - t.len();
        let t = if let Some(r) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) { format!("{}•  {}", " ".repeat(indent), r) } else { line.to_string() };
        // (a heading's marks go; a nested item keeps its indent)
        let t = if t.starts_with('#') { t.trim_start_matches('#').trim_start().to_string() } else { t };
        // [text](url) -> text
        let mut s2 = String::new();
        let mut rest = t.as_str();
        while let Some(i) = rest.find('[') {
            match (rest[i..].find("]("), rest[i..].find(')')) {
                (Some(j), Some(k)) if j < k => {
                    s2.push_str(&rest[..i]);
                    s2.push_str(&rest[i + 1..i + j]);
                    rest = &rest[i + k + 1..];
                }
                _ => {
                    s2.push_str(&rest[..=i]);
                    rest = &rest[i + 1..];
                }
            }
        }
        s2.push_str(rest);
        out.push_str(&s2.replace("**", "").replace('`', ""));
        out.push('\n');
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A feed as the shell sends it, with `patch` merged in (one level deep)
    fn feed(patch: Value) -> Value {
        let mut u = json!({
            "allowed": true, "checked": true, "checking": false, "checkedAt": "21:58", "anything": false, "running": false, "restartComputer": false,
            "t": {"current": "2bfff4d0", "latest": "", "available": false, "job": null, "fraction": -1.0, "ready": false, "staged": "", "restarting": false,
                  "canUpdate": false, "undecided": 0, "error": "", "offline": false, "preparing": 0, "result": null, "hasNotes": false},
            "s": {"count": 0, "aurCount": 0, "aurAll": 0, "includeAur": false, "canCheck": true, "canUpdate": false, "running": false, "job": null,
                  "progress": -1.0, "cancellable": false, "interrupted": false, "hyprTooNew": false, "hold": -1},
            "back": {"count": 0, "skipped": []}, "snap": {"booted": ""}, "info": null,
        });
        for (k, v) in patch.as_object().unwrap() {
            if u[k.as_str()].is_object() && v.is_object() {
                for (k2, v2) in v.as_object().unwrap() {
                    u[k.as_str()][k2.as_str()] = v2.clone();
                }
            } else {
                u[k.as_str()] = v.clone();
            }
        }
        u
    }

    fn button(x: &Status) -> (&str, &str) {
        x.button.as_ref().map(|(l, a)| (l.as_str(), a.as_str())).unwrap_or(("", ""))
    }

    #[test]
    fn one_status_for_each_state() {
        let x = status(&feed(json!({"checked": false})));
        assert_eq!((x.title.as_str(), button(&x)), ("Check for updates", ("Check now", "su.check")));
        let x = status(&feed(json!({"checking": true})));
        assert_eq!((x.title.as_str(), x.busy), ("Checking for updates…", true));
        let x = status(&feed(json!({})));
        assert_eq!((x.title.as_str(), x.detail.as_str(), button(&x)), ("Everything is up to date", "Last checked at 21:58.", ("Check now", "su.check")));
        let x = status(&feed(json!({"anything": true, "t": {"available": true, "latest": "656db163", "canUpdate": true}, "s": {"count": 14, "canUpdate": true}})));
        assert_eq!((x.title.as_str(), x.detail.as_str(), button(&x), x.enabled), ("Updates are ready", "Tessera 2bfff4d0 → 656db163 · 14 system packages", ("Install updates", "su.updateAll"), true));
        // (decisions first: the button opens them)
        let x = status(&feed(json!({"anything": true, "t": {"available": true, "latest": "656db163", "undecided": 2}})));
        assert_eq!(button(&x), ("Install updates", "sheet:decide"));
        let x = status(&feed(json!({"t": {"preparing": 12}})));
        assert_eq!(x.title, "Preparing a new version of Tessera");
        assert!(x.detail.starts_with("Offered in about 12 min"));
        let x = status(&feed(json!({"running": true, "t": {"job": {"state": "running", "phase": "download", "how": "patch", "bytes": 8000000}}})));
        assert_eq!((x.title.as_str(), x.button.is_none()), ("Updating Tessera", true));
        let x = status(&feed(json!({"running": true, "s": {"running": true}})));
        assert_eq!(x.title, "Updating system packages");
        let x = status(&feed(json!({"t": {"ready": true, "staged": "656db163"}})));
        assert_eq!((x.title.as_str(), button(&x)), ("Tessera 656db163 is ready", ("Restart shell", "tu.restart")));
        let x = status(&feed(json!({"running": true, "t": {"restarting": true}})));
        assert_eq!((x.title.as_str(), x.busy), ("Restarting Tessera", true));
        let x = status(&feed(json!({"restartComputer": true})));
        assert_eq!(button(&x), ("Restart computer", "reboot"));
        let x = status(&feed(json!({"t": {"error": "Couldn't reach the update server: timed out", "offline": true}})));
        assert_eq!((x.title.as_str(), button(&x)), ("Can't reach the update server", ("Try again", "su.check")));
        let x = status(&feed(json!({"snap": {"booted": "1287", "assistant": false}})));
        assert_eq!(button(&x), ("Install Btrfs Assistant", "install:btrfs-assistant"));
        // (product UI copy: nothing shouts, no "another update")
        for x in [status(&feed(json!({}))), status(&feed(json!({"checked": false})))] {
            assert!(!x.title.contains('!') && !x.detail.contains("Another update"));
        }
    }

    #[test]
    fn an_update_running_elsewhere_is_never_a_dead_end() {
        let busy = |holder: Value| feed(json!({"t": {"job": {"state": "busy", "holder": holder}}}));
        let e = problem(&busy(json!({"what": "restart", "at": "21:58", "where": "other", "log": "/run/user/1000/tessera-update.x/log"}))).unwrap();
        assert_eq!(e.title, "Software Update is already running");
        assert_eq!(e.detail, "Started at 21:58 in another session, restarting Tessera into an update. Wait for it to finish, then check again.");
        assert_eq!(e.action, ("Show details".to_string(), "sheet:busy-log".to_string()));
        // (no log to show: Check again)
        let e = problem(&busy(json!({"what": "stage", "at": "", "where": "this"}))).unwrap();
        assert_eq!(e.detail, "Started in this session, getting an update ready. Wait for it to finish, then check again.");
        assert_eq!(e.action.1, "su.check");
        // (and Install updates waits for it)
        let u = feed(json!({"anything": true, "t": {"available": true, "latest": "656db163", "canUpdate": true, "job": {"state": "busy", "holder": {}}}}));
        assert!(!status(&u).enabled);
    }

    #[test]
    fn errors_say_how_to_fix_them_with_one_action() {
        let e = problem(&feed(json!({"t": {"job": {"state": "error", "error": "Couldn't download tessera-ui."}}}))).unwrap();
        assert_eq!((e.title.as_str(), e.action.1.as_str()), ("Tessera's update didn't finish", "tu.update"));
        assert!(e.detail.starts_with("Couldn't download tessera-ui. Nothing was changed. Try again"));
        let e = problem(&feed(json!({"s": {"interrupted": true}}))).unwrap();
        assert_eq!(e.action, ("Update packages".to_string(), "sys.update".to_string()));
        let e = problem(&feed(json!({"s": {"job": {"state": "error", "text": "The package database is in use", "error": "Another package manager (pacman, yay, paru or pamac) is running. Wait until it's done, then try again."}}}))).unwrap();
        assert_eq!((e.title.as_str(), e.action.1.as_str()), ("The package database is in use", "sheet:system-log"));
        let e = problem(&feed(json!({"t": {"error": "This copy has 2 commit(s) of its own"}}))).unwrap();
        assert_eq!(e.action.1, "su.check");
        // (offline is the status's, not a second row)
        assert!(problem(&feed(json!({"t": {"error": "Couldn't reach the update server: x", "offline": true}}))).is_none());
        assert!(problem(&feed(json!({}))).is_none());
    }

    #[test]
    fn tesseras_row_in_each_state() {
        let p = tessera_part(&feed(json!({"t": {"available": true, "latest": "656db163", "canUpdate": true, "hasNotes": true}})));
        assert_eq!(p.detail, "2bfff4d0 → 656db163");
        assert_eq!(p.buttons.iter().map(|b| (b.0.as_str(), b.1.as_str(), b.2)).collect::<Vec<_>>(), vec![("What's new", "sheet:notes", 4), ("Update", "tu.update", 0)]);
        let p = tessera_part(&feed(json!({"running": true, "t": {"job": {"state": "running", "phase": "download", "how": "patch", "bytes": 8000000}, "fraction": 0.43}})));
        let (f, pct, line) = p.progress.unwrap();
        assert_eq!((f, pct.as_str(), line.as_str()), (0.43, "43%", "Downloading a patch · 8.0 MB"));
        assert_eq!(p.buttons.iter().map(|b| b.0.as_str()).collect::<Vec<_>>(), vec!["Show details", "Cancel"]);
        assert_eq!(tessera_part(&feed(json!({"t": {"ready": true, "staged": "656db163"}}))).detail, "656db163 is ready to restart");
        assert_eq!(tessera_part(&feed(json!({}))).detail, "2bfff4d0 · Up to date");
        assert_eq!(tessera_part(&feed(json!({"t": {"available": true, "latest": "656db163", "job": {"state": "cancelled"}, "canUpdate": true}}))).detail, "2bfff4d0 → 656db163 · Cancelled, nothing was changed");
        // (a whole build line comes out as its commit)
        assert_eq!(short("0.1.0+1c236def85aff8775a6062089d8595ae1aae8401"), "1c236def");
        assert_eq!(short("2bfff4d0 (beta)"), "2bfff4d0");
    }

    #[test]
    fn systems_row_in_each_state() {
        let p = system_part(&feed(json!({"s": {"count": 14, "aurCount": 1, "includeAur": true, "canUpdate": true}})));
        assert_eq!(p.detail, "15 packages · 1 from the AUR");
        assert_eq!(p.buttons.last().unwrap().1, "sys.update");
        assert_eq!(system_part(&feed(json!({"s": {"count": 3, "aurAll": 2, "canUpdate": true}}))).detail, "3 packages · AUR updates not included");
        let p = system_part(&feed(json!({"running": true, "s": {"running": true, "progress": 0.5, "cancellable": true, "job": {"phase": "install", "text": "Installing", "package": "mesa"}}})));
        assert_eq!(p.progress.unwrap().2, "Installing · mesa");
        assert_eq!(p.buttons.len(), 2);
        let p = system_part(&feed(json!({"running": true, "s": {"running": true, "job": {"phase": "auth"}}})));
        assert_eq!(p.progress.unwrap(), (-1.0, String::new(), "Enter your password in the dialog".to_string()));
        assert_eq!(system_part(&feed(json!({"checked": false}))).detail, "Not checked yet");
        assert_eq!(system_part(&feed(json!({}))).detail, "Up to date");
        assert!(system_part(&feed(json!({"s": {"count": 2, "hyprTooNew": true, "hold": -1}}))).detail.ends_with("choose what to do about Hyprland first"));
    }

    #[test]
    fn the_cache_shows_only_when_there_is_one() {
        assert_eq!(cache_words(Some(3_200_000_000)).as_deref(), Some("3.2 GB"));
        assert_eq!(cache_words(Some(0)), None);
        assert_eq!(cache_words(None), None);
    }

    #[test]
    fn conflict_buttons_say_what_they_do() {
        let file = json!({"kind": "code", "change": "deleted-mine"});
        assert_eq!(choice_label(&file, "mine"), "Keep it deleted");
        assert_eq!(choice_label(&file, "theirs"), "Bring it back");
        assert_eq!(choice_label(&json!({"kind": "code"}), "both"), "Merge both");
        assert_eq!(choice_label(&json!({"kind": "setting", "labels": {"new": "Use the new one"}}), "new"), "Use the new one");
        assert_eq!(choice_label(&json!({"kind": "setting"}), "keep"), "keep");
    }

    #[test]
    fn job_lines() {
        assert_eq!(job_line(&json!({"phase": "download", "how": "full", "bytes": 29949608})), "Downloading the update · 29.9 MB");
        assert_eq!(job_line(&json!({"phase": "build"})), "Building Tessera");
        assert_eq!(job_line(&json!({"phase": "parts", "text": "Building the glass plugin"})), "Building the glass plugin");
        assert_eq!(job_line(&json!({"phase": "prepare", "text": "Preparing"})), "Preparing the update");
    }

    #[test]
    fn notes_as_plain_text() {
        assert_eq!(plain("**New:** see [the docs](https://x.y) and `cfg`\n- one\n  - two"), "New: see the docs and cfg\n•  one\n  •  two");
    }

    #[test]
    fn notes_markdown_styledtext_can_read() {
        assert_eq!(for_styled("## What's new\n- **Faster** [docs](https://x.y)\n> quoted\n---\n<br>\ntext"), "**What's new**\n\n- **Faster** [docs](https://x.y)\nquoted\ntext");
        assert!(slint::StyledText::from_markdown(&for_styled("### Fixes\n- one\n  - two\n\nSee `cfg` and *this*.")).is_ok());
    }

    #[test]
    fn package_lines() {
        let l = pkg("hyprland", "0.55", "0.56", "AUR", false);
        assert_eq!((l.detail.as_str(), l.on), ("0.55  →  0.56", true));
        assert!(!pkg("foot", "", "the terminal", "", false).on);
    }
}
