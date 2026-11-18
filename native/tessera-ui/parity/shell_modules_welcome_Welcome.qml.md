# Parity: shell/modules/welcome/Welcome.qml

The welcome screen: once on a new setup (2.5 s after the desktop is up and
unlocked), from Settings → About, or `ipc welcome open`. Over the blurred
wallpaper, a card in four steps: the look, light/dark/auto, a wallpaper,
done. Enter continues, Esc goes back, Tab moves, Space/Enter picks.
Rust: when and what's offered — host `bridges.rs` (`welcome_open`,
`tell_welcome`, `welcome_call`, timer "welcome", ipc `welcome open`,
`welcome_walls`); the screen — `surfaces/welcome/src/lib.rs`, `flow.rs`,
`ui/welcome.slint`.

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Opens 2.5 s after start when welcome.done is false and not locked | 68-73 | bridges `later("welcome", 2500)`, retried every 5 s while locked | same |
| ipc `welcome open`; Settings → About ("Show the welcome screen") closes Settings first | 74-85 | bridges ipc, `welcome_call("open")` | same |
| Opens on the focused screen (else the first), step 0, look = current | 37-45 | `welcome_open` (`focused`/`first_output`), `Welcome::open` | same |
| Later: welcome.done, put away | 46-49 | `welcome.later` → bridges | same |
| Finish: welcome.done; switch look (switch-look.sh) if changed and available | 50-57 | `welcome_finish` | same |
| Bundled pictures: the Tahoe set and Material's pair, found through links | 89-109 | bridges `start` (same shell lines) → `tahoeDir`/`materialDir` | same |
| Walls offered: Through the day, Day, Dusk, Night, Material light & dark (preview by dark/light) | 111-121 | `welcome_walls`, `flow::walls` | same |
| useWall / isWall / own wall | 122-147 | `flow::pick_wall` → bridges `welcome.wall` / `wallpaper_*`; `flow::is_wall`, `own_wall` | same |
| Mode light/dark/auto applied at once | 175-178 | `welcome.mode` | same |
| Pick a picture of your own (file picker, ~/Pictures, images) | 184-186 | `welcome.pick` → `picker.pick` (reply "wallpaper") | same |
| Window: Overlay "tessera-welcome", keyboard exclusive while shown | 205-214 | `Welcome::open` (Overlay, Exclusive; None + empty input while closing) | same |
| In 300 ms out-cubic, out 200 ms in-cubic (instant with Reduce motion); card rises 12 px | 216-228, 263-266 | `t` animation (0.33,1,0.68,1 / 0.32,0,0.67,0), `still` | same |
| Backdrop: the wallpaper blurred (48), dimmed 45 % black (dark) / 30 % white | 231-251 | ⅙-size decode + Gaussian blur, `#00000073` / `#ffffff4d` | same at a glance (no live blur) |
| Clicks and wheel around the card do nothing | 252-257 | `TouchArea` with `scroll-event` accept | same |
| Card ≤760 wide, radius 28 (Material) / panel; fill, 1 px separator (Glass); **no shadow** | 261-271 | `card` | **fixed** (Rust had a drop shadow) |
| Enter → next, Esc → back | 273-276 | `keys` FocusScope | same |
| "Step N of 4", title, help per step (look-switch line on the last) | 284-310 | `flow.title()`, `help()` | same |
| Step change: the choices slide 24 px from the side they come from (250 ms) and fade in (200 ms), out-cubic; none when still | 313-336 | `changed step` (`slide`, `fade`) | same |
| Look page: Glass / Material (only when available) previews, selected bold, captions | 344-381 | look page | different text: Glass's caption drops "like macOS" (no company names in UI text) |
| Mode page: three tiles drawing a window in that mode | 383-449 | mode page | same |
| Wallpaper page: 3-column grid, previews 400×224, the own-picture tile ("Your picture" / "Choose a picture…") | 451-501 | wall page (`walls` model set once per open, rows updated in place) | same |
| Done page: the chosen look's preview | 503-519 | done page | same |
| Buttons: "Set up later" (not on the last step), Back (from step 2), Continue / "Start using Tessera" | 523-551 | buttons row | same |
| ChoiceCard: ring 3 px in the accent (selected 1, hover 0.35, 150 ms), Space/Enter picks, radio role, focus ring | 555-618 | `ChoiceCard` | same |
| WelcomeButton: 44 px, primary/secondary/text colours with hover/press (darker 1.07 / 1.15), press 97 % over 100 ms, focus ring | 620-660 | `WButton` | same |

## Test plan (arena N, both looks)

1. `TESSERA_ARENA=N ./dev.sh ipc welcome open` (standalone); record 1 s. PASS: backdrop and card fade in over ~300 ms, card rises 12 px; no drop shadow under the card (FAIL if a dark halo is visible below it).
2. Enter → step 2 slides in from the right (24 px, 250 ms) and fades (200 ms); Esc → step 1 slides in from the left.
3. Tab to "Dark", Space → the desktop behind switches to dark at once (config.json `appearance.mode: "dark"`).
4. Step 3: click "Dusk" → ring moves to it, wallpaper changes behind; hover another tile → faint ring (0.35). "Choose a picture…" opens the file picker (close it).
5. Last step: "Start using Tessera" → fades out (~200 ms) and `welcome.done` is true; if the look was changed, switch-look runs (skip in the arena unless intended).
6. Reopen, "Set up later" → closes, done = true.
7. Reduce motion on (`appearance.reduceMotion: true`): open → appears at once, steps change without sliding.

## Test results

(testers fill in)
