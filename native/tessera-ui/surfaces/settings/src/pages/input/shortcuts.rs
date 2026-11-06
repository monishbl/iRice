//! Keyboard (pages/shortcuts.qml, with dictation.qml as its part): Tap
//! Super, every shortcut grouped as the data has them and searchable by
//! name or by keys, each one's menu (change, turn off, reset or remove),
//! the form for one shortcut with its key recorder
//! (ShortcutRecorder.qml), the shortcuts' import and export, and in More
//! options the screenshot editor and Live Text. The binds are the shell's
//! Keymap (feed `keymap`); changes go back to it (`kbDisable`, `kbReset`,
//! `kbRemove`, `kbSave`, `undo`).

use super::super::super::model::{btn, Ctx, MenuItem, Opt, Page, Row};
use super::super::keys;
use super::keymap::{self, canonical, conflicts, describe, format_keys, key_label, key_text, parse_keys, ROLES, SHELL_ACTIONS};
use super::{clear_undo, feed, field_gen, idx, offer_undo, place_all, set_field, undo_text, G};
use serde_json::{json, Value};

const SCOPE: &str = "kb";

fn binds(cx: &Ctx) -> Vec<Value> {
    feed(cx)["keymap"]["binds"].as_array().cloned().unwrap_or_default()
}

fn mod_key(cx: &Ctx) -> String {
    feed(cx)["keymap"]["modKey"].as_str().filter(|s| !s.is_empty()).unwrap_or("SUPER").to_string()
}

fn keytext(cx: &Ctx, k: &str) -> String {
    key_text(k, &mod_key(cx), &keys::style(&*cx.cfg))
}

fn app_name<'a>(cx: &Ctx<'a>) -> impl Fn(&str) -> Option<String> + 'a {
    let shell = cx.shell;
    move |id: &str| shell.get::<crate::services::apps::Apps>().and_then(|a| a.by_id(id)).map(|e| e.name.clone())
}

/// [{name, binds}] in the data's order, your own shortcuts first (Keymap.groups)
pub fn groups(binds: &[Value]) -> Vec<(String, Vec<Value>)> {
    let mut out: Vec<(String, Vec<Value>)> = vec![("Your shortcuts".into(), Vec::new())];
    for b in binds {
        let g = b["group"].as_str().unwrap_or("Other");
        match out.iter_mut().find(|x| x.0 == g) {
            Some(x) => x.1.push(b.clone()),
            None => out.push((g.to_string(), vec![b.clone()])),
        }
    }
    out
}

// ── The form's state (page-local, "kb.*") ───────────────────────────────

fn ls(cx: &Ctx, k: &str) -> String {
    cx.local_s(&format!("kb.{k}"))
}
fn lb(cx: &Ctx, k: &str) -> bool {
    cx.local_b(&format!("kb.{k}"))
}
fn set(cx: &mut Ctx, k: &str, v: impl Into<Value>) {
    cx.local_set(&format!("kb.{k}"), v);
}

/// The recorder's keys: its modifiers and key, or what's typed as text
fn rec_mods(cx: &Ctx) -> Vec<String> {
    cx.local.get("kb.mods").map(super::strings).unwrap_or_default()
}
pub fn rec_result(cx: &Ctx) -> String {
    if lb(cx, "textMode") {
        keymap::from_text(&ls(cx, "text"))
    } else {
        format_keys(&rec_mods(cx), &ls(cx, "key"))
    }
}
fn rec_complete(cx: &Ctx) -> bool {
    if lb(cx, "textMode") {
        !parse_keys(&rec_result(cx)).key.is_empty()
    } else {
        !ls(cx, "key").is_empty()
    }
}
/// recorder.load(keys)
fn rec_load(cx: &mut Ctx, keys: &str) {
    let p = parse_keys(keys);
    set(cx, "mods", Value::Array(p.mods.iter().map(|m| Value::from(m.as_str())).collect()));
    set(cx, "key", p.key.clone());
    set_field(cx, "kb.text", &keymap::to_text(keys));
    set(cx, "textMode", !p.key.is_empty() && !keymap::catchable(&p.key));
}
fn rec_set_mods(cx: &mut Ctx, mods: Vec<String>) {
    let key = ls(cx, "key");
    set(cx, "mods", Value::Array(mods.iter().map(|m| Value::from(m.as_str())).collect()));
    set_field(cx, "kb.text", &keymap::to_text(&format_keys(&mods, &key)));
    set(cx, "error", "");
}

