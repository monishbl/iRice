# shell/modules/sidebar/Sidebar.qml

Parity (12 Nov, written by the lead; not built or tested yet).

Rust: surfaces/sidebar (lib.rs, ui/sidebar.slint).
- Full-screen layer, panel 420 wide at the left under the bar, click outside (below 30 px) closes, Escape closes: same.
- Slides in 40 px and fades (progress over Theme.motion.panel, emphasized/exit curves): same.
- Tabs (Segmented: AI, Translate, Anime when sidebar.booru): same; the tab is kept in Panels.sidebarTab (sidebar.tab): same.
- Keyboard exclusive while open: same. Escape now closes an open language list first (was: the whole sidebar).
- Freed a minute after closing (linger 60 s): the chat is forgotten after 60 s without history (forget_timer): same.
