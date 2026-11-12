# shell/modules/settings/pages/wallpaper.qml ↔ surfaces/settings (pages/look/wallpaper.rs, ui/settings-look.slint wallpaper-preview / sets / wallpaper-grid)

Page id `wallpaper`. Setters: host/shell/src/bridges.rs `wallpaper_set`,
`wallpaper_use_set` (Wallpaper.qml's `set`, `useSet`).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Preview of the picture showing now: min(page, 420) wide, 16:9, radius 28 Material / 14 Glass, fill behind, cover-cropped, decoded at 840×472 (15–27) | wallpaper.rs 61–68, LookBlock `wallpaper-preview` | same |
| 2 | Picture row: detail "Any image on your computer, or pick one below"; button "Choose Picture…" / "Choosing…" disabled while the picker is busy; picker title "Choose a wallpaper", start ~/Pictures, images filter; the pick → Wallpaper.set (dynamic off, live off, path) (29–40) | 70–80, host `setWallpaper` | same |
| 3 | Wallpaper sets group only when there are sets; footer "Pairs named “Name Light”…" (42–47) | 83–121 | same |
| 4 | Set tiles 150×90 in a Flow (12 apart, 4 padding), radius 16/12, 3 px accent border when current (isCurrentSet); pair: light left half, dark right half; name small/500 elided at 150; "Light & dark"/"Time of day" caption secondary; click → useSet (live off; pair: light/dark paths + dynamic appearance; folder: dynamicFolder + dynamic time), pointing hand (48–105) | `Sets` (settings-look.slint 721–782), use_set 103–115 | same (kind line at y 114 vs QML ≈ 119: name height 17 + 6 spacing) |
| 5 | Dynamic group: footer per mode; Change automatically Segmented 300 Off/Time of day/Light & dark (108–121) | 123–135 | same |
| 6 | Pictures (time): folder PathField, placeholder "Built-in set", start ~/Pictures, detail (122–133) | 136–139 | same |
| 7 | Light picture / Dark picture (appearance): file PathFields, placeholders "Built-in day"/"Built-in dusk", images filter, start ~/Pictures (134–155) | 140–141 | same |
| 8 | Live wallpaper: footer; Video PathField (placeholder ~/Videos/wallpaper.mp4, videos filter, start ~/Videos); detail "Couldn't read this video. Check the path, or install ffmpeg." when a video is set but has no poster, else "An .mp4, .webm or .mkv file" (158–173) | 143–150 | **different (standalone)**: `look_feed` sends no `livePoster`, so any set video shows the "Couldn't read this video" detail |
| 9 | Login screen group: footer status or "Install the Tessera login screen with ./install.sh --greeter." (installed = /usr/share/tessera or /usr/share/irice) (176–189) | 152–159 | same |
| 10 | "Use this wallpaper on the login screen" (installed only): detail by root helper; button "Use wallpaper" (Material 40 tonal, Glass 28 fill); in the sandbox only the status "Sandbox: would copy…"; else root helper or pkexec cp; status texts (190–225) | 160–163, host `look.loginWallpaper` (bridges.rs 928–948) | same |
| 11 | Picture grid: the folder's pictures (jpg/jpeg/png/webp/JPG/PNG, by name) then the bundled .jpg; 3 a row, (page−24)/3 wide 16:9, 12 apart, radius 16/10, 3 px accent border on the current; click → Wallpaper.set; pointing hand (228–274) | 165–199, `Grid` | same (list read once while Settings is open; QML's FolderListModel also shows files added while the page is open) |
| 12 | More options: Folder PathField (placeholder ~/Pictures/Wallpapers, start ~/Pictures) (277–291) | 204–205 | same |
| 13 | More options: Live wallpaper → Pause on battery switch (292–302) | 206–208 | same |

## Test plan (both looks)

`setup qml|rust material|glass <bin>`, `open wallpaper`, `shot`.

P1 Top: PASS: the preview (≤ 420 wide, 16:9, rounded 28 Material / 14 Glass)
shows the same picture as the desktop; "Picture" row with "Choose Picture…".

P2 Sets (when the wallpaper folder has pairs/time folders): tiles 150×90, pairs
split light|dark, the current one with a 3 px accent border; click another
(read its centre from the shot). PASS: border moves; Dynamic switches to "Light &
dark" (pair) or "Time of day"; the Light/Dark picture or Pictures rows show.

P3 Dynamic: click "Time of day": footer "Follows sunrise and sunset…", Pictures
row with placeholder "Built-in set". "Light & dark": two rows, placeholders
"Built-in day"/"Built-in dusk". "Off": footer empty, rows gone.

P4 Live wallpaper: type a real video path in Video (e.g. a short .mp4 in the
arena) and press Return. PASS: once the poster is made, detail "An .mp4, .webm
or .mkv file"; a bad path: "Couldn't read this video…". (Rust standalone fails
the good-path case today: #8.) Clear the field afterwards.

P5 Grid: scroll to the thumbnails (`shot`), click one. PASS: border moves to it,
preview and desktop change, Dynamic → Off.

P6 Login screen: in the sandbox, click "Use wallpaper" (if installed). PASS:
footer "Sandbox: would copy this wallpaper to the login screen."

P7 More options: Folder row with placeholder "~/Pictures/Wallpapers"; Pause on
battery toggles.

## Fixed in batch 2 (fixer)

- Not fixed yet: `livePoster` (a video wallpaper shows "Couldn't read this video…"): no host state holds the poster.

## Fixed in batch 3 (fixer)

- The live wallpaper's poster: the shell's, else the one at cache/live-poster-<md5>.jpg (made with ffmpeg off the UI thread when missing); "Couldn't read this video" only once making it failed.

## Test results

(tester)
