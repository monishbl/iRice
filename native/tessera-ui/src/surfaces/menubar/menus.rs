//! The bar's own menus, as shell/modules/menubar/Menus.qml lists them: the
//! system menu, the active app's menu, the Window menu, a tray item's menu
//! (dbusmenu, submenus drilling down in place as BarMenu.qml does), and a
//! space's right-click menu (Workspaces.qml). Rows are data; mod.rs draws
//! them (menubar.slint's Menu) and runs their actions.

use crate::services::hypr::Client;
use crate::services::tray::MenuNode;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Action {
    #[default]
    None,
    /// `rustui act <what> <arg>`: the QML shell does it (RustBridge.qml)
    Act(&'static str, String),
    /// another of the shell's IPC targets (`tiling snap left`)
    Ipc(Vec<String>),
    /// a Hyprland dispatch
    Dispatch(String),
    /// a tray menu's entry (dbusmenu id), or its submenu
    Tray(i32),
    TraySub(i32),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tile {
    /// x, y, w, h as fractions of the screen (Tiling.layouts)
    pub frac: [f32; 4],
    pub label: String,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub text: String,
    pub shortcut: String,
    pub separator: bool,
    pub header: bool,
    /// None: not checkable
    pub checked: Option<bool>,
    pub enabled: bool,
    pub tiles: Vec<Tile>,
    /// stays open when chosen (a tray submenu)
    pub keep_open: bool,
    pub action: Action,
}

impl Default for Row {
    fn default() -> Self {
        Row { text: String::new(), shortcut: String::new(), separator: false, header: false, checked: None, enabled: true, tiles: Vec::new(), keep_open: false, action: Action::None }
    }
}

fn item(text: impl Into<String>, action: Action) -> Row {
    Row { text: text.into(), action, ..Default::default() }
}
fn sep() -> Row {
    Row { separator: true, ..Default::default() }
}
fn header(text: &str) -> Row {
    Row { text: text.into(), header: true, ..Default::default() }
}

/// How shortcuts are written (KeyNames.qml): appearance.keySymbols, or
/// "auto": mac with the command icon, linux otherwise
pub fn key_style(cfg: &serde_json::Value) -> &'static str {
    match cfg["appearance"]["keySymbols"].as_str().unwrap_or("auto") {
        "mac" => "mac",
        "windows" => "windows",
        "linux" => "linux",
        _ if cfg["appearance"]["menuIcon"].as_str() == Some("command") => "mac",
        _ => "linux",
    }
}

/// "SUPER CTRL F" → "⌃⌘F" | "Win+Ctrl+F" | "Super+Ctrl+F"
pub fn combo(spec: &str, style: &str) -> String {
    let name = |k: &str| -> String {
        let up = k.to_uppercase();
        let t: &[(&str, &str)] = match style {
            "mac" => &[("SUPER", "⌘"), ("CTRL", "⌃"), ("ALT", "⌥"), ("SHIFT", "⇧"), ("ESCAPE", "⎋"), ("RETURN", "↩"), ("TAB", "⇥"), ("SPACE", "Space")],
            "windows" => &[("SUPER", "Win"), ("CTRL", "Ctrl"), ("ALT", "Alt"), ("SHIFT", "Shift"), ("ESCAPE", "Esc"), ("RETURN", "Enter"), ("TAB", "Tab"), ("SPACE", "Space")],
            _ => &[("SUPER", "Super"), ("CTRL", "Ctrl"), ("ALT", "Alt"), ("SHIFT", "Shift"), ("ESCAPE", "Esc"), ("RETURN", "Enter"), ("TAB", "Tab"), ("SPACE", "Space")],
        };
        t.iter().find(|(a, _)| *a == up).map(|(_, b)| b.to_string()).unwrap_or_else(|| if k.chars().count() == 1 { up } else { k.to_string() })
    };
    let keys: Vec<&str> = spec.split_whitespace().collect();
    let mods = ["CTRL", "ALT", "SHIFT", "SUPER"];
    let order: &[&str] = if style == "mac" { &mods } else { &["SUPER", "CTRL", "ALT", "SHIFT"] };
    let mut out: Vec<String> = order.iter().filter(|m| keys.iter().any(|k| k.eq_ignore_ascii_case(m))).map(|m| name(m)).collect();
    out.extend(keys.iter().filter(|k| !mods.iter().any(|m| k.eq_ignore_ascii_case(m))).map(|k| name(k)));
    out.join(if style == "mac" { "" } else { "+" })
}

pub fn system(screen: &str, updates: i32, user: &str, style: &str) -> Vec<Row> {
    let plural = |n: i32, one: &str, many: &str| if n == 1 { format!("{n} {one}") } else { format!("{n} {many}") };
    let mut v = vec![
        item("About This Computer", Action::Act("about", screen.into())),
        sep(),
        Row { shortcut: if updates > 0 { plural(updates, "update", "updates") } else { String::new() }, ..item("System Settings…", Action::Act("settings", String::new())) },
    ];
    if updates > 0 {
        v.push(item(format!("Install {}…", plural(updates, "Update", "Updates")), Action::Act("installUpdates", String::new())));
    }
    v.extend([
        sep(),
        // Duplicated bars or a stuck shell: restarts only the shell, apps keep running
        Row { shortcut: combo("SUPER CTRL Escape", style), ..item("Repair Shell", Action::Act("repairShell", String::new())) },
        sep(),
        item("Sleep", Action::Act("power", "sleep".into())),
        item("Restart…", Action::Act("session", screen.into())),
        item("Shut Down…", Action::Act("session", screen.into())),
        sep(),
        item("Lock Screen", Action::Act("power", "lock".into())),
        item(format!("Log Out {user}…"), Action::Act("session", screen.into())),
    ]);
    v
}

/// `entry`: the active app's desktop entry id ("" none); `class`: its window's
pub fn app(name: &str, entry: &str, class: &str) -> Vec<Row> {
    vec![
        Row { enabled: !entry.is_empty(), ..item(format!("New {name} Window"), Action::Act("launch", entry.into())) },
        sep(),
        Row { enabled: !class.is_empty(), ..item(format!("Quit {name}"), Action::Act("quit", class.into())) },
    ]
}

/// Tiling.layouts' fractions, by name
pub fn layout(name: &str) -> ([f32; 4], &'static str) {
    match name {
        "left" => ([0.0, 0.0, 0.5, 1.0], "Left"),
        "right" => ([0.5, 0.0, 0.5, 1.0], "Right"),
        "top" => ([0.0, 0.0, 1.0, 0.5], "Top"),
        "bottom" => ([0.0, 0.5, 1.0, 0.5], "Bottom"),
        "topLeft" => ([0.0, 0.0, 0.5, 0.5], "Top Left"),
        "topRight" => ([0.5, 0.0, 0.5, 0.5], "Top Right"),
        "bottomLeft" => ([0.0, 0.5, 0.5, 0.5], "Bottom Left"),
        "bottomRight" => ([0.5, 0.5, 0.5, 0.5], "Bottom Right"),
        "leftThird" => ([0.0, 0.0, 1.0 / 3.0, 1.0], "Left Third"),
        "centerThird" => ([1.0 / 3.0, 0.0, 1.0 / 3.0, 1.0], "Center Third"),
        "rightThird" => ([2.0 / 3.0, 0.0, 1.0 / 3.0, 1.0], "Right Third"),
        "leftTwoThirds" => ([0.0, 0.0, 2.0 / 3.0, 1.0], "Left Two Thirds"),
        _ => ([0.0, 0.0, 1.0, 1.0], "Fill"),
    }
}

pub fn window(win: &Client, style: &str) -> Vec<Row> {
    let has = !win.address.is_empty() && !win.class.is_empty();
    let addr = if win.address.starts_with("0x") { win.address.clone() } else { format!("0x{}", win.address) };
    let act = |name: &str, args: &str| Action::Dispatch(format!("hl.dsp.window.{name}({{ {args}{}window = \"address:{addr}\" }})", if args.is_empty() { "" } else { ", " }));
    let grouped = win.grouped.len() > 1;
    let tiles = |names: &[&str]| Row {
        tiles: names
            .iter()
            .map(|n| {
                let (frac, label) = layout(n);
                Tile { frac, label: label.into(), action: Action::Ipc(vec!["tiling".into(), "snap".into(), n.to_string()]) }
            })
            .collect(),
        ..Default::default()
    };
    let snap = |n: &str| Action::Ipc(vec!["tiling".into(), "snap".into(), n.into()]);
    let en = |r: Row, on: bool| Row { enabled: on, ..r };
    vec![
        en(item("Minimize", act("move", "workspace = \"special:minimized\", follow = false")), has),
        en(Row { checked: Some(win.floating), ..item("Float", act("float", "action = \"toggle\"")) }, has),
        en(Row { checked: Some(win.pinned), ..item("Keep on All Spaces", act("pin", "action = \"toggle\"")) }, has && win.floating),
        en(
            Row {
                shortcut: combo("SUPER T", style),
                ..item(if grouped { "Move Tab to New Window" } else { "Merge into Tabs" }, if grouped { Action::Dispatch("hl.dsp.window.move({ out_of_group = true })".into()) } else { Action::Act("mergeTabs", String::new()) })
            },
            has,
        ),
        en(Row { checked: Some(false), shortcut: combo("SUPER SHIFT P", style), ..item("Picture in Picture", Action::Ipc(vec!["tiling".into(), "pip".into()])) }, has),
        sep(),
        header("Move & Resize"),
        tiles(&["left", "right", "top", "bottom"]),
        tiles(&["topLeft", "topRight", "bottomLeft", "bottomRight"]),
        tiles(&["leftThird", "centerThird", "rightThird", "leftTwoThirds"]),
        en(Row { shortcut: combo("SUPER CTRL F", style), ..item("Fill", snap("fill")) }, has),
        en(Row { shortcut: combo("SUPER CTRL C", style), ..item("Center", snap("center")) }, has),
        en(Row { shortcut: combo("SUPER CTRL R", style), ..item("Return to Previous Size", Action::Ipc(vec!["tiling".into(), "restore".into()])) }, has),
        header("Arrange"),
        item("Left & Right", Action::Ipc(vec!["tiling".into(), "arrange".into(), "leftRight".into()])),
        item("Top & Bottom", Action::Ipc(vec!["tiling".into(), "arrange".into(), "topBottom".into()])),
        item("Quarters", Action::Ipc(vec!["tiling".into(), "arrange".into(), "quarters".into()])),
        sep(),
        en(item("Zoom", act("fullscreen", "mode = \"maximized\", action = \"toggle\"")), has),
        en(item("Enter Full Screen", act("fullscreen", "mode = \"fullscreen\", action = \"toggle\"")), has),
        sep(),
        en(item("Move to Space 1", act("move", "workspace = 1, follow = false")), has),
        en(item("Move to Space 2", act("move", "workspace = 2, follow = false")), has),
        en(item("Move to Space 3", act("move", "workspace = 3, follow = false")), has),
        sep(),
        en(item("Close Window", act("close", "")), has),
    ]
}

/// A tray menu's entries (BarMenu.trayItems): hidden ones left out,
/// submenus marked " ›" and opened in place
pub fn tray(node: &MenuNode) -> Vec<Row> {
    node.children
        .iter()
        .filter(|e| e.visible)
        .map(|e| {
            if e.separator {
                return sep();
            }
            let sub = !e.children.is_empty();
            Row {
                text: e.label.clone() + if sub { "  ›" } else { "" },
                enabled: e.enabled,
                checked: e.checked,
                keep_open: sub,
                action: if sub { Action::TraySub(e.id) } else { Action::Tray(e.id) },
                ..Default::default()
            }
        })
        .collect()
}

/// Right-click a space: move it to another display (Workspaces.openMenu)
pub fn space(id: i32, here: &str, displays: &[(String, String)]) -> Vec<Row> {
    let mut v: Vec<Row> = displays.iter().filter(|(n, _)| n != here).map(|(n, label)| item(format!("Move to {label} ({n})"), Action::Act("moveSpace", format!("{id} {n}")))).collect();
    if v.is_empty() {
        v.push(Row { enabled: false, ..item("Only one display is connected", Action::None) });
    }
    v
}

/// Material's groups (MaterialMenu): rows between separators share a
/// surfaceContainerLow block; per row, whether it starts or ends one
pub fn groups(rows: &[Row]) -> Vec<(bool, bool)> {
    let mut out = vec![(false, false); rows.len()];
    let mut start = true;
    for i in 0..rows.len() {
        if rows[i].separator {
            start = true;
            continue;
        }
        out[i].0 = start;
        start = false;
        out[i].1 = rows.get(i + 1).is_none_or(|n| n.separator);
    }
    out
}

/// The menu's width (MenuPopover: at least 220, wide enough for its rows)
pub fn width(rows: &[Row]) -> f32 {
    rows.iter().map(|r| r.text.chars().count() as f32 * 7.5 + if r.shortcut.is_empty() { 0.0 } else { 60.0 } + 48.0).fold(220.0, f32::max)
}

/// Material's menu width (MaterialMenu.qml's contentWidth: at least 232,
/// room for each row's text, shortcut and tick)
pub fn material_width(rows: &[Row]) -> f32 {
    let checks = rows.iter().any(|r| r.checked == Some(true));
    rows.iter()
        .map(|r| {
            r.text.encode_utf16().count() as f32 * 7.4
                + if r.shortcut.is_empty() { 0.0 } else { r.shortcut.encode_utf16().count() as f32 * 7.0 + 28.0 }
                + if checks { 28.0 } else { 0.0 }
                + 40.0
        })
        .fold(232.0, f32::max)
}

/// Arrow keys: the next row that can be chosen, from `from` (-1: none yet)
pub fn step(rows: &[Row], from: i32, d: i32) -> i32 {
    let n = rows.len() as i32;
    let mut i = from;
    for _ in 0..n {
        i = (i + d + n) % n;
        let r = &rows[i as usize];
        if !r.separator && !r.header && r.tiles.is_empty() && r.enabled {
            return i;
        }
    }
    from
}

/// Where the menu's panel sits on the screen (glass/Popover.qml): under its
/// item's left edge (Material's 4 px further left, MaterialMenu), but never
/// nearer a screen edge than the glass's shadow margin, 24
pub fn panel_x(item_x: f32, panel_w: f32, screen_w: f32, material: bool) -> f32 {
    let target = if material { item_x - 4.0 } else { item_x };
    target.min(screen_w - panel_w - 24.0).max(24.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn material_menus_are_as_wide_as_the_qml_ones() {
        let row = |t: &str, k: &str| Row { text: t.into(), shortcut: k.into(), separator: false, header: false, checked: None, enabled: true, tiles: Vec::new(), keep_open: false, action: Action::None };
        assert_eq!(material_width(&[row("Sleep", "")]), 232.0);
        // "Repair Shell" and its shortcut, as the system menu has them
        let w = material_width(&[row("Repair Shell", "Super+Ctrl+Esc")]);
        assert!((w - (12.0 * 7.4 + 14.0 * 7.0 + 28.0 + 40.0)).abs() < 1e-3);
    }

    #[test]
    fn menus_keep_off_the_screen_edges() {
        // the system menu, at the bar's left: 24 in, as the QML one
        assert_eq!(panel_x(8.0, 230.0, 1920.0, false), 24.0);
        assert_eq!(panel_x(100.0, 230.0, 1920.0, false), 100.0);
        assert_eq!(panel_x(100.0, 230.0, 1920.0, true), 96.0);
        assert_eq!(panel_x(1800.0, 230.0, 1920.0, false), 1666.0);
    }

    #[test]
    fn shortcuts_in_each_style() {
        assert_eq!(combo("SUPER CTRL Escape", "linux"), "Super+Ctrl+Esc");
        assert_eq!(combo("SUPER CTRL Escape", "mac"), "⌃⌘⎋");
        assert_eq!(combo("SUPER SHIFT P", "windows"), "Win+Shift+P");
        assert_eq!(key_style(&json!({})), "linux");
        assert_eq!(key_style(&json!({"appearance": {"menuIcon": "command"}})), "mac");
        assert_eq!(key_style(&json!({"appearance": {"keySymbols": "windows", "menuIcon": "command"}})), "windows");
    }

    #[test]
    fn the_system_menu_as_menus_qml() {
        let v = system("DP-1", 0, "maya", "linux");
        let texts: Vec<&str> = v.iter().filter(|r| !r.separator).map(|r| r.text.as_str()).collect();
        assert_eq!(texts, vec!["About This Computer", "System Settings…", "Repair Shell", "Sleep", "Restart…", "Shut Down…", "Lock Screen", "Log Out maya…"]);
        assert_eq!(v.iter().filter(|r| r.separator).count(), 4);
        let v = system("DP-1", 3, "maya", "linux");
        assert_eq!(v[2].shortcut, "3 updates");
        assert_eq!(v[3].text, "Install 3 Updates…");
        assert_eq!(v[3].action, Action::Act("installUpdates", String::new()));
        assert_eq!(system("DP-1", 1, "m", "linux")[3].text, "Install 1 Update…");
    }

    #[test]
    fn the_window_menu_acts_on_the_focused_window() {
        let w = Client { address: "55aa".into(), class: "kitty".into(), floating: true, pinned: true, ..Default::default() };
        let v = window(&w, "linux");
        assert_eq!(v[0].action, Action::Dispatch("hl.dsp.window.move({ workspace = \"special:minimized\", follow = false, window = \"address:0x55aa\" })".into()));
        assert_eq!((v[1].checked, v[2].checked, v[2].enabled), (Some(true), Some(true), true));
        assert_eq!(v[3].text, "Merge into Tabs");
        assert_eq!(v.last().unwrap().action, Action::Dispatch("hl.dsp.window.close({ window = \"address:0x55aa\" })".into()));
        assert_eq!(v.iter().filter(|r| !r.tiles.is_empty()).map(|r| r.tiles.len()).sum::<usize>(), 12);
        // no window: only Arrange still works
        let v = window(&Client::default(), "linux");
        let on: Vec<&str> = v.iter().filter(|r| r.enabled && !r.separator && !r.header && r.tiles.is_empty()).map(|r| r.text.as_str()).collect();
        assert_eq!(on, vec!["Left & Right", "Top & Bottom", "Quarters"]);
        // in tabs: take it out
        let g = Client { grouped: vec!["a".into(), "b".into()], ..w };
        assert_eq!(window(&g, "linux")[3].text, "Move Tab to New Window");
    }

    #[test]
    fn tray_entries_and_submenus() {
        let n = MenuNode {
            children: vec![
                MenuNode { id: 1, label: "Open".into(), enabled: true, visible: true, ..Default::default() },
                MenuNode { id: 2, separator: true, visible: true, enabled: true, ..Default::default() },
                MenuNode { id: 3, label: "Hidden".into(), visible: false, enabled: true, ..Default::default() },
                MenuNode { id: 4, label: "Recent".into(), visible: true, enabled: true, children: vec![MenuNode::default()], ..Default::default() },
            ],
            ..Default::default()
        };
        let v = tray(&n);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].action, Action::Tray(1));
        assert!(v[1].separator);
        assert_eq!((v[2].text.as_str(), v[2].keep_open, v[2].action.clone()), ("Recent  ›", true, Action::TraySub(4)));
    }

    #[test]
    fn material_groups_and_keys() {
        let v = system("DP-1", 0, "m", "linux");
        let g = groups(&v);
        // About alone; Settings alone; Repair alone; Sleep..Shut Down; Lock..Log Out
        assert_eq!(g[0], (true, true));
        assert_eq!((g[6], g[7], g[8]), ((true, false), (false, false), (false, true)));
        assert_eq!(step(&v, -1, 1), 0);
        assert_eq!(step(&v, 0, 1), 2, "separators are skipped");
        assert_eq!(step(&v, 0, -1), (v.len() - 1) as i32);
        assert!(width(&v) >= 220.0);
        assert_eq!(space(5, "DP-1", &[("DP-1".into(), "Built-in".into())])[0].enabled, false);
        assert_eq!(space(5, "DP-1", &[("DP-1".into(), "Built-in".into()), ("HDMI-A-1".into(), "TV".into())])[0].text, "Move to TV (HDMI-A-1)");
    }
}
