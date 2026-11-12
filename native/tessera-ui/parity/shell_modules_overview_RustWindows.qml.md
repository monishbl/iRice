# shell/modules/overview/RustWindows.qml → host/desktop/src/wm/mod.rs (`Wm`: `wm_call`, `watch`, `wm.sendall`)

RustWindows.qml was the bridge while the QML shell owned Spaces/Apps/StageManager and tessera-ui drew
the window-management surfaces. Standalone (tessera-ui is the shell) the host's `Wm` service plays both
ends: it keeps Spaces, Apps and StageManager itself and answers the surfaces' `wm::ask` calls.

## Behaviours

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Tell overview, missioncontrol, switcher each display's block of spaces and labels (+ count) whenever they change, and at start | 135-139, 163 | `spaces.resend` (on change and `wm.sendall` 800 ms after start) | same |
| 2 | Tell switcher and stagemanager whether the screen is locked, on change | 140-143, 178-183 | `watch` every 1000 ms (`wm.watch`) | different: polled, up to 1 s late (the switcher also cancels when the lock takes the keyboard: `keyboard_left`, so no visible effect) |
| 3 | Tell stagemanager its waiting stages on change | 144-153, 164 | `stage.resend` | same |
| 4 | Tell missioncontrol App Exposé's app before Panels opens it | 184-190 | `overview.expose` call → `tell("missioncontrol", {expose})` | same |
| 5 | Resend everything when tessera-ui (re)starts or what it draws changes | 166-173 | `wm.sendall` at start | same (one process now) |
| 6 | `spaces.go / newSpace / sendWindow / sendWindowToDisplay / moveSpaceTo (if it exists) / move` | 199-217 | `wm_call` → `spaces.*` | same |
| 7 | `apps.show` (a minimized / set-aside window by address), `apps.pin / unpin` | 218-229 | `apps.show / pin / unpin` | same |
| 8 | `panels.show switcher <screen>` (the switcher registers once its strip shows), `panels.closeOpen`, `panels.openAppStore` | 230-240 | forwarded to the shell host (`h.ask`) | same |
| 9 | `stage.show`, `stage.merge` | 241-246 | `stage.show / merge` | same |
| 10 | Unknown call: a warning | 247-248 | `eprintln!("tessera-ui: no wm call …")` | same |

## Test plan (tester)

Rust shell only (this file has no QML-side UI); the QML pass is the reference for the effects.

1. **Spaces reach the surfaces** (T1): open the Overview: every space has its title ("Desktop"/"Space N" as QML).
   Rename/add a space (+ in the macOS bar) and reopen: the new one is there within one open.
2. **Calls** (T6, T7): in the Overview drag a window to space 3 → `hyprctl clients` shows it there; reorder two
   spaces → order changes; minimize a window (its title-bar button), Alt+Tab to it → it comes back (apps.show).
3. **Expose** (T4): `inbox 'ipc panels exposeApp foot'` (or the dock's "Show All Windows" on foot) → Mission
   Control opens with only foot windows and "foot" + "N windows" on top.
4. **Lock** (T2): hold Alt+Tab (strip shown), then `inbox 'ipc lock lock'` → the strip is gone at once
   (keyboard taken), and after unlocking Alt+Tab works again. Note the time between lock and the strip vanishing.
5. **Log** (T10): `dev run bash -c 'grep "no wm call" /tmp/irice-rt*/tessera-ui*.log'` — empty after 1–4.
