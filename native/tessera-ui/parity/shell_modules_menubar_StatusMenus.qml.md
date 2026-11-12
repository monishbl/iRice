# Parity: shell/modules/menubar/StatusMenus.qml

The status items' popovers: Wi-Fi, Bluetooth, Sound, privacy ("In use"),
battery, resources, weather. Each a glass Popover (lens 0.35, autoClose 5000,
`targetX: Panels.anchorX - 20`, width_), lazily loaded per panel per screen.
Rust: `surfaces/statusmenus/` — one surface for all seven panels
(`src/lib.rs` open 327-381, close 383-402, hold 418-426, message 877-968,
keyboard_left 970-974, after_frame 998-1032; `ui/statusmenus.slint`
`StatusMenus` 238-850; panel table `src/model.rs:16-24`). Wi-Fi is the
network surface's (not audited here). Popover = glass/Popover.qml.

## How a status menu opens and closes

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Opened by `Panels.toggle(name, screen, itemX)` from the bar item; same item again closes; a click within 250 ms of a close doesn't reopen | MenuBar 524 etc.; Panels.qml 115-120 | menubar lib.rs:1017-1019 → host shell/src/panels.rs:97-104, 525-528 → statusmenus lib.rs:826-839 | same |
| Width / offset: Bluetooth 300, Sound 300, privacy 260, battery 280, resources 280, weather 300, x = anchorX − 20 | 19, 38, 51-356 | model.rs:16-24; statusmenus.slint:361 | same |
| Position clamped on screen with the shadow margin (24) | Popover 121 | statusmenus.slint:361 | same |
| Full-screen Overlay layer, keyboard Exclusive while shown | Popover 77-79 | lib.rs:262-271, 372 | same |
| **Outside click** closes (catcher MouseArea over the whole screen, bar strip included) | Popover 93-107 | statusmenus.slint:348-351; lib.rs:915 | same for the screen below the bar |
| Click on a bar item while open: forwarded to the item (switch to its menu, or close if it's this one) | Popover 98-105 (barItemAt) | lib.rs:1012 (input region starts at y 30: the bar gets the click → toggleAt) | same |
| Click on an empty part of the bar strip while open: closes (barItemAt → null → close) | Popover 101-105 | lib.rs:1012 (strip not covered; the bar ignores it) | different: Rust leaves the menu open |
| **Escape** closes (side panel / device menu first) | Popover 127 | statusmenus.slint:353-357; lib.rs:278-284, 899-905 | same |
| **Focus loss** | — (no path: Exclusive keyboard) | lib.rs:970-974 (`keyboard_left` closes) | different (extra): e.g. a Rust bar menu opening takes the keyboard and this closes; QML closes it via Panels instead |
| **autoClose**: 5 s with the pointer off the body; timer restarts each time the pointer leaves; paused while `Panels.holdMenus > 0` | Popover 108-115; 36 | lib.rs:85, 418-426, 906-914; statusmenus.slint:377-378 | same (Rust pauses while a device menu is open instead of holdMenus; when switching panels the running timer is not restarted) |
| Close by Panels (another panel opened, shortcut) | Popover 46, 52 | host panels.rs `close` → `close` command (lib.rs:829) | same |
| Open animation: SpringAnimation spring+0.8 / damping+0.06 / eps 0.004 (Glass 4.6/0.38; Material 6.88/0.56); y −8→0, opacity = progress | Popover 50-61, 122-124 | statusmenus.slint:335-338, 362, 365 (`spring(0.3)` 420 ms) | different: one curve for both looks, not matched |
| Close animation: 154 ms bezier `exit` (Glass 0.4,0,1,1; Material 0.3,0,0.8,0.15); input passes through; ≤700 ms guard | Popover 62-76 | statusmenus.slint:336-340; lib.rs:391-398, 404-414 | same for Glass; Material uses the Glass curve |
| Switching from one status menu to another: old fades out, new springs in (separate popovers) | PanelLoader per panel | lib.rs:337-381 (same window; content swaps, no re-open animation) | different |
| ElasticGrab press-and-drag pull | Popover 82-87 | — | missing |
| Lazy: made on open, unloaded 600 ms after close | 24-30 (PanelLoader) | lib.rs:249-315 (kept hidden) | different by design (faster) |
| Glass body lens 0.35; Material surfaceContainer panel | 35; Popover 130-145 | lib.rs:1015-1031 (glass_shape lens 0.35); statusmenus.slint:366-372 | same |

## Contents

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Wi-Fi: WifiPage + SpeedTestCard (compact) | 50-62 | — (network surface) | not in this surface |
| Bluetooth: BluetoothPage without back (switch, refresh, devices, right-click device menu) | 63-68 | lib.rs:456-472, 693-758; statusmenus.slint:390-399 | same (audit with BluetoothPage.qml); Rust scans 8 s on open (lib.rs:354-357) |
| Sound: CCSlider "Sound" (icon, value muted→0, onMoved setVolume) + SoundPage without title | 69-90 | lib.rs:481-495, 768-813 | same |
| Privacy: "In use" title, empty text, rows (camera/mic/location/shares) with tinted 28 px circles (Material secondaryContainer) | 93-152 | lib.rs:497-510; model.rs `in_use` | same |
| Privacy: Stop sharing button (Material: hover 0.08 / pressed 0.1 state layers, hand cursor; Glass: pressed darker), "Stopping…" 0.6 opacity, disabled while stopping | 154-176 | statusmenus.slint:70-97 (DangerButton), 490 | same |
| Privacy: "…is still sharing" error text | 177-184 | lib.rs:506-508 | same |
| Battery: title + percent (Material primary, large); Material level bar (error ≤20 % not charging); status line (full / charging · full in / on battery · left) | 193-240 | lib.rs:512-527 | same |
| Battery: watts line "Charging at / Using x W" | 242-247 | lib.rs:527 (model `watts_line`) | same |
| Battery: health rows (max capacity, cycles, condition; "Service recommended" warning colour); UPower health fallback | 249-281 | lib.rs:528-532 | different: no UPower healthPercentage fallback (lib.rs:39-40) |
| Battery: `BatteryHealth.refresh()` each time it opens | 200 | host statusmenus_feed.rs:148-160 (on the next 500 ms tick) | different: up to 500 ms late |
| Battery: energy mode Segmented (Low Power / Balanced / Performance if available) or "who manages power" text | 282-304 | lib.rs:533-544, 927-931 | same |
| Resources: CPU (with °C), Memory (GB), Swap bars; danger over 0.9; width Behavior popover (220) | 309-351 | lib.rs:547-551; statusmenus.slint:621-625 | different: the bars' model is replaced on every feed update (`model(rows)`), so the width animation doesn't play and the rows rebuild |
| Resources sample once a second while shown | StatusMenusFeed 57-59 | host statusmenus_feed.rs:93-99 | same |
| Weather: place, icon 34, temp (Material primary 45 px / 40), condition, feels like · H/L, hourly row | 353-398 | lib.rs:553-565 | same (Rust takes 5 hours; QML all of `Weather.hourly`: verify count) |
| Lists change in place (BRIEF rule) | — | lib.rs:502, 532, 550, 564, 652 (`model(...)` new each push) | different: in-use, health, bars, hours, other players rebuilt on every refresh |

## Summary (missing / different, by priority)

1. Clicking an empty part of the bar strip leaves the Rust menu open (QML closes).
2. Open/close motion: one `spring(0.3)` 420 ms curve for both looks (QML Qt springs per look); Material close uses the Glass bezier; switching menus swaps content without the close/open animation.
3. Lists replaced on every refresh (resources bars lose their 220 ms width animation; rows rebuild) — violates "lists change in place".
4. ElasticGrab missing.
5. Focus loss closes (extra path, not in QML).
6. Battery health refresh and the first feed arrive up to 500 ms after opening (host tick), QML sends at once; no UPower health fallback.
7. autoClose timer not restarted when switching menus.

## Fixed in batch 1 (fixer, 7 Nov)

- A click on the bar's empty strip now closes an open status menu (Bar `empty-click` → `panels close`), as the QML catcher does.
- Switching from one status menu to another starts its own 5 s autoClose (the timer kept running from the first menu).
- (Clicks outside below the bar, Escape and the 5 s autoClose were already wired: the tester confirms them on the user's report that they don't work.)

## Test plan (Sonnet tester)

Arena, user's settings, `--bin` the fixer's binary, both looks; `foot &` open.
1. Click the Sound item on the right of the bar (find it in a shot): the Sound menu opens under it. Click the desktop (1300,700): closes. Open again, click inside foot: closes. Open again, click empty bar (600,15): closes.
2. Open Sound, then click the Battery (or Bluetooth) item: Sound closes, the other opens under its item.
3. Open a status menu, move the pointer to (1300,700), wait: shots at 4.5 s (open) and 5.5 s (closed). Open it, rest on it 8 s: stays; leave: closes 5 s later.
4. Open Sound, after 3 s click Battery: Battery stays open the full 5 s after that click.
5. Escape closes it (`dev run wtype -k Escape`).
6. Glass and Material.

## Test results

## Fixes after testing (fixer, 7 Nov, batch 3 build)

- Clicking another item with a status menu open opened nothing (batch 1): its window now takes the bar strip too and passes the click on to the bar (Popover's barItemAt), so the other menu opens in one click.

Retest the FAIL steps above.
