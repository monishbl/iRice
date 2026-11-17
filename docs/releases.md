# Ready-made tessera-ui: releases, signing, channels

Tessera's Rust programs, **tessera-ui** (the interface in Rust) and
**tessera-events** (the system watcher), don't have to be compiled on your
computer. The release builder (below) builds them on server1 for every new
`native/` on `main`, `beta` and every `v*` tag; the user's laptop signs them
and publishes them as GitHub releases; and the installer and Software Update
download the build for the exact version they install. You can still build
them yourself.

## How it works

One entry point does it all: `tools/native-install.py`. `tools/build-native.sh`
(the installer, `tessera rebuild`) and Software Update (`tools/tessera-update.py`)
both go through it.

1. **Which build.** Builds are named by the git tree of `native/` at that
   commit (`git rev-parse HEAD:native`), not by the commit itself. That is
   exactly the Rust sources, so a commit that changes only QML uses the build
   made for the commit before it. The signed manifest also records the commit
   it was built from; when your checkout knows that commit, its `native/` tree
   must be the same.
2. **Which processor.** `/proc/cpuinfo` decides: **x86-64-v3** when every core
   has avx, avx2, bmi1, bmi2, fma, movbe, f16c, lzcnt (shown as `abm`) and
   xsave (Intel Haswell and AMD Excavator onwards), otherwise **generic**
   (any x86-64). Other architectures have no ready-made build.
3. **Where.** A `v*` tag pointing at the commit has its own release; then
   the rolling prereleases, your channel's first: `tessera-ui-latest` (main,
   Stable) and `tessera-ui-beta` (beta, Beta). A build is found by its tree in
   either, so a version promoted from beta to main uses the build beta already
   has. Downloads use curl (https only, timeouts, three retries, a size limit).
4. **Checks**, all of them before anything is installed:
   - the manifest's minisign signature, against `tools/tessera-ui.pub` in
     your checkout (with python-cryptography, which Tessera already installs,
     or the `minisign` program);
   - the signed comment names this tree, and so does the manifest;
   - the archive's size and sha256 match the manifest, and so does each
     program's sha256;
   - each program answers `--version` with the manifest's version. It runs
     without any display (`WAYLAND_DISPLAY`, `WAYLAND_SOCKET` and `DISPLAY`
     unset); nothing else is ever run.
5. **Swap.** Each program is written next to the old one and renamed over it.
   The old one becomes `tessera-ui.prev` (and `tessera-events.prev`), and
   `tessera-ui.json` says where each came from (ready-made or built here, the
   variant, tree and commit). A running tessera-ui keeps its own copy until the
   shell restarts it. One install runs at a time (a lock in the folder).
6. **On any failure** (no build for this version yet, no network, a bad
   signature or checksum) the installed programs stay exactly as they were,
   and it says why. In `auto` it then builds here instead. If that fails too,
   the old build stays, and with no tessera-ui at all the shell draws
   everything in QML.

Everything lives in `$XDG_DATA_HOME/tessera/bin` (usually
`~/.local/share/tessera/bin`).

### Modes

`rust.uiSource` in `~/.config/tessera/config.json`:

| Mode | What it does |
|---|---|
| `auto` (default) | The signed ready-made build for this version and processor. Built here when there isn't one: a checkout with your own edits in `native/`, a version not built yet, no network, or no release key yet. |
| `build` | Always built here (`cargo build --release`), for any x86-64. |
| `buildTuned` | Built here for this processor only (`RUSTFLAGS=-C target-cpu=native`, in its own target folder). |

The installer asks nothing about this by default. Advanced is collapsed:
press **a** on the review page of the interactive installer (anything that
can't work here, such as no cargo or less than about 4 GB of free memory or
disk, is greyed out with the reason). With the classic prompts, or
unattended, pass `./install.sh --ui-source=build` or `--ui-source=buildTuned`.
The plan shows which one applies.

By hand:

