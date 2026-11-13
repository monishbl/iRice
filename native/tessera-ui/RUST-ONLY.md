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
| Circle to Search: breathing edge glow (Glass blue/purple/pink/orange, Material wallpaper tones), entry from the cursor (gesture) or the bottom (key); the circled picture goes to an input bar with chips (What is this?, Search the web, Copy text, Translate) or your own question; the answer streams in a card by the selection with "Continue in sidebar"; Lens "image expired" fixed (the Lens fix **also QML**, shared script) | 13 Nov | Lens fix committed (cause: the upload's session wasn't the browser's; now the browser uploads via a one-time local page; real-browser check pending); circle.rs + circle.slint built (merged with TESSERA_CLOSE/dismiss, both kept); sandbox (Glass): breathing glow, hint, circling, selection, input bar with thumbnail and chips, Esc close all seen working; a chip click and typed text drew no change (answer card, stub AI, sidebar hand-over, error states, Material, pointer entry: unverified), so **disabled** (`circle::ENABLED = false`: "circle" opens the region selector's circle mode as before) until the bar's input is fixed | f204b11 (Lens); 8264ed4 + 2246372 (Circle, off) |
| Image viewer: Super+Q (and the usual close keys) close the viewer instead of the window under it (and the same for every shell overlay that takes the keyboard) | 13 Nov | in progress; **also QML**: done (main 70f3c40, live; Hyprland Lua TESSERA_CLOSE + the shell's `dismiss` shortcut; Rust needs only to register `dismiss`) | |
| Image viewer: redesigned top toolbar (larger icons and buttons, grouping, states, tooltips), both looks | 13 Nov | in progress | |

## Text

| Change | Asked | Status | Commit |
|---|---|---|---|
| Text rendering tuned on femtovg to match the QML's weight and sharpness (not Skia). Causes found: Material weights (Qt ignores weights < bold for Google Sans Flex, Slint applies them: up to +28% ink), Glass hinting (femtovg hints, Qt doesn't). Fixes written, not built (worktree-agent-ab514c47608928d27, uncommitted) | 13 Nov | in progress | |
