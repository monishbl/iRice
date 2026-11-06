//! System health (pages/about-health.qml): each service Tessera uses,
//! checked by the shell when the page opens (services/Health.qml; the
//! bridge asks it), with how to fix what's broken. A fix that runs as
//! administrator is confirmed first: the row shows exactly what will run,
//! and only the second press runs it (the shell runs it, never this).

use super::super::super::model::{btn, Ctx, Opt, Page, Row};
use super::{b, feed, s, strings};
use serde_json::{json, Value};

/// A status as a word (never colour alone)
pub fn word(status: &str) -> &str {
    match status {
        "ok" => "OK",
        "warn" => "Check",
        "fail" => "Not working",
        "off" => "Off",
        s => s,
    }
}

/// A status's colour for the dot: 0 success, 1 warning, 2 danger, 3 tertiary
pub fn tone(status: &str) -> i32 {
    match status {
        "ok" => 0,
        "warn" => 1,
        "fail" => 2,
        _ => 3,
    }
}

fn cap(t: &str) -> String {
    let mut c = t.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

pub fn page(cx: &mut Ctx, p: &mut Page) {
    p.title = "System health".into();
    let f = feed(cx);
    let h = &f["health"];
    let checks: Vec<Value> = h["checks"].as_array().cloned().unwrap_or_default();
    let attention: Vec<&Value> = checks.iter().filter(|c| c["status"] == "fail" || c["status"] == "warn").collect();
    let checking = b(&h["checking"]);
    let err = s(&h["error"]);
    let at = s(&h["checkedAt"]);
    let label = if checking && checks.is_empty() {
        "Checking…".to_string()
    } else if attention.is_empty() {
        "All good".into()
    } else if attention.len() == 1 {
        "1 thing needs attention".into()
    } else {
        format!("{} things need attention", attention.len())
    };
    let detail = if checks.is_empty() {
        String::new()
    } else if attention.is_empty() {
        "Every service Tessera uses is answering.".into()
    } else {
        attention.iter().map(|c| s(&c["name"])).collect::<Vec<_>>().join(", ")
    };
    let failing = attention.iter().any(|c| c["status"] == "fail");
    let top = cx.on(|cx, a| cx.ask("about", if a.i == 0 { "health.copy" } else { "health.check" }, Value::Null));
    p.group("")
        .footer(if !err.is_empty() {
            err
        } else if !at.is_empty() {
            format!("Checked at {at}. Nothing is checked in the background; this page asks each service when it opens.")
        } else {
            String::new()
        })
        .row(Row::new(label).tone(if failing { 2 } else { 0 }).detail(detail).buttons(
            vec![btn("Copy report").icon("copy").off(checks.is_empty()), btn(if checking { "Checking…" } else { "Check again" }).icon("arrow-clockwise").on(checking)],
            top,
        ));

    // Things also running that do Tessera's job twice (services/Conflicts.qml)
    let found: Vec<Value> = f["conflicts"].as_array().cloned().unwrap_or_default();
    let mut rows = Vec::new();
    for c in &found {
        let muted = b(&c["muted"]);
        let id = s(&c["id"]);
        let t = cx.on(move |cx, _| cx.ask("about", if muted { "conflicts.unmute" } else { "conflicts.mute" }, json!([id])));
        rows.push(Row::new(cap(&s(&c["name"]))).detail(format!("{}.\nTo stop it: {}", cap(&s(&c["what"])), s(&c["fix"]))).button(if muted { "Show alerts" } else { "Don't show again" }, t));
    }
    let g = p.group("Also running");
    g.footer("These do jobs Tessera already does, so things can happen twice or not show. Stop the ones you don't need; Tessera doesn't stop them for you.");
    for r in rows {
        g.row(r);
    }

    // Each group's checks
    let confirming = cx.local_s("about.health.confirming");
    let fixing = s(&h["fixing"]);
    for group in strings(&h["groups"]) {
        let mut rows = Vec::new();
        for c in checks.iter().filter(|c| s(&c["group"]) == group) {
            let id = s(&c["id"]);
            let status = s(&c["status"]);
            let asking = confirming == id;
            let pw = b(&c["fixPassword"]);
            let fix = s(&c["fix"]);
            let detail = format!(
                "{}{}",
                s(&c["detail"]),
                if asking {
                    format!("{}{}", if pw { "\nRuns as administrator, after your password: " } else { "\nRuns as administrator: " }, s(&c["fixCommand"]))
                } else if !fix.is_empty() && (status == "fail" || status == "warn") {
                    format!("\nTo fix: {fix}")
                } else {
                    String::new()
                }
            );
            let mut opts: Vec<Opt> = Vec::new();
            let mut roles: Vec<&'static str> = Vec::new();
            if asking {
                opts.push(btn("Cancel"));
                roles.push("cancel");
            }
            if b(&c["canFix"]) {
                let label = if fixing == id { "Fixing…" } else if asking { if pw { "Run with password" } else { "Run fix" } } else { "Fix" };
                let mut o = btn(label).on(fixing == id).off(!fixing.is_empty());
                if asking {
                    o = o.primary();
                }
                opts.push(o);
                roles.push("fix");
            }
            let root = b(&c["fixRoot"]);
            let idc = id.clone();
            let act = cx.on(move |cx, a| match roles.get(a.i as usize) {
                Some(&"cancel") => cx.local_set("about.health.confirming", ""),
                Some(&"fix") => {
                    if root && cx.local_s("about.health.confirming") != idc {
                        cx.local_set("about.health.confirming", idc.clone());
                        return;
                    }
                    cx.local_set("about.health.confirming", "");
                    cx.ask("about", "health.fix", json!([idc]));
                }
                _ => {}
            });
            let mut r = Row::new(s(&c["name"])).detail(detail).ctl("about", "health", act);
            r.opts = opts;
            r.value = word(&status).to_string();
            r.index = tone(&status);
            rows.push(r);
        }
        let g = p.group(group);
        for r in rows {
            g.row(r);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_in_words_and_colours() {
        assert_eq!((word("fail"), tone("fail")), ("Not working", 2));
        assert_eq!((word("ok"), tone("ok")), ("OK", 0));
        assert_eq!((word("off"), tone("off")), ("Off", 3));
        assert_eq!(cap("pipewire-pulse"), "Pipewire-pulse");
    }
}
