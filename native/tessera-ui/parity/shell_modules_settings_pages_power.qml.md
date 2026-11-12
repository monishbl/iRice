# shell/modules/settings/pages/power.qml ↔ surfaces/settings (src/pages/system/power.rs `power`, ui/settings-system.slint `energy`)

Battery & Power. Live values come from the shell: QML reads Power / Backends /
PowerProfiles / System / ShellPriority directly; Rust reads `feed.system.power`
(RustBridgeSystem.qml `feed.power` in the QML shell, host/system
sysinfo/bridge_system.rs standalone) and acts through `cx.ask("system", …)`
(powerMode, caffeineOff, shellPriority, clearEnergy, quitApp: handled in both
bridges). Idle, lid, password and saver settings are config.json (`power.*`).
QQC.Popup: the idle timers' `Dropdown` (shell/common/Dropdown.qml:94) opens one;
Rust: SChoice → the settings menu (xdg_popup).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | No power-profiles-daemon: one row, label `Backends.powerShort`, detail `powerReason`, no footer (42–49) | power.rs:76–77, footer 106 | same |
| 2 | Energy mode row: Segmented Low Power / Balanced (/ Performance when the machine has it), width 330 / 220, current = Power.mode (50–61) | power.rs:78–99 | same |
| 3 | Its detail: "Now <modeName> · <why>" when something decided it, else "" or "This computer has no Performance mode" (53) | power.rs:90–96 | same |
| 4 | Choosing a mode → Power.setMode (59) | `ask system powerMode [v]` → bridge Power.setMode | same |
| 5 | Footer "Cool-down comes first …" only with ppd (42) | power.rs:106 | same |
| 6 | Keeping awake row (Caffeine on): "Caffeine is on · <status lowercased>", button "Turn off Caffeine" → System.setKeepAwake(false,0) (62–70) | power.rs:101–104 | same |
| 7 | Subpage link "Cool-down and performance", icon thermometer-simple, detail, → power/cooldown (76–81) | power.rs:116–118 | same |
| 8 | "Keep Tessera smooth when busy": visible when ShellPriority.available; detail = error or the text; Switch disabled while busy; toggles ShellPriority.set(!on) (83–93) | power.rs:112–125 | same (labelColor Theme.color.label is the default anyway) |
| 9 | Battery group (only with a battery): Critical battery switch `power.critical`; footer = Power.guardError or the text (97–107) | power.rs:128–135 | same |
| 10 | Idle group: title "On battery" (battery) / "When you're away"; 4 Dropdowns (Never, 1 minute … 1 hour), width 160, keys idle*Battery / idle*Ac; "Lock after" detail; hidden with own hypridle (110–118) | power.rs:138–150, `seconds_row`, `seconds_options` (tested) | same |
| 11 | A value not in the list shows itself (Dropdown currentLabel String(value)) | seconds_row:42–52 adds it as an extra option | same |
| 12 | "On the charger" group (battery, no own hypridle), 4 Dropdowns on idle*Ac, footer (119–128) | power.rs:151–163 | same |
| 13 | Own hypridle: "When you're away", row "Using your own idle settings", footer with the path, $HOME → ~ (129–135) | power.rs:164–171 (userHypridle from feed) | same |
| 14 | Lid and waking (battery only): 4 Lid Segmented (Sleep/Lock/Do nothing, width 260) lidBattery/lidAc/lidExternal ("On the charger")/lidCaffeine ("Instead of sleeping"), password after sleep Segmented 0/60/300 width 330, footer (138–157) | power.rs:174–190 | same |
| 15 | More options (folded, closed each time) (160) | `p.more` / `end_more` | same |
| 16 | "Low Power when the battery is low": battery && ppd; Off/Ask/Automatic width 240; detail per state with batterySaverLevel; Ask/Auto set batterySaverAuto, all set batterySaver (165–182) | power.rs:195–219 | same |
| 17 | "Game Mode uses Performance": ppd; whole row disabled without Performance; detail per hasPerformance (183–189) | power.rs:220–226 | same |
| 18 | Apps using significant energy: report runs only while the page shows (Power.energyUsers ++/--) (198–199) | bridge: RustBridgeSystem.qml:35–36 / bridge_system.rs:111–115 on page == "power" | same |
| 19 | Keep track switch `power.appEnergy`; turning it ON clears the old report (Switch's `checked` is the old value) (200–210) | power.rs:231–237 | same |
| 20 | Empty: "Nothing has used much energy lately" (212–215) | power.rs:245–247 | same |
| 21 | Each app: name from its desktop entry (else the ident), detail "N min of CPU" / "N s of CPU", a 120×6 bar (fill / secondary, min 6 px) relative to the top app, Quit (not for bare processes) → Apps.quit (216–248) | power.rs:248–265, `cpu_time` (tested); settings-system.slint:222–253 | same |
| 22 | Footer: "Off: nothing is recorded." / "CPU time over the last N minutes|hour. …" (196) | power.rs:268–272 | same |
| 23 | Idle Dropdown keyboard: closed, Up/Down step through the values without opening; Space/Enter/Alt+Down open (common/Dropdown.qml:31–41) | SChoice (settings-kit.slint:590–592): Space/Return/Down open; Up does nothing | **different** (shared control: Down opens instead of stepping; Up/Down don't step) |

## Test plan (both looks; Material window at 440,205, page column from x≈772)

The arena has no battery: groups 9, 12, 14 and row 16 are hidden in both shells
(say so, don't fake one). Never press anything that suspends, locks or shuts down.

P1 Open: `setup qml material`, `open power`, `shot power-qml-m`; same with
`setup rust material <bin>`. Scroll down with the wheel over the page (P move 1100 600; scroll) and
shot again. PASS: same groups in the same order (Energy mode, the link + priority
row, the idle group titled "When you're away" with 4 dropdowns, or the own-hypridle
group), same texts, same widths (dropdowns 160 px, segmented 330/220).

P2 Energy mode: if ppd is present in the arena, click "Balanced" then "Low Power"
in the segmented (read their x/y from P1's shot). PASS: the selection moves, the
detail reads "Now Low Power · …" in both; set it back. Without ppd: the one row
shows the same powerShort / powerReason text in both.

P3 Idle dropdown: click the "Dim the screen after" dropdown (read y from the shot).
PASS: a popup list (Never … 1 hour), checked item marked; choose "2 minutes";
the dropdown shows it; `run 'cat ~/.config/tessera/config.json'` shows
idleDimAc (or idleDimBattery) = 120. Put it back.

P4 Dropdown keys: Tab to the same dropdown (or click it and Esc), then
`run 'key -k Down'` with the list closed. PASS (QML): the value steps to the next
option without opening. Report what the Rust does (expected: opens the list = #23).

P5 Cool-down link: click it. PASS: "Cool-down and performance" opens with a back
arrow; back returns to Battery & Power.

P6 Caffeine row: `run 'ipc caffeine on'` (or the Quick Settings Caffeine tile via ipc), reopen the page.
PASS: "Keeping awake · Caffeine is on · until off" with "Turn off Caffeine"; click it;
the row disappears in both.

P7 More options: click "More options". PASS: "Energy mode" (Game Mode row, disabled
without Performance) and "Apps using significant energy" with Keep track. Turn Keep
track on; wait ≥ 5 min or note "Nothing has used much energy lately" + footer
"CPU time over the last hour…". Turn it off: footer "Off: nothing is recorded.".
If apps show, compare bars and "Quit" buttons (do NOT click Quit on anything important).

P8 Glass: repeat P1, P3, P7 with `setup qml glass` / `setup rust glass <bin>`.

## Fixed in batch 2 (fixer)

- Idle timers are Dropdowns: Up/Down step the value without opening, Alt+Down/Space/Enter open; the list's size and keys as Dropdown.qml.

## Test results

(tester)
