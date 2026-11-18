# shell/modules/controlcenter/PointerMenu.qml

Choices at the pointer. Rust: the menu at the end of `ControlCenter` (surfaces/controlcenter/ui/controlcenter.slint), surfaces/controlcenter/src/lib.rs show_menu, key().

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| opens at the pointer; from the keyboard at the module | 15-43 | menu-x/y | same |
| flips up when no room below | 50-51 | menu y | fixed |
| open animation: fade + scale 0.94→1 on a spring | CM 36, 74-76 | menu p | fixed |
| keys: Up/Down (skip separators), Home/End, Enter/Space pick, Escape | CM 80-103 | surfaces/controlcenter/src/lib.rs key menu_kbd | fixed |
| outside click dismisses | CM 59 | TouchArea | same |
| scroll when taller than the screen | CM 70-73 | — | missing (CC menus are short) |

## Fixed in batch 2

- keyboard
- animation
- flip up

## Still different / missing

- scrolling

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o TESSERA-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Right-click Focus (menu): the menu fades/grows in (record ~300 ms).
2. Down, Down, Enter: picks the second item (highlight follows; separators skipped). Escape closes.
3. Right-click a module near the bottom of the screen: the menu opens upwards.

## Test results

(tester)
