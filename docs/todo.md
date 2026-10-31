# iRice: later

## New features: chosen 31 Oct, not started

Ask about each one's design before building; one at a time.
- **Voice typing anywhere** (like Windows Win+H): a shortcut, speak, and the
  text types into the focused app. Offline (whisper.cpp or similar), nothing
  sent anywhere.
- **Live captions** (like Windows 11 / Android): subtitles for any audio the
  laptop plays (calls, videos) in a small floating bar. Offline.
- **Drop shelf** (like Yoink / Dropover / ChromeOS Holding Space): shake a
  file while dragging to open a shelf that holds it; collect from several
  places, then drop them all at once.
- **Shake to find the pointer** (like macOS / KDE): wiggle the mouse and the
  pointer grows big for a moment.
- **Undo a bad update** (like openSUSE / Silverblue): Software Update already
  takes a snapper snapshot before updating (snapper + btrfs-assistant are on
  this laptop); add "Go back to before the last update" in Settings and a way
  back at startup if the desktop won't load.
- **Phone as a webcam**: now part of "Android phone on the computer" below.

## Integrations: chosen 31 Oct, not started

Official or open ways in only (published APIs, open standards, the Linux
stack), like Quick Share. For every iRice user, not one laptop: each shows
only when the computer, phone or account supports it, and says what's
missing otherwise. Ask about the design before building; one at a time.

### Android phone on the computer (like Phone Link / Phone Hub)
- Mirror and control any Android phone in a window, use its apps, drag
  files onto it, and use its camera as a webcam for calls.
- Tools: ADB (Google's official Android tool) + scrcpy; the webcam through
  scrcpy's camera source (Android 12+) into a v4l2loopback virtual camera
  (a kernel module: one-time root setup, through irice-root).
- Needs USB debugging switched on once (Developer options). Then cable-free
  with Wireless debugging (Android 11+): Settings shows a pairing QR code the
  phone scans (adb pair, found over mDNS). Many phones turn wireless
  debugging off after a restart or Wi-Fi change: explain that plainly and
  offer the cable as the fallback.
- Ideas: phone in the dock while connected; "Use phone camera" in the
  camera chooser; screen off on the phone while mirroring.

### Google Drive and Microsoft 365 (OneDrive, Outlook calendar, To Do)
- Drive and OneDrive as folders in Files/Dolphin and file pickers; Outlook
  calendar events and Microsoft To Do in Today and the Calendar, like Google
  Calendar/Tasks now.
- Google: the existing GNOME Online Accounts sign-in, with Files on. GNOME
  apps get it through gvfs; Dolphin (KIO) doesn't see gvfs, so check
  kio-gdrive, or an rclone mount (official APIs, works in every file
  manager) as the common path.
- Microsoft: GNOME Online Accounts' Microsoft 365 account (official
  Microsoft Graph API): OneDrive through gvfs (same Dolphin question),
  calendar and To Do read through Graph like Google.qml does. Work/school
  accounts may need their admin's approval: say so when sign-in is refused.
- Offline: show what's cached, never fake a sync.

### Share Wi-Fi by QR code (like Android / iOS)
- "Share" on the connected network in the Wi-Fi menu: a QR code a phone's
  camera joins from (the standard WIFI:T:WPA;S:<name>;P:<password>;; text),
  with the password shown under it on request. Hidden networks and
  WPA3/open networks handled; enterprise (802.1X) networks can't be shared.
- Reading the saved password may need a system password prompt: ask once,
  at the start, never in between.
- The other way round: a Wi-Fi QR in a screenshot or region capture
  (decoded locally, zbar) offers "Join <network>".

### iCloud Calendar and Contacts
- iCloud events in Today and the Calendar, iCloud contacts where contacts
  appear (Spotlight, sharing). For iPhone users.
- CalDAV/CardDAV (caldav.icloud.com / contacts.icloud.com), Apple's official
  standard way in. Needs two-factor authentication and an app-specific
  password from account.apple.com: the sign-in sheet walks through making
  one, and the password is kept in the keyring, never the config file.
- Read first; adding/editing events later. Upgraded iCloud Reminders aren't
  reachable over CalDAV: don't promise reminders.
- Recurrence goes through the existing ICS expander (fails closed on rules
  it can't handle, R19).

### Fingerprint unlock (like Touch ID / Windows Hello)
- Unlock the lock screen, approve system password prompts (polkit, Software
  Update) and optionally sudo and the login screen with a finger.
- fprintd + libfprint (works with most built-in readers); enrolment in
  Settings → Lock Screen through fprintd's D-Bus API (pick a finger, touch
  several times, progress shown). Some readers need a vendor driver from the
  AUR (e.g. Dell/Broadcom ControlVault, some Goodix): detect the reader,
  name the package, install it on request.
- The lock screen tries finger and password at the same time (two PAM
  conversations), never "touch first, then you may type". After a restart the
  password is still needed once (it unlocks the keyring), like macOS.
- Shown only when a supported reader is found.

