# shell/modules/settings/pages/appearance-windows.qml ↔ surfaces/settings (pages/notify/windowrules.rs `windows`)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Shown as the part "Windows, Stages and Overview" of the Windows & Spaces page
(map.rs PARTS; alias `appearance/windows` → `windowrules`). Feed:
`cx.feed["notify"]` (host/shell/src/panels.rs builds it in the standalone
shell; RustBridgeNotify.qml under QML).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Page title "Windows and Overview" (12) | part heading "Windows, Stages and Overview" | same (shown as a part) |
| 2 | Windows footer: titlebars without plugin / controls (plugin-drawn or not) / titlebars / none texts (17) | windowrules.rs 515–527 | **same (fixed before 17 Nov: decorationsPlugin/pluginControls via the wm's settings.feed)** [12 Nov: **different (standalone)**: the notify feed has no `decorationsPlugin` / `pluginControls`, so with Title bars it always says "Title bars need the hyprbars plugin…", and with Window controls always the not-plugin text] |
| 3 | Gaps Segmented 300 None/Compact/Comfortable, detail (18–27) | 528 | same |
| 4 | Decorations Segmented 300 None/Window controls/Title bars (28–37) | 529 | same |
| 5 | Stages group + footer; Segmented 300 Off/On/On with recents; On/recents set strip, all set enabled; details (40–62) | 531–548 | same |
| 6 | Overview Style Segmented 260 Grid/Strip (grid/macos) + details; footer (64–79) | 550–557 | same |
| 7 | More options → Windows: Window transparency Segmented 300 Off/Subtle/More/Custom + detail (84–96) | 562–568 | same |
| 8 | Custom: Focused window / Other windows sliders 220, 0.6–1, rounded 0.01, detail "NN% opaque" (97–113) | 569–575 | same |
| 9 | Window tabs Segmented 300 Hyprland/Tessera, detail (+ "(needs the glass plugin…)" without the plugin) (114–124) | 577–585 | same |
| 10 | hyprbars plugin row (titlebars, plugin not loaded): "Install…" accent pill (Material 40, Glass 28) → Decorations.installPlugin (125–135) | 586–587, host `installHyprbars` | **same (fixed before 17 Nov: same feed)** [12 Nov: **different (standalone)**: shows whenever Title bars is chosen, even with hyprbars loaded (same missing `decorationsPlugin`)] |
| 11 | More options → Overview: With several displays Segmented 260 "A row per display"/"Each its own" + details; Show the wallpaper in spaces switch; Window previews Segmented 260 Snapshot/Hybrid/Live + details (137–170) | 593–613 | same |

## Test plan (both looks)

`setup qml|rust material|glass <bin>`, `open windowrules` (or `open appearance/windows`),
scroll to the "Windows, Stages and Overview" part (read y from a `shot`).

W1 Decorations: click "Title bars". PASS: with hyprbars loaded (`hyprctl plugin
list` in the arena), footer "macOS-style title bars…", no hyprbars row in More
options; without it, "Title bars need the hyprbars plugin…" and the "Install…"
row. Click "Window controls": footer names the compositor-drawn text when the
glass plugin draws controls. Report both shells.

W2 Gaps / Stages / Overview style: click each option; details and footers as
#3, #5, #6 word for word; Stages "On" → detail "Windows get the full width…".

W3 More options: Window transparency "Custom" → two sliders 60–100 %; drag one:
detail "NN% opaque". Window tabs Tessera: detail with/without the plugin note.
Overview rows: details change per option; the wallpaper switch toggles.

## Fixed in batch 2 (fixer)

- Not fixed yet: `decorationsPlugin` / `pluginControls` in the notify feed (the footer's plugin text).

## Fixed in batch 3 (fixer)

- The hyprbars plugin is looked for (hyprctl plugin list) while title bars are chosen, and window controls count as the plugin's when the glass plugin is loaded, when the shell doesn't say.

## Test results

(tester)
