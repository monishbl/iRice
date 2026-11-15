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
| Circle to Search: the edge glow of gradient bands flowing round the screen (Glass blue/purple/pink/orange, Material wallpaper tones), a ring from the pointer or a band rising as it opens (a shimmer tried 14 Nov and taken back; other looks being chosen); the screen holds still while you circle; a glowing trail brightest at the pointer; the circled area lifts out (larger, rounded, a shadow) and shimmers while the AI works; the circled picture goes to an input bar with chips (What is this?, Search the web, Copy text, Translate) or your own question; the answer streams in a card by the selection with "Continue in sidebar"; Lens "image expired" fixed (the Lens fix **also QML**, shared script) | 13–14 Nov | live: shimmer, freeze, trail, lift, input bar with chips and typing checked in arena 2 (Glass and Material); the AI answer card and chip actions unchecked | f204b11 (Lens); 5dff917, 45dffab, 96c1537, 3320b58, d580b65 |
| Control Center (Glass, controls as chips): with a page open (Bluetooth, Wi-Fi…) the other controls fade but keep their glass blur (it dropped to 40% and what's behind showed through sharp) | 14 Nov | live | d580b65 |
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
| Idle CPU 7–12% → 1.4% (15 Nov): the system mirror runs only when a service's state changed and reads fields without cloning; Google doesn't rebuild every task and event each second; one pactl subscribe for the shell (Device alerts and Privacy had their own; orphans leaked on restarts), client events dropped; stream outputs read on change, not each second; Wi-Fi and status menu ticks only while open; dock and bar skip title-only changes; the wake log shows ms by part | 15 Nov | live | 013d565, c88c67d, 282cb06 |
| Speed round 2 (15 Nov): the drag-to-shelf pointer follower stays off while the session is locked (the lock swallowed the release, so a click there ran it at 25 lines a second for a minute); the clipboard keeps its closed window 5 minutes, as emoji (reopening ~84 → ~25 ms); the wake log times panel opens (`panels-act` → `first-frame`, ~tools/open-latency.sh) and names the Sharing service's messages | 15 Nov | live | 46b923a |
| Circle to Search edge light, a setting (Appearance → Circle to Search → Edge light): Dot ripple (default), Glow, Pool, Ripple to rim, Colour vignette, or the Bands; the first five drawn in Rust a frame at a time on a thread of their own (edgefx.rs: a quarter-size light picture, a half-size dot picture), 60 fps for the opening then 20 fps; Material uses the wallpaper's tones made vivid; Reduce motion: one still frame | 15 Nov | live | 513814d |
| Circle to Search, quieter (15 Nov): the edge light reaches half as far and is a quarter fainter; the ripple from where it opened runs 1.5× faster; once something is circled the light draws in to a quarter and the rest of the screen dims; the circled area resizes from grips on its corners and edges (it's searched again when let go) | 15 Nov | done (sandbox: dots, ripple, bands; circled, then resized from a corner) | 553860a |
| Glass edge (plugin): the lens band shows the frosted picture where it bends most (a sharp, squeezed copy of what's just outside, a window edge, the menu bar, a neighbouring tile, read as a second rounded outline inside panels); glass over the desktop's widgets frosts like glass over windows (window frost) | 15 Nov | done (sandbox: the second line 10 px inside the Wi-Fi menu, 131 → 52 on a 38 background); live after Hyprland reloads the plugin | 553860a |
| AI sidebar scrolls to its last reply: wrapped replies are measured at the width they're drawn at (a StyledText left to the layout reported one line, so the chat thought it was 1480 px tall, not ~3700) | 15 Nov | done (sandbox: a 12-message chat reaches its last line) | 553860a |
| Glass in one layer (16 Nov): the plugin blurs its own copy of what's behind (two half-size passes) and the lens bends that one picture; Frost is the blur in px (Appearance → Glass → Advanced, 4–12, 6 by default) for every material; no crossfade of sharp and blurred copies (text behind showed as a ghost), no extra frost over windows, Hyprland's blur never shown under plugin glass. Defaults from the user's pick: tint half, colour ×1.15, lift +5%, a stronger lens (IOR 2 + refraction 2: bend 60 px, band 48 px). An older plugin build still loaded gets frost 1 (its full blur) until Hyprland reloads it | 16 Nov | done (sandbox: Control Center and Wi-Fi menu over icons and text; the new shell on the old plugin) | 7ea15f8 |
| Quick Share from the share sheet: the panels hand the files to Sharing (the sheet said "0 files", never looked for devices, never started Quick Share); Bluetooth headphones that connect become the sound output; Control Center's pages and edit gallery are see-through over the plugin's glass (a panel colour made them solid); no picture of you in Control Center's footer (both looks) | 16 Nov | built and deployed; Quick Share's helper found the user's phone over Wi-Fi when run by hand; the sheet path not yet tried with the phone | 9df8da3 |
| Snappier (16 Nov, from a click timeline in the wake log: press, release, asks, shows, configures, frames): the emoji window stays made once made (its first frame draws every emoji, 1.3–1.9 s with the UI thread held, so an open after its 5-minute retire froze the shell); Screen Time saves off the UI thread (a save waited 119 ms on a busy disk); the island's play/pause shows at once (it waited for the player's own word, a few hundred ms from a browser); the player seeks to the absolute time (pulling back did nothing); album art loads (files without a picture's extension); Control Center's control menus and emoji's skin tones are plugin glass | 16 Nov | done (sandbox clicks: release to frame 10–25 ms once opened before) | (this commit) |
| Sandboxes can't change the laptop's Wi-Fi, Bluetooth, radios or power mode (sandbox/shims) | 14 Nov | done (a test's clicks turned the real Wi-Fi off) | 3320b58 |
