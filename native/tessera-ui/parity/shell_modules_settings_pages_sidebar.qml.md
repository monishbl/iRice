# shell/modules/settings/pages/sidebar.qml ↔ surfaces/settings (src/pages/accounts/ai.rs `sidebar`)

The "Sidebar" part of AI & Sidebar (page `ai`; old link `sidebar` → ai,
heading "Sidebar": map.rs 58/77). No menus.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Group "GIFs" + footer (9–12) | ai.rs 415–416 | same |
| 2 | "KLIPY app key": detail "Key saved"/"No key"; secret TextField 240, placeholder "Replace key"/"Paste key"; commits on Enter and on blur: non-empty trimmed → emoji.klipyKey, field cleared; empty does nothing (13–27) | 397–411 (`key.on = true`: blur commits too) | same |
| 3 | "Content filter" Segmented 260 Off/Low/Medium/High → emoji.gifFilter (28–37) | 412–418 | same |
| 4 | More options › "Anime": "Show Anime tab" (sidebar.booru), "Allow explicit results" (sidebar.booruNsfw) + footer (41–54) | 420–430 | same |
| 5 | More › "Overlay": "Gaming overlays" (overlays.gaming) + detail, footer (55–64) | 431–433 | same |
| 6 | Its own More options button inside the part (separate from ai.qml's) (41) | 420 `p.more(cx)` | same |

## Test plan

Setups as pages_ai. `open sidebar` (scrolls to the "Sidebar" heading).
**Read y from a shot.** Never paste a real KLIPY key.

S1 Page: shot. PASS: GIFs group, detail "No key"/"Key saved" as the arena's
config, More options button; open it: Anime and Overlay groups as the QML.

S2 Segmented: click "Medium" → `jq .emoji.gifFilter` "medium"; back to the old value.

S3 Field: click the key field, type `x`, press Tab (blur). PASS: both shells
store "x" (`jq .emoji.klipyKey`), the field empties, detail "Key saved",
placeholder "Replace key". Restore: `run` jq to set it back to the arena's old value.

S4 Switch "Show Anime tab" off/on: config follows.

## Test results

(tester)
