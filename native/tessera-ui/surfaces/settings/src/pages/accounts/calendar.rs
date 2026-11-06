//! Calendar (pages/calendar.qml): which calendars show (from your accounts,
//! and subscriptions to iCalendar links), where new to-dos go, and reminder
//! alerts. The calendars and lists come from the shell (feed.accounts.google,
//! .cloud, .calendar); shown or hidden and the to-do list are config.json's.

use super::super::super::model::{btn, Ctx, Opt, Page, Row};
use super::{action, arr, feed, field, s};
use serde_json::{json, Value};

/// A Google calendar is shown unless it's hidden ("<account>|<id>", or a
/// bare id from before several accounts: Google.shown)
pub fn google_shown(hidden: &[String], account: &str, id: &str) -> bool {
    let r = format!("{account}|{id}");
    !hidden.iter().any(|h| *h == r || h == id)
}

/// Showing or hiding a Google calendar: the hidden list after
pub fn google_toggle(hidden: &[String], account: &str, id: &str, shown: bool) -> Vec<String> {
    let r = format!("{account}|{id}");
    let mut out: Vec<String> = hidden.iter().filter(|h| **h != r && *h != id).cloned().collect();
    if shown {
        out.push(r);
    }
    out
}

/// The to-do list choice's value for the setting (google.taskListId): a
/// "cloud|…" one as it is; a Google one as "<account>|<id>" (a bare id: the
/// first account's list with it); "" this computer
pub fn task_value(setting: &str, lists: &[(String, String)]) -> String {
    if setting.starts_with("cloud|") {
        return setting.to_string();
    }
    if setting.is_empty() {
        return String::new();
    }
    let hit = match setting.split_once('|') {
        Some((a, id)) => lists.iter().find(|(la, lid)| la == a && lid == id),
        None => lists.iter().find(|(_, lid)| lid == setting),
    };
    hit.map(|(a, id)| format!("{a}|{id}")).unwrap_or_default()
}

/// A link Calendar can subscribe to: https, webcal, a file
pub fn link_ok(url: &str) -> bool {
    ["http:", "https:", "webcal:", "/", "file:"].iter().any(|p| url.starts_with(p))
}

