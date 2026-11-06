//! Trackpad gestures (pages/mouse-gestures.qml): each "fingers + direction
//! → action", changed right in its row; a new one is drafted first. The
//! gestures and their defaults are the shell's Keymap (feed `keymap`);
//! changes go back to it (`gSet`, `gDisable`, `gReset`, `gRemove`, `gAdd`,
//! `undo`).

use super::super::super::model::{btn, Ctx, MenuItem, Opt, Page, Row};
use super::keymap::{action_key, describe_gesture, direction_label, gesture_conflicts, DIRECTIONS, GESTURE_ACTIONS, SHELL_ACTIONS};
use super::{clear_undo, feed, offer_undo, place_all, undo_text, G};
use serde_json::{json, Value};

const SCOPE: &str = "gest";

/// The actions a direction can run: a long press runs Tessera actions only
pub fn actions_for(direction: &str) -> Vec<(Value, String)> {
    let shell = SHELL_ACTIONS.iter().map(|(n, l)| (json!({ "shell": n }), l.to_string()));
    if direction == "hold" {
        shell.collect()
    } else {
        GESTURE_ACTIONS.iter().map(|(n, l)| (json!(n), l.to_string())).chain(shell).collect()
    }
}

/// A free spot for a new gesture (the first fingers and direction nothing uses)
pub fn free_spot(gestures: &[Value]) -> (i64, &'static str) {
    for f in [3, 4, 5] {
        for d in ["left", "right", "up", "down", "pinchin", "pinchout"] {
            if gesture_conflicts(gestures, f, d, "").is_empty() {
                return (f, d);
            }
        }
    }
    (5, "up")
}

fn app_name<'a>(cx: &Ctx<'a>) -> impl Fn(&str) -> Option<String> + 'a {
    let shell = cx.shell;
    move |id: &str| shell.get::<crate::services::apps::Apps>().and_then(|a| a.by_id(id)).map(|e| e.name.clone())
}

/// The label an action shows in its choice
fn action_label(cx: &Ctx, action: &Value, direction: &str) -> String {
    let k = action_key(action);
    actions_for(direction).into_iter().find(|(v, _)| action_key(v) == k).map(|x| x.1).unwrap_or_else(|| describe_gesture(action, &app_name(cx)))
}

/// The three choices of a gesture row: their actions and menus. `set` gets
/// the patch ({fingers} / {direction} / {action})
fn choices(cx: &mut Ctx, fingers: i64, direction: &str, action: &Value, set: impl Fn(&mut Ctx, Value) + Clone + 'static) -> Vec<Opt> {
    let s = set.clone();
    let fid = cx.on(move |cx, a| s(cx, json!({ "fingers": 3 + a.i.clamp(0, 2) as i64 })));
    cx.menu(&fid, (3..=5).map(|n| MenuItem::new(format!("{n} fingers")).checked(n == fingers)).collect());
    let s = set.clone();
    let did = cx.on(move |cx, a| {
        if let Some(d) = DIRECTIONS.get(a.i.max(0) as usize) {
            s(cx, json!({ "direction": d.0 }));
        }
    });
    cx.menu(&did, DIRECTIONS.iter().map(|(n, l)| MenuItem::new(*l).checked(*n == direction)).collect());
    let opts = actions_for(direction);
    let k = action_key(action);
    let values: Vec<Value> = opts.iter().map(|o| o.0.clone()).collect();
    let s = set;
    let aid = cx.on(move |cx, a| {
        if let Some(v) = values.get(a.i.max(0) as usize) {
            s(cx, json!({ "action": v }));
        }
    });
    cx.menu(&aid, opts.iter().map(|(v, l)| MenuItem::new(l.clone()).checked(action_key(v) == k)).collect());
    vec![
        Opt::new(format!("{fingers} fingers")).id(fid),
        Opt::new(direction_label(direction)).id(did),
        Opt::new(action_label(cx, action, direction)).id(aid),
    ]
}

