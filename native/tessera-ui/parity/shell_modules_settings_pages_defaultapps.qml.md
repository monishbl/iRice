# shell/modules/settings/pages/defaultapps.qml ↔ surfaces/settings (src/pages/accounts/defaultapps.rs)

Default Apps (page `defaultapps`). Installed tools come from the bridge feed
`tools` (Tools.has), else Rust looks on PATH. Menus: six Dropdowns (Row::choice).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Group "Default apps" + footer (12–15) | defaultapps.rs 57–58 | same |
| 2 | "Terminal" (Super+Return) Dropdown 220: Automatic + installed of kitty…xterm → defaultApps.terminal (16–25) | 52 | same |
| 3 | "Browser" (Super+W): "Automatic (system default)" + zen-browser…brave → .browser (26–35) | 53 | same |
| 4 | "Code editor" (Super+C): code, codium, zed, cursor, windsurf → .code (36–45) | 54 | same |
| 5 | "Text editor" (Super+X): gnome-text-editor, kate, mousepad → .text (46–55) | 55 | same |
| 6 | Installed check on a command's last word ("kitty -e btop" needs btop) (21 etc.) | `choices` 20–28 | same |
| 7 | A set value not offered shows as itself (Dropdown currentLabel) | 43–46 placeholder | same |
| 8 | More › "Default apps": "Volume mixer" (Ctrl+Super+V) pavucontrol(-qt) → .mixer; "Task manager" (Ctrl+Shift+Escape) gnome-system-monitor, plasma-systemmonitor, kitty -e btop/htop → .tasks (59–84) | 62–71 | same |

## Test plan

Setups: `tools/settings-scenarios.sh setup qml|rust material|glass <bin>`;
Material window 1040×700 at 440,205, page column from x≈772. `open defaultapps`.
**Read y from a shot.**

D1 Page: shot, open More options, shot. PASS: same rows, details, current values.

D2 Lists: open each Dropdown (click at the control, x≈1330), shot the list, Esc.
PASS: the same entries in the same order in both shells (the arena's installed
tools); 36 (Material) / 28 (Glass) rows, list ≥ 220 wide, right edges lined up.

D3 Pick: Terminal → the second entry. PASS: `run 'jq .defaultApps.terminal ~/.config/tessera/config.json'`
shows it, closed control shows it. Set back to Automatic.

D4 Keys: focus the Terminal control (click, Esc), `run 'key -k Down'`. PASS:
steps to the next option without opening (Dropdown), both shells. Restore.

## Test results

(tester)
