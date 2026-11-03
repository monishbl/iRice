# What's pending

Updated 3 November 2026, late. The refactor is merged to main and applied to the laptop.

## Needs you

- **Phone screen lock**: turn it back on (it was set to None for the Quick Share tests).
- **Phone display size**: you lowered it for the tests; set it back if you like (Settings → Display → Display size).
- **Log out and back in once**: the new glass plugin (pointer and focus fixes, Material window buttons) is built and installed, but Hyprland only loads it at login. The health check's "older build is loaded" warning clears then.
- **Login manager**: switching from ly to iRice's own login screen (greetd) waits for your yes.
- **ananicy rule**: installing iRice's process-priority rule waits for your yes.
- **iPhone**: the QR code / browser link works with an Android phone and a phone-sized browser; try it once with a real iPhone.
- **Sharing's firewall ports** (branch `worktree-agent-a71c4b29ca32ea7a6`, once merged): run Software Update once, or `pkexec /usr/lib/irice/irice-root setup ~/iRice/shell`, to install the new root helper and polkit rule. Until then, Settings → Privacy & Security → Firewall says "Run Software Update once". The first run then removes the old permanent ufw rules.
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

## Done: full refactor, no duplicated code

- Material's copies of Glass files: 151 → 2 (both kept on purpose), repeated lines 31,938 → 55. 664 tests pass; both looks pass the smoke check in two sandboxes side by side; the screens the smoke check doesn't open (lock, share sheet, banners, OSD, island, password prompt, Quick Preview, screen tools, shelf, captions, welcome, widgets, Software, screenshot editor) checked by hand in both looks.

## Watch

- **Glass shell froze in the sandbox** (stopped drawing and answering for minutes) in 2 smoke runs this evening, before the refactor merged too; it hasn't come back in 4 runs since, including two side by side under load, nor in 135 panel opens with diagnostics. The smoke check now fails on it and reports whether the compositor or the shell was stuck. If you see the desktop freeze, note the time.

## Next, after the refactor

- **Full Material walkthrough against Glass** (the bug sweep, both looks and the core): every panel, page, menu, right-click, slider and gesture side by side; each difference fixed and tested. Known: the volume right-click (check after the reboot), a blank icon in the Material dock.
- **Efficiency**: the 33 small watcher processes (one supervisor instead).
- **Later, when you say go**: the Rust rewrite, benchmarked against docs/benchmarks-baseline.md.

## Housekeeping

- Website: re-render the hero film and the screenshots that show old names; `site/public/get` isn't deployed (pushing site/ deploys: ask first).
- `dev.sh` still lists papirus.
- Needs you as well: run `pkexec /usr/lib/irice/irice-root setup ~/iRice/shell` once (the Wi-Fi helper's firewall change).

## Done today (for reference)

- Applied to the laptop: original app logos (Clear in Glass, Themed in Material), window gaps setting (Compact), one Off / Bigger / Ring control for Shake to find the pointer, Adwaita for GTK apps, Bibata pointer.
- On the branch: menus stay on screen and scroll; Settings dropdowns fit the window; pinch and spread the right way round; iPhone sharing by QR code.
