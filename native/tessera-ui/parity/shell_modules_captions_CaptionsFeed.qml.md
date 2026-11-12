# shell/modules/captions/CaptionsFeed.qml → surfaces/captions/src/lib.rs (`Feed::from`, `message`)

The shell's half: tells tessera-ui the Captions state and does what it asks.

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Sends on, placement, screen, latest, previous, hint, dim, bar {x,y}, video {x,y,w,h}, lines {hint, previous, latest} on every change | 10-50 | `Feed::from` (test `the_feed_reads_what_the_qml_sends`) | same |
| 2 | Asks `["off"]` (× → Captions.cfg.enabled = false) and `["place", x, y]` (bar let go → barX/barY) | 50-66 | `off`, `place` | same |

## Test plan (tester)
1. `inbox 'qs ipc call captions on'`, then `demo "a" "b"` → the bar shows "b" with "a" dimmer. Click × → captions turn off.
2. Drag the bar 200 px up (`P down` on it, `P glide`, `P up`); `captions off`, `captions on` → it comes back where dropped.