/// Open the form for a bind ("" a new shortcut of your own)
fn open_form(cx: &mut Ctx, id: &str) {
    let all = binds(cx);
    let b = all.iter().find(|b| b["id"].as_str() == Some(id)).cloned().unwrap_or(Value::Null);
    let a = b["action"].clone();
    set(cx, "editId", id);
    set(cx, "view", "form");
    set(cx, "error", "");
    let name = b["label"].as_str().unwrap_or("").to_string();
    set_field(cx, "kb.name", &name);
    set_field(cx, "kb.appSearch", "");
    set(cx, "appId", a["desktop"].as_str().unwrap_or(""));
    set(cx, "role", a["app"].as_str().unwrap_or("terminal"));
    set(cx, "shell", a["shell"].as_str().unwrap_or(""));
    let command = if a.get("exec").is_some() && a["desktop"].as_str().unwrap_or("").is_empty() { a["exec"].as_str().unwrap_or("").to_string() } else { String::new() };
    set(cx, "command", command);
    let kind = if !a["desktop"].as_str().unwrap_or("").is_empty() {
        "app"
    } else if a.get("app").is_some() {
        "role"
    } else if a.get("shell").is_some() {
        "shell"
    } else if a.get("exec").is_some() {
        "command"
    } else {
        "app"
    };
    set(cx, "kind", kind);
    set(cx, "repeating", b["opts"]["repeating"] == true);
    set(cx, "locked", b["opts"]["locked"] == true);
    rec_load(cx, b["keys"].as_str().unwrap_or(""));
}

/// What a shortcut of your own does (None: not chosen yet)
fn form_action(cx: &Ctx, editing: &Value) -> Option<Value> {
    match ls(cx, "kind").as_str() {
        "app" => {
            let id = ls(cx, "appId");
            let known = cx.shell.get::<crate::services::apps::Apps>().and_then(|a| a.by_id(&id)).is_some();
            if known && !id.is_empty() {
                // (the shell makes it a launch: Keymap.appAction)
                Some(json!({ "desktop": id }))
            } else if !id.is_empty() && editing["action"]["desktop"].as_str() == Some(&id) {
                Some(editing["action"].clone())
            } else {
                None
            }
        }
        "role" => Some(json!({ "app": ls(cx, "role") })),
        "shell" => {
            let s = ls(cx, "shell");
            (!s.is_empty()).then(|| json!({ "shell": s }))
        }
        _ => {
            let c = ls(cx, "command").trim().to_string();
            (!c.is_empty()).then(|| json!({ "exec": c }))
        }
    }
}

fn validate(cx: &Ctx, editing: &Value, custom: bool) -> String {
    if !rec_complete(cx) {
        return "Choose a key: click the box and press one, or use Edit as text.".into();
    }
    if !custom {
        return String::new();
    }
    let kind = ls(cx, "kind");
    if kind == "app" && form_action(cx, editing).is_none() {
        return "Choose an app from the list.".into();
    }
    if kind == "shell" && ls(cx, "shell").is_empty() {
        return "Choose a Tessera action.".into();
    }
    if kind == "command" && ls(cx, "command").trim().is_empty() {
        return "Enter a command to run, for example kitty -e btop.".into();
    }
    String::new()
}

