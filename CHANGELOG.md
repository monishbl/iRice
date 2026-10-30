# iRice changelog

## Fewer password prompts · 2026-10-30

- Software Update asks for your password once, when you press **Update**, and never again in the middle: AUR updates no longer ask at each step.
- Your next update also installs a small iRice system helper (one password, as part of that update). From then on, these no longer ask for your password: the fixes in System health and Settings → Sharing, and using your wallpaper on the login screen. Quick Share's faster Wi-Fi links ask once as they start, never during a transfer. This only applies to an administrator's account, at the computer.
- Until that update, everything asks for your password as before.
- The "Running hot" warning no longer goes off for a few seconds of high load: it waits until the CPU has averaged above your limit for about 30 seconds, and it ends once it has averaged 5 °C below it for a minute (it used to need 3 minutes 10 °C below, so it could stay forever).
- Plugging in or unplugging a display is smooth: the screen you're using no longer goes dark, and a new display starts its connect animation sooner. Unplug one and its windows glide onto the screen beside it, from that side; plug it back in and they glide back once its animation finishes, to the space it showed before.
- Island alerts can be put away: swipe one up (drag it, or scroll up over it), use the small × that appears over it, or press Esc while the pointer is on it. Resting the pointer on alerts keeps them up; when it leaves they carry on, with a second to spare.
- Low battery and **Running hot** stay up until you plug in or the CPU cools down (or you dismiss them). A finished download and a newly connected drive stay up for 10 minutes. Everything else goes away as before.
- Island alerts are kept: Notification Center has an **iRice** group with every alert that wasn't already a notification (devices with their battery, drives, displays, sound output, charging, low battery, running hot, downloads, Wi-Fi and VPN…). Remove one with its ×, the group with its own ×, or everything with **Clear all**. The same alert again within a minute updates its line instead of adding another. They're kept when the shell reloads, like notifications.
- The open island has a **Recent** list of the last few alerts. Swipe a line sideways (or use its ×) to remove it, or **Clear** them all. While something is live (music, a timer), **Recent** opens the list in its place.
- The album art in the open island now follows the island's own rounded corners exactly, with no second, rounder corner or inner outline showing at the edges.
- Incoming calls (Discord, WhatsApp, any app) show as a banner that stays until you answer, decline or dismiss it, or the app stops ringing. They never go to the island.

## Sharing and spaces fixes · 2026-10-30

