//! Software Update (pages/update.qml): one Check for updates, one plan, one
//! Update for Tessera and the system together. The updater is the shell's
//! (SoftwareUpdate, TesseraUpdate, Updates, BootSnapshots: pacman and the
//! root helper run there, never here): its state comes in the feed
//! (`su`, `tu`, `up`, `bs`), and every button asks it (`cx.ask("about", …)`).
//! Update notes (UpdateNotes.qml), the package lists, the steps, the wavy
//! progress (WavyProgress.qml) and the output (LogView.qml) are
//! settings-about.slint's views.

use super::super::super::model::{btn, Ctx, Opt, Page, Row};
use super::{b, feed, n, plural, s, strings};
use serde_json::{json, Value};

/// The conflicts' sections, in order: (kind, title, footer)
const KINDS: [(&str, &str, &str); 5] = [
    ("setting", "Settings with new defaults", "You kept these at their defaults. Your current value stays unless you choose the new one."),
    ("preset", "Your glass material", "Keeping your look saves it as a Custom material."),
    ("shortcut", "Shortcut clashes", "The update brings a shortcut on keys you already use."),
    ("removed", "Settings that change name or go away", "You changed these, and the update renames or no longer has them."),
    ("code", "Files you changed", "You changed these Tessera files, and the update changes them too. Keep mine drops the update's change to that file; Use the update's sets yours aside in the backup folder; Merge both is offered when the two fit together."),
];

