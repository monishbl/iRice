# iRice: later

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