fn save(cx: &mut Ctx, take_over: bool) {
    let all = binds(cx);
    let edit_id = ls(cx, "editId");
    let editing = all.iter().find(|b| b["id"].as_str() == Some(&edit_id)).cloned().unwrap_or(Value::Null);
    let custom = edit_id.is_empty() || editing["custom"] == true;
    let err = validate(cx, &editing, custom);
    if !err.is_empty() {
        set(cx, "error", err);
        return;
    }
    let keys = rec_result(cx);
    let clash: Vec<Value> = conflicts(&all, &keys, &edit_id).into_iter().cloned().collect();
    if !clash.is_empty() && !take_over {
        set(cx, "error", format!("These keys are used by {}. Choose other keys, or use them anyway.", clash[0]["label"].as_str().unwrap_or("")));
        return;
    }
    let others: Vec<Value> = if take_over { clash.iter().map(|c| c["id"].clone()).collect() } else { Vec::new() };
    let (label, entry) = if !custom {
        (editing["label"].as_str().unwrap_or("").to_string(), Value::Null)
    } else {
        let a = form_action(cx, &editing).unwrap_or(Value::Null);
        let name = ls(cx, "name").trim().to_string();
        let label = if name.is_empty() { describe(&a, &app_name(cx)) } else { name.clone() };
        let mut opts = serde_json::Map::new();
        if lb(cx, "repeating") {
            opts.insert("repeating".into(), Value::Bool(true));
        }
        if lb(cx, "locked") {
            opts.insert("locked".into(), Value::Bool(true));
        }
        (label, json!({ "keys": keys, "label": name, "action": a, "opts": opts }))
    };
    cx.ask(G, "kbSave", json!([{ "editId": edit_id, "keys": keys, "takeOver": others, "custom": custom, "entry": entry }]));
    set(cx, "view", "list");
    let turned = if others.is_empty() { String::new() } else { format!(" (turned off {})", clash.iter().filter_map(|c| c["label"].as_str()).collect::<Vec<_>>().join(", ")) };
    let text = format!("{label}: {}{turned}", keytext(cx, &keys));
    offer_undo(cx, SCOPE, text);
}

// ── The page ────────────────────────────────────────────────────────────

