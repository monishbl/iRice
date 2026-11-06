//! Report a Bug (pages/about-bugreport.qml): what happened, and what's
//! attached (screenshot, log, system info), each shown before it's sent and
//! each can be left out. The report is the shell's (services/BugReport.qml:
//! it scrubs the log and info and sends them); this shows its state and asks
//! it (`bug.*`). Nothing private is read here.

use super::super::super::model::{btn, Ctx, Opt, Page, Row};
use super::{b, feed, n, s};
use serde_json::{json, Value};

thread_local! {
    static SHOT: std::cell::RefCell<Option<(String, Option<slint::Image>)>> = const { std::cell::RefCell::new(None) };
}

/// The screenshot as it is now (read again when the shell says it changed)
fn shot(path: &str, version: i64) -> Option<slint::Image> {
    let key = format!("{path}?v={version}");
    SHOT.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((k, img)) = c.as_ref() {
            if *k == key {
                return img.clone();
            }
        }
        let img = slint::Image::load_from_path(std::path::Path::new(path)).ok();
        *c = Some((key, img.clone()));
        img
    })
}

/// The label of the Send row (what state the report is in)
pub fn send_label(state: &str, error: &str, wait: i64) -> String {
    match state {
        "sending" => "Sending…".into(),
        "sent" => "Report sent. Thank you!".into(),
        "error" => error.into(),
        _ if wait > 0 => format!("You can send another in {wait} s"),
        _ => "Ready to send".into(),
    }
}

/// A row with buttons and a switch at its end (`i` -1 is the switch)
fn with_switch(label: &str, detail: String, buttons: Vec<Opt>, on: bool, enabled: bool, id: String) -> Row {
    let mut r = Row::new(label).detail(detail).ctl("about", "btnswitch", id);
    r.opts = buttons;
    r.on = on;
    r.enabled = enabled;
    r
}

pub fn page(cx: &mut Ctx, p: &mut Page) {
    p.title = "Report a Bug".into();
    let f = feed(cx);
    let r = &f["bug"];
    let gathering = b(&r["gathering"]);

    // ── What happened ──
    let title = cx.on(|cx, a| cx.ask("about", "bug.set", json!(["title", a.s])));
    let boxes = cx.on(|cx, a| {
        let field = ["happened", "steps", "expected"].get(a.i as usize).copied().unwrap_or("happened");
        cx.ask("about", "bug.set", json!([field, a.s]));
    });
    let fields = vec![
        Opt::new("What happened").detail("What you saw, and when").id(s(&r["happened"])),
        Opt::new("Steps to reproduce").detail("1. Open …  2. Click …  (optional)").id(s(&r["steps"])),
        Opt::new("What you expected").detail("Optional").id(s(&r["expected"])),
    ];
    p.group("What happened")
        .footer("Write in your own words; a few clear sentences help most. Reports become public issues on GitHub, so don't include anything private.")
        .row(Row::new("Title").field(s(&r["title"]), "e.g. Dock disappears after waking", title).width(300.0))
        .row(Row::custom("about", "boxes").id(boxes).data(fields));

    // ── Attached ──
    let has_shot = b(&r["hasShot"]);
    let inc_shot = b(&r["includeShot"]);
    let inc_log = b(&r["includeLog"]);
    let inc_info = b(&r["includeInfo"]);
    let show_log = cx.local_b("about.bug.showLog");
    let show_info = cx.local_b("about.bug.showInfo");
    let shot_act = cx.on(move |cx, a| {
        if a.i < 0 {
            cx.ask("about", "bug.include", json!(["includeShot", !inc_shot]));
        } else {
            cx.ask("about", "bug.editShot", Value::Null);
        }
    });
    let log_act = cx.on(move |cx, a| {
        if a.i < 0 {
            cx.ask("about", "bug.include", json!(["includeLog", !inc_log]));
        } else {
            cx.local_set("about.bug.showLog", !show_log);
        }
    });
    let info_act = cx.on(move |cx, a| {
        if a.i < 0 {
            cx.ask("about", "bug.include", json!(["includeInfo", !inc_info]));
        } else {
            cx.local_set("about.bug.showInfo", !show_info);
        }
    });
    let picture = if has_shot && inc_shot { shot(&s(&r["shot"]), n(&r["shotVersion"])) } else { None };
    let log = if gathering { "Reading the log…".to_string() } else if s(&r["log"]).is_empty() { "The log is empty.".into() } else { s(&r["log"]) };
    let info = if gathering { "Reading…".to_string() } else { s(&r["info"]) };
    let g = p.group("Attached");
    g.footer("Only what's on is sent. Home folders, your user and computer names, email addresses, keys, IP and MAC addresses, and the names of your Wi-Fi networks and Bluetooth devices are taken out of the log and system info first.");
    g.row(with_switch(
        "Screenshot",
        if has_shot { "The screen when you started the report. Mark it up to hide anything private".into() } else { "None was taken. Open this page with the Report a bug shortcut or Search to include one".into() },
        if has_shot { vec![btn("Mark up…").icon("pencil-simple")] } else { Vec::new() },
        inc_shot && has_shot,
        has_shot,
        shot_act,
    ));
    if let Some(img) = picture {
        g.row(Row::custom("about", "shot").image(img));
    }
    g.row(with_switch("Tessera log", if gathering { "Reading…".into() } else { "The last 500 lines, scrubbed".into() }, vec![btn(if show_log { "Hide" } else { "Show" })], inc_log, true, log_act));
    if show_log {
        g.row(Row::custom("about", "log").text_value(log).num(200.0));
    }
    g.row(with_switch("System info", "Tessera, Hyprland and Quickshell versions, graphics, displays, distribution".into(), vec![btn(if show_info { "Hide" } else { "Show" })], inc_info, true, info_act));
    if show_info {
        g.row(Row::custom("about", "log").text_value(info).num(140.0));
    }

    // ── Send ──
    let state = s(&r["state"]);
    let url = s(&r["issueUrl"]);
    let wait = n(&r["waitSeconds"]);
    let mut opts = Vec::new();
    let mut acts: Vec<&'static str> = Vec::new();
    if state == "sent" && !url.is_empty() {
        opts.push(btn("Open issue").icon("arrow-square-out"));
        acts.push("bug.openIssue");
    }
    if state == "error" {
        opts.push(btn("Copy report").icon("copy"));
        acts.push("bug.copy");
        opts.push(btn("Open on GitHub").icon("github-logo"));
        acts.push("bug.openGithub");
    }
    opts.push(btn("Send report").icon("paper-plane-tilt").primary().on(state == "sending").off(state == "sending" || wait > 0 || gathering));
    acts.push("bug.send");
    let act = cx.on(move |cx, a| {
        if let Some(w) = acts.get(a.i as usize) {
            cx.ask("about", w, Value::Null);
        }
    });
    p.group("")
        .footer("Sent to Tessera's report server, which files it as an issue on github.com/monishbl/iRice. Your IP address is used only to limit how many reports come from one network and isn't stored with the report.")
        .row(Row::new(send_label(&state, &s(&r["error"]), wait))
            .tone(if state == "error" { 2 } else { 1 })
            .detail(if state == "sent" { url } else { String::new() })
            .buttons(opts, act));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_send_row_says_where_it_is() {
        assert_eq!(send_label("idle", "", 12), "You can send another in 12 s");
        assert_eq!(send_label("error", "No network", 0), "No network");
        assert_eq!(send_label("idle", "", 0), "Ready to send");
    }
}
