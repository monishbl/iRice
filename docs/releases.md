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
   main's rolling prerelease, `tessera-ui-latest`. Downloads use curl
   (https only, timeouts, three retries, a size limit).
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

## Channels

| Channel | When | Where | How long it stays |
|---|---|---|---|
| **latest** | every push to `main` that touches `native/` | the prerelease `tessera-ui-latest`, one asset set per tree | The **three newest** builds. Older ones are deleted, so an older commit's build is gone soon after main moves on. Installing or updating to such a commit builds locally: that's what the fallback is for. Updating right after a push, before CI has finished (about 15 to 25 minutes), also builds locally, and the check says so. |
| **stable** | every `v*` tag | that tag's own release | As long as the release exists. |

Each set is `tessera-ui-<tree>-x86-64-v3.tar.zst`,
`tessera-ui-<tree>-generic.tar.zst` (both programs in each),
`tessera-ui-<tree>.manifest.json` (commit, tree, version, channel, and each
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
   to publish the first build. After that every change to `native/` and
   every tag publishes on its own.

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
