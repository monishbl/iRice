# What's pending

Updated 20 November 2026.

Where things stand: the Rust shell (`native/tessera-ui`) is the shell. The QML
shell in `shell/` is frozen as the basic fallback (it starts only when the Rust
shell fails twice, with a notice and Try again) and gets compatibility fixes
only. The site (tessera.monishbl.in) serves `/get`. What only the Rust shell
has, and each change's status, is kept in
[native/tessera-ui/RUST-ONLY.md](native/tessera-ui/RUST-ONLY.md); this file
lists only what waits on something else.

## Not yet tried on real hardware

The rows in RUST-ONLY.md whose status says "laptop check pending", "phone
tests pending" or "needs a real session". At the time of writing:

- Popovers closing on a press on another screen (two screens).
- Quick Share with a real phone: the share sheet path, receiving over
  Bluetooth with the faster link, the laptop on 2.4 GHz while receiving.
- The real polkit prompt and the Bluetooth alerts in a standalone session.
- The Tessera boot splashes (Breathe, Orbit, Frost, Bloom) at a real boot.
- The 3-finger long press for circle to search, and "Search the web"
  (Google Lens) through to the browser.

## Offered, waiting for a yes

- **Hide GRUB's menu**: it shows for 5 s every boot; hidden, it would show
  only while you hold Esc or Shift (it's also how you reach snapshots).

## Later

- **Website media**: check the hero film and screenshots for the old product
  names ("App Store", "Spotlight", "Control Center") and re-render any that
  still show them.
- The Material sound pack's source is still unknown (SECURITY-AUDIT.md §6, the licence checklist).
