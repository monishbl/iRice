# What's pending

Updated 3 November 2026. Branch in progress: `quickshare-fast-first` (not merged to main yet).

## Needs you

- **Phone screen lock**: turn it back on (it was set to None for the Quick Share tests).
- **Phone display size**: you lowered it for the tests; set it back if you like (Settings → Display → Display size).
- **Window controls plugin**: reload it or log in again to get the Material window buttons (I wasn't allowed to reload the running compositor's plugin).
- **Login manager**: switching from ly to iRice's own login screen (greetd) waits for your yes.
- **ananicy rule**: installing iRice's process-priority rule waits for your yes.
- **iPhone**: the QR code / browser link works with an Android phone and a phone-sized browser; try it once with a real iPhone.

## Quick Share (in progress)

Done on the branch, tested with your phone:

- Fastest link chosen by measurement before data moves (Wi-Fi Direct ahead of a slow 2.4 GHz network): 500 MB phone→laptop in 55 s, laptop→phone in about 10 s.
- Fallback to Bluetooth when the router keeps devices apart; Wi-Fi preferred over a USB tether.
- Only request / done / failed alerts; link changes show in the transfer's live activity.
- Fixes: unknown Bluetooth messages, pkexec and /etc/shells, the faster-link request phones send.

Still to do:

- [x] 5 GB laptop→phone: 79 s (~75 MB/s, phone's 5.8 GHz Wi-Fi Direct). 5 GB phone→laptop: 160 s (~32 MB/s, laptop's 5 GHz group; ~6 MB/s when it was stuck on 2.4 GHz). 2 GB as 778 files laptop→phone: 66 s. All checksums match.
- [x] Applied to the laptop (main 60cebd4), Wi-Fi helper installed, Hyprland config reloaded.
- [ ] 2 GB of small files phone→laptop (the phone's file picker needs many taps to select them).
- [ ] Remaining conditions: phone Wi-Fi off, laptop not on any network, phone hotspot.
- [ ] Laptop on 2.4 GHz while receiving: the new 5 GHz group next to it (two channels at once) is unit-tested; check it on the real card the next time the Wi-Fi is on 2.4 GHz.
- [ ] Phones are slow to find the laptop over Bluetooth alone (minutes; they find it at once over Wi-Fi). The laptop sends the slower kind of Bluetooth advertisement. Look into the fast kind.
- [ ] Merge to main and apply to the laptop.
- [ ] Put test state back: Quick Share visibility to Everyone, laptop Bluetooth off, test files removed from the phone.

## Approved, not started

- **Settings sweep**: one control instead of a switch plus option (Stage Manager Off / On / On with recents; Dock magnification slider with Off at its left end; Cool-down Off / Ask / Automatic; Sounds with Off first). Each setting on one page with a link elsewhere (find the pointer → Mouse & Trackpad; calendars and task list → Calendar). A test that enforces both rules.
- **Code structure**:
  - Phase 1: remove dead code.
  - Phase 6: efficiency fixes (wavy progress redraws, the hidden Glass UI in the Material lock screen, Resources history, live blur in popups, Mouse polling, island ink layer).
  - Phase 2: merge trivial Material copies back into the shared code (Glass stays pixel-identical).
  - Phase 3: shared components.
- **Bug hunt and efficiency sweep**: both looks and the core.
- **Security and legal audit**: licences of bundled fonts, icons and protocol code; trademark use in names; secure defaults; tests that hold them. Anything that would remove or rename a feature comes to you first.

## Built, waiting to merge

- **Installer** (branch `worktree-agent-a030afe4e31ec7a9c`): interactive TUI plus classic CLI, hardware-aware plan, snapshot first, optional icon themes. To do: review, merge, and verify in a container. Small follow-ups:
  - README still mentions MacTahoe and the macOS pointer.
  - `dev.sh` still lists papirus.
  - Dropped AUR packages aren't in Software Update's removal list.
  - `site/public/get` isn't deployed.
- **CHANGELOG**: entries for the latest work (icons, gaps, Settings restructure, battery, captions, overview, find pointer, Adwaita and Bibata, Quick Share, QR sharing, menus, gestures).

## Done today (for reference)

- Applied to the laptop: original app logos (Clear in Glass, Themed in Material), window gaps setting (Compact), one Off / Bigger / Ring control for Shake to find the pointer, Adwaita for GTK apps, Bibata pointer.
- On the branch: menus stay on screen and scroll; Settings dropdowns fit the window; pinch and spread the right way round; iPhone sharing by QR code.
