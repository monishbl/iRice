# Parity: shell/modules/greeter/GreeterPlain.qml

The plain login screen: black, small date and time at the top, picture or
initial, name row, password field (fingerprint at its end), the line under
it, the session, power buttons in the bottom-right corner.
Rust: `surfaces/greeter/ui/greeter.slint` `PlainStyle`, `PlainChevron`.

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Top line "dddd d MMMM   h:mm" (HH:mm with 24 h), dim 14 px, 24 px down | 32-38 | `Greet.plain-top` (`view::clock`) | same |
| Picture 96 round, else the initial (typed name's or the full name's, "?"), 40 px | 45-68 | avatar `Rectangle`, `view::initial` | same |
| Name row: spacer, ‹, name label (20 px, hover fill #1C1C1F), typed field 236×44, ›, Switch user; chevrons 32 px, hover #232327 | 69-108, 230-255 | `names`, `PlainChevron` | same |
| Field 300×44, #1C1C1F, border only while focused (red with an error); shake 50→10, 70→−8, 70→4, 60→0 | 109-127 | field `Rectangle`, `Shaker` | same |
| Password dots •, 16 px, spacing 2; width leaves room for the fingerprint; keys as GreeterWindow | 128-146 | `Secret` + `key` | same |
| Placeholder "Logging in…" / "Password" | 147-153 | `view::placeholder` | same |
| Fingerprint 22 px at the end while listening: accent, red on a miss | 156-165 | `if Greet.finger-on` | same |
| The line (always there): power confirm "… click it again or press Enter. Esc cancels." / error / message / preview hint; red for bad news | 167-177 | `view::say` (Plain words) | same |
| Session "Name  ▾", hover brightens, click → next | 178-193 | session `Rectangle` | same |
| Power: 44 round, bottom-right 24 px, spacing 8, chosen #3A1F22 + red glyph, hover #232327 | 196-228 | power row | same |
| Only on the primary screen | 33, 41, 198 | `visible: Greet.primary` | same |

## Test plan (arena N: `<bin> --greeter --style plain --demo users`, mock password "tessera")

1. Shot; compare with QML plain (if runnable): top line, centred column, power buttons bottom-right.
2. Click the field: a 1 px border appears (focus); `nope` + Return → shake (+10, −8, +4, 0; ~250 ms), red border, "Incorrect password. Try again." in red.
3. `--demo finger` → fingerprint at the field's end in the accent; `--demo finger-miss` → red.
4. Hover each power button (fill), click Sleep → red glyph + "Sleep: click it again or press Enter. Esc cancels."
5. `--demo none` → "?" initial until a name is typed; typing `bob` shows "B".

## Test results

(testers fill in)
