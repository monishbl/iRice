# Parity: shell/modules/greeter/GreeterWindow.qml

The login screen's window and the actions every style shares: next/previous
person, type a name, next session, submit, Sleep/Restart/Shut Down with
confirmation, Escape, the field cleared and shaken on a refusal.
Rust: `surfaces/greeter/src/lib.rs` (windows per output, `primary`,
effects), `model.rs` (`next_user`, `edit_name`, `to_password`,
`cancel_name`, `next_session`, `submit`, `choose_power`, `cancel`).

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Window black; namespace tessera-greeter; Overlay in a preview, else Top; full screen, ignores exclusive zones | 22-27 | `lib.rs` window `Spec` (`overlay` when mock) | same |
| Keyboard only on the primary screen (exclusive), the others show the clock alone | 26, 104 | `set_primary` (keyboard interactivity per surface), `Greet.primary` | same |
| 24-hour time from TESSERA_GREETER_CLOCK24 | 21 | `view::h24` (env, else config) | same |
| nextUser: not while busy; typing → back to the chosen person; wraps; clears the field | 39-50 | `Model::next_user`, `set_user` → `ClearField` | same |
| editName: the username field focused, its text selected | 53-59 | `edit_name` → `Effect::FocusName` → `focus-name` (select-all) | same |
| toPassword (Enter/Tab in the name), cancelName (Esc) | 60-69 | `to_password`, `cancel_name` → `FocusPassword` | same |
| nextSession (Tab; click), wraps, not while busy | 70-73 | `next_session` | same |
| submit: a power choice → systemctl action (preview: logged only); else login | 74-85 | `submit` → `Effect::Power` (dry in preview/demo) | same |
| choosePower: first click chooses, second (or Enter) does it; focus back to the field | 86-94 | `choose_power` | same |
| Escape: drop the power choice, else quit a preview, else clear the field | 95-103 | `cancel` | same |
| Focus to the password when a screen becomes primary | 104 | `set_primary` → `focus-password` | same |
| A refusal (error set): field cleared, `refused` → the style's shake | 105-112 | `set_error` → `ClearField`, `Refused` → `shake()` | same |
| Person changed → field cleared; typed name dropped → focus back to the password | 113-117 | `set_user`, `set_other(false)` → `FocusPassword` | same |

## Test plan (arena N; preview run as in GreeterAuth's plan; `--demo users` gives 3 made-up people)

1. `<bin> --greeter --demo users` (mock). Down → next person (name changes, field empties). Up → previous; wraps at both ends.
2. Type `abc`, then Down → the field is empty.
3. Click the power button (Shut Down) once → it turns red and the line says "Shut Down: click it again or press Return. Esc cancels."; Esc → back to normal; click twice → stderr `greeter (dry): poweroff` (nothing happens to the arena).
4. Choose Restart, then Return → dry reboot logged.
5. Tab → the session under the field changes ("Hyprland  ▾" …), wraps.
6. Click the name → it turns into a username field with the name selected; type `grace` + Return → that person is chosen (picture/initial changes), focus in the password.
7. Esc in the username field → back to the chosen person.

## Test results

(testers fill in)
