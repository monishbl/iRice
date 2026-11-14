# Rust-only changes

What the Rust shell (tessera-ui, branch `rust-rewrite`) does differently from
the last all-QML version (`main`, 342a6ac) **on purpose**: features and fixes
the user asked for in Rust only. Parity work (making Rust match the QML) is
not listed here. When the QML is retired these are simply how Tessera works;
until then, don't "fix" them back to the QML's behaviour. Rows marked
**also QML** were later ported to `main` too (13 Nov: the user asked for
the vital ones there).

Status: **planned**, **in progress**, **done** (built and checked in a
sandbox), **done, unchecked** (committed, not yet seen in a screenshot).

## Session

| Change | Asked | Status | Commit |
|---|---|---|---|
| Super+Shift+Escape and Ctrl+Alt+Backspace end the session only on a second press within 3 s; Hyprland's own "Press again to log out" notice, so it works with the shell frozen (`sandbox/confirm-exit.sh`, `sandbox/hypr/tty.lua`) | 13 Nov | done; **also QML** (main 98f3153, live on the laptop) | 59e984e |
| Power menu: only the chosen option stands out (Material primary / Glass accent; Restart blue, Shut Down red when chosen); the others share one quiet shape colour, keeping their own outlines, with Restart's and Shut Down's icons a faint blue and red, both looks | 13 Nov | done (checked both looks, rr-8 / rr8-sess); **also QML**: done (main ef107bc, live on the laptop) | 6d14b2f, quiet tone fix, own shapes |

## Lock and login

| Change | Asked | Status | Commit |
|---|---|---|---|
| Password field as see-through frosted glass (lock and login), light rim, readable text, focus state | 13 Nov | done on the lock (arena 6, both wallpapers); login unchecked | e23fdef |
| Fingerprint animation "ridges trace in": ridges glow in turn while waiting, fill to a tick on success, shake red on failure; static under Reduce Motion | 13 Nov | waiting loop and miss checked; match tick, Reduce Motion and login unchecked | e23fdef |

## Screen tools

| Change | Asked | Status | Commit |
|---|---|---|---|
| Circle to Search: an edge glow of smooth gradient bands (no blur) whose colours flow round the screen (Glass blue/purple/pink/orange, Material wallpaper tones), a ring of light from the pointer (gesture) or a band rising from the bottom (key) as it opens; the screen holds still while you circle; the circled picture goes to an input bar with chips (What is this?, Search the web, Copy text, Translate) or your own question; the answer streams in a card by the selection with "Continue in sidebar"; Lens "image expired" fixed (the Lens fix **also QML**, shared script) | 13–14 Nov | live: glow, entry, freeze, circling, input bar with chips and typing checked in arena 2; the AI answer card, chip actions and Material unchecked | f204b11 (Lens); 5dff917, 45dffab, 96c1537, 3320b58 |
| Image viewer: Super+Q (and the usual close keys) close the viewer instead of the window under it (and the same for every shell overlay that takes the keyboard) | 13 Nov | done, unchecked; **also QML**: done (main 70f3c40, live; Hyprland Lua TESSERA_CLOSE + the shell's `dismiss` shortcut) | 1871e37, bb53564 |
| Image viewer: redesigned top toolbar (larger icons and buttons, grouping, Copy, Mark up, Move to Trash with undo, tooltips), both looks | 13 Nov | done, unchecked | 1871e37 |

| The screen freezes while you pick: Screenshot area, Text Capture, Translate and Ask take the area from the picture taken as the selector opened; the colour picker freezes every screen (hyprpicker -r); recording stays live | 14 Nov | screenshot from the frozen picture checked (arena 2) | 3320b58 |

## Text

| Change | Asked | Status | Commit |
|---|---|---|---|
| Text rendering tuned on femtovg to match the QML's weight and sharpness (not Skia): Material weights under bold drawn at 400 as Qt does, glyphs unhinted as Qt's, plain text on whole pixels | 13 Nov | done; live on the laptop (the user found it sharp after a restart) | aee5291 |

## Glass and transitions

| Change | Asked | Status | Commit |
|---|---|---|---|
| Glass goes out with the frame that's drawn (default sync "drawn"; `TESSERA_GLASS_SYNC=taken` for the old way), held at most 100 ms: Control Center and Launchpad no longer show content before their glass | 14 Nov | done (arena) | bd91803, 65d1961, a703041 |
| Desktop widgets follow Settings → Glass (frost, tint…); only the bevel, shadow and margin are their own | 14 Nov | done (arena 2: Liquid vs Frosted differ as elsewhere) | 45dffab |
| Island (Glass, music): the album art clipped to the glass squircle, no second rounding at the corners | 14 Nov | done, unchecked | 6297768 |

## Other

| Change | Asked | Status | Commit |
|---|---|---|---|
| About This Computer: a Shell row, Tessera (Rust) / Tessera (QML) | 14 Nov | done; **also QML** (f9cb132) | 45dffab |
| Clicks: key-only FocusScopes no longer take the click (Settings links, power mode needed two clicks) | 14 Nov | done (arena) | 45dffab |
| Idle CPU: a window's title changing (a terminal's spinner) re-reads Hyprland's windows at most once a second (it re-read them and redrew the dock on every change); Mouse takes only its events | 14 Nov | done (live: ~10% → ~6% idle while a terminal spins) | 145638a |
| Sandboxes can't change the laptop's Wi-Fi, Bluetooth, radios or power mode (sandbox/shims) | 14 Nov | done (a test's clicks turned the real Wi-Fi off) | 3320b58 |
