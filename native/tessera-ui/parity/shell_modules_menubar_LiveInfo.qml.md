# shell/modules/menubar/LiveInfo.qml

Singleton: what each live activity kind says and offers (icon, title,
detail, time, progress, controls), shared by IslandLive, IslandActivity and
IslandFocusCard. Rust: data, not UI. Standalone, `host/shell/src/island.rs`
`derive_live` (446-530) derives recording, timer and stopwatch from host
state; the other kinds come from their owners via `island.activity`
(936-950) or, in QML-shell mode, RustBridge.pushLive; `push_live`
(533-577) sends them with `detail`, `progress` and `all`. Music is built in
the menu bar from MPRIS (`surfaces/menubar/src/lib.rs` 301-310, 363-401).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Seconds clock while something counts (recording, call, share, dictation) | 14-21 | host island.rs:568-576 (1 s Tick::Live, re-derives) | same |
| icon(): recording pause-circle / record, timer timer / coffee, stopwatch, transfer by kind, call camera / phone, share window / screen, sysupdate, dictation, casting / captions, mirroring | 23-50 | derive_live (recording, timer, stopwatch); island_live.rs `derive` (the other eight, from their owners' states) | same (17 Nov: every kind) |
| title(): music track or "Playing", Focus / Break, Stopwatch, Recording (paused), transfer "+ N more", call name, sharing, Updating · step, dictation, casting, captions, mirroring | 52-82 | derive_live; island_live.rs; lib.rs (music: title, else "Playing") | same (17 Nov: every kind; music fixed batch 5) |
| detail(): artist, "m:ss left", stopwatch, recording time, transfer what · amount · speed (· PIN), call since · Muted, share target · since, captions error / setup / previous, mirroring, casting, dictation, sysupdate | 84-122 | derive_live; island_live.rs (transfer what · amount · speed · PIN, call since · Muted, sharing target · since / still sharing, captions error / setup / previous, mirroring, casting, dictation, update text · package) | same (17 Nov: every kind) |
| time(): short pill text per kind | 125-147 | derive_live 478, 493, 510 | same |
| progress(): timer, transfer, sysupdate, casting; −1 otherwise | 150-154 | derive_live (timer); island_live.rs (transfer, update, casting) | same (17 Nov: every kind) |
| controls(music): prev (canGoPrevious) / play-pause / next (canGoNext) | 161-166 | lib.rs:371-380, `island_control` 1122-1139 | same |
| controls(timer): reset / play-pause / skip | 167-172 | derive_live 495-499 (`nc` calls) | same |
| controls(stopwatch): reset (on when stopped) / lap (on when running) / play | 173-178 | derive_live 511-515 | same |
| controls(recording): discard, mic (with a mic; on when the node exists), pause (canPause), stop (danger) | 179-186 | derive_live 465-473 | same (mic always on: no micNode check) |
| controls(transfer / screenShare / captions / mirroring / casting / dictation / sysupdate / call) | 187-221 | island_live.rs (transfer cancel / open Downloads, sharing stop, captions off, mirroring stop, casting play / stop, dictation cancel / stop, update details / cancel, call mute / open / leave) → `island.live` runs their acts | same (17 Nov: every kind; nobody sent them before) |
| Pill shows controls filtered by pieces; rows drop reset; the card shows all | 156-157 | push_live 541 (`controls`), 547 (`all`); `live` call `all` flag 929 | same (menu bar side in progress) |

## Summary

1. Music title fallback is the player's identity, not "Playing" (minor).
2. Recording's mic control is never shown disabled (no mic-node check).
3. Kinds other than recording / timer / stopwatch depend on each owner sending icon / title / detail / progress / controls; nothing in this port checks them.

## Fixed in batch 5 (fixer, 8 Nov)

- Music without a title says "Playing" (was the player's name).
- Recording's mic control: the host doesn't report the mic node, so it's never shown disabled (the recorder's state has no micNode).

## Test plan (batch 5)

- FAKEPLAYER_TRACK="|Artist" fakeplayer: the pill and rows say "Playing".

## Test results (batch 5)

## Re-check 17 Nov (B4)

Summary point 3 was the real gap: standalone, no owner ever called `island.activity`, so only recording, the timer, the stopwatch and music ever showed. host/shell/src/island_live.rs now derives mirroring, casting, dictation, captions (in the island), a call, screen sharing, a system update and transfers from the states their services publish (new: `livecaptions`, `privacy.screen`, `sysupdate`), with LiveInfo's icon, title, time, detail, progress and controls, each control's accessible name, a call's app icon and the captions' lines.
