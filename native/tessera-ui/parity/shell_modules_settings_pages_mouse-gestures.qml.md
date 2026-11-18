# shell/modules/settings/pages/mouse-gestures.qml ↔ surfaces/settings (src/pages/input/gestures.rs, ui/settings-input.slint `GestureRow`, `addbar`, `buttons`)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Gestures (page `mouse/gestures`, title "Gestures"). The gesture editor: each
row is [fingers] [direction] → [action] as three Choice controls (Choice.qml →
the page's ActionMenu, Rust: `cx.menu` per choice, the window's xdg_popup
menu) plus a "…" IconButton opening the row's ActionMenu. A draft group for a
new gesture. One 8 s undo Timer. ImportExport block at the end (its own
parity file).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | pageTitle "Gestures" (14) | gestures.rs 78 | same |
| 2 | Undo: offerUndo(text, snap) shows the Undo group, Timer 8000 ms restart → hidden (16–25, 184–199) | `offer_undo`/`undo_text` (input/mod.rs 72–88: a spawned 8 s sleep per offer, a counter so only the last counts); snapshot in the bridge (RustBridgeInput 59–63, 110–129) | same |
| 3 | Undo row: label = the text, SettingsButton "Undo" icon arrow-counter-clockwise → Keymap.restore(snap), hides, timer stops (187–197) | 106–112 `.buttons([btn("Undo").icon(..)])`, ask `undo` | same |
| 4 | Choice options: fingers "3/4/5 fingers"; directions Keymap.directions labels; actions Keymap.gestureActions + shellActions ({shell: name}); a hold (long press) gets shell actions only (27–34) | 15–22 `actions_for`, keymap.rs DIRECTIONS/GESTURE_ACTIONS/SHELL_ACTIONS (tested) | same |
| 5 | freeSpot: first of fingers 3,4,5 × left,right,up,down,pinchin,pinchout with no conflict; else 5 up (37–43) | 25–34 (tested) | same |
| 6 | Row menu (ActionMenu at the "…" button): "Turn on"/"Turn off" (icon power) → disableGesture + undo "Turned on/off <text>"; custom → "Remove" (trash, danger) + undo "Removed <text>"; default → "Reset to default" (arrow-counter-clockwise, enabled only when changed) + undo "<label> is back to its default" (45–73) | 183–203 `cx.menu`, asks gDisable/gRemove/gReset | same |
| 7 | GestureRow layout: left 20 (M) / 14 (G), height col + 24/20, Flow spacing 8 with Choice widths 120/160 (M), 104/136 (G), arrow-right 14 tertiary centred, action Choice 200; its placeholder = describeGesture(action) when not in the list (76–121) | settings-input.slint 213–268; `action_label` 42–45 | same |
| 8 | Disabled gesture: the choices' Flow at opacity 0.5, **still usable** (92) | slint 229 `opacity: r.enabled ? 1 : 0.5` (SChoice gets no enabled flag) | same |
| 9 | Line under the choices: "Off" / hold-with-Hyprland-action "A long press runs Tessera actions only. Choose one on the right." / "Clashes with X (Y). Change the fingers or direction." / "Changed · was N fingers, dir → action"; warning colour for clash/hold (not when off), else secondary; 14 (M) / caption (G) (122–130) | 163–180, 204 `.tone(4|1)`; slint 259–264 | same |
| 10 | "…" IconButton (28, ghost, dots-three, secondary) right 12 (M) / 8 (G), **tooltip "Options for this gesture"** (132–139) | slint `Dots` 85–109 (28 px, lit while its menu is open) | **same (fixed before 17 Nov: Dots: STip with its label)** [12 Nov: **missing**: no tooltip (only the accessible label); add `STip { text: label; hovered: ta.has-hover; }` to Dots] |
| 11 | Problems: a group per Keymap.userError / saveError: "Something's wrong with the gestures file", detail = error, warning-circle 20 warning (143–155) | 86–88 (`icon` ctl style 4 = 20 px warning) | same |
| 12 | Line above the list: "Swipe or pinch with several fingers. Changes apply right away." small secondary, left 16/4, elided; primary "Add gesture" (plus), disabled while the draft shows or userError (157–182) | 91–103 `addbar`; slint InputBlock 539–559 | same |
| 13 | Add gesture: draft = freeSpot, action {shell: missionControl}, shows (174–180) | 93–98 (local `gest.*`) | same |
| 14 | Draft group "New gesture": x 20/14, y 16/12, Column spacing 8: the three choices (same widths), clash line "Clashes with … Choose other fingers or another direction." (warning, **caption in both looks**), then buttons right-to-left: primary "Add gesture" (disabled on a clash), "Cancel" (202–255) | 114–140: `gesture` row (`value("draft")`, no id → padding 16/12 top, 8 bottom) + `buttons` row (padding top 8, bottom 16/12) | **same (ported 17 Nov, B5: the new gesture's clash line is caption-size in both looks (existing rows keep Row's detail size))** [12 Nov: **different (small)**: the gap between the choices (or the clash line) and the buttons is 16 px in Rust (8 bottom + 8 top) vs 8 in QML; the clash line is 14 px in Material (QML caption)] |
| 15 | Add: Keymap.addGesture({fingers, direction, action}), draft hides, undo "Added N fingers, <direction>" (244–249) | 128–136 ask `gAdd` | same |
| 16 | "Loading gestures…" (body, secondary, left 16/4) while !Keymap.ready (257–263) | 142–143 `note` block | same |
| 17 | Group "Gestures" + footer (“Left or right” … glass plugin.) while ready; "No gestures" row when the list is empty, detail by gestureDefaults (missing defaults file → path text) (265–282) | 144–209 | same |
| 18 | ImportExport sections ["gestures"], "Import and export gestures", while ready (284–289) | 210 `importexport::block` | same (see ImportExport parity) |
| 19 | The page is made anew each time it's opened (SettingsGlass/SettingsMaterial page Loader): leaving the page drops the draft and the undo (implicit) | local `gest.draft` / `gest.undo*` survive page switches (lib.rs 581 clears only `more:` and Wi-Fi keys) | **same (fixed before 17 Nov: gest.* in PAGE_TRANSIENT)** [12 Nov: **different**: Gestures → Mouse → Gestures still shows the draft (and an undo still within 8 s)] |

