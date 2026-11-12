# Parity: shell/modules/lock/StandBy.qml

Bedside Mode on the lock while charging and left alone: three views (clock,
widgets, photos), night red, burn-in moves and rests, the view dots.
Rust: `surfaces/lock/ui/lock.slint` `Bedside` (+ `Analog`, `Month`,
`StackClock`); `src/lib.rs` `bedside()`, `drift()`, `photos()`,
`load_photo()`, `M::Step`, `M::PhotoNext`, `M::Second`, `M::RestOver`.

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Clock weight thin/regular/bold; opacity lock.standbyClockOpacity (0.2..1); size min(16 %h, 14 %w) | 27-30 | `settings()` `sb_weight`, `sb_opacity`; `clock-size` | same |
| Material: clock min(30 %h, 17 %w), weight 200/400/600, inks primaryFixed / Dim, secondaryFixedDim | 46-51 | `m-clock-size`, `sb-m-ink*` | same |
| Views: 0 clock, 1 widgets, 2 photos (only with photos) | 53-54 | `views()`, `sb_view` | same |
| Night (not daylight and 22:00–06:00): dim red ink | 55-57 | `logic::night`, `sb-night`, `ink` | same |
| step(): next/previous view, end a rest, dots, a new spot | 58-63 | `M::Step` | same |
| Burn-in: up/down only, same spot on every screen (standbySameClock) or each its own, room = (size − content)/2 − 48 | 68-115 | `drift()` (`logic::room`, `own_fy`) | same |
| Shown: placed without gliding, dots shown | 107-114 | `bedside()` standby on → `drift(jump)`, `sb_dots += 1` | same |
| Photos from desktop.photosFolder (or ~/Pictures): jpg/jpeg/png/webp | 122-128 | `photos()` | same |
| A random photo every 20 s while that view shows (≥2 photos) | 129-135 | `t_photo` 20 s → `M::PhotoNext` (same one again: stays) | same |
| Photo shown once loaded, **fading in over 800 ms** on coming into the view; the 20 s change swaps in one frame (QML's Loading → Ready beats its fade); hidden while resting | 136-146 | `Bedside` photo `Image` (opacity by `has-photo`, 800 ms); `bedside()` clears `has_photo` on coming in and on leaving; `load_photo` keeps the last one up while the next decodes | **fixed** (batch 1: a cut coming in, a 2.7 s dim on the change) |
| Stage glides to its spot over 3 s InOutSine (a jump while resting / on showing); rests dark: opacity 0 over 2 s InOutQuad | 148-160 | `stage` `animate y` 3000 ms ease-in-out-sine (0 ms when resting/jump), opacity 2000 ms | same |
| Glass clock view: time + "dddd d MMMM   T° Condition" (ink2, max(16, 0.14·size)) | 170-197 | `if view == 0 && !Theme.material` | same |
| Material clock view: MaterialClock + date line (weather at night only) + battery cookie and weather sunny shapes (not at night) | 198-305 | `if view == 0 && Theme.material` | same (font axes: weight only) |
| Widgets view: analog clock + calendar (Glass: Squircle r 28 #1C1C1E, margins 20; Material: r 48 surfaceContainer, margins 28) | 307-377 | `if view == 1` (`Analog`, `BedCalendar`; data `calendar()`) | **fixed** (batch 1: a bare month grid). Now Notification Center's large calendar as the widget's preview draws it (not clickable, today chosen): "Mmm yyyy", Month / Week / To-do tabs, ‹ ›, six weeks with the days around the month dimmed, event dots (3 colours) and a to-do square, "Every day: …", "Today" + add button, today's events and to-dos (or "Nothing on this day…") |
| Photos view: small clock bottom-right, 40 px semibold white, raised shadow | 379-388 | `if view == 2: Text` (stroke #00000080) | same at a glance (outline instead of a raised shadow) |
| Night overlays: red 55 % over widgets/photos, black 35 % always | 391-403 | two `Rectangle`s | same |
| Dots: shown 4 s after a change, fade 400 ms; Material: current a 20 px pill | 405-429 | `dots-shown` (4 s `dt`), 400 ms | same |
| A screen left out stays dark (blank) | 22, 117-120, 151 | `blank` | same |

## Test plan (arena N, both looks; put 3+ JPEGs in the arena's ~/Pictures first: `dev run cp /usr/share/backgrounds/*.jpg ~/Pictures/` or any pictures)

1. `ipc lock lock`, `ipc lock demo standby`; shot after 1 s. PASS: clock view, centred across, dots visible at the bottom for 4 s then fade (400 ms).
2. `run wtype -k Right` → widgets view (analog clock with hands at the time + the calendar: "Nov 2026", Month/Week/To-do tabs with Month chosen, ‹ ›, six weeks with October's and December's days dimmed, today marked, dots under days with events, "Every day: …" when a daily event exists, "Today" with a + and today's events; add a local event for today first: `dev run` Settings or events.json); `-k Right` → photos view: the photo **fades in** (record 1.5 s: opacity ramps over ~800 ms, not a cut); small clock bottom-right.
3. Stay in photos 45 s, recording: every 20 s a new photo replaces the old one in a single frame, as the QML does (its Image reloads faster than its 800 ms fade moves). FAIL if the old one dims for seconds first.
4. `pointer scroll` (down) cycles views like →; up like ←.
5. Night: if the arena's time is 22:00–06:00 or set it, everything is red/dim; else skip.
6. Burn-in: shots at 3 consecutive minute changes: the content's vertical position differs each time, glides (3 s) — record one move: ≈3000 ms in-out.
7. Material: clock view shows the stacked big clock thin, date line, battery cookie (the arena may have no battery: then only the weather sunny shape or nothing).
8. `demo off` / `demo wake` → back to the lock (morph or fade per lock.standbyTransition).

## Test results

(testers fill in)
