# shell/modules/appstore/AppStoreFeed.qml ↔ host/sharing/src/appstore.rs, host/sharing/src/lib.rs, host/shell/src/panels.rs, surfaces/appstore/src/lib.rs

The bridge between the shell's store (services/AppStore.qml) and the App Store
window drawn by tessera-ui. It sends the surface its state, answers, changes and
"show" lines, and it carries out the surface's calls. In the standalone Rust
shell, host-sharing's `Store` holds the store and speaks the same line protocol
to the surface. There is no Material twin: the file has no visuals.

## Behaviours

| # | Behaviour (QML line) | Rust place (file:line) | Status |
|---|---|---|---|
| 1 | `file(u)`: file:// → decoded path, / or https:// kept, else "" (28-31) | host/sharing/src/appstore.rs:95-103 `file` | same |
| 2 | `enrich(a)`: adds `_icon` (AppStore.iconFor → file) and `_source` (sourceName) (32-34) | appstore.rs:139-147 `enrich`, 119-137 `icon_for`, 105-112 `source_name` | same |
| 3 | state feed { open, catalogue (false && pacman), updates (Updates.running), jobs, app (enriched) } sent whenever it changes (35-45) | appstore.rs:442-445 `store_feed`; lib.rs:562-568 `publish` (sent only when it differs from `last_store`) | same |
| 4 | Sent again when tessera-ui starts or starts drawing it (61-69) | lib.rs:784-799 (`last_store` reset on config/outputs change), `settle` | same |
| 5 | Counting: while open, AppStore.users++ and refreshStatus once; users-- when closed or destroyed (47-59) | appstore.rs:194-207 `store_sync` (`counted`), 176-191 `store_users` / `store_refresh_status` | same |
| 6 | Panels.appStoreApp set → tell `show` (enriched) (76-82) | panels.rs:407-416 asks appstore.app; appstore.rs:500-505 tells `show` | same |
| 7 | AppStore.changed(key, installed) → tell `changed` (83-88) | appstore.rs:364-367 `store_done` | same |
| 8 | call "load": token, view, category, query, filter → AppStore.ask discover / category / installed / search (q without "aur:", limit 40, sources all → repo+flathub); answer { token, error, apps, sections } enriched (92-110) | appstore.rs:451-471 `store_call` load, 233-237 Pending::Load answer | same |
| 9 | "close" → Panels.appStoreOpen = false (111-113) | appstore.rs:472-476 (`panels.closeAppStore`); panels.rs:267 | same |
| 10 | "install" / "open" / "remove" when the app has a key (114-125) | appstore.rs:477-494 | same |
| 11 | "catalogue" → AppStore.installCatalogue (126-128) | appstore.rs:495 | same |
| 12 | "website" → Launch.openLink (129-131) | appstore.rs:496 `open_link` | same |
| 13 | "copy" → clipboard (132-134) | appstore.rs:497 (`wl-copy`) | same |
| 14 | Surface side: reads state / answer / changed / show (the window opens on `open`, closes on !open) | surfaces/appstore/src/lib.rs:516-569 `said` | same |

Nothing here is missing or different. The surface's own findings (the in-window
menu, the sections model, hard-coded durations, the pill's hover) are in
AppStoreWindow.qml's parity file.

## Test plan

Never install, remove or update a package. Run each step with
`tools/settings-scenarios.sh setup qml|rust material|glass <bin>`.

1. `run 'ipc panels appStore'`. PASS: the window opens and Discover loads (#3, #5, #8, #14).
   `run 'ipc panels close'` or close the window. PASS: it closes. Then reopen it (#9).
2. Open Spotlight, type an app name (e.g. "gimp"), and choose its "Get" row while the store
   is closed. PASS: the store opens on that app's page. With the store already open,
   do it again for another app. PASS: the page switches (#6).
3. Pick each place and search (repos, Flathub, AUR). PASS: an answer arrives for
   each, with logos (`_icon`) and source names ("Arch repos", "Flathub", "AUR · community") (#1, #2, #8).
4. On an app's page (Material), open the split menu and choose "Copy the name".
   Then `run 'wl-paste'`. PASS: the name was copied (#13). If it has a website, choose "Open the
   website" and check that a browser launch is asked for in the log (#12).
5. Without the catalogue (if the arena has none), the notice shows in both shells (#3).

## Test results

(tester)
