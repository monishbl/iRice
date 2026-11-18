# shell/modules/background/Background.qml → surfaces/wallpaper/src/background (+ ui/background.slint)

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| One window per screen, background layer, full screen, no input, black behind | 21-50, 124-128 | background/mod.rs `show` (Layer::Background, anchors all), background.slint `background: black` | same |
| Picture cropped to cover (PreserveAspectCrop), decoded off the UI thread at the screen's pixels | 156-163 | `refresh` → `picture::decode` on a Pool, size = output mode / window pixels (`after_frame`) | same |
| Which picture: chosen / bundled default / time of day (minute by minute) / light-dark pair / live poster | Wallpaper.qml `source` | `wanted` → core `wallpaper::picture`; `tick` each minute | same |
| Two buffers: the new one fades in over the old, 700 ms InOutQuad, old one let go after | 113-153, 183-195 | slint `fade`/`a-from`/`b-from`, `animate fade 700ms cubic-bezier(0.45,0,0.55,1)`; `faded` → old image dropped | same |
| First picture of the session fades in from black, 450 ms OutCubic | 172-181 | was: shown at once (assumed the QML's picture underneath). **Fixed**: standalone (no QML) fades in from black over 450 ms out-cubic (`first-ms`) | different → fixed |
| A screen plugged in later: first fade 120 ms OutCubic | 172-176 | `first-ms` 120 | same |
| Unreadable picture: the one before stays | 165-168 | `decoded`: None keeps the old | same |
| Lock/unlock settle: wallpaper scale 1.05 → 1, LockFx curve (linear desktop, ease-out cubic scale), zoom off = fade only | 95-104, 128 | `Settle`, `apply_settle`, slint `desk`/`scale`; standalone: host/shell bridges.rs sends `settle` | same |
| Sunrise/sunset for the time-of-day picture | 66-91 | `message` → `wallpaper::told_sun` → `refresh` (host/shell bridges.rs standalone) | same |
| Only reloads when the picture or size changes (window events, unrelated config) | onWantChanged | `refresh` compares `asked` (path, px): no decode, no redraw otherwise; nothing listens to window events | same |
| surface_closed for its own windows only | — | **Fixed**: was any part's surface closing → that output's background dropped and re-made (a black flash, a reload). Now `Rc::ptr_eq` on its own windows | different → fixed |
| Live wallpaper video: muted, looping, cropped to cover (PreserveAspectCrop) | 197-225 | background/video.rs `Player`: ffmpeg `-stream_loop -1`, scaled+cropped to the screen's pixels (at most 1920 wide, the window scales up), raw RGBA read one frame at a time at the video's avg_frame_rate (≤ 60) | was missing (poster only) → **added** (standalone; with the QML shell the QML plays it over the poster, as before) |
| Paused while: Desktop.mostlyCovered(screen) (a full-screen window, or windows ≥ 70% of the screen's area on its shown space) · System.locked · IdleMonitor 120 s (inhibitors respected) · livePauseOnBattery && UPower.onBattery · CoolDown.pauseLive | 202-212 | `live_sync`: `video::mostly_covered` (clients service), ShellState `locked`, idle watch `AWAY` 120 000 ms respecting inhibitors, `wallpaper.livePauseOnBattery` && battery discharging (status service), ShellState `cooldown.pauseLive`; re-checked on every windows/battery/idle/state reading | added (same conditions, per screen) |
| Paused: the last frame stays, nothing decoded | 211 (player.pause) | the reader stops reading; ffmpeg blocks on the full pipe (no CPU) | added |
| The video appears over the poster | (VideoOutput over the poster) | first frame crossfades in over the poster (700 ms in-out-quad, the picture-change fade); later frames replace it in place; the poster decoded again later doesn't cover it | added (the QML cuts from poster to video when playing starts; here it fades) |
| Cost | — | one frame in flight to the window, a slow frame dropped (never queued), buffers handed back and reused (no per-frame allocation), `-threads 2` | — |
| Live wallpaper turned off / changed / screen resized | Loader active | player dropped (ffmpeg killed) or remade at the new size; the new picture crossfades over the last frame | added |
| Blurred backdrop bake for glass (quarter size, saved per wallpaper) | 227-327 | not here: the glass plugin's backdrop (core/plugin) | out of this crate (glass backdrop owner) |
| Menu bar light/dark sampling of the top 3% | 260-285 | core services/wallpaper.rs `WallTop` (same 64×64, two rows, 0.6) | same (core) |
| Desktop widgets, icon analyser in this window | 329-344 | surfaces/desktop (its own window) | n/a here |

Both looks: the background is the same in Glass and Material (only the bundled default differs: `Bundled::file(material, …)`).

## Test plan (Sonnet tester, arena 5, TESSERA-TEST 1920x1080 at scale 1, user's config; both shells; Material then Glass)

1. **First picture (standalone only).** Start recording the whole output (`wf-recorder -o TESSERA-TEST -r 60`), then `./dev.sh reload` with `rust.standalone: true`. Stop after 15 s. `python3 tools/anim-timing.py rec.mp4` on a region of bare wallpaper (e.g. 1500,450 300x200): the wallpaper must come up **from black**, an ease-out fade of **450 ms** (QML firstFade: 450 ms OutCubic). Before this fix it appeared at once. QML pass: the same 450 ms fade.
2. **Change crossfade.** Open the picker (`ipc panels open wallpaper TESSERA-TEST`), Right, Right, record and press Return, record 1.6 s. On a bare-wallpaper region (1500,300 300x200; above the picker) the new picture fades in over the old, **700 ms ease-in-out (InOutQuad)**, with no black or blank frame between (`tools/region-changes.py` shows about 42 changing frames in a row, no frame much darker than both pictures).
3. **No reload on window events.** Close the picker (Esc). Open two `foot` windows, wait 2.5 s, record 1.5 s while closing one (`hyprctl dispatch 'hl.dsp.window.close({ window = "class:foot" })'`), then the other. In the area the closed window uncovered, the wallpaper must appear at once with no black flash and no fade-in (region-changes: only the frames where the window goes). Repeat by opening and closing another surface (the launchpad: `ipc panels open launchpad TESSERA-TEST`, then `ipc panels close`): the wallpaper region outside the launchpad must not change at all. (Before: any part's surface closing made the background drop and remake its window.)
4. **Lock settle.** `ipc lock lock`, wait 2 s, record 1.5 s while unlocking (`ipc lock unlock`): the wallpaper starts 5% larger and settles to 1.0 over the reveal (LockFx: linear desktop, ease-out-cubic scale), lined up with the lock veil (no jump at the end). Same in both shells.
5. **Live wallpaper (standalone).** Put a short mp4 (e.g. `ffmpeg -f lavfi -i testsrc2=size=1280x720:rate=30 -t 6 ~/Videos/live.mp4` inside the arena) and set `wallpaper.live` to its path in config.json. Record 4 s from the change: the poster appears (crossfade), then the moving video crossfades in over it within about a second, and plays (region-changes on 1500,300 300x200: a change every frame, ~30/s, at the video's pace, not faster). Wait 7 s: it loops without a black frame. 
   - Open a `foot` window and make it full screen (`hyprctl dispatch 'hl.dsp.window.fullscreen()'` or the user's shortcut), close it: while it's full screen the tessera-ui process's CPU (`top -b -n 3 -d 1 -p <pid>`, plus the `ffmpeg` child) drops to about 0; after closing, the video goes on.
   - Open two `foot` windows tiled side by side (≥ 70% of the screen): paused (the uncovered gaps show a still frame); close one: plays again.
   - `ipc lock lock`: paused; unlock: plays.
   - Set `cooling.pauseLiveWallpaper` true and trigger Cool-down if the arena can (or skip); with `wallpaper.livePauseOnBattery` true the arena has no battery, so it plays (note it).
   - Leave the pointer still 2 minutes 10 s: paused (CPU ~0); move it: plays.
   - Clear `wallpaper.live`: the chosen picture crossfades in over the last frame and the `ffmpeg` child process is gone (`pgrep -af ffmpeg` in the arena, check its XDG_RUNTIME_DIR).
   QML pass: the same video plays (QtMultimedia), pauses on the same conditions.
6. Glass and Material: steps 1-3 again with `appearance.look` = glass; only the bundled default picture differs.
