# Parity: shell/modules/lock/LockMaterial.qml

The Material look's lock (Android 16): lock icon / face spinner, battery in
the corner, LockClock + date/weather + smartspace, Now Playing (LockPlayer),
notifications (shelf / segmented cards), the unlock pill (avatar cookie,
name ↔ field, send, spinner, fingerprint target), error pill, reader line,
the light reveal.
Rust: `surfaces/lock/ui/lock.slint` `MaterialLock` (+ `MClock`, `Wavy`),
`src/lib.rs` (`settings` inks, `player` position).

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Inks: primaryFixed / Dim for the clock, secondaryFixed / Dim for text; the on-… tones on a light wallpaper | 44-48 | `settings()` `m_*` inks | same |
| Clock size: column 20 %h/12 %w; iOS 17 %h; macOS 15 %h (≤ 20 %w) | 51 | `clock-size` | same |
| Only the large stacked clock morphs into StandBy | 57 | `LockScreen.morphable` | same |
| Small clock (lock.clockSize small): smallT spring (springDefault) into the top-left corner | 61-67, 143-148 | `small-t` (450 ms spring) | different curve: Slint spring vs Qt SpringAnimation (6.08, 0.5) — tester measures |
| Focus to the field at once, after a check, back from StandBy | 72-75, 939-943 | `init`, `busy-now`, `sb-now` | same |
| Motion: rise 32 px with zoom, part = 5 % height × exit (top lifts, bottom drops), opacity 1 − exit | 79-90 | `rise`, `part`, `content.opacity` | same |
| Lock icon 22 px; face looking → Material loading indicator 36 px; accessible "Looking for your face" / "Locked" | 92-115 | lock icon `Rectangle` (`Busy`) | same |
| Battery corner (iOS): charging / low / high icon + % | 117-138 | `if ios && has-battery` | same |
| LockClock: face, size, small size min(72, 7 %h), weight thin→set with the light reveal, round, inks, align left when small/column, ticks except StandBy | 150-177 | `MClock` props | same (weight only; no width/round axes in Slint) |
| Date (hidden for dynamic + 24 h while large), weather icon + ° (iOS) | 180-212 | `lines` first row (`height: self.preferred-height`) | **fixed** (batch 1: `lines` filled the header, so its rows stretched and the date sank under the shelf, worst with the tall overlap and weather faces) |
| Smartspace: next event "Title · time", to-dos "N to-do(s)" | 213-262 | `lines` second row | same |
| Now Playing card 80 px: art cookie (playing) / circle (paused), title/artist, timeline, prev / play (squares off while playing, grows on press) / next | 264-283, 760-930 | `if LockData.has-player` card | same |
| Timeline: position read once a second while playing; **wavy** (amplitude 3, wavelength 40, 1 wl/s, flat near ends) while playing, flat bar paused | 787-848 | `player()` reads MPRIS Position once a second on a thread (`read_position`), `Wavy` | **fixed** (was never shown); wave is quadratic arcs (≈ sine) |
| Shelf (52 px pill, 40 px icon circles, +N) and segmented cards (72 px, 24/6 corners, 2 apart; 1 or 4 in column) | 285-451 | notifications `VerticalLayout` (Rows) | same; lists change in place |
| "N more" (>4 in column) | 444-450 | `if split && note-count > 4` | same |
| Error pill above the unlock pill (warning icon, error colour, wraps at 460) | 467-495 | error `Rectangle` | same |
| Pill width: rest = avatar + name (+ reader) ↔ open 360 by openT spring | 497-503, 945-951 | `pill-w`, `open-t` | same (spring curve as above) |
| Pill shake on a new error: spring from 18 (24, 0.19) | 505-516, 931-935 | `shake` = 18·e^(−6t)·cos(2π·6t), 700 ms | same at a glance — tester measures |
| Pill border 2 px: primary when focused & open, error with an error | 518-526 | `border-width` / `border-color` | same |
| Avatar in a cookie; a 40° turn while checking (spring) | 500-531 | avatar `transform-rotation` (600 ms spring) | same |
| Name ↔ field cross-fade with openT; field: dots (•), letter spacing 2, 18 px; placeholder "Enter password"; every key opens the pill | 533-584 | `name-t`, `input` (`key-pressed` → wake) | same |
| Send: circle, filled with text, hover/press tint, press grows, pointer cursor only when ready; spinner in its place while checking | 586-626 | `send`, `Busy` | same |
| Fingerprint target: circle → cookie on match; pulse (spring from 1.15), miss shake (from 8), ripple ring 600 ms on match; colours by phase | 628-723 | `finger` | same |
| Click on the closed pill opens it (I-beam) | 725-734 | `if !open: TouchArea` | same |
| Reader/camera line under the pill (blank while just listening), 150 ms fade | 739-764 | last `HorizontalLayout` | same |
| Pill opens on a key or a pointer move (>2 px, not the first position), closes 10 s after unless typed / busy / error | 945-972 | `TouchArea moved-by`, `wake()`, `idle` 10 s | same |
| Light reveal: black ring whose hole grows from the centre (zoom) | 975-996 | last `if zoom-in && reveal < 1` | same |

## Test plan (arena N, `appearance.look: material`)

1. **Light reveal.** `ipc lock lock`, record 1 s: black with a circle opening from the centre over ~450 ms (emphasized), the clock thickening from thin.
2. **Pill.** After lock: pill closed (avatar cookie + name). `pointer glide 900 500 300` → opens to 360 px (spring), placeholder "Enter password"; wait 10 s → closes. `run wtype x` → opens, one dot; backspace; wait 10 s → closes. Click on the closed pill → opens.
3. **Wrong password**: `nope` + Return → spinner in the send slot, avatar turns 40°, then error pill above ("Incorrect password. Try again."), pill border red, pill shakes; stays open.
4. **Right password** `mock` + Return → unlocks; record the exit: top part lifts, bottom drops (5 % height), 220 ms.
5. **Fingerprint** `demo finger` → target circle (secondaryContainer) pulses; `demo miss` → errorContainer, shake, line "Not recognised. Try again"; `demo match` → cookie + check + ripple ring growing to 280 px over 600 ms.
6. **Face** `demo face` → the lock icon at the top becomes the loading indicator; line "Looking for your face…" with the smiley.
7. **Clock faces**: for `lock.clockFace` in stacked, dynamic, wide, overlap, analog, weather: shot; analog: the hands point at the time. `lock.clockSize: "small"` → clock springs into the top-left (record; compare duration with QML).
8. **Now Playing** (a player with length, e.g. `mpv --no-video --loop file.ogg` if available): card above the pill; timeline visible within 1 s and **wavy, moving** while playing; pause (click the play button) → button rounds, timeline flat; position advances once a second (two shots 3 s apart).
9. **Notifications**: as LockGlass step 3; Material shapes (52 px pill, 72 px cards).
10. **Minute tick**: at a minute change (StandBy off), record 1 s: the minutes thin and swell back (~600 ms).

## Test results

(testers fill in)
