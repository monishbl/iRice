# iRice: next round

Tick what you want, add notes under any item (`> your note`), and send it back.
Nothing in sections 3–5 has been changed yet.

---

## 1. What changed in the last rounds (for reference)

**Island**
- Black island with a glassy lower half; iOS-style notification cards (optional); alerts stack and stagger in.
- Animated alerts per kind; device alerts (Bluetooth with battery, USB/drives with Open, displays, audio output).
- Live activities: art, scrolling title, playing bars and controls; replace / around / beside layouts; per-piece switches; one-at-a-time or all; pin.
- Mini player over the album art (seek, open player, volume).
- Hot CPU: one alert per hot spell, Low Power while hot (restored after), soft red ripples on the active chip, faint red arch when open; threshold slider.

**Glass**
- Glass / Frost / Solid presets plus Advanced (glass, frost, solid, tint sliders → Custom).
- Even frost with a refracting edge; 3D highlights fade as it frosts, none when solid.
- Plugin v2 (accepts old shells); concentric corners; Control Center as chips or one panel (setting).

**Desktop and system**
- Desktop widgets on a grid (10 kinds, gallery, drag to snap with a grid and ghost).
- Icons: icon-theme launcher (`QS_ICON_THEME`), `IconLookup` (no more pink/black boxes); dock Downloads/Trash; themed Apps button.
- Menu bar: menu icon choice (distro, brand marks); shortcut symbols (auto/macOS/Windows/Linux); every item can be hidden.
- Wallpaper sets: "Name Light/Dark" pairs and time-of-day folders, detected automatically; macOS wallpapers downloaded to `~/Pictures/Wallpapers/macOS` (not in the repo).
- Repair Shell (menu + Super+Ctrl+Esc); tty session can't duplicate shells any more.
- Screenshots: Super+Shift+S full screen, Super+Ctrl+Shift+S area.

**Bug sweep (this round)**
- Install/uninstall: your original Hyprland/greetd config is backed up once and restored correctly (a re-install used to make uninstall restore iRice).
- tty session: lock files no longer leak into the shell and apps (a crash could leave the session without a shell).
- Repair Shell skips processes whose session it can't read; `dev.sh up` never sends an empty window move to your desktop.
- Focus: re-picking the active mode no longer loses what to restore.
- Calendar: quick saves aren't dropped; `calendars.json` is owner-only.
- Window corners and the anti-flash shader are re-applied after any Hyprland config reload; paths with spaces work.
- Notifications: no re-popping of old notifications on reload; none lost in the first 4 s; lookup maps no longer grow forever.
- Auto-hide menu bar reveals itself for island alerts; tray highlight per screen; timer alerts only when a phase really ends (no duplicate banner).
- Hot state always ends (and gives Low Power back) when temperature stops being watched.
- Hidden animations stop (playing bars, recording dot); status row samples CPU/memory when shown.
- One alert per USB drive (not per partition); audio alerts don't double when the sound server restarts.
- Now Playing seek is correct after switching players.
- Dock no longer rebuilds every icon on each focus change; only the Trash item watches a folder.
- Wallpaper: changes during a crossfade aren't dropped; unloadable files don't loop; names with `#`/`%` load; time phases work across midnight; symlinked folders; video poster can't stick; set names like "Daylight" no longer mis-detected; `|` in names safe.
- Desktop widgets: on the first screen only; widgets outside a smaller screen's grid are shown at the nearest free cell; unknown kinds kept.
- World clock across New Year; calculator `ln`; icon analyser frees memory; UiState doesn't overwrite a pending save.
- Glass: re-sent on theme/material change; follows windows to other monitors; detects an unloaded plugin; request queue can't stall; Control Center chips and menus keep their glass in step with motion.
- Control Center: detail page state no longer breaks after the first open; edit mode keeps temporarily unavailable modules.
- Plugin: controls on every window (other spaces, minimized); safe unload (no listeners left); one copy texture per monitor size; GL objects freed.
- Sandbox: no duplicate binds; terminal bind opens one terminal; the login-screen copy is guarded.
- `.gitignore` fixed (generated `.qmltypes`, `sandbox/tools/gen/`, built `.so` no longer tracked).

