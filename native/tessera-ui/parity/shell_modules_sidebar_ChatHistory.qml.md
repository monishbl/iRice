# shell/modules/sidebar/ChatHistory.qml

Parity (12 Nov, written by the lead; not built or tested yet).

Rust: History in ui/sidebar.slint, sync_history in lib.rs.
- Search field (250 ms debounce to Ai.searchChats), groups, rows with title and "time · model", open/pinned states: same.
- Keys: Down/Up, Return opens, Delete, F2 renames, Escape back: same. Added this round: Menu or Shift+F10 opens the selected chat's menu at its row.
- Right-click a row: its menu: same. Added: press-and-hold 500 ms opens it (TapHandler's long press); a held press no longer also opens the chat.
- The row's ⋯ button: no longer a Tab stop (activeFocusOnTab false in QML).
- Times refresh every minute while past chats are open (added: the minute tick).
- Rename in place, delete with Undo bar: same.
