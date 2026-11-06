//! Login Items (pages/about-loginitems.qml): the apps that start when you
//! log in (XDG autostart: ~/.config/autostart and /etc/xdg/autostart), as
//! assets/scripts/xdg-autostart.py sees them, run here as the QML page runs
//! it (it only writes your own ~/.config/autostart). A switch each (a system
//! one is turned off with a copy of it in your folder), Add app… from the
//! installed apps, Remove for ones you added, Start now. Every change shows a
//! toast with Undo.

use super::super::super::model::{Ctx, MenuItem, Page, Row};
use super::{app_icon, b, s};
use serde_json::{json, Map, Value};

fn script() -> String {
    crate::util::asset("shell/assets/scripts/xdg-autostart.py").map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The script's answer (blocking); None when it didn't answer
fn run(args: &[String]) -> Option<Value> {
    let o = std::process::Command::new("python3").arg("-I").arg(script()).args(args).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()?;
    serde_json::from_slice(&o.stdout).ok()
}

/// Skipped by Tessera whatever the switch says
pub fn skipped(it: &Value) -> bool {
    it["status"] == "skipped" || !s(&it["skippedWhenOn"]).is_empty()
}

/// The line under an item's name
pub fn status_text(it: &Value, start_at_login: bool) -> String {
    let from = if it["origin"] == "user" { "From you" } else { "System" };
    if skipped(it) {
        let why = s(&it["skippedWhenOn"]);
        return format!("{from} · Skipped: {}", if why.is_empty() { s(&it["reason"]) } else { why });
    }
    if !b(&it["enabled"]) {
        return format!("{from} · Turned off");
    }
    format!("{from} · {}", if start_at_login { "Starts when you log in" } else { "Paused while login items are off" })
}

/// The list as last read, with the changes made since laid over it (shown
/// at once, before the script has written them)
pub fn shown(items: &[Value], patch: &Map<String, Value>) -> Vec<Value> {
    items
        .iter()
        .filter(|it| patch.get(&s(&it["file"])).map_or(true, |p| p["removed"] != true))
        .map(|it| {
            let mut it = it.clone();
            if let (Some(p), Some(o)) = (patch.get(&s(&it["file"])).and_then(Value::as_object), it.as_object_mut()) {
                for (k, v) in p {
                    if k != "removed" {
                        o.insert(k.clone(), v.clone());
                    }
                }
            }
            it
        })
        .collect()
}

/// Run the script for a change: its answer lands in local[key], with what
/// it was about in local[key + ".about"]
fn call(cx: &mut Ctx, args: Vec<String>, about: Value) {
    let n = cx.local.get("about.login.n").and_then(Value::as_i64).unwrap_or(0) + 1;
    cx.local_set("about.login.n", n);
    let key = format!("about.login.act.{n}");
    cx.local_set(&format!("{key}.about"), about);
    cx.spawn(&key, move || run(&args).unwrap_or_else(|| json!({"ok": false, "error": "It didn't work; try again."})));
}

fn patch(cx: &mut Ctx, file: &str, v: Value) {
    let mut p = cx.local.get("about.login.patch").and_then(Value::as_object).cloned().unwrap_or_default();
    p.insert(file.to_string(), v);
    cx.local_set("about.login.patch", Value::Object(p));
}

fn refresh(cx: &mut Ctx) {
    if cx.local_b("about.login.list.busy") {
        cx.local_set("about.login.again", true);
        return;
    }
    cx.spawn("about.login.list", || match run(&["list".to_string()]) {
        Some(v) if v["items"].is_array() => json!({"items": v["items"]}),
        Some(_) => json!({"error": "The list came back unreadable. Try again; if it keeps happening, report a bug from Settings → Report a Bug."}),
        None => json!({"error": "Python 3 is needed to read them. Install python, then try again."}),
    });
}

pub fn set_on(cx: &mut Ctx, file: &str, name: &str, on: bool, quiet: bool) {
    patch(cx, file, json!({"enabled": on, "status": if on { "start" } else { "off" }}));
    call(cx, vec!["set".into(), file.into(), if on { "on" } else { "off" }.into()], json!({"what": "set", "file": file, "name": name, "on": on, "quiet": quiet}));
}

fn add_app(cx: &mut Ctx, id: &str, name: &str) {
    cx.local_set("about.login.filter", "");
    cx.local_set("about.login.picking", false);
    call(cx, vec!["add".into(), id.into()], json!({"what": "add", "name": name}));
}

/// The script's answers since the page was last built: toasts, and the list again
fn answers(cx: &mut Ctx) {
    let mut keys: Vec<String> = cx.local.keys().filter(|k| k.starts_with("about.login.act.") && !k.ends_with(".busy") && !k.ends_with(".about")).cloned().collect();
    keys.sort();
    let any = !keys.is_empty();
    for k in keys {
        let out = cx.local.remove(&k).unwrap_or(Value::Null);
        let about = cx.local.remove(&format!("{k}.about")).unwrap_or(Value::Null);
        let ok = out["ok"] == true;
        let err = s(&out["error"]);
        let name = s(&about["name"]);
        let file = s(&about["file"]);
        match about["what"].as_str().unwrap_or("") {
            "set" => {
                let on = b(&about["on"]);
                if !ok {
                    cx.toast(format!("Couldn't turn {}{name}. {err}", if on { "on " } else { "off " }), None);
                } else if !b(&about["quiet"]) {
                    let (f, n) = (file.clone(), name.clone());
                    cx.toast(format!("{}{name}", if on { "Turned on " } else { "Turned off " }), Some(super::action(move |cx, _| set_on(cx, &f, &n, !on, true))));
                }
            }
            "add" => {
                if !ok {
                    cx.toast(format!("Couldn't add {name}. {err}"), None);
                } else if b(&out["existed"]) {
                    cx.toast(format!("{name} was already a login item; it's on now"), None);
                } else {
                    let f = s(&out["file"]);
                    cx.toast(format!("Added {name}"), Some(super::action(move |cx, _| call(cx, vec!["remove".into(), f.clone()], json!({"what": "quiet"})))));
                }
            }
            "remove" => {
                if !ok {
                    cx.toast(format!("Couldn't remove {name}. {err}"), None);
                } else {
                    let (f, n) = (file.clone(), name.clone());
                    cx.toast(format!("Removed {name}"), Some(super::action(move |cx, _| call(cx, vec!["restore".into(), f.clone()], json!({"what": "restore", "name": n})))));
                }
            }
            "restore" => {
                if !ok {
                    cx.toast(format!("Couldn't bring back {name}. {err}"), None);
                }
            }
            "start" => cx.toast(if ok { format!("Started {name}") } else { format!("Couldn't start {name}. {err}") }, None),
            _ => {}
        }
    }
    if any || cx.local.remove("about.login.again").is_some() {
        refresh(cx);
    }
}

/// One login item as a row (ItemRow: its icon, name, status, switch, menu)
fn item_row(cx: &mut Ctx, it: &Value, start_at_login: bool) -> Row {
    let dim = skipped(it);
    let file = s(&it["file"]);
    let name = s(&it["name"]);
    let on = b(&it["enabled"]);
    let icon_name = s(&it["icon"]);
    let stem = file.trim_end_matches(".desktop").to_string();
    let px = if cx.material { 64 } else { 56 };
    let icon = app_icon(cx, &[icon_name.as_str(), stem.as_str()], px);
    let (f, n) = (file.clone(), name.clone());
    let toggle = cx.on(move |cx, _| {
        let on_now = cx.local.get("about.login.patch").and_then(|p| p[&f]["enabled"].as_bool());
        set_on(cx, &f, &n, !on_now.unwrap_or(on), false);
    });
    let (f, n) = (file.clone(), name.clone());
    let menu = cx.on(move |cx, a| match a.i {
        0 => call(cx, vec!["start".into(), f.clone()], json!({"what": "start", "name": n})),
        1 => {
            patch(cx, &f, json!({"removed": true}));
            call(cx, vec!["remove".into(), f.clone()], json!({"what": "remove", "name": n, "file": f}));
        }
        _ => {}
    });
    let mut items = vec![MenuItem::new("Start now").icon("play").enabled(b(&it["canStart"]))];
    if b(&it["removable"]) {
        items.push(MenuItem::new("Remove").icon("trash").danger());
    }
    cx.menu(&menu, items);
    let mut r = Row::custom("about", "item").id(toggle).text_value(menu).detail(status_text(it, start_at_login));
    r.label = name;
    r.on = on;
    r.enabled = !dim;
    if let Some(i) = icon {
        r.image = Some(i);
    }
    r
}

pub fn page(cx: &mut Ctx, p: &mut Page) {
    p.title = "Login Items".into();
    cx.want("apps");
    answers(cx);
    let list = cx.once("about.login.list", || match run(&["list".to_string()]) {
        Some(v) if v["items"].is_array() => json!({"items": v["items"]}),
        Some(_) => json!({"error": "The list came back unreadable. Try again; if it keeps happening, report a bug from Settings → Report a Bug."}),
        None => json!({"error": "Python 3 is needed to read them. Install python, then try again."}),
    });
    let loaded = !list.is_null();
    let error = s(&list["error"]);
    let raw: Vec<Value> = list["items"].as_array().cloned().unwrap_or_default();
    // (a change the list now shows needs no patch)
    let mut pm = cx.local.get("about.login.patch").and_then(Value::as_object).cloned().unwrap_or_default();
    pm.retain(|file, v| match raw.iter().find(|it| s(&it["file"]) == *file) {
        None => false,
        Some(it) => v["removed"] == true || v["enabled"] != it["enabled"],
    });
    cx.local_set("about.login.patch", Value::Object(pm.clone()));
    let all = shown(&raw, &pm);
    let start = cx.b("loginItems.startAtLogin");
    let active: Vec<&Value> = all.iter().filter(|it| !skipped(it)).collect();
    let ignored: Vec<&Value> = all.iter().filter(|it| skipped(it)).collect();
    let empty = loaded && error.is_empty() && all.is_empty();
    let picking = cx.local_b("about.login.picking");

    // The master switch
    let t = cx.toggle("loginItems.startAtLogin");
    p.group("")
        .footer(if start { "Apps that ask to start with your session (in ~/.config/autostart and /etc/xdg/autostart)." } else { "Nothing below starts when you log in. You can still start one now from its menu." })
        .row(Row::new("Start them when you log in").detail("Once each time you log in, after the menu bar is up").switch(start, t));

    // Loading, error, empty
    if !loaded {
        p.text("Loading login items…");
    }
    let again = cx.on(|cx, _| {
        cx.local.remove("about.login.list");
        refresh(cx);
    });
    p.group("").row(Row::new("Couldn't read your login items").visible(!error.is_empty()).tone(2).detail(error.clone()).button("Try again", again));
    let pick = cx.on(|cx, _| cx.local_set("about.login.picking", true));
    if empty && !picking {
        p.group("").row(Row::new("No login items").detail("Apps you add here start when you log in.").buttons(vec![super::super::super::model::btn("Add app…").icon("plus").primary()], pick.clone()));
    }

    // The list
    let mut rows = Vec::new();
    for it in &active {
        rows.push(item_row(cx, it, start));
    }
    let g = p.group("Open at login");
    g.footer("Turning one off here turns it off in other desktops too.");
    for r in rows {
        g.row(r);
    }

    // Add app…
    if loaded && error.is_empty() && !empty && !picking {
        p.group("").row(Row::new("Add an app to open at login").detail("Pick from the apps installed on this computer").buttons(vec![super::super::super::model::btn("Add app…").icon("plus").primary()], pick));
    }
    if picking {
        let filter = cx.local_s("about.login.filter");
        let q = filter.trim().to_lowercase();
        let have: Vec<String> = raw.iter().map(|it| s(&it["file"]).trim_end_matches(".desktop").to_string()).collect();
        let shell = cx.shell;
        let mut matches: Vec<(String, String, String)> = if q.is_empty() {
            Vec::new()
        } else {
            shell
                .get::<crate::services::apps::Apps>()
                .map(|a| a.entries.iter().filter(|e| !e.no_display && !have.contains(&e.id) && (e.name.to_lowercase().contains(&q) || e.id.to_lowercase().contains(&q))).map(|e| (e.id.clone(), e.name.clone(), e.icon.clone())).collect())
                .unwrap_or_default()
        };
        matches.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
        matches.truncate(8);
        let first = matches.first().map(|m| (m.0.clone(), m.1.clone()));
        let field = cx.on(move |cx, a| {
            if a.i == 0 {
                cx.local_set("about.login.filter", a.s.clone());
            } else if let Some((id, name)) = &first {
                add_app(cx, id, name);
            }
        });
        let cancel = cx.on(|cx, _| {
            cx.local_set("about.login.filter", "");
            cx.local_set("about.login.picking", false);
        });
        let mut search = Row::new("Search apps").ctl("about", "search", field);
        search.text = filter.clone();
        search.placeholder = "App name".into();
        search.value = cancel;
        let mut hits = Vec::new();
        let px = if cx.material { 64 } else { 56 };
        for (id, name, icon) in &matches {
            let img = app_icon(cx, &[icon.as_str(), id.as_str()], px);
            let (i2, n2) = (id.clone(), name.clone());
            let add = cx.on(move |cx, _| add_app(cx, &i2, &n2));
            let mut r = Row::custom("about", "hit").id(add);
            r.label = name.clone();
            if let Some(i) = img {
                r.image = Some(i);
            }
            hits.push(r);
        }
        let g = p.group("Add an app");
        g.row(search);
        g.row(Row::new("Type part of an app's name, like “sync” or “mail”").visible(q.is_empty()).tone(1));
        g.row(Row::new(format!("No apps match “{}”", filter.trim()))
            .visible(!q.is_empty() && matches.is_empty())
            .detail("Check the spelling, or install the app first. Apps that are already login items aren't listed."));
        for r in hits {
            g.row(r);
        }
    }

    // Skipped by Tessera
    let mut rows = Vec::new();
    for it in &ignored {
        rows.push(item_row(cx, it, start));
    }
    let g = p.group("Not started by Tessera");
    g.footer("These ask to start with your session, but Tessera leaves them out: it does their job itself, they're for another desktop, or they aren't installed.");
    for r in rows {
        g.row(r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_say_where_they_stand() {
        let it = json!({"origin": "user", "enabled": true, "status": "start", "skippedWhenOn": ""});
        assert_eq!(status_text(&it, true), "From you · Starts when you log in");
        assert_eq!(status_text(&it, false), "From you · Paused while login items are off");
        let off = json!({"origin": "system", "enabled": false, "status": "off", "skippedWhenOn": ""});
        assert_eq!(status_text(&off, true), "System · Turned off");
        let sk = json!({"origin": "system", "enabled": true, "status": "skipped", "skippedWhenOn": "", "reason": "Tessera does this"});
        assert!(skipped(&sk));
        assert_eq!(status_text(&sk, true), "System · Skipped: Tessera does this");
    }

    #[test]
    fn changes_show_before_the_script_writes_them() {
        let items = vec![json!({"file": "a.desktop", "enabled": true}), json!({"file": "b.desktop", "enabled": true})];
        let mut p = Map::new();
        p.insert("a.desktop".into(), json!({"enabled": false}));
        p.insert("b.desktop".into(), json!({"removed": true}));
        let s = shown(&items, &p);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0]["enabled"], json!(false));
    }
}
