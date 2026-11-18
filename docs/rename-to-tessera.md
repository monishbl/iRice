# iRice is now Tessera

Renamed 4 November 2026; its mark is the **Badge** (Phosphor's
`circle-wavy`). The repository (`monishbl/iRice` → `monishbl/tessera`) and
the website (`irice.monishbl.in` → `tessera.monishbl.in`) followed on 17 Nov,
and on 18 Nov everything else: the checkout folder on installed machines,
the bug report service, the development sandbox and every file in the
repository. This page is the one place that lists the old names: what they
were, where the few still needed are kept, and when those go.

## What moved

| Before | Now | When |
| --- | --- | --- |
| the `irice` command | `tessera` | 4 Nov |
| `~/.config/irice`, `~/.local/share/irice`, `~/.local/state/irice`, `~/.cache/irice` | `…/tessera` | 4 Nov (moved at the shell's start) |
| `/usr/lib/irice/irice-root`, polkit `org.irice.root.*`, `49-irice.rules`, `49-irice-quickshare.rules` | `/usr/lib/tessera/tessera-root`, `org.tessera.root.*`, `49-tessera.rules`, `49-tessera-quickshare.rules` | 4 Nov (moved by the root setup) |
| `/etc/irice`, `/var/lib/irice`, `/etc/ananicy.d/irice.rules`, drop-ins `irice-*.conf`, Plymouth `irice-<id>` | `tessera` | 4 Nov (root setup) |
| `/usr/share/irice` (the login screen), `irice.desktop` | `/usr/share/tessera`, `tessera.desktop` | 4 Nov (`./install.sh --only=login`); 18 Nov: the root setup renames the session entry too |
| the shell's process `irice-shell`, the glass plugin `irice-glass`, `irice-events`, `IRice.Native` | `tessera-shell`, `tessera-glass`, `tessera-events`, `Tessera.Native` | 4 Nov; 18 Nov: the old ones are no longer recognised |
| `IRICE_*` environment names | `TESSERA_*` | 4 Nov; the old ones count until 1 Feb 2027 |
| github.com/monishbl/iRice, irice.monishbl.in | github.com/monishbl/tessera, tessera.monishbl.in | 17 Nov |
| the checkout `~/iRice` | `~/tessera` (`~/iRice` a link to it for now) | 18 Nov (at the shell's next start after the update) |
| the bug report service `irice-bugs.monishbl.in` (Worker `irice-bugs`, D1 `irice`) | `tessera-bugs.monishbl.in` (Worker `tessera-bugs`, D1 `tessera-bugs`) | 18 Nov (site/bugs/README.md, "Moving from the old name") |
| the dev sandbox: distrobox `irice`, `/tmp/irice-rt-N`, the `IRICE-TEST` output | `tessera`, `/tmp/tessera-rt-N`, `TESSERA-TEST` | 18 Nov |

## What an installed machine goes through

Nothing to do by hand except the root step, which Software Update offers.

1. **The update itself** (Software Update, or `tessera update`) takes the new
   version. An update run by a copy from before 18 Nov is that old copy's
   code; the new code runs from the shell's restart that ends it.
2. **The shell's start** (`launch-shell.sh`, also the console session's
   `sandbox/tty-shell.sh`) runs `shell/assets/scripts/migrate-names.py`
   before anything reads a setting, only while something is left to move
   (`old_names_pending`: a cheap check, so starts stay fast). Once:
   - `…/irice` folders still there (your session's XDG folders and your
     home's own, when a console session points XDG elsewhere) move to
     `…/tessera`; both there: what's only in the old one moves over, the rest
     is kept as `irice.before-tessera`. Links inside that pointed into the
     old folder point into the new one. A note, `RENAMED.txt`, says what moved
     (the 4 Nov note, `MOVED-FROM-IRICE.txt`, is folded into it).
   - **The checkout `~/iRice` becomes `~/tessera`**, with `~/iRice` left as a
     link to it (running programs, the login screen's entry and Hyprland's
     config path keep working through it). Paths to it saved in your settings
     (wallpapers), the console session's home (`.sandbox/home/.config`), the
     installer's `…/tessera/source`, your user units and desktop entries point
     at `~/tessera`; worktrees are repaired (`git worktree repair`). A
     remote at `github.com/monishbl/iRice` (or GitLab's) points at
     `monishbl/tessera`. Software Update also does all this at the end of an
     update run by the new copy.
   - Values saved under the old name take the new one: the menu icon and the
     screenshot editor (`"irice"`), shortcuts that run `$IRICE_SHELL/…`,
     `state.json`'s `_irice_revisions`, `spaces.lua`'s `IRICE_SPACE_BASES`,
     autostart entries for the `irice` desktop, KDE's colour scheme `iRice…`.
   - Files the installer and the shell made under the old name are renamed or
     replaced: `hypr/irice`, `hypr.pre-irice` backups, fish snippets, the
     apps' `irice-colors` includes and files, file-manager menus
     (`irice-share`), the battery guard's `irice-battery-guard.service`, the
     `irice` command; builds nothing runs any more go (`irice-glass.so`; the
     events helper takes Tessera's name when there's no newer one).
   - An old-name link (`…/irice`, `~/iRice`, `hypr/irice`) goes once it's at
     least 14 days old and nothing uses it: no process of yours has it as its
     folder or program, on its command line or in its environment, and no
     session entry, Hyprland file or user unit names it. So `~/iRice` stays
     while the session started from it runs and while the login screen's
     entry still names it.
3. **The root step** (Software Update offers "Tessera's system helper" while
   the installed one isn't this version's; or by hand,
   `sudo /usr/bin/python3 -I ~/tessera/shell/assets/scripts/tessera-root.py setup ~/tessera/shell`)
   installs Tessera's helper first, then moves what the old one installed:
   `/etc/irice`, `/var/lib/irice`, drop-ins, module options, PAM marks, Quick
   Share's trust rule, the boot animation in use, root's `.pre-irice` backups;
   **the login screen's entry**: `irice.desktop` (starting
   `sandbox/irice-session.sh`) becomes `tessera.desktop`, and an entry that
   starts the checkout through `~/iRice` starts `~/tessera` (only an entry
   that already starts that same checkout's session script); then the old
   helper, its polkit files and ananicy rule and the old boot animations go.
   Until it has run, the shell keeps using the old helper and those files
   (no new password prompts).
4. **The login screen's copy** (`/usr/share/irice`, greetd only) moves with
   `./install.sh --only=login`; until then it keeps working.
5. **Log out and in**: the session starts from `~/tessera`, and two weeks
   later the `~/iRice` link goes by itself.

A machine still on a version from before 4 Nov goes through the same steps;
processes of its running session from before then (a watchdog, an update
screen) aren't recognised by the new names, so log out and in after that
update.

## The old names still in the code, and why

Only where an installed machine needs them to update itself across the
rename, each set in one place per language, all to be removed after
**1 February 2027**:

| Where | What it keeps, and why |
| --- | --- |
| `shell/assets/scripts/migrate-names.py` (Python) | the moves above; the old root helper while only it is installed (sharing check, Quick Share's Wi-Fi helper, fingerprint), its folder `/etc/irice` and D-Bus rule, the PAM mark, nftables drop-ins; keyring entries saved as `application irice` / `irice-icloud`; settings backups `*.irice` (`"app": "iRice"`); the updater's `hypr/irice/keymap.json` for a version from before |
| `tessera-root.py`'s section "Tessera's old name" (the root helper) | what setup moves (above) and what other subcommands recognise until then (the old ananicy rule, PAM marks, firewall tags `irice sharing`, the old boot animations). The helper is one file installed as root's own, so it never loads the user side's module |
| `shell/assets/scripts/old-names.sh` (bash) | `IRICE_*` environment names (`old_names_env`, at the shell's, the session's and `dev.sh`'s start), the pending check, the old system paths for the installer and uninstaller, the old session entry, the dev container `irice` until it's renamed |
| `native/tessera-ui/core/src/old_names.rs` (Rust) | `IRICE_*` names (`env::var`), the old root helper and its polkit file, `/etc/irice`, the ananicy rule, Plymouth `irice-<id>`, `/usr/share/irice`, keyring `application irice`, `*.irice` backups, exported `irice-keybinds`, the old menu icon value |
| `shell/common/OldNames.qml` (QML) | the same, for the QML shell |
| `sandbox/irice-session.sh` | the file `irice.desktop` starts, until the root setup replaces that entry |
| `site/wrangler.jsonc`, `site/bugs/wrangler.jsonc` | the old addresses `irice.monishbl.in` and `irice-bugs.monishbl.in`, which installs and issues from before still use |
| `CHANGELOG.md` | two headings from before (Software Update matches its "What's new" by heading: a renamed one would show again) |
| this page, the migration tests (`tests/test_rename_tessera.py`) | the record |

The modules' own tests fail after 1 February 2027 (`old_names.rs`,
`tests/test_rename_tessera.py`), as a reminder: then delete those files and
the lines that use them, and the old routes in the two `wrangler.jsonc`.

## For developers

- The sandbox: `./dev.sh` uses the distrobox `tessera`; until it exists, the
  old `irice` one. Rename it once: `podman rename irice tessera` (or
  `docker rename`), with it stopped (`distrobox stop irice`). Arenas run in
  `/tmp/tessera-rt-N` on the headless output `TESSERA-TEST`; a checkout from
  before 18 Nov uses `/tmp/irice-rt-N`, so don't run the same arena from both.
- `IRICE_ARENA` and the rest still work (`old_names_env`) until 1 Feb 2027.
- Tools that read the live install's settings (`tools/parity-run.sh`,
  `tools/visual-compare.sh`…) read them from `~/tessera`.
