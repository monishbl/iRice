# iRice: later

## Built 31 Oct: check on real hardware

Done and pushed (bdb631a, 0d41611, 6aebe8e), tested in the sandbox; these
parts need the real laptop:
- **Fingerprint**: Settings -> Lock Screen -> Fingerprint -> Install (libfprint-tod
  + libfprint-2-tod1-broadcom + fprintd from the AUR), then enrol a finger.
  Check: the Broadcom ControlVault 3 (0a5c:5843) shows up in fprintd; the lock
  screen takes a finger and a password at once; password prompts and sudo wait
  ~10 s for a finger then ask for the password; the login screen (ly).
- **Dictation**: Settings -> Dictation -> Install (whisper-cpp), then Download
  Base (142 MB). Check Super+H tap and hold with the real microphone
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

## Integrations: built 31 Oct, check with your devices

All built and pushed (988e885 AirPlay, 28b8f9e Chromecast, 52856fc iCloud /
Microsoft 365 / Drive folders, 673ecd9 Android phone, 33433e8 mirroring,
793a61e Fast Pair, d320bae face unlock), tested against stand-ins (a real
PipeWire AirPlay module and an mDNS speaker, a stand-in Chromecast, a real
CalDAV server (Radicale), a stand-in Microsoft Graph and rclone, a stand-in
adb, GNOME Network Displays' own dummy TVs, a real nearby Fast Pair advert).
Still to check with the real things:
- **AirPlay**: Settings -> Sound -> Install, turn on; a HomePod/Apple TV
  appears as an output and plays (about 2 s late).
- **Chromecast**: cast a local video and a YouTube link (needs yt-dlp) to a
  real Chromecast; Allow through firewall first (port 53320 is new).
- **iCloud**: sign in with an app-specific password; add/edit/delete an
  event and see it on the iPhone; contacts in Spotlight.
- **Microsoft 365**: Sign in with Microsoft (goa-signin ms_graph, rebuilt by
  the next update's native helpers); Outlook events and To Do tasks appear.
- **Drive / OneDrive folders**: Install rclone, Connect: the browser sign-in,
  then ~/Google Drive opens in Dolphin.
- **Android phone**: Settings -> Phone -> Install; USB debugging on; Show
  Screen; pair by QR; webcam (Set Up asks for the password: v4l2loopback).
- **Mirroring**: Screen Mirroring tile with a Miracast TV (this laptop's Wi-Fi reports Wi-Fi Direct (iw), so
  P2P is supported) and a Chromecast.
- **Fast Pair**: the WH-CH720N or realme Buds T100 in pairing mode -> the
  island's Connect; open the case while connected -> batteries.
- **Face unlock**: Settings -> Lock Screen -> Face -> Install (AUR,
  howdy-next, compiles), Set Up, Add Face, lock and look. Check that
  pam_howdy works for the shell (a non-root lock screen), as it does for
  hyprlock.

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
