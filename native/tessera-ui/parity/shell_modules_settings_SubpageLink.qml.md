# shell/modules/settings/SubpageLink.qml ↔ settings.slint RowView (kind "link"); model.rs Row::link, Ctx::link / go; lib.rs go

A row that opens a subpage (or another page, scrolled to its `target` row, which flashes).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Click → Panels.settingsPage = page (+ settingsTarget) (20–25, 85–91) | RowView `hover.clicked` → SBus.act(id) → `go:` id / Ctx::go → lib.rs `go` (target revealed, Flash) | same |
| 2 | Height Material max(56, texts+24) / Glass max(44, texts+20) (28) | RowView min-height + padding 12/10 | same |
| 3 | Glass: own fill (margins 2, radius 8): pressed fillHover, hover fill, else none; 150 ms (30–37) | settings.slint:264 `!Theme.material && link` Rectangle | same |
| 4 | Material: the Group's tile and hover layer draw it (10–11) | RowView Material tile | same |
| 5 | Icon 24 / 18 secondary; gap to text 16 / 10; no icon: text at 20 / 14 (38–48) | RowView `has-icon` (link sizes) | same |
| 6 | Label 16 / body; detail 14 / caption secondary (50–64) | RowView Texts | same |
| 7 | Text to the tail: 12 px in both looks (48, `rightMargin: 12`) | settings.slint:318 label `padding-right: … (Theme.material ? 16px : 12px)` | **different** (Material 16 vs 12) |
| 8 | Tail: value (14 / body, secondary), then caret-right 24 Material / 12 bold Glass, tertiary; spacing 8 / 6; right margin 16 / 12 (66–84) | RowView `if link` HorizontalLayout | same |
| 9 | Pointing-hand cursor (89) | `mouse-cursor: link ? pointer` | same |
| 10 | Keyboard: Tab reaches it (activeFocusOnTab), Return opens it (92–93) | none: a link row has no FocusScope | **missing** |
| 11 | Accessible: Link, label, detail as description (94–96) | settings.slint:360 Rectangle: role button, label, description, default action | same at a glance (button role, Slint has no link role) |

## Test plan

L1 `open appearance`, `shot l1-<shell>-<look>` (both looks, both shells). PASS: links' value text, caret and text-to-tail gap match.
L2 Hover/press (Glass): `clicks l2 "<x y of the Glass link>"` (read from L1; the glide gives hover). PASS: the fill fades in on hover (~150 ms), darker while pressed, the Glass subpage opens.
L3 Keyboard: click an empty spot of the page, press Tab (`run 'key -k Tab'`) until a link row is focused, `run 'key -k Return'`. PASS: the subpage opens (QML does).
L4 Target: Notifications page, click a link that names another page's setting. PASS: that page opens scrolled to the row, which flashes.

## Fixed in batch 3 (fixer)

- A Tab stop with a ring; Return/Space open it; 12 px between the text and the arrow in both looks.

## Test results

(tester)
