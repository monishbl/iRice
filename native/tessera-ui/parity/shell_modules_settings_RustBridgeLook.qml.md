# shell/modules/settings/RustBridgeLook.qml ↔ host/shell/src/bridges.rs (settings_act 89–163, look_feed 166–200, look.loginWallpaper 933–950, root helper 1025), panels.rs 455–470

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

The "look" page group (Appearance, Glass, Wallpaper, Desktop & Widgets, Menu Bar & Dock,
Lock & Login Screen). In the standalone shell its feed is `bridges::look_feed` merged with
`Panels.feeds["look"]` (filled only by `settings.feed`, which nobody calls) and its acts are
`bridges::settings_act`.

## Behaviours

Feed fields (QML 206–236):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | `cooldownGlass` CoolDown.glass (207) | bridges.rs 183 `h.state("cooldown.glass")` (appearance.rs 109–110) | same |
| 2 | `sunrise`, `sunset` Daylight (209–210) | bridges.rs 167 `wallpaper::sun()` | same |
| 3 | `path` Wallpaper.path (211) | bridges.rs 185 `h.state("wallpaper.path")` | same |
| 4 | `seed` String(Wallpaper.seed) (212) | bridges.rs 186 `"seed": tone` | **same (fixed before 17 Nov: Material seed from the picture (core seed.rs))** [12 Nov: **different**: sends the tone, not the wallpaper's seed colour (check the Appearance "Wallpaper colour" swatch against QML)] |
| 5 | `accent` Wallpaper.accent (#rrggbb) (213) | bridges.rs 171 `snap_accent(tone, dark)` | same (computed the same way; verify swatch) |
| 6 | `tone` Wallpaper.tone (214) | bridges.rs 186 | same (sent unsliced; the page takes 7 chars, look/mod.rs 86) |
| 7 | `emojiAvailable`, `emojiPackage` EmojiFont (215–216) | — | **same (the page reads fc-list itself per style, as EmojiFont; 17 Nov B5: after Install it looks again every 5 s for 2 min, EmojiFont.install's recheck)** [12 Nov: **missing** (page falls back to its own font list, look/appearance.rs 282–283)] |
| 8 | `glassPlugin`, `glassPluginVersion` (217–218) | bridges.rs 188–191 | same |
| 9 | `appThemeAvailable` AppTheme.available (219) | — | **same (page fallback = AppTheme.available: Material, or matugen on PATH)** [12 Nov: **missing** (page falls back to `on_path("matugen")`, appearance.rs 520)] |
| 10 | `pickerBusy` FilePicker.busy (221) | bridges.rs 193 `picker.busy` | same |
| 11 | `sets` Wallpaper.sets (222) | bridges.rs 194 `wallpaper.sets` (rescan_sets 1057) | same |
| 12 | `livePoster` Wallpaper.livePoster (223) | — | **same (fixed before 17 Nov: wallpaper.rs makes the poster off the UI thread when the shell has none (no false "Couldn't read"))** [12 Nov: **missing**: with a live wallpaper set, the page shows "Couldn't read this video…" (wallpaper.rs 144–150) though the poster exists] |
| 13 | `directory` Wallpaper.directory (224) | bridges.rs 195 | same |
| 14 | `rootHelper` (passwordless: root helper is there; Process 244–248, run while live) (225) | bridges.rs 196, set at 1025 | same (checked once at start rather than each time Settings opens) |
| 15 | `loginStatus` (226, set by copyWallpaper 249–256) | bridges.rs 197, 933–950 | same |
| 16 | `files` Files.installed [{bin,name}] (228) | — | **same (page fallback: the installed file managers on PATH, Files.installed's list)** [12 Nov: **missing** (page falls back to file managers on PATH, desktop.rs 41–45: same list)] |
| 17 | `weather` {ready, place, temp, condition, error, icon} (229) | — | **same (fixed before 17 Nov: bridges.rs look_feed `weather` from core Weather)** [12 Nov: **missing**: Desktop page falls back to the core Weather service (desktop.rs 64–68) but Lock page's clock preview (lock.rs 78–82) has no fallback → no temperature/icon on the lock preview] |
| 18 | `shelfCount`, `shelfOpened` (230–231) | bridges.rs 191–192 (sharing lib.rs 587 state "shelf") | same |
| 19 | `login` {installed, status} LoginScreen (233) | — (host-system login_screen.rs 74 puts it on its private bus "loginscreen" only) | **same (fixed before 17 Nov: login_screen.rs asks settings.feed [look, {login}])** [12 Nov: **missing**: Lock page always says "This computer signs in with another login screen…" and never shows the status] |
| 20 | `boot` {designs, chosen, pending, busy, blocked, error, done, checked, splash, original} BootSplash (234) | — (boot_splash.rs 83–91 puts it on the private bus "bootsplash" only) | **same (fixed before 17 Nov: boot_splash.rs asks settings.feed [look, {boot}])** [12 Nov: **missing**: boot animation cards show only "System" and "None", no designs, chosen always "system"] |
| 21 | `screens` [{name, model, width, height}] (235) | bridges.rs 173–182 | same |
| 22 | Feed sent on every change while live (237) | panels.rs 455–470 (diffed each sync; resent after an act) | same |

Acts (QML 264–325):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 23 | `editControls`: close Settings, Control Center in edit mode (266–270) | bridges.rs 119–124 | same |
| 24 | `switchLook` [glass\|material]: set config, run switch-look.sh detached (271–276) | bridges.rs 126–132 | same |
| 25 | `repairShell`: System.repairShell after 800 ms (277–279, Timer 258–262) | bridges.rs 133 `look.later` 800 → `system.repairShell` | same |
| 26 | `installEmoji` [style] (default: current style) (280–282) | bridges.rs 134 → system.rs 591 | **same (fixed before 17 Nov: empty style → configured one)** [12 Nov: **different**: an empty arg is ignored (system.rs `_ => return`) instead of using `appearance.emojiStyle`] |
| 27 | `buildPlugin`: terminal running tools/build-plugin.sh --load (283–285) | bridges.rs 135–138 | same |
| 28 | `setWallpaper` [/abs path] (286–289) | bridges.rs 139–143 `wallpaper_set` | same |
| 29 | `loginWallpaper`: copy via root helper or pkexec cp; sandbox message (290–297) | bridges.rs 144 → 933–950 | same |
| 30 | `editWidgets`: close Settings, first screen, widget editing (298–302) | bridges.rs 145–149 | same |
| 31 | `shelf`: toggle the Drop shelf (303–305) | bridges.rs 150 `shelf.toggle` | same |
| 32 | `previewLogin` (306–308) | bridges.rs 151 → system.rs 578 → `loginscreen.preview` | same |
| 33 | `bootRefresh`, `bootSet` [id] (309–314) | bridges.rs 152–153 → bootsplash.refresh/set | same (but results never reach the page: row 20) |
| 34 | `playIntro` unless reduce motion (315–318) | bridges.rs 154–158 → lock.playIntro (preview_intro 460) | same |
| 35 | `lock` (319–321) | bridges.rs 159 → lock.lock | same |

## Test plan

Setup: `tools/settings-scenarios.sh setup rust material <bin>`; Settings 1040x700 at 440,205 (take a `shot` to read positions). Repeat each `shot` after `setup qml material` and compare.

L1 Appearance: `open appearance` → `shot l1`. PASS: sunrise/sunset times, wallpaper colour swatch (seed), accent, emoji style availability, Glass plugin row match QML.
L2 Wallpaper: `open wallpaper` → `shot l2`. PASS: current picture, sets list, folder, "Use for login screen" row text (root helper) match. If a live wallpaper is configured: no "Couldn't read this video" (row 12).
L3 Desktop & Widgets: `open desktop` → `shot l3`. PASS: Files app choices, weather footer ("Showing <place>: <temp>°…"), Drop shelf count match.
L4 Shelf toggle: on Desktop & Widgets click Show/Hide shelf (`clicks l4 "x y" "x y"`, twice). PASS: shelf opens then closes; button label follows.
L5 Lock & Login Screen: `open lock` → `shot l5`. PASS: clock preview shows the weather temperature, Login screen footer and boot animation cards (designs, chosen) as QML.
L6 Boot refresh: `log` after L5 shows no "no look." lines; cards update after a few seconds (`shot l6`).
L7 Preview login: click Preview Login Screen → `shot l7`, then `run 'key Escape'`. PASS: the login preview shows and closes.
L8 Edit Controls / Edit Widgets: click each (`clicks l8 "x y"`), `shot`, Esc. PASS: Settings closes, the editor opens.
Never: switchLook, loginWallpaper (writes /usr/share), bootSet, installEmoji, buildPlugin.

## Fixed in batch 4 (fixer)

- Settings merges each group's feed by field (a host-system bridge's whole feed and the services' `settings.feed [group, patch]` add up).
- seed: the wallpaper's seed (Material), the tone (Glass); weather; login/boot from login_screen.rs / boot_splash.rs; installEmoji defaults to the configured style. (livePoster, files, emoji fonts, matugen: Settings looks them up itself, batches 2–3.)

## Test results

(tester)
