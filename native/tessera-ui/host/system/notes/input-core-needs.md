# CORE-NEEDS for the input group

Nothing outside the crate is required to compile. For it to work standalone, other owners need to
honour the contracts in `input.md`:

- host-shell (settings RustBridge): send `input.live`/`devices.live` [bool] when Settings opens/closes,
  and `devices.page [page]` on every page change; forward `settings.act` groups "input" and "devices"
  as `input.act`/`devices.act` [what, …args]. Forward Panels' `open`/`locked` through `set_state`
  ("open", "locked"), which Mouse reads. Answer `island.post`, `sound.play`, `keybinds.refresh`.
- host-desktop: `spaces.arrive [screen]`, `dictation.*`, `screentools.installOcr`; bus keys
  `screentools`, `dictation`, `files`. Whoever draws DisplayConnect reads bus `displayfx` and calls
  `displays.finish [id]`.
- net group: bus `cooldown` is host-shell's (CoolDown); net publishes `feed.wifi`, `feed.backends`,
  `feed.speed`, `feed.speedTest`, `feed.ddc`, bus `wifijoin`/`needs`/`health`/`bluetooth`, and answers
  the `network.*`, `wifijoin.*`, `wifishare.*`, `speedtest.*`, `brightness.setDdc|rescanDdc`,
  `audioprofiles.refresh`, `backends.*`, `needs.*` asks.
- host-sharing: `feed.airplay`, `feed.mirroring`, `feed.phone`, `feed.sharing`, bus `quickshare`/`fastpair`,
  and `airplay.*`, `mirroring.*`, `phone.*`, `quickshare.turnOnBluetooth`, `linkshare.receive`,
  `sharing.shareSelection`.
