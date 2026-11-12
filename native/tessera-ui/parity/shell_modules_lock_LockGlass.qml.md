# Parity: shell/modules/lock/LockGlass.qml

The Glass look's lock: date and big clock, iOS widgets row, Now Playing,
notifications (shelf / column / icons), avatar + name + password field,
the error line and the reader/camera line.
Rust: `surfaces/lock/ui/lock.slint` `GlassLock` (+ `Plate`, `Capsule`,
`FingerGlyph`, `Spin`); data from `src/lib.rs` (`clock`, `widgets`,
`notes_push`, `player`, `field`).

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Lifts away on unlock: opacity 1 − exit, scale 1 + 0.08·exit (zoom) | 41-42 | `GlassLock` opacity / transform-scale | same |
| Hidden once StandBy is fully up | 43 | `visible: !(standby && sb >= 1)` | same |
| Plate: liquid glass / frost / solid (+1 px border) | 47-80 | `Plate` (blurred wallpaper + tint + rim; solid) | different (no refraction shader in Slint: frost-like glass with a light rim) |
| Date 24 px semibold, ink2; clock 168 (iOS) / 150 (macOS) bold, −5 tracking, 85 % ink, 12-hour without AM/PM | 83-112 | `header` | same |
| Clock's soft drop shadow (light ink only) | 104-110 | — | different: no text shadow in Slint |
| Header: y 7 % / 10 % of height, drops 20 px into place with zoom, fades with reveal | 86-87 | `header-y`, `opacity: reveal` | same |
| Clock hidden while the morph flies its copy | 99 | `opacity: morph-on && sb > 0 ? 0 : 1` | same |
| iOS widgets row (64 px round plates): battery capsule (charging bolt / plugged), weather icon + °, next event (calendar + time / "Today"), to-dos (list-checks + count); only those that have something | 114-163 | `if ios: HorizontalLayout` | same |
| Now Playing plate 84 px: art 60 (radius 12), title/artist elided, prev / play-pause / next buttons; above the sign-in (shelf) or beside the clock (column) | 165-220 | `if LockData.has-player: Plate` (hover/press tint on buttons) | same |
| Shelf: app icons (≤6, one per app, newest first) + "+N", accessible "N notifications" | 230-275 | `cards` first row (`LockData.apps`, Rows) | same; lists now change in place (Rows) |
| Cards: newest one (shelf) / latest three (column, wide screen) / none (icons); title = summary (previews) else app; body only with previews, tags stripped; time label | 277-327 | `LockData.cards` (per screen, Rows) | same |
| "N more" under the column (>3) | 328-334 | `if split && note-count > 3` | same |
| Avatar 64 (picture or person glyph), full name | 346-363 | `login` first rows | same |
| Field 240×36, radius 18, password dots, placeholder "Enter Password" / "Fingerprint or Password" | 365-419 | `input := TextInput` + placeholder | same |
| Field takes the keyboard at once (and after a check, and back from StandBy) | 403, 409 | `init`, `changed busy-now`, `changed sb-now` → `input.focus()` | same |
| Shake on a new error: 50 ms →10, 80 →−10, 70 →6, 60 →0 | 371-385 | `shake-x` from `animation-tick`, on `error-count` | same |
| Enter submits; arrow button submits (dim while empty); spinner (circle-notch, 800 ms a turn) while checking | 408, 433-456 | `accepted`, arrow `TouchArea`, `Spin` | same |
| Reader glyph in the field while it's empty (pulse / shake / check) | 420-432 | `FingerGlyph` | same |
| Error under the field on **one line**, as wide as it is | 460-466 | `err-probe`, `login-w = max(360, error width)`, `wrap: no-wrap` | **fixed** (wrapped under the field) |
| Reader/camera line (fingerprint / smiley + text), under an error too | 467-488 | last `HorizontalLayout` | same |
| Time labels move with the clock ("now", "5m ago") | 312 | `clock()` → `notes_push` each minute | same; icons cached (no reload each minute) |

## Test plan (arena N, `appearance.look: glass`)

1. **Layout.** `ipc lock lock`, shot. Compare with the QML shot (`--only qml`): date, clock (168 px iOS), widgets row (battery only in the arena unless weather is set), name, field at the bottom 6 %.
2. **macOS layout** (`lock.layout: "macos"`): clock 150 px at 10 %, no widgets row, field 10 % from the bottom.
3. **Notifications.** Before locking: `notify-send -a Firefox -i firefox 'Download finished' 'report.pdf'`, `notify-send -a Files -i org.gnome.Nautilus 'Copied' '3 files'`. Lock. PASS: shelf with both app icons (real icons from the Colloid theme, not blank), one card (newest: Files) "Files" + "Notification" (previews off), time "now". `lock.previews: true` → "Copied" / "3 files". `lock.notifications: "column"` (wide screen) → cards in a column at 74 % across, up to 3, "N more" beyond. `"icons"` → shelf only. Send a third notification while locked: the shelf grows **without the other icons flickering** (region-changes.py over the first icon: no change).
4. **Typing.** `run wtype abc` → three dots; placeholder gone; arrow full opacity. Backspace ×3 → placeholder back, arrow 40 %.
5. **Wrong password** → shake (record; anim-timing ≈ 260 ms) and "Incorrect password. Try again." on one line centred under the field.
6. **Long error**: `demo lockout` → "Too many attempts: try again in about 9 minutes." on ONE line, centred, wider than the field (FAIL if it wraps).
7. **Busy**: `demo busy` → dots + spinner turning (one turn per 800 ms), arrow hidden, field disabled; `demo off`.
8. **Fingerprint in field**: `demo finger` → placeholder "Fingerprint or Password", glyph in the field pulses once; type a letter → arrow back; `demo miss` → glyph red + shake; `demo match` → green check.
9. **Now Playing**: with a player (`playerctl`-able, e.g. `mpv --no-video` on a file if present in the arena), lock: plate above the field; click play/pause (`pointer move` to the middle button, `click`) → the player toggles; hover shows a light tint.
10. **Unlock lifts**: `mock` + Return, record: 220 ms fade + 8 % grow.

## Test results

(testers fill in)
