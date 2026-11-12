# Parity: shell/modules/menubar/Workspaces.qml

The Spaces strip (on the bar when the island is off, inside the island
otherwise): one chip per space, apps' icons or dots, arrows or dynamic fade
past `workspaceCount`, other displays' groups with their names. Click to go,
right-click to move a space to another display, scroll to step.
Rust: `surfaces/menubar/ui/island.slint` `Spaces` (232-393), chip geometry in
`src/lib.rs:581-648`, strip/groups in `src/spaces.rs` (+ `tessera_core::spacemap`),
actions `src/lib.rs:1085-1094` (go), 1196-1204 (menu, arrows, wheel),
space menu rows `src/menus.rs:225-232`.

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Groups: own spaces, guest (unplugged display) spaces, other displays' (menubar.otherSpaces) with name marks | 45-72 | spaces.rs:1-55, lib.rs:581 | same |
| Overflow window of `workspaceCount` chips that keeps the active one in view (`keepActive` on active/ids/cap change) | 78-97 | spaces.rs:55 (`strip`, start kept in BarEntry.start); lib.rs:582-583 | same |
| Every monitor's active space stays visible outside the page | 101-109 | spaces.rs `strip` | same (verify) |
| Arrows mode: arrow at each end, faint (0.25) when nothing past, dot when something past has windows; caret 10 px bold, 0.7 → 1 on hover | 136-153, 287-312 | island.slint:268-317 | same |
| Arrow click shifts the window by one (enabled only with more); hit area grows 4 px each side | 313-319 | island.slint:288-291, 313-316; lib.rs:1198-1203 | different: Rust hit area is the 12 px arrow only (no −4 margins) |
| Dynamic overflow: chips past the window shrink (`fall` 0.7, 0.45, 0) and fade (0.6, 0.3) | 100, 191-201 | lib.rs:598-603, 621; island.slint:324-325 | different: Rust narrows the width only (height stays 18); QML scales the whole chip (`scale: fall`) |
| Behavior on scale: popover (220) OutCubic; on opacity: popover | 202-203 | island.slint:326-327 (x/width; opacity 220) | different (scale not animated as such) |
| Chip width: Glass icons n·14+10 / active or shownThere 26 / 18; Material 30 / 22 / 14 | 208, 212 | lib.rs:606-620 | same |
| Glass width Behavior: popover OutCubic | 226 | island.slint:326 (Theme.popover, ease-out) | different: ease-out ≠ OutCubic (small) |
| Material width: SpringAnimation springFast (spring 12.8, damping 0.54, ζ 0.6 k 800, epsilon 0.25) | 207-211 | island.slint:326 (420 ms `spring(0.2)`) | different: not matched to springFast |
| Chip colour: active fg 0.28 (hot: tinted with red 0.45), shownThere 0.22, hover 0.14; Material primary / secondaryContainer / onSurface 0.08 hover; Behavior colour hover (150) | 216-217, 227 | island.slint:349-352 | same except Glass hot: Rust #ff453a at 0.55 instead of `Qt.tint(fg 0.28, red 0.45)` |
| Icons style: up to 3 app icons 12 px, spacing 2 | 204-205, 229-241 | lib.rs:604; island.slint:354-366 | same |
| Dots: Glass 5×5 empty (0.35) / 10 occupied (0.85) / 14 active; Material 8 h, 8/14/22 wide, primary / secondary / onSurfaceVariant (0.38 empty) | 242-256 | island.slint:367-374 | same |
| Dot width Behavior: Glass hover 150 ms (linear); Material springFast; Material colour Behavior hover | 251, 257-258 | island.slint:373 (Glass 150 ms ease-out; Material 420 ms spring(0.2)) | different: Material dot colour not animated; spring not matched |
| On another display: 8×1.5 bar under the chip (0.9 shown there / 0.5) | 260-270 | island.slint:376-382 | same |
| Group mark (display name) before a group's first chip, 10 px Medium, 0.6 | 168-185 | island.slint:334-343 | same |
| `activeCenter` reported for the island's heat ripples | 22, 218-225 | — (island.slint:1180: a glow instead, no ripples) | missing (follows from the island having no ripples) |
| Left click → `Spaces.go(id)` (made on its display if it doesn't exist yet) | 117-122, 276-281 | island.slint:389; lib.rs:1085-1094 | same |
| Right click → `openMenu`: "Move to <label> (<name>)" per other display with a base, else disabled "Only one display is connected" | 276-278, 324-335 | island.slint:384-388; lib.rs:814-826; menus.rs:225-232 | same rows |
| Space menu is a ContextMenu: centred on the chip (`pointX - width/2`), y 30 downward, spring 5 / 0.36 with scale 0.94→1, Exclusive keyboard, Esc/arrows/Return, outside click; no autoClose | 336-349 (+ glass/ContextMenu.qml) | lib.rs:1197 → `open_menu(Space)` (the bar Menu: left edge at the chip's centre, MenuPopover look, 5 s autoClose) | different: position (left-aligned to chip centre, not centred), look (bar menu vs context menu), adds 5 s autoClose, different spring |
| Wheel: down → `m+1`, up → `m-1` (this display's spaces), accepted (bar volume scroll doesn't fire) | 351-353 | island.slint:258-266; lib.rs:1204 | same |
| Capsule: Glass fg 0.1, Material surfaceContainerHigh, radius h/2; none when `plain` (in the island) | 124-129 | island.slint:251-254 (Theme.pill = surfaceContainerHigh) | same |
| Accessibility | — | island.slint:255-256, 328-332 | Rust extra |

## Summary (missing / different, by priority)

1. Right-click space menu: Rust opens the bar MenuPopover left-aligned at the chip centre with a 5 s autoClose; QML opens a centred ContextMenu (scale 0.94→1, spring 5/0.36, no autoClose).
2. Dynamic overflow shrinks width only; QML scales the whole chip (scale animation 220 ms OutCubic).
3. Material width/dot springs not matched to springFast (ζ 0.6, k 800); Material dot colour not animated.
4. Arrow hit area lacks the 4 px margin.
5. Glass "hot" active-chip colour approximated (#ff453a 0.55).
6. `activeCenter` (heat ripple origin) not reported (the Rust island draws no ripples).

## Batch 2

The space menu is centred on its chip, no autoClose (MenuBar.qml.md test plan step 6). Look/spring still the bar menu.

## Batch 3

Context-menu scale 0.94 to 1 about the top centre; dynamic overflow scales the chip as a whole; Material widths on a fast spring (MenuBar.qml.md batch 3 steps 12-13).

## Fixed in batch 4 (fixer, 7 Nov)

- The space menu now draws as ContextMenu does in either look: the Glass-style rows (28 px, headers 26) with the theme's highlight, not Material's grouped rows, centred, scaling 0.94 → 1.

## Test plan (batch 4)

- In Material, right-click a space chip: the menu's rows are plain 28 px rows with a highlight (no Material groups/pills), centred on the chip.

## Test results (batch 4)
