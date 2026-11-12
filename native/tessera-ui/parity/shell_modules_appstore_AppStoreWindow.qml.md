# shell/modules/appstore/AppStoreWindow.qml ↔ surfaces/appstore (src/lib.rs, src/logic.rs, ui/appstore.slint)

The App Store ("Software"): a regular window with search and places on the
left, app cards on the right and an app's page over them. The window's state
(place, search, filter, page, token) lives in `surfaces/appstore/src/lib.rs`.
The texts come from `logic.rs` and the drawing from `ui/appstore.slint`. The
store itself is host-sharing's (`host/sharing/src/appstore.rs`; see
AppStoreFeed.qml's parity file). There is no Material twin file: both looks are
`Theme.material` branches inside this file, and every row below covers both.
The window's only popup is the Material split button's menu (shell/common/SplitButton.qml).

## Behaviours

| # | Behaviour (QML line) | Rust place (file:line) | Status |
|---|---|---|---|
| 1 | Window made only while open (LazyLoader on Panels.appStoreOpen), title "Software", 1040×720 / 980×660, min 820×640 / 760×500, transparent (22–30) | lib.rs:197-251 `open_window` (WindowSpec), 253-266 `close_window`; appstore.slint:842-846 | same (fresh toplevel each open, as the LazyLoader). The app id is "tessera-appstore" (documented) |
| 2 | Closed by the compositor → Panels.appStoreOpen = false (31) | lib.rs:694-700 `surface_closed` → ask "close" | same |
| 3 | On open: users++, refreshStatus, load(), detail = Panels.appStoreApp (98-104); users-- on close (105) | lib.rs:238-245; host/sharing/src/appstore.rs:194-207 `store_sync` counts | same |
| 4 | Places: Discover, 9 categories, Installed, with icons (46-58); heading "Results for “q”" or the place name (60) | logic.rs:8 PLACES, :58 `heading`; lib.rs:411-423 | same |
| 5 | pick(place): clears the search field, go() (65-68); go(): detail=null, view, category, load (69-74) | lib.rs:616-622 M::Pick, 270-275 `go` | same (also stops the debounce; QML's cleared text restarts it, then it does nothing) |
| 6 | load(): ++token, loading, clear failed/apps/sections, AppStore.ask per view; an answer for an old token is dropped (75-97) | lib.rs:277-288 `load`, 532-544 answer + token check | same; Rust also scrolls the list to the top (285), QML's Flickable keeps contentY |
| 7 | Typing: query = text; "aur:" prefix sets filter aur; 300 ms debounce restarted (110-115) | lib.rs:290-298 `typed`; logic.rs:108 `is_aur_query` | same |
| 8 | Debounce fires: ≥2 chars (trimmed) → search, else back to discover if searching (140-149) | lib.rs:609-615 M::Debounced | same |
| 9 | Enter: stop debounce, search if ≥2 chars (116-120) | lib.rs:603-608 M::Entered | same |
| 10 | Panels.appStoreApp changes while open → that app's page (121-127) | lib.rs:564-568 `show` → `show_page` (320-327) | same (Rust also resets the page scroll and closes the menu) |
| 11 | AppStore.changed(key, installed): mark detail, apps, sections; reload when on Installed (128-139) | lib.rs:545-563 | same |
| 12 | Sidebar 272 / 230 wide, surfaceContainer / translucent rgba(.12,.12,.14,.78) / (.94,.94,.96,.8) (152-157) | appstore.slint:852-858 | same |
| 13 | Glass search: TextField 28 high, placeholder "Search apps" hidden while focused, accent border while focused, Esc clears, focused on open unless an app was asked for (167-177) | appstore.slint:227-276 GlassSearch; lib.rs:246-250 `invoke_focus_search` | same |
| 14 | Material pill search 56 high: colour highest when focused / hover tint (8% onSurface) / high, 150 ms; IBeam cursor; tap focuses; magnifier 22 px; 16 px text; placeholder; Esc clears (178-228) | appstore.slint:279-340 PillSearch | **different**: hover comes from a sibling TouchArea (292-295) under the input's Rectangle (303-340), so over the text area `has-hover` is likely false and the tint drops. The QML HoverHandler covers the whole pill. Also the 150 ms is hard-coded, not `Theme.hover` |
| 15 | Pill clear button (×, 40×40, icon 20) when text ≠ "": clears and refocuses (229-243) | appstore.slint:341-355 | same |
| 16 | "Categories" caption between Discover and the categories, 12 px gap before Installed (255-277) | appstore.slint:883-915 | same |
| 17 | PlaceRow 44 / 32 high, pill / 8 radius; Material: selected secondaryContainer + 8/10% layer on hover/press, others onSurface layer, 150 ms; Glass: selected fillPressed, hover fillHover, no animation (283-298) | appstore.slint:360-408 PlaceRow | same (Material's 150 ms hard-coded, not Theme.hover) |
| 18 | PlaceRow icon 22 / 16 (Material filled when selected), label weights; Tab focus, Return/Enter/Space activate, focus ring (299-330) | appstore.slint:366-405 | same (Material fill: font's default, documented) |
| 19 | Content: surface / #1E1E21 / #F5F5F7; columns = max(1, floor((w-48+gap)/(300+gap))), gap 12 / 16 (333-339) | appstore.slint:919-927; logic.rs:27 | same |
| 20 | List Flickable, StopAtBounds, hidden while a page shows; heading 32 / large, subtitle per view/filter (341-370) | appstore.slint:929-959; logic.rs:66 `subtitle` | same |
| 21 | Filter chips (search only): All / Arch repos / Flathub / AUR; Material: ticked when chosen, width animates 150 ms OutCubic, outline when off, hover tint; Glass: accentTint + accent border when on, hover fillHover; Return/Space; focus ring (372-434) | appstore.slint:411-461 FilterChip, 962-971 | **different**: width animation `ease-out` (420), QML is OutCubic → `cubic-bezier(0.33, 1, 0.68, 1)` |
| 22 | Choosing a chip: filter set, strips "aur:" from the field when not AUR, load (395-400) | lib.rs:623-633 M::Filter | same |
| 23 | Catalogue notice (no catalogue + pacman): info icon, two lines, "Install Catalogue" / "Installing…" busy while Software Update runs; primary on Material (436-474) | appstore.slint:974-1015; lib.rs:427-428 | same |
| 24 | Loading row: Material spinner 32 + text per view/filter (476-493) | appstore.slint:1018-1032; logic.rs:76 | same |
| 25 | Failure text in danger, wrapped (494-501) | appstore.slint:1033-1037; logic.rs:86 | same |
| 26 | Empty state: title + help per view/filter (502-518) | appstore.slint:1038-1053; logic.rs:90, 98 | same |
| 27 | Discover: sections (title 22 / title) of cards; elsewhere one grid (520-558) | appstore.slint:1056-1087 CardGrid | same; **different (systemic)**: lib.rs:460-463 rebuilds the whole sections model (`ModelRc::new(VecModel::from(..))` for the outer list and each section's apps) whenever the section count changes, which is every Discover load (load() clears to 0). The hand-made `update()` (lib.rs:158-180) also falls back to `ModelRc::new(VecModel::from(rows))` (179). Should use `tessera_core::rows::Rows` |
| 28 | AppCard 88 / 76 high, radius 20 / module-6; Material surfaceContainer + 8/10% layer hover/press; Glass fill / fillHover; 150 ms; pointer cursor; Return/Space opens; focus ring (848-875) | appstore.slint:492-564 | same (150 ms hard-coded) |
| 29 | Card: logo 56 / 44 (AppIcon in the look's style, else initial tile), name, summary (" " if none), source (876-907) | appstore.slint:464-489, 519-553; lib.rs:338-398 | same |
| 30 | Card button: "…" busy, Open installed, Install / Get; installed → open, AUR → its page, else install (908-915) | appstore.slint:554-561; lib.rs:639-646; logic.rs:292 `card_act` | same |
| 31 | Page Flickable: back button (40/24 Material, 28/16 Glass, accessible "Back") → detail=null (562-587) | appstore.slint:1092-1115; lib.rs:647-650 | same. The page scroll goes back to the top for each app (lib.rs:323); QML keeps it (documented) |
| 32 | Back IconButton: press elastic (IconButton.qml ElasticControl), Material radius squares while pressed 150 ms OutCubic, hover fill 150 ms | appstore.slint:188-224 AsIconButton | **different**: no press scale at all (no `transform-scale`); hover duration hard-coded |
| 33 | Page header: logo 112 / 96, name 32 / display elided, summary ≤2 lines elided, source chip (outlined 32 / filled 22) "· verified", "by"/"packaged by" developer in primary / tertiary (589-630, 829-846) | appstore.slint:1116-1173; logic.rs:196, 223 | same (the summary's line count is estimated, documented) |
| 34 | Glass: Get / Open / "Installing…" primary, busy spinner; Remove (danger) "Removing…" when installed (643-651, 665-671) | appstore.slint:1195-1208; logic.rs:135, 145; lib.rs:651-661 | same |
| 35 | Material: SplitButton 48 high, Install / Open / Installing… / Removing…, download / open icon, spinner while busy (652-659; SplitButton.qml 44-106) | appstore.slint:590-663, 1181-1194 | same (press swell approximated, documented) |
| 36 | SplitButton lead: Down opens the menu (`open = true`, only when it has items) (SplitButton.qml 95-98) | appstore.slint:614-617 | **different**: Down *toggles* (`toggle(..)` → `menu-open = !menu-open`, 1191), so a second Down closes it |
| 37 | SplitButton trail: 40 wide, inner radius round while open / 8 pressed / 4 (spring), caret turns 180° in Theme.motion.popover OutCubic; click / Return / Space / Down toggles (109-152) | appstore.slint:664-708 | same (spring approximated; 220 ms hard-coded instead of `Theme.popover`) |
| 38 | Split button's menu: Remove (danger, disabled while busy) when installed, "Open the website" when there's a homepage, "Copy the name"; ≥200 wide, 44 px items, icons 20, hover/press layer, radius 16, shadow; below the button, right-aligned, clamped 8 px; fades in 220 / out 150; a click outside closes (660-662; SplitButton.qml 154-236) | lib.rs:497-508 items, 670-683 M::MenuPick; appstore.slint:1346-1405 | **different**: drawn as a Rectangle (plus a full-window TouchArea) inside the window, not a real popup. Its items can't be reached by keyboard, Esc works only while the button has focus, and it is clipped to the window. Should be a `PopupSpec` popup (see settings `open_menu`) |
| 39 | Error under the buttons (AppStore.errorFor, danger, wrapped) (673-680) | appstore.slint:1210-1216; lib.rs:483 | same |
| 40 | Note "Software Update asks for your password…" while busy and not Flathub (681-686) | appstore.slint:1217-1223; lib.rs:484 | same |
| 41 | AUR warning card: tertiaryContainer / warning tint + border, warning icon, bold first sentence, "hardly anyone uses this" when popularity <0.1 and votes <10, votes · popularity · out of date (689-721) | appstore.slint:1227-1251; logic.rs:154, 168 | same (StyledText line height, documented) |
| 42 | Description when ≠ summary, 15 / body, line height 1.3 / 1.25 (723-732) | appstore.slint:1253-1257; logic.rs:243 | same apart from the line height (documented) |
| 43 | Glass details: "Package p · version · repo", "Website" link: accentInk + underline on hover, pointer, -4 margin, opens (734-756) | appstore.slint:1260-1293 | same apart from the underline (documented) |
| 44 | Material "About this app" Group: Package, Version (if any), From, Website + "Open" text button (757-795) | appstore.slint:713-774, 1295-1341 | same (the website elides where the QML wraps, documented) |
| 45 | Look switch while open (Theme.material bindings everywhere) | lib.rs:577-587 `config_changed` | same |
| 46 | Durations follow Theme.motion (hover 150·pace, popover 220·pace, press 100·pace) | appstore.slint hard-codes 150ms (152, 212, 291, 382, 420, 514, 631, 691, 727), 220ms (700, 1353, 1358), 100ms (146) | **different**: use `Theme.hover` / `Theme.popover` (theme.slint:91-92) so snappy and cool-down pacing apply |

## Test plan

Never install, remove or update a package. Only search, hover, open pages and
menus, and press Esc. Run every step in both shells (`qml`, `rust`) and both looks
(`material`, `glass`), and compare against the QML.

1. `tools/settings-scenarios.sh setup qml material` (then `rust material <bin>`,
   `qml glass`, `rust glass <bin>`), then `tools/settings-scenarios.sh run 'ipc panels appStore'`.
   Then run `shot open`. The window opens centred at 1040×720 (Material) or 980×660 (Glass).
   On 1920×1080 that is about x 440 y 180 (Material) or x 470 y 210 (Glass). Read the real
   positions from the shot. PASS: Discover is selected, the sections are loading
   or loaded, and the search field has the caret (#3, #13).
2. Opening time: record from `ipc panels appStore` to the first frame in both
   shells (tools/anim-timing.py). Rust must not be slower (#1).
3. Hover: glide over the place rows, a card, a filter chip, the back button and
   the pill search (over the text area and the icon). `shot` each. PASS:
   the same hover tints as the QML. The pill keeps its tint over the text area
   (#14, #17, #21, #28, #32).
4. Click "Development", then "Installed", then "Discover" (`clicks places …`).
   PASS: the heading and subtitle change, loading shows, the list scrolls to the
   top, and the selected pill moves (#5, #6, #17).
5. Type: click the search field, then `run 'key -k …'` or `wtype firefox`. Wait 0.4 s.
   PASS: "Results for “firefox”", the chips show, and the loading text names the repos and
   Flathub (#7, #8, #20, #21, #24). Type `wtype x` quickly and check that only one load follows (debounce).
   Esc clears the field and Discover returns after 300 ms (#13, #14). Pill: the × clears it (#15).
6. Type `aur: yay`. PASS: the AUR chip is on and the subtitle says "Community packages from the AUR".
   Click "All". PASS: the field loses "aur:" (#7, #22). Material: record the chip
   width change (150 ms, OutCubic) (#21).
7. Type `zzzzqqq`. PASS: the empty state "No apps found for “zzzzqqq”" with the AUR hint (#26).
8. Keyboard: Tab through the place rows, the chips and the cards. Return opens. PASS:
   focus rings like the QML (#18, #21, #28).
9. Click a card body (not its button). PASS: the app's page shows the logo, name, summary,
   chip and developer. Glass shows Get or Open, plus Remove if installed. Material shows the
   split button and "About this app". Click Back. PASS: the list is where it was (#31, #33-#35, #43, #44).
10. Material page: click the split button's caret. PASS: the menu opens below,
    right-aligned, with "Open the website" (if it has one) and "Copy the name". The caret turns.
    Hover the items. Press Esc. PASS: it closes. Open it again and click outside:
    it closes. Open it again and click "Copy the name": it closes (do not click Remove).
    Focus the lead button and press Down twice. QML: it stays open (#36-#38).
11. Search with `aur: yay` and open an AUR result's page (its card body, not the
    button). PASS: the warning card shows, with votes and popularity (#41).
12. Switch the look while the window is open (Settings → Appearance). PASS: it restyles (#45).
13. Close the window with Super+Q / `hyprctl dispatch closewindow`. PASS: Panels
    closes, and `ipc panels appStore` opens a fresh window again (#2).

## Fixed in batch 3 (fixer)

- The split button's menu is its own popup (AsMenu) under the button's right end, 8 px apart; keys ↑/↓/Enter/Space/Esc; a click elsewhere closes it (and that click on the button doesn't reopen it); Down on the main part only opens it.
- Discover's sections and cards change in place (a new load keeps the cards it still has); durations follow the theme (Theme.hover/popover); the filter chip's width on OutCubic.
- Not done: the search pill's hover over its text; the back button's press scale; keeping the scroll on reloads.

## Fixed in batch 4 (fixer)

- Batch 4: the back button gives 0.97 when pressed.

## Test results

(tester)