pub fn shortcuts(cx: &mut Ctx, p: &mut Page) {
    let km = feed(cx)["keymap"].clone();
    let ready = km["ready"].as_bool().unwrap_or(false);
    let user_error = km["userError"].as_str().unwrap_or("").to_string();
    let all = binds(cx);
    let view = if ls(cx, "view") == "form" { "form" } else { "list" };
    let edit_id = ls(cx, "editId");
    let editing = if edit_id.is_empty() { Value::Null } else { all.iter().find(|b| b["id"].as_str() == Some(&edit_id)).cloned().unwrap_or(Value::Null) };
    let custom = edit_id.is_empty() || editing["custom"] == true;

    let tap = cx.s("keyboard.superTap");
    let tv = ["overview", "spotlight", "none"];
    let tp = cx.pick("keyboard.superTap", &tv);
    p.group("").row(Row::new("Tap Super").detail("Pressing Super on its own; it still works in every shortcut").segmented(&["Overview", "Search", "Nothing"], idx(&tv, &tap), tp).width(260.0));

    // ── Problems ──
    for e in [&km["defaultsError"], &km["userError"], &km["saveError"]].into_iter().filter_map(|e| e.as_str()).filter(|e| !e.is_empty()) {
        p.group("").row(Row::new("Something's wrong with the shortcuts files").detail(e).ctl(G, "icon", String::new()).data(vec![Opt::new("").icon("warning-circle").style(4)]));
    }

    if view == "list" {
        list(cx, p, &all, ready, &user_error);
    } else {
        form(cx, p, &all, &editing, custom, &user_error);
    }

    // ── More options ──
    p.more(cx);
    if view == "list" {
        let tools = feed(cx)["tools"].clone();
        let swappy = tools["swappy"] == true;
        let editor = cx.s("screenshot.editor");
        let ed = cx.on(|cx, a| cx.set("screenshot.editor", if a.i == 1 { "swappy" } else { "tessera" }));
        let thumb = cx.b("screenshot.thumbnail");
        let tt = cx.toggle("screenshot.thumbnail");
        let markup = cx.b("screenshot.markup");
        let mt = cx.toggle("screenshot.markup");
        let mut seg = Row::new("Markup editor").segmented(&["Tessera", "Swappy"], (editor == "swappy") as i32, ed).width(200.0);
        if !swappy {
            seg = seg.off(&[1]);
        }
        p.group("Screenshots")
            .footer(if swappy { "" } else { "Install swappy to use it as the markup editor." })
            .row(seg)
            .row(Row::new("Show floating thumbnail").visible(editor != "swappy").detail("Click it to mark up the screenshot; off shows a notification with Edit").switch(thumb, tt))
            .row(Row::new("Open area screenshots in Swappy").visible(editor == "swappy").detail("To crop, draw and add text, then save or copy").switch(markup, mt).enabled(swappy));

        // Live Text: the text in screenshots and pictures
        let ocr = feed(cx)["ocr"].clone();
        let updating = feed(cx)["updatesRunning"].as_bool().unwrap_or(false);
        let has = ocr["hasTesseract"] == true;
        let pending = super::strings(&ocr["installing"]);
        let installing = updating && !pending.is_empty();
        let err = ocr["error"].as_str().unwrap_or("").to_string();
        let inst = cx.on(|cx, _| cx.ask(G, "installOcr", json!([""])));
        let search = cx.b("screenshot.textSearch");
        let st = cx.toggle("screenshot.textSearch");
        let mut rows = vec![
            Row::new("Text recognition isn't installed")
                .visible(!has)
                .detail("Install Tesseract to select and copy the text in screenshots and pictures, and to find screenshots by their text.")
                .buttons(vec![btn(if installing { "Installing…" } else { "Install" }).icon("download-simple").on(installing).off(updating)], inst),
            Row::new("Find screenshots by their text")
                .detail("Search finds a screenshot by the words in it. They're read in the background, not on battery below 30% or in power saver; turning this off deletes them.")
                .switch(search, st)
                .enabled(has),
        ];
        if has {
            for l in ocr["languages"].as_array().cloned().unwrap_or_default() {
                let code = l["code"].as_str().unwrap_or("").to_string();
                let installed = l["installed"] == true;
                let this = installing && pending.contains(&format!("tesseract-data-{code}"));
                let mut r = Row::new(l["name"].as_str().unwrap_or("")).detail(if installed { "Installed" } else { "Not installed yet" });
                if !installed {
                    let c = code.clone();
                    let id = cx.on(move |cx, _| cx.ask(G, "installOcr", json!([c])));
                    r = r.buttons(vec![btn(if this { "Installing…" } else { "Install" }).icon("download-simple").on(this).off(updating)], id);
                }
                rows.push(r);
            }
        }
        let g = p.group("Text in screenshots");
        g.footer(if !err.is_empty() {
            format!("Couldn't install: {err} Check your connection, then try again.")
        } else {
            "Text is read on this computer with Tesseract; nothing is uploaded. It reads your system's and your keyboard layouts' languages, plus English.".to_string()
        });
        for r in rows {
            g.row(r);
        }
    }
    p.end_more();

    p.part(cx, "Dictation", super::dictation::dictation);
    place_all(p);
}