### Face unlock (like Windows Hello)
- Unlock with an infrared camera: Howdy (PAM), the same places as fingerprint.
- Only with an IR camera (some need their IR emitter switched on:
  linux-enable-ir-emitter); never offered for a plain webcam, which a photo
  could fool. Clear note that it's less secure than a password.
- Works alongside fingerprint and password at the same time.

### Cast the screen to a TV (like Windows "Project" / Miracast)
- From Control Center: find TVs and sticks that accept Miracast, then mirror
  the screen or extend it (the TV as an extra display, through a Hyprland
  virtual output, so spaces work there too).
- GNOME Network Displays (Miracast over Wi-Fi Direct through
  NetworkManager; recent versions may also cast to Chromecast: check). Needs
  a Wi-Fi card with Wi-Fi Direct; say so when it's missing. The Quick Share
  note above about NetworkManager and Wi-Fi Direct groups applies: let
  NetworkManager own the group.
- A clear "Casting to <TV>" island activity with Stop.

### Chromecast and Google TV (Google Cast)
- Send what's playing (a YouTube link, a video or music file) to a
  Chromecast or Google TV; control it from the island and Now Playing
  (pause, seek, volume, stop).
- Found over mDNS (_googlecast._tcp); open Cast tools (pychromecast/catt).
  Local files are served from the laptop over the local network only while
  casting. Media casting only; full screen mirroring to a Chromecast isn't
  practical (that's the Miracast item, or GNOME Network Displays if it
  supports Cast).

### AirPlay speakers (Apple)
- HomePods, Apple TVs and AirPlay speakers appear as outputs in the Sound
  menu and Control Center, like any other speaker.
- Built into PipeWire (its RAOP discover module, needs Avahi): a config
  drop-in turned on from Settings → Sound. Check which AirPlay 2-only
  devices accept it, password-protected speakers, and the ~2 s delay (fine
  for music, not video: say so).

### Earbuds pop-up (Google Fast Pair)
- Open the case of Fast Pair earbuds/headphones (Pixel, OnePlus, Sony, JBL,
  Bose and many more) near the computer: a card in the island offers
  Connect, then shows each bud's and the case's battery while connected.
- Google's Fast Pair spec is public: listen for its BLE advertisements
  (service 0xFE2C, model ID) through BlueZ, key-based pairing (ECDH), battery
  from the advertisements' battery data.
- The device's real name and picture come from Google's model lookup, which
  needs an API key: fall back to the Bluetooth name and a generic icon.
  Pairing isn't synced to a Google account (no Google Play services here).
- Respect Bluetooth off, and don't scan constantly on battery (only while
  Bluetooth is on, at a low duty cycle).


## Quick Share: paused (30 Oct)

Works with a real phone (OnePlus 15): found over Bluetooth, no pairing prompt
(BlueZ's experimental RAP and LE Audio control plugins left out), fast when
the phone is on the same Wi-Fi, slow (Bluetooth) otherwise.
- **Off the same Wi-Fi it stays slow.** NetworkManager removes every Wi-Fi
  Direct group it didn't start (nm-supplicant-interface.c: GroupStarted →
  an interface object nobody keeps → RemoveInterface), and Intel Wi-Fi can't
  run a hotspot on 5 GHz. Ideas: let the phone host and join it as a P2P
  client through NetworkManager's own Wi-Fi P2P (a wifi-p2p connection);
  or a hotspot when the laptop's Wi-Fi is on 2.4 GHz.
- Check the second same-Wi-Fi transfer: once the phone's address didn't
  answer although it was on the network (Wi-Fi asleep? ping blocked?).

## Done, tested in the sandbox: needs your check on real hardware

- Windows opened from Settings (file pickers, Google sign-in) appear on top of Settings.
- Links and "Open in the app" bring the app forward (focus follows pointer keeps it).

## Done, tested in the sandbox

- Stale download card in the island; scrolling over the island switches activities; Notification Center header.
- Notification inline replies (sent and received with a test app) and the quick reply shortcut.
- Keyboard and screen-reader basics (focus ring only from the keyboard).
- Battery & Power settings page; System health page.

## Needs your check on real hardware or accounts

- Calls: a real Discord and WhatsApp call (Answer/Decline, Missed call, the Call pill after answering).
- Displays with the BenQ: per-display spaces (its own 1, 2, 3; kept on unplug), the connect animation timing (1.8 s), Overview moves across displays with a real mouse, the switch cue (Off / Glow / Dim).
- BenQ speakers in Control Center → Sound.
- Claude Desktop: Quick Entry stays open and looks right; no wallet prompt (sign in once).
- Webex: Super+Q closes the window and keeps it in the tray (open it, press Super+Q, report).
- Island: flick/×/Esc to dismiss, Recent list, the iRice group in Notification Center.
- Battery & Power: the 20% prompt, sleeping at 3%, lid actions, new idle timers taking effect.
- Google: several accounts, editing events, offline changes (needs a real account).
- Replies with your real messaging apps.

## Waiting on real hardware

- Lid sleep and resume lock, multi-monitor pointer focus and boundaries, the greeter with greetd.
