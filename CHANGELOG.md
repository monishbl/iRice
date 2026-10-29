# iRice changelog

## Audit fixes and reliable startup · 2026-10-29

- Lock immediately before animations and wait for compositor lock confirmation before Sleep. Start an independent idle/sleep locker in installed and TTY sessions, preserving configured idle listeners.
- Protect UI state with private recovery journals and merge concurrent edits; publish notes atomically and report save failures. Back up installed configuration before replacement and restore files when update preparation fails.
- Use HTTPS with certificate fingerprint checks for LocalSend. Bound transfer resources, reject truncated/oversized uploads, honour cancellation and avoid concurrent filename overwrites. HTTP-only receiving peers must enable HTTPS; discovery is not independently authenticated.
- Correct floating/tiled startup rules, calendar recurrence and date boundaries, stale request results, timer drift, process identity checks and native glass IPC retries. Stop advertising notification inline replies until a composer exists.
- Replace the greeter with the approved minimal keyboard-first login prompt, retaining user/session selection and confirmed power actions.
- Restore Super-tap Overview with an explicit release binding. Initialize Qt apps with the KDE platform theme so Dolphin starts with readable dark-mode text; explicit platform-theme choices are preserved.
- Record the newly reported calendar overflow/gradient agenda, Settings window stacking, app-link activation, stale download activity, island scrolling and notification-header issues as **open R51–R56**. These are not claimed fixed in this update.

### Upgrade and validation

- Install/update dependencies, including `openssl` and `plasma-integration`, rebuild the compositor plugin and native helpers, and restart the session. Fully quit and reopen Dolphin so it inherits the startup theme environment.
- Validation: **50 Python and 27 Node tests pass**; 286 QML files parse; shell syntax checks and both native builds pass. Tests include offscreen Qt/Quickshell, disposable Git repositories and local TLS/IPC fixtures.
- Real lock/suspend/resume, greetd authentication and physical multi-monitor acceptance checks remain pending. This push does not install or restart the running desktop.
- See the [audit status and queued issues](docs/bug-audit-2026-10-28.md) for individual fixes and verification limits.

## Pointer focus and multiple displays · 2026-10-29

- Hover focus now rechecks the stationary pointer after workspace/window changes. New windows take focus and bring the pointer when they open elsewhere.
- Every displayed monitor's active workspace stays highlighted and visible through workspace-strip overflow.
- Settings → Mouse adds shared-corner protection, edge resistance/push again, selection-only confinement, and Off. Corner size is adjustable. Requires the rebuilt compositor plugin; pointer constraints in games take precedence and deliberate pointer warps remain available.
- Region selection is clipped to its starting monitor, including off-screen windows. Native pointer confinement has a short lease so it releases if the shell exits during selection.

## Google integration and clipboard image reuse · 2026-10-29

- Settings → Google adds Desktop OAuth sign-in, Calendar sync and primary-calendar quick-add, and Google Tasks sync/editing in existing reminder widgets. New tasks remain local unless a Google task list is selected. Desktop Online Accounts is the default sign-in route; custom OAuth is optional. Real-account verification is pending.
- Keep opens in the browser; native personal Keep sync is not supported by this implementation. See [setup and limitations](docs/google-integration.md).
- Clipboard previews now use stable cached image URLs and atomic decoding, retain unchanged history models, and cover all images in the 200-item history window instead of only the first 48.

### Upgrade and verification notes

- Rebuild the iRice compositor plugin and restart the session to activate pointer barriers and reliable stationary-pointer refocus. Screenshot clipping works without the plugin.
- Google sign-in uses GNOME Online Accounts. New installs include `gnome-online-accounts`, `gnome-control-center` and `python-gobject`; existing installs need those dependencies before using the account flow. Users do not need a Google Cloud project for this route.
- Validation: 29 Python/C++/Lua regression tests and 5 screenshot-geometry tests passed; changed QML parses; isolated Quickshell account/focus checks passed; the native plugin builds against installed Hyprland 0.56.2 headers.
- Live Google authentication, physical multi-monitor pointer behavior, workspace-chip appearance and clipboard-grid rendering still need verification. The live compositor was not hot-reloaded during development.
- Added the detailed bug audit and missing-feature/power strategy documents. Findings in those reports remain tracked work unless explicitly marked implemented; this update does not fix every audited issue.