```sh
tools/build-native.sh              # as rust.uiSource says
tools/build-native.sh --build      # build here, generic
tools/build-native.sh --tuned      # build here, for this processor
tools/native-install.py status     # what's installed, and from where
tools/native-install.py plan --online   # what an install would do now
tools/native-install.py restore    # swap the .prev build back (and again to undo that)
```

### Updates and going back

Software Update's check asks the helper what the new version would get, and
the "Updates are ready" line says so, for example
`Tessera 1a2b3c4d → 5e6f7a8b · tessera-ui: ready-made (v3)`, or
`tessera-ui: built here, tuned`. When updating, the new build is installed
right after the new code is in, inside the part of the update that rolls back
on failure. If the update rolls back, the code goes back and
`native-install.py install --only-prev` swaps the previous build back from
`.prev`, with no download or build. Any time the checkout returns to the
version of the `.prev` build, an install swaps it back instead of downloading.

## Which build is this?

tessera-ui knows exactly what it is, and says it the same way everywhere:

```sh
$ tessera-ui --version
tessera-ui 0.1.0 (beta, 7baa48d, 2026-11-17, x86-64-v3, signed release)
$ tessera-ui --version --json
{"name":"tessera-ui","version":"0.1.0","commit":"7baa48d…","short":"7baa48d","date":"2026-11-17","dirty":false,
 "branch":"","channel":"beta","variant":"x86-64-v3","profile":"release","kind":"release","line":"tessera-ui 0.1.0 (…)"}
```

