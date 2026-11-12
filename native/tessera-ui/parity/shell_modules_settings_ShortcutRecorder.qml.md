# shell/modules/settings/ShortcutRecorder.qml ↔ surfaces/settings (shortcuts.rs `rec_*`, `recorder_event`; keymap.rs `key_name`/`catchable`/`to_text`/`from_text`; settings-input.slint `Recorder`)

The Keyboard form's key picker: four modifier chips (toggles), "+", a box that
catches ONE plain key, or the keys as text ("Edit as text"). State is page-local
(`kb.mods`, `kb.key`, `kb.text`, `kb.textMode`). No popups.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | `result`: text mode → fromText(text), else formatKeys(mods, key); `complete`: text mode → parsed key non-empty, else key set (19–20) | `rec_result`, `rec_complete` shortcuts.rs:64–77 | same |
| 2 | load(keys): mods/key from parseKeys, text = toText(keys), text mode when the key isn't catchable (mouse:, code:, XF86…, two keys) (25–32) | `rec_load` shortcuts.rs:79–85, keymap.rs `catchable` (tested) | same |
| 3 | toText ($MODKEY→SUPER_L, $MOD→SUPER) / fromText (SUPER/WIN/META/MOD4→$MOD, SUPER_L→$MODKEY, CTRL/CONTROL, ALT, SHIFT upper-cased, rest kept) (36–52) | keymap.rs `to_text`/`from_text` (tested) | same |
| 4 | Chips SUPER / CTRL / ALT / SHIFT: solid Chip, 32 tall, ≥ 56 wide, checked = in mods; label = KeyNames name (SUPER shows the mod key's name: ALT in the arena); Mac style "<symbol> Command/Control/Option/Shift"; click toggles, updates text, emits edited (53–57, 104–123) | shortcuts.rs:436–451, `recorder_event` 0..3 → `rec_set_mods` (clears error); slint Recorder SChip | same |
| 5 | "+" in tertiary, title size (124–130) | slint Recorder | same |
| 6 | Box 168×32: Material filled pill (outlined while listening), Glass fill / accent 12 % + 2 px accent border while focused, 1 px separator otherwise (132–147) | slint `catcher` | same |
| 7 | Box text: "Press a key…" while focused; "Tap <mod> alone" for $MODKEY; the key's label (600 weight); "Click, then press a key" (secondary) (150–158) | slint catcher Text + `key_shown` shortcuts.rs:452 | same |
| 8 | Click the box → focus; Tab reaches it (activeFocusOnTab) (140, 159–163) | TouchArea → `fs.focus()`; FocusScope | same |
| 9 | A key press: modifiers alone (Shift/Ctrl/Alt/Meta/Super/AltGr) are ignored; anything else records ONE key — including Esc, Backspace, Tab, Return, Space (they're keys, not "cancel/clear"); held Shift/Ctrl/Alt/Meta turn those chips on (never off); text updates; edited (164–185) | slint `fs.key-pressed` (accepts everything) → `recorder_event` 10 (`key_name`, held bits) | same |
| 10 | Letters/digits upper-case; F1–F24; named keys (arrows, Space, Return/Enter, Tab/Backtab, Escape, BackSpace, Delete, Insert, Home, End, Page_Up/Down, Print, punctuation) (63–74, 85–94) | keymap.rs `key_name` | **different**: Slint has no Print key text, so Print Screen can't be recorded (QML: "Print"); confirm in test R4 |
| 11 | Shifted symbols (US) → the key underneath + SHIFT on (? → Slash, ! → 1 …) (75–84, 95–98) | `key_name` shifted list (tested) | same |
| 12 | Any other key → "code:<scan code>" (e.g. é, keypad keys Qt names oddly, media keys reaching the window) (99–100) | `key_name` → None: the press is ignored (Slint gives no scan code) | **different** (no code:N fallback) |
| 13 | After a key, focus moves to the "Edit as text" button (never traps the keyboard) (184) | `self.clear-focus()` | **different** (focus is dropped, not moved on: the next Tab starts from the window's start) |
| 14 | Text mode: field min(width, 420) × 32, placeholder "SUPER + SHIFT + S", help text (caption, secondary) under it; every edit / commit emits edited (189–213) | slint Recorder `if r.on` InField + Text; `recorder_event` 20 | same |
| 15 | The text field shows what the page loads into it (load / toggle / "Use default keys" set textField.text) (29, 55, 182, 226) | InField `gen: r.num`, but `place_all` overwrites a custom row's `num` with its tile place (input/mod.rs:46–58), so `set_field("kb.text")` never reaches a field that already exists | **different**: in text mode, "Use default keys" with a default the box can't catch (mouse/code), or reopening the form on another text-mode bind, leaves the old typed text on screen while Rust holds the new one |
| 16 | "Edit as text" ↔ "Use the key picker": to text fills it from the picker's keys and focuses the field; back parses the text, keeps the key only if catchable; edited (215–231) | `recorder_event` 21 | same except the field isn't focused on switching to text (**missing** `focusInput()`) |
| 17 | Accessible: EditableText "Key: <label or none>. Press a key to change it." (148–149) | catcher `accessible-role: text-input`, label from `r.value` | same |

## Test plan (both looks; Material window at 440,205, page column from x≈772; arena mod key = ALT)

Open Keyboard (`open shortcuts`), click "Add shortcut", pick kind "Command", type `true`
(so Save is possible), then use the recorder in the Keys group (read its y from a shot:
chips at the left of the page column, the box after "+"). Never Save a key that runs
anything harmful; Cancel at the end of each test.

R1 Chips: click SUPER (labelled with the mod key, "Alt" in the arena) and SHIFT. PASS: both
chips checked in both shells; the box still "Click, then press a key".

R2 One key: click the box (shot: "Press a key…"), `run 'key -k q'`. PASS: box shows "Q",
the box lets go of focus. `run 'key -k Tab'` then shot: QML's focus ring is on
"Edit as text" (#13); report Rust's.

R3 Special keys are recorded, not obeyed: click the box, `run 'key -k Escape'`. PASS:
box shows "Escape", the Settings window stays open, the form stays. Same with
`key -k BackSpace` (box "BackSpace"), `key -k Return`, `key -k F5` ("F5").

R4 Print: click the box, `run 'key -k Print'`. PASS (QML): "Print Screen". Report Rust (#10).

R5 Held modifier: clear chips, click the box, `run 'key -M ctrl -k k -m ctrl'`. PASS: CTRL chip
turns on, key "K". Shifted symbol: click the box, `run 'key -M shift -k slash -m shift'` (or `key "?"`):
SHIFT on, key "/" (Slash).

R6 Unknown key: click the box, `run 'key é'`. PASS (QML): "Key <n>" (code:N). Report Rust (#12: ignored).

R7 Text mode: click "Edit as text". PASS: field holds e.g. "SUPER + CTRL + K" (the picker's
keys), help text under it; QML puts the caret in the field (#16; in Rust click the field first).
`run 'for i in $(seq 30); do key -k BackSpace; done; key "SUPER + mouse:272"'`, then
click "Use the key picker": the chips show SUPER, the box is empty (mouse isn't catchable).

R8 Field reload (#15): close the form, open Change shortcut on a default bind whose keys are
a mouse button or code (search `mouse` or `click`; if none, skip). The recorder opens in text
mode. Type extra text, then click "Use default keys". PASS (QML): the field shows the
default again. Report Rust.

R9 Glass: R1, R2, R7 with `setup … glass` (box: fill + accent border while listening).

## Fixed in batch 2 (fixer)

- Not fixed yet (see shortcuts.qml's notes): Print, code:N, focus after a key, caret in the text field.

## Fixed in batch 3 (fixer)

- Print Screen records as "Print" (core sends Slint's SysReq for Keysym Print / Sys_Req; keymap.rs names it).
- After a key, the focus goes to "Edit as text"; pressing it puts the caret in the text field.
- Not done: keys with no name (code:N) need the key's code from core.

## Test results

(tester)
