//! Focus (pages/focus.qml): the modes, what each lets through, what it
//! changes and when it turns on by itself. The modes are the shell's
//! (services/Focus.qml, focus.json): they come in the feed
//! (`feed.notify.focus`), and every change goes back to it (`setMode`,
//! `addMode`, `removeMode`, `restoreMode`).

use super::super::super::model::{Ctx, Opt, Page, Row};
use super::{feed, fmt_time, place};
use serde_json::{json, Value};

const ICONS: [&str; 12] = ["moon", "briefcase", "user", "bed", "book-open", "barbell", "car", "game-controller", "heart", "music-notes", "code", "coffee"];
const DAYS: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];

/// "22:30" or "7:05": a time Focus takes (/^\d{1,2}:\d{2}$/)
pub fn is_time(t: &str) -> bool {
    let Some((h, m)) = t.split_once(':') else { return false };
    (1..=2).contains(&h.len()) && m.len() == 2 && h.chars().all(|c| c.is_ascii_digit()) && m.chars().all(|c| c.is_ascii_digit())
}

/// The days with `d` switched, in order
pub fn toggle_day(days: &[i64], d: i64) -> Vec<i64> {
    let mut out: Vec<i64> = days.iter().copied().filter(|x| *x != d).collect();
    if out.len() == days.len() {
        out.push(d);
    }
    out.sort();
    out
}

/// "Calendar, Slack" -> ["Calendar", "Slack"]
pub fn app_list(t: &str) -> Vec<String> {
    t.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}

/// Change a mode (Focus.setMode)
fn patch(cx: &mut Ctx, id: &str, p: Value) {
    cx.ask("notify", "setMode", json!([id, p]));
}

/// Change a mode's schedule, keeping the rest of it (Object.assign)
fn sched(cx: &mut Ctx, id: &str, sch: &Value, p: Value) {
    let mut s = if sch.is_object() { sch.clone() } else { json!({}) };
    if let (Some(o), Some(q)) = (s.as_object_mut(), p.as_object()) {
        for (k, v) in q {
            o.insert(k.clone(), v.clone());
        }
    }
    cx.ask("notify", "setMode", json!([id, { "schedule": s }]));
}

fn now_ms() -> i64 {
    crate::util::now_ms()
}

