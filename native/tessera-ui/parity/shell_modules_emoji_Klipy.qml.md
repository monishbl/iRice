# shell/modules/emoji/Klipy.qml → surfaces/emoji (src/klipy.rs; paging and states in src/lib.rs `gif_*`)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Key: KLIPY_API_KEY, else Config emoji.klipyKey; nothing fetched without one | 15-16, 38-43 | `key`, `gif_fetch` | same |
| 2 | Trending (q "") / search, 24 a page, customer id, locale country, content filter, gif,jpg | 63-77 | `url` | same |
| 3 | Rows: still (sm/xs/md jpg) for the grid, small gif, share gif (hd/md/sm); type gif only; empty url dropped | 99-118 | `parse` | same (the small animated gif isn't used: see PickerWindow) |
| 4 | Same query already loaded/loading with rows: kept | 44-45 | `gif_fetch` | same |
| 5 | Next page on the last six or the end; a failed next page keeps what's shown (badKey replaces it) | 57-61, 133-141 | `gif_more`, `gif_page` | same |
| 6 | Status: 0 → offline, 401/403 or result false → badKey, other → error (code), parse fail → error −1; 12 s timeout → offline | 81-131, 143-157 | `fetch` (curl --max-time 12) | same |
| 7 | Older answers ignored (serial) | 64, 82 | `serial` | same |
| 8 | A new key starts over (clears the list, status idle) | 159-164 | `config_changed` | **fixed** (was: the old key's GIFs stayed until another query) |

## Test plan (tester)
Arena 1920×1080, panel centred x 750..1170; GIF tab at (1141, 398).
1. Without a key: `P click 1141 398` → "Add a KLIPY key to use GIFs" with Open Settings / Get a key (same as QML).
2. With a key (mark not testable here if the arena has no network or key): GIF tab → "Loading trending GIFs…" then a 3-column grid; type `cat` → after 350 ms "Searching GIFs…" then results; scroll to the end → spinner under the grid and a next page appends without the grid jumping.
3. Change emoji.klipyKey in the config while the GIF tab is open → the grid clears and loads again with the new key (bad key: "KLIPY didn't accept the key").
