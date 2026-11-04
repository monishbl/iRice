# What's pending

Updated 4 November 2026. Main is applied to the laptop.

## Needs you

- **Quiet start and the new login screen**: run `XDG_CONFIG_HOME=~/iRice/.sandbox/home/.config ~/iRice/install.sh --only=login` once (password), then restart. It holds the boot splash until the login screen, keeps boot messages off the screen, and brings the click-to-edit username and the ly-style login row. Not applied yet.
- **Phone**: turn its screen lock back on, and set its display size back if you like (both changed for the Quick Share tests).
- **iPhone**: try the QR code / browser link once with a real iPhone.
- **Licence follow-ups (small)**: check emojibase's exact copyright line against its LICENSE file; record which Unsplash photo each website background is.

## In progress

- **AI sidebar chat history**: every chat kept as its own session; a history view to search, open, rename, pin and delete them (delete with Undo); Settings → AI: keep history, delete old chats, delete all.

## Next, in your order

1. **Fewer background processes**: the ~33 small watcher processes become one supervisor. Then **the Rust rewrite**, benchmarked against docs/benchmarks-baseline.md, when you say go.
2. **Quick Share Bluetooth discovery**: phones take minutes to find the laptop over Bluetooth alone (instant over Wi-Fi); the laptop sends the slower kind of advertisement.

## Not yet tried on real hardware

- The iRice boot splashes (Breathe, Orbit, Frost, Bloom) at a real boot; you're on the system's own for now.
- The quiet start's splash → login handover (after the command above).
- The 3-finger long press for circle to search, and "Search the web" (Google Lens) through to the browser.
- Quick Share: the laptop on 2.4 GHz while receiving (the 5 GHz group beside it is unit-tested); 2 GB of small files phone→laptop.

## Offered, waiting for a yes

- **Hide GRUB's menu**: it shows for 5 s every boot; hidden, it would show only while you hold Esc or Shift (it's also how you reach snapshots).

## Watch

- **Glass shell froze in the sandbox** in 2 smoke runs on 3 Nov, not since (many runs). The smoke check fails on it and says whether the compositor or the shell was stuck. If your desktop freezes, note the time.

## Later

- **Website media shows old names**: re-render the hero film (`site/film/hero/film.html`, the Bedside Mode caption) and the screenshots showing "App Store", "Spotlight Search" or "Control Center". `site/public/get` isn't deployed (pushing site/ deploys: ask first).
- `dev.sh` still lists papirus.

## Done recently (4 Nov)

- Login screen switched to iRice's (greetd); system helper installed (firewall ports only while visible, "Keep iRice smooth when busy", boot animation picker); Glass plugin reloaded at login.
- Material vs Glass sweep: Material controls everywhere, settings lined up, Glass given Material's fixes; then Glass got Quick Settings' footer, the lock picture and name, the App Drawer menu and A–Z (a letter lights its whole set), and Search's Clear and Ask AI.
- Material's lock keeps your Classic layout, with the switch now in Material's Settings.
- Quick Share's upgrade tests no longer depend on this laptop's firewall.
