# shell/modules/settings/MoreOptions.qml ↔ settings.slint MoreButton (BlockView kind "more"); model.rs Page::more / end_more / folded_key_of; lib.rs "more:" keys

A page's less-used settings folded away behind "More options".

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Closed each time the page opens (5–6, 13) | lib.rs:511, 579 `local.retain(!k.starts_with("more:"))` | same |
| 2 | A search / link to one of its rows opens it (6–7; Settings.reveal) | model.rs `folded_key_of` → lib.rs:669 opens that key | same |
| 3 | Button row 44 tall (19–21) | MoreButton `height: 44px` | same |
| 4 | Click toggles; label "More options" / "Fewer options" (49, 68) | MoreButton `ta.clicked` → SBus.act → Page::more toggles its local | same |
| 5 | Space / Return / Enter toggle; Tab-focusable; accessible button, name, press (22–31) | MoreButton FocusScope `key-pressed`, accessible-* | same |
| 6 | Material: tonal pill 40 high, label+32 wide, secondaryContainer + 8 % hover / 10 % press layer; Glass: 30 high text button at x −4, label+16, fill on hover only; ColorAnimation 150 ms (33–42) | MoreButton `pill` | same |
| 7 | Text 14 px Material / body, 500; onSecondaryContainer / accentInk (47–52) | `lr` Text | same |
| 8 | Caret-down 12 px bold turns 180° when open, 150 ms **Easing.OutCubic** (53–61) | settings.slint:455 `animate transform-rotation { duration: SM.hover; easing: ease-out; }` | **different** (Slint `ease-out` ≠ OutCubic: use `ease-out-cubic`) |
| 9 | Pointing-hand cursor (67) | `mouse-cursor: pointer` | same |
| 10 | Focus ring round the pill, radius +4, keyboard focus only (71–74) | SFocusRing | **different** (shared SFocusRing geometry: see SettingsButton #12) |
| 11 | Body: its groups 22 apart, shown only when expanded (77–82) | Page::more `folding`; `Page::shown` skips folded blocks; page gap 22 | same |

## Test plan

M1 `open about` (Material and Glass, both shells), `shot m1-…`; read the "More options" position.
M2 `REGION="<x>,<y> 240x60" clicks m2 "<x y>" "<x y>"` (the button twice). PASS: the caret turns over in ~150 ms, fast start / slow end like the QML recording (`python3 tools/anim-timing.py`), groups appear and go, the label flips.
M3 Hover: glide onto the pill and off. PASS: Material layer / Glass fill fades ~150 ms.
M4 `open appearance` then `open about`. PASS: closed again.
M5 Type a row label that lives inside More options in the search field, click the result. PASS: More options opens, the row flashes.
M6 Keyboard: Tab to the button, `run 'key -k space'`. PASS: toggles; the focus ring shows 2 px out (Glass 2 px wide, Material 3 px) as the QML.

## Fixed in batch 3 (fixer)

- The caret turns on OutCubic.

## Test results

(tester)
