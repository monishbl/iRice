# iRice changelog

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
