# shell/modules/screentools/RustBridge.qml → surfaces/screentools (src/lib.rs `Bridge`, `ScreenTools::state` / `message`)

This file stays in the shell: it tells tessera-ui the shell's state and answers what it asks. The Rust side only reads and asks.

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | State (select, selectScreen, countdown, countdownScreen, translation, recording, available, gsr, folder, home, toolbarRecord, mics, OCR state, shareOpen, focused) to screentools and quicklook, on every change and when RustUi starts | 18-55 | `Bridge::from_json`, `state()` | same |
| 2 | `options` before the toolbar opens (Options showing / recording only) | 57-69 | `command("options")` → `Toolbar::prepare` | same |
| 3 | Live Text answers and Wi-Fi codes, to both parts | 71-79, 128-139 | `message` → shots / quick look | same |
| 4 | Asks: screen.captured, screen.cancel, screen.do (shot, select, record, translationClose), recorder.set (checked keys and values), recorder.folder (pick / open), livetext.read / act / install, panels.share | 81-144 | `crate::shell::ask(...)` | same |
| 5 | (Rust only) the toolbar's and the region selector's windows are made 1.5 s after start | — | `M::Prepare` | new: the QML creates them quickly on demand; Rust makes them ahead so the first open is as quick |

## Test plan (tester)
1. Covered by the screentools and quick look plans. In addition, in the Rust shell: change a setting in the toolbar's Options (e.g. Frame rate 30 fps), close, check `recorder.fps` is 30 in the shell's config.json, reopen → 30 fps chosen.
