# shell/modules/menubar/IslandEvent.qml

One alert in the open island: an animated glyph with title, detail, battery
rings and pill buttons; the HUD's level row (with IslandLevels under it); a
ringing call (IslandCall); or an iOS-style notification card. No Material
twin (Material is inline). Rust: `surfaces/menubar/ui/island.slint`
`AlertView` (536-883), `Pill` (480-509), data from `surfaces/menubar/src/lib.rs`
`alert_rows` (422-493), handlers `handle` (1226-1273), state in
`src/island.rs`. Line numbers as of this audit (island.slint and lib.rs are
being edited concurrently).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Height per layout: HUD 34 (+ levels), ringing = call card, card = its column, row 52 (60 with a picture) + more row | 54-55 | island.slint:545-547 | different: no levels or ringing branch; more row a fixed 36 px |
| Intro: wait `delay`, glyph.play(), `appear` 0→1 320 ms OutCubic and the stagger clock `t` 0→1 700 ms | 57-85 | island.slint:546-549 (opacity 320 ms ease-out after `a.delay`) | different: opacity only, no stagger clock |
| Slides down 10 px as it appears | 117 | — | missing |
| A new uid replays the intro, an update in place doesn't | 62-76 | lib.rs `alert_rows.set_keyed(.., uid)` (~693) | same |
| Given more time while leaving: leave is cancelled | 65-69 | src/island.rs:165-177 `replace` (leaving = false) | same |
| leave(): `gone` 0→1, 140 ms × CoolDown.motionScale, InCubic, slides up 18 px and fades, then dismissRequested | 86-107, 116-117 | island.slint:548-549 (fade 140 ms ease-in) | different: fade only, no slide, no motionScale |
| Island.expiring(uid) → leave | 119-126 | src/island.rs:264-276 `sweep` sets `leaving` | same (fade only) |
| HoverHandler (shows the close button) | 128 | — | missing |
| DragHandler: drag up follows (`pull` ≤ 0, opacity ≥ 0.3), past −24 px or −400 px/s leaves, else springs back (popover ms, OutCubic); off for HUD and ringing | 129-147, 108-115 | — | missing |
| WheelHandler: sums "up" (inverted for natural scrolling) within 400 ms, ≥ 120 leaves; off for HUD, ringing, and while the list scrolls | 148-166 | island.slint:770-776 | different: any delta-y > 0 dismisses at once; no inverted handling; not off for a ringing call (scrolling removes the call without Decline) |
| HUD value bound live to Brightness / Audio / mic | 181-182 | lib.rs:485 `hud_value` = posted value | different: follows only reposts (host `hud()` reposts on change) |
| setHud → Brightness.set / Audio.setMicVolume / setVolume | 183-191 | lib.rs:1256-1263 (brightnessctl, wpctl -l 1.0) | same |
| HUD icon from the live mute state | 197-204 | island.slint:559-567 (`a.icon`) | different: the posted icon |
| Icon click mutes (mic or output; off for brightness), margins −6, pointer cursor | 205-211 | island.slint:569-574, lib.rs:1226-1231 | same (30 px area; pointer cursor also over brightness) |
| Device name after a switch, width min(text, 40 %) | 214-224 | island.slint:587-597 | same; bar start uses min(40 %, 140 px), not the text width: gap differs for short names |
| Glass bar: 8 px, white 0.16 track, ink fill, width SpringAnimation spring 5 damping 0.4, off while pressed | 225-241 | island.slint:598-611 (`spring(0.3)` 260 ms) | different: spring always on (lags behind a drag) |
| Bar press / drag sets the level, hit area 10 px around | 242-253 | island.slint:612-617 | different: hit area only the 8 px bar |
| Wheel over the bar ±0.05 | 254-257 | island.slint:618-621 | same |
| Material: M3 tall Slider with accessible name (Volume / Microphone volume / Brightness) | 259-272 | island.slint:602 (16 px track, no handle) | different: no handle, no accessible name |
| Levels button 28 px: hover 0.12 / press 0.2 (150 ms colour), caret-down ↔ caret-up bold 13, a11y Outputs / Inputs / Displays; toggles IslandLevels inline | 273-297 | island.slint:577-586, lib.rs:1233-1241 | different: opens the shell's Sound menu (or Control Center for displays); caret never flips; label always "Outputs"; no colour animation |
| IslandLevels under the HUD (resets to its list when hidden) | 299-306 | — | missing (see IslandLevels.qml.md) |
| More buttons: right-to-left Flow that wraps, spacing 6, fades in at stagger 0.45; Glass 0.16/0.24/0.3, Material tonal with state layer; runs `m.act` | 309-343 | island.slint:779-790, host island.rs:876 `more` | different: one row, no wrap, no fade-in; Material tonal has no hover/press layer |
| Glyph (IslandGlyph) | 349-355 | island.slint:629-686 | see IslandGlyph.qml.md |
| Title 15 DemiBold; fades in and slides 14 px at stagger 0.1 | 359-368 | island.slint:692-698 | different: no stagger |
| Subtitle small, `subtitleColor` or ink2, tabular; stagger 0.25 | 369-379 | island.slint:699-704 | different: no stagger, no tabular figures |
| Text column: 26 px after an audio glyph (waves), else 14; 10 px before trailing | 357 | island.slint:687-689 | same |
| Trailing: rings + 8 px + pills; width Behavior 200 ms OutCubic; fades and scales 0.7→1 at stagger 0.4 | 384-396 | island.slint:706-769 | different: ring→pill gap 6 px; no width animation; no fade/scale |
| Battery ring 34 px, r 15, 3 px; red ≤ 20 %, else success (Material accent); number 11 DemiBold | 397-433 | island.slint:713-756, lib.rs:435-444 | same (static) |
| Ring sweep grows with the stagger | 424 | — | missing |
| Level arriving after the card is up: ring traces from empty (600 ms × motionScale OutCubic), number fades in | 406-412, 429 | — | missing |
| Earbuds: L / R / Case rings 28 px, 2.5 px, labels 10 Medium; trace in as they arrive | 434-483 | island.slint:713-756 | same (static); trace-in missing |
| Second action (outlined pill): Glass 0 / 0.1 / 0.16, border 0.2; Material outline 0.38 + state layer | 485-511 | island.slint:757-763 | same |
| Action (filled pill): Glass 0.16 / 0.24 / 0.3; Material primary + state layer | 512-537 | island.slint:764-768 | same |
| A button runs its action, then everything posted before it goes (`acted(uid, since)`) | 27-32 | lib.rs:1242-1247 → RustBridge islandAct / host island.rs:876-902 | same |
| Ringing call → IslandCall | 541-550 | — | missing: a ringing call draws as a plain alert row (bell, name, app) with no Answer / Decline (see IslandCall.qml.md) |
| Card headline 18 Medium, up to 3 lines, line height 1.1; fades / rises 8 px at 0.05 | 559-571 | island.slint:796-804 | different: max-height 66 px elide; no stagger |
| Glass card: squircle r 20, white 0.14, rim 0.22 (Material tonal 0.1, no rim); rises 18 px at 0.2 | 575-584 | island.slint:807-812 | different: plain rounded rect; no stagger |
| Card top highlight (white 0.08 → 0 gradient, upper half) | 585-595 | — | missing |
| Avatar: photo (`card.avatar`) or initials on #8E8E93; scales 0.6→1 at 0.3 | 605-631 | island.slint:818-830 | different: initials only (AlertRow has no avatar), no scale-in |
| Material avatar in a cookie9 (ShapedArt) on tertiaryContainer | 632-654 | island.slint:822 (circle) | different: circle, no photo |
| App badge 20 px, −3 px offset | 655-660 | island.slint:831-838 | same |
| Name 15 DemiBold / time small | 667-686 | island.slint:843-858 | same |
| Message 14, 3 lines, dim 0.85; fades at 0.35 | 687-697 | island.slint:859-866 | different: max-height 60 px; no fade-in |
| Card's own buttons (≤ 2), tonal, `m.invoke` | 699-732 | island.slint:868-878 (`btn i`) | different: `btn` runs only for host-desktop uids (host island.rs:882-887); RustBridge islandAct ignores it (QML-shell mode no-op); no Material state layer |
| Clicking the card runs onAction (pointer cursor when it has one) | 736-744 | — | missing |
| Close button: 20 px over the glyph's / card's top-left corner while hovered or focused, 150 ms OutCubic; #3A3A3C/#48484A/#5A5A5E; 28 px target; a11y "Dismiss alert" | 748-780 | — | missing |
| `unseen`: the call's pulse holds while nobody sees it | 24, 549 | — | n/a (no call card) |