/// The list: search, undo, every group of shortcuts, import and export
fn list(cx: &mut Ctx, p: &mut Page, all: &[Value], ready: bool, user_error: &str) {
    let query = ls(cx, "query").trim().to_string();
    let search = cx.on(|cx, a| match a.i {
        0 => set(cx, "query", a.s.clone()),
        2 => open_form(cx, ""),
        _ => {}
    });
    p.custom(G, "searchbar")
        .text(ls(cx, "query"))
        .num(field_gen(cx, "kb.query"))
        .data(vec![Opt::new("Add shortcut").icon("plus").on(user_error.is_empty())])
        .id(search);

    if !ready {
        p.custom(G, "note").text("Loading shortcuts…");
    }

    if let Some(t) = undo_text(cx, SCOPE) {
        let u = cx.on(|cx, _| {
            cx.ask(G, "undo", json!([]));
            clear_undo(cx, SCOPE);
        });
        p.group("").row(Row::new(t).buttons(vec![btn("Undo").icon("arrow-counter-clockwise")], u));
    }

    let shown: Vec<(String, Vec<Value>)> = groups(all)
        .into_iter()
        .map(|(n, b)| (n, b.into_iter().filter(|b| keymap::matches(b, &query)).collect::<Vec<_>>()))
        .filter(|(n, b)| !b.is_empty() || (n == "Your shortcuts" && query.is_empty()))
        .collect();
    if !query.is_empty() && shown.iter().all(|g| g.1.is_empty()) {
        let clear = cx.on(|cx, _| set_field(cx, "kb.query", ""));
        p.group("").row(Row::new(format!("No shortcuts match “{query}”")).detail("Try part of a name, like “screenshot”, or keys, like “super shift s”.").button("Clear search", clear));
    }

    let style = keys::style(&*cx.cfg);
    for (name, list) in shown {
        let mut rows = Vec::new();
        if list.is_empty() {
            let add = cx.on(|cx, _| open_form(cx, ""));
            rows.push(Row::new("No shortcuts of your own yet").detail("Add one to open an app, run a command or start a Tessera action.").buttons(vec![btn("Add shortcut").icon("plus")], add));
        }
        for b in &list {
            let id = b["id"].as_str().unwrap_or("").to_string();
            let disabled = b["disabled"] == true;
            let custom = b["custom"] == true;
            let changed = b["changed"] == true;
            let label = b["label"].as_str().unwrap_or("").to_string();
            let default_keys = b["defaultKeys"].as_str().unwrap_or("").to_string();
            let what = if disabled {
                "Off".to_string()
            } else if b["keysChanged"] == true {
                format!("Changed · was {}", keytext(cx, &default_keys))
            } else if custom {
                b["actionText"].as_str().unwrap_or("").to_string()
            } else {
                String::new()
            };
            let caps: Vec<String> = super::strings(&b["keyList"]);
            let refs: Vec<&str> = caps.iter().map(String::as_str).collect();
            let caps = keys::keys(&style, &refs);
            let dk = keytext(cx, &default_keys);
            let (mid, ml) = (id.clone(), label.clone());
            let menu = cx.on(move |cx, a| match a.i {
                0 => open_form(cx, &mid),
                1 => {
                    cx.ask(G, "kbDisable", json!([mid, !disabled]));
                    offer_undo(cx, SCOPE, format!("{}{ml}", if disabled { "Turned on " } else { "Turned off " }));
                }
                2 if custom => {
                    cx.ask(G, "kbRemove", json!([mid]));
                    offer_undo(cx, SCOPE, format!("Removed {ml}"));
                }
                2 => {
                    cx.ask(G, "kbReset", json!([mid]));
                    offer_undo(cx, SCOPE, format!("{ml} is back to {dk}"));
                }
                _ => {}
            });
            let items = vec![
                if custom { MenuItem::new("Edit").icon("pencil-simple") } else { MenuItem::new("Change shortcut").icon("keyboard") },
                MenuItem::new(if disabled { "Turn on" } else { "Turn off" }).icon("power"),
                if custom { MenuItem::new("Remove").icon("trash").danger() } else { MenuItem::new("Reset to default").icon("arrow-counter-clockwise").enabled(changed) },
            ];
            cx.menu(&menu, items);
            let mut r = Row::custom(G, "bind").detail(what).data(caps.into_iter().map(Opt::new).collect()).id(menu).enabled(!disabled);
            r.label = label;
            rows.push(r);
        }
        let g = p.group(name.clone());
        g.footer(if name == "Your shortcuts" && !list.is_empty() { "Your shortcuts win when they share keys with a default." } else { "" });
        for r in rows {
            g.row(r);
        }
    }

    if ready {
        super::importexport::block(cx, p, "keys", &["keybinds", "shortcuts", "gestures"], "Import and export");
    }
}

