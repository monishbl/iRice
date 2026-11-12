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
| PwObjectTracker on outputs + streams (volume) or inputs (mic) | 26-28 | — | missing |
| Choice row: 32 px, picture 22 px or icon 16, name 13 (DemiBold when current), tick or caret-right | 31-92 | — | missing |
| Row hover 0.08 / press 0.14 (Material 0.1), 150 ms colour; Material current row on tertiaryContainer, round ends, state layer | 41-47 | — | missing |
| Row click → picked | 85-91 | — | missing |
| Level: Glass 6 px track (ink 0.16) and fill; Material Slider | 95-122 | — | missing |
| Level press / drag sets (Glass) | 123-133 | — | missing |
| Level wheel ±0.05 | 134-136 | — | missing |
| Outputs / inputs list, current = default sink / source; pick sets preferred default | 140-155 | lib.rs:1233-1241 → shell Sound menu | different: a separate menu, not inline |
| Displays: built-in picture or monitor icon, name (≤ 40 %), its own level → Brightness.setFor | 158-199 | lib.rs:1236 → Control Center page | different: Control Center instead |
| "App volumes ›" (when streams exist) → apps page; back row | 202-214 | — | missing |
| Per-app: icon (click mutes, 0.35 when muted), name (≤ 35 %), level (unmutes on move) | 215-256 | — | missing |
| Sound settings / Display settings → Panels.openSettings | 259-264 | — | missing |
| Reset to the list when hidden | IslandEvent 305 | — | n/a |

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
