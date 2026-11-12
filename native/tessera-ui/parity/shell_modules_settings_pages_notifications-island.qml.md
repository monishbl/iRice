# shell/modules/settings/pages/notifications-island.qml ↔ surfaces/settings (src/pages/notify/notifications.rs `island`)

The Island subpage (page `notifications/island`, title "Island"). No Popup,
no Timer.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Page title "Island" (11) | `p.title` 342 | same |
| 2 | Group "Island" + footer; "Island" switch → island.enabled (13–20) | 343–349, 403–407 | same |
| 3 | "Live activities" (only while on) Segmented 270 Replace spaces/Around/Beside ↔ replace/split/beside + detail (21–32) | 350 | same |
| 4 | While on: 17 alert switches (labels as QML, same order); checked = Island.wants(kind) (newer kinds on until touched); toggling rewrites island.events and adds the kind to island.known (34–55) | 352–402, `wants` (tested) | same |
| 5 | "network" when NetworkAlerts can't watch: label secondary, detail = reason, switch off and disabled (40–46) | 378–399 (`feed.networkCanWatch/networkReason`, tone 1) | same |
| 6 | More › "When several are live" Segmented 200 One at a time/All + detail (64–75) | 428–432 | same |
| 7 | More › "Music player" Segmented 240 Island/Faded/Small: Small → playerLayout side; else playerArt = v, playerLayout overlay; current 2 when side; detail per state (76–94) | 433–443 | same |
| 8 | More › 12 "… shows" rows, each a **Flow 320 wide of Chips that wraps onto more lines**, chips **not solid** (Glass: on = accent tint with a 1 px accent edge, label colour text; Material filter chips with a tick); checked = Island.shows; click = Island.setShows(kind, piece, !checked) (95–130) | 445–477 `.chips(..).width(320)`, `shows`/`set_shows` (tested) | **different**: the chips control is one HorizontalLayout – no wrapping (Music's 7 chips run far past 320 px and push into the label), `width` ignored, and SChip `solid` is always true (Glass on = solid accent with white text) |
| 9 | More › "Running hot at" (on and temperature wanted): slider 70–100 step 1 (220), detail "N °C · the CPU package is M °C now / not reporting", live temperature (131–143) | 479–490 (`feed.temperature`) | same |
| 10 | More › "Keep the island open", "Volume and brightness in the island" switches + details (only while on) (144–155) | 491–492 | same |
| 11 | More › "In full screen" Segmented 330 Show alerts/Only important/Compact pill ↔ show/important/compact (Island.fsMode: unknown → show), detail per value (156–169) | 493–507 | same |

## Test plan

Setups as in pages_menubar (both shells, both looks). `open notifications/island`;
read y from a shot.

I1 Page: shot; switch the Island off. PASS: only the Island switch row and
footer remain; on again: Live activities and the 17 switches return.

I2 Alerts: switch "USB devices and drives" off then on. PASS: island.events
loses/gains "usb", island.known gains "usb" (`run 'jq .island ~/.config/tessera/config.json'`), same in both shells.

I3 Chips: open More options, scroll to "Music shows", shot. PASS (as QML): the
7 chips wrap onto two or three lines inside a 320 px column right of the
label; Glass: on chips accent-tinted with an accent edge (not solid accent),
off chips grey fill; Material: on chips secondaryContainer with a tick. Click
"Album art": toggles; for "Calls show" (a newer kind) the first click writes
the defaults out (island.livePieces gains call.icon, call.name, call.time, call.mute + the change).

I4 Music player: click "Small" → detail "Small album on the left…", config
playerLayout "side"; "Faded" → playerArt faded, layout overlay.

I5 Running hot: with "CPU running hot" on, the row shows; detail's temperature
updates (wait 5 s, shot twice).

## Fixed in batch 2 (fixer)

- The pieces' chips wrap onto more lines within 320 px (Row::wrap: each chip's line worked out from its estimated width) and aren't solid (Glass: accent tint with an accent edge when on).

## Test results

(tester)
