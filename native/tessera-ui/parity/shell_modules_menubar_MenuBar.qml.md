# Parity: shell/modules/menubar/MenuBar.qml

The menu bar window (one per screen): auto-hide, fullscreen, the left menus,
spaces/island, the right-hand items with their hover tips, scroll gestures.
Rust: `surfaces/menubar/src/lib.rs` (Bar per output, `refresh`), `src/items.rs`
(the right side), `ui/menubar.slint` (`Bar`, `Item`, `LeftItem`, the tip).
Bar menus (system/app/Window/tray/space) are audited in BarMenu/Menus/MaterialMenu;
the concurrent change (full-screen layer, Exclusive keyboard, 5 s autoClose,
kept hidden, resources hover 500 ms, stable models) is treated as done.

Rust paths are relative to `native/tessera-ui/surfaces/menubar/` unless noted.

## Window, states, layers

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| One bar per screen in `menubar.screens` (none while Rust draws it) | 20 | src/lib.rs:1389-1391, 786-795 | same |
| Visible only after the wallpaper started (`Wallpaper.started`) | 27 | — | missing: Rust bar shows at once at login |
| `revealed` = !autoHide or reveal hovered or a panel open on this screen or island event | 32 | src/lib.rs:561 | same (also counts its own bar menu: `menu_here`) |
| Reveal hover covers the whole window (incl. the 420 px island area) | 72 | ui/menubar.slint:424-426 (`reveal` TouchArea, 30 px tall) | different: Rust only hovers the 30 px strip; pointer on an open island does not count as "hovered" for auto-hide |
| `overFs` / `chromeShown` (chrome hidden over fullscreen unless auto-hide reveals it) | 37-39 | src/lib.rs:556-562 | same |
| Layer Overlay when autoHide or island lifted, else Top | 40 | src/lib.rs:716-724 | same |
| Keyboard Exclusive while the island wants keys (Esc dismisses an alert) | 43 | src/lib.rs:226 (always `None`) | missing (lib.rs header lists "Esc on an alert" as not yet) |
| Height 420 with the island, 30 without | 56 | src/lib.rs:719 (420 / 64 for tips) | different by design: 64 px without island so the in-window tip fits |
| Exclusive zone 0 with autoHide, 30 otherwise | 57-58 | src/lib.rs:718, 725-729 | same |
| Input mask: full 30 px when shown, 2 px strip while hidden, none over fullscreen; plus the island shape | 60-71 | src/lib.rs:1284-1299 | same |
| Touchscreen pull-down from the bar: right third Control Center, else Notification Center (48 px, DragHandler) | 82-95 | — | missing (lib.rs:12 "touch swipes") |
| `opaque` = background or autoHide or (Glass) solid tier | 101 | src/lib.rs:513 | different: Rust ignores `Theme.glass.tier === "solid"` (Glass solid tier gets no strip) |
| `wallInk` dark/light text over the wallpaper's top | 104 | src/lib.rs:503, 537-538 | same (0.85 black / 0.96 white) |
| Material `pills` (tonal pills for left/right groups when no background) | 108-111, 159-176 | ui/menubar.slint:455-470 | same (Theme.pill = surfaceContainerHigh vs QML island surface veil: verify colour) |
| Bar background colour (Material surfaceContainer; Glass solid / 0.92 autoHide / 0.6) | 133-137 | src/lib.rs:546-552, ui/menubar.slint:439-441 | different: Glass non-solid uses bar_tint (0.12 with plugin, 0.6 without) instead of `Theme.glass.solid` alpha |
| IdleInhibitor while Caffeine on | 113-116 | — | missing (lib.rs:11 says logind's inhibitor still holds) |
| `Resources.menubarActive` binding (sample CPU/mem while item shown) | 117-121 | host side (RustBridge binding / host resources) | same (via bridge) |
| Slide-in on reveal: y −30→0, Behavior y `Theme.motion.popover` (220) bezier `emphasized` (Glass 0.2,0.9,0.1,1; Material 0.05,0.7,0.1,1) | 127, 130 | ui/menubar.slint:434, 436 | different: Rust easing cubic-bezier(0.2,0,0,1) for both looks |
| Opacity 0↔1, Behavior 220 ms (default linear) | 128, 131 | ui/menubar.slint:435, 437 | same |
| Login intro: `Translate y: -12 * LockFx.settle` | 129 | — | missing |
| Top screen corners (Corner) when screenCorners and chrome | 140-141 | ui/menubar.slint:442-453 | same |

## Scroll

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| WheelHandler on the strip (enabled by `scrollGestures`): left half brightness ±0.05 (if backlight), right half volume ±0.05 | 145-156 | ui/menubar.slint:427-431 → src/lib.rs:1097-1111 | same (brightnessctl/wpctl 5%; Rust rejects pure-horizontal scroll, QML treats y=0 as "down") |
| Scroll over the spaces goes to Workspaces (its own WheelHandler wins) | 241 / Workspaces 351 | island.slint:258-266 (accepts) | same |

## Left side

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| System menu item: icon from `appearance.menuIcon`; "distro" draws the OS logo image tinted to `fg` (MultiEffect colorization) | 189-201 | src/settings.rs:11-31, ui/menubar.slint:488-494 | different: "distro" falls back to Tux glyph; no tinted distro logo |
| System menu click → `Panels.toggle("systemMenu", x)` | 200 | ui/menubar.slint:493 → src/lib.rs:1160-1167, 901-906 | same (toggle) |
| App menu item (bold active app name), click toggles appMenu | 202-210 | ui/menubar.slint:495-502 | same |
| Window menu item, click toggles windowMenu | 211-218 | ui/menubar.slint:503-508 | same |
| Right-click on left items opens their menu too (MenuBarItem accepts Left+Right, onClicked ignores button) | 189-218 (+MenuBarItem 179) | ui/menubar.slint:245-247 (TouchArea left only) | different: right-click on system/app/Window does nothing in Rust |
| Window title text, 0.65 alpha, elided at `bar.width/2 - leftItems.x - 360` | 219-229 | ui/menubar.slint:509-516 | same |
| Active highlight while its menu is open | 198, 207, 215 | ui/menubar.slint:492, 500, 506; src/lib.rs:571-579 | same |

## Centre

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Workspaces when island off (Material: pill +20 px) | 233-247 | ui/menubar.slint:471-478, 520-538; src/lib.rs:648 | same (see Workspaces.qml.md) |
| IslandView + IslandAlertStack (alerts card under an open island) | 250-266 | ui/island.slint `Island`; src/island.rs | partly: alerts card below open island missing (lib.rs:13-14) — audited with the island files |

## Right side items (each MenuBarItem: hover chip, click, HoverTip 500/120 ms)

All Rust items: `items.rs` builds them, `menubar.slint` `Item` (45-224) draws them,
hover tip timers at `menubar.slint:413-422`, tip at 641-660, clicks in `lib.rs:1021-1082`.

| Item / behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Util buttons (screenshot, colorPicker needs hyprpicker, mic, keyboard, darkMode, powerProfile needs ppd, record): icons + actions | 276-292 | src/items.rs:146-196; src/lib.rs:1040-1052 | same |
| Util buttons' HoverTips (texts) | 293-306 | src/items.rs:150-170 | same |
| Recording item (danger, m:ss counting each second), click stops | 311-319, 326 | src/items.rs:198-201; src/lib.rs:746-755 (1 s tick), 1054 | same |
| Recording tip "Recording · m:ss · click to stop" | 320-325 | src/items.rs:200 | same |
| Shelf item (badge = count, fill when opened or drag over, accent while drag over) | 332-338 | src/items.rs:203-211 | different: no drag-over fill/accent colour |
| Shelf DropArea: drop files → Shelf.drop + show shelf | 339-352 | — | missing (lib.rs:12) |
| Shelf tip | 353-358 | src/items.rs:208 | same |
| Shelf click → Shelf.toggle | 338 | src/lib.rs:1055 | same |
| Caffeine cup; click → Control Center on caffeine page; tip | 362-377 | src/items.rs:213-215; src/lib.rs:1056 | same |
| Tray items: shown unless collapsed and closed | 383-387 | src/items.rs:217-222 | same |
| Tray icon: `IconLookup.safe(icon, "application-x-executable")` | 388 | src/lib.rs:281-298 (`tray_image`) | different: no fallback icon (blank when the item has none) |
| Tray left click: `activate()` unless `onlyMenu`; right click (or onlyMenu) with a menu → trayMenu toggle | 391-398 | src/lib.rs:1027-1039 | same (re-click of the open item's menu closes it) |
| Tray tip: tooltipTitle ‖ title ‖ id | 399-404 | src/items.rs:220; core services/tray.rs:56-58 | same |
| Tray highlight while its menu is open | 390 | src/items.rs:220 (`tray_menu`) | same |
| Tray arrow (collapseTray): caret, click toggles; tip "Hide app icons" / "Show app icons · n" (tip has no `!active`) | 408-420 | src/items.rs:223-230; src/lib.rs:1059-1067 | same |
| Tray tucks away 12 s after opening, timer runs only while no tray menu is open (restarts when the menu closes) | 48-52 | src/lib.rs:1062-1065, 1154-1159 | different: Rust single-shot 12 s from the click; if a tray menu is open when it fires it is skipped and never re-armed → tray stays out |
| Now Playing item (island off): waveform/music-note, title ≤ 28 chars; click toggles nowPlaying; tip title · artist | 423-441 | src/items.rs:232-243; src/lib.rs:1068 | same |
| Resources item: cpu icon, "cpu% · mem%", danger over 90/95 %; click toggles resourcesMenu | 444-451 | src/items.rs:245-253; src/lib.rs:1069 | same |
| Resources: ResourceHover (500 ms open / 120 ms close, not while menu open) | 453-458 | ui/menubar.slint:381-412, 555-556 | same (concurrent change) |
| Weather item (ready only), temp; click toggles weatherMenu; tip condition · H/L · place | 462-477 | src/items.rs:255-263; src/lib.rs:1070 | same |
| Keyboard layout (two+ layouts), click → next; tip "name · click to switch" | 480-491 | src/items.rs:265-269; src/lib.rs:1058 | same |
| Clock position: Material before battery, Glass at the end | 494-498, 675-679 | src/items.rs:284-286, 339-341 | same |
| Battery: BatteryGlyph (level, charging, plugged, lowPower, percent inside), text outside / ≤20 % | 501-523 | src/items.rs:288-297; ui/menubar.slint:137-148 | same (glyph's cut-outs outlined: menubar.slint:5-6) |
| Battery click toggles batteryMenu; tip `BatteryHealth.summary()` | 524-533 | src/items.rs:294; src/lib.rs:1071 | same (fallback summary when shell gives none) |
| Bluetooth icon (slash / connected / plain), bold; click toggles bluetoothMenu; tip | 537-560 | src/items.rs:299-301; core services/bluez.rs:80-100 | same (visibility by `bt.present` vs `defaultAdapter !== null`) |
| Wi-Fi glyph (strength arcs), elsewhere (iwd…) handling; click toggles wifiMenu; tip | 563-590 | src/items.rs:303-314, 91-113; ui/menubar.slint:150-164 | same |
| Sound icon; click toggles soundMenu; tip "vol% · sink" | 593-607 | src/items.rs:316-320; src/lib.rs:1074 | same |
| Focus icon; click → Control Center focus page; tip name · status | 610-625 | src/items.rs:322-324; src/lib.rs:1057 | same |
| AI (sparkle, filled while sidebar open) → toggle sidebar; tip | 627-640 | src/items.rs:325-328; src/lib.rs:1076 | same |
| Search → toggle spotlight; tip | 641-654 | src/items.rs:329-331; src/lib.rs:1077 | same |
| PrivacyDots → toggle privacyMenu | 655-658 | src/items.rs:332-334 | same (see PrivacyDots.qml.md: Rust adds a tip) |
| Control Center → toggle; tip "Quick Settings" | 659-673 | src/items.rs:335-337; src/lib.rs:1078 | same |
| Clock: text format (date/seconds/24 h), unread indicator (off while Focus), click toggles notificationCenter | 681-698 | src/items.rs:270-282; src/settings.rs:37-49 | same |
| Clock tip "dddd d MMMM yyyy · n notifications" | 691-696 | src/items.rs:279 | same |
| SystemClock precision minutes / seconds | 699-702 | src/lib.rs:746-755 | same |
| Right-click on any right-side item = same action as left (onClicked ignores button) | MenuBarItem 179-180 | ui/menubar.slint:65-69; src/lib.rs:1021 (ignores `right` except tray) | same |

## Hover tips (HoverTip, all right-side items)

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Open after 500 ms of hover, close 120 ms after leaving, per item | HoverTip 25-39 | ui/menubar.slint:413-422 | different: one shared tip; moving item→item keeps it open and swaps text/x instantly (QML: old closes after 120 ms, new opens after its own 500 ms); timer may not restart on a direct A→B move |
| Hidden while the item's menu is open (`hovered && !active`) | e.g. 295 | ui/menubar.slint:557 | same |
| Click hides the tip | — (QML keeps it unless the item becomes active) | ui/menubar.slint:552 | different: Rust clears the tip on every click (keyboard layout, darkMode… keep it in QML) |
| Tips while a menu is open | Popover catcher covers the bar → no hover | Rust bar keeps getting hover over its strip | different: Rust shows chips/tips on bar items while a menu/popover is open |
| Stale hover after a panel opens/closes (`hoverFresh`) | MenuBarItem 166-183 | src/lib.rs:1610-1619 (PointerExited on open change) | same |

## Summary (missing / different, by priority)

1. Hover tips: one shared tip that swaps instantly between items instead of per-item 500 ms open / 120 ms close; click always hides it; tips and hover chips appear on the bar while a menu is open.
2. Tray auto-collapse timer not re-armed after a tray menu closes (tray can stay out forever).
3. Right-click on system / app / Window items does nothing (QML opens the menu).
4. Keyboard Exclusive for island alerts (Esc dismiss) missing.
5. Touchscreen pull-down (Control Center / Notification Center) missing.
6. Shelf as a drop target (DropArea, drag-over fill/accent) missing.
7. Reveal slide easing differs (Rust cubic-bezier(0.2,0,0,1) vs emphasized per look); login settle translate missing; bar shows before the wallpaper.
8. Auto-hide hover area is only the 30 px strip (QML: whole window incl. island).
9. Glass solid tier not treated as opaque; Glass strip colour differs from `Theme.glass.solid` alpha.
10. Distro logo (tinted image) not drawn; tray icon has no fallback icon.
11. IdleInhibitor for Caffeine missing.

## Fixed in batch 2 (fixer, 7 Nov)

- Hover tips behave as one HoverTip per item: the item pointed at restarts its own 500 ms; moving to another item closes the shown tip 120 ms after leaving and opens the new one 500 ms after arriving (the shared tip swapped text instantly and could be cancelled by enter/leave order). The tip appears with a 220 ms OutCubic fade and 4 px drop and vanishes at once (QML unloads it). Clicking an item hides it.
- Right-click on the system / app / Window items opens their menu (MenuBarItem accepts Left and Right).
- The privacy dots have no tip (PrivacyDots has none).
- After a tray item's menu closes, the tucked-away tray's 12 s start again (it could stay out for ever).
- A space's right-click menu is centred on its chip and has no 5 s autoClose (Workspaces' ContextMenu); still drawn as the bar menu (look and spring differ).
- Live figures don't shift: the island's status figures and the compact pill's time are as wide as the same text with every digit 0 (Slint has no tabular figures: a stable width instead of tabular digits).

## Test plan (Sonnet tester)

1. Glide onto the Wi-Fi item (find it in a shot), stay: its tip appears at 500 ± 50 ms (record and time it). Glide straight to the battery item: the Wi-Fi tip disappears ~120 ms after leaving it, the battery tip appears ~500 ms after arriving (not instantly with swapped text).
2. Move off the bar: the tip vanishes at ~120 ms with no fade-out.
3. Right-click the logo: the system menu opens; right-click the app name: the app menu.
4. With privacy in use (`dev run pw-record /dev/null &` for the mic), hover the privacy dots 1 s: no tip.
5. Tray: open the tray arrow, right-click a tray icon (`faketray.py`), close its menu; 12 s later the tray tucks away.
6. Right-click a space chip: its menu is centred under the chip; it stays open after 6 s without the pointer; outside click closes it.
7. Timer running (`ipc timer toggle`): record the compact pill for 5 s: the time's width doesn't change as digits change (the dots/controls don't move; `tools/region-changes.py` on the controls' area shows no change).

## Test results

## Fixed in batch 3 (fixer, 7 Nov)

- **Privacy dots pop in** (scale 0 → 1, 200 ms ease-out-back) as each appears.
- **Distro logo** (Settings → Appearance → Menu icon: distro): os-release's LOGO from the icon theme, 16 px, tinted with the bar's text colour (Image `colorize`).
- **Caffeine keeps the screen awake**: while `caffeine.on`, the bar holds a Wayland idle inhibitor (zwp_idle_inhibitor_v1 on the first bar's surface), released when it goes off (MenuBar.qml's IdleInhibitor).
- **Auto-hide**: the pointer over the island counts as over the bar (the QML's whole-window hover), so an open island doesn't let the bar hide.
- **Space menu**: a context menu's look: centred under the chip (batch 2) and now scaling 0.94 → 1 about its top centre (the glass follows).
- **Spaces, dynamic overflow**: chips past the window's ends are scaled as a whole (height too, 220 ms OutCubic), not only narrowed; Material chip widths on a fast spring (≈ springFast: ζ 0.6, k 800 → 222 ms, bounce 0.4).
- Not done: the shelf as a drop target on the bar (needs drag-and-drop into the bar surface: core/dnd.rs offers it but the bar has no drop handling yet); login settle translate; Glass solid tier treated as opaque.

## Test plan additions (batch 3)

8. Privacy: start a mic stream (`dev run pw-record /dev/null &`): the orange dot pops in (record 0.5 s: it grows from nothing with a slight overshoot).
9. Set `appearance.menuIcon: "distro"` (arena's os-release LOGO, e.g. archlinux-logo): the system menu item shows that logo in the bar's text colour.
10. Caffeine on (Control Center → Caffeine, or its shortcut): `dev run hyprctl clients`/the compositor's idle: with a 30 s idle timeout configured in the arena, the screen doesn't go idle while it's on; off: it does.
11. Auto-hide (`menubar.autoHide: true`): reveal the bar, move onto the island and open it: the bar stays revealed while the pointer is on the island; leaving both hides it.
12. Right-click a space chip: the menu grows from 94 % about its top centre (record it).
13. Dynamic overflow (`menubar.workspaceOverflow: "dynamic"`, more spaces than `workspaceCount`): chips past the ends are smaller in both directions and fainter.

## Test results (batch 3)

## Fixed in batch 4 (fixer, 7 Nov)

- Files dragged onto the shelf item drop onto the shelf (MenuBar.qml's DropArea): the bar takes drops (core dnd); over the shelf item it turns accent; let go there, the files (or text) go on the shelf (`shelf drop`) and it shows (`shelf show menubar <screen>`); our own drags and drops anywhere else on the bar are refused. (The glyph doesn't switch to filled while hot: its width would loop through the bar's layout; the accent colour shows it.)

## Test plan (batch 4)

- Shelf drop: with `shelf.menubar` on, start a file drag (e.g. `python3 sandbox/tools/fakedrag.py ~/Pictures/x.png` or drag from Thunar) so the shelf item appears; move over it: it turns accent; drop: the shelf opens with the file. Drop the same file elsewhere on the bar: nothing is added.

## Test results (batch 4)

## Fixed in batch 5 (fixer, 8 Nov)

- The shelf item's glyph sits in a fixed 15 px slot, so it fills (and turns accent) while files are over it without looping through the bar's layout.

## Test plan (batch 5)

- Drag files over the shelf item: its tray glyph fills and turns accent.

## Test results (batch 5)
