# shell/modules/shelf/ShelfProbes.qml → (stays QML; Rust: none, by design — see surfaces/shelf/src/lib.rs header)

The probes are invisible 3 px drop strips (and a 360 px square after a shake) that only tell the shell a drag of files arrived. They draw nothing, so tessera-ui doesn't port them; ShelfProbes.qml keeps running while Rust draws the shelf, and it is also where ShelfFeed is instantiated.

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Instantiates ShelfFeed (the Rust shelf's feed) | 325-326 | (feeds `surfaces/shelf`) | same |
| 2 | Edge strips left/right/top, 3 px, per screen, while Shelf.probing and the edge option | 362-394 | — | QML (not ported, by design) |
| 3 | Shake square 360 × 360 under the pointer while probing | 395-407 | — | QML (not ported, by design) |
| 4 | Files only (file: URLs) → Shelf.dragArrived(where, screen, y); exit → dragLeft; drop → offer + keep | 336-359 | — | QML (not ported, by design) |

## Test plan (tester)
Sandbox, arena 1920×1080, Rust drawing the shelf. Needs a real file drag (a file manager in the arena); without one, mark not testable here.
1. Hold a file drag and `P glide 1919 500 400` (right edge) → the shelf card opens at the right, centred near y 500 (Rust card; ShelfWindow geometry).
2. Hold a file drag and shake it (`P glide` left/right ×4, 200 px, 80 ms each) → the drop zone appears under the menu bar (ShelfDropZone plan).
3. A text drag to the edge → nothing opens.
