# iRice: later

## Squircle corners everywhere

Glass surfaces (dock, panels, Control Center, menus, island, cards, widgets)
use continuous-curvature (squircle) corners. Bring the same shape to every
other rounded shape too: buttons, text fields, list rows, Settings groups,
chips, badges. That needs one shared continuous-corner shape (a small
shader or a Shape path) that replaces plain `Rectangle { radius }` across
the shell.

## Open bugs

- Windows opened from Settings (wallpaper picker, Connect Google) appear behind Settings.
- "Open in the app" links from a browser don't bring the target app forward.
- A finished download can leave a stale "Unconfirmed … · Downloading 0 B" card in the island.
- Scrolling over the island doesn't switch between its activities.
- Notification cards scroll behind the Notification Center header and spoil its backdrop.

## Next features

- Power & Sleep settings: idle timers on AC and battery, lid actions, lock timing, and which app is keeping the computer awake.
- One power policy: manual profile, thermal response, Caffeine, Game Mode and battery resolved in one place, each with an owner and an expiry.
- Critical battery: decide who suspends or hibernates, and handle Caffeine and a crashed shell.
- Per-app energy: which app is keeping the CPU busy or waking the machine.
- Notification inline replies (the server no longer advertises them until there's a composer).
- Keyboard and screen-reader basics for IconButton and Dropdown, as Switch has.
- A health page: which daemons and protocols are actually reachable, not just installed.
- Google: several accounts, event editing, offline changes; real-account testing of sign-in and sync.
- Real-hardware checks: lid sleep and resume lock, multi-monitor pointer focus and boundaries, the greeter with greetd.