---

## 2. Audit findings not fixed yet (tick to fix)

- [x] **Spaces on a second monitor**: chips show workspace ids 1..n globally; on a monitor on space 6 you see chips 1–6 that belong to the other monitor. Fix: list only this monitor's spaces.
- [x] **Wi-Fi password field**: typing a password can be wiped by signal-strength updates while the list refreshes.
- [x] **Elastic pull during slider drags**: the panel moves a little under the pointer while dragging a slider or reordering modules.
- [x] **Clicks swallowed for ~0.2 s while a popover closes**.
- [x] **Two Note widgets share one note** (should each have their own text). - yes
- [x] **Stopwatch** ticks 20×/s even when only whole seconds are shown.
- [x] **Hyprland config paths** assume `~/.config` (ignore `XDG_CONFIG_HOME`).
- [x] **dev.sh down/tty** trust the PID file in `/tmp/irice-rt` without checking what process it is.

---

## 3. Performance and resource optimizations (proposals; tick to apply)

Measured cost today: the shell idles near 0% CPU; costs show up during animation, with many windows, or while something is live.

**Glass**
- [x] **Batch glass updates**: one socket request per frame for all windows (today: one connection per window per change). Fewer wakeups for Hyprland and the shell during animations.
- [x] **One screen copy per layer per frame** in the plugin (today: one `glBlitFramebuffer` per glass shape; Control Center chips = ~13 copies a frame while it animates).
- [x] **Skip the sync loop when nothing moved**: compare a cheap geometry hash before building JSON (today: `JSON.stringify` of every shape every frame for 400 ms after a change).

**Sampling and processes**
- [x] **Temperature without a process**: read the resolved sensor file directly (today a `sh` + `cat` is spawned every sample).
- [x] **Debounce settings writes**: sliders (glass, threshold) write `config.json` on every step; write after 300 ms idle.
- [x] **Recent apps**: write `state.json` less often (today on every focus change).
- [x] **Window list**: compute `Hyprland.toplevels` filters once in a service (dock, spaces, overview, mission control each re-scan all windows on every change).

**Rendering**
- [x] **Pre-rendered icon tiles**: cache each app's styled tile as an image instead of a live shader layer per dock icon (fewer offscreen buffers, less GPU memory).
- [x] **Heat ripple / arch**: use a pre-blurred texture and animate scale/opacity (today the blur re-renders every frame while hot, which can be for hours).
- [x] **Playing bars at 30 fps** (today 60 fps while music plays).
- [x] **Blur strength**: tune Hyprland blur passes/size for the UHD 620 (fewer passes, same look at this size).
- [x] **Live wallpaper**: hardware video decode (VA-API) when available.

**Memory and startup**
- [x] **Lazy-load more panels** (Settings pages, App Library, Overview load only when opened; most already do).
- [x] **Unload thumbnails** in Overview / Mission Control when closed.
- [x] **Measure first**: add `./dev.sh perf` to log CPU/GPU/memory of Hyprland + shell over a scripted session, so each change above is checked with numbers.

---

## 4. What else from iOS / macOS? (tick what you want)

**Window management**
- [x] Drag-to-edge tiling with a live preview (macOS Sequoia) — drag a window to an edge/corner and see where it lands.
- [x] App Exposé: show all windows of the current app (four-finger swipe down or click the dock icon twice).
- [x] Picture in Picture: pop a video out into a small always-on-top corner window.
- [x] Window tabs / grouping like Finder tabs.
- [ ] Hot corners (you skipped these before — still no?).

**Dynamic island and menu bar**
- [x] Privacy indicators: orange/green dot when the mic, camera or screen sharing is in use (+ which app).
- [x] Download / file-copy progress as a live activity.
- [x] Call / meeting activity (Discord, Meet, Zoom: mute, leave).
- [x] VPN / hotspot / Wi-Fi changes as alerts.
- [x] App badges on dock icons (unread counts from apps that report them).

