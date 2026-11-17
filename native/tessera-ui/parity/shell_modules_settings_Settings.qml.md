# shell/modules/settings/Settings.qml ↔ surfaces/settings (lib.rs, model.rs)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

The window's logic: made on open, history, search, paging, finding a row.
Batch 1 (page switching smooth and fast) touched the paging and the models.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | LazyLoader: the window is made when Settings opens, gone when it closes (13–16) | lib.rs `open` → `make_window`, `close` → `drop_window` | same |
| 2 | Size 1040×700 Material / 860×620 Glass (19–20) | lib.rs `window_size` | same |
| 3 | minimumSize 840×520 / 720×480 (21) | lib.rs `make_window` (min 0×0) | **intentional: no minimum size (Hyprland would open the window at it)** [12 Nov: different: a minimum given before the first frame makes Hyprland open the floating window at its minimum; left out on purpose] |
| 4 | onClosed → Panels.settingsOpen false (23) | `surface_closed` → `close(say)` | same |
| 5 | Back/forward history, 50 kept, forward cleared on a new page (31–72) | `visit`, `step` (tested: history_like_a_browser) | same |
| 6 | An older page id goes where it lives now (SettingsMap.aliases) (38, 153–158) | `resolve` (tested) | same |
| 7 | Panels.settingsTarget → reveal the row (49–55) | `command("page")`, `go` → `rebuild` → M::Reveal | same |
| 8 | Google sign-in: the window steps aside, back after (74–87) | `message` feed accounts.signingIn → surface hide/show | **intentional: hidden, not minimised (no foreign-toplevel minimise for our own window)** [12 Nov: different: hidden, not minimised (no foreign-toplevel minimise for our own window)] |
| 9 | Alt+Left / Alt+Right: back / forward (88–89) | settings.slint `keys` | same |
| 10 | Search: sections whose name matches first, then index entries, every word must match, 60 at most (91–107) | index.rs `search` | same |
| 11 | The index: every page's labels/titles/details, read from the page files (108–131) | index.rs `read_pages` + `build`, on a thread | same |
| 12 | go(page, text): an alias resolved, the target kept, revealed when already on the page (134–145) | `go`, M::Act "search:" / "go:" | same |
| 13 | reveal(): find the row (retries 12×60 ms), open a closed More options around it, scroll it to 80 px from the top, flash it; a second pass 300 ms later (146–219) | `rebuild` (folded_key_of opens More options) → reveal timer 80 ms → Flash (settings.slint `changed reveal-seq`) | same (one pass: Rust knows the layout once built) |
| 14 | Mouse Back/Forward buttons (221–224) | — | **intentional: the platform reports extra mouse buttons as one** [12 Nov: different: the platform reports extra buttons as one; not told apart] |
| 15 | A press outside the search field while typing in it releases its focus (225–236) | settings.slint (TextInput) | not checked in batch 1 |
| 16 | The look's view in a Loader (238–254): the page itself is a Loader in each view, made anew on every switch | settings.slint `pages` (PageView per kept page) + model.rs `Pages` | **intentional: pages kept, by design** [12 Nov: different by design: a page once made is kept (8 at most, least recently shown dropped) and shown again at once, then updated in place] |
| 17 | (implicit) A page's bindings re-evaluate only what changed | model.rs `Models::apply`/`sync`/`keep`: blocks and rows matched by position, or by key when their number changed; a row's options/pictures lists kept and changed in place | fixed in batch 1 (before: every row's option lists were new models on every build, so every segment, chip and thumbnail was made again on every build, and a group appearing remade the whole page) |
| 18 | (implicit) Live data updates the page | `data()`: one build per burst of service updates (120 ms) | fixed in batch 1 (before: a whole-page build for every update of every service) |
| 19 | (implicit) App icons on pages load without holding the window | model.rs `Ctx::icon`/`plain_icon` → lib.rs `Job::Icon` on a thread | fixed in batch 1 (before: Login Items, Window Rules and Shortcuts drew each icon on the UI thread the first time) |
| 20 | (implicit) Files read once (os-release, PATH lookups) | appearance.rs `os_release` (OnceLock), defaultapps.rs `on_path` (cache) | fixed in batch 1 (before: read on every build) |

## Page-switch diagnosis (batch 1)

Measured in arena 8 before the fixes (Material, the user's config, loaded
machine at 94 °C): clicking 9 sidebar pages, QML first page change 819 ms
mean, Rust 808 ms mean (the same offset in both: the numbers include the tool's
~0.8 s pointer lead, compare the two only); longest stall QML 0 ms, Rust 17 ms.
So in the sandbox the switch itself wasn't slower; on the laptop (HiDPI,
real GPU, FemtoVG drawing the whole window every frame) the costs below add up:

1. **The whole window redrawn 60 times a second while Settings is open**
   (Material): the sidebar's cookie turned with `animation-tick()`, so every
   frame FemtoVG drew the whole 1040×700 (×scale) window and the damage pass
   walked every item. A switch had to fit between those frames. Now the cookie
   steps from a Rust timer every 80 ms (1.2° a step, 24 s a round, as the QML).
2. **The sidebar made anew on every switch**: `set_sections(ModelRc::new(…))`
   rebuilt all ~25 entries (icons, tiles, the cookie Path) each time; and the
   search results on every key. Now `Rows`, two rows change on a switch.
3. **The page made anew on every switch** (`blocks.set_vec`): a long page's
   elements all created and laid out at once. Now pages are kept (8), going
   back is instant; the first visit still makes the page (as the QML's Loader).
4. **Every build remade every row's lists** (`opts()`/`data` were new
   `ModelRc`s, never equal): every segmented control, chip row and wallpaper
   thumbnail was destroyed and made again on every build — after every click,
   and on every service update. Now kept and changed in place.
5. **A build per service update** (sysstat, network, audio… each tick): now
   one per 120 ms burst.
6. **Synchronous work in builders**: app icons rendered on the UI thread on
   first sight (Login Items: one per app), os-release read and PATH scanned on
   every build. Now on threads / once.
7. Missing behaviour found on the way: Material's open-page pill that springs
   from page to page (SettingsMaterial.qml 151–166) — added; Home/End in the
   sidebar — added.

## Test plan (tester, arena N, both looks)

Tools: `tools/settings-scenarios.sh` (steps: setup/open/shot/clicks/keys/run/log/down;
`ARENA=N OUT=dir`), `tools/page-switch.py`, `tools/anim-timing.py`,
`tools/region-changes.py`. Binary: the fixer's `native/tessera-ui/target/release/tessera-ui`.
Material window: 1040×700 at 440,205 on the 1920×1080 test screen. Glass:
860×620 centred (530,230): take a shot after setup and read the sidebar rows' y.

T1 Page switch speed (Material, then Glass):
1. `ARENA=N OUT=$O tools/settings-scenarios.sh setup qml material`
2. `REGION="440,205 1040x700" … clicks qml-mat "560 385" "560 435" "560 485" "560 535" "560 626" "560 676" "560 767" "560 858" "560 335"`
   (Wallpaper, Desktop & Widgets, Menu Bar & Dock, Lock & Login Screen, Notifications & Island, Focus, Windows & Spaces, Displays, Appearance)
3. `python3 tools/page-switch.py $O/qml-mat 330 60 700 620` (the page area of the recording)
4. `setup rust material <binary>`, same clicks as `rust-mat`, same analysis.
5. Again `clicks rust-mat-2` with the same points (second visit: kept pages).
PASS: Rust mean first change ≤ QML's + 17 ms; Rust worst stall ≤ 34 ms; second
visit (rust-mat-2) mean first change ≤ first visit's. Report all three lines.

T2 Build times: `tools/settings-scenarios.sh log | grep "settings:.*shown in"` after T1.
PASS: every first visit under 30 ms; report the list (page, ms).

T3 Live updates don't remake rows: on Displays (or Sound), `REGION="440,205 1040x700"`
record 5 s idle (`run 'sleep 5'` inside a `clicks` with no points, or wf-recorder in
`run`), then `tools/region-changes.py <mp4> 330 60 700 620`. PASS: no changed frames
except a value that really changes (a level meter); no whole-row flashes.

T4 More options adds only its groups: on Notifications & Island click "More
options" (Material ≈ 822,820 — read it from a shot), recording. `region-changes.py`
on the area above the button. PASS: nothing above the button changes.

T5 Wallpaper thumbnails: open Wallpaper; wait 3 s; switch to Appearance and back
by clicking. PASS: thumbnails show at once on return (no grey tiles, no fade in).

T6 Icons off the loop: open About → Login Items (and Windows & Spaces → window
rules if present). PASS: the page shows at the same time as with the QML
(page-switch.py), icons may fill in right after.

## Test results

(tester)