/// The form for one shortcut
fn form(cx: &mut Ctx, p: &mut Page, all: &[Value], editing: &Value, custom: bool, user_error: &str) {
    let edit_id = ls(cx, "editId");
    let back = cx.on(|cx, _| set(cx, "view", "list"));
    p.custom(G, "formhead")
        .title(if edit_id.is_empty() { "New shortcut" } else if custom { "Edit shortcut" } else { "Change shortcut" })
        .text(if custom { String::new() } else { format!("{} · {}", editing["label"].as_str().unwrap_or(""), editing["group"].as_str().unwrap_or("")) })
        .id(back);

    // The name
    if custom {
        let id = cx.on(|cx, a| set(cx, "name", a.s.clone()));
        let gen = field_gen(cx, "kb.name");
        p.group("Name").row(Row::new("Shown in Settings and the cheatsheet").ctl(G, "field", id).text_value(ls(cx, "name")).placeholder("Open Firefox").num(gen).width(260.0));
    }

    // The keys: the recorder
    let result = rec_result(cx);
    let style = keys::style(&*cx.cfg);
    let mk = mod_key(cx);
    let mods = rec_mods(cx);
    let key = ls(cx, "key");
    let text_mode = lb(cx, "textMode");
    let chips: Vec<Opt> = ["SUPER", "CTRL", "ALT", "SHIFT"]
        .iter()
        .map(|m| {
            let name = if style == "mac" {
                format!("{} {}", keys::name(&style, m), match *m {
                    "SUPER" => "Command",
                    "CTRL" => "Control",
                    "ALT" => "Option",
                    _ => "Shift",
                })
            } else {
                keys::name(&style, if *m == "SUPER" { mk.as_str() } else { *m })
            };
            Opt::new(name).on(mods.iter().any(|x| x == m)).id(m.to_string())
        })
        .collect();
    let key_shown = if key == "$MODKEY" { format!("Tap {} alone", keys::name(&style, &mk)) } else if key.is_empty() { String::new() } else { key_label(&key) };
    let rec = cx.on(|cx, a| recorder_event(cx, a.i, a.f, &a.s));
    let mut r = Row::custom(G, "recorder")
        .data(chips)
        .text_value(ls(cx, "text"))
        .value(key_shown)
        .num(field_gen(cx, "kb.text"))
        .id(rec);
    r.on = text_mode;
    r.label = if key.is_empty() { String::new() } else { key.clone() };
    let mut rows = vec![r];
    if !custom {
        let def = editing["defaultKeys"].as_str().unwrap_or("").to_string();
        if canonical(&result) != canonical(&def) {
            let d2 = def.clone();
            let u = cx.on(move |cx, _| {
                rec_load(cx, &d2);
                set(cx, "error", "");
            });
            rows.push(Row::new("Default").detail(keytext(cx, &def)).button("Use default keys", u));
        }
    }
    let g = p.group("Keys");
    g.footer("Most combinations go to the window manager before Settings sees them, so turn the modifiers on here and press only the last key.");
    for r in rows {
        g.row(r);
    }

    // Keys another shortcut uses
    let clash: Vec<Value> = if rec_complete(cx) { conflicts(all, &result, &edit_id).into_iter().cloned().collect() } else { Vec::new() };
    if !clash.is_empty() {
        let names = clash.iter().filter_map(|c| c["label"].as_str()).collect::<Vec<_>>().join(", ");
        let anyway = cx.on(|cx, _| save(cx, true));
        p.group("").row(Row::new(format!("Used by {names}"))
            .detail(format!("Only one shortcut can use {}.", keytext(cx, &result)))
            .button(if clash.len() == 1 { "Use anyway (turns that one off)" } else { "Use anyway (turns those off)" }, anyway));
    }

    // What a shortcut of your own does
    if custom {
        let kind = ls(cx, "kind");
        let kinds = ["app", "role", "shell", "command"];
        let kp = cx.on(move |cx, a| {
            set(cx, "kind", kinds[a.i.clamp(0, 3) as usize]);
            set(cx, "error", "");
        });
        let mut seg = Row::custom(G, "segmented").id(kp).data(["App", "Default app", "Tessera action", "Command"].iter().map(|t| Opt::new(*t)).collect());
        seg.index = idx(&kinds, &kind);
        let mut rows = vec![seg];
        match kind.as_str() {
            "role" => {
                let role = ls(cx, "role");
                let labels: Vec<String> = ROLES.iter().map(|r| r.1.to_string()).collect();
                let cur = ROLES.iter().position(|r| r.0 == role).map_or(-1, |i| i as i32);
                let id = cx.on(|cx, a| {
                    if let Some(r) = ROLES.get(a.i.max(0) as usize) {
                        set(cx, "role", r.0);
                    }
                });
                rows.push(Row::new("Opens your").detail("The app chosen for it in Settings → Default apps").choice(&labels, cur, id).width(180.0));
            }
            "shell" => {
                let sh = ls(cx, "shell");
                let labels: Vec<String> = SHELL_ACTIONS.iter().map(|r| r.1.to_string()).collect();
                let cur = SHELL_ACTIONS.iter().position(|r| r.0 == sh).map_or(-1, |i| i as i32);
                let id = cx.on(|cx, a| {
                    if let Some(r) = SHELL_ACTIONS.get(a.i.max(0) as usize) {
                        set(cx, "shell", r.0);
                        set(cx, "error", "");
                    }
                });
                rows.push(Row::new("Action").choice(&labels, cur, id).placeholder("Choose an action").width(260.0));
            }
            "command" => {
                let id = cx.on(|cx, a| set(cx, "command", a.s.clone()));
                rows.push(Row::new("Command").detail("Runs with sh, for example kitty -e btop").field(ls(cx, "command"), "kitty -e btop", id).width(300.0));
            }
            _ => {
                // An app: search, then pick
                let id = cx.on(|cx, a| set(cx, "appSearch", a.s.clone()));
                let q = ls(cx, "appSearch").trim().to_lowercase();
                rows.push(Row::custom(G, "search").id(id).text_value(ls(cx, "appSearch")).placeholder("Search apps").num(field_gen(cx, "kb.appSearch")));
                let shell = cx.shell;
                let mut entries: Vec<(String, String, String)> = shell
                    .get::<crate::services::apps::Apps>()
                    .map(|a| a.entries.iter().filter(|e| !e.no_display).map(|e| (e.id.clone(), e.name.clone(), e.generic_name.clone())).collect())
                    .unwrap_or_default();
                cx.want("apps");
                entries.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
                let total = entries.len();
                let matched: Vec<&(String, String, String)> = entries.iter().filter(|e| q.is_empty() || e.1.to_lowercase().contains(&q) || e.2.to_lowercase().contains(&q) || e.0.to_lowercase().contains(&q)).collect();
                let chosen_id = ls(cx, "appId");
                // The chosen app first, then up to 30
                let mut show: Vec<&(String, String, String)> = matched.iter().copied().filter(|e| e.0 == chosen_id).collect();
                show.extend(matched.iter().copied().filter(|e| e.0 != chosen_id).take(30));
                for (aid, aname, _) in show {
                    let icon = app_icon(cx, aid);
                    let (i2, n2) = (aid.clone(), aname.clone());
                    let pick = cx.on(move |cx, _| {
                        set(cx, "appId", i2.clone());
                        set(cx, "error", "");
                        if ls(cx, "name").trim().is_empty() {
                            set_field(cx, "kb.name", &format!("Open {n2}"));
                        }
                    });
                    let mut r = Row::custom(G, "app").id(pick);
                    r.label = aname.clone();
                    r.on = *aid == chosen_id;
                    r.image = icon;
                    rows.push(r);
                }
                if total == 0 {
                    rows.push(Row::new("Loading apps…"));
                } else if matched.is_empty() {
                    rows.push(Row::new(format!("No apps match “{}”", ls(cx, "appSearch").trim())).detail("Check the spelling, or choose Command to run any program."));
                } else if matched.len() > 31 {
                    rows.push(Row::new(format!("Showing 30 of {}. Type to narrow the list.", matched.len())));
                }
            }
        }
        let g = p.group("Action");
        for r in rows {
            g.row(r);
        }

        let rep = lb(cx, "repeating");
        let rt = cx.on(|cx, _| {
            let v = lb(cx, "repeating");
            set(cx, "repeating", !v);
        });
        let locked = lb(cx, "locked");
        let lt = cx.on(|cx, _| {
            let v = lb(cx, "locked");
            set(cx, "locked", !v);
        });
        p.group("Options")
            .row(Row::new("Repeat while held").detail("Like volume keys").switch(rep, rt))
            .row(Row::new("Works on the lock screen").switch(locked, lt));
    }

    let error = ls(cx, "error");
    if !error.is_empty() {
        p.custom(G, "note").text(error).num(2.0);
    }

    let buttons = cx.on(|cx, a| {
        if a.i == 0 {
            set(cx, "view", "list");
        } else {
            save(cx, false);
        }
    });
    p.custom(G, "actions")
        .data(vec![btn("Cancel"), btn(if edit_id.is_empty() { "Add shortcut" } else { "Save shortcut" }).primary().off(!user_error.is_empty())])
        .id(buttons);
}