pub fn page(cx: &mut Ctx, p: &mut Page) {
    let f = feed(cx);
    let g = &f["google"];
    let c = &f["cloud"];
    let connected = g["connected"].as_bool().unwrap_or(false);
    let g_accounts = arr(&g["accounts"]);
    let c_accounts = arr(&c["accounts"]);
    let cloud = !c_accounts.is_empty();
    let microsoft = c_accounts.iter().any(|a| s(&a["provider"]) == "microsoft");
    let identity = |list: &[Value], id: &str| list.iter().find(|a| s(&a["id"]) == id).map(|a| s(&a["identity"])).unwrap_or_default();
    let provider_of = |account: &str| match c_accounts.iter().find(|a| s(&a["id"]) == account).map(|a| s(&a["provider"])).as_deref() {
        Some("icloud") => "iCloud",
        Some("microsoft") => "Outlook",
        _ => "",
    };

    // ── From your accounts: shown or hidden, one switch per calendar ──
    if connected || cloud {
        let g_cals = arr(&g["calendars"]);
        let c_cals = arr(&c["calendars"]);
        let mut rows = Vec::new();
        let syncing = g["busy"].as_bool().unwrap_or(false) || c["busy"].as_bool().unwrap_or(false);
        rows.push(Row::new("No calendars yet").visible(g_cals.is_empty() && c_cals.is_empty()).detail(if syncing { "Syncing…" } else { "They appear after the first sync" }).tone(1));
        if connected {
            let hidden = cx.list("google.hiddenCalendars");
            for cal in &g_cals {
                let (account, id) = (s(&cal["account"]), s(&cal["id"]));
                let shown = google_shown(&hidden, &account, &id);
                let detail: Vec<String> = ["Google".to_string(), if g_accounts.len() > 1 { identity(&g_accounts, &account) } else { String::new() }, if cal["writable"].as_bool().unwrap_or(false) { String::new() } else { "View only".into() }]
                    .into_iter()
                    .filter(|x| !x.is_empty())
                    .collect();
                let t = cx.on(move |cx, _| {
                    let next = google_toggle(&cx.list("google.hiddenCalendars"), &account, &id, shown);
                    cx.set("google.hiddenCalendars", Value::Array(next.into_iter().map(Value::from).collect()));
                });
                let title = format!("{}{}", s(&cal["title"]), if cal["primary"].as_bool().unwrap_or(false) { " (primary)" } else { "" });
                rows.push(Row::new(title).detail(detail.join(" · ")).switch(shown, t));
            }
        }
        let hidden = cx.list("cloud.hiddenCalendars");
        for cal in c_cals.iter().filter(|c| !provider_of(&s(&c["account"])).is_empty()) {
            let (account, id) = (s(&cal["account"]), s(&cal["id"]));
            let r = format!("{account}|{id}");
            let shown = !hidden.contains(&r);
            let detail: Vec<String> = [provider_of(&account).to_string(), if c_accounts.len() > 1 { identity(&c_accounts, &account) } else { String::new() }, if cal["writable"].as_bool().unwrap_or(false) { String::new() } else { "View only".into() }]
                .into_iter()
                .filter(|x| !x.is_empty())
                .collect();
            let t = cx.on(move |cx, _| {
                let mut next: Vec<String> = cx.list("cloud.hiddenCalendars").into_iter().filter(|x| *x != r).collect();
                if shown {
                    next.push(r.clone());
                }
                cx.set("cloud.hiddenCalendars", Value::Array(next.into_iter().map(Value::from).collect()));
            });
            rows.push(Row::new(s(&cal["title"])).detail(detail.join(" · ")).switch(shown, t));
        }
        let gr = p.group("From your accounts");
        gr.footer("Shown calendars appear in Calendar, Today and Search. Open an event in Notification Center's calendar to change it, or drag it to another day; calendars you can only view stay read-only. Repeating iCloud and Outlook events are changed on your iPhone, at icloud.com or in Outlook.");
        for r in rows {
            gr.row(r);
        }
    }

    // ── To-dos: where new ones go (Google Tasks or Microsoft To Do) ──
    if connected || microsoft {
        let lists: Vec<(String, String)> = arr(&g["taskLists"]).iter().map(|l| (s(&l["account"]), s(&l["id"]))).collect();
        let mut values = vec![String::new()];
        let mut labels = vec!["This computer only".to_string()];
        for l in arr(&g["taskLists"]) {
            values.push(format!("{}|{}", s(&l["account"]), s(&l["id"])));
            labels.push(format!("{}{}", s(&l["title"]), if g_accounts.len() > 1 { format!(" · {}", identity(&g_accounts, &s(&l["account"]))) } else { String::new() }));
        }
        for l in arr(&c["taskLists"]) {
            values.push(format!("cloud|{}|{}", s(&l["account"]), s(&l["id"])));
            labels.push(format!("{} · To Do", s(&l["title"])));
        }
        let cur = task_value(&cx.s("google.taskListId"), &lists);
        let index = values.iter().position(|v| *v == cur).map_or(-1, |i| i as i32);
        let pick = cx.pick_values("google.taskListId", values.into_iter().map(Value::from).collect());
        p.group("To-dos")
            .footer("Tasks from your accounts appear in To-do with your local reminders. Changes show at once and reach Google or Microsoft in the background; deleting can be undone for 8 seconds. Google Tasks has due dates but not times. If a task changes on another device before your change reaches Google, Google's version is kept and a notification shows what you had.")
            .row(Row::new("New to-dos go to").choice(&labels, index, pick).width(260.0));
    }

    // ── Subscriptions: iCalendar links ──
    let cal = &f["calendar"];
    let sources = arr(&cal["sources"]);
    let mut rows = vec![Row::new("No subscriptions yet").visible(sources.is_empty()).detail("Add a link below to see its events in Notification Center and Search.")];
    for src in &sources {
        let (name, url) = (s(&src["name"]), s(&src["url"]));
        let enabled = src["enabled"].as_bool().unwrap_or(true);
        let short: String = {
            let u = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or(&url);
            let cut: String = u.chars().take(60).collect();
            format!("{cut}{}", if url.chars().count() > 68 { "…" } else { "" })
        };
        let (u2, n2) = (url.clone(), name.clone());
        let act = cx.on(move |cx, a| {
            if a.i == 0 {
                cx.ask("accounts", "calSet", json!([u2, !enabled]));
            } else {
                cx.local_set("cal.removed", json!({"name": n2, "url": u2}));
                // (Undo for 8 seconds)
                cx.spawn("cal.removed", || {
                    std::thread::sleep(std::time::Duration::from_secs(8));
                    Value::Null
                });
                cx.ask("accounts", "calRemove", json!([u2]));
            }
        });
        let color = super::super::super::model::color(&s(&src["color"]));
        rows.push(Row::new(name).detail(short).ctl("accounts", "source", act).data(vec![Opt::new("").color(color).on(enabled)]));
    }
    let err = s(&cal["lastError"]);
    let gr = p.group("Subscribed calendars");
    gr.footer(if err.is_empty() { "Events refresh every 15 minutes. Reminders you add in Notification Center or Search are kept on this computer.".to_string() } else { err });
    for r in rows {
        gr.row(r);
    }

    // The last one removed, with Undo (8 s)
    let removed = cx.local.get("cal.removed").cloned().unwrap_or(Value::Null);
    if removed.is_object() {
        let r2 = removed.clone();
        let undo = cx.on(move |cx, _| {
            cx.ask("accounts", "calAdd", json!([s(&r2["name"]), s(&r2["url"])]));
            cx.local_set("cal.removed", Value::Null);
        });
        p.custom("accounts", "removed").text(format!("Removed “{}”", s(&removed["name"]))).id(undo);
    }

    // ── Add a calendar ──
    let url_enter = action(|cx, _| add_calendar(cx));
    let name_row = field(cx, "cal.name", "Name", "Work", 260.0, false, None);
    let url_row = field(cx, "cal.url", "Link", "https://… .ics", 260.0, false, Some(url_enter.clone())).detail(cx.local_s("cal.err"));
    let has_url = !cx.local_s("cal.url").trim().is_empty();
    let ue = url_enter.clone();
    let add_id = cx.on(move |cx, a| ue(cx, a));
    let mut add_btn = btn("Add calendar");
    if has_url {
        add_btn = add_btn.primary();
    }
    p.group("Add a calendar")
        .footer("Google Calendar: Settings → your calendar → Integrate calendar → Secret address in iCal format. iCloud: share the calendar publicly and copy the link. Outlook: Settings → Shared calendars → Publish → ICS. A local .ics file path works too.")
        .row(name_row)
        .row(url_row)
        .row(Row::new("").buttons(vec![add_btn], add_id));

    // ── Alerts ──
    let values = [-1i64, 0, 5, 10, 15, 30, 60];
    let labels: Vec<String> = ["Never", "At start time", "5 minutes", "10 minutes", "15 minutes", "30 minutes", "1 hour"].iter().map(|x| x.to_string()).collect();
    let cur = cal["defaultAlert"].as_i64().unwrap_or(10);
    let index = values.iter().position(|v| *v == cur).map_or(-1, |i| i as i32);
    let pick = cx.on(move |cx, a| {
        if let Some(v) = values.get(a.i.max(0) as usize) {
            cx.ask("accounts", "calAlert", json!([v]));
        }
    });
    p.group("Alerts").row(Row::new("Remind me before events").detail("For events without their own alert").choice(&labels, index, pick));
}