- Phones show up when you send: while iRice looks for devices it tells phones nearby that someone is sharing, so they make themselves visible (as one phone does for another).
- "Send to a nearby device…" appears in Dolphin and other file managers again (it was added where they didn't look).
- **Send files…** in Control Center: on the Sharing page and in the Sharing tile's right-click menu.
- An empty space between two in use (say 6, between 5 and 7) keeps its chip in the menu bar and its place in Overview, so you can still go to it.
- Fewer warnings in iRice's log (notification banners, the window snap outline, saved settings).
- Updates run in the background at low priority, so the desktop stays smooth while packages install or AUR software compiles.
- One island alert per thing: headphones whose battery shows up a moment later, or a device that reconnects, update the alert already showing instead of adding a second.

## Quick Share: firewall, Wi-Fi Direct and pairing fixes · 2026-10-30

- If your firewall (ufw or firewalld) blocks sharing, Settings → Sharing and System health say so, with an **Allow through firewall** button.
- Quick Share now always listens on port 53318, so one firewall rule covers it.
- The firewall check recognises port ranges, so a firewall that already lets sharing through isn't reported as blocking it.
- Wi-Fi Direct starts again (it was refused by the Wi-Fi service).
- Wi-Fi Direct no longer ends the moment it starts: NetworkManager leaves the link alone while a transfer uses it.
- Quick Share no longer asks for your password for a Wi-Fi Direct link that can't work: NetworkManager ends such links the moment they start, and Intel Wi-Fi can't run a hotspot on 5 GHz. For full speed, have the phone on the same Wi-Fi.
- No more hotspot attempts on 5 GHz Intel Wi-Fi, where they can't work; transfers move on to the next option straight away.
- Phones no longer need to ask to pair: **Stop pairing prompts** in Settings → Sharing turns off the experimental Bluetooth features that caused it (Ranging, and LE Audio controls that made phones take the computer for earbuds).
- A phone shows once in the Bluetooth lists, not once per connection.

## Battery & Power, System health, replies and more · 2026-10-30

**Battery & Power** (new in Settings)
- Choose when the screen dims, turns off and locks, and when the computer sleeps, separately on battery and on the charger.
- Closing the lid: sleep, lock or do nothing, on battery, on the charger, or with another display connected. Choose when the password is needed after sleep.
- One energy mode, with the reason shown: Cool-down, the battery saver, your choice and Game Mode no longer fight each other.
- At 20% the island offers Low Power (or turns it on by itself if you choose). Warnings at 10% and 5%, and the computer sleeps at 3%, even with Caffeine on or if iRice isn't running.
- See which apps used the most energy in the last hour, and quit them.

**System health** (Settings → About)
- Checks every service iRice relies on (sound, network, Bluetooth, sharing, power, portals, the glass plugin…) and says how to fix what's broken. Fix restarts things for you, and asks for your password when it needs to change system services.

**Notifications**
- Reply to messages (Discord, Telegram, Signal, KDE Connect…) right from the banner, Notification Center or the island. Super+Ctrl+N replies to the newest message.
- Notification cards no longer show through the Notification Center header.
- Calendar, To-do and Focus tabs now sit inside their card.

**Google**
- Connect several Google accounts; their calendars show together.
- Create, edit, move and delete Google Calendar events. Repeating events ask "This event" or "All events".
- Changes made offline are saved and sent later. If something changed on Google meanwhile, you're told and can copy what you had.

**Keyboard**
- Every button, switch, slider, list and menu works with Tab, the arrows, Space and Enter, with a focus ring only when you use the keyboard. Screen readers hear each control's name.

**Island and sharing**
- One scroll or swipe over the island moves to the next activity.
- Downloads: no more stuck "Unconfirmed … 0 B" card; a finished download offers Open and Show in Folder.
- The Sharing control in Control Center is a pill like Wi-Fi: tap to turn on for 10 minutes, or open its menu for No one / 10 minutes / Everyone.

## Quick Share: fast transfers, fewer alerts · 2026-10-30

- Quick Share now moves files over a direct Wi-Fi link instead of slowly over Bluetooth: NetworkManager no longer takes the link down as it starts, and the phone is told the link's real channel. If the direct link can't start, a hotspot on your current channel is tried next.
- While Quick Share is on, phones connecting and disconnecting over Bluetooth aren't announced in the island; Quick Share's own prompt and progress say what's happening.

## Wi-Fi and Bluetooth everywhere · 2026-10-29

- Right-click a Wi-Fi network or a Bluetooth device, in the menu bar, Control Center or Settings, for Connect, Disconnect, Forget, Copy name and more. Press and hold works too.
- Long lists of networks and devices scroll instead of running off the screen.
- Settings → Network lists your Wi-Fi networks and lets you join them (including office and hidden networks), with Known networks to forget.
- New Settings → Bluetooth: your devices, nearby devices to pair (found while the page is open), and how this computer appears to others.

## Quick Share over Bluetooth fixed, apps come forward · 2026-10-29

**Quick Share**
- Phones sending over Bluetooth disconnected after two seconds and reported "failed". iRice now answers the phone's connection in time, and its Bluetooth announcement fits your Bluetooth chip.
- If a phone paired with this computer during an earlier try, remove that pairing on both devices first; Quick Share doesn't need it.

**Apps come forward**
- Clicking a link, or "Open in the app" in the browser, brings the app forward even with focus following the pointer. The pointer moves onto it, so focus stays there.
- An app that asks for attention while you're busy elsewhere doesn't steal focus: its dock icon bounces and the island offers Show.
- File pickers and sign-in windows open centred, on top of Settings. While you sign in to Google, Settings waits in the dock and comes back by itself.

## iRice's own Quick Share, enterprise Wi-Fi · 2026-10-29

**Quick Share with Android, built in**
- iRice has its own Quick Share; no extra app is needed. Turn it on in Settings → Sharing or in Control Center: No one, Everyone for 10 minutes, or Everyone.
- Works like it does between phones: nearby phones find this computer over Bluetooth, even without the same Wi-Fi, and files then move over a fast direct Wi-Fi link while your normal Wi-Fi stays connected. On the same Wi-Fi, they simply use that.
- You're asked in the island before anything is saved, with the same PIN the phone shows. Files, links, text and Wi-Fi networks all work.
- A direct Wi-Fi link needs your password once; if you skip it, the transfer still finishes over Bluetooth, slowly.
- Needs Bluetooth on. Settings has a button to turn it on.

**Sharing everywhere**
- The share sheet lists every nearby device in one place: Android phones over Quick Share, everything else over LocalSend.
- A new Sharing control in Control Center, in four sizes: a toggle, Off · 10 min · On, and larger ones with nearby devices.
- Share… in the right-click menu of the desktop, the Downloads stack, and Nautilus, Dolphin, Thunar and Nemo (add or remove them in Settings → Sharing).
- Share selected text with Super+Ctrl+S.

**Report a bug**
- Settings → Report a bug, or from Spotlight, a shortcut or a gesture. Add a screenshot (with Mark up… to hide anything), the iRice log and system info, and see exactly what will be sent first. Personal details like your name, paths, network names and addresses are removed from the log.

**Google**
- Connect Google account opens your browser and comes back connected; no detour through GNOME Settings.
- Sync only fetches what changed: at login, when you unlock, when you open Notification Center, and every 15 minutes while you're using the computer. Nothing while it's locked or idle. Choose which calendars to show.

**Wi-Fi**
- Join office, college and eduroam networks: pick the sign-in method (PEAP, TTLS, TLS or PWD), and use the system's certificates or choose a certificate file. For TLS, add your own certificate and key.
- Join other network… at the end of the list joins a hidden network.
- Joining a phone's hotspot works: Join no longer misses your click while the hotspot comes and goes in the list. If the password is too short, Join says so.

**Fixes**
- Tapping Super twice quickly no longer leaves a half-open Overview that ignores clicks.

After updating, run Update again so it installs the new packages (`iw`, `wpa_supplicant`, `dnsmasq`, `avahi`, Python libraries).

## Fixes after review · 2026-10-29

**Calendar**
- A monthly repeating event in any calendar stopped every Calendar widget from showing, and emptied Notification Center's calendar. Fixed.
- In Notification Center, a busy week shows a few items per day and "+N more", so it no longer stretches the panel.

**Google**
- Edits and deletions always reach the task you meant, even while a sync is running. Changes show at once and are sent when Google is free.
- Deleting a Google task can be undone for 8 seconds.
- If the Google helper can't run, Settings says what to install.

**AI**
- An API key is only sent over plain http:// to this computer. Remote servers need https://.
- Removing a model also removes its key from the keyring, with 8 seconds to undo.

**Desktop colours**
- The Dolphin colour fix no longer replaces a Qt or KDE colour scheme you picked yourself.

**Battery and stability**
- Background helpers left behind by a crashed shell are cleaned up again.
- Less background work: state saving, the icon index and download progress all cost less.
- Glass shapes no longer stay on screen after the shell quits.
- A damaged state file is set aside instead of stopping settings from saving.

**Lock, sleep and login**
- The Locking animation setting works again.
- If the screen can't be locked, the computer stays awake and tells you, instead of not sleeping without a word.
- The login screen has clickable Sign in and power buttons, and plain error messages.

**Other**
- Spotlight's Lock Screen locks straight away again.
- A new window no longer pulls focus or the pointer when your window rules say it shouldn't.
- The Today screen shows your events and to-dos when it opens at login.
- Sharing settings show Quick Share's status in one row.

## Glass calendar and full weekly lists · 2026-10-29

- The large Calendar widget keeps its glass background. The list of the day's events now slides up over the calendar, which fades and blurs behind it.
- The week view lists everything on the desktop widget. Add events and to-dos with the + button.

## Calendar containment and scrolling agenda · 2026-10-29

- The large Calendar widget stays inside its frame in every view. Scroll to see all of a day's events and to-dos; the arrow at the top brings the dates back.
- Scrolling over the widget no longer changes the month. Use the arrows for that.

## Super-tap correction verified in the running session · 2026-10-29

- Tapping Super on its own opens Overview again. Super with another key works as before.
- Dolphin and other Qt apps keep readable colours after an update restarts the shell. Quit and reopen Dolphin once if it still looks wrong.

## Audit fixes and reliable startup · 2026-10-29

**Lock and sleep**
- The screen locks immediately, before any animation, and Sleep waits until the lock is really in place.
- Locking and sleeping when idle now works in every iRice session.

**Your data**
- Settings and UI state survive crashes and two shells writing at once. Notes are saved safely and you're told if a save fails.
- Updates back up your configuration first and put things back if something goes wrong.

**Sharing**
- LocalSend uses HTTPS and checks the other device's certificate. Devices with HTTPS off must turn it on to receive.

**Login screen**
- A new, simpler login screen: pick the user and session, type your password.

**Fixes**
- Repeating calendar events on month ends, leap days and intervals.
- Tiled or floating at startup, timers that drifted, and results from an older search replacing newer ones.
- Notifications no longer offer a Reply that didn't work.

After updating, run Update once more so it installs `openssl` and `plasma-integration`, then log out and in again.

## Pointer focus and multiple displays · 2026-10-29

- With focus following the pointer, the window under it gets focus again after you switch spaces or a window closes.
- Each display's current space stays visible in the menu bar's spaces, even when there are more spaces than fit.
- Settings → Mouse: stop the pointer slipping to another display at shared corners or edges, with an adjustable corner size.
- A screenshot selection stays on the display where you started it.

## Google integration and clipboard image reuse · 2026-10-29

- **Settings → Google:** sign in with your Google account (through GNOME Online Accounts; no Google Cloud project needed). Your Google Calendar events and Google Tasks show in the Calendar, Reminders and Today widgets, and you can edit tasks.
- Google Keep opens in the browser; it can't be synced.
- The clipboard panel loads image previews once and keeps them, and previews every image in your history.

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
