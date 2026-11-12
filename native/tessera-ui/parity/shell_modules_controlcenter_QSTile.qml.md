# shell/modules/controlcenter/QSTile.qml

Material tile. Rust: `QsTile` (surfaces/controlcenter/ui/controlcenter.slint ~820-1030), clicks in surfaces/controlcenter/src/lib.rs (`act`, `secondary`, `key`).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| colours on/container/ink | 32-38 | QsTile props | same |
| act: editing→resize, toggle/action, info→page | 46-53 | surfaces/controlcenter/src/lib.rs act() | fixed: an info tile's button (CPU & Memory) opens its page (it asked the host, which ignored it) |
| openDetail / secondary (menu, page, switch) | 55-71 | surfaces/controlcenter/src/lib.rs secondary() | same; the page now opens level with the tile, not the pointer |
| Keys Space/Enter/arrows/Delete | 83-106 | surfaces/controlcenter/src/lib.rs key() | same (autorepeat not filtered: different) |
| Menu key / Shift+F10 | 92-93 | surfaces/controlcenter/src/lib.rs key() | fixed: Shift+F10 handled; opens the page (else the menu at the tile, not at 0,0) |
| corner rest/pressed ×0.7 press spring | 121-125 | QsTile corner | fixed: no squaring while editing (QML tileMouse is off then) |
| hover/press tint 8/10%, 150 ms | 126-135 | QsTile background | fixed: no press tint while editing |
| press-and-hold 550 ms → detail, click suppressed | 213-238 | tile TouchArea + hold | same |
| edit: click resizes, drag moves (QuickSettings DragHandler) | 255-260 | tile pointer-event | fixed: after a drag the tile springs back / to its new slot (it stayed offset by the drag) |
| remove badge | 261-284 | QsTile badge | same |
| FocusRing 3px secondary | 113-115 | QsTile ring | different: 2px primary |

## Fixed in batch 2

- info tile button opens its page
- Shift+F10 / Menu key
- no press squaring/tint while editing
- drag offset after drop

## Still different / missing

- focus ring style
- Space/Enter autorepeat

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Material, open CC. Click a toggle tile: on/off flips, corner rounds to 36 on; record the press: corner shrinks to ×0.7 and springs back.
2. Hold a tile 1 s: its menu or page opens, no toggle on release.
3. Edit controls (pencil): drag a tile 2 slots right and release: it lands in the new slot and **does not stay offset**; drag 1 px-ish (under 3 px): click = resize.
4. While editing press a tile: no tint, corner stays.
5. Make CPU & Memory 2×2 (edit, click it twice), Done; click its round icon: the CPU & Memory page slides in.
6. Tab to a tile with a page (Wi-Fi), press Shift+F10: the Wi-Fi page opens.

## Test results

(tester)
