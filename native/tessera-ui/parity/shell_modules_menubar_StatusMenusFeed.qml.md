# Parity: shell/modules/menubar/StatusMenusFeed.qml

While tessera-ui draws the status menus, what only the shell knows
(BatteryHealth, energy mode, Privacy, Resources, AudioProfiles) goes to the
statusmenus surface, and its requests come back.
Rust: standalone `host/system/src/net/statusmenus_feed.rs` (whole file),
consumer `surfaces/statusmenus/src/lib.rs:877-896` (feed) and its
`shell::ask("statusmenus", …)` calls (lib.rs:685, 712, 783, 929, 932).
Paths below are relative to `native/tessera-ui/`.

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Active only while Rust draws statusmenus and one of its panels is open | 20-22 | host/system/src/net/statusmenus_feed.rs:30-31, 148-151 | same |
| State: health {capacity, cycles, watts, condition}, power {ppd, hasPerformance, profile, short}, privacy {…}, resources {cpu, memory, swap, memUsedGb, memTotalGb, temperature}, profiles [{name, kind, busy}] | 24-35 | statusmenus_feed.rs:68-84, 49-58 | same |
| On a state change while open: send after a 250 ms debounce (Timer `later`) | 40-45 | statusmenus_feed.rs:108-110, 162-165 (500 ms poll, send when changed) | different: 500 ms polling instead of 250 ms debounce (updates up to ½ s late) |
| On opening / switching menu: refresh battery health (battery) or audio profiles (sound), then send at once — before the open reaches tessera-ui | 46-55 | statusmenus_feed.rs:148-160 | different: detected on the next 500 ms tick, so the first contents (health, privacy, resources) can be stale or empty for up to 500 ms after the menu appears |
| Resources samples once a second while the resources menu shows (`graphUsers` ±1) | 56-59 | statusmenus_feed.rs:93-99, 158 | same |
| Resend when tessera-ui (re)starts | 61-66 | — (standalone: in-process, no restart) | n/a |
| Calls: stopSharing → Privacy.stopSharing | 71-73 | statusmenus_feed.rs:126 | same |
| mode <profile> (validated) → Power.setMode | 74-77 | statusmenus_feed.rs:127-132 | same |
| profile <i> → AudioProfiles.pick(alternatives[i]) | 78-83 | statusmenus_feed.rs:133-139 | same |
| btFix → Backends.fixBluetooth | 84-86 | statusmenus_feed.rs:140 | same |
| pin <bus> → Media.pinned (matching dbusName or desktop entry) | 87-92 | statusmenus_feed.rs:141 (`media.pin`) | same (desktop-entry match: verify in the media service) |
| After a call, resend if open | — | statusmenus_feed.rs:144-146 | Rust extra (fine) |

## Summary (missing / different, by priority)

1. Opening a status menu: QML sends fresh readings before the menu shows; the host waits for its 500 ms tick (stale/empty health, privacy, resources for up to 500 ms; battery health refresh late).
2. Updates while open: 500 ms poll vs 250 ms debounce.

## Fixed in batch 5 (fixer, 8 Nov)

- Opening a status menu: the host checks after every turn (`settled`), so the readings go to the menu in the same turn Panels opens it (they waited for the next 500 ms tick); updates while open every 250 ms when changed (StatusMenusFeed.qml's debounce).

## Test plan (batch 5)

- Open the battery menu: its health line is filled on the first frame (record the opening: no empty → filled change). Resources menu: figures present at once.

## Test results (batch 5)