/// What the recorder said: 0..3 a modifier chip, 10 a key pressed (`f` the
/// modifiers held: 1 Shift, 2 Ctrl, 4 Alt, 8 Super; `s` its text), 20 the
/// text edited, 21 Edit as text / Use the key picker
fn recorder_event(cx: &mut Ctx, i: i32, f: f32, s: &str) {
    match i {
        0..=3 => {
            let m = ["SUPER", "CTRL", "ALT", "SHIFT"][i as usize];
            let mut mods = rec_mods(cx);
            if mods.iter().any(|x| x == m) {
                mods.retain(|x| x != m);
            } else {
                mods.push(m.into());
            }
            rec_set_mods(cx, mods);
        }
        10 => {
            let Some((name, shifted)) = keymap::key_name(s) else { return };
            let held = f as i32;
            let mut mods = rec_mods(cx);
            for (bit, m) in [(1, "SHIFT"), (2, "CTRL"), (4, "ALT"), (8, "SUPER")] {
                if (held & bit != 0 || (shifted && m == "SHIFT")) && !mods.iter().any(|x| x == m) {
                    mods.push(m.into());
                }
            }
            set(cx, "key", name);
            rec_set_mods(cx, mods);
        }
        20 => {
            set(cx, "text", s);
            set(cx, "error", "");
        }
        21 => {
            if lb(cx, "textMode") {
                // Back to the picker with what was typed
                let p = parse_keys(&keymap::from_text(&ls(cx, "text")));
                set(cx, "mods", Value::Array(p.mods.iter().map(|m| Value::from(m.as_str())).collect()));
                set(cx, "key", if keymap::catchable(&p.key) { p.key } else { String::new() });
                set(cx, "textMode", false);
            } else {
                let t = keymap::to_text(&format_keys(&rec_mods(cx), &ls(cx, "key")));
                set_field(cx, "kb.text", &t);
                set(cx, "textMode", true);
            }
            set(cx, "error", "");
        }
        _ => {}
    }
}