**Lock screen and idle**
- [x] iOS-style lock screen: big clock, widgets, notifications, Now Playing.
- [x] StandBy: when charging and idle, a full-screen clock / widgets / photos view.
- [ ] Boot splash (Plymouth) matching the iRice look.
- [x] Startup / screenshot / trash sounds (optional UI sounds).

**Productivity**
- [x] Quick Look: press Space on a file (in the file picker / Spotlight / Downloads stack) to preview it.
- [x] Spotlight: unit and currency conversion, dictionary definitions, file content search.
- [x] Screenshot markup: crop, draw, arrows, text before saving/sharing. - while using super+ctrl+shift+s
- [x] AirDrop-like sharing with nearby devices (LocalSend integration).
- [ ] Shortcuts / Automations: "when I join Wi-Fi X → Work focus", "at 10 pm → Night Light", etc.
- [ ] Writing Tools: select text anywhere → rewrite / summarize / proofread with the AI (Super+key).
- [ ] Notification summary: a digest of low-priority notifications a few times a day.

**Widgets**
- [x] Smart Stack: several widgets in one tile, rotating or swipeable.
- [x] More widgets: battery history, screen time, Shortcuts buttons, stocks, a to-do with due dates.

**Look and feel**
- [x] Matching app theme: generate GTK/Qt (and terminal) colours from the accent/wallpaper so apps match the shell.
- [x] Tahoe-like cursor and icon theme defaults (suggest packages; you approve installs).
- [x] Reduce transparency / increase contrast (accessibility settings like macOS).
- [x] Desktop icons / stacks on the desktop (files and folders), like Finder's desktop.

**Gestures and input**
- [x] Pinch with four fingers → App Library; spread → show desktop.
- [x] Three-finger drag (move windows without clicking).
- [ ] Ambient light auto-brightness (if your laptop has the sensor).

---

## 5. Preferences for the next round

- [ ] Ask me before each feature / [X] build the ticked list straight through - but ask questions if needed - especially ui
- [X] Performance first, then features / [ ] features first
- Any hard limits (battery, GPU, packages I may install)?
  > go full on, no limits
  
  
  
  
 - [x] all workspaces show wallpaper - show a gray/white container show apps open
 - [x] add a live temp stat next to cpu in dynamic insland
---

## 6. Status after this round

Everything ticked in sections 2–5 is built and was checked in the sandbox and with Quickshell 0.2.1.
Measured with `./dev.sh perf` (shell CPU / frames per second): idle 0.5% → 0%; hot state 12.5% / 60 → ~6% / 20;
music 8.9% / 43 → 5.6% / 26; a dock magnification sweep 70% → 33%.

Needs you:
- [ ] Clean up leftover watcher processes from earlier dev sessions (they filled PipeWire's client slots; the cause is fixed):
      `bash -c 'for d in /proc/[0-9]*; do [ "$(awk "{print \$4}" $d/stat 2>/dev/null)" = 1 ] || continue; case "$(cat $d/comm 2>/dev/null)" in pactl|udevadm|nmcli|bash) ;; *) continue;; esac; tr "\0" "\n" <$d/environ 2>/dev/null | grep -qx IRICE_SANDBOX=1 && kill ${d#/proc/}; done'`
- [ ] Tahoe themes: `paru -S mactahoe-icon-theme-git apple_cursor apple_hyprcursor mactahoe-gtk-theme`
- [ ] Nearby sharing through your firewall: `sudo ufw allow 53317`
- [ ] Sandbox tools (optional): `./dev.sh run sudo pacman -S libqalculate ripgrep plocate swappy psmisc poppler ffmpegthumbnailer python-gobject matugen`

Notes:
- Blur passes/size were measured; no difference on the UHD 620, so the look was left as it was.
- Copies made inside a file manager's own process don't show as a live activity (cp, mv, rsync, dd do).
- Leave for calls only appears when the app has a separate call window (Zoom, Slack huddles); browser calls get Open, so no tabs close.
- Finder-style tabs need the glass plugin to know which tab is in front.
