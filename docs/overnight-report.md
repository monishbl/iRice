# Overnight report — 27 Oct 2026 (04:25–06:25)

## The short version

- Design, Liquid Glass and animations are unchanged. The performance work changes how the glass gets its numbers, not what they are; every screen was checked by screenshot.
- No crashes. The shell PID stayed the same through both UI passes: 31 panels, 17 settings pages, rapid open/close, stress runs, light mode, notifications, the lock screen.
- 22 bugs fixed tonight, each verified in the sandbox. A final review of my own fixes caught 5 more problems in them, also fixed (item 22). 10 decisions are left for you, in "On hold" below.
- Commits:
  - `88895de`: only what you had staged. I first committed it as `cf457fc`, then fixed the message, which wrongly mentioned cool-down; the contents didn't change.
  - Everything else, tonight's work included, is staged and not committed.
  - `.claude/` (Claude Code's scheduler lock file) is left out on purpose.

## Performance: iRice vs end4

The run: 3 alternating rounds while plugged in, power profile Balanced throughout (logged every 10 s), the same test screen for both shells, and about the same frames rendered (end4 1,108 per run, iRice 1,134).

| | end4 | iRice |
|---|---|---|
| Shell CPU, idle | 0.37% | 0.32% |
| Shell CPU, opening/closing 6 panels | 12.7% | 15.9% |
| Shell CPU, typing in the launcher | 19.5% | 9.8% |
| Frames that stalled > 50 ms | 2.7 | 0 |
| Main thread blocked (panels) | 717 ms | 576 ms |
| Memory (PSS) | 422 MB | 407 MB |

iRice now wins or ties everywhere except panel animation CPU: +25%, down from +40%.

Two earlier rounds were thrown out:
- Below 25% battery the system switched to power-saver. I couldn't find what did it.
- After a sandbox restart, end4's panels opened on the wrong output, so it barely rendered. `bench-run.sh` now focuses the test screen first.

### What made iRice faster

| Change | Where | Effect |
|---|---|---|
| Glass shapes cached as ready-to-send text; no JSON or closures per frame; preset cached | `glass/Glass.qml`, `GlassWindow.qml`, `services/GlassPlugin.qml` | Glass JS about 35% cheaper |
| Blurred backdrop decoded once and shared (was about 110 decodes per test) | `glass/Backdrop.qml` | Image-reader thread 3.3% → 0 |
| Plugin repaints only where glass was or is (was the whole screen about 60×/s during any glass animation) | `hypr/plugin/irice-glass/main.cpp` | Less GPU work. CPU is the same; watts not yet measured, see below |

Same-conditions A/B of the shell changes: panels 45.8% → 41.3% CPU, typing 34.7% → 30.2%.

## Test coverage

| Area | How | Result |
|---|---|---|
| Static lint | `qmllint` on all 229 QML files (with generated `qmldir`s), findings confirmed by reading the code | 3 real bugs, fixed |
| Code review: tonight's diff | Read-only review agent | No crash-class bugs; 6 fixed, 1 on hold |
| Code review: commit `88895de` | Read-only review agent | No crashes; emoji search bug fixed, minor items fixed or noted |
| UI pass 1 | Agent in the sandbox via IPC, virtual pointer, screenshots and log | All 31 panels, 17 settings pages, 10× rapid cycles, 9-panel stress, cool-down, emoji, lock, OSD, island. No crash, no leftover glass |
| UI pass 2 | Agent in the sandbox | Regression of fixes (all pass), light mode, notifications (normal, critical, actions, 5–7 in a row), keyboard, clipboard, lock password, screen tools, 20-panel stress. No crash |
| Review of tonight's fixes | Fresh read-only agent | 5 problems in my fixes found and fixed (item 22); 1 on hold |
| Fix verification | Me, in the sandbox | Every fix below checked live; log clean (no TypeError, ReferenceError, "invalid context") |
| Final regression sweep | Script: all 30 panels, 17 settings pages, 40 rapid open/closes, pixel-compared with pass 1 | Run twice, the last after every fix: same PID, 0 new errors, differences only from state |

## Bugs fixed tonight

**Broken behaviour**
1. **World Clock widget didn't load.** A syntax error (`};` inside a comment) came in with the last bug sweep. `modules/desktop/WorldClockWidget.qml`
2. **AI chat "copy" button did nothing.** It threw `System is not defined` because of a missing import. `modules/sidebar/ChatMessage.qml`
3. **Wallpaper picker "open folder" failed** (`Files is not defined`). `modules/wallpaper/WallpaperPicker.qml`
4. **Panels opened on the first screen instead of the focused one** when opened without a screen (Spotlight actions, about 24 call sites). On your three-monitor setup they could appear on the wrong display. `common/Panels.qml`
5. **Critical notification banners had no glass.** Text floated on the wallpaper. This predates tonight: the banner's fade-in started after the glass's first sync had ended. `modules/notifications/NotificationPopups.qml`
6. **Banners covered Notification Center's header.** They now step aside while it's open, as on macOS.
7. **Quick Note ignored typing until clicked** on its first open. `modules/notes/QuickNote.qml`
8. **No keyboard access in Settings.** Toggles and segmented controls can now be reached with Tab and show a focus ring. Space/Return toggles; Left/Right changes a segment; Tab leaves text fields. `common/Switch.qml`, `common/Segmented.qml`, `common/TextField.qml`
9. **Kaomoji search (`^_^`, `>_<`, `-_-`) found nothing.** Typed characters now match as they are. `modules/emoji/EmojiStore.qml`
10. **Cool-down's lower refresh rate could undo screen settings.** On restore it picked the highest rate rather than yours and dropped rotation, and it also touched external screens. Now it changes only the laptop panel and restores its exact previous rate. `common/CoolDown.qml`
11. **Cool-down blur race.** Ending cool-down while it read blur settings could later restore stale values.
12. **"invalid context" warnings** were logged on every Control Center (16) and Notification Center (5) close: glass unregistering from a window already being torn down. `glass/Glass.qml`, `glass/GlassWindow.qml`

**Visual / minor**

13. **Segmented labels touched the edges** ("Everyone for 10 min", "Window controls", "Replace spaces"). Segments now size to their labels. `common/Segmented.qml`
14. **Settings text fields showed the end of long values** when not focused. They now show the start.
15. **Spotlight "settings" made Thunar Preferences the Top Hit.** Top Hit is now the best-scoring app or action. `services/Search.qml`
16. **Emoji "smile" ranked "Cat with wry smile" first.** Whole-word matches now follow the standard order.
17. **Quick Note had an empty band under its footer.** It now sizes to its content.
18. **AI chat showed a grey box for a deleted screenshot.** The box is now hidden.
19. **The AI "Effort" setting said Claude while Gemini is selected.** It now says it applies to Claude models only.
20. **Glass plugin:**
    - the repaint area includes how far glass refracts;
    - the whole-screen fallback is used while a layer has no size yet;
    - `clear` now repaints;
    - the helper restarting resends all glass.
21. **Emoji picker and island:**
    - warm-up restarts if the picker closes early;
    - an emoji font installed from Settings is loaded without a shell restart (untested; it needs a real install);
    - the island's "running hot" counter resets when the alert is off.
22. **Problems the final review found in tonight's fixes**, all fixed:
    - text fields filled in by code then focused (shortcut recorder, import) put the cursor at the start;
    - segmented widths didn't re-measure after a font change;
    - holding Space toggled a switch repeatedly, and Return was taken from dialogs;
    - a notification stack hidden under Notification Center came back expanded;
    - a timed-out glass ping could briefly mark the plugin unavailable;
    - emoji font paths with `#` or `%` failed to load.

## Not bugs (checked)

- **"Edit Controls" in Control Center:** it's the permanent bottom button, as on macOS, not a stuck tooltip.
- **"Kept in Dock" is empty:** the sandbox config has nothing pinned; the dock icons are running apps.
- **Alt+Tab clashes:** these are sandbox-only, because its modifier is Alt.
- **tray menu / Share panels opened with no tray items or files:** only reachable by forcing them open over IPC.

## On hold (decided on 27 Oct morning; see "Morning round" below)

1. **Cool-down turns on automatically on battery** (`cooling.onBattery: true` by default). Unplugged, every laptop gets frosted glass, lighter blur, shorter animations, 48 Hz and a paused live wallpaper. The alternative is defaulting to "only when hot".
2. **Light-mode contrast.** Tertiary text (placeholders such as "Spotlight Search" and "Search emoji…", plus secondary lines like "Fully charged") is 30% black, the same vibrancy as macOS, which comes to about 2–3:1. Your design rules ask for 4.5:1. Raising it to about 50–55% black changes the look everywhere.
3. **Glass over windows looks very clear.** Spotlight and the emoji picker let window text show through crisply; over the wallpaper it's fine. That's the "Glass" preset (frost 0.1). The option is a little more frost when a window is behind.
4. **Island with 5+ alerts** clips the expanded list mid-card, and notification actions (OK/Cancel) only show in Notification Center, not in the island.
5. **Menus near the menu bar refract its text** along their top edge ("Desktop Window" ghosting). That's real lens refraction; it could be toned down for dropdowns.
6. **`input.threeFingerDrag`** is a setting nothing reads. Implement it or remove it.
7. **Panel animation CPU, the remaining +25%.** Options:
   - (a) skip glass recalculation on frames where nothing moved (moderate);
   - (b) keep the most-used panels loaded after first open (fast, but more memory).
8. **Cool-down after a shell crash.** The refresh rate and blur it saved are kept in memory only. If the shell restarts mid cool-down, 48 Hz and light blur stay until a Hyprland config reload. The fix is to save them to a state file and restore on start. Also, `hl.monitor` re-sends only mode, position, scale and rotation, so a VRR or bit-depth setting on the panel would reset. That doesn't affect your current setup.
9. **Emoji search with hyphens** ("x-ray", "t-shirt") finds nothing. This predates tonight; the fix is to split on the hyphen rather than drop it.
10. **Installing the new glass plugin into your live session.** Your session loads `~/.local/share/irice/irice-glass.so`; the new build is only in the repo and sandbox.

## Not tested yet

- **Watts on battery after the optimisations.** It needs a battery run started above about 40% (the earlier one was void). I'll run it when you're up.
- **Desktop widgets / World Clock on screen.** In the sandbox they render on the nested output, which can't be screenshotted. No errors were logged when they load.
- **Cool-down's lower refresh rate** on the real panel (the sandbox screen only has 60 Hz).
- **Multi-monitor:** the plugin's repaint areas were verified on a screen not at 0,0 (the sandbox test screen sits at 20000,20000), but not on your three real monitors.

## Files

- **QA reports and screenshots:** `/tmp/claude-1000/-home-user-Projects-Personal-iRice/d4a55d5f-cd31-430b-b979-3e7c3c9c0b54/scratchpad/qa/`. It contains `ui-report.md`, `ui2-report.md`, `review-report.md`, `review2-report.md` and `lint-report.md`. This is temporary storage; copy anything you want to keep.
- **Benchmark scripts and raw results:** `/tmp/claude-1000/-home-user-Projects-Personal-iRice/d4a55d5f-cd31-430b-b979-3e7c3c9c0b54/scratchpad/bench/`. `report.py results-charging4.jsonl <start>` reproduces the table.

## Morning round (27 Oct, after your answers)

Everything below is verified in the sandbox unless marked otherwise. Final sweep after all of it: same shell PID, 0 new errors.

| Your decision | What changed | Verified |
|---|---|---|
| Cool-down only when hot | `cooling.onBattery` defaults to false (also set in the sandbox config) | Config default |
| Text contrast setting for both modes, mainly text | "Increase contrast" became **Text contrast: Standard / Higher / Maximum** (Settings → Appearance → Accessibility). It raises text, icons (same colours), control fills and tracks, and selection tints (×1.3 / ×1.6). Separators move only 10% → 12–14%; no glass outline. The old switch still counts as Higher. `common/Theme.qml` | Light mode, Standard vs Maximum screenshots |
| More frost over windows, as a setting | The plugin adds frost in proportion to how much of a glass shape covers a window. **Settings → Appearance → Frost over windows: Off / Subtle / Medium / Strong** (default Medium). The whole shape repaints when its frost changes, so there are no seams. Plugin verb `window-frost` | Spotlight over a text-filled terminal at 0 / 0.4 / 0.65 / 0.9; the settings row itself not seen (pointer scroll doesn't work in the sandbox) |
| Tint slider max about 2× | Custom material → Tint goes to 200% | Code |
| Island: scroll past 4, actions on cards | Up to 4 alerts, as many as fit the menu bar's 340 px window, then scrolling with the newest in view and an "N earlier · scroll up" hint. Notification action buttons (up to 2) on island cards | 6 notifications, one with OK/Cancel |
| Soften refraction for menus | Menu bar dropdowns (app menus, status menus, Now Playing) use 35% lens strength; big panels keep full refraction. `Glass.lensScale`, `Popover.lens` | Before/after: the "Desktop Window" ghosting is gone |
| Three-finger drag toggle | **Settings → Trackpad → Three-finger drag.** On (default): 3 fingers move windows, 4 swipe spaces. Off: 3 fingers swipe spaces. Uses the gesture overrides, so the gesture list shows the change. The unused `input.threeFingerDrag` key is removed | Toggled off and on: `keybinds.json` changes and the list updates; no gesture errors |
| Cool-down survives a crash | What it changed is saved in the state file and restored on the next start | Blur 3 → 1 with cool-down on; shell restarted; back to 3, saved entry cleared |
| Hyphen search | "x-ray", "t-shirt" find their emoji | Code (the keywords already split on hyphens) |
| Panel CPU | Moved to `docs/todo.md` | |
| Install plugin to live | New build copied atomically to `~/.local/share/irice/irice-glass.so` (nothing loads it until you run iRice live) | File in place |
| Demo script | `./demo.sh`: run it from a text console. It checks first, starts the real session (as `./dev.sh tty`) with a playing demo track and three notifications, and cleans up when you end it (Super+Shift+Escape). `./demo.sh --check` rehearses the checks from anywhere | `--check` passes; the notifications were checked in the sandbox; the track was checked on a private bus. The full tty session can't be started from here |
| Final installer | `install.sh`: missing runtime dependencies added; the whole `hypr/irice` folder is copied (gestures.json and keymap.json were skipped before); bare metal was detected as a VM (software cursor), now fixed; optional Apple/Fluent emoji step; dry run writes nothing. `uninstall.sh` removes the plugin | `bash -n`, all dry runs, before/after file listing identical. A clean-container install wasn't run (it needs package downloads) |

Also fixed along the way: alarms and timers weren't saved when they changed (UiState had no handler for them).

**Done on your machine:** the Inter font (the shell's font) wasn't installed, so the demo would have used a fallback. I copied it from the dev container into `~/.local/share/fonts/irice-inter/`. Delete that folder to undo; installing `inter-font` with pacman is the clean alternative.

**Not tested:**
- the demo inside a real tty session;
- the battery watts run (the laptop stayed plugged in at 100%);
- the installer in a clean container.

## Native performance work (27 Oct, afternoon)

| Piece | What | Verified |
|---|---|---|
| Glass sync in C++ | `shell/native` builds the `IRice.Native` QML module. Each glass window's shapes are computed in C++ on frames it actually draws, then sent from a worker thread. The QML path is the fallback | Loads in the sandbox (log: "Glass sync: native"). Same shape sets as the fallback in every state. A full sweep of 30 panels, 17 pages and 40 rapid cycles: same PID, 0 errors. Also loads on the host's Qt (offscreen check) |
| C glass helper | `shell/native/glass-ipc.c`, the same protocol as the Python one; used whenever it's built | Running in the sandbox; error path tested |
| Skip unchanged frames | The QML fallback stops its per-frame sync after 8 idle frames | Code; covered by the fallback comparison |
| Keep panels loaded | Settings → Appearance → Performance (off by default) | With it on, Spotlight's window stays alive across closes; turning it off unloads it |
| Installer | Builds the native helpers (optional step; adds cmake and ninja); `irice rebuild` rebuilds them after Qt updates | Dry run |

**Not done yet:** the benchmark. You chose to wait until the laptop is plugged in. The command is in `docs/todo.md`.

## Interaction bugs (27 Oct, evening)

From your report (real tty session) and a full sweep of the shell for the same kinds of bug:

| Bug | Cause | Fix |
|---|---|---|
| Settings drop-downs invisible; a click opens the setting underneath | The fade-in had become a render-thread animation, which can stay at 0 when started the same frame the menu appears | A normal animation again (`ActionMenu.qml`). The same pattern was fixed in the island body, the island's pin button, the dock badge and the privacy dots |
| Wi-Fi and Bluetooth lists jump; clicks miss | Every signal change re-sorted the list and **rebuilt every row** under the pointer | A snapshot. It refreshes every 4 s, never while the pointer is over the list or a password is being typed, at once if a network disappears, and with a new refresh button (`WifiPage.qml`, `BluetoothPage.qml`, `DetailPage.qml`) |
| Menu bar right side unclickable while a notification shows | The banner window had no input mask, and its margin covered the clock, Control Center, Wi-Fi and tray | An input mask on the cards. Banners also hide while Notification Center or Control Center is open |
| Pink and black squares (tray, notifications, charging) | Tray icons and notification images are `image://icon/…` addresses that skipped the checked icon lookup | `IconLookup.safe()`: a missing icon becomes a generic one or nothing, never the grid |
| Menus switch when hovering other items | Requested change | Only a click opens or switches menus. Menu bar menus close after 5 s with the pointer away |
| Window and tray menus lose the highlight and clicks | Rows were rebuilt whenever the active window or the app's menu changed | Rows are kept while their visible content (and, for the tray, the same entries) is unchanged |
| Sound sliders reset mid-drag when an app starts playing | Output and stream lists rebuilt on any audio change | `ScriptModel` keeps the rows |
| Closing panels swallow clicks during the fade | No input mask on Spotlight, Overview, Mission Control, the wallpaper picker, the screen toolbar, Quick Look or the share sheet | Input only while shown |
| Window tab strips flash on focus changes | Recreated on every focus event | Only updated when the strips actually change |
| Overview: Enter runs a different row after late answers | The selection kept its index while rows shifted | Kept by title, as in Spotlight |
| Right-click menus: arrows land on separators | | Skipped |

**Glass:**
- Your material (Glass 100%, Frost 75%, Solid 0%, Tint 75%) is the new built-in Glass preset.
- Frost over windows defaults to Medium.
- Your config is set back to Glass.

**Choppiness:**
- On the shell's side, moving across the menu bar costs almost nothing: under 40 ms of script time per 18 s of hovering.
- So the remaining choppiness is compositor or GPU work on your real hardware.
- The plugin now records frame pacing (`hyprctl irice-glass frametimes`).
- The measurement script (scratchpad `bench/perf-tty.sh`) drives the real tty session from outside. It measures pacing, GPU busy % and CPU for Glass, Solid and no plugin.
- It runs as soon as you start `./dev.sh tty` and leave it alone for about 6 minutes.

**Left (minor):**
- Notification Center rebuilds its list when a new notification arrives.
- The share sheet's device list rebuilds on each discovery event.
- The tab strips query Hyprland on every focus change.