pub fn gestures(cx: &mut Ctx, p: &mut Page) {
    p.title = "Gestures".into();
    let km = feed(cx)["keymap"].clone();
    let ready = km["ready"].as_bool().unwrap_or(false);
    let user_error = km["userError"].as_str().unwrap_or("").to_string();
    let list: Vec<Value> = km["gestures"].as_array().cloned().unwrap_or_default();
    let defaults: Vec<Value> = km["gestureDefaults"].as_array().cloned().unwrap_or_default();

    // ── Problems ──
    for e in [&km["userError"], &km["saveError"]].into_iter().filter_map(|e| e.as_str()).filter(|e| !e.is_empty()) {
        p.group("").row(Row::new("Something's wrong with the gestures file").detail(e).ctl(G, "icon", String::new()).data(vec![Opt::new("").icon("warning-circle").style(4)]));
    }

    // The line above the list, with Add gesture
    let drafting = cx.local_b("gest.draft");
    let spot = free_spot(&list);
    let add = cx.on(move |cx, _| {
        cx.local_set("gest.draft", true);
        cx.local_set("gest.fingers", spot.0);
        cx.local_set("gest.direction", spot.1);
        cx.local_set("gest.action", json!({ "shell": "missionControl" }));
    });
    p.custom(G, "addbar")
        .title("Swipe or pinch with several fingers. Changes apply right away.")
        .text("Add gesture")
        .data(vec![Opt::new("plus").on(!drafting && user_error.is_empty())])
        .id(add);

    // Undo the last change
    if let Some(t) = undo_text(cx, SCOPE) {
        let u = cx.on(|cx, _| {
            cx.ask(G, "undo", json!([]));
            clear_undo(cx, SCOPE);
        });
        p.group("").row(Row::new(t).buttons(vec![btn("Undo").icon("arrow-counter-clockwise")], u));
    }

    // A new gesture, before it's added
    if drafting {
        let fingers = cx.local.get("gest.fingers").and_then(Value::as_i64).unwrap_or(3);
        let direction = cx.local_s("gest.direction");
        let action = cx.local.get("gest.action").cloned().unwrap_or(json!({ "shell": "missionControl" }));
        let others = gesture_conflicts(&list, fingers, &direction, "");
        let clash = others.first().map(|o| format!("Clashes with {} ({}). Choose other fingers or another direction.", o["text"].as_str().unwrap_or(""), o["actionText"].as_str().unwrap_or("")));
        let opts = choices(cx, fingers, &direction, &action, |cx, patch| {
            for (k, v) in patch.as_object().into_iter().flatten() {
                cx.local_set(&format!("gest.{k}"), v.clone());
            }
        });
        let ok = clash.is_none();
        let (dir2, act2) = (direction.clone(), action.clone());
        let buttons = cx.on(move |cx, a| {
            if a.i == 0 {
                cx.local_set("gest.draft", false);
                return;
            }
            cx.ask(G, "gAdd", json!([{ "fingers": fingers, "direction": dir2, "action": act2 }]));
            cx.local_set("gest.draft", false);
            offer_undo(cx, SCOPE, format!("Added {fingers} fingers, {}", direction_label(&dir2).to_lowercase()));
        });
        p.group("New gesture")
            .row(Row::custom(G, "gesture").data(opts).value("draft").detail(clash.unwrap_or_default()).tone(4))
            .row(Row::custom(G, "buttons").data(vec![btn("Cancel"), btn("Add gesture").primary().off(!ok)]).id(buttons));
    }

    if !ready {
        p.custom(G, "note").text("Loading gestures…");
    } else {
        let mut rows = Vec::new();
        if list.is_empty() {
            rows.push(Row::new("No gestures").detail(if defaults.is_empty() {
                format!("Tessera's default gestures weren't found ({}/gestures.json). Reinstall Tessera to get them back.", km["hyprDir"].as_str().unwrap_or(""))
            } else {
                "Add one to switch spaces, open Overview and more.".to_string()
            }));
        }
        for g in &list {
            let id = g["id"].as_str().unwrap_or("").to_string();
            let fingers = g["fingers"].as_i64().unwrap_or(3);
            let direction = g["direction"].as_str().unwrap_or("").to_string();
            let action = g["action"].clone();
            let disabled = g["disabled"] == true;
            let custom = g["custom"] == true;
            let changed = g["changed"] == true;
            let others = gesture_conflicts(&list, fingers, &direction, &id);
            let def = if custom { None } else { defaults.iter().find(|d| d["id"].as_str() == Some(&id)) };
            let hold_only = direction == "hold" && action.is_string();
            let what = if disabled {
                "Off".to_string()
            } else if hold_only {
                "A long press runs Tessera actions only. Choose one on the right.".into()
            } else if let Some(o) = others.first() {
                format!("Clashes with {} ({}). Change the fingers or direction.", o["text"].as_str().unwrap_or(""), o["actionText"].as_str().unwrap_or(""))
            } else if let (true, Some(d)) = (changed, def) {
                format!(
                    "Changed · was {} fingers, {} → {}",
                    d["fingers"].as_i64().unwrap_or(0),
                    direction_label(d["direction"].as_str().unwrap_or("")).to_lowercase(),
                    describe_gesture(&d["action"], &app_name(cx))
                )
            } else {
                String::new()
            };
            let warn = (!others.is_empty() || hold_only) && !disabled;
            let gid = id.clone();
            let opts = choices(cx, fingers, &direction, &action, move |cx, patch| cx.ask(G, "gSet", json!([gid, patch])));
            // Its menu: turn on or off; remove (yours) or reset (a default)
            let (text, label) = (g["text"].as_str().unwrap_or("").to_string(), g["label"].as_str().unwrap_or("").to_string());
            let mid = id.clone();
            let menu = cx.on(move |cx, a| match a.i {
                0 => {
                    cx.ask(G, "gDisable", json!([mid, !disabled]));
                    offer_undo(cx, SCOPE, format!("{}{text}", if disabled { "Turned on " } else { "Turned off " }));
                }
                1 if custom => {
                    cx.ask(G, "gRemove", json!([mid]));
                    offer_undo(cx, SCOPE, format!("Removed {text}"));
                }
                1 => {
                    cx.ask(G, "gReset", json!([mid]));
                    offer_undo(cx, SCOPE, format!("{label} is back to its default"));
                }
                _ => {}
            });
            let mut items = vec![MenuItem::new(if disabled { "Turn on" } else { "Turn off" }).icon("power")];
            items.push(if custom { MenuItem::new("Remove").icon("trash").danger() } else { MenuItem::new("Reset to default").icon("arrow-counter-clockwise").enabled(changed) });
            cx.menu(&menu, items);
            rows.push(Row::custom(G, "gesture").data(opts).detail(what).tone(if warn { 4 } else { 1 }).id(menu).enabled(!disabled));
        }
        let g = p.group("Gestures").footer("“Left or right” covers both directions, “Any direction” every swipe, “Pinch or spread” both pinches. A long press (fingers resting still) needs Tessera's glass plugin.");
        for r in rows {
            g.row(r);
        }
        super::importexport::block(cx, p, "gestures", &["gestures"], "Import and export gestures");
    }
    place_all(p);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_press_runs_tessera_actions_only() {
        assert!(actions_for("hold").iter().all(|a| a.0.is_object()));
        assert_eq!(actions_for("up").len(), GESTURE_ACTIONS.len() + SHELL_ACTIONS.len());
    }

    #[test]
    fn a_new_gesture_goes_where_nothing_is() {
        let g = vec![json!({"id": "a", "fingers": 3, "direction": "horizontal"}), json!({"id": "b", "fingers": 3, "direction": "up"})];
        assert_eq!(free_spot(&g), (3, "down"));
        assert_eq!(free_spot(&[]), (3, "left"));
    }
}
