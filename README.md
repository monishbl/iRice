# iRice

A Hyprland desktop shell in the style of macOS Tahoe, with Liquid Glass, everything end4's illogical-impulse dots offer, and more from macOS: Stage Manager, window tiling, Focus modes, an App Library and a matching login screen. It's written from scratch in [Quickshell](https://quickshell.org) and tuned for integrated GPUs.

## What's in it

**Desktop**
- Transparent menu bar with everything end4's bar offers:
  - system, app and Window menus, plus the window title
  - Spaces with app icons (click or scroll to switch)
  - tray icons with glass menus, Now Playing and a recording indicator
  - status items with their own menus: battery and energy mode, Wi-Fi, Bluetooth, sound, CPU and memory, weather, keyboard layout
  - every item can be hidden in Settings → Menu bar, including the app and Window menus, Search, AI, Control Center and the clock
  - optional buttons: screenshot, colour picker, mic, keyboard, dark mode, energy mode, recording
  - unread notifications as a bell with a dot, a dot, or a count; scroll gestures (left half brightness, right half volume), auto-hide, per-screen
  - click a menu to open it, another title to switch, the same title to close
- **Dynamic island** in the menu bar, black like the iPhone's: dots for your spaces, a compact live activity (recording, timer, music, stopwatch), and alerts that spring open with their own animation: headphones and other Bluetooth devices connecting (with a battery ring), USB devices and drives (with Open), displays, the charger filling its ring, sound moving to another output, low battery, track changes, Focus, CPU or memory running hot. When the CPU package passes a temperature you set (90 °C by default), a "Running hot" alert opens and, until it cools, the closed island turns red with heat ripples from the active space. Alerts that arrive together stack in one island and stagger in, each leaving on its own time. Live activities (music, a timer, the stopwatch, recording) sit in the closed island: art, a scrolling title and playing bars, with controls; they replace the space dots, or sit around or beside them (Settings → Menu bar, where every piece can be switched off). Point at it for a mini player over the album art with seek, open-the-player and volume. Several live at once: scroll over the island to switch, or list them all. Pin it open with the pin or a right-click. Notifications can appear there as iOS-style cards instead of banners. Hover it for details; the volume and brightness HUD can show there too. Preview any alert with `qs ipc call island demo device` (or usb, display, audio, charging, battery, notification).
- **Overview:** tap Super for this screen's spaces as plain containers with a tile per open window (app icon and name). Drag a tile to another space, or type to search apps.
- **Privacy dots:** orange while an app uses the microphone, green for the camera, purple while the screen is shared; click for which apps.
- **More live activities:** downloads and file copies (cp, mv, rsync, dd) with progress and speed; calls and meetings (Discord, Zoom, Teams, Slack, Google Meet and more) with mute, open and leave. Wi-Fi, VPN, hotspot and Ethernet changes are alerts. The status row of the open island shows the CPU temperature next to CPU.
- Window decorations (Settings → Appearance): none, window controls, or macOS title bars via the hyprbars plugin. Window controls are a small pill at the top of each window that opens into close, minimize and zoom; with the glass plugin, Hyprland draws them so they move with the window.
- Window corners: adjustable radius with continuous (squircle) corners
- Minimize: windows park in the dock's Minimized section
- **Dock:** recent apps, minimized windows, a Downloads stack and Trash, right-click menus, optional magnification and macOS-style auto-hide (rest at the bottom edge to bring it up). Unread-count badges from apps that report them (Telegram, Discord, Thunderbird...) and a dot when an app wants attention. Click the app in front again for App Exposé.
- **Desktop widgets** on a snapping grid: Clock, World Clock, Weather, Calendar, Reminders (with due dates), Batteries, Battery History, Now Playing, System, Screen Time, Stocks, Shortcuts (one-click buttons), Photos, Note (each with its own text) and **Smart Stack** (several widgets in one, scroll to flip; it brings forward music, the next event or a low battery), in small, medium and large sizes. Drag to move; right-click for sizes, Remove and the widget's own options; Edit Widgets (right-click the desktop) opens a gallery to add more.
- **Files on the desktop** (~/Desktop) on the same grid, from the top right: double-click to open, drag to arrange, right-click for Quick Look, Share, Move to Trash (with Undo). **Use Stacks** groups them by kind.
- Rounded screen corners

**Windows**
- **Tiling:** halves, quarters, thirds, fill and centre from the Window menu, from the tile grid that opens when you rest on the green window button, or with Super+Ctrl+Arrows / F / C. **Drag a window to a screen edge or corner** and a glass preview shows where it will go (needs the glass plugin). Arrange two to four windows side by side; Return to Previous Size puts a window back.
- **App Exposé:** one app's windows from every space (three fingers down, or click the app in front in the dock).
- **Picture in Picture:** browsers' video pop-outs float small in a corner on every space; any window can do it from the Window menu or Super+Shift+P.
- **Window tabs:** merge windows into tabs (Window menu or Super+T, Super+[ / ] to switch). Tabs look like Hyprland's bar restyled, or like Finder's (Settings → Appearance).
- **Stage Manager:** one group of windows in front, the rest as thumbnails on the left. New apps open in their own stage; Shift-click a thumbnail to merge it into the front one. Turn it on in Control Center.

**Apps**
- **App Library** (Super+Shift+Space or the dock's Apps button): every app in category folders or A–Z, with search and keyboard navigation.
- **App icon styles** (Settings → Appearance): Default, Dark, Clear, Tinted, or Auto (dark in dark mode), like Tahoe.

**Panels**
- **Control Center:** laid out like Tahoe's, each control its own glass chip (or on one glass panel: Settings → Control Center → Panel background): Wi-Fi, Bluetooth, Focus, Now Playing, display and sound sliders and more. Choose Edit Controls to add, remove, resize and drag controls, from a gallery that also has game mode, anti-flashbang, Identify Song, keyboard, colour picker, screenshot, recording, battery, weather, CPU and memory, EasyEffects and Cloudflare WARP (hidden when their tool isn't installed). Right-click a control (or click a wide one's label) for its details, which open as a second popover beside Control Center: Wi-Fi networks, Bluetooth devices, Focus modes, and sound output with a per-app mixer.
- **Notification Center:** history grouped by app, calendar with your events, to-do list, and a Pomodoro timer with stopwatch.
- **Notifications:** glass banners with actions. Several stack into one with an expand button, and only a new one slides in.
- **Spotlight:** apps, calculator, unit and currency conversion ("10 km to mi", "100 usd in eur"), definitions ("define serendipity"), actions, calendar events, `;` clipboard history, `:` emoji, `/` files by name and by what's inside, `>` commands, web search. Shift+Space or Ctrl+Y on a file opens Quick Look. Ask AI right in Spotlight (`?` or the Ask row) for a short answer, then continue in the sidebar. Type "remind me to call mum at 5pm" to add a reminder.
- **Clipboard** (Super+V): history with image previews, search, pins that survive restarts, paste as plain text.
- **Quick Look:** previews of images, video, audio, PDFs, text and folders without opening an app (Spotlight, the Downloads stack, desktop files).
- **Nearby sharing**, like AirDrop: send files to phones and computers running LocalSend (Share in Quick Look and on desktop files), and receive from them after an Accept in the island (Settings → Sharing: No one, Everyone for 10 minutes, Everyone).
- **Mission Control:** live window previews and Spaces.
- **Sidebar:**
  - **AI chat:** Claude by default; Gemini and any OpenAI-compatible endpoint, including Ollama and OpenRouter.
  - **Translator**
  - **Anime image browser:** safe-rated results unless you switch on Explicit.

**Focus and calendar**
- **Focus modes:** Do Not Disturb, Work, Personal, Sleep, and your own. Each lets chosen apps through, can change the wallpaper and light/dark mode while on, and can run on a schedule, for an hour, or until the evening. The active mode shows in the menu bar.
- **Calendar:** subscribe to Google, iCloud, Outlook or Nextcloud calendars with their .ics link (Settings → Calendar). Events show in Notification Center and Spotlight, with reminder notifications. Add your own reminders in plain words.

**Tools**
- Screenshots of the screen, a window or a selection; Super+Shift+5 opens a toolbar like macOS. Area screenshots open in Swappy to crop, draw and annotate (Settings → Keyboard Shortcuts).
- **Ask about the screen** (Super+Shift+A): select part of the screen and ask the AI about it ("what's this error?").
- Copy text from the screen (OCR), translate text on screen, record the screen.
- Overlay widgets (Super+G): notes, resources, recorder, volume mixer, floating image, crosshair, MangoHud FPS limit. Widgets can be pinned and made click-through.
- On-screen keyboard, colour picker, keyboard-shortcut cheatsheet, periodic table.

**System**
- **Lock screen** (Settings → Lock Screen): iOS layout (large clock, small widgets, Now Playing with controls, notifications) or macOS layout; cards in glass, frost or solid; light, dark or automatic text; notification previews optional.
- **StandBy:** on a charging laptop left alone, the lock screen becomes a large clock, two widgets or your photos (red at night), and the screen stays on.
- Power dialog, polkit password prompt, volume and brightness HUD.
- **Sound effects:** a chime at login, the shutter for screenshots, paper for Empty Trash (Settings → Notifications).
- **Match apps** (Settings → Appearance): GTK, Qt/KDE, terminal (foot, kitty, Alacritty) and browser colours from your accent or wallpaper, with matugen. **Accessibility:** Reduce transparency and Increase contrast. Tahoe-like defaults: MacTahoe icons and the macOS cursor are used when installed.
- **Login screen** that matches the lock screen (greetd), set up with `./install.sh --greeter`.
- **Wallpapers:** dynamic (time of day using sunrise and sunset, or light/dark pictures), live video wallpapers that pause when covered, and an Auto appearance that turns light at sunrise and dark at sunset. **Wallpaper sets** are found in your wallpaper folder automatically: pictures named "Name Light" and "Name Dark" switch with the theme, and a sub-folder of pictures follows the time of day; pick one in Settings → Wallpaper. (macOS wallpapers are Apple's and aren't included; if you have them, name them that way, e.g. "Tahoe Light.jpg" / "Tahoe Dark.jpg".)
- Battery warnings, update notices, idle handling (hypridle).
- Settings app (Super+,) for appearance, wallpaper, menu bar, dock, desktop and weather, notifications, Focus, calendar, AI, sidebar and shortcuts.

## Glass

Choose the material in Settings → Appearance: Glass, Frost or Solid, or your own mix of the three with Advanced… (lens, frost and opacity sliders; the 3D highlights fade as it frosts and are gone when solid):

| Material | What it does | Cost |
|---|---|---|
| **Liquid** (default) | Refracts and lenses whatever is behind it, windows included | Low |
| Frosted | Blurs what's behind it | Low |
| Solid | No transparency; also makes the menu bar opaque | None |

**Lens** sets how thick the glass looks: Thin (edge lensing only), Depth (a thick lens that bends the whole shape), or Auto (depth on large panels, thin on small controls).

Real Liquid glass over windows needs the **irice-glass** Hyprland plugin, which the installer builds (or Settings → Appearance → Build plugin…). Hyprland then draws the glass itself, and only where it repaints anyway, so static glass costs nothing. Without the plugin, Liquid refracts the wallpaper only and falls back to Frosted over windows. Rebuild the plugin after a Hyprland update; until then iRice uses Frosted.

The shell idles below 0.1% CPU. A live wallpaper is the exception while it's visible (about 10% of a core for a 720p video in testing); it pauses when windows cover most of the screen.

**Motion** (Settings → Appearance): Subtle, or Expressive for springier popovers, buttons and dock. Press and drag on a popover, menu or button and it gives like rubber, then springs back; nothing moves on hover.

## Install

On Arch Linux (a fresh install or your current one), one line does everything:

```sh
bash <(curl -fsSL https://raw.githubusercontent.com/monish-cosol/iRice/main/setup.sh)
```

It downloads iRice to `~/.local/share/irice/repo` and runs the installer, which:
1. checks your system (Arch, sudo, internet, disk space, a GPU render device, virtual machine)
2. updates the system and installs Hyprland, Quickshell and everything iRice uses (all from the official repositories)
3. optionally installs the Tahoe icons, macOS cursor and GTK theme from the AUR (it sets up `yay` if you have no AUR helper)
4. compiles the shaders and builds the Liquid Glass plugin against your Hyprland
5. installs the shell (`~/.config/quickshell/irice`) and its Hyprland config, backing up yours once (`~/.config/hypr.pre-irice`)
6. optionally downloads macOS wallpapers and starts you on Tahoe (light by day, dark at night)
7. turns on NetworkManager, Bluetooth, power profiles and, in a VM, the guest tools
8. sets up the iRice login screen (greetd), or starts Hyprland when you log in on tty1
9. checks the result and tells you how to fix anything that's off

Each step can be retried or skipped if it fails, the installer can be run again at any time (it updates), and everything is logged to `~/.cache/irice/install.log`. From a clone: `./install.sh`. Options: `--yes` (no questions), `--dry-run`, `--dev` (link the shell to the clone), `--no-deps`, `--no-greeter`, `--no-extras`, `--verify`.

Afterwards the `irice` command looks after it:

| Command | |
|---|---|
| `irice doctor` | check everything (in a session, also that the shell and glass plugin are running) |
| `irice update` | get the latest iRice and install it |
| `irice rebuild` | rebuild the glass plugin and shaders, e.g. after a Hyprland update |
| `irice repair` | restart the shell (duplicated bars, a frozen shell); apps keep running |
| `irice logs` | the shell's log |
| `irice uninstall` | remove iRice, put your Hyprland config and login screen back |

**In a virtual machine** (for trying it or a demo): give it 4 GB of memory, 2+ cores and 3D acceleration, or Hyprland can't start: in virt-manager, *Video: Virtio* with *3D acceleration* and *Display Spice: OpenGL* on; in VirtualBox, *VMSVGA* with *Enable 3D Acceleration*. Install Arch (e.g. `archinstall` with a user that has sudo, NetworkManager, and no desktop), log in, run the line above and restart. The installer turns on the software cursor and the guest tools for the VM by itself.

Put your own Hyprland changes in `~/.config/hypr/custom.lua`. iRice loads it last, and an error in it won't break the rest of the config.

Optional extras:
- `songrec` for Identify Song
- `mangohud` for the FPS limit
- `ollama` for local AI
- `easyeffects`, `cloudflare-warp-bin` for their toggles
- hyprbars for title bars: Settings → Appearance → Decorations → Title bars → Install… builds it with `hyprpm`

To preview the login screen from your session: `IRICE_GREETER_MOCK=1 qs -p shell/greeter.qml` (the password is "irice"). iRice runs on Quickshell 0.2 or newer, including end4's pinned build, so the installer doesn't replace an installed one.

## Keys

Super is ⌘. Press **Super+/** for the full list, which is read live from Hyprland.

| Keys | Action |
|---|---|
| Super (tap) | Overview of spaces, with search |
| Super+Space | Spotlight |
| Super+Shift+Space | App Library |
| Super+V | Clipboard history |
| Super+Tab | Mission Control |
| Super+A | AI and tools sidebar |
| Super+C / Super+N | Control Center / Notification Center |
| Super+, | Settings |
| Super+Shift+3 / 4 / 5 | Screenshot screen / area / tools |
| Super+Shift+S | Screenshot of the screen (saved straight away) |
| Super+Ctrl+Shift+S | Screenshot of an area |
| Super+Escape | Close everything (if a panel is stuck) |
| Super+Ctrl+Escape | Repair the shell: duplicated bars or a frozen shell. Restarts only the shell; apps and windows keep running (also in the system menu) |
| Super+Shift+T / Y | Copy text from screen / translate screen |
| Super+Shift+A | Ask AI about part of the screen |
| Super+Shift+R | Start or stop recording |
| Super+G / Super+K | Overlay widgets / on-screen keyboard |
| Super+Ctrl+Q | Lock |
| Super+Q, F, D | Close, full screen, zoom window |
| Super+Ctrl+Arrows | Tile to a half of the screen |
| Super+Ctrl+F / C / R | Fill, centre, return to previous size |
| Super+Shift+P | Picture in Picture |
| Super+T, Super+[ / ] | Merge into tabs (or move a tab out), previous / next tab |
| Super+1–9 | Spaces |
| Super+scroll | Next or previous space |

**Gestures**

| Gesture | Action |
|---|---|
| 3 fingers left or right | Switch spaces |
| 3 fingers up / down | Overview / App Exposé |
| 4 fingers left / right | Sidebar / Notification Center |
| Pinch / spread 4 fingers | App Library / Show Desktop |
| Three-finger drag (Settings → Trackpad) | Move the window under your fingers; the swipes above move to 4 fingers |
| Touchscreen: pull down from the menu bar | Control Center (right third) or Notification Center |
| Touchscreen: swipe up from the bottom edge | Show a hidden dock |

## Files

- `~/.config/irice/config.json`: settings. Edits apply live.
- `~/.config/irice/focus.json` and `calendars.json`: Focus modes and calendar subscriptions (also editable in Settings).
- `~/.local/state/irice/`: layout, to-dos, notes, clipboard pins, your reminders, AI chat and API keys when no keyring is running. Private files are owner-only.
- `~/.cache/irice/`: the baked glass backdrop, Quick Look's PDF pages.
- With Match apps on: `irice-colors.*` files next to your GTK, terminal and Qt configs (each config gets one include line); turning it off empties them.

## Trying it without installing

From a text console (Ctrl+Alt+F3), run `./dev.sh tty`. It starts iRice as its own Hyprland session with real input, while your normal desktop keeps running (Ctrl+Alt+F2 to switch back, Super+Shift+Escape or Ctrl+Alt+Backspace to end). Its settings live in the sandbox folder.

If it freezes:
- A watchdog restarts the shell when it stops responding; Super+Ctrl+Alt+R restarts it by hand, Super+Escape closes any stuck panel.
- If the whole session hangs, switch to another console (Ctrl+Alt+F4) or your normal desktop and run `./dev.sh tty-stop` (`--force` to kill it outright, `--dry-run` to see what it would stop). It only touches the test session.

## Development

All development happens in a nested Hyprland inside a distrobox (`./dev.sh`), never on the live session. It has its own home folder, runtime directory, D-Bus session and a headless 1920×1080 test output for screenshots.

```sh
./dev.sh create      # one-time: create the container and install the toolchain
./dev.sh up          # start the sandbox (Alt stands in for Super)
./dev.sh reload      # rebuild shaders and restart the shell
./dev.sh ipc panels open controlCenter IRICE-TEST   # drive the UI
./dev.sh shot shot.png                               # screenshot the test output
./dev.sh hyprctl …   # talk to the sandbox compositor
./dev.sh logs | down
./dev.sh perf [secs] [phases]   # CPU, memory, frames and glass copies per scripted phase
```

What the sandbox shares with the host, and how it's guarded:
- **Audio:** the host's PipeWire, so volume changes are real. `IRICE_HOST_AUDIO=0` turns this off.
- **System D-Bus:** network and Bluetooth toggles act on real hardware.
- **Guarded:** power actions only notify, the polkit agent isn't registered, and host AI keys are stripped.
- **Processes:** distrobox shares the host process list. `./dev.sh down` and `reload` find sandbox processes by their runtime directory, so never use `pkill`.

Layout: `shell/` (Quickshell config), `hypr/` (Hyprland config), `sandbox/` (nested compositor), `tools/` (shader, icon, emoji and wallpaper generators). [ROADMAP.md](ROADMAP.md) tracks status.

## Not included

- **Calendar editing:** subscribed calendars are read-only (CalDAV writes would need an account integration); your own reminders are editable. Times with a time zone are read as local time.

- **Global app menus:** Linux apps would need an AppMenu registrar service, and Quickshell doesn't provide one. The menu bar shows the app and window menus instead.
- **hyprglass "Ultra" tier:** the plugin hooks Hyprland internals and wasn't tested here. Auto, Liquid, Frosted and Solid cover the looks without it.
- **Vertical or bottom bar:** end4 offers these; a macOS menu bar stays at the top.

Fonts and icons: Inter, Phosphor (MIT) and Papirus. The bundled wallpapers are generated by `tools/gen-wallpapers.sh`.
