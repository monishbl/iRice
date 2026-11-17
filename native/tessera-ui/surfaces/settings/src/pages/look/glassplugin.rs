//! "Glass effect is off": the row at the top of Appearance and of its Glass
//! page while the Glass look is in use and the tessera-glass Hyprland plugin
//! isn't working (not built, built for another Hyprland, an older build, or
//! its last build failed), with the button that builds it inside the shell
//! (host/shell/src/glassbuild.rs runs tools/build-plugin.sh --load) and how
//! far that build is. More options' "Compositor glass" row says the same.
//!
//! The shell's side is `feed["look"]["glassBuild"]` (glassbuild.rs's state):
//!   running, headers   the running Hyprland's version, its headers' (pkg-config)
//!   builtFor, built    tessera-glass.built-for, and whether the plugin file is there
//!   build              {active, failed, fraction (-1: unknown), time, error}
//!   other              another build (the shell, an update) is running
//!   starting, skipped  Build was just pressed; it was skipped (a sandbox)

use super::super::super::model::{btn, Ctx, Page, Row};
use super::feed;
use serde_json::{json, Value};

/// What the row says and offers (None: the plugin works, or there's no glass)
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    /// one of: missing, updated, outdated, unloaded, failed, building
    pub kind: &'static str,
    pub detail: String,
    /// "Build glass", "Rebuild glass", "Try again" (empty while building)
    pub button: &'static str,
    pub busy: bool,
    /// Show log beside the button
    pub log: bool,
    /// the build's progress (0…1; -1: no honest number), while building,
    /// and what the bar says ("40%", else the time so far)
    pub fraction: f64,
    pub progress: String,
    /// the button can't run now (another build is running)
    pub blocked: bool,
}

pub const TITLE: &str = "Glass effect is off";
pub const HEADERS: &str = "Hyprland's headers don't match the running Hyprland: update the hyprland package, then log out and back in";

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

/// X.Y.Z out of a version ("v0.57.2-b" → "0.57.2")
pub fn xyz(v: &str) -> String {
    let start = v.find(|c: char| c.is_ascii_digit()).unwrap_or(v.len());
    let rest = &v[start..];
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
    rest[..end].trim_end_matches('.').to_string()
}

/// Why the last build failed, in one line
pub fn failure_reason(st: &Value) -> String {
    let err = s(&st["build"]["error"]).trim();
    let (headers, running) = (xyz(s(&st["headers"])), xyz(s(&st["running"])));
    let low = err.to_lowercase();
    if (!headers.is_empty() && !running.is_empty() && headers != running) || (low.contains("headers") && low.contains("match")) || low.contains("version mismatch") {
        return HEADERS.into();
    }
    let line = err.lines().next().unwrap_or("").trim_start_matches("error:").trim();
    if line.is_empty() {
        return "Show log says why".into();
    }
    // (one line: a compiler's message can be long)
    if line.chars().count() > 140 {
        format!("{}…", line.chars().take(139).collect::<String>())
    } else {
        line.to_string()
    }
}

/// The row for the plugin's state: `glass` is whether the look has glass
/// (Glass look, not Solid), `loaded`/`version` what the ping found
pub fn problem(glass: bool, loaded: bool, version: i64, st: &Value) -> Option<Problem> {
    if !glass {
        return None;
    }
    let build = &st["build"];
    let other = st["other"] == true;
    let p = |kind, detail: String, button| Problem { kind, detail, button, busy: false, log: false, fraction: -1.0, progress: String::new(), blocked: other };
    if st["starting"] == true || build["active"] == true {
        let fraction = build["fraction"].as_f64().unwrap_or(-1.0);
        let how = if build["active"] == true { super::super::about::update::build_progress_text(fraction, s(&build["time"])) } else { String::new() };
        let detail = if how.is_empty() { "Building…".to_string() } else { format!("Building… {how}") };
        return Some(Problem { busy: true, fraction, progress: how, blocked: false, ..p("building", detail, "Building…") });
    }
    let outdated = loaded && (version == 3 || version == 4);
    if loaded && !outdated {
        return None;
    }
    let mut row = if build["failed"] == true {
        Problem { log: true, ..p("failed", format!("The last build failed: {}", failure_reason(st)), "Try again") }
    } else if outdated {
        p("outdated", "An older build: update it so glass never shows over full-screen apps".into(), "Rebuild glass")
    } else {
        let (built_for, running) = (xyz(s(&st["builtFor"])), xyz(s(&st["running"])));
        if !built_for.is_empty() && !running.is_empty() && built_for != running {
            p("updated", "Hyprland was updated, so the plugin needs rebuilding".into(), "Rebuild glass")
        } else if st["built"] != true {
            p("missing", "The glass plugin isn't built yet".into(), "Build glass")
        } else {
            p("unloaded", "The glass plugin is built, but Hyprland hasn't loaded it".into(), "Rebuild glass")
        }
    };
    if st["skipped"] == true {
        row.detail = format!("{} · Skipped in a sandbox", row.detail);
    }
    if other {
        row.detail = format!("{} · Another build is running", row.detail);
    }
    Some(row)
}

