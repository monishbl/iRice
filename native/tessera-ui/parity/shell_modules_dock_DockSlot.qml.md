# shell/modules/dock/DockSlot.qml → surfaces/dock/src/lib.rs (`activate`, `rebuild`, `update`)

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| running = the app has windows | 176-177 | `model::windows_of` → `Item.running` | same |
| Name per kind ("Apps", "Downloads", "Trash", the window's title, the entry's name) | 181 | `model::build` | same (phone: n/a) |
| Asking for attention (urgent) → bounce | 185-186 | `rebuild`: a new urgent address among the item's windows bumps `bounces[id]` | same |
| Trash full/empty: watches ~/.local/share/Trash/files | 190-201 | a count every 2 s on a thread (`watch_trash`); the icon swaps when it crosses 0 | same (polled: up to 2 s late) |
| Right-click: the menu, pointing at the icon's centre | 209-213 | `release` → `activate(button 1)` → `open_menu` | same |
| Apps button: toggle Launchpad on this screen | 216-217 | `shell_ipc panels toggle launchpad` | same |
| Minimized: restore | 219-220 | `clients::restore` to the focused screen's space | same |
| Downloads: the stack at the icon | 222-224 | `open_stack` → part `downloadsstack` | same (the second click lands on the stack's overlay and closes it: the toggle) |
| Trash: open it | 226-227 | `open_trash` | same |
| Not running: bounce and launch | 232-234, 240 | `bounces += 1`, `apps::launch` | same |
| The app in front with windows in sight: App Exposé | 237-238 | **was missing** (it focused the app) — now `shell::ask("panels.appExpose", [id, screen])` | fixed |
| Else Apps.activateOrLaunch (Show Desktop's windows bring the desktop back) | 240 | standalone: `wm::ask("apps.activateOrLaunch")` (host/desktop apps.rs: the whole logic); under the QML shell: its newest visible window, else restore | fixed (was the simple version only) |

## Test plan

### Geometry (the user's settings, 1920x1080 TESSERA-TEST, scale 1, no windows open)
Items: Apps button | Downloads, Trash (pinned is empty; the divider before Downloads).
- Glass (icon 48, gap 9, pad 12, bar 72, bottom gap 6, magnification ×1.6): pill x 857..1063, y 1002..1074; icon centres x Apps 894, Downloads 970, Trash 1027; y 1038. Hover band y 961..1074.
- Material (icon 48, pad 8, gap 9, bar 64, bottom gap 16): pill x 861..1059, y 1000..1064; icon centres x 894, 970, 1027; y 1032.
Each running app adds 57 px (icon + gap) before the divider and shifts the pill left by 28.5 px: re-measure from a shot.
Use `P glide` (not jumps) so hover enters/leaves. Record with wf-recorder at 60 fps; time with tools/anim-timing.py; widget reloads with tools/region-changes.py.

1. Click Apps (894, y) → Launchpad opens on TESSERA-TEST; click again → closes.
2. Click Downloads (970, y) → the Downloads stack opens above the icon (see DownloadsStack plan).
3. Click Trash (1027, y) → a file manager window opens on trash:/// (or xdg-open).
4. Pin foot (config `dock.pinned` ["foot"]). Click foot's icon (first icon after Apps, x≈951) when not running → it hops twice and foot opens. Click again while foot is in front with its window visible → **App Exposé** (Mission Control showing foot's windows) opens (was missing). Click a non-front app → its window is focused.
5. Minimise foot (`hyprctl dispatch 'hl.dsp.window.move({ window = "class:foot", workspace = "special:minimized", follow = false })'`) → a minimized item appears after the divider; click it → the window comes back on the current space.
6. Urgent: make a window urgent (e.g. `foot -e sh -c 'sleep 2; printf "\a"; sleep 30'` with foot's bell=urgent, unfocused) → its icon hops twice and a red dot (Glass #FF3B30 ~16 px at the icon's top right; Material 8 px error-colour dot) appears.
