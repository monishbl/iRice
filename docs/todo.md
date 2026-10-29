# iRice: later

## Squircle corners everywhere

Glass surfaces (dock, panels, Control Center, menus, island, cards, widgets)
use continuous-curvature (squircle) corners. Bring the same shape to every
other rounded shape too: buttons, text fields, list rows, Settings groups,
chips, badges. That needs one shared continuous-corner shape (a small
shader or a Shape path) that replaces plain `Rectangle { radius }` across
the shell.

## Done, tested in the sandbox: needs your check on real hardware

- Windows opened from Settings (file pickers, Google sign-in) appear on top of Settings.
- Links and "Open in the app" bring the app forward (focus follows pointer keeps it).
- Quick Share over Bluetooth works with a real phone (OnePlus 15); the fast Wi-Fi Direct link is fixed but not yet confirmed on the phone.

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
