# shell/modules/overlay/Overlay.qml → surfaces/overlay (src/lib.rs, model.rs, ui/overlay.slint)

Springs: springFast 12.8/0.54 → 222 ms spring(0.4); springPress 7.5/0.3 → 290 ms spring(0.57) (Qt steps 16 ms: ω = √(62.5·spring), ζ = 62.5·damping / 2ω; Slint d = 2π/ω, bounce = 1 − ζ).

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Full-screen overlay; scrim (Material m3 scrim 32 %, Glass black 30 %) fades on popover; click on it or Esc closes | 144-160 | scrim, `close` | same |
| 2 | Crosshair at the screen centre (size/thickness/gap/colour/dot), shown while gaming or open | 162-190 | `OvCross` | same |
| 3 | Toolbar glass pill centred at y 44: one toggle per widget (Material 48 px, 8 apart; Glass 40 px, 4 apart) | 191-260 | toolbar `for tool` | same |
| 4 | Toolbar toggle (Material): corners 8 pressed / 16 on / round, on springFast; press swell (ElasticControl) | 228-236 | `animate border-radius`, `transform-scale` | **was** 250 ms spring(0.6) corners, fixed 1.06 on 250 ms spring(0.35); now 222 ms spring(0.4), 1 + min(6 %, 10 px / size) on 290 ms spring(0.57) |
| 5 | Recorder pill (Record / Stop): Material corners 8 pressed on springFast, press swell; Glass accent / danger | 369-392 | `Pill` | **was** 250 ms spring(0.6), 1.03 on spring(0.35); now springFast + springPress swell |
| 6 | Switches (Switch.qml): Material knob 16 → 24 on → 28 pressed, x and size on springFast; Glass knob x on popover, emphasized | Switch.qml 39-84 | `Toggle` knob | **was** 250 ms spring(0.6) in both looks; now Material 222 ms spring(0.4), Glass `Theme.popover` cubic-bezier(0.2, 0.9, 0.1, 1) |
| 7 | Mixer sliders, one per stream, follow the sound server | 420-470 | `sliders`, `data(Sound)` | **was** a new model on every sound change (a slider being dragged was rebuilt under the pointer); now one `Rows`, set in place |
| 8 | Toolbar toggles' list | 222-260 | `tools` | **was** a new model per push; now one `Rows` |
| 9 | Crosshair editor: 6 swatches, Size / Thickness / Gap sliders, dot switch | 497-550 | crosshair widget | same |
| 10 | FPS widget: limits 30/60/90/120/off | 555-590 | fps widget | same |

## Test plan (tester)
Both looks. Open: `inbox 'ipc panels open overlay TESSERA-TEST'` (or the overlay hotkey).
1. Shot: scrim (32 % / 30 %), toolbar pill centred at y 44; toggles 48 px (Material) / 40 px (Glass).
2. Material: `P glide` onto the first toggle, `P down` → corners square to 8 and the button swells ≈ 6 % (`rec ov-press 0.6 "P down"`); `P up` → toggles the widget, corners 16 when on. Settles in ≈ 0.22 s; compare with QML ± 1 frame.
3. Open the mixer widget; start audio in the arena (`paplay` a file, if available, else mark not testable here); `P down` on a stream slider and drag it while the stream's level changes: the knob stays under the pointer (before: dropped).
4. Any switch (e.g. crosshair dot): Material knob slides on the fast spring with a slight overshoot; Glass slides in 220 ms without overshoot.
5. Recorder widget: Record pill, `P down` → Material corners 8 + swell. Don't start a recording unless the arena allows it.
6. `-k Escape` → closes; click the scrim → closes.
