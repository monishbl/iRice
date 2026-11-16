# Ready-made tessera-ui: releases, signing, channels

Tessera's Rust programs, **tessera-ui** (the interface in Rust) and
**tessera-events** (the system watcher), don't have to be compiled on your
computer. GitHub Actions builds them for every change to `native/` and every
release tag, signs them, and the installer and Software Update download the
build for the exact version they install. You can still build them yourself.

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
| **Stable** | every push to `main` that touches `native/` | the prerelease `tessera-ui-latest`, one asset set per tree | The **three newest** builds. Older ones are deleted, so an older commit's build is gone soon after main moves on. Installing or updating to such a commit builds locally: that's what the fallback is for. Updating right after a push, before CI has finished (about 15 to 25 minutes), also builds locally, and the check says so. |
| **Beta** | every push to `beta` that touches `native/` | the prerelease `tessera-ui-beta`, the same way | The three newest builds of beta. |
| **Tags** | every `v*` tag | that tag's own release | As long as the release exists. |

The installer looks in its channel's rolling release first, then the other's
(`native-install.py plan --channel beta` to ask for one). Promoting beta to
main pushes the same `native/` tree to main: main's workflow builds it again,
and until that's done the build is found in `tessera-ui-beta`.

Each set is `tessera-ui-<tree>-x86-64-v3.tar.zst`,
`tessera-ui-<tree>-generic.tar.zst` (both programs in each),
`tessera-ui-<tree>.manifest.json` (commit, tree, version, channel: stable,
beta or release, and each
variant's file, size and sha256, and each program's sha256 and version), and a
`.minisig` signature next to each file.

The workflow is `.github/workflows/tessera-ui.yml`. It builds in an
`archlinux:base-devel` container (so the libraries match Arch users') with the
full release profile (LTO, one codegen unit), caches cargo with
actions/cache, and has a marked `TODO(PGO)` for profile-guided builds of
stable tags (not built yet).

## One-time setup: the signing key

Until this is done the workflow still builds, then says in its log that it
skipped signing and publishing, and installs build locally. Nobody else can
do these steps for you: the private key must only ever be on your computer
and in the repository's secrets.

1. Install minisign: `sudo pacman -S minisign`.
2. Make the key pair, with a password:

   ```sh
   mkdir -p ~/.minisign
   minisign -G -p tools/tessera-ui.pub -s ~/.minisign/tessera-ui.key -c "Tessera tessera-ui release key"
   ```

   Run it in the Tessera checkout so the public key lands in `tools/`. Back up
   `~/.minisign/tessera-ui.key` and its password somewhere safe (a password
   manager). Without them you can't sign again, and every install would need
   a new public key.
3. Give GitHub the secret key and its password:

   ```sh
   gh secret set TESSERA_UI_MINISIGN_KEY < ~/.minisign/tessera-ui.key
   gh secret set TESSERA_UI_MINISIGN_PASSWORD   # it asks; paste the password
   ```
4. Commit and push the public key:

   ```sh
   git add tools/tessera-ui.pub
   git commit -m "Releases: the tessera-ui signing key (public half)"
   git push
   ```

   The push touches no `native/` file, so run the workflow once by hand
   (Actions → tessera-ui → Run workflow, or `gh workflow run tessera-ui.yml`)
   to publish the first build (and once on the `beta` branch, after you
   create it: `gh workflow run tessera-ui.yml --ref beta`). After that every
   change to `native/` on main or beta, and every tag, publishes on its own.

Before publishing, the workflow checks each signature against the committed
`tools/tessera-ui.pub`, so a key and public key that don't belong together
never get published.

**If the key leaks:** make a new pair (step 2), replace both secrets (step 3)
and commit the new `.pub` (step 4). Copies of Tessera from before that commit
then refuse new builds and build locally until they update. Delete the
releases signed with the old key.

## Checking a build by hand

```sh
tree=$(git rev-parse HEAD:native)
base=https://github.com/monishbl/iRice/releases/download/tessera-ui-latest   # or …/download/v1.2.3
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
| 1 | `https://github.com/monishbl/iRice.git` | everything (today's only source) |
| 2 | `https://gitlab.com/monishbl/iRice.git` | the code (git), and the builds from its generic package registry: `https://gitlab.com/api/v4/projects/monishbl%2FiRice/packages/generic/tessera-ui/<tag>/<file>` |
| 3 | `https://codeload.github.com/monishbl/iRice/tar.gz/refs/heads/<branch>` | a fresh install only: the files without git history (Software Update can't update such a copy until `setup.sh` is run again) |

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
CI jobs below say they skipped (a notice, never a failed build).

1. On gitlab.com, create the project **monishbl/iRice** (empty, without a
   README), **public**, so installs need no login. (Another path: change it in
   `tools/sources.txt`, `setup.sh` and the README, and set the repository
   variable or secret `GITLAB_PROJECT` to it.)
2. In the project: Settings → Repository → Protected branches: allow force
   push on `main` (and `beta` once it exists) for Maintainers, since the
   mirror always follows GitHub.
3. Settings → Access tokens: a **project access token**, role Maintainer,
   scopes **write_repository** and **api**, with an expiry you'll remember.
4. On GitHub: Settings → Secrets and variables → Actions → New repository
   secret **`GITLAB_TOKEN`** with that token. (Optional: `GITLAB_PROJECT`.)
5. Run the **gitlab-mirror** workflow once by hand (Actions → gitlab-mirror →
   Run workflow) to fill the mirror; from then on every push to `main` or
   `beta` and every tag updates it. The **tessera-ui** workflow uploads each
   new build and its signatures to the project's package registry
   (`tessera-ui/<tag>`), keeping the three newest builds of each rolling tag.
   Builds published before the token existed reach GitLab with the next
   change to `native/` (or rerun the workflow).

Renew the token before it expires: with an expired one the gitlab-mirror job fails at the
push and tessera-ui's GitLab step warns; the mirror stops moving (GitHub keeps working).
