# Parity: shell/modules/menubar/HoverTip.qml

A small tip under a bar item after a moment's hover: its own Overlay layer
window, no input. Glass: glass piece (lens 0.35); Material: plain tooltip,
inverseSurface, 4 px corners.
Rust: one tip per bar inside the bar's own surface — `surfaces/menubar/ui/menubar.slint`
375-422 (state + timers), 552-565 (items feed it), 619-636 (island battery), 641-660
(the tip); glass shape in `src/lib.rs:1727-1731`.

Used by every right-side bar item in MenuBar.qml (util buttons 293, recording 320,
shelf 353, caffeine 371, tray 399, tray arrow 414, Now Playing 435, weather 471,
keyboard 485, battery 528, Bluetooth 547, Wi-Fi 584, sound 601, Focus 619, AI 634,
search 648, Control Center 667, clock 691). Not on: system/app/Window items,
PrivacyDots, Resources (ResourceHover instead).

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Open delay 500 ms after `hovered` turns true (restart on each enter) | 25-34 | menubar.slint:413-417 | different: one Timer `running: tip-want && !tip-open`; moving straight from item A to B can leave it running from A (B's tip early) and an open tip is never re-delayed |
| Close delay 120 ms after `hovered` turns false | 25, 35-39 | menubar.slint:418-422 | same for a single item |
| Position: target's bottom centre + 8 px gap (y 35), clamped to the screen | 30-31, 51-54 | menubar.slint:646-647 (`tip-top: 35px`) | same |
| Moving to another item: old tip closes (120 ms), new one opens after its own 500 ms | per-instance (each item has its own HoverTip) | menubar.slint:557-564 (single shared tip) | different: text and x swap instantly, no close/reopen (macOS-like, not QML-like); order of leave/enter events can also cancel the tip |
| Hidden while the item's menu is open (`hovered: parent.hovered && !parent.active`) | MenuBar 295 etc. | menubar.slint:557 | same |
| Own Overlay layer window, no keyboard, empty input mask | 41-57 | drawn inside the bar surface (Top layer unless auto-hide), outside the input region (lib.rs:1284-1299) | different: under Overlay surfaces (e.g. an open menu) and clipped to the bar window (64 px without island) |
| Fade in + drop 4 px, Behavior `Theme.motion.popover` (220) OutCubic, only on appear | 59-67 | menubar.slint:647-650 (opacity, y; 220 ms ease-out) | different: Rust also fades out (QML disappears at once when the LazyLoader unloads); ease-out ≠ OutCubic |
| Glass: Glass piece radius control, lensScale 0.35, padding 12×7 | 68-76, 55-56 | menubar.slint:642-652; lib.rs:1727-1731 (glass_shape lens 0.35, radius 10) | same (radius 10 vs `Theme.radius.control`: verify) |
| Material: radius 4, inverseSurface, padding 8×4, inverseOnSurface text | 77-82, 88 | menubar.slint:642-655 | same |
| Text: one line, Theme.font.small, tnum | 83-90 | menubar.slint:653-659 | same except tnum |
| Tip hidden while the bar is hidden (auto-hide) | — | menubar.slint:648 (`&& revealed`) | Rust extra (fine) |
| Battery tip under the island's battery figure (island's own HoverTip) | (IslandStatus) | menubar.slint:624-632 | same mechanism |

## Summary (missing / different, by priority)

1. Single shared tip: switching items swaps text/position instantly instead of close-after-120 ms + open-after-500 ms per item; the 500 ms delay is not reliably restarted per item, and leave/enter ordering can cancel the new tip.
2. Tip lives in the bar's Top-layer surface (not an Overlay window): hidden under overlays, clipped to the bar window height.
3. Fade-out on close (QML vanishes instantly); easing ease-out vs OutCubic.
4. No tabular figures (Slint).

## Batch 2

Per-item tips, instant vanish, 220 ms OutCubic appear: see shell_modules_menubar_MenuBar.qml.md "Fixed in batch 2" and its test plan steps 1-2.
