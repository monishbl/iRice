//! About (pages/about.qml): this computer, the subpages (System health,
//! Login Items, Report a Bug), the welcome screen again, and Back up and
//! restore (assets/scripts/settings-backup.py, run here as the QML page runs
//! it: the settings, shortcuts, Focus modes and wallpaper in one file,
//! without keys, passwords, accounts or this computer's displays; Restore
//! shows what's in the file first, then Undo puts back what was there).

use super::super::super::model::{btn, Action, Ctx, Page, Row};
use super::{feed, s, sysinfo, tilde};
use serde_json::{json, Value};

fn script() -> String {
    crate::util::asset("shell/assets/scripts/settings-backup.py").map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}
fn config_dir() -> String {
    crate::theme::config_path().parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The helper's answer: one JSON object (blocking)
pub fn helper(args: Vec<String>) -> Value {
    let out = std::process::Command::new("python3").arg("-I").arg(script()).args(&args).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output();
    out.ok()
        .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok())
        .unwrap_or_else(|| json!({"ok": false, "error": "The backup helper didn't answer. Check that Python 3 is installed, then try again."}))
}

/// "2026-11-05T15:04:00" as "5 Nov 2026, 3:04 PM"
pub fn created(iso: &str) -> String {
    const MON: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let num = |a: usize, b: usize| iso.get(a..b).and_then(|x| x.parse::<u32>().ok());
    let (Some(y), Some(mo), Some(d), Some(h), Some(mi)) = (num(0, 4), num(5, 7), num(8, 10), num(11, 13), num(14, 16)) else { return String::new() };
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    format!("{d} {} {y}, {h12}:{mi:02} {}", MON.get(mo.saturating_sub(1) as usize).unwrap_or(&""), if h < 12 { "AM" } else { "PM" })
}

/// What a settings file holds, in a sentence (about.qml's describe)
pub fn describe(i: &Value) -> String {
    let mode = s(&i["mode"]);
    let mut parts = vec![format!(
        "{} look{}",
        if i["look"] == "material" { "Material" } else { "Glass" },
        if mode.is_empty() {
            String::new()
        } else {
            format!(", {}", match mode.as_str() {
                "light" => "light",
                "dark" => "dark",
                "auto" => "light and dark by the sun",
                m => m,
            })
        }
    )];
    parts.push(format!("wallpaper: {}", s(&i["wallpaper"])));
    let n = i["shortcuts"].as_i64().unwrap_or(0);
    if n > 0 {
        parts.push(format!("{n} {}", if n == 1 { "shortcut change" } else { "shortcut changes" }));
    }
    let focus = super::strings(&i["focusModes"]);
    if !focus.is_empty() {
        parts.push(format!("Focus: {}", focus.join(", ")));
    }
    let c = created(&s(&i["created"]));
    format!("{}{}", parts.join(" · "), if c.is_empty() { ".".to_string() } else { format!(". Saved {c}.") })
}

/// The look restored: switched to, as Settings → Appearance does
fn follow(cx: &Ctx, look: &str) {
    let now = if cx.material { "material" } else { "glass" };
    if !look.is_empty() && look != now {
        if let Some(sw) = crate::util::asset("shell/assets/scripts/switch-look.sh") {
            Ctx::detach(vec!["setsid".into(), "-f".into(), sw.to_string_lossy().into_owned(), look.to_string()]);
        }
    }
}

