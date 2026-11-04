# What's pending

Updated 4 November 2026. Main is applied to the laptop.

## Needs you

- **Quiet start and the new login screen**: run `XDG_CONFIG_HOME=~/iRice/.sandbox/home/.config ~/iRice/install.sh --only=login` once (password), then restart. It holds the boot splash until the login screen, keeps boot messages off the screen, and brings the click-to-edit username and the ly-style login row. Not applied yet.
- **Phone**: turn its screen lock back on, and set its display size back if you like (both changed for the Quick Share tests).
- **iPhone**: try the QR code / browser link once with a real iPhone.
- **Licence follow-ups (small)**: check emojibase's exact copyright line against its LICENSE file; record which Unsplash photo each website background is.

## Next, in your order

1. **The Rust rewrite, helpers first** (the QML interface stays): irice-events is done; next Quick Share, the Wi-Fi and link-share helpers, the screenshot text index, backups, then glass-ipc. Each benchmarked against docs/benchmarks-baseline.md and docs/benchmarks-events.md.
2. **Quick Share Bluetooth discovery**: phones take minutes to find the laptop over Bluetooth alone (instant over Wi-Fi); the laptop sends the slower kind of advertisement.
3. **Rust on the laptop**: `rust` is now a build package; the next install or update offers it (irice-events was built in the dev container this time).

## Not yet tried on real hardware

- The iRice boot splashes (Breathe, Orbit, Frost, Bloom) at a real boot; you're on the system's own for now.
- The quiet start's splash → login handover (after the command above).
- irice-events on real hardware: closing the lid with "keep awake when closed" settings, sleep and wake, plugging a USB drive or mouse, a real screen share and camera (the inhibitors are confirmed held by it).
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

- **irice-events** (Rust): one process follows the system for the shell; on the real session 37 processes under the shell became 6 and 200 MB became 49 MB. Without it the shell uses its old watchers.
- **AI sidebar chat history**: past chats with search, rename, pin, delete with Undo; Settings → AI → Chat history.
- **Test sandboxes are hidden and silent** (a private virtual KWin display and a silent private PipeWire).

- Login screen switched to iRice's (greetd); system helper installed (firewall ports only while visible, "Keep iRice smooth when busy", boot animation picker); Glass plugin reloaded at login.
- Material vs Glass sweep: Material controls everywhere, settings lined up, Glass given Material's fixes; then Glass got Quick Settings' footer, the lock picture and name, the App Drawer menu and A–Z (a letter lights its whole set), and Search's Clear and Ask AI.
- Material's lock keeps your Classic layout, with the switch now in Material's Settings.
- Quick Share's upgrade tests no longer depend on this laptop's firewall.
