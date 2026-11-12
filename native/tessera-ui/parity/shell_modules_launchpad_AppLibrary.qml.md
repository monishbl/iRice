# shell/modules/launchpad/AppLibrary.qml

A Scope holding both looks (AppLibraryGlass, AppDrawer), each made only in
its look.

| Behaviour | QML | Rust | Status |
|---|---|---|---|
| One drawer per look; switching looks swaps them | 7-10 | one `LaunchpadPanel` drawing both (Theme.material), restyled on a look change (config_changed / plugin_changed: theme, glyphs, icon style) | same |
| PanelLoader: opens per screen through Panels ("launchpad") | AppLibraryGlass 15-20, AppDrawer 18-23 | command "open"/"close" with the screen; one window, moved to the asked output | same |
| Kept made between openings (PanelLoader keeps the window through the close) | PanelLoader.qml | the window is kept (hidden, not dropped) after the first open; lists are kept `Rows` | same |

## Test plan

1. Open the launchpad in Material, close; switch appearance.look to glass
   (config.json) and open again: the Glass App Library shows (not the
   drawer), and back.
2. Open, close, open again within 1 s: the second open starts at once
   (no frame of an empty window; record and compare the first moving frame
   time to the first open's).