## Summary (priority: what a person clicks or sees first)

1. Ringing call has no Answer / Decline (generic row); a wheel up removes it without declining.
2. No close button on hover, no swipe-up dismiss; wheel dismiss fires on the first tick and ignores natural scrolling.
3. Notification card: clicking it does nothing (onAction missing); its own buttons are a no-op in QML-shell mode; no avatar photo, no top highlight.
4. HUD: the outputs / inputs / displays list (IslandLevels) isn't inline (opens a menu instead); caret and label fixed; Material slider has no handle; bar hit area 8 px; spring lags while dragging.
5. Motion: no text stagger, no slide-in / slide-out, no ring sweep or trace-in, trailing width doesn't animate.
6. Small: more row doesn't wrap; ring→pill gap 6 vs 8; tonal buttons have no state layer; no tabular figures.

## Fixed in batch 1 (fixer, 7 Nov)

- **Every alert's buttons were dead**: the row's wheel TouchArea was its last child, laid over the whole row, and took every press from the action / action 2 Pills. It now wraps the row underneath (`row := TouchArea`), so the Pills get their clicks; its hover drives the close button.
- The close button (×, 20 px circle over the glyph's top-left, 28 px target, fades in 150 ms OutCubic on hover, not for a ringing call or while leaving) → `dismiss(uid)` → `island.remove`. (Cards: not yet.)
- Wheel / swipe up dismisses after a notch (accumulated up deltas, reset after 400 ms or a down delta), not on the first tick. (Slint reports no "inverted" flag: natural-scrolling touchpads dismiss with the wheel direction "up", i.e. fingers down.)
- Nested lists (rings, more buttons, card buttons) are kept per alert uid and the alert list keyed by uid: a refresh (a temperature update each second) no longer remakes rows under the pointer.
- Colours named ("danger", "orange", rgba(…)) are read.

## Test plan (Sonnet tester)

1. `ipc island demo download`: hover the alert — the × appears at its glyph's top-left; click "Show in Folder" (outlined): it runs (a file manager opens or the host logs it) and the alert leaves. `ipc island demo download` again, click "Open": likewise.
2. `ipc island demo connect`, click "Connect": the alert changes in place to "Connecting… keep the case open nearby".
3. `notify-send -A ok=OK -A later=Later "Parcel" "Your earbuds ship today"` (card in the island when notifications show there): click OK: the action fires (`notify-send` prints `ok` when run with `--wait`) and the card leaves.
4. `ipc island demo usb`: click × → it leaves. Again: one wheel notch up over it → it leaves; a single small touchpad tick does not.
5. Running hot: see shell_services_Island.qml.md "Test plan: Running hot".
6. Both looks.

## Test results

## Fixed in batch 2 (fixer, 7 Nov)

- **Esc dismisses the newest alert** (IslandView.dismissNewest): a press on the island while it shows alerts gives the bar the keyboard (Exclusive, MenuBar.qml's `wantsKeys`) until the pointer leaves the island; Esc then removes the newest alert (a ringing call: declines it). Only resting the pointer never takes the keyboard.
- **A notification card's close button**: × over the card's top-left corner while the card is hovered (20 px, 150 ms fade), → removes it. (While the pointer is over one of the card's own buttons the × hides: the hover area is under the card.)
- The ringing call is its own card (see IslandCall.qml.md).

## Test plan (Sonnet tester)

1. `ipc island demo download`; click on the alert's title (not a button); `dev run wtype -k Escape`: the alert leaves. Again, but without clicking (only hover): Escape does nothing to the island (the focused window gets it).
2. Two alerts (`ipc island demo usb; ipc island demo wifi`): click one, Escape twice: newest first, then the other.
3. A notification card: `notify-send -a Files "Copied" "3 files to Documents"` (when notifications show in the island as cards); hover it: × at the card's top-left; click: it goes.

## Test results

## Fixes after testing (fixer, 7 Nov, batch 3 build)

- A tiny touchpad tick dismissed (batch 1 FAIL 7): now a wheel notch (15 px in one event) or 60 px of a swipe within 400 ms; a few-pixel tick doesn't.

Retest the FAIL steps above.