/// Add calendar (or Enter in the link): the link checked first
fn add_calendar(cx: &mut Ctx) {
    let url = cx.local_s("cal.url").trim().to_string();
    if !link_ok(&url) {
        cx.local_set("cal.err", "Paste a link that starts with https:// or webcal://, or a file path.");
        return;
    }
    cx.local_set("cal.err", "");
    let name = cx.local_s("cal.name").trim().to_string();
    cx.ask("accounts", "calAdd", json!([name, url]));
    cx.local_set("cal.name", "");
    cx.local_set("cal.url", "");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shown_and_hidden_calendars() {
        let hidden = vec!["a|1".to_string(), "2".to_string()];
        assert!(!google_shown(&hidden, "a", "1"));
        assert!(!google_shown(&hidden, "b", "2")); // (a bare id)
        assert!(google_shown(&hidden, "b", "1"));
        assert_eq!(google_toggle(&hidden, "b", "2", false), vec!["a|1".to_string()]);
        assert_eq!(google_toggle(&hidden, "b", "1", true), vec!["a|1".to_string(), "2".to_string(), "b|1".to_string()]);
    }

    #[test]
    fn where_new_todos_go() {
        let lists = vec![("a".to_string(), "L1".to_string()), ("b".to_string(), "L2".to_string())];
        assert_eq!(task_value("", &lists), "");
        assert_eq!(task_value("L2", &lists), "b|L2");
        assert_eq!(task_value("a|L1", &lists), "a|L1");
        assert_eq!(task_value("a|gone", &lists), "");
        assert_eq!(task_value("cloud|x|y", &lists), "cloud|x|y");
    }

    #[test]
    fn links_calendar_takes() {
        assert!(link_ok("https://x/y.ics") && link_ok("webcal://x") && link_ok("/home/me/a.ics"));
        assert!(!link_ok("x.ics") && !link_ok(""));
    }
}