/// The problem for this page's settings and the shell's feed
pub fn current(cx: &Ctx) -> Option<Problem> {
    let glass = !cx.material && super::super::glass_tier(cx) != "solid";
    let f = feed(cx);
    let loaded = f["glassPlugin"].as_bool().unwrap_or(cx.shell.plugin);
    let version = f["glassPluginVersion"].as_i64().unwrap_or(0);
    problem(glass, loaded, version, &f["glassBuild"])
}

/// The buttons a problem offers (Show log, then the primary one)
fn actions(cx: &mut Ctx, pr: &Problem) -> (Vec<super::super::super::model::Opt>, String) {
    let log = pr.log;
    let act = cx.on(move |cx, a| {
        if log && a.i == 0 {
            cx.ask("look", "glassBuildLog", Value::Null);
        } else {
            cx.ask("look", "buildPlugin", json!([]));
        }
    });
    let mut b = Vec::new();
    if pr.log {
        b.push(btn("Show log").icon("terminal-window"));
    }
    b.push(btn(pr.button).primary().on(pr.busy).off(pr.busy || pr.blocked));
    (b, act)
}

/// The row at the top of a page (nothing when the plugin works)
pub fn top(cx: &mut Ctx, p: &mut Page) {
    let Some(pr) = current(cx) else { return };
    let (b, act) = actions(cx, &pr);
    let g = p.group("");
    g.row(Row::new(TITLE).icon("warning").detail(pr.detail.clone()).buttons(b, act));
    if pr.busy {
        g.row(Row::custom("about", "progress").num(if pr.fraction >= 0.0 { pr.fraction.max(0.02) as f32 } else { -1.0 }).text_value(String::new()));
        // (the percent or the time is in the row's detail: not said twice)
    }
}

