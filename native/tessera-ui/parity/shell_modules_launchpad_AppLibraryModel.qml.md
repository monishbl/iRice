# shell/modules/launchpad/AppLibraryModel.qml

Rust: surfaces/launchpad/src/model.rs (`Lib`, tested in its tests), the
menu in lib.rs (`menu_for`).

| Behaviour | QML | Rust | Status |
|---|---|---|---|
| Groups → category folders, first match wins, "Other" last, empty ones dropped | 13-24, 28-37 | GROUPS, folders() | same |
| Apps: no NoDisplay, sorted case-blind then exact | 27 | sorted() (cats_for filters no_display) | same |
| view / folder / selected; showingApps | 39-44 | Lib | same |
| Search: Search.score(name), generic × 0.8, keywords × 0.6, ≥ 30, best first | 45 | search() over rank::score | same |
| selected clamped when the count changes | 46-47 | Lib::count / select | same |
| reset, toggleView, activate (launch closes the panel; folder opens), back, move | 50-89 | Lib methods | same |
| menuItems: desktop actions under "Shortcuts", then "Actions": Open, Keep in / Remove from Dock | 94-106 | menu_for (actions() from the desktop file) | same |
| letters ("#" for non A–Z), letterAt | 110-127 | letters(), letter_at() | same |
| pick / inPicked; picked cleared when the selection leaves the letter or A–Z ends | 132-142 | pick(), lit(), select() | same |
| Apps.iconFor for every icon (entry icon, a path, else the id; generic fallback) | Apps.qml 76-84 | wm::AppIcons::path → Icons::path(name, application-x-executable) | same lookup; see the icon note below |

Icons (checked with the user's Colloid and with Adwaita): see
`icons_in_the_themes` in model.rs (an ignored test that resolves and draws
every installed app's icon with each theme). On this machine (7 Nov):
Colloid: 71 apps, all found by their own name, all draw; Adwaita: 71, three
fall back to application-x-executable as IconLookup does
(org.prismlauncher.PrismLauncher, freedownloadmanager, hwloc), none fail.
So the launchpad's lookup isn't what lost icons: its grid was a new model
on every refresh (each hover, each icon drawn), so every cell's picture was
dropped and drawn again — blank for a frame or more. Now the cells are one
kept `Rows`.

Known in core (not this crate): `wm::AppIcons::done` doesn't remember an
icon that failed to draw, so it's queued and drawn again on every refresh,
and nothing (not even application-x-executable) is shown for it.

## Test plan

Unit tests: `cargo test --release -p tessera-launchpad` (model tests:
folders, search, letters, back/activate). In the sandbox (either look):
1. Open; the folders listed are the same and in the same order in both
   shells (Productivity, Development, Internet, …, Other), with the same
   counts.
2. Type `term`: the same apps in the same order in both shells.
3. A–Z: the letter index lists the same letters; "#" for digits.
4. Right-click Firefox (or any app with desktop actions): "Shortcuts" with
   its actions, then "Actions": Open, Keep in Dock (Remove from Dock if
   pinned). Choose Keep in Dock: the dock gains it.
5. Icons: every tile shows its theme icon (Colloid) — compare tile by tile
   with the QML screenshot; none blank, none the generic gear where QML
   has a real icon. Repeat with appearance.iconTheme = Adwaita.

## Tester round 1 (desktop-batch1.md): failures and what was done

| Failure | Cause | Status |
|---|---|---|
| App icons in the Categories view are monochrome glyphs (Glass: Clear; Material: Themed circles) where QML shows each app's coloured Colloid logo on its squircle tile | The user's `appearance.iconStyle` is `"default"`. AppIcon.qml reads it as mode 0 (the app's own logo, tiled with iconShape squircle). core's `appicon::Style::from_settings` read `"default"` as the old name of `"look"` (Clear / Themed). The launchpad already draws every icon through the shared lookup and style (`wm::AppIcons`: Apps.iconFor → IconLookup, styled as AppIcon's default `styled: true`, in both views and in folder previews), so it follows core | **needs core's fix**: it is in ef9fd43 on `parity-int` ("iconStyle default is each app's own logo again"), which is **not** in this branch yet (3bbaaa3 doesn't carry it; core/src/appicon.rs here still maps default → look). Nothing to change in the launchpad; merge ef9fd43's appicon.rs and the tiles show the coloured logos |
| Category tiles "lay out differently" | Measured on the tester's shots (rust-/qml-{material,glass}-launchpad.png, crop 380,140–1300,500): card positions, sizes, corners, preview grid and labels match to the pixel in both looks; what differs is only the icons' style (Material's themed circles inside the card read as a different layout) and the accent (core's) | same once the icon style is fixed. One small difference seen: Material's selected folder name draws heavier in Rust (Slint's 600) than in QML (DemiBold rendered as the family's 500): left as the QML asks |
