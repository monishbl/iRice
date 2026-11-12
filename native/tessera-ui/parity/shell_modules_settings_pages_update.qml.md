# shell/modules/settings/pages/update.qml ↔ surfaces/settings (src/pages/about/update.rs `page`, ui/settings-about.slint notes/pkgs/steps/progress/log)

Software Update (page `update`). The updater is the shell's (SoftwareUpdate,
TesseraUpdate, Updates, BootSnapshots); Rust reads feed.about.su/tu/up/bs
(RustBridgeAbout.qml 171–245) and asks `su.*`, `tu.*`, `up.*`, `bs.*`.
UpdateNotes.qml and LogView.qml have their own parity files.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | On open SoftwareUpdate.checkUndo(), BootSnapshots.refresh() (24–27) | RustBridgeAbout.qml 27–29 | same |
| 2 | showPlan = checked && !running && !over && anything; over = job && state ≠ running && !running (23, 28) | update.rs 127–128 | same |
| 3 | Status footer: sandbox text / "Tessera was updated… backed up to <~path>." / "Nothing is checked on its own…" (43) | 133–140 (bridge `home()` makes the ~) | same |
| 4 | Status label: Updating… / Checking for updates… / Software Update / Updates are ready / Everything is up to date (45) | 141–151 | same |
| 5 | Status detail: before a check the pacman/AUR text; else "Tessera <current>[ → <latest>]**[ · <report.ui.label>]**[ · Hyprland <now>]" (46) | 153–164 | **missing**: the ` · <report.ui.label>` part (feed has no `ui`) |
| 6 | "Check for updates"/"Check again" (arrow-clockwise, busy while checking), hidden while running → check() (47–53) | 165–173 | same |
| 7 | "Restart to finish the last update" (restartPending, not running): Restart → systemctl reboot (56–65) | 174 | same |
| 8 | Error group: "Tessera's part can't go ahead", detail error, "Try again" → check() (69–80) | 177–179 | same |
| 9 | "What's new in Tessera" (plan, available, changelog or notes): UpdateNotes 14 px top/bottom (83–95) | 182–186 | see UpdateNotes parity |
| 10 | Conflict groups in order setting/preset/shortcut/removed/code with titles and footers; each conflict a row: detail (+ "\nmine · theirs" for files), Segmented max(implicit, 260) of short labels, −1 until chosen → TesseraUpdate.choose (96–133) | 15–21, 24–34, 187–220 | same |
| 11 | "Hyprland" group (plan, hyprTooNew): footer by hold (held list / falls back / tested with max); "Hyprland <short> isn't supported…" Segmented ≥ 300 Keep <now or "the current one"> / Update anyway, current by hold (136–151) | 223–238 | same |
| 12 | "Packages affected": headings System · N update(s) (+ AUR when included) "(N held)", Tessera adds, Tessera drops; lines 26 px: name (≤ 45 %, medium), badge pill 18 (held for Tessera / skipped · you went back from it / AUR / AUR · not included / removed / kept), right text "from  →  to" or reason, mono for versions; dim 0.5; first 8 + accent "and N more"/"Show fewer" (156–295) | 104–116, 241–294; slint pkgs 261–348 | same (name not capped at 45 %: a long name pushes the versions; badge not capped at 30 %) |
| 13 | Footer snapshot / pacman-cache text (234) | 299–303 | same |
| 14 | "Include AUR updates" switch (aurList) and "Remove packages Tessera no longer uses" switch (dropped) (296–313) | 305–309 | same |
| 15 | Update group (plan): "N things left to decide" / "Ready to update"; detail choose-text / plan parts joined, first letter capital; "Update" (arrow-circle-up, primary), disabled (0.45) unless canUpdate (317–333) | 313–321, plan_detail 37–50 | same |
| 16 | Progress group (running or over), title "Update", footer fake / keeps going / ""; label & detail per interrupted / running (applying, job.text, auth) / done (undo, N packages updated, Updated) / error / restart; buttons Cancel (cancellable), Restart (done + restart), Done (over) (336–364) | 324–391 | same |
| 17 | Steps line (steps > 1): names joined "  ·  ", current bold, 30 px, **elided** (366–378) | 366, 392–394; slint 351–371 | different (small): no elide (a long list runs past the row) |
| 18 | Progress (Updates.running): WavyProgress max(0.02, p) + "N%" tnum, 32 px (379–394) | 395–397; slint 374–418 | different: Material is a flat bar, not the wave (documented in settings-about.slint) |
| 19 | "Details" row (job): detail pacman (+ " and the AUR helper"); Copy (when shown) + Show/Hide details (terminal-window / caret-up) (396–414) | 368–380, 398–399 | same |
| 20 | Output: LogView 240 tall (margins 10), follow, "Waiting for output…", @@step lines dropped; Updates.logUsers counted while shown (415–427) | 400–404; bridge 38–39, 234 | same |
| 21 | "Going back" (not running, and snapshot/undo/skipped/canMenu); footer unless booted from snapshot (434–441) | 409–413, 506–509 | same |
| 22 | "Started from snapshot X" accent label; detail assistant / install; button Open Btrfs Assistant / Installing… / Install Btrfs Assistant (busy, disabled while updates run) (444–456) | 417–438 (`tone 4`) | same |
| 23 | Undo row: label asks/normal; secondary when versions missing; detail missing list (4 …) / asking "N packages go back to the version before <dddd h:mm ap>…" / "The update on <d MMMM, h:mm ap> changed N packages. a, b, c…"; Cancel + Go Back (icon only when not asking), enabled canGoBack; two presses (458–487) | 439–471 (bridge undoDay/undoLong) | same |
| 24 | Skipped packages: "<name> <to>" switch includeSkip (489–500) | 472–483 | same |
| 25 | "Snapshots in the boot menu": detail error / no grub-btrfs / on / off; red on error; Install (busy, disabled while updates run) or switch (disabled while busy) (502–523) | 484–505 | same |
| 26 | "What's new" after an update (result with notes): UpdateNotes (bottom 6) + "Updated to <now>" Done → dismissResult (527–546) | 516–522 | same |

## Test plan

Setups as in pages_menubar. `open update`; read y from a shot. **Never press
Update, Go Back, Restart, Install or the snapshot switch.** "Check for updates"
only downloads package lists and may be pressed. If (and only if) the status
footer reads "Updates are off in the dev sandbox…", the update is simulated
and U4 may be run; otherwise skip U4.

U1 `shot upd-start`. PASS: label "Software Update", detail "Tessera and your
system (pacman, and the AUR…)", "Check for updates" button; same footer.

U2 Click "Check for updates", `shot upd-checking` at once, wait 20 s,
`shot upd-plan`. PASS: "Checking for updates…" with a busy button, then the
plan. Compare the status detail: QML may show " · <ui label>" after the
version — Rust must show the same string.

U3 Packages: if there are > 8, click "and N more" then "Show fewer". PASS:
the list grows/shrinks; badges, dimming and mono versions as QML.

U4 (sandbox only) Update → progress group: steps line, bar + %, Show details
→ the dark log box 240 tall following the end; Hide details; Done when over.
Shot each in both shells.

U5 Going back: if the group shows, click "Go Back" **once** (it only asks),
`shot upd-ask`, then **Cancel**. PASS: label "Go back to the earlier
versions?", detail with the weekday time; Cancel restores.

## Fixed in batch 3 (fixer)

- The status line: " · <the UI's label>" between the version and Hyprland (the standalone shell's about feed and RustBridgeAbout send report.ui).

## Test results

(tester)