thread_local! {
    static ICONS: std::cell::RefCell<std::collections::HashMap<String, Option<slint::Image>>> = Default::default();
}

/// An app's icon from the icon theme (AppIcon.qml without the look's styles)
fn app_icon(cx: &Ctx, id: &str) -> Option<slint::Image> {
    let icon = cx.shell.get::<crate::services::apps::Apps>().and_then(|a| a.by_id(id)).map(|e| e.icon.clone()).unwrap_or_default();
    ICONS.with(|m| {
        m.borrow_mut()
            .entry(icon.clone())
            .or_insert_with(|| {
                let path = if icon.starts_with('/') { Some(std::path::PathBuf::from(&icon)) } else { crate::icon_theme::find(&icon, None) };
                path.and_then(|p| slint::Image::load_from_path(&p).ok())
            })
            .clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn your_shortcuts_come_first() {
        let b = vec![json!({"id": "a", "group": "Apps"}), json!({"id": "c", "group": "Your shortcuts"}), json!({"id": "b", "group": "Windows"}), json!({"id": "d", "group": "Apps"})];
        let g = groups(&b);
        assert_eq!(g.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["Your shortcuts", "Apps", "Windows"]);
        assert_eq!(g[1].1.len(), 2);
        assert_eq!(groups(&[])[0].1.len(), 0);
    }
}
