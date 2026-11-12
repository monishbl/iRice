# Parity: shell/modules/greeter/GreeterNameInput.qml

The typed-username field's keys.
Rust: `surfaces/greeter/ui/greeter.slint` `NameInput` (used by Glass,
Plain and Material; the text style types into its login row instead).

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Text is auth.otherName; edits write it back | 13-14 | `NameInput.edited` → `Greet.name-edited`; `set-name` from Rust | same |
| Read-only while logging in | 15 | `read-only: Greet.busy` | same |
| Selection colours from the ink | 17-18 | each style's `selection-*` | same |
| Enter or Tab → the password (a listed name becomes that person) | 20-21 | `key-pressed` Return/Tab → `to-password` | same |
| Up / Down → the people (leaves typing) | 22-23 | Up/Down → `next-user` | same |
| Esc → back to the chosen person | 24 | Escape → `cancel-name` | same |
| "Username" placeholder while empty | 26-32 | each style's `if nm.text == ""` | same |
| Accessible name "Username" | 25 | `accessible-label` | same |

## Test plan (arena N, Glass, Plain and Material: `--style look --look glass|material`, `--style plain`, all with `--demo users`)

1. Click "Switch user" (the person-with-arrows button) → field with the current name, all selected. Type `ada` → the selection is replaced. Tab → focus to the password, Ada chosen.
2. Switch user again, type `nobody` + Return → the picture shows the placeholder/initial "N", the password field has focus (a typed, unlisted user).
3. In the username field, Up → back to the list (previous person).
4. `--demo none` (no one listed): the username field shows at once with "Username"; Esc does nothing (no list to go back to).

## Test results

(testers fill in)
