# Desktop batch 1 results (tester, arena 9, bin desktop-b1-63308b3)
Shots in /home/user/tessera-verify/d9/ (c-*.png = QML vs Rust montages; qml-* / rust-* per look). Recordings deleted after timing.
Method: foot windows floated via window_rule to x>=1000 (widgets uncovered); region-changes over 32,54 704x524.

## DesktopWidgets (and the no-reload step of WidgetFrame, StackWidget, ShortcutsWidget, ClockWidget, DesktopIcons)
- No reload on window open/focus/close: PASS in all four combos (Rust glass, Rust material, QML glass, QML material): 0 changed frames over the widgets while windows opened (window region did change), focused (click) and closed.
- Right-click widget menu (stack): PASS, same items/✓/positions as QML (material shot rust-material-menu-stack vs qml-material-menu-stack). Escape closes: PASS.
- Right-click bare desktop menu: PASS (Edit Widgets, Change Wallpaper, Use Stacks ✓, Show Files ✓, Desktop Settings) at pointer, downward. Material only; Glass not checked.
- Edit Widgets: cells + badges + gallery appear: PASS (material, shot only, fade/badge spring timing NOT measured). Bare-desktop click ends editing: PASS.
- Drag: clock widget dragged, saved to config (clock:m:4:1): PASS (material). FAIL: dragging the shortcuts widget starting on a tile (600,130) does nothing in Rust; QML moves it (shortcuts:m:6:1). Repro: material, move 600 130, down, glide 600 330 400, glide 1400 330 400, up; expect config desktop.widgets shortcuts to change.
- Resize via menu (batteries -> Small): PASS, in place, other widgets 0 changed frames. Glass NOT RUN.
## ShortcutsWidget
- FAIL: clicking a tile does nothing in Rust (App Drawer at 604,175: no launchpad), QML opens Launchpad. Both looks (c-sc-lp.png, rows 1-2).
- Hover brightens, press grows/squares corners (material): PASS by shot; timing NOT measured.
- Icons: QML tiles use filled icons, Rust outline icons (material); minor.
## StackWidget
- Scroll one notch flips one page (material, shot): PASS; timing NOT measured (recordings hung/no-change). Dot hover/click jumps page: PASS (material).
## ClockWidget
- FAIL (look): material clock digits are bold in Rust, light weight in QML (c-material-idle.png). Material palette also differs: QML brown/yellow container, Rust pink/peach (check seed handling). Glass: QML day label orange vs Rust red.
- Tabular digits, minute-change redraw: NOT RUN.
## WeatherWidget (in the stack)
- FAIL: Glass weather medium has an hourly row (00 03 06 09 12 with temps) in QML; Rust shows blank space (c-glass-idle.png). Weather icon is filled in QML material, outline in Rust.
## BatteriesWidget: ring/This computer same as QML, PASS by shot (both looks); Bluetooth rings NOT RUN.
## DesktopIcons
- FAIL: with Use Stacks on, the Documents stack icon is a generic white document in Rust, a red PDF icon in QML; Archives/Folders fine. (c-material shots rust-material-idle vs qml-material-menu-stack)
- Items 2-5 NOT RUN.
## AnalogClock, NowPlaying, Photos, WidgetGallery keyboard/hover, WidgetFrame lock/unlock/badge timing, launchpad/Today/background/wallpaper: NOT RUN (time).

## Dock (Dock, DockSlot, DockItem, DockMenu, MaterialDockItem, DownloadsStack, MaterialDownloadsStack)
- Dock present both looks; hover name labels (Apps/Downloads/Trash): PASS. Glass magnification grows icon under pointer in both (curve/timing NOT measured: recordings returned no usable frames).
- FAIL (icons, root problem), both looks: Colloid place/app-grid icons not loading in Rust dock. Glass: Apps = white mono grid on dark tile (QML coloured 3x3 on white tile); Downloads = grey flat folder on dark tile (QML blue Colloid folder-download, no tile); Trash = blank dark tile (QML grey tile with trash can). Material: Downloads = pale pink flat folder, Trash = blank brown disc (QML blue folder, grey trash can). Shots c-qml-glass-dock.png vs c-rust-glass-dock.png, same for material.
- FAIL: trash state. Same arena home: QML shows trash full and "Empty Trash…" red/enabled; Rust greyed and blank icon.
- Right-click Downloads/Trash menus open with right items: PASS. Downloads click opens stack with correct Colloid mimetype icons (zip, txt, png, pdf): PASS both looks. Title/Open in Files present.
- Escape closes menus and stack: PASS.
- A stray empty outlined rectangle below the dock appeared once in Rust material while a menu/window had just closed (rust-material-menu-stack.png), gone later: NOT reproduced, note only.
- Items 4-7 (pinned/minimise/urgent), autohide, GlassDock timing: NOT RUN.
## Launchpad (opened via ipc)
- FAIL: all app icons in Rust launchpad are monochrome tinted glyphs (generic) in Categories view on both looks; QML shows coloured real Colloid app icons (c-sc-lp.png rows 3-4). Also Rust material/glass category tiles lay out differently.
## Icons check: missing under Colloid: dock Apps (view-app-grid), Downloads (folder-download), Trash (user-trash / full), launchpad app icons (all), Documents desktop-stack icon.
Counts: PASS 13, FAIL 10, NOT RUN many. No file fully verified, no tracker changes.
