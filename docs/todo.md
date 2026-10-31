# iRice: later

## Built 31 Oct: check on real hardware

Done and pushed (bdb631a, 0d41611, 6aebe8e), tested in the sandbox; these
parts need the real laptop:
- **Fingerprint**: Settings -> Lock Screen -> Fingerprint -> Install (libfprint-tod
  + libfprint-2-tod1-broadcom + fprintd from the AUR), then enrol a finger.
  Check: the Broadcom ControlVault 3 (0a5c:5843) shows up in fprintd; the lock
  screen takes a finger and a password at once; password prompts and sudo wait
  ~10 s for a finger then ask for the password; the login screen (ly).
- **Dictation**: Settings -> Dictation -> Install (whisper-cpp), Base is
  downloaded by default. Check Super+H tap and hold with the real microphone
  (the hold's release bind was only checked as registered, not pressed: wtype
  can't press compositor binds in the sandbox), and typing into a browser and a
  terminal (wtype; it falls back to the clipboard).
- **Drop shelf**: drag real files from Dolphin/Nautilus (the sandbox had only
  text drags), shake and edge on the real touchpad, and dragging out into a
  browser upload and an email. A release sometimes went missing in the
  sandbox (covered: 4 s still, or the follower's end, counts as up).
- **Wi-Fi QR**: qrencode and zbar come with the next update (packages.sh);
  scan the code with a phone, and a screenshot of a router sticker.

## New features: chosen 31 Oct, not started

Ask about each one's design before building; one at a time.
- **Live captions** (like Windows 11 / Android): subtitles for any audio the
  laptop plays (calls, videos) in a small floating bar. Offline.
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

### Face unlock (like Windows Hello)
- Unlock with an infrared camera: Howdy (PAM), the same places as fingerprint.
- Only with an IR camera (some need their IR emitter switched on:
  linux-enable-ir-emitter); never offered for a plain webcam, which a photo
  could fool. Clear note that it's less secure than a password.
- Works alongside fingerprint and password at the same time (the lock
  screen already runs a second PAM conversation for the finger: Lock.qml).

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


## Quick Share: two speed-ups waiting for a hardware check (branch `quickshare-l2cap-shield`)

Written and unit-tested (332 tests pass), not on `main` until checked:
- **Faster Bluetooth (L2CAP).** Phones open an L2CAP channel first when the
  computer advertises one; iRice now does (no root), instead of GATT writes
  acknowledged one at a time (~6 KB/s). Also fixes iRice's own L2CAP when
  sending (it never worked: "Invalid sdu_length: 0" in the phone's log).
  **Check:** phone on USB adb (USB debugging on, File transfer mode), then
  send a photo over Bluetooth only, both ways: speed vs ~6 KB/s, and the
  file byte-identical. Drivers: tools/quickshare-check on the branch
  (blerx.py / blesend.py / phone.py).
- **Wi-Fi Direct with the internet up.** While the Wi-Fi helper hosts a
  group, a D-Bus rule refuses wpa_supplicant's RemoveInterface (how
  NetworkManager ends groups it didn't start), so NetworkManager keeps the
  normal connection. On this laptop the group goes on 2.4 GHz next to the
  5 GHz connection (the kernel lacks CFG80211_REG_RELAX_NO_IR, so no 5 GHz
  group beside a connection). **Check, from a real terminal (not the
  Claude chat: pkexec needs a tty), e.g. a second SSH session:** run
  `python3 tools/quickshare-check/shield-test.py` (on the branch; asks the
  password once). Expect the group up on every line, wlan0 still on the home Wi-Fi,
  internet up, and the rule off at the end. Then a real receive from a
  phone on another network.
Then merge the branch into `main`.

## Quick Share: tested with a real phone (31 Oct)

Realme GT Neo 3T (Android 14, Play services 26.34) over USB adb, both ways:
- Phone → computer, same Wi-Fi: works (found over mDNS or Bluetooth; a
  3 MB photo moves to the network and arrives in 0.3 s, byte-identical).
- Phone → computer, no shared network: the phone finds it over Bluetooth
  and joins iRice's NetworkManager hotspot within a second; it then needs
  DHCP, which ufw blocked here (the firewall fix now opens 67/udp, and
  Quick Share runs it once the update installs the new root helper).
  **Check after updating:** send a photo from a phone that's on another
  network (or mobile data); it should move to "Wi-Fi hotspot" in the island.
  Without DHCP it fails safely: the Wi-Fi comes back at once and the rest
  goes over Bluetooth (~6 KB/s).
- Computer → phone, same Wi-Fi: works (byte-identical).
- Computer → phone, no shared network: works (NetworkManager joins the
  phone's Wi-Fi Direct group in ~3 s, no password; byte-identical).
- With the phone's Wi-Fi radio off, Android's Quick Share won't send at all
  ("Turn on Wi-Fi"): Google's rule, phone to phone too.
- Still open: Bluetooth-only transfers are ~6 KB/s (acknowledged GATT writes,
  as Nearby's own clients do; a faster LE connection interval needs root).
  Receiving over L2CAP (advertising a PSM) could help.

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
