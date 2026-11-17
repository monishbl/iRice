# Battery saver

The Rust shell's low-battery mode (17 Nov). It makes the shell itself
lighter; it never changes Hyprland's settings, and the system's power
profile only if you ask for it.

## When it is on

Settings → Battery & Power → Battery saver (`power.saver`):

| Setting | When |
|---|---|
| When low (default, `auto`) | on battery at `power.saverLevel` % or less (10–50, default 20); off again when plugged in or 5 % above the level |
| On battery (`battery`) | whenever the computer runs on battery |
| Off (`off`) | only by hand |

Control Center's **Battery saver** tile (add it in the editor) turns it on or
off by hand, on battery only; plugging in ends it and forgets the choice. When
it comes on by itself, the island (or, with the island off, a notification)
says "Battery saver is on · The shell runs lighter until you plug in" with
**Turn off**. Turned off that way it stays off until you plug in or the
battery is back above the band.

**Also use Low Power** (`power.saverProfile`, off by default, shown only with
power-profiles-daemon) asks for the power-saver profile while it is on, at the
same priority as the existing "Low Power when the battery is low".

The decisions are `power_policy::Saver` (host/system, unit tested); the flag
the rest of the shell reads is `tessera_core::saver`.

## What it changes

- **60 Hz at most.** No surface presents more than 60 frames a second, even on
  a 120 Hz display; frame timers and unpaced animations tick at 60 Hz
  (`wayland.rs` `held_until`, `pacing::frame_period`). Frames held back that way
  don't count as missed (no "use the other GPU?" question).
- **Reduce motion.** The settings every part reads are shown with
  `appearance.reduceMotion` on (the file is not changed; Settings shows your own
  value), so every Reduce motion path runs: the lock screen's fingerprint and
  loops hold still, wavy progress bars are flat (lock, island, Control Center,
  Wi-Fi), no press bounce, shorter slides. Theme's new `still` token carries it
  to Slint.
- **Cheaper glass.** Cool-down's frosted glass (no lens refraction, the
  costliest part) unless Cool-down already chose solid.
- **Fewer wakeups.** With nothing moving, Slint's timers wake the loop on a
  500 ms grid instead of 250 ms.
- **Slower background refreshes** (×4): weather every 2 hours, Google calendars and
  tasks and iCloud/Microsoft accounts every hour; resource graphs sample every 2 s
  while shown (20 s for the desktop widget and island). Syncs on unlock or when
  Notification Center opens are unchanged.

## Trying it in a sandbox

`TESSERA_FAKE_BATTERY=<file>` (with `TESSERA_SANDBOX=1`, which the sandbox
sets) replaces UPower with a file read every second: `15 discharging`,
`80 charging`, `100 full`. Export it before `./dev.sh up`, then write the file
to plug in, unplug or drain the battery.
