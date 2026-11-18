# shell/modules/controlcenter/QSMedia.qml

Material media card. Rust: `QsMedia` (surfaces/controlcenter/ui/controlcenter.slint), surfaces/controlcenter/src/lib.rs push_footer/want_art, host/system/src/accounts/cc.rs cc.media + tick.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| position read on open and on a player/track change, then 1 s while playing | 35-42 | host/system/src/accounts/cc.rs tick pos_key | fixed |
| seek absolute | 221-230 | host/system/src/accounts/cc.rs seek: offset from a fresh Position | fixed (offset from a value up to 1 s old) |
| seek hit area +6 px | 221-230 | seek TouchArea | fixed |
| bar jumps at once on a seek | 221-230 | surfaces/controlcenter/src/lib.rs held "seek" | fixed |
| WavyProgress while playing (sine, drifting) | 194-200 | Path wave | fixed |
| art kept while loading | 63 | surfaces/controlcenter/src/lib.rs want_art | fixed (batch 2) |
| Space/Enter on the card: play/pause | 134-146 | surfaces/controlcenter/src/lib.rs key | fixed (focus is the card, not the button) |
| cookie / circle art | 68-73 | rounded square | different (documented) |
| carousel of players | QuickSettings 368-377 | — | missing |
| cast label with device | 257 | fixed label | different |

## Fixed in batch 2

- position on open
- absolute seek
- seek reach
- wavy line
- keyboard play

## Still different / missing

- carousel
- cookie art
- cast label

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o TESSERA-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Pause a long track at 1:00, open CC: the bar is at 1:00 (not 0).
2. Play: the done part is a drifting wave; pause: flat.
3. Click the bar at 3/4: it jumps there at once, `playerctl position` ≈ 3/4 of the length.
4. Tab to Now Playing, Space: play/pause toggles.

## Test results

(tester)