/// The helper's answers that came in since the page was last built
fn answers(cx: &mut Ctx) {
    if let Some(out) = cx.local.remove("about.backup.export") {
        let ok = out["ok"] == true;
        cx.local_set("about.backup.status", if ok { String::new() } else { s(&out["error"]) });
        cx.local_set("about.backup.savedTo", if ok { s(&out["file"]) } else { String::new() });
    }
    if let Some(out) = cx.local.remove("about.backup.inspect") {
        let ok = out["ok"] == true;
        cx.local_set("about.backup.status", if ok { String::new() } else { s(&out["error"]) });
        let file = cx.local_s("about.backup.choosing");
        cx.local_set("about.backup.pending", if ok { json!({"file": file, "info": out}) } else { Value::Null });
    }
    if let Some(out) = cx.local.remove("about.backup.restore") {
        if out["ok"] != true {
            cx.local_set("about.backup.status", s(&out["error"]));
        } else {
            let skipped = super::strings(&out["skipped"]);
            cx.local_set("about.backup.status", if skipped.is_empty() { String::new() } else { format!("Left out {}.", skipped.join(" and ")) });
            cx.ask("about", "focusReload", Value::Null);
            let undo_file = s(&out["undo"]);
            let undo: Action = super::action(move |cx, _| {
                let f = undo_file.clone();
                cx.spawn("about.backup.undo", move || helper(vec!["restore".into(), f, "--config".into(), config_dir(), "--no-undo".into()]));
            });
            cx.toast("Settings restored", Some(undo));
            follow(cx, &s(&out["look"]));
        }
    }
    if let Some(back) = cx.local.remove("about.backup.undo") {
        let ok = back["ok"] == true;
        cx.local_set("about.backup.status", if ok { String::new() } else { s(&back["error"]) });
        if ok {
            cx.ask("about", "focusReload", Value::Null);
            cx.toast("Your earlier settings are back", None);
            follow(cx, &s(&back["look"]));
        }
    }
}

