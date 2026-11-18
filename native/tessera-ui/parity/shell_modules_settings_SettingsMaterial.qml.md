# shell/modules/settings/SettingsMaterial.qml ↔ surfaces/settings/ui/settings.slint (Theme.material)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Settings' Material look: the list pane (search pill, pages in tonal circles,
the open one on a pill that springs there) and the page pane (top app bar,
large headline, the page).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Window on m3 surface; list pane 300 px on surfaceContainer (35–41) | settings.slint `frame`, `side` | same |
| 2 | Search pill 52 px, radius 26; colour: editing → surfaceContainerHighest, hover → +8 % onSurface, else surfaceContainerHigh, 150 ms (45–64) | `search`/`field` | same |
| 3 | Search: I-beam cursor over it; a tap focuses the input (65–71) | `fold-hover` TouchArea (mouse-cursor text, clicked → focus) | same |
| 4 | Typing sets the query; Enter goes to the first result; Esc clears and leaves the field (83–97) | `search-input` edited/accepted/key-pressed | same |
| 5 | Down from the field goes into the results (or, no query, the pages list) (99–104) | `search-input` key-pressed: Down/Up move `res-cur` over the results (lit + focus ring), Enter opens that one; no query: focus to `side-keys` | **added in batch 2** |
| 6 | Clear button (40 px, ×) when text; its `tooltip` is only its accessible name (IconButton.qml 12–13 draws no tooltip) (114–131) | `clear` (accessible-label "Clear search") | same |
| 7 | The pages list, group headings (12 px, 500, 0.5 tracking; 4/18 px above, 8 below) (172–190) | `side-list` sections | same |
| 8 | **The open page's pill**: one secondaryContainer pill (48 px, radius 24) that moves to the open page on springFast (ζ 0.6, k 800 ≈ 300 ms with a small overshoot); put there without motion when the list is made (151–166, 196–209) | `side-holder` pill Rectangle, `pill-y`/`pill-placed`, `animate y { 300ms spring(0.6) }`, items report their y (`changed cur`/`init`) | **added in batch 1** (before: the item's own background switched instantly) |
| 9 | Keyboard: the list is one Tab stop (the open page); Up/Down/Home/End open the previous/next/first/last page; focus follows (211–223) | `side-keys` FocusScope (focused by a click on a page, Down from the search, or Tab) → `key-nav(-1/1/-2/2)`; the open page ringed while it has focus | **batch 2**: one Tab stop with its ring; Up/Down/Home/End only while the list has focus (before: anywhere, even inside a page's controls) |
| 10 | Item state layer: onSurface (open: onSecondaryContainer) at 10 % pressed / 8 % hover, 150 ms (233–238) | item `if Theme.material: Rectangle` state layer | same |
| 11 | Icon tile 34 px: tonal circle; the open page's a primary cookie9 turning 24 s a round, only with pressBounce (Material, not Reduce motion) and while shown (240–271) | Path `cookie`, `spin-angle` from lib.rs `spin_timer` (every 80 ms, 1.2°) | **changed in batch 1**: was `animation-tick()` (the whole window redrawn every frame while open); now 12.5 steps a second, same speed |
| 12 | Label 15 px, open: DemiBold onSecondaryContainer (272–278) | item Text | same (600) |
| 13 | Click opens the page; pointing-hand cursor (279–286) | `ita` TouchArea | same |
| 14 | Search results: page tile 40 px, text 16 + where 14, state layer, Enter/Space opens, Up/Down move (300–390) | `results` rows (`res-cur`) | same (keys from the search field: batch 2) |
| 15 | No results: icon 32, "No results", hint (392–420) | `if searching && results.length == 0` | same |
| 16 | ScrollIndicator on the list: shown while scrolling (900 ms) or hovered, fades 150/400 ms; drag/click to move (424–427; ScrollIndicator.qml) | `ScrollBar` | **fixed in batch 1** (was a bar always shown when the list overflowed, no hover/drag) |
| 17 | Top bar 64 px: surface, surfaceContainer once scrolled (150 ms); back/forward 40 px icon buttons at 0.38 when unusable; title (22 px) fades in when the headline has scrolled under (150 ms) (436–472) | `toolbar`, `arrows`, Material title | same |
| 18 | Page column min(880, width−64) centred, 4 px down; crumb on subpages (primary, hover onSurface) goes up; headline 32 px Medium, 24 below (479–520) | `page` VerticalLayout, crumb, headline | same |
| 19 | The page: a Loader made anew on every page; content scrolled to 0; then reveal (522–530) | `pages` + `PageView` (kept, 8), `page-changed()` → content-y 0, reveal timer | **intentional: pages kept, by design** [12 Nov: different by design (kept pages: instant when going back)] |
| 20 | Flash: primary 16 % + 2 px primary ring, 150 in / 900 hold / 400 out (534–548) | `Flash` | same |
| 21 | ScrollIndicator on the page (550–553) | `ScrollBar` | fixed in batch 1 (see 16) |

## Test plan (Material; window 1040×700 at 440,205 on 1920×1080)

M6 (batch 2) Search keys: click the search (560 246), `run 'key dock; sleep 0.5; key -k Down; sleep 0.3; key -k Down; sleep 0.3; shot'` then `run 'key -k Return'`.
PASS: the second result is lit with a ring after two Downs; Enter opens it (as the QML, where Down moves focus through the results).
With an empty search: click it, `key -k Down`, then `key -k Down` again: the next page opens (focus went to the pages list, ringed).

M7 (batch 2) Tab stop: click the search, press Tab until the pages list has focus. PASS: the open page shows a focus ring;
Up/Down change page; typing Down inside a page's text field does not change page.

Use `tools/settings-scenarios.sh` (`ARENA=N OUT=$O`), QML first then Rust, the user's config.

M1 (retest after batch-1 FAIL) The pill now moves in 200 ms on cubic-bezier(0.3, 1.25, 0.5, 1)
(a small overshoot), matching the QML's measured 140–220 ms. For the frame rate, also send the
arena's tessera-ui SIGUSR1 once before the clicks and once right after, and report the
`stats {"surface":"settings",…}` line: frames, render_us p50/p95/max. Every frame of this window is
drawn whole (FemtoVG has no partial rendering); if render_us p50 is above ~16 ms the frame rate
is the window's drawing cost (the whole page and sidebar every frame), not the pill: report it
for the lead (core renderer).

M1 Pill spring: `REGION="440,205 1040x700" … clicks pill Appearance→Displays: "560 858"`, then
`"560 335"`. `tools/region-changes.py $O/pill.mp4 0 100 300 600`. PASS: the pill's
move lasts 250–400 ms in both shells, with the Rust within ±60 ms of the QML; it
passes the target slightly and settles (look at frames: `ffmpeg -i … -vf fps=60`).

M2 Cookie: Settings open on Appearance, pointer parked off the window, record 3 s
(`run 'wf-recorder -g "20452,20312 34x34" -f /tmp/c.mp4 -r 60 & r=$!; sleep 3; kill -INT $r'`,
copy it out with `dev run cat`). PASS: the cookie turns about 45° in 3 s
(24 s a round) in both; Rust changes ≤ 13 times a second. Then send SIGUSR1 to
the arena's tessera-ui (its PID from `pgrep -x tessera-ui.real` whose
/proc/PID/environ has XDG_RUNTIME_DIR=/tmp/tessera-rt-N; `kill -USR1 PID` twice,
3 s apart, idle) and read the log line `stats {"surface":"settings","frames":…}`:
PASS ≤ 45 frames in 3 s (was ~180).

M3 Keys: click Appearance (560 335), then `run 'key -k Down; sleep 1; key -k Down; sleep 1; key -k End; sleep 1; key -k Home'`
with a shot after each (`shot k1`…`k4`). PASS: Wallpaper, Desktop & Widgets, the last page (About), Appearance.

M4 Scroll indicator: on Appearance, park the pointer on the page (1100 700),
`run 'P scroll 3'` (if the wheel doesn't reach the window, drag the indicator: `P move 1466 300 down glide 1466 500 300 up`).
PASS: a thin thumb appears at the right edge while it scrolls and fades about 1 s after; nothing shows when idle.

M5 Switch timing and kept pages: see Settings.qml's T1/T2.

## Test results

(tester)
