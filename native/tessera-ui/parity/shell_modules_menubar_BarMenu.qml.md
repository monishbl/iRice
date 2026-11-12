# Parity: shell/modules/menubar/BarMenu.qml

One bar menu on one screen, drawn by the look's menu (Glass MenuPopover,
Material MaterialMenu); tray menus list the open tray item's dbusmenu entries
and drill into submenus in place. Behaviour of the popover itself (open/close,
outside click, Escape, autoClose) is in glass/Popover.qml, summarised here and
detailed in MaterialMenu.qml.md.
Rust: `surfaces/menubar/src/lib.rs` (one `MenuEntry` window, `open_menu`,
`close_menu`, `show_rows`, tray `message`), `src/menus.rs` (`tray`, rows),
`ui/menubar.slint` `Menu` (706-895).

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| One BarMenu per panel per screen, each in a PanelLoader (made on open, unloaded 600 ms after close) | 37-56, 58-77 | lib.rs:859-899 (one window, kept hidden, moved to another output by remaking) | different by design (kept: faster) |
| Look switch: Glass MenuPopover / Material MaterialMenu | 40, 61 | menubar.slint:769-848 (`Theme.material` branches) | same |
| Tray menu rows: separator; text + "  ›" for submenus; enabled; checked; keepOpen for submenus | 21-35 | menus.rs:203-221 | same (Rust also drops `visible: false` entries) |
| Tray submenu drills down in place (handle = submenu) | 29-30 | lib.rs:996-1000 (`TraySub`), 1629-1638 (rows replaced when the answer comes) | same |
| Tray entry triggers (`e.triggered()`) and closes | 31-32 | lib.rs:991-995, 1004-1013 | same |
| On (re)open the menu shows `Panels.trayMenu` (root), not the last submenu | 47, 68 | lib.rs:919-922 (asks the root menu again) | same |
| Live rows kept when the rebuilt list is the same (no lost highlight/click) | MenuPopover 19-32 | lib.rs:831-854 (`ui_rows.set` by position, stable model) | same (concurrent change) |
| Closes: outside click | Popover 93-107 | menubar.slint:747-750; lib.rs:1695-1705 | same (concurrent change: full-screen layer) |
| Closes: Escape | MenuPopover 70, Popover 127 | menubar.slint:756 | same |
| Closes: 5 s without the pointer over it (`autoClose: 5000`, not while `holdMenus`) | MenuPopover 11, Popover 110-115 | lib.rs:74, 939-949, 1185-1195 | same (holdMenus not consulted: no forms in bar menus) |
| Closes: click on the same bar item; another item switches menus | Popover 98-105 + Panels.toggle | lib.rs:903-906, 1021-1025 | same |
| Focus loss | — (QML keeps Exclusive; never loses it except to another Exclusive layer) | lib.rs:1645-1650 (`keyboard_left` closes) | different (extra, macOS-like) |
| Switching to another menu: old closes (fade 154 ms) while new springs in from 0 | PanelLoader per panel + Popover 52 | lib.rs:913-933 (same window: rows swap, panel jumps, no re-open animation) | different |

## Summary (missing / different, by priority)

1. Switching from one bar menu to another swaps content in place with no close/open animation (QML: separate popovers, old fades out, new springs in).
2. Focus loss closes the Rust menu (QML has no such close path) — benign but different.
3. Window kept hidden instead of lazily made (intentional, faster).

## Fixed in batch 1 (fixer, 7 Nov)

- The bar menus (system, app, Window, a tray item's, a space's) never closed on a click outside: the window was the panel's size and the keyboard OnDemand (so `keyboard_left` never fired). Now the window is the whole screen (Popover.qml's catcher: `Menu` has a full-window TouchArea → `dismissed`), its input region all but the bar's 30 px strip, keyboard Exclusive while open (arrows, Return, Escape work at once), None while closing; input passes through during the close animation.
- A click on the bar's empty strip closes it (and a Panels popover: status menu, Control Center…), as the catcher's `Panels.close()` with no item there (Bar `reveal.clicked` → `empty-click`).
- autoClose 5000: 5 s with the pointer away from the panel (or never on it) closes it; resting on it (rows included: the hover TouchArea wraps the rows) waits (`menu_hold`, `M::MenuHovered`, `M::MenuAutoClose`).
- Clicking the tray item whose menu is open closes it (it reopened: the click closed it, then opened it again).
- The menu window is kept hidden between openings (it was dropped and remade each time); its rows change in place (`Rows`).

## Test plan (Sonnet tester)

Arena, user's settings, `--bin` the fixer's binary, both looks, 1920×1080 scale 1. Open a foot window first (`foot &`) so there is a window to click.
1. Click the system menu (logo, ≈ (18,15)): the menu opens. Click the desktop at (1300,700): it closes (fade ~140 ms). A shot 0.5 s later shows no menu.
2. Open the app menu (app name, ≈ (70,15)); click inside the foot window: the menu closes.
3. Open the Window menu; click an empty part of the bar (≈ (600,15)): it closes.
4. Open the system menu; click the app name: the system menu closes and the app menu opens. Click the app name again: it closes (toggle).
5. Open the system menu and move the pointer to (1300,700) without clicking; wait: shots at 4.5 s (open) and 5.5 s (closed). Open it again, rest the pointer on a row for 8 s: still open; move off: it closes 5 s later.
6. Keyboard: open the system menu by a click, then `dev run wtype -k Down -k Down -k Return`: the highlighted item runs; open again, Escape closes.
7. Tray (`python3 sandbox/tools/faketray.py &`): right-click its icon → its menu; click the icon again → closes; open, click the desktop → closes; open, wait 5.5 s → closes.
8. Space menu: right-click a space chip in the island → its menu; click outside → closes.
9. Material and Glass: the panel sits under its item as before (no sideways offset).

## Test results

## Fixes after testing (fixer, 7 Nov, batch 3 build)

- Clicking another bar item with a menu open only closed it (batch 1): the menu's window now takes the bar strip too (as the QML catcher) and passes a click there on to the bar (the bar gets the press and release at that point): the item there acts, another menu opens in the same click, the same one toggles closed, an empty spot closes it.

Retest the FAIL steps above.
