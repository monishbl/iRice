# shell/modules/dock/Dock.qml → surfaces/dock/src/lib.rs (Part `dock`)

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| One scope holding both looks' docks, each made only in its look (DockGlass / MaterialDock) | 8-11 | one `Dock` part; dock.slint draws both looks off `Theme.material`; a look change re-applies the theme and rebuilds (`config_changed(look)`) | same |
| Settings → Dock → Show the dock off: no dock windows | 7 (each Variants' model) | `config_changed`: `!s.enabled` drops every window; `output_added` refuses while off | same |
| A window per screen (Variants over Quickshell.screens) | children | `output_added` / `output_removed` | same |
| A window is made again only for its own screen | Variants | `surface_closed` took ANY part's closed surface for its own and dropped the dock on that output. **Fixed**: only the dock's own windows (Rc::ptr_eq), made again on the same screen; its menu's surface closes just the menu | fixed |

## Test plan

### Geometry (the user's settings, 1920x1080 TESSERA-TEST, scale 1, no windows open)
Items: Apps button | Downloads, Trash (pinned is empty; the divider before Downloads).
- Glass (icon 48, gap 9, pad 12, bar 72, bottom gap 6, magnification ×1.6): pill x 857..1063, y 1002..1074; icon centres x Apps 894, Downloads 970, Trash 1027; y 1038. Hover band y 961..1074.
- Material (icon 48, pad 8, gap 9, bar 64, bottom gap 16): pill x 861..1059, y 1000..1064; icon centres x 894, 970, 1027; y 1032.
Each running app adds 57 px (icon + gap) before the divider and shifts the pill left by 28.5 px: re-measure from a shot.
Use `P glide` (not jumps) so hover enters/leaves. Record with wf-recorder at 60 fps; time with tools/anim-timing.py; widget reloads with tools/region-changes.py.

1. Both looks: idle shot. One dock pill at the bottom centre of TESSERA-TEST. PASS if present in Glass and Material (appearance.look).
2. Settings → Dock → Show the dock: set `dock.enabled` false in config.json → the dock disappears within ~1 s; true again → it comes back (no restart).
3. **Other surfaces closing don't remake the dock** (the fixed bug): with the dock shown, open and close Control Center (`ipc panels open controlCenter TESSERA-TEST`, then `ipc panels close`), open+close Launchpad, open a foot window and close it. Record the dock region (x 820..1100, y 940..1080) throughout: `tools/region-changes.py rec.mp4 820 940 280 140` must show changes only where an icon is added/removed (foot) — no frame where the whole dock vanishes or redraws from blank.
