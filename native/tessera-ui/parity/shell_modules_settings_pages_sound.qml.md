# shell/modules/settings/pages/sound.qml ↔ surfaces/settings (pages/devices/sound.rs, ui/settings-devices.slint `level` / `check`)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Devices and volumes: tessera-ui's own sound-server reading (core services/audio.rs,
`cx.want("audio")`), set there too (Cmd::Sink/SourceVolume, Mute, DefaultSink/Source).
AirPlay / Needs from RustBridgeDevices.qml `feed.airplay`, `feed.needs`, `healthFixing`,
`updatesRunning`; acts airplayInstall, avahiOn. No QQC.Popup on this page.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Outputs = sinks, not streams (AudioStreams.outputs); inputs = Audio/Source nodes, not monitors (16–17) | audio.rs `sinks` / `sources` (monitors dropped) | same |
| 2 | Live levels/mute (PwObjectTracker) (19–21) | `cx.want("audio")` → rebuild on change | same |
| 3 | On show: AirPlay.check() (22) | bridge onPageChanged "sound" | same |
| 4 | Level: IconButton (speaker-high/-x, microphone/-slash) toggles mute (29–34) | sound.rs `level` 42–71, slint `level` GhostIcon 800–809 | same |
| 5 | Level IconButton: Tab focus + FocusRing, Space/Enter, Material press radius → 8 (150 ms OutCubic), hover/press colour animated, ElasticControl (common/IconButton.qml 19–56) | slint GhostIcon 43–59 | **same (fixed before 17 Nov: GhostIcon FocusScope, Material radius 8 pressed, animated colours)** [12 Nov: **different**: no keyboard focus/ring, no Material pressed radius, no colour animation, no press swell] |
| 6 | Level Slider 200 wide; moving it hushes the OSD, unmutes, sets the volume (35–47) | sound.rs 55–64 (`audio.hush`, Mute false, Volume) | same |
| 7 | Slider accessible name "Output volume" / "Input volume" (39) | SSlider label = row label ("Volume" / "Input volume") | **different (a11y name only, remaining): the output slider's name is "Volume"** [12 Nov: different (a11y name only: output says "Volume")] |
| 8 | Output group: "Volume" row, detail = default sink's name (description ‖ nickname ‖ name) (55–59) | sound.rs 82–84, `audio::list_name` | same |
| 9 | One row per output: name, "AirPlay" detail for raop_sink.*; "Use" button, or an accent check (24 Material / 16 Glass) on the one in use (60–79) | sound.rs 85–95, slint `check` 849–853 | same |
| 10 | Output footer about AirPlay delay when speakers > 0 (54) | sound.rs 96–98 | same |
| 11 | Input: "No microphone found" (secondary) when none (86–91) | sound.rs 106–107 (tone 1) | same |
| 12 | Input volume row, detail = default source description (92–97) | sound.rs 109 | same |
| 13 | Input rows with Use / check only when > 1 input; label description ‖ name (98–115) | sound.rs 111–124 | same |
| 14 | Sound effects SubpageLink (bell icon, On/Off value, → notifications) (118–127) | sound.rs 130–132 | same |
| 15 | More options: "Microphone in the Sound menu" switch (controlCenter.micInSound) (133–142) | sound.rs 137–139 | same |
| 16 | AirPlay needs PipeWire row (149–152) | sound.rs 162 | same |
| 17 | Network discovery is off: "Turn On" / "Turning on…" busy (153–162) | sound.rs 163–167 | same |
| 18 | AirPlay speakers row: detail by state, danger label on errors; Install (download icon, busy, disabled while updating) when checked && !installed; Switch (audio.airplay) when installed (163–187) | sound.rs `airplay_detail` (unit-tested) 147–159 | same |

## Test plan (Material window at 440,205; page column from x≈772)

Never click anything that changes the host's sound: the arena may reach the
host's sound server. Only drag sliders / press Use when the output's name is
the arena's own (Dummy / Null output); otherwise hover only.

S1 Look: `setup qml material`, `open sound`, `shot snd-qml-m`; rust the same;
both looks. PASS: Output group (Volume row with mute icon + 200 px slider,
then one row per output with Use or a check), Input group, Sound effects link
showing On/Off, More options. Same texts and order.

S2 Hover: glide onto the mute icon (read x,y from the shot, ≈ 1215,y). PASS:
the 28 px ghost circle (Material) / rounded fill (Glass) shows; press-and-hold
(`run 'P down; sleep 0.3; P up'` only if the output is the arena's own) — QML
corners tighten to 8 and swell; Rust: report what happens.

S3 Keyboard (code check + run): `run 'key -k Tab'` repeatedly from the page
top, shot each. PASS (QML): focus ring lands on the mute button. Rust: report
(expected missing, row 5).

S4 Sound effects link: click it. PASS: Notifications page opens in both; back.

S5 More options: open, shot. PASS: "Microphone in the Sound menu" switch; AirPlay
group with whichever of the three rows the arena's Needs give, same detail
text as QML. Toggle the microphone switch twice: it follows and returns.

## Fixed in batch 2 (fixer)

- Not fixed yet: GhostIcon's keyboard/press shapes; the output slider's accessible name.

## Test results

(tester)
