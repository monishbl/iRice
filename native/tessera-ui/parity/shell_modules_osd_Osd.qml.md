# shell/modules/osd/Osd.qml

The volume / microphone / brightness OSD: when a level changes it shows for
a moment — in the island (Glass, island HUD on: the default), the Glass
capsule (island HUD off), Material's volume panel (MaterialVolumePanel.qml),
or the thin bar over a fullscreen app. Also owns the media keys. Rust:
`surfaces/osd/src/lib.rs` (+ `logic.rs`, `ui/osd.slint`); the media keys
are host `host/shell/src/shortcuts.rs`; the island's level row is the menu
bar's (`island.hud` in `host/shell/src/island.rs`).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| value / icon / label per kind (muted shows 0) | 28-33 | logic.rs value/icon/label | same |
| Panels.osdVisible while the capsule shows (banners move down) | 35-41 | lib.rs refresh → `{"kind":"osd","shown"}` | same |
| show(): ignored until armed (2 s after start) | 43-45, 77 | `arm` timer, `logic::place` | same |
| show(): quiet after Tessera's own sliders (Audio/Brightness.quietUntil), unless a device name | 47-48 | `quiet_until` + hush messages | same |
| Fullscreen window focused → thin bar | 54-61 | `Place::Bar` | same |
| Island HUD on (Glass) → `Island.hud(kind, device)` | 63-66 | `Place::Island` → `shell::ask("island.hud", [kind, device])` | **fixed**: Rust did nothing here, so in Glass (island HUD on by default) no change of volume or brightness showed anywhere in standalone |
| island.hud's level: the live reading | Island.hud reads Audio/Brightness | host island.rs `hud()` reads core Sound/Backlight | **fixed**: it read state "hud.*" that nothing sets (0%) |
| Otherwise the capsule / panel; hides after 1.6 s (2.5 s with a device name) | 67-72, 79 | `Place::Hud`, `hide_after` | same |
| Switch of output / input: its name after 700 ms; changes within 1.5 s of a switch don't show of their own | 74-79, 99-128 | logic::Watch, `switch` timer | same |
| Pointer resting on it keeps it up; leaving restarts the 1.6 s | 84-86 | after_frame `pointer_on` | same |
| Hidden: the levels list closes | 87 | hide_now | same |
| setLevel(v): Brightness.set / setMicVolume / setVolume, quiet | 88-96 | `set_level` | same; **fixed**: the level shows at once and is held 600 ms over the server's readings (no jumping back under the pointer); brightness never below 1% (as Brightness.set) |
| Capsule (Glass): glass 280×52 under the bar, 220 ms emphasized in, exit curve out, scale 0.94→1 | 141-180 | OsdCapsule | same |
| Capsule icon click mutes (not brightness) | 182-200 | `toggle-mute` | same |
| Capsule level: press/drag (margins −10), wheel ±5% | 212-243 | `drag` TouchArea | same |
| Capsule button: Outputs/Inputs/Displays, opens IslandLevels below | 245-277 | `more` → ToggleLevels | same |
| IslandLevels rows: outputs (tick), App volumes ›, per-app levels (icon mutes), displays' level, settings | 270-277 | logic::rows, Levels | same; rows now a `Rows` (in place) |
| "Sound settings" / "Display settings" open Settings | IslandLevels 259-264 | `Act::Settings` → `shell::ask("panels.openSettings", [page])` | **fixed**: it ran shell-ipc.sh (the QML shell's IPC), which standalone has no QML to answer |
| A row's level (display, app) follows the pointer at once | IslandLevels | `row_held` | **fixed** |
| Fullscreen bar: 30% wide, 5 px, fade 150 in / 100 out, 4 px travel (not with Reduce motion), 1.2 s | 281-372 | OsdBar | same (halo: shadow only, no MultiEffect) |
| ipc `osd preview <kind>` | 375-383 | message `preview` | same |
| Media keys volumeUp/Down: Audio.setVolume(volume ± 0.05) (unmutes) + click sound | 386-387 | shortcuts.rs | **fixed**: Rust didn't unmute (the level changed while still muted: the OSD showed 0); a held key repeated from a stale reading — steps now go on from the level just set |
| volumeMute / micMute toggle | 388-389 | shortcuts.rs | same |
| brightnessUp/Down ±5% (no backlight: nothing) | 390-391 | shortcuts.rs | same; steps accumulate; never below 1%. DDC external displays: not via the keys (the QML's Brightness.set targets the focused DDC display) — **missing**, noted |
| Brightness written through logind; brightnessctl when logind refuses | (Brightness.qml uses brightnessctl) | core services/brightness.rs | **fixed** (fallback added) |

## Tester plan

Arena, user's settings. Levels: `dev run wpctl get-volume @DEFAULT_AUDIO_SINK@`.
Keys: `dev run tessera-ui ipc call shortcut volumeUp` (what the bind runs).

Glass (island HUD on, the default):
1. Wait 3 s after start. `ipc call shortcut volumeUp`: the island shows the
   volume row with the new level (shot within 300 ms). PASS if the island
   shows "Sound" and a level matching wpctl. Before the fix: nothing showed.
2. `brightnessUp`: the island shows "Display" with the level.
3. `micMute`: the island shows "Microphone off".
4. Mute (`volumeMute`), then `volumeUp`: wpctl shows not MUTED and +5%.
5. Run `volumeUp` 6 times in one command (`for i in 1..6; tessera-ui ipc call
   shortcut volumeUp`): +30% (not less).

Glass, island HUD off (Settings → island hud false, or config
`island.hud=false`):
6. `volumeUp`: the capsule under the bar at the right, fades/grows in
   (220 ms); hides after 1.6 s. Hover it (glide onto it): it stays while the
   pointer rests; leave: gone 1.6 s later.
7. Drag its level 20%→80% (pointer down on the thin bar, glide, up):
   follows with no backward jump; wpctl ≈ 0.8. Wheel ±5%.
8. Click its icon: mutes (icon speaker-x, level empty). Click again: unmutes.
9. Click the caret button: Outputs list below. Click "Sound settings":
   Settings opens on Sound and the OSD closes.

Material (`--look material`):
10. `volumeUp`: the panel springs in at the right edge (overshoot), hides
    after 1.6 s; record and compare with the QML (enter ≈ springDefault,
    exit 200 ms).
11. Hover it, drag the tall slider bottom→top: the value pill beside the
    handle shows the number, handle thins to 2 px, level follows the
    pointer (no backward jumps); wpctl follows.
12. Wheel over the slider ±5%. Mute toggle: squarer tertiary when muted.
13. Sliders button: the card with outputs opens to the left; pick another
    output (needs two sinks: `dev run pactl load-module module-null-sink
    sink_name=second`): ticked; 700 ms later a toast with its name.
14. With something playing: App volumes ›, drag the app's level: follows;
    click its icon: mutes (dimmed).
15. `brightnessUp` (Material): the panel shows the sun, no mute toggle;
    Displays card: Built-in display row with its level, drag it.
16. Fullscreen (`hyprctl dispatch fullscreen` on a foot window): `volumeUp`
    shows the thin bar at the top centre, fades out after 1.2 s.
17. Open the Control Center and drag its Sound slider: no OSD appears (hushed);
    1 s after letting go, `volumeUp` shows the OSD again (not hushed forever).

## Test results

(tester)
