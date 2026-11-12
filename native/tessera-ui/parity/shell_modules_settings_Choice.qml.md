# shell/modules/settings/Choice.qml ↔ settings-kit.slint SChoice

A pop-up button for one of several options; its list opens in the page's
ActionMenu (now its own popup: see ActionMenu.qml's parity file).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Width max(140 / 96, label + 52 / 40); height 40 / 28; radius pill / control−2 (24–27) | SChoice min/preferred width, height | same |
| 2 | Disabled: 0.38 / 0.45 opacity (28) | `opacity` | same |
| 3 | Glass colour: pressed fillPressed, hover or open fillHover, else fill, 150 ms (29–31) | `!Theme.material` Rectangle | same |
| 4 | Material: FilledField, outlined while open, hover state (57–64) | `SFilled { active: open; hovered }` | same |
| 5 | Space / Return / Enter / Down open the list (36–41) | `fs` key-pressed | same |
| 6 | The label: the current option's, else the placeholder in secondary; elided (65–72) | `lt` | same |
| 7 | Caret: Material caret-down 20 px turning 180° while open (200 ms OutCubic); Glass caret-up-down 11 px bold (73–81) | SIcon + `transform-rotation` | **fixed in batch 1** (didn't turn) |
| 8 | Click: takes focus, toggles the list (open → close) (82–90) | `ta.clicked` → `fs.focus()`, `show()`; lib.rs M::Menu closes when the same control's list is open | **fixed** (focus, toggle) |
| 9 | Accessible: combo box, its name, the current label as description, press opens (43–46) | accessible-* | same |
| 10 | Focus ring (47–49) | SFocusRing | same |

## Test plan

C1 Caret (Material): record the Choice (REGION around it, ~200×60) while clicking
it open and pressing Esc. PASS: the caret turns over in ~200 ms on open and back on close, as the QML.
C2 Toggle: click → open, click the Choice again → closed (see ActionMenu A4).
C3 Keyboard: Tab to the Choice (or click it and Esc), press Down: the list opens.

## Test results

(tester)
