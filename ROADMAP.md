# Roadmap

How iRice covers each end4 (illogical-impulse) feature, and what's left.

## Foundation
- [x] Nested sandbox with a headless test output (`./dev.sh`)
- [x] Design tokens, persisted config (`~/.config/irice/config.json`, live-reloaded)
- [x] Liquid glass shader: wallpaper refraction, specular rim, dispersion, soft shadow
- [x] Frosted tier (compositor blur via `BackgroundEffect.blurRegion`) and `auto` (liquid over wallpaper, frosted over windows)
- [x] About This Computer
- [x] Reduce Transparency (Solid tier, opaque menu bar), light mode
- [ ] Ultra tier (hyprglass): not built, see README
- [x] Performance: lazy panels, shared baked backdrop, 4 always-on windows per screen, ~0.1% idle CPU
- [x] Wallpaper accent extraction → "wallpaper" accent in Settings

## Shell (end4 equivalent → iRice)
- [x] Bar → menu bar (system menu, app name, status items, clock)
- [x] Dock → glass dock with magnification, running dots, launch bounce
- [x] Right sidebar toggles/sliders → Control Center
- [x] Notification popups → glass banners
- [x] Sidebar notifications + calendar → Notification Center
- [x] Overview search → Spotlight (apps, calculator, actions, `;` clipboard, `>` command, web)
- [x] Menu bar menus: system, app, Window; tray icons with glass menus
- [ ] Global app menus: needs an AppMenu registrar service (not in Quickshell)
- [x] Session screen → shutdown/restart dialog
- [x] Lock screen
- [x] OSD → volume/brightness HUD
- [x] Control Center sub-pages: Wi-Fi list, Bluetooth devices, sound output, volume mixer
- [x] Overview (workspaces + window previews) → Mission Control
- [x] Left sidebar: AI chat (Claude default; Gemini, OpenAI-compatible, Ollama), translator, anime/booru
- [x] AI: conversation persistence
- [x] AI: image input (screenshots), streaming verified against a mock server
- [ ] AI: verify with a real key
- [x] Spotlight: emoji, files (calculator is built in; qalc not needed)
- [x] Polkit agent dialog
- [x] Region selector: screenshot, OCR, recording (image search skipped: needs uploading to a third party)
- [x] Screen translator
- [x] Wallpaper selector
- [x] Cheatsheet (keybinds, periodic table)
- [x] On-screen keyboard
- [x] Overlay widgets (notes, recorder, FPS limiter, crosshair, resources, floating image)
- [x] Desktop widgets (clock, weather)
- [x] Screen corners, media controls, todo, pomodoro, timer
- [x] Settings app
- [x] Services: weather, updates, SongRec, battery warnings, idle (EasyEffects: launch it from Spotlight)

## Round 3: end4 bar parity and windows
- [x] Spaces indicator with app icons, window title, resources, weather, keyboard layout, unread badge
- [x] Battery, Wi-Fi, Bluetooth, Sound, resources and weather menus
- [x] Menu bar buttons, scroll gestures, auto-hide, per-screen
- [x] Control Center toggle grid (game mode, anti-flashbang, Identify Song, EasyEffects, WARP, …)
- [x] Decorations setting: none / window controls / hyprbars title bars (plugin build untested here)
- [x] Minimize to a dock section
- [x] Runs on Quickshell 0.2.1 (end4's build) and 0.3

## Round 4: beyond end4
- [x] Tiling: Window menu layouts, green-button tile grid, Super+Ctrl shortcuts, arrange, return to previous size
- [x] Stage Manager with a thumbnail strip (Control Center toggle)
- [x] App Library with category folders, A–Z and search; Apps button in the dock
- [x] Clipboard manager: pins, image previews, plain-text paste
- [x] Focus modes: allowed apps, wallpaper and appearance per mode, schedules, timed modes, menu bar icon
- [x] Calendar: .ics subscriptions (recurrence, moved and cancelled instances), local reminders, alerts, Spotlight quick-add
- [x] Dynamic wallpapers (time of day, light/dark), live video wallpapers, Auto appearance
- [x] App icon styles: Default, Dark, Clear, Tinted, Auto
- [x] Login screen for greetd (installer option, preview mode)
- [x] AI: ask about the screen (image input), quick answers in Spotlight
- [ ] Test on the real session: hyprbars title bars, greetd login, live wallpaper with hardware decoding

## Round 5: polish and real Liquid Glass
Done:
- [x] tty session: shell watchdog, Super+Ctrl+Alt+R force restart, Ctrl+Alt+Backspace exit, `./dev.sh tty-stop`
- [x] Freeze guards: panels can't linger after closing (CloseGuard), Super+Escape closes everything
- [x] Menu bar: click another title to switch, same title to close, slide along the bar to switch
- [x] Unread indicator styles (bell + dot default, dot, count, bell + count, off); decluttered defaults
- [x] Dock rewrite: fixed window (no flicker), hover lift, optional magnification, recent apps, minimized thumbnails, Downloads stack, Trash, right-click menus, macOS-style auto-hide
- [x] Squircle icon tiles (auto/neutral/accent) and Tahoe-style Dark/Clear/Tinted glyph rendering
- [x] Wallpaper change: double-buffered crossfade, no blank frames
- [x] Notifications: stacked banners with expand, only new ones slide in; Notification Center grouped by app
- [x] Super+Shift+S screenshot of an area
- [x] Sandbox virtual pointer for real hover/click tests

- [x] Hyprland glass plugin (irice-glass): Liquid / Frosted / Solid presets, Lens setting (Thin / Depth / Auto), Frosted fallback without it
- [x] Window controls drawn by the plugin; benchmarked against hyprbars and the shell pill (all within noise; only the plugin keeps up with moving windows)
- [x] Dynamic island: space dots, compact live activity, expanding events, hover card; resources only when tight; optional HUD
- [x] Tahoe Control Center layout with Edit Controls (add, remove, resize, drag)
- [x] Super-tap overview with search and drag-to-space
- [x] Gestures: touchpad swipes, Super+scroll, touchscreen edge pulls on the menu bar and dock
- [x] Elastic pull on popovers, buttons and dock; Subtle / Expressive motion; adjustable continuous window corners

Needs testing on real hardware (can't be driven in the nested sandbox):
- [ ] Super tap, touchpad gestures, touchscreen edge pulls
- [ ] Plugin glass and window controls on the host session

## Round 6: island and desktop widgets
- [x] Black dynamic island with glass rim; iOS-style notification cards (optional)
- [x] Animated alerts per kind: ripples, battery and charging rings, sound waves, drop-in, shake
- [x] Device alerts: Bluetooth (battery ring), USB devices and drives (Open), displays, audio output and wired headphones
- [x] Desktop widgets on a grid: 10 kinds, small/medium/large, drag to snap, right-click menu, Edit Widgets gallery
- [ ] Device alerts on real hardware (plugging things in can't be done in the sandbox)

## Delivery
- [x] Real Hyprland config (SUPER keybinds, layer rules, animations)
- [x] Install script and dependency list
- [x] Default wallpaper
