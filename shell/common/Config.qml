pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// User settings, persisted to $XDG_CONFIG_HOME/irice/config.json. Edits to the
// file apply live; changes made from the UI are written back.
Singleton {
    id: root

    readonly property string dir: (Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config") + "/irice"
    property alias options: adapter
    property bool ready: false

    Process {
        running: true
        command: ["mkdir", "-p", root.dir]
    }

    Timer {
        id: writeTimer
        interval: 300 // after a slider stops moving, not on every step
        onTriggered: file.writeAdapter()
    }

    FileView {
        id: file
        path: root.dir + "/config.json"
        watchChanges: true
        onFileChanged: reload()
        onAdapterUpdated: writeTimer.restart()
        onLoaded: {
            // The display switch cue was on/off before it had styles
            if (adapter.workspaces.switchCue === "")
                adapter.workspaces.switchCue = adapter.workspaces.dimOthers ? "glow" : "off";
            // 30 Oct: the anime tab and the gaming overlays are off unless
            // you turn them on again (once, for settings saved before then)
            if (adapter.defaultsVersion < 1) {
                adapter.sidebar.booru = false;
                adapter.defaultsVersion = 1;
            }
            root.ready = true;
        }
        onLoadFailed: error => {
            if (error === FileViewError.FileNotFound)
                writeAdapter();
            root.ready = true;
        }

        JsonAdapter {
            id: adapter

            property JsonObject google: JsonObject {
                property string taskListId: "" // "<account>|<list id>"; empty = keep new tasks local
                property string newEventCalendar: "" // "<account>|<calendar id>"; empty = this computer
                property list<string> hiddenCalendars: [] // "<account>|<calendar id>" not shown (Settings → Google)
            }
            // Earbuds nearby: Google Fast Pair (services/FastPair)
            property JsonObject fastPair: JsonObject {
                property bool enabled: true
            }
            // An Android phone (services/Phone, Settings -> Phone)
            property JsonObject phone: JsonObject {
                property bool enabled: true // look for phones (adb)
                property bool dock: true // in the Dock while connected
                property bool screenOff: true // its screen stays off while it's shown here
                property string camera: "back" // as a webcam: back | front
            }
            // iCloud and Microsoft 365 (services/CloudAccounts)
            property JsonObject cloud: JsonObject {
                property list<string> hiddenCalendars: [] // "<account>|<calendar id>" not shown
            }

            property JsonObject appearance: JsonObject {
                property string mode: "dark" // dark | light | auto (light from sunrise to sunset)
                property string accent: "blue" // see Theme.accents, or "wallpaper"
                property string glass: "liquid" // liquid | frosted | solid | custom (liquid needs the irice-glass plugin; without it, frosted)
                property string menuIcon: "irice" // leftmost menu: irice | distro | linux | windows | apple | android | command | terminal | cpu | atom | planet | rocket | star | lightning | flame | leaf | snowflake | moon | cat | ghost | alien | steam | github | circle
                property string iconTheme: "" // app icon theme; empty = automatic (KDE / GNOME setting, else Papirus, Breeze, Adwaita)
                property string keySymbols: "auto" // auto (follows menuIcon) | mac | windows | linux
                // Custom material (Settings → Appearance → Advanced), each 0..1
                property real glassLens: 1 // how much it bends what's behind
                property real glassFrost: 0.75 // how much it blurs
                property real glassSolid: 0 // how opaque it is
                property real glassTint: 0.75 // how much it darkens what's behind (milky in light mode)
                // Optics, on top of whichever material (Settings → Appearance → Glass → Advanced)
                property real glassIor: 1.5 // index of refraction, 1 (no bending) .. 2; glass is about 1.5
                property real glassRefraction: 1 // how far in from the edge the bending reaches (0.4 .. 2)
                property real glassDispersion: 0.35 // colour fringes at the rim (0 none .. 1)
                property real glassWindowFrost: 0.75 // extra frost where glass covers a window (0 off .. 1), drawn by the plugin
                property string glassDepth: "auto" // thin | depth | auto (thick lens on small controls)
                property bool wallpaperTint: true
                property bool screenCorners: true
                property string decorations: "none" // none | controls | titlebars
                property string windowTabs: "hyprland" // tab bar of grouped windows: hyprland (restyled) | finder (drawn by iRice)
                property string iconStyle: "default" // default | dark | clear | tinted | auto (dark in dark mode)
                property string iconShape: "squircle" // squircle (every icon on the same tile) | original
                property real iconContrast: 0.6 // clear and tinted icons: how far the logo stands apart from its tile (0 .. 1)
                property string iconTile: "auto" // tile behind transparent logos: auto (from the logo) | neutral | accent
                property int screenCornerRadius: 12
                property int windowRadius: 20 // app window corners (continuous curve)
                property string cursor: "macos" // pointer: macos | bibata (end4's)
                property string windowOpacity: "subtle" // app windows see-through: off | subtle | more | custom (fullscreen, video and games stay opaque)
                property real windowOpacityActive: 0.97 // custom: the focused window
                property real windowOpacityInactive: 0.93 // custom: the other windows
                property string motion: "subtle" // subtle (macOS) | expressive (iOS 26: more pull and bounce)
                property bool reduceMotion: false // fades instead of travel (display connect effect and window moves, Today screen)
                // Accessibility (like macOS): solid surfaces instead of glass;
                // stronger text, separators and outlines
                property bool reduceTransparency: false
                property bool increaseContrast: false // the old switch: on counts as "higher" (see contrast)
                property string contrast: "standard" // text contrast: standard | higher | maximum
                // Match apps: GTK, Qt, terminal and browser colours follow the
                // accent or the wallpaper (needs matugen): off | accent | wallpaper
                property string appTheme: "off"
                property list<string> appThemeTargets: ["gtk", "qt", "terminals", "browsers"]
            }

            property JsonObject lock: JsonObject {
                property string layout: "ios" // ios (clock, widgets, notifications, Now Playing) | macos (clock; avatar and field)
                property string material: "system" // cards and field: glass | frost | solid | system (follows Appearance)
                property string text: "auto" // light | dark | auto (from the wallpaper)
                property bool previews: false // notification text on the lock screen (else app names only)
                property bool fingerprint: true // a finger unlocks it too, once one is added (Fingerprint)
                property bool face: true // your face unlocks it too, once one is added (Face: Howdy, IR camera)
                property int faceCount: 0 // faces added from Settings (Howdy's own list is root's)
                property bool standby: true // StandBy: charging and idle shows a full-screen clock
                property bool standbyShift: true // StandBy moves a little every minute and rests dark for a minute every 15 (burn-in)
                property list<string> standbyScreens: [] // screens that show StandBy (others go dark); empty = every screen
                property bool standbyWakeOnMouse: true // moving the mouse leaves StandBy (keys always do)
                property bool standbySameClock: true // every screen shows the clock in the same place
                property string unlockTransition: "zoom" // lock → desktop: zoom (iOS parallax) | fade | none
                property string lockTransition: "zoom" // the lock screen's entrance: zoom (settles into place) | fade | none
                property string standbyTransition: "morph" // lock → StandBy: morph (the clock moves and thins) | fade | none
                property real standbyClockOpacity: 0.7 // StandBy clock brightness (0.2 .. 1)
                property string standbyClockWeight: "thin" // thin | regular | bold
            }

            property JsonObject notificationCenter: JsonObject {
                // The panel behind notifications and widgets; without it (the
                // default) each is its own glass card, like Control Center's chips
                property bool background: false
            }

            property JsonObject sharing: JsonObject {
                // Nearby devices can send you files (LocalSend), like AirDrop:
                // off | 10min (everyone, for 10 minutes) | everyone
                property string receiving: "off"
                property string name: "" // how you appear to others; empty = "<user>'s computer"
                // Android Quick Share (QuickShare.qml): visible to phones nearby,
                // off | 10min | everyone, as on Android; files go to quickShareFolder
                property string quickShare: "off"
                property string quickShareFolder: "" // empty = ~/Downloads
                // With no shared network, this computer's Wi-Fi pauses for a fast
                // link (a hotspot of its own, or the phone's group); off: Bluetooth
                property bool quickShareHotspot: true
            }

            // Settings -> Sound
            property JsonObject audio: JsonObject {
                property bool airplay: false // AirPlay speakers as outputs (services/AirPlay)
            }
            property JsonObject sounds: JsonObject {
                property bool enabled: true
                property bool startup: true // when you log in (not on shell reloads)
                property bool screenshot: true
                property bool trash: true // emptying the Trash
                property bool charging: true // plugging in the charger
                property bool displays: true // connecting or removing a display
                property bool lock: true // locking and unlocking
                property bool notifications: true // banners (not in a Focus mode, not low urgency, not when the app asks for silence)
                property bool battery: true // low battery (20%, 10%, 5%)
                property bool volumeKeys: true // the volume keys
                property bool usb: true // USB devices plugged in or removed
                property bool received: true // files arriving over LocalSend
                property bool recording: true // screen recording starts and stops
                property bool today: true // the morning screen (Today) opens
                property bool error: true // a wrong password
                property string startupFile: "" // your own sound per event; empty = iRice's
                property string chargingFile: ""
                property string displaysFile: ""
                property string pack: "marimba" // style of iRice's sounds: marimba | soft | keys | taps | minimal | glass (assets/sounds/<pack>)
                property real volume: 0.6
            }

            // Screen recording (services/Recorder.qml): gpu-screen-recorder,
            // or wf-recorder when it isn't installed. The toolbar remembers
            // the last choices here.
            property JsonObject recorder: JsonObject {
                property string area: "screen" // screen | window | region
                property string mic: "" // "" off | default | a source's name
                property bool systemAudio: true
                property bool cursor: true
                property int countdown: 0 // seconds: 0 | 3 | 5
                property int fps: 60 // 30 | 60
                property string codec: "h264" // h264 | hevc | av1
                property string quality: "high" // medium | high | very_high | ultra
                property string folder: "" // "" = ~/Videos/Screen Recordings
            }
            property JsonObject screenshot: JsonObject {
                property string editor: "irice" // irice (floating thumbnail, click to mark up) | swappy
                property bool thumbnail: true // iRice editor: floating thumbnail after a screenshot; off = a notification with Edit
                property bool markup: true // Swappy only: area screenshots open in Swappy to crop, draw and annotate
            }

            property JsonObject wallpaper: JsonObject {
                property string path: "" // empty = bundled tahoe-dusk
                // Dynamic: off | time (cycles through a folder over the day) | appearance (light and dark pictures)
                property string dynamic: "off"
                property string dynamicFolder: "" // empty = bundled Tahoe dawn, day, dusk, night
                property string lightPath: "" // empty = bundled tahoe-day
                property string darkPath: "" // empty = bundled tahoe-dusk
                // Live wallpaper: a video file, muted and looping; pauses when windows cover the desktop
                property string live: ""
                property bool livePauseOnBattery: true
                property string directory: "" // empty = ~/Pictures/Wallpapers
            }

            property JsonObject menubar: JsonObject {
                property bool background: false
                property bool showSeconds: false
                property bool showDate: true
                property bool autoHide: false
                property string workspaces: "icons" // off | dots | icons
                property int workspaceCount: 5 // always shown; more appear when used
                property string workspaceOverflow: "arrows" // more spaces than shown: arrows | dynamic (shrinking, fading dots at the ends)
                property bool otherSpaces: true // also list the spaces living on other displays (marked)
                property bool windowTitle: false
                property bool resources: false
                property bool weather: true
                property bool search: true // Spotlight button (Super+Space works anyway)
                property bool ai: true // sidebar button (Super+A)
                property bool controlCenter: true // Control Center button (Super+C)
                property bool clock: true // clock / Notification Center (Super+N)
                property bool appMenu: true // the active app's name and menu, left
                property bool windowMenu: true // the Window menu, left
                property bool battery: true
                property string batteryPercent: "off" // off | inside (cut into the battery) | outside (beside it); "true"/"false" from before mean outside/off
                property bool wifi: true
                property bool bluetooth: false
                property bool sound: false // volume lives in Control Center
                property bool keyboardLayout: true // shown only with 2+ layouts
                property string notificationIndicator: "bellDot" // bellDot | dot | count | bellCount | off
                property bool scrollGestures: true // left half: brightness, right half: volume
                // Extra buttons: screenshot | colorPicker | mic | keyboard | darkMode | powerProfile | record
                property list<string> utilButtons: ["screenshot"]
                property list<string> screens: [] // empty = every screen
            }

            property JsonObject controlCenter: JsonObject {
                // Module layout, "id:WxH" in order (see ModuleDefs); empty = the Tahoe default
                property list<string> layout: []
                // A glass panel behind the controls; off = each control is its own glass chip (Tahoe)
                property bool background: false
                // Extra toggles row; ones whose tools aren't installed are hidden
                property list<string> toggles: ["mic", "lowPower", "stageManager", "screenshot", "colorPicker", "gameMode", "antiFlashbang", "keyboard", "musicRecognition", "record", "easyEffects", "warp"]
            }

            property JsonObject ai: JsonObject {
                property string model: "" // key of the selected user-defined model
                property list<string> models: [] // JSON model records; credentials stay in AiKeys
                property bool modelsMigrated: false
                property string effort: "medium" // low | medium | high | xhigh | max (Claude)
                property bool showThinking: true
                property string ollamaUrl: "http://localhost:11434"
                property string systemPrompt: "You are a helpful assistant in a Linux desktop sidebar (Hyprland). Be concise and direct. Use Markdown: short paragraphs, lists where they help, and fenced code blocks for commands or code."
                // Legacy user models, imported once into models above.
                property list<var> extraModels: []
            }

            property int defaultsVersion: 0 // one-time changes to saved settings (onLoaded)
            // Overlay (Super+G): the floating image, crosshair and FPS limit
            // widgets are only offered when this is on
            property JsonObject overlays: JsonObject {
                property bool gaming: false
            }
            property JsonObject sidebar: JsonObject {
                property bool booru: false
                property string booruProvider: "yandere" // yandere | konachan | danbooru
                property bool booruNsfw: false
                property string translateTarget: "en"
            }

            property JsonObject desktop: JsonObject {
                property bool clock: true
                property bool weather: true
                // Plain ints: Quickshell 0.2's JsonAdapter can crash on var properties
                property int clockX: 48
                property int clockY: 64
                property int weatherX: 48
                property int weatherY: 260
                // Widgets on the desktop grid: "kind:size:col:row", size s | m | l
                property list<string> widgets: ["clock:m:0:0", "weather:m:0:1", "calendar:s:0:2", "batteries:s:1:2"]
                property string photosFolder: "" // empty = ~/Pictures
                property string fileManager: "" // opens folders and the Trash: nautilus | dolphin | thunar | ...; empty = automatic
                property list<string> worldClocks: ["Cupertino|America/Los_Angeles", "London|Europe/London", "Tokyo|Asia/Tokyo", "Sydney|Australia/Sydney"]
                property bool icons: true // files and folders in ~/Desktop on the desktop
                property bool useStacks: false // grouped by kind into stacks
                property list<string> iconPositions: [] // "path\tcol\trow" in icon spots (half a widget cell)
                property bool smartRotate: true // Smart Stacks bring forward what matters now
                property list<string> stocks: ["AAPL", "MSFT", "^GSPC", "BTC-USD", "NVDA", "GOOGL"] // Yahoo Finance symbols for the Stocks widget
            }

            // Today (services/Today.qml): the morning screen on the first
            // unlock of the day (after 4 AM), and its desktop widget
            property JsonObject today: JsonObject {
                property string mode: "ask" // ask (every morning; Pin for today keeps it as a widget) | widget (the widget only, every day) | both | off
                property string entrance: "bloom" // bloom | shrink | drop
                property string name: "" // what the greeting calls you (Settings → About); empty = your account's name
            }
            property JsonObject weather: JsonObject {
                property string location: "" // empty = detect from IP
                property string units: "metric" // metric | imperial
            }

            property JsonObject island: JsonObject {
                property bool enabled: true // workspace dots become a dynamic island
                property bool hud: true // volume and brightness show in the island instead of a separate HUD
                // Events that pop it open: music | charging | battery | timers | recording | focus | bluetooth
                // | resources | usb | displays | audio | notifications (cards instead of banners)
                property list<string> events: ["music", "charging", "battery", "timers", "recording", "focus", "bluetooth", "resources", "usb", "displays", "audio"]
                property list<string> known: [] // kinds the user has switched in Settings (newer kinds default to on)
                // Live activities when collapsed: replace (the spaces) | split (around them) | beside (after them)
                property string activityLayout: "replace"
                // Several live at once, open island: one (focused, scroll to switch) | all (stacked)
                property string activityMode: "one"
                property bool pinned: false // stays open even with nothing happening
                property int hotTemp: 90 // °C: at or above this (twice in a row) the temperature alert fires and the island turns red
                property bool hotThrottle: true // while hot, use the Low Power profile (the previous one comes back after)
                // What a collapsed live activity shows, "kind.piece" (Settings → Menu bar)
                property list<string> livePieces: ["music.art", "music.title", "music.scroll", "music.bars", "music.prev", "music.play", "music.next", "timer.icon", "timer.label", "timer.time", "timer.play", "timer.skip", "stopwatch.icon", "stopwatch.time", "stopwatch.play", "stopwatch.reset", "recording.icon", "recording.time", "recording.stop"]
                // Open island music player: overlay (controls over the album art) | side (art beside the controls)
                property string playerLayout: "overlay"
                property string playerArt: "island" // controls over the art: island (the art is the island, fading like its black) | faded (fades out at its edges)
                // Over a fullscreen app (a video, a game): show (alerts drawn over it, then gone) | important (only calls,
                // low battery, running hot, timers and alarms; the rest goes quietly to Notification Center) | compact (a small pill; click for the alert)
                property string fullscreen: "show"
            }

            property JsonObject windowRules: JsonObject {
                // Per-app window rules (Settings → Window Rules), one JSON string per app:
                // {"class","name","float","workspace" (0 = any),"opaque","start" ("" | "maximize" | "fullscreen"),"pin","width","height" (0 = none)}
                // Written to window-rules.lua for Hyprland. Plain strings: JsonAdapter 0.2 is unreliable with var.
                property list<string> rules: []
                property string defaultLayout: "tiled" // new windows open: tiled | floating (a rule can make an app the opposite; dialogs always float)
            }

            property JsonObject defaultApps: JsonObject {
                // A command per role for keybinds and the shell (Settings → Desktop); empty = automatic
                // (the first installed: kitty, Nautilus, your system browser, VS Code…; see open-app.sh)
                property string terminal: ""
                property string browser: ""
                property string code: ""
                property string text: ""
                property string office: ""
                property string mixer: ""
                property string tasks: ""
            }
            property JsonObject notes: JsonObject {
                property string folder: "" // Markdown notes (Spotlight "note …", quick note); empty = ~/Documents/Notes
            }
            property JsonObject emoji: JsonObject {
                property string style: "android" // emoji drawn by the shell: android (Noto) | apple | windows (Fluent)
                property string klipyKey: "" // your KLIPY app key for the picker's GIF tab (partner.klipy.com/api-keys); KLIPY_API_KEY in the environment wins
                property string gifFilter: "medium" // GIF safety filter: off | low | medium | high
            }
            property JsonObject resources: JsonObject {
                property string graphStyle: "area" // CPU, memory, network… graphs: area (a line with a soft fill) | bars | line
            }
            property JsonObject network: JsonObject {
                property string speedTest: "cloudflare" // speed test server: cloudflare (speed.cloudflare.com, needs only curl) | ookla (speedtest.net; needs speedtest or speedtest-cli)
            }
            property JsonObject power: JsonObject {
                // Battery & Power (Settings; services/Power.qml)
                property string mode: "balanced" // your energy mode: power-saver | balanced | performance
                property bool batterySaver: true // at batterySaverLevel on battery, once per discharge: offer Low Power in the island
                property bool batterySaverAuto: false // turn it on straight away instead (the island offers Undo)
                property int batterySaverLevel: 20
                property bool gamePerformance: true // Game Mode asks for Performance
                property bool critical: true // warn at 10% and 5%, sleep at 3% (battery-guard, even if the shell isn't running)
                // Idle, in seconds (0 = never), on the charger and on battery. Like a Mac:
                // dim, then the screen goes off and locks, then sleep (never on the charger)
                property int idleDimAc: 300
                property int idleLockAc: 600
                property int idleOffAc: 600
                property int idleSleepAc: 0
                property int idleDimBattery: 120
                property int idleLockBattery: 300
                property int idleOffBattery: 300
                property int idleSleepBattery: 900
                // Closing the lid: sleep | lock | nothing
                property string lidAc: "sleep"
                property string lidBattery: "sleep"
                property string lidExternal: "nothing" // on the charger with another display connected
                property string lidCaffeine: "lock" // with Caffeine on, instead of sleeping: lock | nothing | sleep
                property int passwordAfterSleep: 0 // seconds asleep before the password is needed again: 0 | 60 | 300
                property bool appEnergy: true // keep an hour of which apps used the CPU (a look every 5 min)
            }
            property JsonObject cooling: JsonObject {
                // Cool-down (Settings → Appearance): less work while hot, on battery or switched on
                property bool whenHot: true // when the CPU is hot (Menu Bar → island → temperature threshold)
                property bool onBattery: false // also on battery (off: the cheaper look only when it's needed)
                property string glass: "frosted" // glass during cool-down: keep | frosted | solid
                property bool lightBlur: true // fewer, smaller Hyprland blur passes
                property bool shortAnimations: true
                property bool pauseLiveWallpaper: true
                property bool refreshDrop: false // also drop the laptop panel to its lowest refresh rate (48 Hz on most); the switch blanks the screen for a moment
                property string hotAction: "auto" // when the CPU gets hot: auto (cool down by itself) | ask (an island alert asks first)
            }
            property JsonObject performance: JsonObject {
                // Spotlight, Control Center and Notification Center stay loaded
                // after their first open: they open faster and take less CPU,
                // for some memory (common/PanelLoader.qml)
                property bool keepPanelsLoaded: false
            }
            property JsonObject keyboard: JsonObject {
                property string superTap: "overview" // tapping Super alone: overview | spotlight | none
            }
            // Dictation (services/Dictation): speech to text with whisper.cpp, on
            // this computer. Super+H: tap to start and stop, or hold while speaking
            // The drop shelf (services/Shelf): what opens it besides its
            // shortcut (Super+Alt+D)
            property JsonObject shelf: JsonObject {
                property bool shake: true // shaking the pointer while dragging
                property bool edge: true // holding a drag at a screen's left or right edge
                property bool menubar: true // an icon in the menu bar while it holds something
            }
            property JsonObject dictation: JsonObject {
                property string model: "base" // tiny | base | small | medium | large-v3-turbo (assets/scripts/dictate.py)
                property string language: "" // a code ("en"), "auto", or "" for the system's language
                property bool sounds: true // a tick as it starts and stops listening
            }
            property JsonObject captions: JsonObject {
                // Live captions (Settings → Accessibility, services/Captions):
                // subtitles for the sound this computer plays
                property bool enabled: false
                property string placement: "bar" // bar (floating, movable) | island | strip (across the bottom)
                property string language: "auto" // "auto", or a code ("en")
                property real barX: -1 // the floating bar's place on its display (-1: centred above the Dock)
                property real barY: -1
            }
            property JsonObject findPointer: JsonObject {
                // Shake the mouse: the pointer grows for a moment, with a ring
                // around it (hypr/irice/findpointer.lua, services/FindPointer)
                property bool enabled: true
            }
            property JsonObject workspaces: JsonObject {
                // Moving to another display (keyboard or pointer)
                // How the display you moved to shows it: glow (a soft accent glow at its edges) | dim (it dims briefly) | off.
                // Empty until set: then it comes from dimOthers, the older on/off setting (true: glow, false: off)
                property string switchCue: ""
                property bool dimOthers: true // (older setting, read once to set switchCue)
                property bool warpPointer: true // move the pointer to that display
            }
            property JsonObject mouse: JsonObject {
                // Mice (Settings → Mouse); trackpads keep their own settings
                property string accelProfile: "adaptive" // adaptive (speeds up with faster moves) | flat (same distance at any speed)
                property real speed: 0 // -1 .. 1
                property bool naturalScroll: false
                property real scrollSpeed: 1 // 0.2 .. 3
                property bool leftHanded: false
                property bool middleEmulation: false // left + right together = middle click
                property string boundaryMode: "corners" // off | corners | resistance | capture
                property int cornerSize: 24 // logical pixels at each end of a monitor edge
                property string focus: "hover" // window focus: hover (follows the pointer; Hyprland's default) | click
            }
            property JsonObject trackpad: JsonObject {
                // Trackpads (Settings → Trackpad); gestures live in keybinds.json
                property real speed: 0 // -1 .. 1
                property bool tapToClick: true
                property bool tapAndDrag: true // tap, then drag with the finger still down
                property bool dragLock: false // a tap-drag keeps holding after you lift, until the next tap
                property bool naturalScroll: true
                property real scrollSpeed: 0.7 // 0.2 .. 3
                property bool disableWhileTyping: false
                property string rightClick: "twoFingers" // twoFingers (click with two fingers) | corner (click the bottom-right corner)
                property bool middleClick: false // three-finger click / both bottom corners = middle click
            }
            property JsonObject displays: JsonObject {
                // Settings → Displays: "name\tmode\tx\ty\tscale\ttransform\tenabled\tmirror" per display, written to hypr/irice monitors
                property list<string> monitors: []
                // When a display connects or leaves (services/DisplayFx.qml):
                // off | extend | liquid | warp, and the sound panned toward it
                property string connectAnimation: "extend"
                property bool directionalSound: true
            }
            property JsonObject switcher: JsonObject {
                property bool enabled: true // Alt+Tab window switcher with previews
            }
            property JsonObject privacy: JsonObject {
                // Where a screen share shows (Settings → Menu Bar): island (a live activity; no dot meanwhile) | dot (the purple privacy dot) | both. With the island off, the dot.
                property string screenShareIndicator: "island"
            }
            property JsonObject hotCorners: JsonObject {
                // none | overview | missionControl | desktop | notificationCenter | launchpad | lock | standby | quickNote
                property string topLeft: "overview"
                property string topRight: "none"
                property string bottomLeft: "none"
                property string bottomRight: "none"
                property int delay: 200 // ms the pointer rests in the corner before the action runs
            }

            property JsonObject overview: JsonObject {
                // Window previews (Overview, Mission Control, Stage Manager strip): snapshot (captured once each time it opens) | live (keep updating while open, up to 30 fps) | hybrid (live for the current space, snapshots for the rest)
                property string previews: "snapshot"
                // grid (the spaces as large previews, search on top) | macos (a strip of spaces on top, only a search in the middle)
                property string style: "grid"
                property bool wallpapers: false // space thumbnails (Overview, Mission Control) show the wallpaper behind their windows
                // With several displays: rows (the screen it opens on shows every display's spaces, a row each) | each (it opens on every screen, each with its own spaces)
                property string displays: "rows"
            }

            // Settings → Login Items (assets/scripts/xdg-autostart.py reads it at login)
            property JsonObject loginItems: JsonObject {
                property bool startAtLogin: true // off: no XDG autostart app starts with the session
            }

            property JsonObject stageManager: JsonObject {
                property bool enabled: false
                property bool strip: true // show waiting stages on the left
            }

            property JsonObject dock: JsonObject {
                property bool enabled: true // off: no dock (App Library stays on its shortcut)
                property list<string> pinned: ["org.kde.dolphin", "thunar", "firefox", "code", "foot", "org.gnome.Calculator", "org.xfce.mousepad"]
                property int iconSize: 48
                property real gap: 0.18 // space between icons, as a fraction of the icon size (0.05 .. 0.4)
                property bool appsButton: true // App Library button at the start
                property bool magnification: false
                property string autoHide: "off" // off | always | windows (hide when a window overlaps it)
                property bool recentApps: true // up to 3 recent apps after a divider
                property bool downloads: true
                property bool trash: true
                property real magnifiedScale: 1.45
            }
        }
    }
}
