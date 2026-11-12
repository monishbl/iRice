# shell/modules/settings/pages/lock.qml ↔ surfaces/settings (src/pages/look/lock.rs `lock`, ui/settings-look.slint)

Lock & Login Screen (page `lock`). Material's clock carousel (ClockFaces.qml)
and the boot animation cards (BootAnimations.qml) have parity files of their
own; here only that they're placed. No Popup, no Timer.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Material only: Group "Clock" = ClockFaces carousel (Loader active only in Material), "Size" Segmented 300 Large/Small ↔ clockSize, "Thickness" slider 220 100–1000 step 20 → round(v) (21–49) | lock.rs 57–90 (`clock-faces` custom row, `choice`, slider) | same (carousel: see ClockFaces parity) |
| 2 | Untitled group: footer Material = per notifications value (shelf/column/icons), Glass = per layout (macos / else) (53–56) | 92–105 | same |
| 3 | "Layout" Segmented 300 Widgets/Classic ↔ ios/macos; detail Material only (two texts) (57–61) | 106–110 | same |
| 4 | "Notifications" Segmented 300 Newest/Beside/Icons ↔ shelf/column/icons; detail Glass only (three texts) (62–66) | 111–120 | same |
| 5 | "Show notification previews" switch + "Off shows only which app sent each notification" (67–71) | 121–123 | same |
| 6 | "Bedside Mode" group + footer, switch lock.standby (74–82) | 125–129 | same |
| 7 | "Login screen": footer by LoginScreen.installed / status / default; "Style" Segmented Your look/Text/Plain ↔ look/text/plain + detail per value; "Preview" + "Shows it on this screen…" + SettingsButton "Preview Login Screen" → LoginScreen.preview() (86–104) | 131–152 (`feed.login`, ask previewLogin) | same |
| 8 | "Starting up": BootSplash.refresh() each time the page is made; footer chain error / busy / blocked / done / no splash / design description … + " Changing it asks for your password." (108–117) | 154–159 (`local look.bootRefreshed`, cleared on page change) + `boot_footer` (tested) | same |
| 9 | "Boot animation" row + "What shows while your computer starts and shuts down", then BootAnimations cards (118–122) | 216–217 (`boot` custom row) | same (cards: BootAnimations parity) |
| 10 | "Login intro": detail reduceMotion / Material / Glass texts; "Play Intro" button (disabled with Reduce motion) → LockFx.playIntro(), then a switch lock.loginIntro, 12 px apart (123–143) | 193–218, settings-look.slint 1144 `intro` | same |
| 11 | Untitled group: "Preview" + "Locks the screen now", "Lock Screen" tonal button (Material 40 px +32, Glass 28 px +28, radius control, Medium) → System.power("lock") (146–160) | 220–221 `button("Lock Screen")` → ask look lock → `lock.lock` | same at a glance (Glass label + 24 instead of + 28; Rust adds pointing hand) |
| 12 | More › "Cards and field" (Glass only) Segmented 300 Glass/Frost/Solid/System ↔ lock.material + detail; "Text" 300 Light/Dark/Automatic + detail (163–177) | 226–230 | same |
| 13 | More › Bedside Mode: "Prevent burn-in", "Wake with the mouse" switches, "Clock brightness" slider 0.2–1 step 0.05 with "N%", "Clock weight" Segmented 240 Thin/Regular/Bold; all disabled while standby is off (178–211) | 232–243 `.enabled(standby)` | same |
| 14 | More › Transitions: "Unlocking" (zoom/fade/none, details), "Locking" (zoom/fade/none, details), "Into Bedside Mode" (morph/fade/none, detail for morph; disabled without standby), each 240 (212–231) | 245–261 | same |
| 15 | More › "Bedside Mode on displays" (only with > 1 screen, dimmed without standby, footer): "Same clock position" switch; a switch per screen labelled "model  ·  name" or name, detail "W × H"; empty list = all; turning every one on → [] (234–272) | 263–291 `standby_screens` (tested) | same (without the shell's feed: names only, no size) |
| 16 | More › Material only "Clock": "Width" slider 25–151 step 1, "Digits" Segmented 200 Rounded/Sharp ↔ clockRound; both hidden for the analog face (273–299) | 293–302 | same |

## Test plan

Setups as in pages_menubar (both shells, both looks). `open lock`; read y from a shot.

L1 Page: shot from the top, scroll (`run 'P move 1100 600 scroll 5'`) and shot to
the end, with More options open. PASS: Material starts with "Clock" (carousel,
Size, Thickness) and has the Clock width/digits group in More; Glass has
neither but has "Cards and field". Same texts, footers, order as QML.

L2 Details follow the choice: click Notifications "Beside" then "Icons".
PASS: Glass – the row detail changes; Material – the group footer changes.
Layout "Classic": Glass footer "The clock up top…", Material detail "The clock and the date, nothing under them".

L3 Bedside off: switch "Bedside Mode" off, open More options. PASS: Prevent
burn-in, Wake with the mouse, Clock brightness, Clock weight and "Into Bedside
Mode" are greyed and don't react; labels stay as the QML shows them.

L4 Login intro: turn Accessibility › Reduce motion on, back to lock. PASS: detail
"Off while Reduce motion is on (Accessibility)", Play Intro greyed, switch still usable.

L5 Buttons (watch only, don't lock in a recording you need): "Preview Login
Screen" opens the login preview (Esc closes); "Lock Screen" locks the arena
(`run 'ipc lock unlock'` after).

L6 Material digits: choose the analog face in the carousel, open More options.
PASS: Width and Digits rows hidden.

## Test results

(tester)