## Your own AI models · 2026-10-28

- Settings → AI now lets you add, edit, remove and select models with a display name, model ID, provider and credentials. New installations have no preset models; existing selected models, extra models and provider keys are preserved.
- Supports Google AI Studio, Vertex AI (Gemini, with an Express API key or OAuth access token), OpenRouter, Anthropic, OpenAI, Ollama and custom OpenAI-compatible servers. Connection-specific fields appear as needed.
- Model selection is shared by sidebar chat and Spotlight. The same model ID can be used across different providers and servers, with credentials stored separately from model definitions.

## Dolphin colors and easier file sharing · 2026-10-28

- Dolphin's text and background now follow iRice's light/dark mode even with Match apps off. KDE and Qt palettes stay consistent, including after turning accent matching off.
- Settings → Sharing now has a Send files button with multiple-file selection. Spotlight also has a Send Files action. LocalSend sending works independently of receiving.
- Added optional Android Quick Share / Nearby Share integration through Packet, available from the share sheet, Sharing settings and Spotlight. Packet handles device selection, confirmation and progress; it requires Bluetooth and the same Wi-Fi network.

## Keyboard power confirmation and window focus · 2026-10-28

- Spotlight's power commands open the session menu with the requested action selected. Use the arrow keys to change it, Enter to confirm, or Esc to cancel. Mouse clicks only select an action, and holding Enter cannot accidentally confirm it.
- App windows can take keyboard focus when they open or request activation, without waiting for the mouse to move.

## Island closes without glitches · 2026-10-28

**Fixes**
- When the island closes, its contents and pin button go at once instead of being cut off by the shrinking edges, and its background fades along with the shape instead of stalling and then popping.

## Island closes cleanly · 2026-10-28

**Fixes**
- Closing the island no longer turns it into a round blob or cuts off its contents. It shrinks smoothly back into the small pill.
- The system stats row in the open island is centred.

## Smoother island · 2026-10-28

**Fixes**
- The album art no longer reloads and flickers each time you open the music player in the island.
- The volume and brightness island is a pill from the start, instead of a rectangle that turned into one.

## Hover tips across the menu bar · 2026-10-28

- Point at a menu bar icon for a moment to see what it's doing:
  - **Wi-Fi:** the network and signal.
  - **Bluetooth:** what's connected and each device's battery.
  - **Sound:** volume and output.
  - **Others:** weather, Focus, keep awake, recording time, the song playing, keyboard layout, app tray icons, and the full date with unread notifications.
- Icon-only buttons (Search, AI, Control Center and the extra buttons) show their name.

## StandBy stays centred, and update conflicts you decide · 2026-10-28

**StandBy**
- The clock now always stays centred across the screen. To protect against burn-in it only moves up and down, a good distance each minute.

**Software Update**
- A file that both you and the update changed is now listed for you to decide: **Merge both** (when the two changes fit together cleanly), **Keep mine** or **Use the update's**. Before, your version was kept without a word and the update's change was lost.
- Also listed: files deleted on one side and changed on the other, binary files, and a file of yours with the same name as one the update adds.
- Whenever you choose the update's version, yours is saved in the update's backup folder first.

## Cooler, safer updates, and fixes · 2026-10-28

**Battery and heat**
- Fixed: the screen-sharing detector kept a CPU core busy, and every shell restart left another copy running, which made the laptop hot. It now costs almost nothing, and nothing iRice starts in the background outlives the shell any more. A new shell also ends anything an earlier one left behind.
- Lighter background checks for the camera, location and file copies.
- Stage Manager previews pause when hidden and are capped at 5 frames a second. A live wallpaper pauses when the screen is locked or idle.
- The island's music bars and heat ripples redraw less often, and pause when hidden or locked. The ripples stop entirely during Cool-down.
- The whole shell now idles below 1% of one core.

