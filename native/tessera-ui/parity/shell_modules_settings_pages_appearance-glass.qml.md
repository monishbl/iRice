# shell/modules/settings/pages/appearance-glass.qml ↔ surfaces/settings (pages/look/appearance.rs `glass`, ui/settings-look.slint glass-material / glass-advanced / check)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Page id `appearance/glass` (title "Glass"; "Match apps" in Material).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Title "Match apps" in Material, "Glass" in Glass (13) | appearance.rs 382 | same |
| 2 | Material: row with info icon "Glass belongs to the Glass look", detail "Material's surfaces are solid colours…switch back.", button "Show looks" → page appearance (17–29) | 385–391 | same |
| 3 | Glass group footer: needs-plugin text when lens > 0.5 and no plugin; else per material (35) | 424–433 | same |
| 4 | Material Segmented: 230 wide Glass/Frost/Solid, 300 with Custom when custom; → appearance.glass (41–48) | 400–423, `glass-material` | same |
| 5 | "Advanced…" pill: label + 20, 28 tall, radius 14, small/500, fill → fillHover on hover or while the popover is open; pointing hand; vertically centred on the segmented (50–64) | `glass-material` ctl (Advanced… Rectangle) | same |
| 6 | **Advanced popover is a QQC.Popup** (65–73): opens on click (`advanced.open()`, a second click while open closes it by the press-outside then reopens it); placed under the button, `y = button.height + 8`, `x = button.width − 300` (right edges lined up); width 300, padding 16; non-modal; `margins: 12` keeps it inside the Settings window (moves up near the bottom); closes on Esc and on a press outside | `glass-advanced` block **inline under the Glass group** (`Advanced`, settings-look.slint 823–908), toggled by `look.glassAdvanced` | **same (fixed before 17 Nov: its own popover window under the button: right edges lined up, 8 px below, 300 wide)** [12 Nov: **different**: not a popup (pushes the rows below it down, sits after the Glass group's footer, not under the button); no Esc / click-outside close; a second click closes instead of reopening] |
| 7 | On open (onAboutToShow): copies the current amounts into glassLens/Frost/Solid/Tint (tint `a[3] ?? 0.45`) so the sliders don't jump (75–81) | 403–415 (`open` branch) | same |
| 8 | Background GlassPopupBackground, radius Theme.radius.control + 16 = 26 (Glass), the glass menu fill (dark rgba(.11,.11,.13,.55) / light rgba(.97,.97,.99,.62), solid when tier solid) (82) | Rectangle r 26, `Theme.menu-bg`, 1 px separator border, drop shadow 16 blur 4 y | same at a glance |
| 9 | Content scrolls when taller than the window: height = min(content, max(240, windowH − 2·12 − 2·16)), StopAtBounds (84–89) | none (inline, the page scrolls) | **same (fixed before 17 Nov: Advanced is its own popover window (settings-popover.slint): a Flickable with an indicator when taller than there's room for)** [12 Nov: **missing** (needed once it's a popup)] |
| 10 | Column spacing 14: "Custom material" body/600 label (93–97) | `Advanced` | same |
| 11 | 4 amount sliders, each 268 wide: name small/500 left, reading "NN%" small secondary tabular right (18 tall row), Slider, hint caption tertiary. Glass "How much it bends what's behind" 0–1; Frost "How much it blurs" 0–1; Solid "How opaque it is" 0–1; Tint "How much it darkens what's behind" (dark) / "How milky it is" (light) 0–2. Moving rounds to 0.01 and sets glass = custom (98–140) | ADVANCED[0..4], AdvancedSlider | same (Rust wraps hints; QML's amount hints don't wrap; reading not tabular figures) |
| 12 | "Optics" body/600 with 4 top padding (143–148) | padding-top 18 | same |
| 13 | Live preview 268×96: background #1C1C1E/#F2F2F5, 12 stripes (even ones accent 85 %), "Aa" 40 px bold centred; the real glass shader over it in a rect (48,18,w−96,h−36), radius 30, bevel 24, refraction = refraction·max(0.15, lens), chroma, rim, gloss ≥ 0.5 (150–204) | `Advanced` preview: stripes + "Aa" + a frosted rounded rect (no shader) | **intentional: the preview has no glass shader in a client window (the plugin draws glass in the compositor); stripes + Aa as the QML's backdrop** [12 Nov: different (Slint can't run the glass shader: a frosted rounded rect, so IOR/refraction/dispersion don't visibly change it)] |
| 14 | 3 optics sliders: Index of refraction 1–2 reading "1.50"; Refraction 0.4–2 "NN%"; Dispersion 0–1 "None" under 0.005 else "NN%"; hints wrap; moving rounds to 0.01 and does NOT switch to Custom (205–248) | ADVANCED[4..7], `advanced_value` | same |
| 15 | "Reset to Glass" SettingsButton with arrow-counter-clockwise: glass liquid, ior 1.5, refraction 1, dispersion 0.35, lens/frost/solid/tint = presets.liquid [1, 0.75, 0, 0.75] (249–263) | act 200 | same |
| 16 | Motion Segmented 300 Subtle/Snappy/Expressive + 3 details (270–280) | 434–451 | same |
| 17 | Window corners slider 220, 0–32 step 1, detail "N px, continuous curve" (281–292) | 437–452 | same |
| 18 | Rounded screen corners switch (293–299) | 439–453 | same |
| 19 | Match apps group: footer by availability / Off / source; "Colours from" Segmented 260 Off/Accent/Wallpaper, enabled only when AppTheme.available (302–316) | 495–530 | **same (appThemeAvailable falls back to Material \|\| matugen on PATH = AppTheme.available)** [12 Nov: **different (standalone)**: `available = cx.material \|\| feed.appThemeAvailable`, and host `look_feed` sends no `appThemeAvailable`, so in Glass the segmented is always disabled and the footer always "Install matugen…" even with matugen installed] |
| 20 | Target switches (not when Off): GTK apps / Qt and KDE apps / Terminals (foot, kitty, Alacritty) / Firefox (with Pywalfox) and Chromium; toggle adds/removes the id in appThemeTargets (317–331) | 511–525 | same |
| 21 | More options → Glass group: Lens Segmented 260 Thin/Depth/Auto, only when lens > 0.4 and solid < 0.9 (339–349) | 557–558 | same |
| 22 | Compositor glass (hidden when tier solid): detail outdated / active / not loaded; accent pill 28 "Build plugin…"/"Update plugin…" (darker on hover) runs build-plugin.sh --load in a terminal; check-circle 18 success when active (350–379) | 538–547, host `buildPlugin` | **same (fixed before 17 Nov: glassBuild/glassPlugin in look_feed; Build/Rebuild glass in-shell (M10))** [12 Nov: **different (standalone)**: `glassPluginVersion` isn't in `look_feed`, so "outdated" (versions 3, 4) never shows] |
| 23 | Tint glass with wallpaper colour switch (380–386) | 548–560 | same |
| 24 | Frost over windows (plugin and tier ≠ solid): Segmented 300 Off/Subtle/Medium/Strong = 0/0.5/0.75/0.95, current = nearest (387–405) | 550–565, `nearest` | same |

## Test plan (Glass look for 3–24; Material for 1–2, 19–20)

`setup qml glass <bin>` / `setup rust glass <bin>`, `open appearance/glass`, `shot`.
Read positions from the shot; the Advanced… pill sits right of the Material
segmented (≈ x 1400, y ≈ 330 at the top of the page: read it).

G1 Material row: click Glass/Frost/Solid; footer text changes as #3 for each.
With the plugin missing and Glass chosen: "Glass needs the tessera-glass…".

G2 Advanced opens as a popover: click "Advanced…", `shot`. PASS (QML): a 300 px
wide glass panel just under the pill (8 px gap), its right edge lined up with the
pill's, floating over the Motion/Window corners rows (they don't move); the pill
stays lit. Rust must look the same (today it's inline under the group: FAIL #6).

G3 Contents: in the open popover, `shot`. PASS: "Custom material"; Glass, Frost,
Solid, Tint sliders with "NN%" readings and hints (Tint hint "How much it darkens
what's behind" in dark, "How milky it is" in light); "Optics"; the stripes
preview with "Aa" centred; Index of refraction "1.50", Refraction "100%",
Dispersion "35%"; "Reset to Glass" button. The readings equal the material shown
before opening (sliders don't jump).

G4 Sliders: drag the Frost slider to ~30 %. PASS: reading follows, the Material
segmented gains "Custom" and selects it (300 wide); footer "Your own mix…".
Drag Dispersion to the left end: reading "None", Material stays as it was
(optics don't switch to Custom).

G5 Reset: click "Reset to Glass". PASS: Material = Glass, readings 100/75/0/75 %,
1.50, 100 %, 35 %; "Custom" leaves the segmented.

G6 Closing: open it, `key -k Escape`. PASS: popover closes, Settings stays open.
Open it, click on the page left of it (≈ 800, 600). PASS: closes. Click the pill
twice quickly: QML ends open (closes on press, reopens on click) — record both.

G7 Near the bottom / scrolling: move the Settings window so its bottom is near
the screen bottom, or make it 1040×500; open it. PASS: the popover stays inside
the window (12 px margin) and scrolls inside when taller.

G8 Motion: click each; details as #16. Window corners: drag; detail "N px,
continuous curve" with whole numbers. Rounded screen corners toggles.

G9 Match apps: with matugen installed (`which matugen` in the arena), "Colours
from" is enabled in both shells; click Accent: the 4 target switches appear;
footer "Apps get colours from your accent colour…"; Off: they go, footer "Apps
keep their own colours.". Without matugen: disabled + "Install matugen…".

G10 More options: Lens row only for Glass (not Frost/Solid); Compositor glass
shows "Active: Hyprland draws the glass" + green check with the plugin, else
"Build plugin…" accent pill; Frost over windows only with the plugin.

G11 Material look: `setup … material`, `open appearance/glass`. PASS: title
"Match apps", the info row + "Show looks" (click → Appearance page), then Match
apps group (enabled), More options.

## Fixed in batch 2 (fixer)

- **Advanced… is its own popup now** (settings-popover.slint, lib.rs `sync_popover`; the block is `Block::popover`): under the pill, right edges lined up, 8 px apart; 300 wide, as tall as the window has room for (window height − 56, 240–840) and scrolling inside; Esc or a click elsewhere closes it; the pill again closes it (a click that just closed it doesn't reopen it); another page closes it. The pill stays lit while open (r.on).
- "Colours from" (Match apps): available when matugen is on the PATH, when the shell doesn't say (`appThemeAvailable`).
- "Update plugin…": the standalone shell now sends `glassPluginVersion` (host/shell bridges.rs look_feed).
- Still different: the preview is a frosted rectangle, not the glass shader; the four amount hints wrap; readings have no tabular figures.

## Test results

(tester)