/// A conflict's button label (update.qml's `short`)
pub fn short(c: &Value, ch: &str) -> String {
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

/// What an update will touch, as the Update row says it
pub fn plan_detail(plan: &Value) -> String {
    let parts = [
        if b(&plan["system"]) || b(&plan["aur"]) { "the system" } else { "" },
        if n(&plan["install"]) > 0 || n(&plan["remove"]) > 0 { "Tessera's packages" } else { "" },
        if b(&plan["setup"]) && !b(&plan["tessera"]) { "Tessera's system helper (for the firewall fix, Quick Share and more)" } else { "" },
        if b(&plan["tessera"]) { "Tessera" } else if n(&plan["rebuild"]) > 0 { "the glass plugin (rebuilt for the new Hyprland)" } else { "" },
    ];
    let j = parts.iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(", ");
    let mut c = j.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Update notes (UpdateNotes.qml) as a custom row: each changelog entry
/// (title, date, its Markdown as plain text), the commits as pointers when
/// there's none; the newest only until "Show all"
fn notes(cx: &mut Ctx, key: &str, changelog: &Value, fallback: &Value) -> Row {
    let all = cx.local_b(key);
    let entries: Vec<Opt> = changelog.as_array().map(|a| a.iter().map(|e| Opt::new(s(&e["title"])).detail(s(&e["date"])).id(plain(&s(&e["body"])))).collect()).unwrap_or_default();
    let total = entries.len();
    let shown: Vec<Opt> = if all { entries } else { entries.into_iter().take(1).collect() };
    let fb = if total == 0 { strings(fallback).iter().map(|l| format!("•  {l}")).collect::<Vec<_>>().join("\n") } else { String::new() };
    let k = key.to_string();
    let toggle = cx.on(move |cx, _| {
        let v = cx.local_b(&k);
        cx.local_set(&k, !v);
    });
    let mut r = Row::custom("about", "notes").id(toggle).data(shown).text_value(fb).num(total as f32);
    r.on = all;
    r
}

/// Markdown as the notes show it: bold and code marks dropped, links as
/// their text, list items as bullets
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

/// One package line (update.qml's PkgLine): name, a badge, versions or a reason
fn pkg(name: &str, from: &str, to: &str, badge: &str, dim: bool) -> Opt {
    let right = if from.is_empty() { to.to_string() } else { format!("{from}  →  {to}") };
    // (versions in mono, they line up; a reason in the normal font)
    let mono = !from.is_empty() || (to.chars().any(|c| c.is_ascii_digit()) && !to.contains(' '));
    Opt::new(name).detail(right).id(badge).off(dim).on(mono).style(1)
}
fn heading(text: String) -> Opt {
    Opt::new(text).style(0)
}
fn more(total: usize, open: bool, which: f32) -> Opt {
    Opt::new(if open { "Show fewer".to_string() } else { format!("and {} more", total - 8) }).style(2).num(which)
}

pub fn page(cx: &mut Ctx, p: &mut Page) {
    let f = feed(cx);
    let (su, tu, up, bs) = (&f["su"], &f["tu"], &f["up"], &f["bs"]);
    let running = b(&su["running"]);
    let checking = b(&su["checking"]);
    let checked = b(&su["checked"]);
    let anything = b(&su["anything"]);
    let report = &tu["report"];
    let job = &up["job"];
    let over = !job.is_null() && s(&job["state"]) != "running" && !running;
    let show_plan = checked && !running && !over && anything;
    let available = b(&tu["available"]);
    let plan = &su["plan"];

    // ── Status, and Check for updates ──
    let footer = if !b(&tu["allowed"]) {
        "Updates are off in the dev sandbox (they would change this copy of Tessera); the system part is simulated.".to_string()
    } else if !tu["result"].is_null() {
        let backup = s(&tu["result"]["backup"]);
        format!("Tessera was updated. Your settings and shortcuts were backed up{}.", if backup.is_empty() { String::new() } else { format!(" to {backup}") })
    } else {
        "Nothing is checked on its own. Checking downloads nothing but the package lists.".into()
    };
    let label = if running {
        "Updating…"
    } else if checking {
        "Checking for updates…"
    } else if !checked {
        "Software Update"
    } else if anything {
        "Updates are ready"
    } else {
        "Everything is up to date"
    };
    let hypr = s(&su["hyprNow"]);
    let detail = if !checked {
        "Tessera and your system (pacman, and the AUR if you use yay or paru), together.".to_string()
    } else if !report.is_null() {
        format!(
            "Tessera {}{}{}",
            s(&report["current"]),
            if available { format!(" → {}", s(&report["latest"])) } else { String::new() },
            if hypr.is_empty() { String::new() } else { format!(" · Hyprland {hypr}") }
        )
    } else {
        String::new()
    };
    let check = cx.on(|cx, _| cx.ask("about", "su.check", Value::Null));
    let reboot = cx.on(|cx, _| cx.ask("about", "reboot", Value::Null));
    p.group("")
        .footer(footer)
        .row(if running {
            Row::new(label).detail(detail)
        } else {
            Row::new(label).detail(detail).buttons(vec![btn(if checked { "Check again" } else { "Check for updates" }).icon("arrow-clockwise").on(checking)], check)
        })
        .row(Row::new("Restart to finish the last update").visible(b(&su["restartPending"]) && !running).detail("The kernel or system libraries changed.").buttons(vec![btn("Restart").icon("arrow-clockwise")], reboot));

    // Errors say what happened and what to do
    let err = s(&tu["error"]);
    let again = cx.on(|cx, _| cx.ask("about", "su.check", Value::Null));
    p.group("").row(Row::new("Tessera's part can't go ahead").visible(!err.is_empty() && !running).detail(err).button("Try again", again));

    // ── The plan: Tessera ──
    let has_notes = report["changelog"].as_array().is_some_and(|a| !a.is_empty()) || report["notes"].as_array().is_some_and(|a| !a.is_empty());
    if show_plan && available && has_notes {
        let r = notes(cx, "about.update.notesAll", &report["changelog"], &report["notes"]);
        p.group("What's new in Tessera").row(r);
    }
    let conflicts: Vec<Value> = tu["conflicts"].as_array().cloned().unwrap_or_default();
    if show_plan && available {
        for (kind, title, foot) in KINDS {
            let mine: Vec<&Value> = conflicts.iter().filter(|c| c["kind"] == kind).collect();
            if mine.is_empty() {
                continue;
            }
            let mut rows = Vec::new();
            for c in mine {
                let file = kind == "code";
                let detail = if file && !s(&c["mine"]).is_empty() { format!("{}\n{} · {}", s(&c["detail"]), s(&c["mine"]), s(&c["theirs"])) } else { s(&c["detail"]) };
                let choices = strings(&c["choices"]);
                let labels: Vec<String> = choices.iter().map(|ch| short(c, ch)).collect();
                let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
                // (-1 until chosen: nothing is picked for you)
                let chosen = s(&tu["choices"][s(&c["id"])]);
                let cur = choices.iter().position(|x| *x == chosen).map_or(-1, |i| i as i32);
                let id = s(&c["id"]);
                let ch2 = choices.clone();
                let pick = cx.on(move |cx, a| {
                    if let Some(ch) = ch2.get(a.i as usize) {
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

    // ── The plan: Hyprland newer than this Tessera supports ──
    if show_plan && b(&su["hyprTooNew"]) {
        let hold = n(&su["hold"]);
        let footer = match hold {
            1 => format!("Held for this update only: {}. Asked again next time.", strings(&su["heldHypr"]).join(", ")),
            0 => "If the glass plugin can't be built for it, glass falls back to frosted until Tessera catches up.".into(),
            _ => format!("This Tessera is tested with Hyprland {}.", s(&su["hyprMax"])),
        };
        let keep = format!("Keep {}", if hypr.is_empty() { "the current one".to_string() } else { hypr.clone() });
        let pick = cx.on(|cx, a| cx.ask("about", "su.hold", json!([a.i])));
        p.group("Hyprland").footer(footer).row(
            Row::new(format!("Hyprland {} isn't supported by this Tessera yet", s(&su["hyprShort"])))
                .detail("Choose what to do for this update.")
                .segmented(&[keep.as_str(), "Update anyway"], match hold { 1 => 0, 0 => 1, _ => -1 }, pick)
                .width(300.0),
        );
    }

    // ── The plan: packages affected ──
    let missing = strings(&tu["missing"]);
    let dropped = strings(&tu["dropped"]);
    let aur_list = up["aurList"].as_array().cloned().unwrap_or_default();
    if show_plan && (b(&plan["system"]) || b(&plan["aur"]) || !missing.is_empty() || !dropped.is_empty()) {
        let held = strings(&su["held"]);
        let held_hypr = strings(&su["heldHypr"]);
        let include_aur = b(&su["includeAur"]);
        let remove_dropped = b(&su["removeDropped"]);
        let mut sys: Vec<(Value, bool)> = up["list"].as_array().cloned().unwrap_or_default().into_iter().map(|x| (x, false)).collect();
        sys.extend(aur_list.iter().cloned().map(|x| (x, true)));
        let open = [cx.local_b("about.update.more0"), cx.local_b("about.update.more1"), cx.local_b("about.update.more2")];
        let mut lines: Vec<Opt> = Vec::new();
        if !sys.is_empty() {
            let count = n(&plan["sysCount"]) + if include_aur { n(&plan["aurCount"]) } else { 0 };
            lines.push(heading(format!("System · {}{}", plural(count, "update"), if held.is_empty() { String::new() } else { format!(" ({} held)", held.len()) })));
            for (x, aur) in sys.iter().take(if open[0] { usize::MAX } else { 8 }) {
                let name = s(&x["name"]);
                let is_held = held.contains(&name);
                let badge = if is_held {
                    if held_hypr.contains(&name) { "held for Tessera" } else { "skipped · you went back from it" }
                } else if *aur {
                    if include_aur { "AUR" } else { "AUR · not included" }
                } else {
                    ""
                };
                lines.push(pkg(&name, &s(&x["from"]), &s(&x["to"]), badge, is_held || (*aur && !include_aur)));
            }
            if sys.len() > 8 {
                lines.push(more(sys.len(), open[0], 0.0));
            }
        }
        if !missing.is_empty() {
            lines.push(heading("Tessera adds".into()));
            for m in missing.iter().take(if open[1] { usize::MAX } else { 8 }) {
                lines.push(pkg(m, "", &s(&tu["reasons"][m]), "", false));
            }
            if missing.len() > 8 {
                lines.push(more(missing.len(), open[1], 1.0));
            }
        }
        if !dropped.is_empty() {
            lines.push(heading("Tessera drops".into()));
            for d in dropped.iter().take(if open[2] { usize::MAX } else { 8 }) {
                lines.push(pkg(d, "", "", if remove_dropped { "removed" } else { "kept" }, !remove_dropped));
            }
            if dropped.len() > 8 {
                lines.push(more(dropped.len(), open[2], 2.0));
            }
        }
        let toggle_more = cx.on(|cx, a| {
            let k = format!("about.update.more{}", a.f as i32);
            let v = cx.local_b(&k);
            cx.local_set(&k, !v);
        });
        let snap = s(&su["snapshot"]);
        let aur_t = cx.on(move |cx, _| cx.ask("about", "su.includeAur", json!([!include_aur])));
        let drop_t = cx.on(move |cx, _| cx.ask("about", "su.removeDropped", json!([!remove_dropped])));
        p.group("Packages affected")
            .footer(if !snap.is_empty() {
                format!("A snapshot is taken first ({snap}). Afterwards you can go back to the versions you have now, below.")
            } else {
                "Afterwards you can go back to the versions you have now, below: pacman keeps them in its cache. Tessera backs up your settings.".into()
            })
            .row(Row::custom("about", "pkgs").id(toggle_more).data(lines))
            .row(Row::new("Include AUR updates")
                .visible(!aur_list.is_empty())
                .detail("AUR packages are built from scripts anyone can submit, and they're built without showing you what changed. Include them only if you trust these packages.")
                .switch(include_aur, aur_t))
            .row(Row::new("Remove packages Tessera no longer uses").visible(!dropped.is_empty()).detail("Turn off if you use them yourself.").switch(remove_dropped, drop_t));
    }

    // ── Update ──
    if show_plan {
        let undecided = n(&su["undecided"]);
        let go = cx.on(|cx, _| cx.ask("about", "su.update", Value::Null));
        p.group("")
            .footer("Your system is updated first, then Tessera, which restarts the shell; your apps keep running. Your password is asked once, at the start.")
            .row(Row::new(if undecided > 0 { format!("{} left to decide", plural(undecided, "thing")) } else { "Ready to update".into() })
                .detail(if undecided > 0 { "Choose for each item above; nothing changes until you update.".to_string() } else { plan_detail(plan) })
                .buttons(vec![btn("Update").icon("arrow-circle-up").primary().off(!b(&su["canUpdate"]))], go));
    }

    // ── Progress (while it runs, and how it ended) ──
    if running || over {
        let interrupted = b(&up["interrupted"]);
        let state = s(&job["state"]);
        let label = if interrupted {
            "The last update was interrupted".to_string()
        } else if running {
            if b(&tu["applying"]) { "Updating Tessera".into() } else if !s(&job["text"]).is_empty() { s(&job["text"]) } else { "Updating".into() }
        } else if state == "done" {
            if s(&job["kind"]) == "undo" { "Back to the earlier versions".into() } else if n(&job["upgraded"]) > 0 { format!("{} updated", plural(n(&job["upgraded"]), "package")) } else { "Updated".into() }
        } else {
            s(&job["text"])
        };
        let detail = if interrupted {
            "Update again to finish; pacman picks up where it stopped.".to_string()
        } else if running {
            if s(&job["phase"]) == "auth" { "Enter your password in the dialog.".into() } else { s(&job["package"]) }
        } else if state == "error" {
            s(&job["error"])
        } else if b(&job["restart"]) {
            "Restart to finish: the kernel or system libraries changed.".into()
        } else {
            String::new()
        };
        let mut buttons = Vec::new();
        let mut acts: Vec<&'static str> = Vec::new();
        if b(&up["cancellable"]) {
            buttons.push(btn("Cancel"));
            acts.push("su.cancel");
        }
        if over && state == "done" && b(&job["restart"]) {
            buttons.push(btn("Restart").icon("arrow-clockwise"));
            acts.push("reboot");
        }
        if over {
            buttons.push(btn("Done"));
            acts.push("up.dismiss");
        }
        let act = cx.on(move |cx, a| {
            if let Some(w) = acts.get(a.i as usize) {
                cx.ask("about", w, Value::Null);
            }
        });
        let steps: Vec<Opt> = su["steps"].as_array().map(|a| a.iter().map(|x| Opt::new(s(&x["name"])).on(b(&x["now"]))).collect()).unwrap_or_default();
        let progress = up["progress"].as_f64().unwrap_or(0.0) as f32;
        let details = b(&up["showDetails"]);
        let mut dbtn = Vec::new();
        if details {
            dbtn.push(btn("Copy").icon("copy"));
        }
        dbtn.push(btn(if details { "Hide details" } else { "Show details" }).icon(if details { "caret-up" } else { "terminal-window" }));
        let dact = cx.on(move |cx, a| {
            if details && a.i == 0 {
                cx.ask("about", "up.copyLog", Value::Null);
            } else {
                cx.ask("about", "up.showDetails", json!([!details]));
            }
        });
        let log = s(&up["log"]);
        let has_job = !job.is_null();
        let g = p.group("Update");
        g.footer(if b(&up["fake"]) {
            "In the dev sandbox the update is simulated; nothing is installed."
        } else if running {
            "Keeps going if you close Settings; the island shows how far it is."
        } else {
            ""
        });
        g.row(if buttons.is_empty() { Row::new(label).detail(detail) } else { Row::new(label).detail(detail).buttons(buttons, act) });
        if steps.len() > 1 {
            g.row(Row::custom("about", "steps").data(steps));
        }
        if b(&up["running"]) {
            g.row(Row::custom("about", "progress").num(progress.max(0.02)).text_value(format!("{}%", (progress.max(0.0) * 100.0).round())));
        }
        if has_job {
            g.row(Row::new("Details").detail(format!("The full output of pacman{}", if b(&job["aur"]) { " and the AUR helper" } else { "" })).buttons(dbtn, dact));
            if details {
                let mut r = Row::custom("about", "log").text_value(if log.is_empty() { "Waiting for output…".to_string() } else { log }).num(240.0);
                r.on = true; // (follow the end)
                g.row(r);
            }
        }
    }

    // ── Going back ──
    let from_snapshot = !s(&bs["booted"]).is_empty();
    let undo_count = n(&su["undoCount"]);
    let skipped_all = su["skippedAll"].as_array().cloned().unwrap_or_default();
    let can_menu = b(&bs["canMenu"]);
    if !running && (from_snapshot || undo_count > 0 || !skipped_all.is_empty() || can_menu) {
        let mut rows = Vec::new();
        let installing = b(&f["needsInstalling"]);
        let updates_running = b(&up["running"]);
        if from_snapshot {
            let assistant = b(&bs["assistant"]);
            let act = cx.on(move |cx, _| {
                if assistant {
                    cx.ask("about", "bs.openAssistant", Value::Null)
                } else {
                    cx.ask("about", "needs.install", json!(["btrfs-assistant"]))
                }
            });
            rows.push(
                Row::new(format!("Started from snapshot {}", s(&bs["booted"])))
                    .tone(4)
                    .detail(if assistant {
                        "Your system is as it was then, until you restart. To keep it, restore this snapshot in Btrfs Assistant (Snapper → Restore), then restart."
                    } else {
                        "Your system is as it was then, until you restart. To keep it, restore this snapshot with Btrfs Assistant: install it, then choose Snapper → Restore."
                    })
                    .button(if assistant { "Open Btrfs Assistant" } else if installing { "Installing…" } else { "Install Btrfs Assistant" }, act)
                    .busy(!assistant && installing)
                    .enabled(assistant || !updates_running),
            );
        }
        if undo_count > 0 && !from_snapshot {
            let asking = cx.local_b("about.update.asking");
            let missing = strings(&su["undoMissing"]);
            let pk = strings(&su["undoPackages"]);
            let detail = if !missing.is_empty() {
                format!("Some earlier versions aren't on this computer any more: {}{}", missing.iter().take(4).cloned().collect::<Vec<_>>().join(", "), if missing.len() > 4 { "…" } else { "" })
            } else if asking {
                format!("{} go back to the version before {}. Your files and settings stay as they are. It asks for your password.", plural(undo_count, "package"), s(&su["undoDay"]))
            } else {
                format!("The update on {} changed {}. {}{}", s(&su["undoLong"]), plural(undo_count, "package"), pk.iter().take(3).cloned().collect::<Vec<_>>().join(", "), if undo_count > 3 { "…" } else { "" })
            };
            let mut bs2 = Vec::new();
            if asking {
                bs2.push(btn("Cancel"));
            }
            bs2.push(btn("Go Back").icon(if asking { "" } else { "clock-counter-clockwise" }).off(!b(&su["canGoBack"])));
            let act = cx.on(move |cx, a| {
                if asking && a.i == 0 {
                    cx.local_set("about.update.asking", false);
                } else if !asking {
                    cx.local_set("about.update.asking", true);
                } else {
                    cx.local_set("about.update.asking", false);
                    cx.ask("about", "su.goBack", Value::Null);
                }
            });
            rows.push(
                Row::new(if asking { "Go back to the earlier versions?" } else { "Go back to before the last update" })
                    .tone(if missing.is_empty() { 0 } else { 1 })
                    .detail(detail)
                    .buttons(bs2, act),
            );
        }
        let include = strings(&su["includeSkipped"]);
        for x in &skipped_all {
            let name = s(&x["name"]);
            let on = include.contains(&name);
            let nm = name.clone();
            let t = cx.on(move |cx, _| cx.ask("about", "su.includeSkip", json!([nm, !on])));
            rows.push(
                Row::new(format!("{} {}", name, s(&x["to"])))
                    .detail(if on { "Included in this update" } else { "Skipped: you went back from this version. A newer one is installed as usual." })
                    .switch(on, t),
            );
        }
        if can_menu && !from_snapshot {
            let grub = b(&bs["grubBtrfs"]);
            let menu_on = b(&bs["menuOn"]);
            let err = s(&bs["error"]);
            let detail = if !err.is_empty() {
                err.clone()
            } else if !grub {
                "To start from a snapshot when the desktop won't load, GRUB can list them (grub-btrfs). It asks for your password once.".into()
            } else if menu_on {
                "GRUB lists your snapshots under “snapshots”: start from one to see your system as it was.".into()
            } else {
                "GRUB can list your snapshots, to start from one when the desktop won't load.".into()
            };
            let r = Row::new("Snapshots in the boot menu").detail(detail).tone(if err.is_empty() { 0 } else { 2 });
            if grub {
                let t = cx.on(move |cx, _| cx.ask("about", "bs.setMenu", json!([!menu_on])));
                rows.push(r.switch(menu_on, t).enabled(!b(&bs["busy"])));
            } else {
                let i = cx.on(|cx, _| cx.ask("about", "needs.install", json!(["grub-btrfs"])));
                rows.push(r.button(if installing { "Installing…" } else { "Install" }, i).busy(installing).enabled(!updates_running));
            }
        }
        let g = p.group("Going back");
        if !from_snapshot {
            g.footer("If the desktop doesn't start after an update, Tessera offers to go back on its own. From a text console (Ctrl+Alt+F3), run: tessera undo-update");
        }
        for r in rows {
            g.row(r);
        }
    }

    // ── After a Tessera update: what's new, until Done ──
    let result = &tu["result"];
    let result_notes = result["changelog"].as_array().is_some_and(|a| !a.is_empty()) || result["notes"].as_array().is_some_and(|a| !a.is_empty());
    if !result.is_null() && !running && result_notes {
        let r = notes(cx, "about.update.resultAll", &result["changelog"], &result["notes"]);
        let done = cx.on(|cx, _| cx.ask("about", "tu.dismissResult", Value::Null));
        p.group("What's new").row(r).row(Row::new(format!("Updated to {}", s(&result["now"]))).button("Done", done));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_buttons_say_what_they_do() {
        let file = json!({"kind": "code", "change": "deleted-mine"});
        assert_eq!(short(&file, "mine"), "Keep it deleted");
        assert_eq!(short(&file, "theirs"), "Bring it back");
        assert_eq!(short(&json!({"kind": "code"}), "both"), "Merge both");
        assert_eq!(short(&json!({"kind": "setting", "labels": {"new": "Use the new one"}}), "new"), "Use the new one");
        assert_eq!(short(&json!({"kind": "setting"}), "keep"), "keep");
    }

    #[test]
    fn the_plan_in_words() {
        assert_eq!(plan_detail(&json!({"system": true, "tessera": true})), "The system, Tessera");
        assert_eq!(plan_detail(&json!({"rebuild": 1})), "The glass plugin (rebuilt for the new Hyprland)");
        assert_eq!(plan_detail(&json!({})), "");
    }

    #[test]
    fn notes_as_plain_text() {
        assert_eq!(plain("**New:** see [the docs](https://x.y) and `cfg`\n- one\n  - two"), "New: see the docs and cfg\n•  one\n  •  two");
    }

    #[test]
    fn package_lines() {
        let l = pkg("hyprland", "0.55", "0.56", "AUR", false);
        assert_eq!((l.detail.as_str(), l.on), ("0.55  →  0.56", true));
        assert!(!pkg("foot", "", "the terminal", "", false).on);
    }
}