**Software Update**
- Only one update runs at a time, across every iRice session.
- A failed iRice update puts everything back exactly as it was, your own changes included.
- Package lists from an update are checked as plain text and never run. Only valid package names reach pacman.
- AUR updates are now a choice in the plan (off by default), because AUR packages are built from user-submitted scripts.
- Once your password is accepted, a system update can't be cut off halfway by the shell restarting or closing.

**Fixes**
- The microphone stays muted after a recording if you muted it during one: fixed.
- Discarding two recordings in a row left the first file behind: fixed.
- The Today screen could open at 4 AM if you were still at your desk, and then not in the morning: it now waits for your next login or unlock.
- A removed Today widget no longer comes back after a restart.
- A display-connect animation could replay when another screen's finished: fixed.
- Clipboard times were missing after a restart until the next copy: fixed.
- Green and orange accents get dark text on their buttons, for contrast.
- Quit leaves apps alone when it can't tell whether a "Save changes?" window is open.

## Today, a smarter accent, and apps that behave · 2026-10-28

**Today**
- On your first unlock after 4 AM, a glass panel greets you with today's events, your to-dos and the weather (now, high and low, rain, and the next hours), with a rising chime timed to the animation.
- Choose how it appears (Bloom, Shrink or Drop) and whether it shows every morning, as a desktop widget, both, or not at all: Settings → Desktop & Widgets → Today.
- **Pin for today** keeps it on the desktop until midnight. Set the name it greets you by in Settings → About.

**Accent colour**
- Your accent is used where it reads, a lighter shade of it on dark backgrounds when needed, and neutral otherwise. It's never darkened, so yellow no longer turns brown.
- Control Center looks like macOS: an "on" button is a white circle with the icon in your accent, or in near-black when the accent can't be read on white.
- Desktop widgets (clock, calendar, reminders, Screen Time, stocks) use your accent.

**Desktop calendar**
- The Calendar widget now works like Notification Center's: the month with busy days, Month and Week views, quick add, and a To-do tab where you can add, rename, tick off and delete (with Undo).

**Clipboard (Super+V)**
- Grouped by when you copied things: Just now, hours ago, Yesterday, weekdays, then dates.
- Bigger image previews, and a two-column grid on the Images tab.
- Fixed: the panel could be empty again after an update restarted the shell.

**Battery**
- The battery menu shows battery health (maximum capacity, condition, and cycle count when reported) and how much power is going in or out.
- Point at the battery for its charge, time left and energy mode.

**Glass and icons**
- New glass optics in Settings → Appearance → Glass → Advanced…: index of refraction, refraction width and dispersion, with a live preview.
- Clear and Tinted app icons are see-through glass tiles with one-tone logos that read evenly across apps; Tinted colours them with your accent.

**Apps and windows**
- Apps no longer open maximized over everything; allow it per app in Settings → Windows.
- Menus and dropdowns in apps like Claude and Webex stay open when you move the pointer.
- Webex shows its own name and icon. Quit now ends apps that keep running in the tray.

**Fixes**
- Sliders no longer scroll the page when your pointer drifts while dragging.
- Brightness and volume icons match the rest of Control Center.
- The island's status row shows system stats only; your events moved to Today.

## The update screen waits for your desktop · 2026-10-28

**Fixes**
- After an update, the screen could go black for a few seconds before your wallpaper and menu bar came back. The update screen now stays until your desktop has drawn, then fades out over it.
- Clicking the update screen while it works does nothing.

## One update for everything, a screen recorder, iOS-style Control Center · 2026-10-28

**Software Update**
- One **Update** button now updates your system and iRice together, in a safe order:
  1. a snapshot first, if snapper or Timeshift is set up;
  2. your system (`pacman -Syu`), with the packages iRice needs;
  3. AUR packages, if you use yay or paru;
  4. iRice itself, rebuilt for the Hyprland you now have.
