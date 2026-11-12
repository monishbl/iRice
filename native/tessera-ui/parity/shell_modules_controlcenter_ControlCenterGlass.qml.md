# shell/modules/controlcenter/ControlCenterGlass.qml

Glass Control Center window. Rust: Glass part of `ControlCenter` (surfaces/controlcenter/ui/controlcenter.slint), surfaces/controlcenter/src/lib.rs.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| page ↔ ccPage | 32-59 | surfaces/controlcenter/src/lib.rs/host/system/src/accounts/cc.rs | same |
| panel dim 0.14 while a page is open | 60 | dim-shown | same |
| Escape order | 62-68 | surfaces/controlcenter/src/lib.rs key | same |
| a press outside the page closes it AND reaches the control | 69-72 | GlassModule.pressed-any, body TouchArea | fixed: one click now both closes the page and acts (two clicks were needed) |
| page popover spring, slide 12, scale 0.96 | 81-88 | sub-shown | same |
| page height spring | 107 | sub-h | missing (snaps) |
| close resets editing/page | 175 | surfaces/controlcenter/src/lib.rs close | same |
| gallery rows, reset | 215-308 | gallery | same; no momentum |
| module springs, drag | 345-362 | gm | fixed drag offset |
| footer fades while a page is open | 374-375 | footer | same |

## Fixed in batch 2

- click-through when a page is open
- drag offset

## Still different / missing

- page height spring
- gallery momentum

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Glass: open the Wi-Fi page (right-click). Click the Bluetooth toggle **once**: the page closes and Bluetooth toggles in the same click.
2. Open a page, click the panel's empty area: the page closes.
3. Close the CC with a page open, reopen: no page (record the close: no stutter; longest frame gap ≤ QML's).

## Test results

(tester)
