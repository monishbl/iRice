# Parity: shell/modules/greeter/GreeterText.qml

ly's text login: F1/F2/F3 power keys and the time along the top, a box with
the host name and three rows (session, login, password) in monospace, the
status line.
Rust: `surfaces/greeter/ui/greeter.slint` `TextStyle`, `TextField`;
`Model::set_row`, `name_key`.

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Background #0B0C0E; colours fg/dim/accent/bad | 18-24 | `TextStyle` | same |
| Rows 0 session, 1 login, 2 password; leaving the login row takes the typed name | 25-26 | `Greet.row`, `set-row` → `row-changed` → `Model::set_row` (`pick_typed`) | same |
| Top: "F1 shutdown", "F2 reboot", "F3 sleep" (red when chosen, hover fg), click → choosePower | 28-54 | `keys` row | same |
| Top right: "ddd d MMM  h:mm AP" / "HH:mm" | 55-63 | `Greet.text-top` | same |
| Box ≤560 wide, 1 px dim border, centred; host name (or "Tessera") | 65-88 | `box`, `Greet.host` | same |
| Session row "< Name >"; ←/→ (row 0) or a click on its left/right half change it | 89-94, 199-237 | `TextField` (`step` by click half), `key` Left/Right | same |
| Login row: the name + "_" while on it; typed as in ly (printable chars append, Backspace removes; Ctrl/Alt ignored) | 95-100, 148-158 | `name-key` via `Greet.printable`, `Model::name_key` | same |
| Password row: one input holding the keyboard, "*" dots, caret only on row 2; typing anything moves to row 2 | 101-134 | `pw := Secret` (`bullet: "*"`, `caret: row == 2`, `edited` → row 2) | same |
| Return: row 1 → row 2, else submit | 128-133 | `key` Return | same |
| Up/Down clamp rows; Tab / Backtab cycle | 135-138 | `key` | same |
| Left/Right on row 1 swallowed, otherwise passed on | 139-146 | `return Greet.row == 1` | same |
| Esc → cancel | 147 | `key` Escape | same |
| F1/F2/F3 (not while busy) → power choice, Enter or the same key confirms | 159-163 | `key` F1..F3 | same |
| Click on the password → row 2 | 165-172 | `TouchArea` over the input | same |
| Status: fingerprint glyph (not with a power choice) + text: power confirm / error / "logging in…" / message / preview / "Enter to log in" | 175-195 | last row, `view::say` (Text words) | same |
| No shake in this style | — | `shake()` empty | same |

## Test plan (arena N: `<bin> --greeter --style text --demo users`, mock password "tessera")

1. Shot: three rows, password row label in the accent, caret blinking after the (empty) stars.
2. Up → login row accent, "ada_"; type `x` → "adax_"; Backspace → "ada_"; Up → session row; Right → next session `< … >`; Down Down → password row.
3. Type `abc` → `***`; Return with a wrong password → red "Incorrect password. Try again.", field empty; `tessera` + Return → quits after ~1.2 s.
4. F2 → "reboot" red + "reboot: Enter (or the same key again) to confirm, Esc to cancel"; F2 again → dry `reboot` in stderr.
5. Click the session value's right half → next session; left half → previous.

## Test results

(testers fill in)