- Your password is asked in iRice's own dialog. No terminal opens, and the update keeps going if you close Settings.
- **Show details** shows the full output while it runs. The island shows progress.
- Nothing is checked on its own any more. Press **Check for updates** when you want to.
- If Arch has a newer Hyprland than iRice is tested with, you choose each time: keep the current one for now, or update anyway.
- These notes, and a list of the packages each update touches.

**Screen recording**
- Press Super+Shift+5, or Record in Control Center, to open a recording bar: Entire screen, Window or Selected region.
- Options: a microphone (pick which one), system audio, the mouse pointer, and a 3 or 5 second countdown.
- While recording, the island shows the time with Stop. Open it for Pause, Mic mute with a level meter, and Discard (8 s to undo).
- Recordings are saved to `~/Videos/Screen Recordings` as MP4 at 60 fps. Change this in Settings → Desktop & Widgets → Screen recording.
- Uses [gpu-screen-recorder](https://git.dec05eba.com/gpu-screen-recorder/about/), which encodes on the graphics card. Without it iRice falls back to wf-recorder: no pause, one audio source.

**Control Center**
- Wi-Fi, Bluetooth and the other wide controls are pills. When on, the button turns white with a coloured icon.
- Square tiles like Now Playing use iOS rounding, with continuous corners.
- Brightness and volume are capsules without labels. In Edit Controls, choose wide or tall for each.
- CPU & Memory opens a page with graphs of the last minute, in four rows: processor and temperature, graphics, memory and swap, and network down and up.
- A speed test, in the Wi-Fi menu and in Settings → Network.

**Menu bar and island**
- The battery is drawn like iOS 27: a solid capsule, red when low, yellow in power saver, with a bolt or plug when charging. Show the percentage inside it, beside it, or not at all in Settings → Menu Bar.
- Wi-Fi, Bluetooth and the Control Center icon match it.
- Privacy dots are only for the microphone, camera and location. Screen sharing is detected from the actual stream and shown in the island or as a dot, your choice, with **Stop sharing**.
- Spaces with windows show as short pills, empty ones as dots. With more spaces than shown, choose arrows or dots that shrink and fade at the ends.
- 18 more menu icons to choose from, in Settings → Appearance.

**Displays**
- Plugging in a display plays a short animation from the side it's on: Extend (a soft, blurred wipe), Liquid (a drop that grows to fill the screen) or Warp.
- An optional sound comes from that side. Try them in Settings → Displays.

**Windows and spaces**
- New windows open tiled or floating, your choice, with exceptions per app in Settings → Windows. Dialogs always float, and switching rearranges what's open.
- A macOS-style Overview: a strip of your spaces on top and search in the middle. The space wallpaper is now a setting, off by default.
- Live window previews are capped at 30 frames a second, and only when Live is chosen. Mission Control and Stage Manager follow the same setting.

**Settings**
- Grouped like System Settings, with a toolbar (back, forward, and where you are).
- Long pages open subpages, like Appearance → Glass.
- Search stays at the top, folds to a magnifier when you scroll, and ends when you click anywhere else.

**Look**
- Glass surfaces have continuous (squircle) corners. The dock is a rounded rectangle, with a slider for the space between icons.
- Clear and tinted icons lift each logo off its tile, with a contrast slider.

**Fixes**
- The clipboard panel was empty in the iRice login session. It now shows your history.
- The screen-sharing dot no longer flickers when iRice draws its own window previews.

<!-- packages: +gpu-screen-recorder -->

## Spaces you can reorder, and a tap of Super opens Overview · 2026-10-28

**Spaces**
- With more spaces than **Spaces always shown**, the menu bar shows a window of them with arrows at the ends. A dot under an arrow means a space past it has windows.
- Drag a space in Overview or Mission Control to put it somewhere else. Its windows move with it.

**Keyboard**
- A tap of Super on its own opens Overview. Super held with another key works as before.

**Dock**
- The dock is a wide glass slab with softly rounded corners and more room around the icons, like macOS Tahoe.
