# shell/modules/settings/pages/dock.qml ↔ surfaces/settings (src/pages/look/menubar.rs `dock`)

The Dock, shown as a part of Menu Bar & Dock (page `menubar`, part "Dock";
`open dock` lands there). No Popup, no Timer.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | "Show the dock" switch → dock.enabled; footer "The App Drawer stays on Super+Ctrl+Space, and windows use the space at the bottom." only while off (15–22) | menubar.rs 179–183 | same |
| 2 | The other groups (and More's) disabled + 0.45 opacity while the dock is off (26–27, 61–62, 92–93) | `.dim(!on)` on the size, Kept and Add groups | same for those three; **More options group is not dimmed in QML either** (126) → same |
| 3 | "Size": detail "N px" (Material: clamped 36–56), slider 220, Material 36–56 / Glass 32–72, step 2, value raw iconSize (28–40) | 186–197 | same |
| 4 | "Apps button" switch + "Opens the App Drawer (also Super+Ctrl+Space)" (41–45) | 198 | same |
| 5 | "Automatically hide" Segmented 260 Never/Over windows/Always ↔ off/windows/always, detail per value (46–56) | 199–206 | same |
| 6 | "Kept in Dock" (footer "Running apps that aren't kept appear after the divider."): a row per pinned id, label the app's name or the id + "Not installed" (65–72) | 208–228 | same |
| 7 | Remove button: round, Material 40 px tonal (secondaryContainer, 8 % layer on hover, minus 20 px), Glass 26 px fill/fillHover, bold minus 12 px; click removes the id (73–85) | `buttons(vec![btn("").icon("minus")])` → SButton round | same (Rust adds a pointing-hand cursor and press scale; QML has neither) |
| 8 | "Add to Dock": "Search apps" field 240, placeholder "App name"; with ≥ 2 characters up to 6 matching visible, not-pinned apps, each with "Add" (90–104) | 230–252 (`local dock.filter`) | same |
| 9 | Add button: accent fill pill, Material 40 px / Glass **26 px**, label + 32 / + 20, onAccent, 14 px Medium / small DemiBold; click appends the id (109–119) | `button("Add").primary()` | different (small): Glass SButton is 28 px tall and label + 24 |
| 10 | More › "Space between icons": slider 220, 0.05–0.4 continuous, written rounded to 0.01; detail round(iconSize × gap) px (128–138) | 280 | same |
| 11 | More › "App names in the App Drawer" (Material only) + "Off shows only the icons" (139–144) | 281 | same |
| 12 | More › "Recent apps" + detail, "Downloads", "Trash" switches (145–157) | 282–284 | same |
| 13 | More › "Magnification" (Glass only): one slider 1–2 step 0.05, left end (< 1.025) = off, else scale max(1.1, v) and on; detail "N% when you point at an icon" / "Off"; words Off / Large under it (160–189) | 267–290 `.words("Off","Large")` | same |
| 14 | The list updates live when the pinned list changes elsewhere (dock right-click "Remove") | rebuild on config change | same |

## Test plan

Setups as in pages_menubar (both shells, both looks). `open dock` (lands on
Menu Bar & Dock at the Dock heading). Read every y from a shot.

D1 Off: click "Show the dock" off. PASS: the footer line appears, the Size,
Kept in Dock and Add to Dock groups fade to 0.45 and don't respond to clicks;
on again restores.

D2 Size: Material: the slider runs 36–56, Glass 32–72; drag it from one end to
the other (`run 'P move X Y down; P move X2 Y; P up'`). PASS: detail goes by 2 px,
`dock.iconSize` saved even, same ends as QML.

D3 Remove/Add: note the first pinned app; click its minus. PASS: the row goes,
the dock loses the icon. Click "Search apps", type its name's first 3 letters
(`run 'key fir'`). PASS: up to 6 matches appear while typing (field keeps focus
and its text through every letter), the removed app among them; "Add" puts it
back at the end of Kept in Dock. Shot the Add button in Glass: QML 26 px tall.

D4 More: open More options. PASS: Material shows "App names in the App Drawer"
and no Magnification; Glass the opposite. Glass: drag Magnification to the far
left: detail "Off"; one step right: "110% when you point at an icon".

D5 Space between icons: drag; detail = round(size × gap) px, matching QML for
the same slider position.

## Fixed in batch 2 (fixer)

- Not fixed yet: Glass "Add"/"Clear all" are 28 px tall (QML 26).

## Fixed in batch 3 (fixer)

- Glass "Add" (and Notifications' "Clear all") are 26 px tall, 20 px wider than their label (SButton `compact`).

## Test results

(tester)