pub fn page(cx: &mut Ctx, p: &mut Page) {
    let f = feed(cx);
    let info = cx.once("about.sysinfo", sysinfo::read);
    let name_ph = {
        let n = s(&f["welcomeName"]);
        let n = if n.is_empty() { s(&info["name"]) } else { n };
        let first = n.split_whitespace().next().unwrap_or("").to_string();
        if first.is_empty() { "Your name".to_string() } else { first }
    };
    let name_set = cx.on(|cx, a| {
        if a.i == 1 {
            let t = a.s.trim().to_string();
            if t != cx.s("today.name") {
                cx.set("today.name", t);
            }
        }
    });
    let today_name = cx.s("today.name");
    p.group("")
        .row(Row::new("Tessera").shows("1.0"))
        .row(Row::new("System").shows(if info.is_null() { "Linux".to_string() } else { s(&info["os"]) }))
        .row(Row::new("Computer name").shows(s(&info["host"])))
        .row(Row::new("Your name").detail("Today greets you by it. Empty: your account's name").field(today_name, name_ph, name_set).width(220.0))
        .row(Row::new("Processor").shows(s(&info["cpu"])))
        .row(Row::new("Graphics").shows(s(&info["gpu"])))
        .row(Row::new("Memory").shows(s(&info["memory"])));

    let checks = f["healthChecks"].as_i64().unwrap_or(0);
    let attention = f["healthAttention"].as_i64().unwrap_or(0);
    let health = cx.link("about/health", "");
    let login = cx.link("about/loginitems", "");
    let bug = cx.link("about/bugreport", "");
    p.group("")
        .row(Row::link("System health", "about/health")
            .icon("heartbeat")
            .detail("Whether the services Tessera uses are working, and how to fix them")
            .value(if checks == 0 { String::new() } else if attention == 0 { "All good".into() } else { format!("{attention} to check") })
            .id(health))
        .row(Row::link("Login Items", "about/loginitems").icon("rocket-launch").detail("The apps that start when you log in").id(login))
        .row(Row::link("Report a Bug", "about/bugreport").icon("bug").detail("Tell Tessera what went wrong, with what helps fix it").id(bug));

    let welcome = cx.on(|cx, _| cx.ask("about", "welcome", Value::Null));
    p.group("")
        .footer("Settings live in ~/.config/tessera/config.json and apply as you change them.")
        .row(Row::new("Welcome screen").detail("Pick the look, light or dark, and a wallpaper again").button("Show", welcome));

    // ── Back up and restore: your setup in one file ──
    answers(cx);
    let status = cx.local_s("about.backup.status");
    let saved = cx.local_s("about.backup.savedTo");
    let pending = cx.local.get("about.backup.pending").cloned().unwrap_or(Value::Null);
    let busy = ["about.backup.export.busy", "about.backup.inspect.busy", "about.backup.restore.busy", "about.backup.undo.busy"].iter().any(|k| cx.local_b(k));
    let back_up = cx.on(|cx, a| {
        if a.i == 1 {
            let dir = a.s.clone();
            cx.local_set("about.backup.status", "Saving…");
            cx.local_set("about.backup.savedTo", "");
            cx.spawn("about.backup.export", move || helper(vec!["export".into(), dir, "--config".into(), config_dir()]));
        } else {
            cx.choose_file(&a.id, true, "Choose where to save your settings", "~/Documents", "");
        }
    });
    let show = {
        let folder = saved.rsplit_once('/').map_or("/".to_string(), |x| if x.0.is_empty() { "/".to_string() } else { x.0.to_string() });
        cx.on(move |cx, _| cx.ask("about", "openFile", json!([folder])))
    };
    let restore = cx.on(|cx, a| {
        if a.i == 1 {
            let file = a.s.clone();
            cx.local_set("about.backup.status", "Reading…");
            cx.local_set("about.backup.pending", Value::Null);
            cx.local_set("about.backup.choosing", file.clone());
            cx.spawn("about.backup.inspect", move || helper(vec!["inspect".into(), file]));
        } else {
            cx.choose_file(&a.id, false, "Choose a Tessera settings file", "~/Documents", "Tessera settings|*.tessera *.irice");
        }
    });
    let decide = {
        let file = s(&pending["file"]);
        cx.on(move |cx, a| {
            cx.local_set("about.backup.pending", Value::Null);
            if a.i == 1 {
                let file = file.clone();
                cx.local_set("about.backup.status", "Restoring…");
                cx.spawn("about.backup.restore", move || helper(vec!["restore".into(), file, "--config".into(), config_dir()]));
            }
        })
    };
    p.group("Back up and restore")
        .footer(if !status.is_empty() { status } else { "One file with your settings, shortcuts, Focus modes and wallpaper, without passwords or accounts. Restore it here or on another computer.".into() })
        .row(Row::new("Back up")
            .detail(format!("Saves “Tessera settings {}.tessera” in a folder you choose", crate::util::now("%Y-%m-%d")))
            .buttons(vec![btn("Back Up Settings…").off(busy)], back_up))
        .row(Row::new("Saved").visible(!saved.is_empty()).detail(tilde(&saved)).button("Show in Files", show))
        .row(Row::new("Restore").detail("Your keys, accounts and displays here stay as they are").buttons(vec![btn("Restore…").off(busy)], restore))
        .row(Row::new("Restore these settings?")
            .visible(!pending.is_null())
            .detail(if pending.is_null() { String::new() } else { describe(&pending["info"]) })
            .buttons(vec![btn("Cancel"), btn("Restore Settings").primary().off(busy)], decide));

    // ── More options ──
    p.more(cx);
    {
        let cfg = crate::theme::config_path().to_string_lossy().into_owned();
        let open = cx.on(move |cx, _| cx.ask("about", "openFile", json!([cfg])));
        p.group("").row(Row::new("Configuration file").button("Open", open));
    }
    p.end_more();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_settings_file_described() {
        let i = json!({"look": "material", "mode": "auto", "wallpaper": "Tessera's own", "shortcuts": 1, "focusModes": ["Work"], "created": "2026-11-05T15:04:00"});
        assert_eq!(describe(&i), "Material look, light and dark by the sun · wallpaper: Tessera's own · 1 shortcut change · Focus: Work. Saved 5 Nov 2026, 3:04 PM.");
        let i = json!({"look": "glass", "mode": "", "wallpaper": "a.jpg", "shortcuts": 0, "focusModes": [], "created": ""});
        assert_eq!(describe(&i), "Glass look · wallpaper: a.jpg.");
        assert_eq!(created("2026-01-02T00:09:00"), "2 Jan 2026, 12:09 AM");
    }
}