## Test plan

Setups as in pages_mouse (both shells, both looks). `open mouse/gestures`,
`shot g-top`. Material window 1040×700 at 440,205; page column from x≈772;
read each control's y from the shot.

G1 Page: PASS: line + Add gesture, group "Gestures" with every default
gesture, footer, Import and export; same order and texts in both shells.

G2 Choice menus: click a row's fingers choice; shot the open menu; pick
"5 fingers". PASS: config/keybinds gesture changes, the row shows "Changed ·
was 3 fingers, …" (or a clash line in warning colour). Repeat for direction
and action; for direction "Long press" the action menu has only Tessera
actions.

G3 Row menu: click "…" on a default gesture. Shot. PASS: "Turn off" (power) and
"Reset to default" (disabled unless changed). Turn off: choices at half
opacity, line "Off", Undo row "Turned off …" appears; wait 9 s: gone. Do it
again and click Undo within 8 s: gesture back on.

G4 Tooltip: hover the "…" 1.5 s and shot. PASS (QML): tooltip "Options for
this gesture". Rust must show it too.

G5 Draft: click Add gesture. PASS: group "New gesture" with a free spot and
"Mission Control"-type action, Add gesture (top bar) greyed. Shot and measure
the gap between the choices and the Cancel/Add buttons: QML 8 px, Rust must
match. Pick a clashing fingers/direction: clash line in warning, Add disabled.
Cancel: draft gone.

G6 Add: draft with a free spot → Add gesture. PASS: row appears in the list,
Undo "Added N fingers, up"; Undo removes it.

G7 Page switch: Add gesture (draft open), click the sidebar's Mouse & Trackpad,
then the Gestures link again. PASS (QML): no draft. Rust must match.

## Fixed in batch 3 (fixer)

- The "…" has its tooltip; the draft (gest.*) starts over with the page.

## Test results

(tester)
