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

- Battery & Power: the 20% prompt, sleeping at 3%, lid actions, new idle timers taking effect.
- Google: several accounts, editing events, offline changes (needs a real account).
- Replies with your real messaging apps.

## Waiting on real hardware

- Lid sleep and resume lock, multi-monitor pointer focus and boundaries, the greeter with greetd.
