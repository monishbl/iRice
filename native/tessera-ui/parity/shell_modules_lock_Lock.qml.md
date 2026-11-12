# Parity: shell/modules/lock/Lock.qml

The session lock's rules: when it's held, the unlock/lock transitions and the
veil, Bedside Mode's (StandBy) shared state and burn-in moves, the password,
the fingerprint reader and the face camera (three PAM conversations), the
faillock lockout text, and the `lock` IPC.

Rust, standalone: the host half is `host/shell/src/bridges.rs` (`set_locked`,
`set_held`, `lockfx`, `unveil`, `lock_event`, `idle_watches`/`idle_heard`, ipc
`lock …`); the lock half is `surfaces/lock/src/lib.rs` (`set_held`, `take`,
`release`, `rules`, `woke`, `submit`, `auth`, `demo_state`, `M::…` timers),
`auth.rs` (PAM in a child process), `logic.rs` (faillock, texts), `veil.rs`
(the veil).

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Locking takes the compositor lock at once (`held = true`), no animation first; LockFx rest | 88-96 | bridges `set_locked(true)` → `set_held` → lock `set_held(true)` → `take()` (ext-session-lock) | same |
| A restart while locked: held at start | 71 | bridges `start` sets `locked` false (standalone starts unlocked) | different: n/a standalone (the compositor keeps a dead client's lock; the watchdog restarts) |
| Typed password cleared on every lock change | 85-87 | lock `set_held` clears `password` | same |
| Unlock, transition "none": veil off, rest, let go at once | 102-108 | bridges `set_locked(false)` "none" path | same |
| Unlock, zoom/fade: desktop set back (LockFx.prepare), veil up, content lifts away 220 ms, then let go | 109-116, 119-127 | bridges: `veiled = true`, `lockfx("prepare")` (tells background + lock veil), `leaving`, `later("leave", 220)` | **fixed** (veil was missing) |
| The veil: the wallpaper in an Overlay layer (`tessera:lockveil`) over windows/dock/menu bar, scale = LockFx.wallpaperScale, opacity = LockFx.veil, no input | 158-181 | `surfaces/lock/src/veil.rs` (`LockVeil` window per screen, the lock's own decoded picture, empty input region, `frame()` = LockFx's curves) | **fixed** (was missing: windows/dock/bar popped in at once) |
| Reveal 150 ms after letting go (60 ms once the compositor says it's unlocked): LockFx.reveal(560 zoom / 360 fade) | 128-134, 150-154 | bridges `timer("reveal")`, `lock_event` secure false → `later("reveal", 60)` | same |
| Veil lowered once the desktop is at rest, or once the lock is secure | 136-142, 147-149 | veil drops itself at desktop = 1 (`Veil::tick`); bridges `lockfx("rest")` tells `veil: rest` | same; plus a 3 s safety (never left over the windows) |
| Locked and unlocked again before it showed: veil off, reveal(300) | 97-100 | bridges `unveil` + `lockfx("reveal", 300)` | same |
| 5 min idle while charging (lock.standby, not keepAwake) locks | 186-190 | bridges `idle_watches` (STANDBY idle 300 s) → `set_locked(true)` | same |
| Bedside Mode after 20 s alone on the lock (charging or `demo standby`) | 191-199 | lock `rules()` `t_idle` 20 s → `M::IdleOver` | same |
| `woke()`: back after StandBy or 30 s → tries back; face armed on first touch >1 s after lock; another face round 3 s after tries ran out | 205-235 | lock `woke()` (logic::came_back) | same |
| New lock: everything starts over (tries, face arming, idle) | 236-249 | lock `set_held` | same |
| Typing (password non-empty) is someone there | 250 | lock `set_password` → `woke` | same |
| `standby` = held, not leaving, can stand by, idle, nothing typed, not busy | 251 | lock `standby()` | same |
| Any input while locked (compositor idle notification, 0.5 s, inhibitors ignored) wakes the camera, unless in StandBy | 256-261 | bridges LOCK_WAKE idle → `{"wake":true}` → lock `told` | same |
| Burn-in: a new spot every minute (up/down, ≥0.5 away), 15 min → dark rest 1 min, moves while dark | 263-321 | lock `M::Second` minute change → `drift`, `t_rest` 60 s → `M::RestOver` | same |
| standbyShift changed → drift | 322-325 | `config_changed` → `drift` | same |
| endRest when StandBy is used | 327-333 | `M::Step` | same |
| Notifications list to the lock | 393-414 | host-desktop `today/notif.rs` → `{"notes":…}` | same |
| Fingerprint: conversation while held, 400 ms after, 30 s waits, 5 misses end it, quick "no match" (<700 ms) ignored, busy reader retried every 300 ms ≤50 times, "Not recognised" 1.6 s | 425-617 | lock `rules`, `M::FingerRetry`, `auth()` Finger arms, `M::FingerMissOver` | same (real only; sandbox: `demo finger/miss/match`) |
| Sleep: reader let go on PrepareForSleep(true), every try back on wake, face armed | 477-529 | `watch_sleep` → `M::Sleep` | same |
| Clock jump >10 s while locked: fresh reader | 530-552 | `M::Second` | same |
| Face: Howdy conversation, 3 tries, "Looking for your face…", "Face not recognised", broken → off until next lock, never in StandBy | 619-675 | lock face arms, `M::FaceRetry` | same |
| Password: PAM `login`; success unlocks; failure: "Incorrect password. Try again.", error sound, lockout check; PAM error: "Couldn't check your password. Try again."; field cleared | 677-703 | `submit`, `auth()` Password, `auth.rs` (System backend only when real) | same |
| Sandbox: never checks a password ("This is a sandbox: unlock it with ./dev.sh ipc lock unlock.") | 55-61 | sandbox → `Backend::Mock`, accepts "mock" (TESSERA_LOCK_TEST_PASSWORD overrides) | different on purpose: lets testers type a right and a wrong password; never PAM against the account |
| Faillock: conf + `faillock --user`, "Too many attempts: try again in about N minutes." | 705-764 | `check_lockout` (real only), `logic::lockout`, `logic::locked_text` | same (`demo lockout` shows the text) |
| Refused by the compositor: QML locks itself instead | 374-377, 416-423 | lock `LockEvent::Finished` while held → `M::Retake` after 1 s, 5 tries, then `refused` | **fixed** (standalone had no other locker: nothing was locked) |
| IPC `lock lock`, `lock unlock` (sandbox only), `lock demo …` (sandbox only) | 766-807 | bridges `ipc` | same; demo also `typing|wrong|busy|lockout|miss` |
| Fast typing: every key kept, in order, and Return checks what the field shows | (QML: the field's text is the password) | lock `set_password` / `field()`: the screen typed on (`typed_on`) is never written back with the Rust copy; it is still sent to other screens and when Rust clears it | **fixed** (batch 1: each edit came back late and set the field to an older text, so keys typed at wtype's default speed were lost or reordered and a right password was refused although 4 dots showed) |

## Test plan (arena N, both looks; never the host)

Setup: `tools/live-scenarios.sh --arena N --bin <worktree>/native/tessera-ui/target/release/tessera-ui --look glass` (then `--look material`), or copy it and run only the steps below after `run_pass rust`'s setup. All commands are `IRICE_ARENA=N ./dev.sh …`.

1. **Lock at once.** `ipc lock lock`; record 1 s (`wf-recorder` in the arena). PASS: the first frame after the command already covers the screen (no frame of the desktop without the lock); the content plays its entrance (zoom: wallpaper settles from 105 % over 450 ms, clock drops 20 px into place; Material: the black circle opens from the centre).
2. **Wrong password.** `run wtype nope` then `run wtype -k Return`. PASS: field shows dots while typing; ~0.7 s spinner; then "Incorrect password. Try again." under the field (Glass, on one line) / in a red pill above it (Material), the field shakes (Glass 260 ms: +10, −10, +6, 0; Material: a decaying spring ~0.7 s), field empty again, focus still in it (type again without clicking).
2b. **Fast typing.** `ipc lock lock`, then 8 times: `run wtype -d 0 mock`, `run wtype -k Return`. PASS: each try logs "password check: success" (8/8; lock again between tries).
3. **Right password + unlock transition + veil.** Open two `foot` windows first (`run foot &` twice), `ipc lock lock`, then `run wtype mock`, `run wtype -k Return`; record 2 s from Return. PASS: content lifts away (opacity → 0, Glass grows 8 %) over 220 ms; then the lock is gone and the **windows, dock and menu bar fade in** under a veil of the wallpaper (veil gone by ~80 % of 560 ms after ~150 ms; zoom: wallpaper scales 105 % → 100 %). FAIL if windows/dock appear at full opacity on the first frame after the lock goes. Time with `python3 tools/anim-timing.py rec.mp4`: lock gone → desktop at rest ≈ 150 + 560 ms (zoom) / 360 ms (fade, set `lock.unlockTransition: "fade"`).
4. **Unlock "none".** Set `lock.unlockTransition: "none"`; lock, type `mock` + Return. PASS: desktop back on the next frame, no veil, no zoom.
5. **IPC unlock (sandbox).** `ipc lock lock`, `ipc lock unlock`. PASS: same as 3 (veil and settle).
6. **Fingerprint states.** `ipc lock lock`; `ipc lock demo finger` → "Touch the fingerprint reader to unlock" + reader glyph pulses (Glass: in the field, 1→1.14→1; Material: the round target springs from 1.15); `demo miss` → red, shakes, "Not recognised. Try again"; `demo match` → green check (Material: cookie + ripple ring 600 ms); `demo off`.
7. **Face.** `demo face` → "Looking for your face…" with the smiley (Glass) / the lock icon becomes Material's loading indicator (Material); `demo off`.
8. **Lockout text.** `demo lockout` → "Too many attempts: try again in about 9 minutes."; field shakes.
9. **Bedside Mode.** `demo standby`, wait 1 s. PASS: screen goes to StandBy (see StandBy parity). `demo wake` → back. Then with `demo standby` still charging, leave it alone 20 s after a wake: StandBy again by itself (record: lock → StandBy morph 650 ms).
10. **Burn-in moves** (long): in StandBy, take a shot at each minute change for 3 minutes; the clock is at a different height each minute (glides 3 s), always centred across.
11. **Refused lock** (code path; only if the arena can run a second locker): not required.

## Test results

(testers fill in)
