# shell/modules/settings/pages/appearance.qml ↔ surfaces/settings (pages/look/appearance.rs `appearance`, ui/settings-look.slint look-cards / look-thumbs / menu-icons / emoji)

Page id `appearance`. The look previews are shell/common/LookPreview.qml ↔
settings-look.slint `LookPreview`. The feed is `cx.feed["look"]`: in the
standalone shell it comes from host/shell/src/bridges.rs `look_feed`, under
the QML shell from RustBridgeLook.qml.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Choosing a look: nothing when it's the current one; else sets appearance.look and runs switch-look.sh detached (17–22) | appearance.rs `choose_look` → host bridges.rs `switchLook` | same |
| 2 | Material: two large cards, Glass then Material, 24 apart, 6 top padding; each preview (page.width−24)/2−12 wide, 10:16, x 6 (25–51) | settings-look.slint LookBlock `look-cards` | same |
| 3 | Card label under it: check-circle 20 px primary when chosen + "Material"/"Glass" 16 px, primary/600 when chosen else onSurface/500; label clickable, pointing hand (52–74) | look-cards label row | same |
| 4 | Text under the cards (16 px side padding, 14 px, onSurfaceVariant): "Material (experimental), after Google's Material 3 Expressive: tonal colour…apps keep running." (78–86) | look-cards `b.text` | same |
| 5 | LookPreview: hover ring 0.35 / selected 1 (150 ms), press scale 0.98 (100 ms OutCubic), Space/Return/Enter chooses, pointing hand, focus ring (LookPreview.qml 39–57, 32–37, 444–453) | settings-look.slint LookPreview | same |
| 6 | LookPreview's clock updates every 30 s (LookPreview.qml 437–442) | `time_short` at page build (`b.footer` / `r.text`) | same at a glance (the page is built again on service updates; check after a minute change) |
| 7 | Glass: "Look" row with two 128×80 thumbnails, 14 apart, caption names (600 when chosen) clickable; only when material/shell.qml exists beside the shell (96–138) | `look-thumbs` ctl; `crate::util::asset("material/shell.qml")` | same |
| 8 | Look row detail by look (101) | appearance.rs 102–106 | same |
| 9 | Appearance: Segmented 260 Light/Dark/Auto → appearance.mode; detail "Light from sunrise (h) to sunset (h)" only in Auto, TimeFormat.hm (139–149) | 116–125, `hm` | same |
| 10 | 24-hour time switch, detail "Clocks, events and alarms show 17:30 / 5:30 PM" (150–157) | 127–128 | same |
| 11 | App icons: Material 400 wide Default/Themed/Dark/Clear/Tinted/Auto, Glass 340 wide without Themed; "look" shows Themed (Material) / Clear (Glass); details per style (158–169) | 130–147 | same |
| 12 | Material: "Colours from" 9 swatches 22 px, 8 apart, right to left (wallpaper rightmost, image icon 11 px white), ring 3 px label colour; detail "Your wallpaper, like Material You" / "This colour" (170–207) | 149–163, SSwatch | same (wallpaper swatch colour: QML Wallpaper.seed; standalone feed sends `seed` = the tone: check it's the same colour) |
| 13 | Material: Colour style Segmented 340 Tonal/Vibrant/Expressive/Neutral + details (208–219) | 165–177 | same |
| 14 | Material: Transparency slider 220, 0–1, shows 0 while reduceTransparency; moving rounds to 0.05 and turns reduceTransparency off when > 0; 3 details (220–237) | 179–195 | same |
| 15 | Glass: Accent colour 9 swatches right to left (wallpaper = Wallpaper.accent, image icon), ring white (dark) / 50 % black; detail "Following the wallpaper" (238–273) | 196–212 | same |
| 16 | SubpageLink "Glass" (Glass only): drop-half, detail, value Glass/Frost/Solid/Custom → appearance/glass (278–285) | 218–229 | same |
| 17 | SubpageLink "Match apps" (Material only): paint-brush, detail → appearance/glass (286–292) | 230 | same |
| 18 | Accessibility: Reduce transparency (Glass only), Reduce motion, Text contrast Segmented 280 (writes contrast, clears increaseContrast; current = Theme.contrastLevel) (295–329) | 232–250 | same |
| 19 | More options (MoreOptions) folding the rest (332) | `p.more` / `p.end_more` | same |
| 20 | Icon contrast slider 220, 0–1, rounded to 0.01, only for Clear/Tinted (335–346) | 279–283 | same |
| 21 | Emoji: detail "In the emoji picker (Super+.) and Search" or "<package> isn't installed; showing Noto until it is"; Install font button (Material 40 px tonal, Glass 28 px fill, hover) only when missing → EmojiFont.install; Segmented 300 Noto/Apple/Fluent → emoji.style (347–382) | 258–284, `emoji` ctl | **different (standalone)**: `look_feed` has no `emojiAvailable`/`emojiPackage`, so the Install button never shows and the detail never says it's missing |
| 22 | Menu bar and shortcuts footer, "; Auto follows the icon…" only for Auto (384–387) | 333–337 | same |
| 23 | Menu icon: 18 choices 40×32, 4 apart, 8 a row; on = accentTint + 2 px accent border; hover fillHover; distro shows /etc/os-release LOGO from the icon theme; "tessera" also on for "circle"/"irice" (388–435) | MENU_ICONS, `menu-icons` ctl | same |
| 24 | Menu icon tooltip: the choice's name (Badge, <distro>, Linux…) after 600 ms hover (429–431) | — | **missing** (no tooltip anywhere in Settings) |
| 25 | Distro choice's name: SystemInfo.osName or "Your distro" (394) | `read_os_release` falls back to "Linux" | different (fallback text) |
| 26 | Icon theme: Dropdown 220 "Automatic" + installed themes (not hicolor, scanned each time the page loads); choosing a different one saves it and repairs the shell 800 ms later (436–465) | 309–342 (`cx.once`), host `repairShell` → `look.later 800` | same (list read once while Settings is open) |
| 27 | Shortcut symbols: Segmented 300 Auto/⌘ macOS/Windows/Linux, detail "e.g. " + KeyNames.combo("SUPER CTRL F") (466–476) | 329–343 | same |

## Test plan (both looks; Material window 1040×700 at 440,205, page column from x≈772)

`tools/settings-scenarios.sh`: `setup qml material <bin>` / `setup rust material <bin>`,
then the same with `glass`. Each step: `open appearance`, `shot`, then the clicks.
Read every y from a shot (the page scrolls); x of right-hand controls ≈ 1300–1450.

A1 Top (Material): `open appearance`, `shot`. PASS: two cards side by side, the
chosen one ringed in primary with a check-circle + bold primary name; the text
under them word for word as #4. Glass: the "Look" row with two 128×80 thumbnails
(if material/ exists), caption names.

A2 Preview hover/press: move the pointer onto the not-chosen card (Material: its
centre, ≈ x 1230, y from the shot), `shot`. PASS: a faint ring (35 %) round it.
Press and hold (`pointer down`), `shot`: the picture shrinks a little (0.98).
Release away from it (don't switch looks in the test).

A3 Appearance mode: click "Auto" in the Appearance segmented, `shot`. PASS:
detail "Light from sunrise (…) to sunset (…)" in the clock format; then Light:
detail gone. Toggle 24-hour time: detail flips "17:30" ↔ "5:30 PM", and the
sunrise times change format.

A4 App icons: Material shows 6 options (Themed second), Glass 5. Click each;
detail text matches #11 for each.

A5 Material colours: click a seed swatch (read x from shot; wallpaper swatch is
the rightmost). PASS: 3 px ring moves; detail "This colour"; clicking the
wallpaper one: "Your wallpaper, like Material You". Colour style: click each,
detail matches #13. Transparency: drag the slider to the middle. PASS: detail
"50%: panels, menus…"; to the left end: "Solid. Slide to…".

A6 Glass accent: click "red", then the wallpaper swatch (rightmost, image icon).
PASS: ring moves; detail "Following the wallpaper" only for the wallpaper one.

A7 Links: Glass: the "Glass" link shows value (Glass/Frost/Solid/Custom) and
opens the Glass page. Material: "Match apps" link opens the Match apps page.

A8 Accessibility: Glass shows Reduce transparency, Material doesn't. Text
contrast: click each; details as #18.

A9 More options: click "More options", `shot`. Icon contrast only when App icons
is Clear or Tinted. Emoji row: segmented Noto/Apple/Fluent; set emoji.style to
a font that isn't installed (e.g. Apple, when ttf-apple-emoji is missing).
PASS: detail "ttf-apple-emoji isn't installed; showing Noto until it is" and an
"Install font" button left of the segmented (Material 40 px tonal pill). Report
both shells (Rust standalone currently fails this: #21).

A10 Menu icon: hover a choice for 1 s, `shot`. PASS: a tooltip with its name
(e.g. "Rocket"). Click "Rocket": accent ring + tint. Distro choice shows the
distro logo (or a Linux logo).

A11 Icon theme: open the dropdown, `shot`: "Automatic" then the installed
themes. Don't pick one in the user's arena unless the sandbox is isolated (it
restarts the shell after 800 ms).

A12 Shortcut symbols: click "⌘ macOS": detail "e.g. ⌘⌃F" style; Auto: footer
gets "; Auto follows the icon…".

## Fixed in batch 2 (fixer)

- Menu icon choices: their name in a tooltip after 600 ms (kit `STip`).
- The distro's name falls back to "Your distro".
- Emoji "Install font": shown when fc-list doesn't find the style's font (EmojiFont.qml's test), when the shell doesn't say; looked for again after Install.

## Test results

(tester)
