# Parity: shell/modules/lock/LockSurface.qml

One screen of the lock: the wallpaper, the look's lock (LockGlass /
LockMaterial), StandBy over them, the lock and unlock transitions inside the
surface, the lock → StandBy morph, what wakes StandBy, the keyboard.
Rust: `surfaces/lock/ui/lock.slint` `LockScreen` (and `Bedside`), settings
in `src/lib.rs` `settings()`, the wallpaper in `after_frame` (`decode_pair`).

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Surface black under everything | 27 | `LockScreen` `background: black` | same |
| lock.material system → glass/frost/solid by the glass tier; darkInk by lock.text / wallpaper top | 24-25 | `settings()` (`plate`, `dark_ink`; WallTop) | same |
| StandBy only on screens in lock.standbyScreens (others dark) | 36, 157 | `bedside()` `standby_here` (`on_screen`) | same |
| `sb` 0→1: 650 ms morph / 400 ms fade, InOutCubic; none: instant | 41-46 | `animate sb` cubic-bezier(0.65,0,0.35,1) | same |
| morphOn: morph, here, clock view, lock clock morphable (Material: stacked, large) | 47-48 | `morph-on` / `morphable` | same |
| Unlock: `exit` 0→1 over leaveMs 220, OutCubic; wallpaper leans in 5 % (zoom) | 54-59, 71-81 | `animate exit` 220 ms (0.33,1,0.68,1); wallpaper `transform-scale` | same |
| Idle inhibitor while StandBy shows | 60-64 | `bedside()` `inhibit_idle` per surface | same |
| Frost: wallpaper blurred once (¼ size, blur, saturation) for the cards | 82-103 | `decode_pair` (¼ size, blur 8, saturation 1.2) | same at a glance (Slint has no live blur) |
| Lock entrance: zoom = wallpaper from 105 % over 450 ms OutCubic + reveal 0→1 (Theme.motion.large 450, emphasized); fade = reveal only; none = at once | 105-124 | `enter` / `reveal` (450 ms; Material (0.05,0.7,0.1,1), Glass (0.2,0.9,0.1,1)), started once the screen has shown and its wallpaper is decoded (`waiting` → `reveal()`; 800 ms at most, at once without a picture) | **fixed** (batch 1: it started on the first frame while the wallpaper still decoded, so the circle opened over black and the picture cut in after) |
| The look's lock (Glass or Material) | 126-148 | `if !Theme.material: GlassLock` / `MaterialLock` | same |
| StandBy keys: ←/→ change view; any other key wakes and focuses the field | 162-171 | `Bedside` `FocusScope key-pressed` → `step` / `woke`; the lock's `sb-now` change refocuses the input | same |
| StandBy mouse: move > 24 px (lock.standbyWakeOnMouse, only once fully in) wakes; click wakes; wheel steps | 172-193 | `Bedside` `TouchArea` (`check()`, `clicked`, `scroll-event`) | same |
| Glass morph: two copies of the clock fly from the lock's clock to StandBy's spot, bold one fading out (0.3→0.8), StandBy's weight fading in (0.2→0.75) | 197-234 | `if morphing && !Theme.material` block | same |
| Material morph: the lock's stacked clock flies, thinning weight, colours mixing, dimming | 235-262 | `if morphing && Theme.material` block (`StackClock`, weight in steps of 40) | same (no width axis: Slint sets only weight) |
| Typing keeps StandBy away | 263-267 | `set_password` → `woke` | same |
| Pointer moving anywhere on the lock (not StandBy) is someone there | 268-274 | `LockScreen` TouchArea `changed mouse-x` / **`mouse-y`** → `woke` (throttled 250 ms in Rust) | **fixed** (a purely vertical move didn't count) |

## Test plan (arena N, Glass and Material)

1. **Entrance.** For each `lock.lockTransition` zoom / fade / none: `ipc lock lock`, record 1 s. PASS: zoom — wallpaper scales 1.05 → 1 over ~450 ms while the content fades in and drops 20 px (Glass) / rises 32 px with the black circle opening (Material); fade — only opacity; none — all there on the first frame. anim-timing: the wallpaper is in the first frame the content starts moving (never black with the clock already in), start ≤ ~800 ms after the surface maps, ≈ 450 ms of change.
2. **Exit.** Unlock with `mock` + Return, record. PASS: 220 ms out-cubic fade; zoom: content grows 8 % (Glass) / top lifts & bottom drops 5 % of height (Material), wallpaper leans in 5 %.
3. **Pointer wakes.** Lock; `pointer move 960 300`; `pointer glide 960 800 600` (vertical only). PASS: in the log (`dev run …tessera-ui*.log`) no error; with `demo standby` first then `demo wake` … (behavioural check: Material's pill opens on the vertical glide — see LockMaterial).
4. **StandBy wake rules.** `demo standby`; wait 1 s (sb fully in). `pointer glide` 10 px → stays; glide 60 px → wakes (morph back, 650 ms). `demo standby`; `run wtype -k Right` → view changes (dots show), stays in StandBy; `run wtype a` → wakes, field focused, "a" not typed? (QML: the key that wakes isn't typed; check the same). `demo standby`; `pointer scroll` down → next view.
5. **standbyWakeOnMouse off** (`lock.standbyWakeOnMouse: false`): glide 200 px in StandBy → stays.
6. **standbyScreens**: with one screen only, set `lock.standbyScreens: ["NOPE"]` → StandBy is black (blank) but still wakes on a key.
7. **Morph** (lock.standbyTransition morph/fade/none): `demo standby`, record 1.2 s. PASS: morph — the big clock flies into StandBy's clock place (650 ms, in-out), no pop at the end; fade — 400 ms cross-fade; none — instant.

## Test results

(testers fill in)
