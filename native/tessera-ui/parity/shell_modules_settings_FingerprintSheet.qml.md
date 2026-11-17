# shell/modules/settings/FingerprintSheet.qml ↔ surfaces/settings (src/pages/system/security.rs `sheet`, ui/settings-system.slint `FpSheet`)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Adding a finger: in QML a modal **layer-shell overlay window** over the whole
focused screen (namespace tessera:fingerprint, exclusive keyboard). In Rust
the sheet replaces the Security page inside Settings (local `fp.sheet`).
Progress state: RustBridgeSystem.qml `sheet` {takes, misses, retry, flash}.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | open(f): finger, shown, Fingerprint.enroll(f); close(): cancelEnroll if enrolling, hide (25–34) | security.rs 266–269 (Add), 146–151 (Cancel/Done) | same |
| 2 | taken = done ? 11 : stages>0 ? min(10, floor(11·count/stages)) : min(10, count); target = −1 when done/failed; edges = target ≥ 5 (37–47) | art.rs 144–154, security.rs 103–105 | same |
| 3 | Title: "Your <finger> is added" / "That didn't work" / "Now the edges" / "Add your <finger>"; Material 24 regular, Glass 20 DemiBold, centred (144–153) | sheet_texts 58–93; FpSheet 85–91 | same |
| 4 | Text (40 px high, wrapped, 14 px): done text / enrollError / retry text / "Getting the reader ready…" / step[target]; red while a retry shows (not done/failed) else secondary (154–166) | sheet_texts; FpSheet 92–99 (`b.dim`) | same |
| 5 | Retry text from Fingerprint.hints (or "Try that one again.") for 2200 ms (62–68, 97–99) | RustBridgeSystem.qml 70, 79–80 | same |
| 6 | Art 200×248 (FingerprintArt: own parity file) (168–176) | FpSheet 100–148 | see FingerprintArt |
| 7 | 11 dots, 6 apart, 4 top padding: target 16 wide else 6, h 6 r 3; success when done / accent taken / accent 50 % target / toggleOff; width and colour animate Theme.motion.hover (179–195) | FpSheet 150–161 (SM.hover) | same |
| 8 | Caption 12 px tertiary tnum: "All 11 sections recorded" / "" / "Section N of 11 · the edges/the centre" (196–202) | security.rs 110–116, FpSheet 162–167 | same (no tnum) |
| 9 | Buttons right-aligned, 8 apart: "Add Another Finger" (done, another free finger) → addAnother; "Try Again" (failed) → enroll(finger); "Done" (primary)/"Cancel" → close (205–223) | security.rs 126–153, FpSheet 169–178 | same |
| 10 | Card 420 wide, height body+48, radius Material 28 / Glass 20, colour surfaceContainerHigh / #232327 / white, Glass 1 px separator border (124–136) | FpSheet 74–81 | same |
| 11 | Scrim over the **whole screen**: black 0.5 (dark) / 0.3, blocks clicks, doesn't dismiss (113–117) | FpSheet 66–68: scrim only over the page area (radius 16) | **intentional: in-window sheet (the page area), as designed** [12 Nov: different (in-window; Settings' sidebar still usable)] |
| 12 | Open/close animation: progress 0→1 over Theme.motion.panel OutCubic (close 0.66× InCubic): scrim alpha, card opacity, card scale 0.96→1; window kept 400 ms to fade out (70–82, 115, 134–135) | — | **same (ported 17 Nov, B5: open 300 ms OutCubic (scrim, opacity, scale 0.96→1) was there; Cancel/Done now fades and shrinks it away in 200 ms InCubic before the page comes back (security.rs fp.closing, settings-system.slint FpSheet `closing`))** [12 Nov: **missing**: FpSheet appears/disappears at once] |
| 13 | **Esc closes the sheet** (cancels the enrolment) (118–122) | — (module doc: Esc closes Settings) | **same (fixed before 17 Nov: FpSheet FocusScope: Esc = its last button (Cancel/Done))** [12 Nov: **different**: Esc closes the whole Settings window and leaves fprintd enrolling until the page is rebuilt] |
| 14 | Keyboard exclusive while shown; the window on the focused monitor (83–89) | — | **intentional: in-window, by design** [12 Nov: different (by design: in-window)] |

## Test plan

Not testable in the arena without a fingerprint reader, and testers must
**never enrol a finger**. With a reader only: Settings › Privacy & Security ›
Add (QML and Rust), then immediately **Cancel** (no touch on the reader).

F1 Open the sheet (Add), `shot sheet-open` within 100 ms and again after 500 ms.
PASS (QML): the first shot shows the card part-faded/scaled; Rust: note.

F2 Look: title "Add your <finger>", text "Getting the reader ready…" then
step 1, dots (first one 16 px wide), caption "Section 1 of 11 · the centre",
a Cancel button. Same in both.

F3 Esc: press Esc with the sheet open. PASS (QML): sheet closes, Settings stays,
fprintd stops (log). Rust: write what happens (expected now: Settings closes).

F4 Cancel: click Cancel. PASS: sheet gone, Security page back, no finger added.

Code review only: #5 retry 2200 ms, #9 button set.

## Fixed in batch 3 (fixer)

- Esc is Cancel/Done (the sheet's last button), not Settings' close; the sheet fades in and grows from 0.96 (300 ms OutCubic).
- Not done: its fade out (it's made and dropped with the page's block).

## Test results

(tester)
