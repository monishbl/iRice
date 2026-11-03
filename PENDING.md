# What's pending

Updated 3 November 2026. Branch in progress: `quickshare-fast-first` (not merged to main yet).

## Needs you

- **Phone screen lock**: turn it back on (it was set to None for the Quick Share tests).
- **Phone display size**: you lowered it for the tests; set it back if you like (Settings → Display → Display size).
- **Window controls plugin**: reload it or log in again to get the Material window buttons (I wasn't allowed to reload the running compositor's plugin).
- **Login manager**: switching from ly to iRice's own login screen (greetd) waits for your yes.
- **ananicy rule**: installing iRice's process-priority rule waits for your yes.
- **iPhone**: the QR code / browser link works with an Android phone and a phone-sized browser; try it once with a real iPhone.
- **Material sound pack: source unrecorded.** The 21 sounds in `material/assets/sounds/material/` have no generator and no credit, so their licence is unknown (listed as such in THIRD_PARTY_NOTICES.md). Tell me where they came from (and their licence), or let me regenerate them with `tools/make-sounds.py` like the Glass ones. Until then they shouldn't go into a public release.
- **Licence follow-ups (small):** check emojibase's exact copyright line against its LICENSE file (the notice says "Copyright (c) Miles Johnson"); record which Unsplash photo each website background is.
- **Website media shows the old names:** the hero film (re-render `site/film/hero/film.html`: the Bedside Mode caption) and screenshots that show "App Store", "Spotlight Search" or "Control Center" in the UI. The page text and alt texts already use the new names.

## Quick Share (in progress)

Done on the branch, tested with your phone:

- Fastest link chosen by measurement before data moves (Wi-Fi Direct ahead of a slow 2.4 GHz network): 500 MB phone→laptop in 55 s, laptop→phone in about 10 s.
- Fallback to Bluetooth when the router keeps devices apart; Wi-Fi preferred over a USB tether.
- Only request / done / failed alerts; link changes show in the transfer's live activity.
- Fixes: unknown Bluetooth messages, pkexec and /etc/shells, the faster-link request phones send.

Still to do:

- [x] 5 GB laptop→phone: 79 s (~75 MB/s, phone's 5.8 GHz Wi-Fi Direct). 5 GB phone→laptop: 160 s (~32 MB/s, laptop's 5 GHz group; ~6 MB/s when it was stuck on 2.4 GHz). 2 GB as 778 files laptop→phone: 66 s. All checksums match.
- [x] Applied to the laptop (main 60cebd4), Wi-Fi helper installed, Hyprland config reloaded.
- [x] Laptop on no network: 5 GB phone→laptop 158 s (Bluetooth then 5 GHz Wi-Fi Direct).
- Phone Wi-Fi off: Android's Quick Share refuses to send at all (its rule, not ours). Phone hotspot: skipped, as you chose. 2 GB of small files phone→laptop: not done (the phone's picker needs a tap per file).
- [ ] Laptop on 2.4 GHz while receiving: the new 5 GHz group next to it (two channels at once) is unit-tested; check it on the real card the next time the Wi-Fi is on 2.4 GHz.
- [ ] Phones are slow to find the laptop over Bluetooth alone (minutes; they find it at once over Wi-Fi). The laptop sends the slower kind of Bluetooth advertisement. Look into the fast kind.
- [x] Merged to main and applied to the laptop; test state put back (Quick Share visible to Everyone, Bluetooth off, test files removed).

## Bugs reported

- [ ] Quick Settings (Material): right-clicking the volume slider leaves the panel faded and slid left, the sound page never showing. First in the full bug sweep.

## Approved, not started

- **Settings sweep**: one control instead of a switch plus option (Stages Off / On / On with recents; Dock magnification slider with Off at its left end; Cool-down Off / Ask / Automatic; Sounds with Off first). Each setting on one page with a link elsewhere (find the pointer → Mouse & Trackpad; calendars and task list → Calendar). A test that enforces both rules.
- **Code structure**:
  - Phase 1: remove dead code.
  - Phase 6: efficiency fixes (wavy progress redraws, the hidden Glass UI in the Material lock screen, Resources history, live blur in popups, Mouse polling, island ink layer).
  - Phase 2: merge trivial Material copies back into the shared code (Glass stays pixel-identical).
  - Phase 3: shared components.
- **Bug hunt and efficiency sweep**: both looks and the core.

## Built, waiting to merge

- **Installer**: merged into main (586 tests pass). Was the branch `worktree-agent-a030afe4e31ec7a9c`: interactive TUI plus classic CLI, hardware-aware plan, snapshot first, optional icon themes. To do: review, merge, and verify in a container. Small follow-ups:
  - README still mentions MacTahoe and the macOS pointer.
  - `dev.sh` still lists papirus.
  - Dropped AUR packages aren't in Software Update's removal list.
  - `site/public/get` isn't deployed.
- **CHANGELOG**: entries for the latest work (icons, gaps, Settings restructure, battery, captions, overview, find pointer, Adwaita and Bibata, Quick Share, QR sharing, menus, gestures).

## Done today (for reference)

- Applied to the laptop: original app logos (Clear in Glass, Themed in Material), window gaps setting (Compact), one Off / Bigger / Ring control for Shake to find the pointer, Adwaita for GTK apps, Bibata pointer.
- On the branch: menus stay on screen and scroll; Settings dropdowns fit the window; pinch and spread the right way round; iPhone sharing by QR code.
