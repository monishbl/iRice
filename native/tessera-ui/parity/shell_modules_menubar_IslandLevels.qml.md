# shell/modules/menubar/IslandLevels.qml

The list under the island's volume / mic / brightness bar, opened with the
button at its end: outputs (ticked), App volumes ›, Sound settings; inputs;
each display with its own level. Also used by `shell/modules/osd/Osd.qml`
(270) and `MaterialVolumePanel.qml` (403), outside this audit. Material
inline. Rust: none. The HUD's button instead opens the shell's Sound menu
(`rustui act toggleAt soundMenu`) or the Control Center's page for displays:
`surfaces/menubar/src/lib.rs` 1233-1241; noted as "not yet" in lib.rs 15-17.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| PwObjectTracker on outputs + streams (volume) or inputs (mic) | 26-28 | core audio `Sound` (sinks, sources, streams: the sound server's own protocol, live) | same (the list is rebuilt from it on each change while open) |
| Choice row: 32 px, picture 22 px or icon 16, name 13 (DemiBold when current), tick or caret-right | 31-92 | island.slint HUD list rows (34 px), `kind` choice / nav / back / settings | same (17 Nov; 34 px rows as the HUD row; no output pictures: core audio has no device-picture lookup, icons by kind) |
| Row hover 0.08 / press 0.14 (Material 0.1), 150 ms colour; Material current row on tertiaryContainer, round ends, state layer | 41-47 | island.slint rows `pickable` hover / press, Material current on tertiary container | same (17 Nov) |
| Row click → picked | 85-91 | island.slint `level-pick(id)` → lib.rs `M::LevelPick` | same |
| Level: Glass 6 px track (ink 0.16) and fill; Material Slider | 95-122 | island.slint display / app level (Glass 6 px ink 0.16; Material 12 px secondary-container + primary) | same (Material: a flat slider's track, no handle) |
| Level press / drag sets (Glass) | 123-133 | island.slint level TouchArea (press, drag) → `level-set` | same |
| Level wheel ±0.05 | 134-136 | island.slint level scroll-event | same |
| Outputs / inputs list, current = default sink / source; pick sets preferred default | 140-155 | lib.rs levels rows (`sink:` / `source:`), `M::LevelPick` → `DefaultSink` / `DefaultSource` | same (fixed after batch 1: inline, was the shell's Sound menu) |
| Displays: built-in picture or monitor icon, name (≤ 40 %), its own level → Brightness.setFor | 158-199 | lib.rs levels rows from state `displays` (host system mirrors Brightness.displays), built-in: shell/assets/devices/laptop.svg; `display:<id>` → `brightness.setFor [id, v, true]` | same (17 Nov; was the Control Center) |
| "App volumes ›" (when streams exist) → apps page; back row | 202-214 | lib.rs `levels_page`, rows `page:apps` (nav, caret-right) / `page:list` (back, caret-left) | same (17 Nov; the apps were listed under the outputs) |
| Per-app: icon (click mutes, 0.35 when muted), name (≤ 35 %), level (unmutes on move) | 215-256 | island.slint app row (icon TouchArea → `level-mute`, 0.35 muted); lib.rs `M::LevelMute` / `M::LevelSet` (StreamMute off, then StreamVolume) | same (17 Nov; the app's own icon from the icon theme) |
| Sound settings / Display settings → Panels.openSettings | 259-264 | lib.rs row `settings` → `panels.openSettings ["sound" / "displays"]`, the list closes | same (17 Nov) |
| Reset to the list when hidden | IslandEvent 305 | lib.rs `levels_page.clear()` on opening and when the HUD goes | same |

## Summary

1. The whole inline list is missing; the HUD's button opens the Sound menu or the Control Center instead (a different place, the island loses focus).
2. Per-app volumes reachable from the island: missing.
3. Per-display brightness levels in the island: missing.

## Fixed in batch 5 (fixer, 8 Nov)

- The HUD's caret opens the list inline under the volume / mic row (it opened the Sound menu): the outputs (or, for the mic, the inputs), each with its icon (headphones for Bluetooth), the one in use bold with a check; a click switches the default (core audio DefaultSink / DefaultSource). Then, for volume, each app playing with its own level (drag, click, scroll ±5 %: StreamVolume). The caret turns to caret-up; the alerts are held while it's open; it closes with the HUD.
- Brightness keeps opening the Control Center's brightness (no per-display levels yet).

## Test plan (batch 5)

- `wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+` (or the volume key) so the HUD shows in the island; click its caret (right end): a list of outputs appears under the row (the arena has at least the dummy sink), the default bold with a check; if an app plays (`pw-play` a file, or the fake player through pipewire), its row with a level. Click another output (create one: `pactl load-module module-null-sink sink_name=test`): `pactl get-default-sink` follows. Drag an app's level: `pactl list sink-inputs` volume follows. Click the caret again: the list closes. Mic HUD (`ipc island hud mic`): inputs listed, no apps.

## Test results (batch 5)

## Re-check 17 Nov (B4)

The outputs / inputs and the apps' levels had been made inline after batch 1 (rows still said missing). Ported now: the displays' own levels (brightness HUD), the "App volumes ›" page with its back row, the apps' icons (mute on click), Sound / Display settings, Material's current row. Before: 10 missing, 2 different; after: 13 same.
