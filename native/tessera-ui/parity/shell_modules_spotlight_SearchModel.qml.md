# shell/modules/spotlight/SearchModel.qml → surfaces/spotlight/src/lib.rs (`open`, `refresh`, `run`, `key`, `ask_button`, `back`)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | reset on open: answer off (stop quick AI), field cleared, results cleared, selection 0, field focused, clipboard refreshed | 18-25 | `open` (ai.stopQuick, clipboard.refresh, `invoke_open`) | same |
| 2 | typed: leave the AI answer, results = Search.query(text), selection 0 | 27-34 | `edited` → `refresh(true)` | same (the shell's rows — calendar, contacts, store, clipboard — come from the host in the same loop turn; while on their way, ≤ 120 ms, the old list stays: no flicker) |
| 3 | later answers (files, conversions, definitions): results again, keeping the row moved to (by title), else 0 | 37-49 | `refresh(false)`, `rank::keep_selection` | same |
| 4 | run(i): an AI row answers in place; others close Search then run | 51-62 | `run` | same |
| 5 | askAi: what's typed (minus "?") to the AI; empty → sidebar | 66-82 | `ask_button` | same |
| 6 | backToResults: answer off, results again, field focused | 84-91 | `back` | same |
| 7 | Enter: continue in chat while answering, else run the selection | 96-98 | `key` Return | same |
| 8 | Down/Up clamp; Tab next, wrapping | 99-107 | `key` | same |
| 9 | Shift+Space / Ctrl+Y on a file row: Quick Look | 108-113 | `handles`, `key` → panels.quickLook | same |
| 10 | Results reach the view without remaking every row | (QML ListView over a new JS array remakes its delegates) | **was** a new model on every keystroke; now one `Rows` set by position | **fixed** (better than QML) |

## Test plan (tester)
Both looks; `inbox 'ipc panels open spotlight TESSERA-TEST'`.
1. `rec sp-type 1 "wtype fir"` → results within one frame of the last key; first changed frame after the last key: Rust not later than QML. Selection on row 0.
2. `wtype -k Down -k Down` → row 2; `-k Up` ×5 → row 0 (no wrap); `-k Tab` repeatedly → wraps to 0 after the last.
3. Type `2+2` → answer "4" first; `-k Return` → `wl-paste` gives 4, Search closed.
4. Type `/.bash`: file rows arrive later; move to the 2nd file row before they all arrive; the selection stays on that title as more come.
5. On a file row: `wtype -M shift -k space -m shift` → Quick Look; `-M ctrl -k y -m ctrl` → the same.
6. Sparkle (Ask) with an empty field → sidebar opens. `?hello` + Return without AI set up → Settings → AI.
7. Close and reopen after typing: field empty, no results.


## Round 2 (after panels-batch2 results)
| # | Finding | Cause | Fix |
|---|---------|-------|-----|
| R1 | Software rows missing (`fir`: Top Hit Firefox "Get · Flathub" + a Software group; `9*9foot`: "Looking for apps…") | standalone: host-desktop's `search.query` always answered `"store": []` and never asked the store | `search.query`'s store flag now asks host-sharing's `appstore.lookUp` (or `forget`), sends its `appstore.quick` apps as Software rows ("Get · <source>[ · N votes]", the store's icon; a "Looking for apps…" / "Looking on Flathub…" row while loading), and answers again whenever the store's revision changes (`settled`). Clicking one opens the store on it (`panels.openAppStore`). |
| R2 | `fi` (Material): QML Bulk Rename, Thunar Preferences, Calculator, Foot Client; Rust Calculator first | equal scores (keyword "files"/"file size" → 48) kept the app list's order; QML's order is DesktopEntries' hash order | ties now broken by name (`core/src/rank.rs`). Note: no "Calculator" desktop entry was found in the arena image's layers, so whether it ties can't be checked here; if it still leads, its score differs (report its desktop file) |

Re-test: both looks, `wtype fir`, wait 1.5 s → Software rows; `wtype 9*9foot` → "Looking for apps…"; `fi` order.
