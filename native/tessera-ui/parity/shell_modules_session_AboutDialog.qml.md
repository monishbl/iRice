# shell/modules/session/AboutDialog.qml → surfaces/session ("about" in ui/session.slint, `rows` in src/lib.rs)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Same Dialog as the session dialog, 300 wide; opens on Dialog's spring per look, closes 192 ms exit | 12-50 | `which == "about"`, `animate progress` | **was** 420 ms spring(0.12) / spring(0.3); now 312 ms spring(0.1) Material, 388 ms spring(0.23) Glass |
| 2 | Logo, name, version; rows (system, kernel, processor, memory, display…) label left, value right | 55-110 | `rows` | same |
| 3 | Esc or a click outside closes | 110-125 | keys, `dismissed` | same |

## Test plan (tester)
Both looks. `inbox 'ipc panels open about IRICE-TEST'`.
1. Shot vs QML: the same rows and values, labels left, values right.
2. Record the open (1 s) and compare the curve as in SessionDialog.
3. `-k Escape` closes.
