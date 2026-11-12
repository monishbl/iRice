# shell/modules/settings/SettingsGlass.qml ↔ surfaces/settings/ui/settings.slint (!Theme.material)

Settings' Glass look: a translucent sidebar (search on top, sections on
coloured tiles), the pane with a toolbar (back, forward, Section › Subpage).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Sidebar 230 px, rgba(.12,.12,.14,.78) dark / (.94,.94,.96,.8) light (25–29) | `side` background #1f1f24c7 / #f0f0f5cc | same |
| 2 | Search: sticky; scrolled down (contentY > 4) and idle it folds into a magnifier; hover or click opens it (width/opacity 150 ms OutCubic) (30–70) | `folded`, `field` width/opacity animations, `fold-hover`, magnifier | same |
| 3 | Esc clears and leaves; Enter (commit) goes to the first result; Down moves into the list (44–60) | `search-input` | same (Down into the results / the list: **batch 2**) |
| 4 | Sections under group headings (caption, DemiBold, tertiary; 12 above, 4 below) (85–98) | sections list | same |
| 5 | Section row 32 px r8: open → accent, hover → fill; tile 22 px r6 in the section's colour with a white filled icon 13 px; label body, open onAccent (99–128) | item | same |
| 6 | Keyboard: one Tab stop; Up/Down/Home/End open the previous/next/first/last (105–117) | `side-keys` → key-nav, ring on the open section | **batch 1** Home/End; **batch 2** one Tab stop, ring, keys only while it has focus |
| 7 | Click opens (MouseArea, pointing hand) (129–134) | `ita` | same |
| 8 | Search results: tile + text + where, hover fill, Enter opens, Up/Down move (137–228) | results (`res-cur`) | same (batch 2) |
| 9 | ScrollIndicator on the sidebar and the page (shown while scrolling or hovered, fading) | `ScrollBar` | **fixed in batch 1** (was always shown) |
| 10 | Pane #1E1E21 / #F5F5F7; toolbar 52 px with back/forward, Section › Subpage (the section goes up), hairline once scrolled (140–330) | `pane`, `toolbar` | same |
| 11 | Page: Loader made anew per page; content to 0; reveal (357–370) | kept `pages` (PageView) | different by design (kept: instant going back) |
| 12 | Flash: accent 2 px ring 150 / 900 / 400 (372–382) | `Flash` | same |
| 13 | Sidebar re-made on every switch | — | **fixed in batch 1** (Rust remade the whole sidebar each switch: now `Rows`, two rows change) |

## Test plan (Glass: `tools/settings-scenarios.sh setup qml glass` / `setup rust glass <bin>`)

Window 860×620, centred at 530,230 on 1920×1080; read the section rows' y from
`shot glass-open` (rows are 32 px + 2 px apart, under headings).

G1 Switch timing: Settings.qml T1 with the Glass rows' points and `REGION="530,230 860x620"`,
page area `231 52 629 568` for page-switch.py. PASS as T1.
G2/G4 (retest after batch-1 FAILs): the magnifier has no click area of its own now, so the
search's hover sees it and opens the field on hover; the scroll indicators show only for a
person's scroll (Flickable `flicked`) or a drag of the thumb, and hide 900 ms after.

G2 Search fold: on a long page scroll the sidebar (drag its indicator, or `P scroll`):
the search folds to a magnifier in 150 ms; glide over it: it opens; away: folds.
G3 Keys: as Material M3. G4 Scroll indicator: as Material M4.

## Test results

(tester)
