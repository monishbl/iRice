# iRice is now Tessera

Renamed 4 November 2026. Everything that said iRice says Tessera, and its
mark is the **Badge** (Phosphor's `circle-wavy`), the wavy seal that was
the last menu-icon choice in Settings → Appearance. It's the default menu
icon now, and it's in the boot animations, the look switch and the login
intro.

A desktop set up while it was called iRice keeps working on the old system
pieces until you run the two setup steps below, and on the new ones after.

## What moved

| Before | Now |
| --- | --- |
| the `irice` command | `tessera` (the installer removes `irice`) |
| `~/.config/irice`, `~/.local/share/irice`, `~/.local/state/irice`, `~/.cache/irice` | `…/tessera` |
| `/usr/lib/irice/irice-root`, polkit `org.irice.root.*`, `49-irice.rules` | `/usr/lib/tessera/tessera-root`, `org.tessera.root.*`, `49-tessera.rules` |
| `/etc/irice`, `/var/lib/irice` | `/etc/tessera`, `/var/lib/tessera` |
| `/etc/ananicy.d/irice.rules` | `/etc/ananicy.d/tessera.rules` (matches `tessera-shell` and `irice-shell`) |
| the shell's process name `irice-shell` | `tessera-shell` |
| `/usr/share/irice` (the login screen), `irice.desktop` | `/usr/share/tessera`, `tessera.desktop` |
| Plymouth themes `irice-<id>` | `tessera-<id>` |
| the glass plugin `irice-glass.so` (`hyprctl irice-glass`) | `tessera-glass.so` (`hyprctl tessera-glass`; it still answers to `irice-glass`) |
| `irice-events`, `IRice.Native` | `tessera-events`, `Tessera.Native` |
| `IRICE_*` environment names | `TESSERA_*` |
| `hypr/irice`, layer namespaces `irice:*`, polkit, drop-ins, file names | `tessera` |

Not renamed: the GitHub repository (`monishbl/iRice`), the website and its
domains (`site/`, `irice.monishbl.in`), your checkout folder (`~/iRice`),
and the dev sandbox's machine-wide names (the `irice` distrobox,
`/tmp/irice-rt*`, the `IRICE-TEST` output), which every checkout from
before and after the rename shares.

## What happens by itself

- **Your settings and data move at the shell's next start**, once
  (`shell/assets/scripts/migrate-names.py`, run by `launch-shell.sh`; the
  installer and the build scripts run it too). Each `…/irice` folder is
  renamed to `…/tessera`, and the old name becomes a link to the new one,
  so anything still using it finds the same files. If both exist, what's
  only in the old one moves over and the rest is kept as
  `irice.before-tessera`. A note, `MOVED-FROM-IRICE.txt`, says what moved.
  A shell that reloaded in place before restarting keeps reading the old
  folders until it restarts.
- **The root helper:** until the root setup has run, the shell uses the
  old `/usr/lib/irice/irice-root` with its own polkit actions, so nothing
  asks for a password that didn't before. Once Tessera's helper is
  installed it uses that one.
- **The shell's name:** it keeps running as `irice-shell` while only the
  old ananicy rule is installed (or a watchdog or update screen from before
  the rename watches it), so its priority stays the same; it's
  `tessera-shell` once the root setup has installed the new rule, which
  matches both names.
- **The glass plugin:** Hyprland loads whichever build is there: the repo's
  `tessera-glass.so`, else the one built before the rename. The running
  session keeps the one it loaded until you log out. Tessera's plugin also
  answers to `irice-glass`, and the shell finds either.
- **Old environment names** (`IRICE_ROOT`, `IRICE_TTY`, `IRICE_SANDBOX`,
  `IRICE_USER_DIR`, `IRICE_ARENA`…) still work everywhere the new ones do;
  the new name wins when both are set.
- **The session's lock, watchdog and idle timers** from before the rename
  are recognised and never doubled.
- **The login screen's entry** (`irice.desktop`) still works: it starts
  `sandbox/irice-session.sh`, which starts `tessera-session.sh`.
- Kept values: API keys in the keyring (`application irice`), iCloud
  passwords, settings backups (`.irice`), exported shortcuts, Quick Share
  trust and the boot animation in use are all read under either name.

## The one-time setup, with your password

From `~/iRice`, after the update is in (Software Update, or `git pull`):

1. **The root setup**, which installs Tessera's helper, polkit action and
   rule and ananicy rule, moves `/etc/irice` to `/etc/tessera`, renames the
   drop-ins, fingerprint marks, Quick Share's trust rule and the boot
   animation in use, then removes the old helper, its polkit files, its
   ananicy rule and the old boot animations:

   ```
   sudo /usr/bin/python3 -I ~/iRice/shell/assets/scripts/tessera-root.py setup ~/iRice/shell
   ```

   (Software Update does the same in its root step.)

2. **The login screen**, which installs `/usr/share/tessera`, points greetd
   at it, removes `/usr/share/irice`, replaces `irice.desktop` with
   `tessera.desktop` and the Plymouth drop-in with Tessera's:

   ```
   ~/iRice/install.sh --only=login
   ```

3. **Log out and in.** The new session starts as Tessera: the shell named
   `tessera-shell`, the new plugin if you rebuilt it
   (`~/iRice/tools/build-plugin.sh`), and Hyprland's own config read
   afresh.

Optional, when convenient: `~/iRice/tools/build-native.sh` and
`~/iRice/shell/native/build.sh` rebuild `tessera-events` and
`Tessera.Native` under their new names (the old builds keep working until
then).

## If you never run them

Everything keeps working on the old system pieces: the shell uses
`/usr/lib/irice/irice-root` and its polkit rule (no new password prompts),
keeps the name `irice-shell` that `/etc/ananicy.d/irice.rules` matches,
greetd keeps starting the login screen in `/usr/share/irice` (its look and
wallpaper are still written there), and `irice.desktop` keeps starting the
session. Only the names in those system places stay old.

## The checkout folder

`~/iRice` keeps its name. To rename it later, move it and point the login
screen's entry at the new place; nothing else stores the folder's path:

```
mv ~/iRice ~/Tessera && ~/Tessera/tools/setup-session.sh
```

(`setup-session.sh` rewrites `/usr/share/wayland-sessions/tessera.desktop`,
whose `Exec` is the session script's full path.)

## For developers

`./dev.sh` reads `TESSERA_ARENA`, `TESSERA_VISIBLE` and the rest, and
still `IRICE_ARENA` and friends. The arenas keep `/tmp/irice-rt-N` and the
`IRICE-TEST` output, so a checkout from before the rename and one from
after never start two sandboxes for the same arena. `tools/smoke-looks.sh`
works with the main checkout's `dev.sh` from either side of the rename.