/// More options' "Compositor glass" row, in step with the top one
pub fn compositor_row(cx: &mut Ctx) -> Row {
    let row = Row::new("Compositor glass");
    match current(cx) {
        Some(pr) => {
            let (b, act) = actions(cx, &pr);
            row.detail(pr.detail).buttons(b, act)
        }
        None => row.detail("Active: Hyprland draws the glass").ctl("look", "check", String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(v: Value) -> Value {
        let mut base = json!({"running": "0.57.2", "headers": "0.57.2", "builtFor": "", "built": false, "build": {}, "other": false, "starting": false, "skipped": false});
        for (k, x) in v.as_object().unwrap() {
            base[k] = x.clone();
        }
        base
    }

    #[test]
    fn versions() {
        assert_eq!(xyz("v0.57.2-b"), "0.57.2");
        assert_eq!(xyz("0.57.2.r12.gabc"), "0.57.2");
        assert_eq!(xyz("Hyprland 0.56"), "0.56");
        assert_eq!(xyz(""), "");
    }

    #[test]
    fn not_built() {
        let p = problem(true, false, 0, &st(json!({}))).unwrap();
        assert_eq!((p.kind, p.detail.as_str(), p.button, p.busy, p.log), ("missing", "The glass plugin isn't built yet", "Build glass", false, false));
    }

    #[test]
    fn built_for_another_hyprland() {
        let p = problem(true, false, 0, &st(json!({"builtFor": "0.56.2", "built": true}))).unwrap();
        assert_eq!((p.kind, p.detail.as_str(), p.button), ("updated", "Hyprland was updated, so the plugin needs rebuilding", "Rebuild glass"));
        // (the same Hyprland, but not loaded)
        let p = problem(true, false, 0, &st(json!({"builtFor": "0.57.2", "built": true}))).unwrap();
        assert_eq!((p.kind, p.button), ("unloaded", "Rebuild glass"));
        // (the running version not known yet: nothing claimed about it)
        let p = problem(true, false, 0, &st(json!({"builtFor": "0.56.2", "built": true, "running": ""}))).unwrap();
        assert_eq!(p.kind, "unloaded");
    }

    #[test]
    fn an_older_build() {
        for v in [3, 4] {
            let p = problem(true, true, v, &st(json!({"builtFor": "0.57.2", "built": true}))).unwrap();
            assert_eq!((p.kind, p.detail.as_str(), p.button), ("outdated", "An older build: update it so glass never shows over full-screen apps", "Rebuild glass"));
        }
    }

    #[test]
    fn the_last_build_failed() {
        let f = st(json!({"build": {"failed": true, "error": "main.cpp:12:3: error: 'x' was not declared in this scope"}}));
        let p = problem(true, false, 0, &f).unwrap();
        assert_eq!((p.kind, p.button, p.log), ("failed", "Try again", true));
        assert_eq!(p.detail, "The last build failed: main.cpp:12:3: error: 'x' was not declared in this scope");
        // (the headers aren't the running Hyprland's: said so, with what to do)
        let p = problem(true, false, 0, &st(json!({"headers": "0.57.2", "running": "0.57.1", "build": {"failed": true, "error": "main.cpp:1: error: x"}}))).unwrap();
        assert_eq!(p.detail, format!("The last build failed: {HEADERS}"));
        let p = problem(true, false, 0, &st(json!({"build": {"failed": true, "error": "error: Hyprland's headers (0.57.2) don't match the running Hyprland (0.57.1)"}}))).unwrap();
        assert_eq!(p.detail, format!("The last build failed: {HEADERS}"));
        // (no error line: the log says)
        let p = problem(true, false, 0, &st(json!({"build": {"failed": true, "error": ""}}))).unwrap();
        assert_eq!(p.detail, "The last build failed: Show log says why");
        // (a failure while an older build still works: still offered)
        assert_eq!(problem(true, true, 4, &f).unwrap().kind, "failed");
    }

    #[test]
    fn building() {
        let p = problem(true, false, 0, &st(json!({"build": {"active": true, "fraction": 0.4, "time": "0:16 · less than a minute left"}}))).unwrap();
        assert_eq!((p.kind, p.detail.as_str(), p.busy), ("building", "Building… 40%", true));
        // (no history: the time so far)
        let p = problem(true, false, 0, &st(json!({"build": {"active": true, "fraction": -1.0, "time": "0:07"}}))).unwrap();
        assert_eq!(p.detail, "Building… 0:07");
        // (just pressed: before the build's first word)
        let p = problem(true, false, 0, &st(json!({"starting": true}))).unwrap();
        assert_eq!((p.detail.as_str(), p.busy), ("Building…", true));
        // (rebuilding an older build that's loaded: shown too)
        assert_eq!(problem(true, true, 4, &st(json!({"starting": true}))).unwrap().kind, "building");
    }

    #[test]
    fn working_or_no_glass() {
        assert_eq!(problem(true, true, 5, &st(json!({"builtFor": "0.57.2", "built": true}))), None);
        // (loaded and current: an old failure doesn't matter)
        assert_eq!(problem(true, true, 5, &st(json!({"build": {"failed": true, "error": "x"}}))), None);
        // Material, or Solid glass: never offered
        assert_eq!(problem(false, false, 0, &st(json!({}))), None);
        // (no feed yet: not built is what's known)
        assert_eq!(problem(true, false, 0, &Value::Null).unwrap().kind, "missing");
    }

    #[test]
    fn sandbox_and_another_build() {
        let p = problem(true, false, 0, &st(json!({"skipped": true}))).unwrap();
        assert_eq!(p.detail, "The glass plugin isn't built yet · Skipped in a sandbox");
        let p = problem(true, false, 0, &st(json!({"other": true}))).unwrap();
        assert!(p.blocked);
        assert_eq!(p.detail, "The glass plugin isn't built yet · Another build is running");
    }
}
