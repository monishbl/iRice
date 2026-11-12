# shell/modules/controlcenter/WifiList.qml

Wi-Fi networks. Rust: host/system/src/accounts/cc.rs wifi_page/wifi_block, rows in `Block`.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| sort connected/known/signal, top 30 | 57-59 | host | same |
| snapshot: re-sort every 4 s only when not hovered/asking | 60-101 | host/system/src/accounts/cc.rs hold_order + cc.hover | fixed: rows no longer move under the pointer |
| password field focused when it opens | 293-294 | pw init focus | fixed |
| Enter joins / Escape cancels | 295-296 | pw | same |
| hint cleared when the prompt changes | 102-106 | host/system/src/accounts/cc.rs | fixed |
| prompt cleared on hide | 140-150 | host/system/src/accounts/cc.rs open_edge | fixed |
| enterprise form / share card / join other | 262-272, 368-376 | Settings | missing: open Settings → Network (WifiJoinForm, WifiShareCard) |
| scanner on while shown | 131-138 | one rescan | different |
| prompt scrolls into view | 119-129 | — | missing |

## Fixed in batch 2

- lists hold still
- password focus
- hint
- prompt cleared on close

## Still different / missing

- join form, share card
- continuous scanning
- scroll prompt into view

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Wi-Fi page with several networks (arena: fake list, or real). Rest the pointer on the list for 10 s: rows do not reorder. Move away: within 4 s they re-sort.
2. Click a secured unknown network: the password field appears **focused** (type at once, Enter joins).
3. Type 3 chars, Enter: "at least 8 characters" hint. Escape, open another network's prompt: no hint.
4. Open a prompt, close CC, reopen: no prompt.

## Test results

(tester)
