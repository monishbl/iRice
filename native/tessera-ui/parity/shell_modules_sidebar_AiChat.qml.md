# shell/modules/sidebar/AiChat.qml

Parity (12 Nov, written by the lead; not built or tested yet).

Rust: surfaces/sidebar.
- Header: model picker (chip; Material pill in secondary container while its menu is open; not configured: opens Settings → AI), past chats and new chat HeaderButtons: same. HeaderButton now takes Tab focus, Space/Enter, keyboard focus ring, Material press swell (fixed this round).
- Composer: Return sends, Shift+Return a new line, Ctrl+V adds a clipboard picture else pastes: same.
- Tools: + (file picker), camera (click: screen; right-click toggles its menu; press-and-hold 500 ms opens it — added this round), caret (menu), lasso (circle to search): same. Ghost buttons now Tab-focusable with ring, Glass squish (0.97/0.95/0.9 by motion), Material swell (fixed this round); disabled ones no longer dim (the QML doesn't).
- Send button: Material swell while pressed (added), arrow cursor as the QML (was a hand).
- Files dropped on the chat: the pictures added, "Drop pictures to add them" frame while dragging over the panel (added this round via core's data device; only on the AI tab, not in past chats).
- Pictures waiting (64 px, remove ×), blind notice with Switch / Add a model and Search the web: same.
- List scrolls to the end on new rows and while streaming: same.
- Different: whitespace-only text shows the send button enabled (the service ignores it on send).
