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
        onLoaded: root.ready = true
        onLoadFailed: error => {
            if (error === FileViewError.FileNotFound)
                writeAdapter();
            root.ready = true;
        }

        JsonAdapter {
            id: adapter

            property JsonObject appearance: JsonObject {
                property string mode: "dark" // dark | light | auto (light from sunrise to sunset)
                property string accent: "blue" // see Theme.accents, or "wallpaper"
                property string glass: "liquid" // liquid | frosted | solid | custom (liquid needs the irice-glass plugin; without it, frosted)
                property string menuIcon: "irice" // leftmost menu: irice | distro | linux | windows | apple | android
                property string iconTheme: "" // app icon theme; empty = automatic (KDE / GNOME setting, else Papirus, Breeze, Adwaita)
                property string keySymbols: "auto" // auto (follows menuIcon) | mac | windows | linux
                // Custom material (Settings → Appearance → Advanced), each 0..1
                property real glassLens: 1 // how much it bends what's behind
                property real glassFrost: 0.75 // how much it blurs
                property real glassSolid: 0 // how opaque it is
                property real glassTint: 0.75 // how much it darkens what's behind (milky in light mode)
                property real glassWindowFrost: 0.75 // extra frost where glass covers a window (0 off .. 1), drawn by the plugin
                property string glassDepth: "auto" // thin | depth | auto (thick lens on small controls)
                property bool wallpaperTint: true
                property bool screenCorners: true
                property string decorations: "none" // none | controls | titlebars
                property string windowTabs: "hyprland" // tab bar of grouped windows: hyprland (restyled) | finder (drawn by iRice)
                property string iconStyle: "default" // default | dark | clear | tinted | auto (dark in dark mode)
                property string iconShape: "squircle" // squircle (every icon on the same tile) | original
                property string iconTile: "auto" // tile behind transparent logos: auto (from the logo) | neutral | accent
                property int screenCornerRadius: 12
                property int windowRadius: 20 // app window corners (continuous curve)
                property string cursor: "macos" // pointer: macos | bibata (end4's)
                property string windowOpacity: "subtle" // app windows see-through: off | subtle | more (fullscreen, video and games stay opaque)
                property string motion: "subtle" // subtle (macOS) | expressive (iOS 26: more pull and bounce)
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
                property bool standby: true // StandBy: charging and idle shows a full-screen clock
                property bool standbyShift: true // StandBy moves a little every minute and rests dark for a minute every 15 (burn-in)
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
            }

            property JsonObject sounds: JsonObject {
                property bool enabled: true
                property bool startup: true // when you log in (not on shell reloads)
                property bool screenshot: true
                property bool trash: true // emptying the Trash
                property real volume: 0.6
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
                property bool batteryPercent: false
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
                property string model: "claude-opus-5-5"
                property string effort: "medium" // low | medium | high | xhigh | max (Claude)
                property bool showThinking: true
                property string ollamaUrl: "http://localhost:11434"
                property string systemPrompt: "You are a helpful assistant in a Linux desktop sidebar (Hyprland). Be concise and direct. Use Markdown: short paragraphs, lists where they help, and fenced code blocks for commands or code."
                // Extra models: [{ "id": "...", "name": "...", "provider": "openai" | "openrouter" | "ollama" | "gemini" | "anthropic" }]
                property list<var> extraModels: []
            }

            property JsonObject sidebar: JsonObject {
                property bool booru: true
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
            }

            property JsonObject windowRules: JsonObject {
                // Per-app window rules (Settings → Window Rules), one JSON string per app:
                // {"class","name","float","workspace" (0 = any),"opaque","start" ("" | "maximize" | "fullscreen"),"pin","width","height" (0 = none)}
                // Written to window-rules.lua for Hyprland. Plain strings: JsonAdapter 0.2 is unreliable with var.
                property list<string> rules: []
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
            property JsonObject cooling: JsonObject {
                // Cool-down (Settings → Appearance): less work while hot, on battery or switched on
                property bool whenHot: true // when the CPU is hot (Menu Bar → island → temperature threshold)
                property bool onBattery: false // also on battery (off: the cheaper look only when it's needed)
                property string glass: "frosted" // glass during cool-down: keep | frosted | solid
                property bool lightBlur: true // fewer, smaller Hyprland blur passes
                property bool shortAnimations: true
                property bool pauseLiveWallpaper: true
                property bool lowRefresh: true // the display's lowest refresh rate (48 Hz on most laptop panels)
            }
            property JsonObject performance: JsonObject {
                // Spotlight, Control Center and Notification Center stay loaded
                // after their first open: they open faster and take less CPU,
                // for some memory (common/PanelLoader.qml)
                property bool keepPanelsLoaded: false
            }
            property JsonObject switcher: JsonObject {
                property bool enabled: true // Alt+Tab window switcher with previews
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
                // Window previews: snapshot (captured once each time the Overview opens) | live (keep updating while open) | hybrid (live for the current space, snapshots for the rest)
                property string previews: "snapshot"
            }

            property JsonObject stageManager: JsonObject {
                property bool enabled: false
                property bool strip: true // show waiting stages on the left
            }

            property JsonObject dock: JsonObject {
                property list<string> pinned: ["org.kde.dolphin", "thunar", "firefox", "code", "foot", "org.gnome.Calculator", "org.xfce.mousepad"]
                property int iconSize: 48
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
