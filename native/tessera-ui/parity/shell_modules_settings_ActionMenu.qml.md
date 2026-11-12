# shell/modules/settings/ActionMenu.qml ↔ surfaces/settings (ui/settings-menu.slint, lib.rs open_menu/close_menu)

The one menu a Settings page opens (a Choice's list, a row's "…" menu, a
device's menu). QML: a QQC.Popup; Rust before this batch: a Rectangle drawn
inside the window. Now: its own xdg_popup (core `State::prepare_popup`).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | A popup of its own, above everything on the page (12–14, 103) | lib.rs `open_menu` → `st.prepare_popup(window, PopupSpec{grab: true})` + `ui::SettingsMenu` | **fixed** (was a Rectangle inside the window, cut off by its edges) |
| 2 | Items 44 px Material / 32 px Glass; at most 360 / 320 tall, the rest scrolls inside (17–18, 105–108, 132–139) | `menu_size` (tested), settings-menu.slint Flickable | same |
| 3 | Width max(240 / 220, the longest label + 60 / 44 + 10) (104) | `menu_size` (label width estimated from its characters: 7.6 / 7.4 px each, capped at 480) | same at a glance (estimate, not measured text) |
| 4 | Where: below the control, right edges lined up, 4 px apart; above when there's more room there (36–55) | PopupSpec anchor = the control, offset (control w − menu w, 4); the compositor flips it above / slides it (FlipY, SlideX/Y) | same (and now it may leave the window, as asked) |
| 5 | Opens scrolled so the checked item is in the middle (53) | `SettingsMenu.settle(checked)` | same |
| 6 | Keyboard: ↑/↓ move to the next usable item round the list, Enter/Return/Space choose, Esc closes (113–131) | settings-menu.slint `keys` (`step`, pick, dismiss) | **fixed** (none before) |
| 7 | The keys' item stays in view (20–27) | `keep-in-view()` | **fixed** |
| 8 | The keys' item or the hovered one is lit (148) | `lit: !off && (hover \|\| current == i)` | fixed (hover only before) |
| 9 | A click outside closes it (CloseOnPressOutside, and the page-wide MouseArea) (82–91, 107) | popup grab → popup_done → `surface_closed` → `close_menu` | same |
| 10 | Choosing closes it, then runs the item's action; focus back to the control (69–78) | M::MenuPick → `close_menu` → `act` | same (focus: the control keeps its FocusScope focus from the click) |
| 11 | Checked: tertiaryContainer, radius 12 (Material); lit: 8 % state layer (danger: error); Glass: accent fill, white text (149–151) | settings-menu.slint item | same |
| 12 | Check mark / item icon at 12/8 px, 20/12 px; label at 44/28 px, 14 px Material (0.1 tracking) / body (155–171) | settings-menu.slint | same |
| 13 | Disabled items: 38 % (Material) / tertiary, no pointer, not choosable (145, 168–177) | `off` | same |
| 14 | Pointing-hand cursor on usable items (176) | `mouse-cursor` | same |
| 15 | Background: GlassPopupBackground (radius Theme.radius.menu) (110) | Material surfaceContainer r16 (+1 px outline in dark), Glass menu-bg r12 + hairline | same at a glance (no blur behind the popup: the compositor's layer blur doesn't apply to xdg popups) |
| 16 | ScrollIndicator inside when it scrolls (186–189) | thin bar in settings-menu.slint | same at a glance (no fade) |
| 17 | Esc in the window while a menu is open closes the menu, not the window | settings.slint keys (menu-open → menu-dismiss); the popup has the keyboard anyway | same |

## Test plan (both looks; Material window at 440,205)

Find a Choice: Appearance page, scroll down to "Menu bar icon"/"Icon theme"
(Material) — take a shot to read its position, or Notifications & Island →
"Style" (Material: the "Material" pill at ≈ 1325,580 in a 1920×1080 shot: read it).

A1 Opens as a popup: click the Choice. `dev hyprctl layers`/`clients` don't list
popups; instead take a shot. PASS: the list shows under the control, right edges
lined up, 4 px gap, checked item marked; with the Settings window moved so the
control is near the screen bottom (`hyprctl dispatch` move the window to y≈700), the list opens
above it, or is slid inside the screen, and may extend past the window's edge.

A2/A6 (retest after batch-1 FAILs): while a menu is open, keys the Settings window gets
(the compositor may leave the keyboard with it) go to the menu: ↑/↓/Home/End move, Return/Space
choose, Esc closes only the menu; the Choice no longer takes them while its list is open.
The Notifications "Style" list is Dropdown.qml's (not ActionMenu's): 36/28 px rows, as wide as
the control and 200 at least, at most 280 tall, opening with the chosen one lit (Glass: accent
fill, white text) and the pointer moving the lit one; 11 px check, body-size text.
ActionMenu itself (a row's "…", the Keyboard page's Default app / action) stays 44/32 px.

A2 Keys: open it, `run 'key -k Down; key -k Down; key -k Return'`. PASS: the
item two below the first usable one is chosen, the menu closes, the Choice shows it.
Again with `key -k Escape`: closes, nothing chosen, the Settings window stays open.

A3 Outside click: open it, click on the sidebar (560 676). PASS: the menu closes;
compare with the QML whether that click also opened Focus (report both).

A4 Same control again: open it, click the Choice again. PASS: it closes (QML: Choice toggles).

A5 Long list: a Choice with many items (Default Apps → Terminal, or Keyboard → layout).
PASS: 360/320 px tall at most, scrolls with the wheel/drag inside, the checked one in view on open.

A6 Look: shot of the open menu in both shells. PASS: same item height, check mark,
tertiary-container checked row (Material), accent row under the pointer (Glass).

## Test results

(tester)
