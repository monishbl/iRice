# shell/modules/emoji/EmojiPicker.qml → surfaces/emoji (src/lib.rs: `command`, `open`)

EmojiPicker.qml stays in the shell when tessera-ui draws the picker: its GlobalShortcut and IpcHandler still drive Panels, and RustUi.qml forwards Panels' open/close to the Rust part as commands.

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Super+Period (`hl.dsp.global("tessera:emoji")`) toggles "emoji" on the focused screen | 30-35 | (QML, kept) → `command("toggle")` | same |
| 2 | IPC `qs ipc call emoji toggle / open / close` | 37-49 | (QML, kept) → `command` | same |
| 3 | On every open (however opened) the pointer and focused window are looked up for placement, and the data is asked for | 22-28, EmojiStore 272-299 | `open`: `hypr j/cursorpos` + `j/activewindow` off the UI thread, `load` | same |
| 4 | The QML windows load only while Rust doesn't draw it | 53-56 | — | same (not drawn twice) |

## Test plan (tester)
Arena 1920×1080; with no focused window the panel opens centred: x 750..1170, y 326..754.
1. `rec em-ipc 1 "ipc emoji toggle"` → the picker opens (QML vs Rust: same place, same spring); `ipc emoji toggle` again closes; `ipc emoji close` when closed does nothing.
2. `inbox 'ipc panels open emoji IRICE-TEST'` → opens; `wtype -k Escape` closes and Panels forgets it (a second `ipc emoji toggle` opens again, not closes).
3. The shortcut: mark not testable here unless the arena's compositor binds `tessera:emoji`.
