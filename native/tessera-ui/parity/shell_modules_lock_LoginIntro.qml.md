# Parity: shell/modules/lock/LoginIntro.qml

The login intro: the first start in a session with lock.loginIntro on (not
Reduce motion), with the startup chime, 1.3 s (Glass) / 1.0 s (Material);
Settings' Play Intro and `ipc call loginIntro play` preview it.
Rust: when — host `bridges.rs` (`start` → `arm_intro`, `lock.sessionStarted`
→ `maybe_play`, `introHold` 900 ms, `introReveal`, `introDone`,
`preview_intro`, ipc `loginIntro play`); drawing — `surfaces/lock/src/intro.rs`
(`LoginIntro` windows in lock.slint).

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Armed on the first start in the session (no startedMark / old mark), not locked, setting on, no Reduce motion | 28-55 | bridges `start` (runtime-dir marks, `lock.loginIntro`, `appearance.reduceMotion`) | same |
| arm: LockFx.prepare(no zoom), cover up, 900 ms hold limit | 65-76 | `arm_intro` (`lockfx("prepare")`, `introHold` 900, tells lock `intro: arm`) | same |
| Plays when the chime has played (and Glass's picture is ready) | 77-80, 108-114 | `lock.sessionStarted` → `maybe_play`; the lock decodes its picture itself | same (QML too treats the picture as ready when tessera-ui draws) |
| Never held more than 900 ms | 128-133 | `timer("introHold")` | same |
| play: clock 0 → length × pace; reveal of the desktop at 150 (Material) / 350 (Glass) ms over 800 / 900 ms | 81-87, 137-151 | `play_intro` (`introReveal`, `introDone`), intro.rs 16 ms ticks; the clock starts once every cover is mapped and (Glass) has its picture, 900 ms after play at most (`asked`, `ready()`) | **fixed** (batch 1: started at "play", so Glass played over black while its picture decoded — 433 ms, no focus pull — and Material lost its first ~300 ms before the cover was up) |
| finish: cover down, LockFx rest unless locked | 88-99 | `finish_intro`, intro `stop` | same |
| Locking while it shows ends it | 121-127 | `set_locked` → `finish_intro` | same |
| Preview (Play Intro, ipc) whatever the setting | 100-106, 115-120, 169-174 | `preview_intro`; settings "playIntro" (skips with Reduce motion) | same |
| TESSERA_INTRO_PACE slows it | 136 | `Bridges::pace()`, intro `pace` | same |
| Overlay layer `tessera:lockveil`, no keyboard, clicks go through | 180-190 | intro.rs `window()` (Overlay, KeyboardInteractivity::None, empty input region) | same |
| Glass: black → wallpaper into focus: black over it 1→0 by 450 ms; blur 1→0 by 900 ms; scale 1.06→1 by 1000 ms; cover fades 650–1250 ms (out-cubic) | 192-231 | `glass_frame()`; blur as a cross-fade from the blurred copy | same timing; blur is a cross-fade (no live blur in Slint) |
| Material: surface cover with the mark (17.5 % of the short side) opening into a hole that grows (in-out 0–900 ms) to 1.05 × diagonal, shape cookie9 → softBurst (250) → circle (520) on a spring (ζ 0.6, k 200), spin 90°, ring thinning 200–900 ms, badge fading/shrinking by 250 ms | 233-320 | `material_shapes()`, `shape::cover_with_hole`, `shape::ring`, badge props | same (badge isn't cut by the hole: it has faded by then) |

## Test plan (arena N, both looks)

1. `ipc call loginIntro play` (or `IRICE_ARENA=N ./dev.sh ipc loginIntro play`); record 2 s from the command. PASS Glass: black → the wallpaper sharpening and settling from 106 % (≈1 s), then the cover fades (650–1250 ms) revealing the desktop whose widgets/dock settle in; total ≈ 1.3 s; clicks during it reach the desktop (`pointer click` on a desktop icon mid-intro selects it).
2. Same on Material: the cookie mark with the badge, opening into a growing window that morphs to a circle and spins 90°, gone by 1.0 s.
3. Slowed: restart the arena shell with `TESSERA_INTRO_PACE=10` in its environment if practical, record and compare frame timings against the QML at the same pace (anim-timing: Glass end ≈ 13 s).
4. With `appearance.reduceMotion: true`: Settings → Play Intro does nothing.
5. `ipc lock lock` while it plays → the intro is gone at once, the lock shows.

## Test results

(testers fill in)
