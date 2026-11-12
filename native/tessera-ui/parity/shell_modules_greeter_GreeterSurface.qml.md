# Parity: shell/modules/greeter/GreeterSurface.qml (and its Material twin material/modules/greeter/GreeterSurface.qml)

The look's login screen. Glass: dark background, date and big time, avatar,
name row with chevrons and Switch user, frosted field (fingerprint / send
arrow), the line under it, the session, Sleep/Restart/Shut Down along the
foot. Material: the Material lock's clock (LockClock faces), the account
pill that springs open, the say pill, Round buttons, session chip.
Rust: `surfaces/greeter/ui/greeter.slint` `GlassStyle`, `MaterialStyle`
(+ `LockClock`, `Round`, `Secret`, `Shaker`), texts in `src/view.rs`.

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Glass: #0E0E10, date 24 px semibold 75 %, time 150 (primary) / 120 bold −5 at 16 % | G 27-49 | `GlassStyle` header | same |
| Glass: avatar 72 round (picture or user glyph), border | G 57-81 | avatar `Rectangle` | same |
| Glass: name row — spacer, ‹ (several, not typing), name label / typed field, ›, Switch user / ✕ | G 82-120 | `names` row, `GlassChevron` (hover 18 %) | same |
| Glass: field 240×34 frosted, red border with an error; shake 50→12, 70→−10, 70→5, 60→0 | G 121-139 | `field`, `Shaker` frames (`view::shake_frames`) | same |
| Glass: password dots •, letter spacing 2; read-only while busy or a power choice; Up/Down person, Tab session, Esc cancel, Return submit | G 140-158 | `Secret` + `key` | same (caret drawn by hand) |
| Glass: placeholder "Logging in…" / "Use Fingerprint or Enter Password" / "Enter Password" | G 159-165 | `view::placeholder` | same |
| Glass: fingerprint glyph while listening (red on a miss), the send arrow once typed (hover 45 %) | G 168-191 | right-hand `Rectangle` | same |
| Glass: the line under it (power confirm / error / message / preview hint), red for bad news | G 193-203 | `view::say` | same |
| Glass: session "Name  ▾", hover 90 %, click → next | G 204-218 | session `Rectangle` | same |
| Glass: power buttons 44 round, chosen red, hover 30 %, labels | G 221-267 | `powers` | same |
| Material: background surfaceContainerLowest (dark) / surfaceContainer | M 49-54 | `Greet.m.bg` | same |
| Material: pill opens on a key or a pointer move > 2 px (not the first position), closes 10 s after; open while typing a name / typed / busy / error / power choice | M 31-47, 55-64 | `open`, `wake()`, `idle` 10 s, `moved()` | same |
| Material: openT spring (MaterialSpring default) | M 35-38 | `animate open-t 600ms spring(0.5)` | different curve possible — tester measures against QML |
| Material: clock at 18 %: LockClock face (weather → dynamic), size min(15 %h, 20 %w), weight; colours primaryFixed/Dim/tertiaryFixed (dark) | M 26-27, 66-83 | `LockClock` | same (weight only: no width/round axes) |
| Material: analog hands turn **forward** with OutBack 450 ms | (LockClock 254-260) | `LockClock` `animate ah, am`; Rust `forward()` keeps angles growing | **fixed** (went the short way back at :00 and 12:00) |
| Material: date (hidden for dynamic + 24 h) | M 84-91 | `show-date` | same |
| Material: say pill above (≤440 wide, error colour for bad news) | M 100-120 | `say-pill` | same |
| Material: name above the pill fading in with openT; typed username pill 360×56 | M 122-158 | `nl`, typed `Rectangle` | same |
| Material: row ‹ pill › Switch user; pill rest width ↔ 360; shake spring from 18 | M 160-182 | row props, `Shaker` (spring frames) | same |
| Material: avatar cookie, 40° turn while busy (spring) | M 193-224 | avatar `Rectangle` (700 ms spring) | same |
| Material: name ↔ password cross-fade; placeholder "Logging in…" / "Enter password"; every key wakes | M 225-272 | `name-text`, `pw` (`key` → `wake`) | same |
| Material: send circle (primary once typed), spinner while busy, fingerprint slot (error container on a miss) | M 273-330 | `send`, `Busy`, `finger-slot` | same |
| Material: click on the closed pill opens it; on the name types another | M 331-345 | `if !root.open: TouchArea` | same |
| Material: session chip + Round power buttons (danger when chosen, hover tint, corners 22→14 pressed on a fast spring) | M 353-417 | corner row, `Round` (250 ms spring) | same |

## Test plan (arena N; Glass `--style look --look glass`, Material `--look material`; preview password "tessera")

1. Shots of `--demo users`, `typing`, `wrong`, `busy`, `finger`, `finger-miss`, `session`, `power`, `other`, `none`, `single` in both looks; compare with the QML greeter (`qs -p shell/greeter.qml` with TESSERA_GREETER_MOCK=1, if available in the arena) — positions, texts, colours.
2. Glass shake: type `x` + Return; record: offsets +12, −10, +5, 0 over 250 ms.
3. Material pill: start; no input → closed (cookie + name); glide the pointer 50 px → opens (record, measure open time vs QML); wait 10 s → closes. Press a letter → opens with one dot.
4. Material analog (`lock.clockFace: "analog"` in the arena's greeter config): at a minute change the minute hand steps forward with overshoot; at :00 it goes 6° forward (not a full turn back).
5. Material power: click Restart once → error-container colour + say pill "Restart: press Enter or click it again"; press and hold a Round button → its corners square off.
6. Both: Tab cycles sessions (chip text / "Name  ▾"), Up/Down people.

## Test results

(testers fill in)