The parts: the channel it was built for (`stable`, `beta`, `release` for a
`v*` tag, or `local build`), the commit it was built from (`-dirty` when
`native/tessera-ui` had uncommitted edits), that commit's date, the variant
(`x86-64-v3`, `generic`, or `native`: tuned for the computer that built it),
the profile when it isn't `release` (`fast profile`, `dev profile`), and
`signed release` (the release builder's, published signed) or `built
locally`. A local developer build looks like `tessera-ui 0.1.0 (local build,
7baa48d-dirty, 2026-11-17, generic, fast profile, built locally)`.

Where it shows:

- **Settings → About → Version**: two lines (`Beta · 7baa48d · 17 November
  2026`, `x86-64-v3 · signed release`) and **Copy** (the `--version` line,
  for a bug report).
- **Settings → Software Update → Channel → This build**: the same two lines,
  then, once checked, what the channel has (`Available on Beta: version
  5e6f7a8b, ready-made (v3)` or `The latest on Beta.`).
- Both say plainly when the build isn't from the channel Software Update
  follows (`update.channel`, else the branch the checkout follows): "This
  build is from Beta, but Software Update follows Stable. Check for updates
  to get Stable's version."; a local build off another branch says so; a
  build promoted from Beta to Stable unchanged says "Built on Beta and
  published to Stable unchanged." (server1 packs beta's build again for main,
  so the binary still says beta). When a newer build was installed but the
  shell hasn't restarted yet, they say that too.
- `tools/native-install.py status`: what's installed, and the installed
  tessera-ui's `--version` line (kept in `tessera-ui.json` as `build`).

How it's decided (`native/tessera-ui/build.rs`, the rules in
`core/src/build_resolve.rs`): these variables first, else git, else unknown.

| Variable | Set by | Values |
|---|---|---|
| `TESSERA_BUILD_CHANNEL` | the release builder (the target's channel); native-install's local builds (only when the checkout follows `origin/main` or `origin/beta`) | `stable`, `beta`, `release`; unset: the branch (main: stable, beta: beta, else `local build`) |
| `TESSERA_BUILD_VARIANT` | the release builder; native-install (`generic`, or `native` for buildTuned) | unset: RUSTFLAGS' `-C target-cpu`, else `generic` |
| `TESSERA_BUILD_KIND` | the release builder (`release`); native-install (`local`) | anything but `release` is a local build |
| `TESSERA_BUILD_COMMIT` | the release builder | a full commit; unset: `git rev-parse HEAD` |
| `TESSERA_BUILD_DATE` | the release builder | `YYYY-MM-DD`; unset: the commit's date (`git log -1 --format=%cs`) |

It doesn't slow builds: the build script reruns only when one of these
variables changes, when HEAD moves (HEAD, the branch's ref, `packed-refs`), or
when a source in `native/tessera-ui` changes (then the binary recompiles
anyway); never on every build. Without git (a tarball) it watches only the
variables, and an unknown commit reads `unknown`. The release builder's
`--version` line is what the manifest records per variant
(`programs.tessera-ui.version`), and installs check the download says
exactly that.

## Update channels

Settings → Software Update → **Channel** says which Tessera to follow
(`update.channel` in `config.json`):

| Channel | Branch | Changes when |
|---|---|---|
| **Stable** (the default) | `main` | you promote a version (`git push origin beta:main`, or a merge into main). There's no schedule. |
| **Beta** | `beta` | every push to `beta`. |

Every branch carries both shells (QML, and Rust with the QML fallback), so the
channel only decides which branch the checkout follows. Until a channel is
chosen (`update.channel` empty), the branch the checkout already follows
decides: `main` is Stable, `beta` is Beta, and a developer's checkout on any
other branch keeps following that branch ("your branch"; choosing a channel
moves it to that channel's branch).

**Both channels always ask.** Nothing is installed on its own:

- *The notice.* While **Check automatically** is on (`update.autoCheck`, on by
  default), the shell runs `tools/tessera-update.py peek --mark` a few minutes
  after login and then every six hours. It fetches only the channel's branch
  and says whether to mention it: once per new version, and again a day later
  while it isn't installed (`$XDG_STATE_HOME/tessera/update-notice.json`). The
  notice reads "Update available" with **Update** and **Later**. Update opens
  Software Update and runs a full check; Later leaves it. Going back to an
  older version (below) is never a notice, only a choice in Settings.
- *The check and Update* are Software Update's, as before: what's new, every
  conflict to decide, then one Update for Tessera and the system.

**How the check and Update work per channel** (`tools/tessera-update.py`):

1. `check` fetches the channel's branch only
   (`git fetch origin +refs/heads/<branch>:refs/remotes/origin/<branch>`, so
   a copy cloned with one branch gets the other when it's chosen) and compares
   it with the checkout: *none* (up to date), *update* (the channel's version
   has everything this copy has, and more), or *switch* (it lacks some of this
   copy's commits). A commit no branch on the remote has is your own work:
   then nothing is applied automatically, as before.
2. A *switch* is going back to an older version (`"downgrade": true`), usually
   Beta to Stable. Software Update says so plainly: "Going back to Stable:
   this installs an older version of Tessera", how many changes it's behind,
   and which ones go away (`"leaving"`). Your settings and shortcuts are
   backed up first; switching back to Beta brings the changes back.
3. Update (`apply`, behind the update screen) checks out the channel's branch
   at its version (a fast-forward when it's newer on the same branch) and makes
   the checkout follow it, then installs that version's tessera-ui as below.
   On any failure everything goes back: the branch that was checked out, at
   the version it was, your edits, your settings, and the previous tessera-ui.

`tessera-update.py info` (no download) gives the channel, branch, version and
its date: the line next to the picker ("On Beta: version 1a2b3c4d, from 14
November 2026").

Going back after an update stays as it was: the system part has **Go back to
before the last update** (pacman's cache), tessera-ui keeps the previous
build as `.prev` (`native-install.py restore`), and a failed Tessera update
rolls back on its own. To return to the version you had before a channel
switch, switch back.

## Build channels

| Releases | When | Where | How long it stays |
|---|---|---|---|
| **Stable** | the release builder finds a new `native/` tree on `main` | the prerelease `tessera-ui-latest`, one asset set per tree | The **three newest** builds. Older ones are deleted, so an older commit's build is gone soon after main moves on. Installing or updating to such a commit builds locally: that's what the fallback is for. Updating right after a push, before the build is out (server1 looks every 10 minutes and builds for about an hour; the laptop publishes within 10 minutes after), also builds locally, and the check says so. |
| **Beta** | the same, for `beta` | the prerelease `tessera-ui-beta`, the same way | The three newest builds of beta. |
| **Tags** | the release builder finds a `v*` tag without a build | that tag's own release | As long as the release exists. |

The installer looks in its channel's rolling release first, then the other's
(`native-install.py plan --channel beta` to ask for one). Promoting beta to
main pushes the same `native/` tree to main: server1 then packs beta's build
again for main (no new build: it keeps the newest six trees it built), the
laptop signs and publishes it to `tessera-ui-latest`, and until that's done
the build is found in `tessera-ui-beta`.

Each set is `tessera-ui-<tree>-x86-64-v3.tar.zst`,
`tessera-ui-<tree>-generic.tar.zst` (both programs in each),
`tessera-ui-<tree>.manifest.json` (commit, tree, version, channel: stable,
beta or release, and each
variant's file, size and sha256, and each program's sha256 and version), and a
`.minisig` signature next to each file.

## The release builder

GitHub Actions can't run while the account is locked for billing, so two of
the user's machines make the releases instead, each doing one half:

| Machine | Does | Never does |
|---|---|---|
| **server1** (`tools/release-builder/tessera-release`, as `~/bin/tessera-release`; a systemd user timer on the tens: :00, :10, …) | builds both variants, packs them and writes the manifest, exactly like the workflow; leaves the set ready | sign (there is no key on server1) or publish |
| **the laptop** (`tools/release-builder/publish-release`, as `~/tessera-verify/publish-release.sh`; a systemd user timer on the fives: :05, :15, …) | fetches each ready set, checks it, signs it, checks the signatures, publishes it with its own `gh`, marks it published on server1 | build anything (light work only: ssh, scp, checksums, minisign, gh) |

What they make is exactly what `.github/workflows/tessera-ui.yml` made (same
names, archives, manifest and signatures), so `tools/native-install.py` can't
tell the difference. The workflow is kept, but it runs only by hand now (and
fails to start while Actions are locked): two publishers of the same tree
would upload different builds, and installs refuse a manifest that doesn't
match its archive. Turn its push trigger back on only after stopping both
timers.

Releases upload fine while Actions are locked: the lock stops workflows, not
the releases API (the first beta release went out this way on 17 November
2026).

### server1: building

For `main`, `beta` and every `v*` tag, each run:

1. Fetches them from GitHub (https, the public repository) into its own clean
   clone, `~/tessera-release/src` (never `~/iRice`, `~/tessera-build` or the
   agents' `~/tb-*`).
2. Names the build by `git rev-parse <commit>:native`. Nothing to do when the
   release already has that tree's manifest (GitHub, asked anonymously), or a
   set for it is ready or published here. A commit without
   `tools/tessera-ui.pub` is skipped (said once in the log): nothing built from
   it could be checked. That's why `main` is skipped until it has the key.
3. Builds both variants like the workflow: `cargo build --release --locked`
   (the full `release` profile: LTO, one codegen unit; never `fast`) for
   tessera-ui and tessera-events, `RUSTFLAGS="-C target-cpu=x86-64-v3"` and
   `RUSTFLAGS=""`, with `TESSERA_BUILD_CHANNEL` (the target's channel),
   `TESSERA_BUILD_VARIANT`, `TESSERA_BUILD_KIND=release`,
   `TESSERA_BUILD_COMMIT` and `TESSERA_BUILD_DATE` for tessera-ui (so its
   `--version` says what it is: "Which build is this?" above), each variant
   in its own target folder
   (`~/tessera-release/target/<variant>`). Every cargo goes through
   `~/bin/tbuild` (not urgent: a cgroup scope with CPU and IO weight 10 and
   capped memory, paused by tgovernor when memory runs short), so the
   machine's own work always comes first. The first beta build took 27 + 25
   minutes, sharing the machine with another build. Built variants are kept in
   `~/tessera-release/built/<tree>` (the newest six trees).
4. Packs and writes the manifest exactly like the workflow into
   `~/tessera-release/out/<release>/<tree>/` (both archives and the manifest,
   unsigned) and writes `READY` last: what the set is, and each file's size and
   sha256. The laptop's mark renames it `PUBLISHED`; the three newest
   published sets per release are kept.

```sh
tessera-release                     # what the timer runs: main, beta, v* tags
tessera-release --branch beta       # one branch (no tags)
tessera-release --build-only        # build and pack; no ready set
tessera-release --status            # the status JSON
tessera-release --list-ready        # the sets waiting for the laptop
systemctl --user start tessera-release            # a run now, as the timer would
systemctl --user disable --now tessera-release.timer   # stop building releases
bash tools/release-builder/install.sh             # install or update it (from a checkout on server1)
```

### The laptop: signing and publishing

Each run first checks what it needs: the key `~/.minisign/tessera-ui.key`,
its password in `~/.minisign/tessera-ui.password` (mode 600), minisign, and
`gh` logged in to an account that can publish to `monishbl/tessera`. When one is
missing it says what to do (once in the log, and in the status file),
fetches nothing, and the sets wait on server1. Then for each set
`tessera-release --list-ready` gives (over `ssh -o BatchMode=yes
user@server1`):

1. Copies it with scp into `~/.cache/tessera-publish/<release>/<tree>/`.
2. Checks it: exactly both archives and the manifest; each file's size and
   sha256 as server1 listed them; the manifest's format, tree, commit and
   release, and both archives' size and sha256; and, when the checkout here
   has the commit, that its `native/` is that tree. A set that fails is
   neither signed nor published, and the log says why.
3. Signs each file with minisign (the password handed to minisign on its
   input, never written or logged anywhere else), the signed comment naming
   the file, the tree and the commit (what installs check), and checks every
   signature against `tools/tessera-ui.pub` of that commit: from the checkout
   here (`TESSERA_REPO`, default `~/Projects/Personal/iRice`), else that
   commit's file on GitHub, never server1's copy. A key and public key that
   don't belong together never get published.
4. Publishes with `gh` to its release (created the first time as the workflow
   did: a prerelease whose tag points at that commit for main and beta; a
   tag's own release for a tag): the archives first, the manifest last; a
   rolling release keeps its three newest builds. Then downloads the manifest
   back from GitHub and compares it.
5. When `~/.config/tessera/gitlab-token` exists here, copies the files to the
   GitLab mirror's package registry (skipped quietly without it; a failure
   there is only a warning).
6. Marks the set published on server1 (`tessera-release --mark-published`),
   so it's never offered again.

```sh
~/tessera-verify/publish-release.sh             # what the timer runs
~/tessera-verify/publish-release.sh --dry-run   # fetch, check and sign into the staging folder; publish nothing
~/tessera-verify/publish-release.sh --status    # the status JSON
systemctl --user start tessera-publish           # a run now, as the timer would
systemctl --user disable --now tessera-publish.timer   # stop publishing
bash tools/release-builder/install-publisher.sh  # install or update it (from this checkout)
```

### Where to look

| What | Where |
|---|---|
| server1's log | `~/.cache/tessera-release.log` (cargo's own output: `~/tessera-release/logs/<tree>-<variant>.log`) |
| server1's status | `~/.cache/tessera-release.json`: `state` (`idle`, `building`, `ready`, `failed`), `target`, `tree`, `commit`, `step`, `progress` (`done` crates, `about` from the last build), `started` and `finished` (the last build), `checked` (the last look), `error`, `ready` (the sets waiting for the laptop), and `targets.<name>`: the last result per branch or tag (`ready`, `published`, `up-to-date`, `built`, `skipped` or `failed` with `why`) |
| the laptop's log | `~/.cache/tessera-publish.log` |
| the laptop's status | `~/.cache/tessera-publish.json`: `state` (`idle`, `fetching`, `checking`, `signing`, `publishing`, `failed`), `current`, `waiting`, `checked`, `error`, and `published.<release>`: the last set published (tree, commit, URLs, sizes, seconds) |
| the timers | `systemctl --user list-timers 'tessera-*'`, `journalctl --user -u tessera-release` (server1), `journalctl --user -u tessera-publish` (laptop) |

### What it needs, once

Nothing here needs sudo.

1. **On the laptop:** the signing key at `~/.minisign/tessera-ui.key` (mode
   600) and its password in `~/.minisign/tessera-ui.password` (mode 600, the
   password on one line):

   ```sh
   bash -c 'umask 077; read -rs -p "Key password: " pw && printf "%s\n" "$pw" > ~/.minisign/tessera-ui.password; echo'
   ```
2. **On the laptop:** minisign (`sudo pacman -S minisign`) and `gh` logged in
   as an account that can publish to `monishbl/tessera` (the active account; it
   checks before every publish and says so when it can't). ssh to
   `user@server1` with a key (BatchMode: it never asks for a password).
3. **On server1:** cargo, zstd, git, and `~/bin/tbuild`. No key, no password,
   no GitHub login.
4. **Optional, on the laptop:** the GitLab mirror's token in
   `~/.config/tessera/gitlab-token` (mode 600), and its project path in
   `~/.config/tessera/gitlab-project` when it isn't `monishbl/tessera`.

## The signing key

The key pair was made once (16 November 2026) with minisign:

```sh
mkdir -p ~/.minisign
minisign -G -p tools/tessera-ui.pub -s ~/.minisign/tessera-ui.key -c "Tessera tessera-ui release key"
```

The public half is committed as `tools/tessera-ui.pub`. The secret key and its
password live only on the user's laptop (`~/.minisign/`), where releases are
signed; never on the build machine. Back both up somewhere safe (a password
manager): without them nobody can sign again, and every install would need a
new public key. The publisher never copies or prints either.

**If the key leaks:** make a new pair (as above), put its password beside it,
and commit the new `.pub` to main and beta. Copies of Tessera from before that
commit then refuse new builds and build locally until they update. Delete the
releases signed with the old key.

## Checking a build by hand

```sh
tree=$(git rev-parse HEAD:native)
base=https://github.com/monishbl/tessera/releases/download/tessera-ui-latest   # or …/download/v1.2.3
curl -LO "$base/tessera-ui-$tree.manifest.json" -LO "$base/tessera-ui-$tree.manifest.json.minisig"
curl -LO "$base/tessera-ui-$tree-x86-64-v3.tar.zst" -LO "$base/tessera-ui-$tree-x86-64-v3.tar.zst.minisig"

# The signatures (each prints its signed comment: the file, tree= and commit=)
minisign -V -p tools/tessera-ui.pub -m "tessera-ui-$tree.manifest.json"
minisign -V -p tools/tessera-ui.pub -m "tessera-ui-$tree-x86-64-v3.tar.zst"

# The checksum the signed manifest gives
jq -r '.variants["x86-64-v3"].sha256' "tessera-ui-$tree.manifest.json"
sha256sum "tessera-ui-$tree-x86-64-v3.tar.zst"

# What's inside
tar --zstd -tvf "tessera-ui-$tree-x86-64-v3.tar.zst"
```

`tools/native-install.py fetch` does the same checks in a temporary folder
and installs nothing.

## Mirrors

Tessera's code and its ready-made builds come from an ordered list,
`tools/sources.txt` (one place; `TESSERA_MIRRORS="url url …"` overrides it
for one run, and `setup.sh` carries a copy, kept the same by
`tests/test_mirrors.py`):

| Order | Source | Used for |
|---|---|---|
| 1 | `https://github.com/monishbl/tessera.git` | everything (today's only source) |
| 2 | `https://gitlab.com/monishbl/tessera.git` | the code (git), and the builds from its generic package registry: `https://gitlab.com/api/v4/projects/monishbl%2FiRice/packages/generic/tessera-ui/<tag>/<file>` |
| 3 | `https://codeload.github.com/monishbl/tessera/tar.gz/refs/heads/<branch>` | a fresh install only: the files without git history (Software Update can't update such a copy until `setup.sh` is run again) |

- **setup.sh** clones from the first that works; a source is looked at first
  (5 seconds to connect, `info/refs`), so a dead one, or a GitLab project that
  doesn't exist yet (it answers 401), is passed over in a second or two,
  never with a password prompt. A copy cloned from a mirror gets GitHub as its
  `origin`.
- **Software Update** (`tools/tessera-update.py`, and `tessera update` through
  its `fetch` command) fetches the channel's branch (`main` or `beta`) from
  `origin`, then, when `origin` is one of these sources, from each other one
  by URL into the same `origin/<branch>`: your remote is never rewritten. A
  fork's checkout only uses its own remote.
- **Ready-made tessera-ui** (`tools/native-install.py`) looks in GitHub's
  releases, then the GitLab mirror's packages. Every source is checked
  exactly the same way: the signature against `tools/tessera-ui.pub`, the
  manifest's tree and commit, each checksum. A download refused anywhere is
  never installed; a mirror without the build just falls through.

### Pacman mirrors and the AUR

Before the first package is installed or updated (`install.sh`,
`tools/setup-tty.sh`, Software Update's system part), `tools/pac.sh` checks the
mirror lists pacman.conf uses, Arch's `mirrorlist` and CachyOS's
`cachyos*-mirrorlist`: fewer than 5 servers, or the first server (or 2 of the
first 3) behind or not answering, and they're re-ranked
(`cachyos-rate-mirrors` on CachyOS, else `reflector --latest 20 --protocol
https --sort rate`, installed first when missing), each old list kept once as
`<list>.tessera-backup`. The probes run side by side, 3 seconds each.
`--plan` and `--dry-run` show it as a "Mirrors" row and change nothing.
Manjaro's lists (pacman-mirrors) are left alone. Software Update asks
`tools/pac.sh mirrors-stale` as you; its root part re-ranks with fixed
commands only (`tessera-root update --mirrors`, which takes nothing but those
lists' paths).

When aur.archlinux.org doesn't answer, AUR packages (yay itself, the pointer
and emoji extras) come from Arch's GitHub mirror of the AUR, one branch per
package (`git clone --branch <pkg> --single-branch
https://github.com/archlinux/aur.git`), built with `makepkg -si`.

### One-time setup: the GitLab mirror

Until this is done, every source after GitHub is passed over quietly, and the
laptop's publisher skips the upload (quietly, never a failed publish).

1. On gitlab.com, create the project **monishbl/tessera** (empty, without a
   README), **public**, so installs need no login. (Another path: change it in
   `tools/sources.txt`, `setup.sh` and the README, and set the repository
   variable or secret `GITLAB_PROJECT` to it.)
2. In the project: Settings → Repository → Protected branches: allow force
   push on `main` (and `beta` once it exists) for Maintainers, since the
   mirror always follows GitHub.
3. Settings → Access tokens: a **project access token**, role Maintainer,
   scopes **write_repository** and **api**, with an expiry you'll remember.
4. For the builds: on the laptop that publishes, put the token in
   `~/.config/tessera/gitlab-token` (mode 600; the project path in
   `~/.config/tessera/gitlab-project` if it isn't `monishbl/tessera`). The
   publisher then uploads each new build and its signatures to the
   project's package registry (`tessera-ui/<tag>`), keeping the three newest
   builds of each rolling tag. Builds published before the token existed
   reach GitLab with the next change to `native/`.
5. For the code: on GitHub, Settings → Secrets and variables → Actions → New
   repository secret **`GITLAB_TOKEN`** with that token (optional:
   `GITLAB_PROJECT`), and run the **gitlab-mirror** workflow once by hand
   (Actions → gitlab-mirror → Run workflow); from then on every push to `main`
   or `beta` and every tag updates it. This one is still a GitHub workflow:
   while the account's Actions are locked it doesn't run, and the mirror's
   git copy stands still.

Renew the token before it expires: with an expired one the gitlab-mirror job fails at the
push and the publisher's GitLab step warns; the mirror stops moving (GitHub keeps working).
