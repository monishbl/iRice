# Icons (core: icon lookup for every surface)

The user: "icons are not loading" (live: Material dark, icon theme Colloid,
installed in ~/.local/share/icons; Colloid's folders are links into
Colloid-Light, `apps@2x -> apps`, `Inherits=hicolor,breeze`).

## The QML (shell/common/IconLookup.qml, assets/scripts/icon-index.py)

| behaviour | QML | Rust before | status now |
|---|---|---|---|
| theme | $QS_ICON_THEME (launch-shell.sh: Settings' iconTheme, Adwaita, KDE, GNOME, GTK, then Papirus…) | services/icons.rs theme_name: same | same |
| index | icon-index.py on every start, run in the background: the theme, what it inherits, hicolor, /usr/share/pixmaps; scalable/large over small, colour over symbolic; written to ~/.cache/tessera/icon-index-<theme>.tsv, re-made when older than a week or a theme folder changed | read the .tsv if there was one, **whatever its age**, else walked the themes; **never wrote it** (standalone the QML never runs: no index file ever, or a stale one forever) | the .tsv sent at once, then checked as icon-index.py checks it (age, folders' times); stale or missing: walked, sent again, written for next time |
| links to nothing | (Qt's iconPath fallback covers it) | a broken link could be the "best" file: the name then found nothing | links to nothing skipped when indexing |
| name variants | name, lower case, a reverse-DNS id's last part, spaces → dashes; fallbacks; `-symbolic`; application-x-executable | Icons::path/exact: same | same |
| other lookups (tray, the menu bar's app icons by window class, the lock's and the island's notification icons, the dock's stacks, Quick Look, Settings) | IconLookup everywhere | core/src/icon_theme.rs: **the GTK theme from gtk-3.0/settings.ini only** (not Settings' iconTheme), no inheritance, size/kind folders guessed; misses before anything was indexed remembered forever | the shell's index first (same variants); before it's built, the shell's theme chain walked, then GTK's, Papirus, breeze, Adwaita, hicolor; misses before the index aren't kept, and the index's arrival clears what was found without it |
| Notification icons (banners, Center) | IconLookup.path | own lookup: re-read the whole .tsv per name, else `<theme>/<size>/<kind>` only (Colloid is `<kind>/<size>`: never found), misses kept forever | core's lookup; misses kept only once the index is there |
| who starts the index | IconLookup is a singleton, always | only parts that `want("icons")` (dock, launchpad, desktop, spotlight…); the tray and the menu bar never did | also with the tray, Hyprland's events (the menu bar's app icons) and notifications |
| an icon file that can't be drawn (an SVG resvg refuses) | IconImage shows nothing | AppIcons re-queued it on every refresh, drawing it again and again, never showing anything | remembered as failed: the generic app icon instead, logged with TESSERA_PERF (`debug: icon not drawn: <path>`) |

Not changed (other agents' crates, to check when they refresh): the menu
bar keeps `app_icons` per class forever (surfaces/menubar/src/lib.rs:242):
an icon resolved before the index arrived stays the fallback until restart.
It should drop that map on `data(Icons)`. Parts using AppIcons ask again on
each refresh, so they pick the index up.

## Test plan (tester; arena with the user's config: Material dark, Colloid)

Setup: the arena's home needs Colloid: copy (not link) the user's
`~/.local/share/icons/Colloid`, `Colloid-Light`, `Colloid-Dark` into
`.sandbox/arena-N/home/.local/share/icons/` (cp -a, so the links inside
stay links). Delete `.sandbox/arena-N/home/.cache/tessera/icon-index-*.tsv`
first (standalone with no index: the user's case). Run with TESSERA_PERF=1.

1. Start standalone. Within ~5 s `.cache/tessera/icon-index-Colloid.tsv`
   exists and has > 50 000 lines. Restart: the log shows no rebuild delay
   (icons appear on the first frame of the dock).
2. Dock: Files (org.gnome.Nautilus or thunar), foot, Firefox (if present),
   Settings, the App Drawer: every icon is Colloid's (compare with
   `Colloid-Light/apps/scalable/<name>.svg`), none blank, none the generic
   executable unless Colloid and hicolor have no icon for it.
3. Launchpad / App Drawer: page through all apps; count blank or generic
   icons; for each, name the desktop entry's Icon= and whether Colloid has it.
4. Window classes: open foot, thunar, `gnome-calculator`
   (org.gnome.Calculator), a flatpak if any, and a Steam game window if
   possible (class steam_app_N): Mission Control, Overview, the switcher
   (Alt+Tab) and the menu bar's app name/icon show the app's icon.
5. Notifications: `notify-send -a Firefox -i firefox 'A' 'b'`,
   `notify-send -a Files -i org.gnome.Nautilus 'C' 'd'`,
   `notify-send -i dialog-warning 'E' 'f'`, `notify-send 'No icon' 'g'`:
   banners and Notification Center show Colloid's firefox, Nautilus,
   dialog-warning, and the generic information icon.
6. Tray: start an app with a tray icon (nm-applet, blueman, or
   `python3 -I` a small StatusNotifierItem if none): its icon shows.
7. Lock screen notification icons and Quick Look file icons: one each.
8. The window-management surfaces (all through `wm::AppIcons`, window class
   → desktop entry → the index): the switcher's tiles (48 px icon and 16 px
   badge: not the tiny generic circle), Overview's search results (styled app
   icons, not flat glyphs), Overview's grid window badges (a 20 px app icon,
   not a dot or an empty disc), Mission Control's window chips and its dock
   (the apps' Colloid icons, not monochrome glyphs). Open each right after
   the shell starts (prewarmed windows) and again a minute later: the same.
   With TESSERA_PERF, `debug: icon not drawn` lines name any file that fails.
9. Both looks (`--look glass` too). `grep "icon not drawn" tessera-ui.log`:
   list any file (and whether it opens in an SVG viewer).
PASS when every icon the QML shell shows in the same steps (run the QML pass
the same way) shows in the Rust too.

## Step file

`parity/steps/icons.steps`: this plan as runnable steps.
