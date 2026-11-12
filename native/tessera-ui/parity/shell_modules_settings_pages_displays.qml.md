# shell/modules/settings/pages/displays.qml ↔ surfaces/settings (pages/devices/displays.rs, ui/settings-devices.slint `confirm` / `arrange` / `picker`)

Data: RustBridgeDevices.qml `feed.displays` (Displays.monitors + lw/lh, number,
scales; confirming/confirmLeft; lowered = CoolDown.savedRates keys), `feed.ddc`,
`feed.mirroring`, `feed.needs`, `updatesRunning`. Acts: displaysSet / SetMany /
Identify / Keep / Revert / Reset, ddcSet / ddcRescan, mirrorInstall / Start / Stop, fxPreview.

QQC.Popup: the four Dropdowns (Mirror, Resolution, Refresh rate, Scale) are
common/Dropdown.qml, whose list is a **QQC.Popup** (line 94 there). Rust: `.choice()` →
the Choice + ActionMenu xdg_popup (see shell_modules_settings_ActionMenu.qml.md).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | On show: Displays.refresh() (15) | bridge onPageChanged "displays" | same |
| 2 | `current` = selected ?? first enabled ?? first (18–19); `placed` = enabled && !mirror (20) | displays.rs 149–152 (`displays.selected` local) | same |
| 3 | "Keep these display settings?" card while Displays.confirming: title 16/body DemiBold, "Going back … in N second(s)." (tnum), Go back / Keep settings (primary); Material primaryContainer r24, Glass white/5 % r12 + accent border; padding 20/14 × 16/12 (24–63) | displays.rs 157–164, slint `confirm` 547–584 | same (no tabular figures on the countdown) |
| 4 | Countdown ticks each second (Displays.confirmLeft) | feed → rebuild | same |
| 5 | No displays: "Looking for displays…" / "No displays found. Hyprland didn't report any outputs." (66–73) | displays.rs 166–167 | same |
| 6 | Arrangement canvas 220 tall, pad 20, scale k = min((w−40)/(box.w·1.6), (h−40)/(box.h·1.4)), centred (81–107) | slint Arrangement 455–466 | same |
| 7 | Tile: r12 / r6; Material surfaceContainerHighest, chosen primaryContainer + 2 px accent; Glass white 8/16 % (dark) or black 5/10 %, border 1 / 2 accent (162–180) | slint 477–480 | same |
| 8 | Built-in display: separator strip on top, max(3, 6 % h) (182–191) | slint 485–491 | same |
| 9 | Tile text: number DemiBold max(13, min(28, h·0.3)); label caption when h > 60, elided (192–212) | slint 492–510 | same |
| 10 | Press on a tile chooses it (219) | TouchArea down → "select\|name" | same |
| 11 | Drag (only with > 1 placed): open-hand / closed-hand cursor; threshold 4 px; the tile follows the pointer (216–218) | slint 512, `moved` 527–537 | same |
| 12 | Dragged tile above the others (z 2 while drag.active) (176) | slint tile: no `z` | **different**: a tile dragged over a later one is drawn under it |
| 13 | Drop: snap to the nearest free edge, lined up within 120 logical px, then shift so top-left is 0,0; only moved displays sent (Displays.setMany) (109–157, 220–224) | displays.rs `snap`/`drop_at` (unit-tested) + "drop\|…" | same (QML also drops after a ≥ 2 px move that never became a drag; Rust needs > 4 px: no visible difference) |
| 14 | After drop the tile goes home (binds to the new x/y) (168–171, 223) | dx/dy = 0 on release | same |
| 15 | Footer: "Drag displays…" (> 1 placed) / "Connect another display to arrange them." (79) | displays.rs 204 | same |
| 16 | Identify row: "Identify" / "Shows each display's number on it for a moment" + "Identify Displays" (230–234) | slint 597–643 | same |
| 17 | Which display: Segmented (only > 1 display), width min(page, 120·n), centred, "N  label" (238–245) | displays.rs 208–218, slint `picker` | same |
| 18 | Group title "label (name)"; footer description, or "This display is off…" (248–253) | displays.rs 319–320 | same |
| 19 | Use this display: Switch; off + 45 % and the note when it's the only one on; on → {enabled:true, mirror:""} (304–313) | displays.rs 228–230 | same |
| 20 | Mirror (> 1 display, on): Dropdown Off + other enabled non-mirrored displays (314–323) | displays.rs 232–248 | same (popup: see top) |
| 21 | Resolution: unique sizes, largest first, first "(Native)"; choosing picks the fastest rate there (324–337) | `resolutions`, `fastest` | same |
| 22 | Refresh rate: modes at this size fastest first, "N Hz" (2 decimals); current = closest; detail "Cool-down has lowered it for now" (338–347) | `rates`, `rate`, `lowered` | same |
| 23 | Scale: 100…200 % + current, "N %  ·  looks like W × H", Dropdown 260 wide (348–358) | `scales` (bridge sends them), `.width(260)` | same |
| 24 | Rotation: Segmented 260 (Standard/90°/180°/270°), keeps the flip bit (359–368) | displays.rs 286–289 | same |
| 25 | Brightness (DDC/CI): slider 220, detail "N %, set on the display itself (DDC/CI)" (372–381) | displays.rs 298 | same |
| 26 | Brightness: install ddcutil note (not built-in, not installed) (382–386) | displays.rs 299 | same |
| 27 | Brightness: problem text (module/access/scanning/no answer), danger label when a problem, "Check Again" disabled while scanning (387–396) | displays.rs 303–313 | same |
| 28 | Flip switch (transform ≥ 4) (397–405) | displays.rs 315–317 | same |
| 29 | Wireless displays: footer by state (not installed / error / no NetworkManager / no P2P / how-to) (409–417) | displays.rs 327–340 | same |
| 30 | Look for TVs while the page is up and Mirroring is available; stop on leave (419–433) | bridge onPageChanged + onAvailableChanged | same |
| 31 | Install row: "Install"/"Installing…", download icon, busy, disabled while updating, error in danger (434–446) | displays.rs 345–352 | same |
| 32 | "Looking for TVs…" row (447–451) | displays.rs 353 | same |
| 33 | One row per TV: name, "Mirroring"/"Connecting…"/protocol name; Mirror (screencast) / Stop (stop) (452–466) | displays.rs 356–365 (bridge sends protocolName) | same |
| 34 | More options (folded): Animation Segmented 300 (off/extend/liquid/warp) (470–484) | displays.rs 375–388 | same |
| 35 | Directional sound switch (485–489) | displays.rs 389 | same |
| 36 | Preview Left/Above/Right (icons), "off" previews as extend (490–499) | displays.rs 380–390 | same |
| 37 | Switching displays: warp pointer switch; switch cue Segmented 200 + detail by value (501–521) | displays.rs 392–408 | same |
| 38 | Restore Defaults (enabled when saved monitors) (522–530) | displays.rs 410–414 | same |

