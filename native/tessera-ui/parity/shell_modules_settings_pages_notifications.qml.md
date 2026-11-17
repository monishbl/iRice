# shell/modules/settings/pages/notifications.qml ↔ surfaces/settings (src/pages/notify/notifications.rs `notifications`, ui/settings-notify.slint `sound`)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Notifications & Island (page `notifications`). Opens one popup: the sound
**Style Dropdown** (shell/common/Dropdown.qml: a `QQC.Popup`); Rust uses the
Choice control and the window's menu (ActionMenu parity: an xdg_popup).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | "Do Not Disturb" switch checked = Focus.on (any Focus mode); on → Focus.deactivate(), off → Focus.activate("dnd", 0, false); footer (12–19) | notifications.rs 105–116 (`feed.focus.on`, ask focusOn/focusOff → bridges.rs `focus.activate ["dnd",0,false]`) | same |
| 2 | Link "Island" (icon pill, detail, value On/Off from island.enabled) → notifications/island (20–29) | 118–126 | same |
| 3 | "Style": now = off / material (Material look + materialSounds) / sounds.pack; detail per value ("material" text only in Material); Dropdown 200 wide, options Off, Material (Material look only), Marimba, Taps, Soft, Keys, Minimal, Glass (67–84) | 130–154, 177 `.choice(..).width(200)` | same |
| 4 | Choosing: not off → (Material) materialSounds = (v == material), v ≠ material → pack = v; enabled = v ≠ off; then plays "notifications" (Qt.callLater, at once) (85–95) | 155–169, `play_when_still` | same at a glance (plays 450 ms later, after the save) |
| 5 | Dropdown control: Glass fill squircle (radius control − 2), caret-up-down 11; Material filled pill, caret-down 14 that turns over in 200 ms OutCubic while open; ≥ 140 wide (51–82) | SChoice (settings-kit 565) | same (Material caret 20 px in Rust vs 14 – Choice parity's) |
| 6 | Dropdown list (QQC.Popup): 4 px under the control, right edges lined up, width max(control, 200), at most 280 tall, items **36 (Material) / 28 (Glass)** high, check 11 px at 8/6, label at 28/24, Glass lit item accent, Material chosen tertiaryContainer r12 (94–197) | ActionMenu menu (`cx.choice` → settings-menu.slint) | **same (fixed before 17 Nov: Dropdown list: 4 px under, right edges lined up, ≥ 200 wide, ≤ 280 tall, 36/28 px items, 11 px check)** [12 Nov: **different**: the ActionMenu's items are 44/32 px, width ≥ 240/220, at most 360/320 tall] |
| 7 | Dropdown keys while closed: **Up/Down step to the previous/next option without opening**; Space/Enter/Alt+Down open; in the list Home/End jump (31–40, 116–139) | SChoice keys: Space/Enter/Down open only | **same (fixed before 17 Nov: SChoice: Up/Down step while closed, Space/Return/Alt+Down open; Home/End in the list (settings-kit.slint, settings-menu.slint))** [12 Nov: **missing**: Up/Down stepping while closed; Down opens instead; Home/End in the list] |
| 8 | "Volume" (only while effects on): "N%", slider 220 0–1 continuous; on release plays "charging" (98–110) | 170–178 | **same (fixed before 17 Nov: plays on release (a.i == 1); keys after they rest)** [12 Nov: different (small): plays 450 ms after the last move even while still held, not on release] |
| 9 | "Notification history": detail "N kept" (live); "Clear all" tonal button (Material 40 px, Glass **26 px**), 0.4 opacity and no click with none kept → Notifications.clearAll() (112–127) | 180–182 `.button("Clear all").enabled(count>0)` | same at a glance (Glass 28 px; disabled look: Material on-surface 12 %/38 % instead of 0.4 opacity) |
| 10 | More › "Sound effects": 12 SoundRows (charging, displays with own file; lock, notifications (detail "Not in a Focus mode, and not for apps that ask for silence"), battery, volumeKeys, usb, received, recording, error, screenshot, trash without); each hidden while effects are off; detail = own file's name (131–146, 31–58) | 187–204 `sound_row` | same |
| 11 | SoundRow controls, 8 px apart: "Play" (icon play) → Sounds.play(key, true); "Choose…" (own-file rows; disabled while FilePicker busy) → file chooser "Choose a sound", ~/Music, Sounds filter → sounds.<key>File; "Reset" (only with a file) → ""; the switch sounds.<key> (42–57) | sound_row + settings-notify.slint 274 | same |
| 12 | More › "Logging in" (footer): link "Sound when you log in" (icon music-notes, detail "Your own file" / "name · style") → notifications/startup; SoundRow startup (always shown, no switch); "Volume" one slider 0–1, left end (< 0.025) off, else max(0.05, v) and on, detail "N%"/"Off", words Off / Loud; plays "startup" on release when on (148–190) | 206–238 | same (sound: 450 ms after the last move, as #8) |
| 13 | More › "Notification Center panel" switch + footer (191–198) | 240–243 | same |

## Test plan

Setups as in pages_menubar (both shells, both looks). `open notifications`;
read y from a shot. Sound is silent in the arena: check the shell log for
the play calls (`log`).

N1 Page: shot top and with More options open, scrolled to the end. PASS: same
rows, texts, footers, order. Glass Style list has no "Material".

N2 DND: click the switch on. PASS: on, the menu bar shows the DND/Focus icon;
from a terminal turn Focus off (`run 'ipc focus off'` or the menu bar): the
switch follows within a second, both shells.

N3 Style dropdown: click it. Shot the open list. PASS (QML): item height 36
(Material) / 28 (Glass), list ≥ 200 wide, right edge on the control's, 4 px
below; compare Rust's sizes and write both. Pick "Taps": config sounds.pack
"taps", enabled true, detail "Short, low taps", a play logged.
Choose "Off": Volume row and the More sound rows disappear.

N4 Style keys: click the Style control, Esc (closes, focus stays), then
`run 'key -k Down'`. PASS (QML): the choice steps to the next option without
a list opening; `key -k Up` steps back. Rust must do the same.

N5 Volume: drag the Volume slider and hold still 1 s before letting go
(`run 'P move X Y down; P move X2 Y; sleep 1; P up'`). Note when "charging"
plays (log time) in both shells.

N6 Sound rows: More options, "Plugging in the charger" › Choose… → a file
chooser titled "Choose a sound" opens in ~/Music; pick a .ogg: detail shows its
name and a Reset button appears; Reset clears both.

N7 Clear all: with notifications (`run 'notify-send a b'` ×2) detail "2 kept";
click Clear all → "0 kept", button greyed. Shot both looks.

## Fixed in batch 2 (fixer)

- Style is a Dropdown: its list has 36/28 px rows, at least the control's width and 200, at most 280 tall; it opens on the chosen one with the keys on it; closed, Up/Down step through the styles and Alt+Down/Space/Enter open it; Home/End in the list.
- Volume: the sound plays as the slider is let go (SSlider `released` → act i 1), not 450 ms after the last move; keys still play once they rest.

## Test results

(tester)
