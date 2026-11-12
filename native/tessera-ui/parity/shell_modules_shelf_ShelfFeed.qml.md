# shell/modules/shelf/ShelfFeed.qml → surfaces/shelf (src/logic.rs `Feed::parse`; src/lib.rs `message`, `shell::ask` calls)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Sends `{state:{opened, items[{uri,path,name,kind,text}], screen, side, atY, zone, zoneScreen}}` on change, on start and when drawing switches | 238-265 | `Feed::parse`, `follow` | same |
| 2 | Calls: drop [uris,text], removeAt i (bounds-checked), clear, hide, share (with paths, on the shelf's screen), hovered bool, zoneHover bool, cancelZone, keep [uris,text], sendOn [uris] | 266-305 | `M::*`, `drag_card`, `drag_zone`, `say_hovered`, `set_over` | same (hovered/zoneHover sent only on change) |
| 3 | Pictures gone from the shelf drop their thumbnails | — | `thumbs.retain` | Rust only (cache) |
| 4 | A feed that changes one item changes only that row | (ListView delegates) | `CardWin.rows: Rows` | **was** a new model per feed; now in place |

## Test plan (tester)
Sandbox, arena 1920×1080.
1. `inbox 'ipc shelf open'` → the card appears (opened reached it). `inbox 'ipc shelf close'` → it fades.
2. `inbox 'ipc shelf zone'` → the drop zone appears on the focused screen; `P click 960 30` → cancelZone, it goes.
3. With items (ShelfWindow step 3): Clear (`P click 1856 66`) → the shell's Shelf empties and the card shows the empty state; Share (`P click 1826 66`) → the share sheet opens with the shelf's files.
