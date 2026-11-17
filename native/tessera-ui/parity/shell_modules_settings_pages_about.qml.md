# shell/modules/settings/pages/about.qml ↔ surfaces/settings (src/pages/about/about.rs `page`, sysinfo.rs)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

About (page `about`): this computer, the subpages, the welcome screen,
Back up and restore (settings-backup.py, run by Rust itself), More options ›
Configuration file. No popups besides file choosers and the restore toast.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Info rows: Tessera "1.0", System (osName), Computer name, Processor, Graphics, Memory as secondary text (14–31) | about.rs 134–141 (sysinfo read once, off the UI thread) | same |
| 2 | "Your name" + detail; TextField 220, placeholder first word of account name or "Your name", commits trimmed to today.name (19–28) | 119–138 (`a.i == 1` commit) | same |
| 3 | Links: System health (heartbeat; value "" / All good / N to check), Login Items (rocket-launch), Report a Bug (bug), each with its detail (33–54) | 143–155 | same |
| 4 | "Welcome screen" + detail; "Show" pill (Material 40 tonal, Glass 26 fill pill), Space/Enter, focus ring → Panels.welcomeRequested(); footer config path (55–76) | 157–160 (`button("Show")` → ask welcome) | **different (small visual, remaining): Glass Show is a 26 px compact button with the control radius, not a pill** [12 Nov: different (small): Glass button is SButton 28 px with control radius, not a 26 px pill] |
| 5 | Group "Back up and restore", footer = status or the one-file text (83–87) | 204–205 | same |
| 6 | "Back up": detail "Saves “Tessera settings <yyyy-MM-dd>.tessera”…"; "Back Up Settings…" disabled while busy → folder picker "Choose where to save your settings" (~/Documents) → status Saving… → export → savedTo / error (120–129, 179–183) | 168–177, 206–208 | same |
| 7 | "Saved" (savedTo): detail path with ~; "Show in Files" → opens the folder (184–189) | 178–181, 209 | same |
| 8 | "Restore": "Restore…" disabled while busy → file picker "Choose a Tessera settings file" filter *.tessera *.irice → Reading… → inspect → pending / error (130–139, 190–194) | 182–192, 210 | same |
| 9 | "Restore these settings?" (pending): describe (look, mode, wallpaper, shortcut changes, Focus, "Saved d MMM yyyy, h:mm AP"); Cancel, "Restore Settings" primary (disabled while busy) (140–148, 195–204) | describe 37–64, 211–214 | same |
| 10 | Restore: Restoring… → status "Left out …" or ""; Focus.reload(); toast "Settings restored" with Undo (restore the undo file --no-undo → toast "Your earlier settings are back", Focus.reload, follow look); switch-look.sh when the look differs (154–177) | answers 77–114, follow 67–74 | same |
| 11 | PageButton: Material 40 px tonal / primary filled; Glass 26 px pill; disabled opacity 0.5; Space/Enter/Return; focus ring (212–248) | SButton | **different (small visual, remaining): Glass PageButtons are 28 px SButtons (control radius, disabled 0.45)** [12 Nov: different (small): Glass 28 px, control radius, disabled 0.45] |
| 12 | More options › "Configuration file": "Open" → config.json (251–266) | 216–223 | same |

## Test plan

Setups as in pages_menubar. `open about`; read y from a shot. **Never
press "Restore Settings"**; "Restore…" may open the chooser, then Cancel it.
"Back Up Settings…" writes a file: allowed only into /tmp of the arena (pick a
folder there), or skip.

AB1 `shot about-top`, More options open `shot about-more`. PASS: same info
values, links and values (health value as on About in QML), texts, footers.

AB2 Your name: click the field, type "Sam", Enter. PASS: config today.name
"Sam"; clear it and Enter → "" and the placeholder (first name) shows.

AB3 Links: click System health, back; Login Items, back; Report a Bug, back.
PASS: each subpage opens with its title.

AB4 Restore…: click it → a file chooser titled "Choose a Tessera settings
file" in ~/Documents; Esc/Cancel. PASS: nothing changes, footer unchanged.

AB5 Glass buttons: `shot about-glass-btns` zoomed on Show / Back Up Settings….
Write their heights and corner shape (QML 26 px pills).

## Test results

(tester)
