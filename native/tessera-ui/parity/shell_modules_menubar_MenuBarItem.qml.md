# Parity: shell/modules/menubar/MenuBarItem.qml

One bar entry: icon / image / drawn glyph / text, unread mark, badge, a
rounded highlight on hover, press and while its menu is open.
Rust: `surfaces/menubar/ui/menubar.slint` `Item` (45-224, right side) and
`LeftItem` (227-265, left side); data from `src/items.rs` `Entry`.

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Registers with Panels (barItemAt: a popover forwards clicks on the bar strip to the item) | 29-30 | — (Rust popovers leave the 30 px strip to the bar: lib.rs:1699, statusmenus lib.rs:1012) | different mechanism, same effect for clicks on items; see Popover notes |
| Size: content + 18 px wide, 24 px tall | 32-33 | menubar.slint:52-53, 235-236 | same |
| Highlight: Glass alpha 0.22 active / 0.2 pressed / 0.12 hovered; Material 0.16 / 0.1 / 0.08; radius item / pill | 37-40 | menubar.slint:54-57, 237-238 | same |
| Behavior on colour `Theme.motion.hover` (150 ms) | 41 | menubar.slint:58, 239 (`Theme.hover` 150 ms) | same |
| Unread mark before text: bellDot (bell 14 px + 7 px dot at x 9), dot, count pill, bellCount; Material primary / Glass system blue | 49-109 | menubar.slint:78-126 | same |
| Count text tabular figures (`tnum`) | 105 | menubar.slint:116-123 | different: Slint has no font features (not tabular) |
| IconImage 16 px (tray icon), optional tint (`MultiEffect colorization`) for a distro logo | 111-122 | menubar.slint:127-136 | different: no tint path (distro logo not supported) |
| Glyph loader (battery, Wi-Fi) | 123-128 | menubar.slint:137-164 | same |
| Icon 15 px (`iconSize`), fill, colour | 129-136 | menubar.slint:165-171 (15 px; LeftItem 16 px) | same |
| Text: Material 14 px / Glass body; weight Bold→ Material DemiBold / Glass Bold, else Medium; tnum | 137-147 | menubar.slint:172-179 | same except tnum |
| Badge bubble top-right (danger, 9 px bold, 99+) | 150-164 | menubar.slint:208-223 | same |
| Hover only trusted after the pointer moves again once `Panels.open` changes (`hoverFresh`) | 166-174, 181-182 | lib.rs:1610-1619 (PointerExited dispatched to bars on open change) | same |
| MouseArea hover, accepts Left+Right, `clicked(mouse)` on release | 175-180 | menubar.slint:63-71 (left `clicked`, right via pointer-event up while hovered) | same for right-side items |
| Left+Right on left-side items (system/app/Window) | 175-180 | menubar.slint:245-247 (left only) | different: right-click ignored on LeftItem |
| No hover tip of its own (tips come from HoverTip children in MenuBar.qml) | — | menubar.slint:64 (`hovered(on, x, tip)` feeds the shared tip) | same role |
| Accessibility: QML none special | — | menubar.slint:59-61, 240-244 (role, label, expanded) | Rust extra (fine) |

## Summary (missing / different, by priority)

1. Right-click on left-side items (LeftItem) does nothing; QML acts as a click.
2. Tinted image (distro logo) not supported.
3. No tabular figures in counts/text (Slint limitation; note only).

## Batch 2

Right-click on the left items now acts as a click (MenuBar.qml.md test plan step 3).

## Batch 3

Distro logo tinted (MenuBar.qml.md batch 3 step 9).
