# shell/modules/settings/RustBridgeInput.qml ↔ host/system/src/input/input_bridge.rs (Input service: feed 245–289, act 310–475, call 545–660), keymap.rs, mouse.rs; host/desktop screen/dictation.rs, screen/service.rs

The "input" page group (Mouse & Trackpad, Gestures, Keyboard shortcuts, Dictation, Live Text).
The Input service owns Keymap's files and the import/export state and sends the feed every
300 ms while `input.live`. Mice come from bus "mouse" (same crate); Dictation and Live Text
are read from bus "dictation" and "screentools", which nothing in host-system puts (their
owners are in host-desktop and publish host state `dictation` / nothing).

## Behaviours

Feed (QML 25–57):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | `mice`, `touchpads`, `boundaryAvailable` (26–28) | input_bridge.rs 266, 280 (bus "mouse", mouse.rs 210–212) | same |
| 2 | `keymap` {ready, defaultsError, userError, saveError, modKey, hyprDir, threeFingerDrag, binds[…], gestures[…], gestureDefaults[…]} (29–40) | 247–264, 281–284 | same |
| 3 | `dictation` {hasBinary, ready, installing, installError, models, downloading, downloadProgress, downloadError, lastDownload} (41–51) | 267, 285 (bus "dictation" never put) | **missing**: all defaults (no binary, no models); read `h.state("dictation")` (desktop dictation.rs 126–137: models are `info.models`; it has no `installError`/`lastDownload`) |
| 4 | `tools` {ready, swappy, wtype} (52) | 286 (`ready` always true, PATH lookup) | same |
| 5 | `ocr` {hasTesseract, installing, error, languages} (53) | 268, 287 (bus "screentools" never put) | **missing**: Live Text languages never listed, tesseract "not installed"; desktop screen/service.rs keeps `ocr_languages`, `has_tesseract`, `ocr_installing`, `ocr_install_error` but publishes no state for it |
| 6 | `updatesRunning` (54) | 288 bus "updates" | same |
| 7 | `ie` {keys, gestures} import/export states (55, 177–188) | 65–80, 289 (`dialog` from zenity/kdialog on PATH) | same |
| 8 | Feed told on every change (57) | 291–301, polled every 300 ms (773–777) | same (polled) |

Acts (QML 65–160, IeState 262–328):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 9 | `threeFingerDrag` (67–69) | 312–314 | same |
| 10 | `kbDisable`, `kbReset`, `kbRemove` with one undo snapshot (71–82) | 316–327 | same |
| 11 | `kbSave` {editId, keys, takeOver, custom, entry} (83–105) | 328–360 | same |
| 12 | `gSet`, `gDisable`, `gReset`, `gRemove`, `gAdd` (107–125) | 361–377 | same |
| 13 | `undo` (126–131) | 378–384 | same |
| 14 | `ie` [scope, op, …]: import/export/end4/go/cancelPath/apply/cancelPreview/undo/dismiss; dialog via zenity/kdialog else typed path; 8 s undo (133–137, 262–328) | 385–475 | same |
| 15 | `dictRefresh`, `dictInstall`, `dictDownload` [model], `dictCancel`, `dictRemove` [model] (139–153) | 390–394 → dictation.* (desktop dictation.rs 406–450) | same |
| 16 | `installOcr` [lang] (154–156) | 395 → screentools.installOcr (service.rs 232) | same (result never fed: row 5) |

## Test plan

Setup: `tools/settings-scenarios.sh setup rust material <bin>`; window 1040x700 at 440,205 (`shot` first). Compare each `shot` with the QML pass.

I1 Mouse: `open mouse` → `shot i1`. PASS: the same mice/trackpads with their settings, three-finger drag row as QML.
I2 Gestures: `open mouse/gestures` → `shot i2`. PASS: same gesture list and texts.
I3 Shortcuts: `open shortcuts` → `shot i3`. PASS: same groups, keys and mod key.
I4 Disable + Undo: disable one shortcut (`clicks i4 …` on its switch), `shot i4a`, click Undo in the toast, `shot i4b`. PASS: disabled then back, keybinds.json unchanged at the end (`run 'cat ~/.config/tessera/keybinds.json'` before/after).
I5 Export: Keyboard → Export… with a typed path if no dialog (`~/Documents/test-shortcuts.json`), `shot i5`. PASS: "Exported to ~/Documents/test-shortcuts.json".
I6 Import preview: Import… the file from I5, `shot i6` (PASS: preview lists 0 changes or the same as QML), Cancel.
I7 Dictation: open the Dictation page (Keyboard → Dictation) → `shot i7`. PASS: binary/model state and model list as QML.
I8 Live Text languages: same area → `shot i8`. PASS: tesseract installed state and the language list as QML.
Never: dictInstall/dictDownload/dictRemove, installOcr, kbSave on real shortcuts (unless undone), import apply.

## Fixed in batch 4 (fixer)

- Settings merges each group's feed by field (a host-system bridge's whole feed and the services' `settings.feed [group, patch]` add up).
- dictation from host state (models, installError, lastDownload added in screen/dictation.rs); OCR state published by screen/service.rs; the keymap file dialog's hasZenity/hasKdialog/home.

## Test results

(tester)
