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
        interval: 100
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
                property real glassFrost: 0.1 // how much it blurs
                property real glassSolid: 0.12 // how opaque it is
                property real glassTint: 0.45 // how much it darkens what's behind (milky in light mode)
                property string glassDepth: "auto" // thin | depth | auto (thick lens on small controls)
                property bool wallpaperTint: true
                property bool screenCorners: true
                property string decorations: "none" // none | controls | titlebars
                property string iconStyle: "default" // default | dark | clear | tinted | auto (dark in dark mode)
                property string iconShape: "squircle" // squircle (every icon on the same tile) | original
                property string iconTile: "auto" // tile behind transparent logos: auto (from the logo) | neutral | accent
                property int screenCornerRadius: 12
                property int windowRadius: 20 // app window corners (continuous curve)
                property string motion: "subtle" // subtle (macOS) | expressive (iOS 26: more pull and bounce)
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
                property list<string> worldClocks: ["Cupertino|America/Los_Angeles", "London|Europe/London", "Tokyo|Asia/Tokyo", "Sydney|Australia/Sydney"]
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