## Test plan (Material window 1040×700 at 440,205; page column from x≈772)

The arena has one display on (IRICE-TEST); WAYLAND-1 is disabled by the
script, so the page should list two displays (one off). Never change
IRICE-TEST's mode/scale/rotation except in D6.

D1 Look: `setup qml material`, `open displays`, `shot dsp-qml-m`; then
`setup rust material <bin>`, same, `shot dsp-rust-m`. Repeat with `glass`.
PASS: same groups in the same order (arrangement + Identify, display picker if
two displays, the chosen display's group, Wireless displays, More options);
tile colours, number size, footer texts, row heights match.

D2 Picker: if the Segmented shows, click the other display's segment (read
its x,y from the shot). PASS: the group below switches to "… (WAYLAND-1)" with
"This display is off…" footer and only "Use this display"; click the first
segment back. Do NOT toggle "Use this display".

D3 Tile press: click the IRICE-TEST tile (centre of the arrangement, ≈ 1110,330:
read it). PASS: it is chosen (primary container / 2 px accent); cursor is the
plain arrow (only one placed display, so no drag); release does nothing else.

D4 Dropdowns: click Resolution's control, `shot`, Esc; same for Refresh rate
and Scale. PASS: list opens under the control, right edges lined up, current
item checked, "(Native)" on the first resolution, scale labels
"N %  ·  looks like W × H"; Esc closes without changing anything.

D5 More options: open it (click "More options", read y), shot. PASS: Animation
Segmented (300 wide), Directional sound, Preview buttons with arrow icons,
Switching displays group, Restore Defaults (disabled unless the config has
saved monitors). Click Animation "Liquid" then back to its original value:
the Segmented follows. Do not click Preview (it animates the arena's screen;
optional: click "Left" once in both shells and record it with `clicks`).

D6 (optional, both shells) Keep card: change Scale to 125 %, shot within 2 s.
PASS: the "Keep these display settings?" card appears at the top with a
counting-down "Going back to the previous settings in N seconds."; click
"Go back" (read its position from the shot; it moves when the scale
changes). PASS: the scale returns to 100 % and the card goes away.

D7 Drag (can't run with one placed display): skip, and note it. Code review
only: the dragged tile has no `z` raise (row 12).

## Fixed in batch 2 (fixer)

- A display being dragged is drawn above the others (z 2).
- Mirror / Resolution / Refresh rate / Scale are Dropdowns (list size, keys: see notifications.qml's notes).

## Test results

(tester)
