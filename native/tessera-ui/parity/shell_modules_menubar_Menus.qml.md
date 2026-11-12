# Parity: shell/modules/menubar/Menus.qml

The bar's menus' contents: system menu, active app menu, Window menu, tray
menu (one BarMenu per screen each).
Rust: `surfaces/menubar/src/menus.rs` (`system` 95-119, `app` 121-127,
`window` 148-199, `tray` 203-221), chosen in `src/lib.rs:799-829` (`rows_for`),
run in `src/lib.rs:985-1013`; actions that only the shell can do go through
`rustui act` (host `shell/src/panels.rs:521-560` standalone, RustBridge.qml beside QML).

## System menu

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| About This Computer → about panel on this screen | 15 | menus.rs:98 | same |
| System Settings… with "n update(s)" shortcut | 17 | menus.rs:100 | same |
| Install n Update(s)… (only with updates) | 18 | menus.rs:102-104 | same |
| Repair Shell, shortcut Super Ctrl Escape | 21 | menus.rs:108 | same |
| Sleep / Restart… / Shut Down… / Lock Screen / Log Out user… | 23-28 | menus.rs:110-116 | same |
| Live: rows re-evaluate when `Updates.count` changes while open | 84 (binding) | lib.rs:910 (rows built on open only) | different: an update count change while open is not shown until reopened |

## App menu

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| New <App> Window (enabled with a desktop entry) | 35 | menus.rs:123 | same |
| Quit <App> (enabled with an active window) | 37 | menus.rs:125 | same |
| Live: re-evaluates when the active app changes while open | 94 | lib.rs:910 | different: built once on open |

## Window menu

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Minimize | 49 | menus.rs:165 (moves to special:minimized) | same (verify Apps.minimize does the same) |
| Float (checked = floating) | 50 | menus.rs:166 | same |
| Keep on All Spaces (checked = pinned, enabled when floating) | 51 | menus.rs:167 | same |
| Merge into Tabs / Move Tab to New Window (Super T) | 52 | menus.rs:168-174 | same |
| Picture in Picture, checked = `Tiling.isPip(win)`, Super Shift P | 53 | menus.rs:175 (`checked: Some(false)` always) | different: never shows the tick |
| Move & Resize header + three rows of tiles | 55-58 | menus.rs:177-180 | same |
| Fill / Center (Super Ctrl F / C) | 59-60 | menus.rs:181-182 | same |
| Return to Previous Size, enabled = `has && Tiling.canRestore(win)` | 61 | menus.rs:183 (enabled = has) | different: enabled even with nothing to restore |
| Arrange: Left & Right / Top & Bottom / Quarters | 62-65 | menus.rs:184-187 | same |
| Zoom / Enter Full Screen | 67-68 | menus.rs:189-190 | same |
| Move to Space 1-3 | 70-72 | menus.rs:192-194 | same |
| Close Window | 74 | menus.rs:196 | same |
| Live: re-evaluates when the active window changes (rows kept when equal) | 104 + MenuPopover 19-32 | lib.rs:910 | different: built on open only (checked states don't follow a change made elsewhere while open) |

## Tray menu

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| The clicked tray item's menu (`Panels.trayMenu`) | 109-117 | lib.rs:919-922, 1036 | same (see BarMenu.qml.md) |

## Summary (missing / different, by priority)

1. Picture in Picture never shows its check mark (QML: `Tiling.isPip(win)`).
2. Return to Previous Size enabled even when there is nothing to restore (QML: `Tiling.canRestore`).
3. Rows are computed once on open; QML menus are live bindings (updates count, active app, active window state) while open.

## Fixed in batch 5 (fixer, 8 Nov)

- An open system / app / Window menu follows changes (the active app, the window's floating/pinned state, the updates count) while open: its rows are rebuilt in place on each change (the QML's bindings).
- Not done: Picture in Picture's check and Return to Previous Size's enabled state need host/desktop's tiling to publish which windows are pipped / restorable (not this group's crate): reported.

## Test plan (batch 5)

- Open the Window menu on a floating foot window, then `hyprctl dispatch` toggling float on it: "Float" unchecks while the menu stays open.

## Test results (batch 5)