pub fn focus(cx: &mut Ctx, p: &mut Page) {
    let f = feed(cx)["focus"].clone();
    let modes: Vec<Value> = f["modes"].as_array().cloned().unwrap_or_default();
    let active = f["active"].as_str().unwrap_or("").to_string();
    let status = f["statusText"].as_str().unwrap_or("").to_string();
    let h24 = cx.b("appearance.clock24");

    // A mode just added is the one shown (the shell said its id)
    let added = f["added"].as_str().unwrap_or("").to_string();
    if !added.is_empty() && cx.local_s("focus.addedSeen") != added {
        cx.local_set("focus.addedSeen", added.clone());
        cx.local_set("focus.selected", added);
    }
    let first = modes.first().and_then(|m| m["id"].as_str()).unwrap_or("").to_string();
    let mut selected = cx.local_s("focus.selected");
    if selected.is_empty() || !modes.iter().any(|m| m["id"].as_str() == Some(selected.as_str())) {
        selected = first;
    }
    let mode = modes.iter().find(|m| m["id"].as_str() == Some(selected.as_str())).cloned();

    // The modes, then Add a mode
    let n = modes.len();
    let mut rows = Vec::new();
    for (i, m) in modes.iter().enumerate() {
        let id = m["id"].as_str().unwrap_or("").to_string();
        let sch = &m["schedule"];
        let when = if sch["enabled"].as_bool().unwrap_or(false) {
            format!("Scheduled {}–{}", fmt_time(sch["from"].as_str().unwrap_or(""), h24), fmt_time(sch["to"].as_str().unwrap_or(""), h24))
        } else {
            "Manual".into()
        };
        let detail = if active == id { format!("{status} · {when}") } else { when };
        let pick = {
            let id = id.clone();
            cx.on(move |cx, _| cx.local_set("focus.selected", id.clone()))
        };
        let mut r = Row::custom("notify", "item").id(pick).icon(m["icon"].as_str().unwrap_or("circle"));
        r.label = m["name"].as_str().unwrap_or("").to_string();
        r.detail = detail;
        r.on = selected == id;
        r.index = place(i == 0, false);
        r.from = 44.0;
        r.to = 56.0;
        let _ = n;
        rows.push(r);
    }
    let add = cx.on(|cx, a| {
        let t = a.s.trim();
        if a.i == 1 && !t.is_empty() {
            cx.ask("notify", "addMode", json!([t]));
        }
    });
    rows.push(Row::new("Add a mode").ctl("notify", "entry", add).placeholder("Name, then Enter").width(200.0));
    let g = p.group("");
    g.footer("Click a mode to edit it. Turn modes on from Quick Settings or the menu bar.");
    for r in rows {
        g.row(r);
    }

    // Undo for a deleted mode (8 s)
    let removed = cx.local.get("focus.removed").cloned().unwrap_or(Value::Null);
    let at = cx.local.get("focus.removedAt").and_then(Value::as_i64).unwrap_or(0);
    if removed.is_object() && now_ms() - at < 8000 {
        let r2 = removed.clone();
        let undo = cx.on(move |cx, _| {
            cx.ask("notify", "restoreMode", json!([r2.clone()]));
            cx.local_set("focus.selected", r2["id"].as_str().unwrap_or("").to_string());
            cx.local.remove("focus.removed");
        });
        p.group("").row(Row::new(format!("Deleted “{}”", removed["name"].as_str().unwrap_or(""))).button("Undo", undo).minor());
    }

    let Some(mode) = mode else { return };
    let mid = mode["id"].as_str().unwrap_or("").to_string();
    let sch = mode["schedule"].clone();

    let name = {
        let id = mid.clone();
        cx.on(move |cx, a| {
            let t = a.s.trim();
            if a.i == 1 && !t.is_empty() {
                patch(cx, &id, json!({ "name": t }));
            }
        })
    };
    let icon_now = mode["icon"].as_str().unwrap_or("").to_string();
    let icons: Vec<Opt> = ICONS.iter().map(|i| Opt::new(*i).icon(i).on(icon_now == *i)).collect();
    let icon = {
        let id = mid.clone();
        cx.on(move |cx, a| {
            if let Some(i) = ICONS.get(a.i.max(0) as usize) {
                patch(cx, &id, json!({ "icon": i }));
            }
        })
    };
    let apps = {
        let id = mid.clone();
        cx.on(move |cx, a| {
            if a.i == 1 {
                patch(cx, &id, json!({ "allowApps": app_list(&a.s) }));
            }
        })
    };
    let wall = {
        let id = mid.clone();
        cx.on(move |cx, a| {
            if a.i == 1 {
                let home = std::env::var("HOME").unwrap_or_default();
                let t = a.s.trim();
                let w = match t.strip_prefix('~') {
                    Some(rest) => format!("{home}{rest}"),
                    None => t.to_string(),
                };
                patch(cx, &id, json!({ "wallpaper": w }));
            }
        })
    };
    let looks = ["", "light", "dark"];
    let look = {
        let id = mid.clone();
        cx.on(move |cx, a| {
            if let Some(v) = looks.get(a.i.max(0) as usize) {
                patch(cx, &id, json!({ "appearance": v }));
            }
        })
    };
    let scheduled = sch["enabled"].as_bool().unwrap_or(false);
    let st = {
        let (id, s) = (mid.clone(), sch.clone());
        cx.on(move |cx, _| sched(cx, &id, &s, json!({ "enabled": !scheduled })))
    };
    let time_at = |cx: &mut Ctx, key: &'static str| {
        let (id, s) = (mid.clone(), sch.clone());
        cx.on(move |cx, a| {
            let t = a.s.trim();
            if a.i == 1 && is_time(t) && s[key].as_str() != Some(t) {
                let mut o = serde_json::Map::new();
                o.insert(key.to_string(), Value::from(t));
                sched(cx, &id, &s, Value::Object(o));
            }
        })
    };
    let from = time_at(cx, "from");
    let to = time_at(cx, "to");
    let days: Vec<i64> = sch["days"].as_array().map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();
    let day_chips: Vec<Opt> = DAYS.iter().enumerate().map(|(i, d)| Opt::new(*d).on(days.contains(&(i as i64)))).collect();
    let day = {
        let (id, s, days) = (mid.clone(), sch.clone(), days.clone());
        cx.on(move |cx, a| sched(cx, &id, &s, json!({ "days": toggle_day(&days, a.i as i64) })))
    };
    let delete = {
        let m = mode.clone();
        cx.on(move |cx, _| {
            cx.local_set("focus.removed", m.clone());
            cx.local_set("focus.removedAt", now_ms());
            cx.local.remove("focus.selected");
            cx.ask("notify", "removeMode", json!([m["id"].clone()]));
            // (the Undo goes after 8 s)
            let key = format!("focus.undoTick.{}", now_ms());
            cx.spawn(&key, || {
                std::thread::sleep(std::time::Duration::from_millis(8100));
                Value::Null
            });
        })
    };
    let appearance = mode["appearance"].as_str().unwrap_or("").to_string();
    let allow = mode["allowApps"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default();

    let mut icon_row = Row::new("Icon").ctl("notify", "icons", icon);
    icon_row.opts = icons;
    let mut days_row = Row::new("Days").visible(scheduled).ctl("notify", "days", day);
    days_row.opts = day_chips;
    p.group(mode["name"].as_str().unwrap_or(""))
        .row(Row::new("Name").field(mode["name"].as_str().unwrap_or(""), "", name).width(200.0))
        .row(icon_row)
        .row(Row::new("Allowed apps").detail("Their banners still show. Separate names with commas, e.g. Calendar, Slack.").field(allow, "None", apps).width(220.0))
        .row(Row::new("Wallpaper").detail("Shown while the mode is on; yours comes back afterwards.").field(mode["wallpaper"].as_str().unwrap_or(""), "No change", wall).width(220.0))
        .row(Row::new("Appearance").segmented(&["No change", "Light", "Dark"], super::idx(&looks, &appearance), look).width(240.0))
        .row(Row::new("Schedule").detail("Turns on and off by itself at these times.").switch(scheduled, st))
        .row(Row::new("From").visible(scheduled).detail("24-hour time, e.g. 22:30").field(sch["from"].as_str().unwrap_or(""), "", from).width(90.0))
        .row(Row::new("To").visible(scheduled).field(sch["to"].as_str().unwrap_or(""), "", to).width(90.0))
        .row(days_row)
        .row(Row::new("Delete this mode").visible(mid != "dnd").button("Delete", delete).danger());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_days_and_apps() {
        assert!(is_time("22:30") && is_time("7:05"));
        assert!(!is_time("7:5") && !is_time("123:00") && !is_time("ab:cd") && !is_time("22"));
        assert_eq!(toggle_day(&[1, 3], 2), vec![1, 2, 3]);
        assert_eq!(toggle_day(&[1, 2, 3], 2), vec![1, 3]);
        assert_eq!(app_list(" Calendar, ,Slack "), vec!["Calendar".to_string(), "Slack".to_string()]);
    }
}
