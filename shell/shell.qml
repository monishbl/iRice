//@ pragma UseQApplication
//@ pragma IconTheme Papirus-Dark
//@ pragma Env QSG_DISTANCEFIELD_ANTIALIASING=gray
// iRice shell entry point.
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.common
import qs.modules.cheatsheet
import qs.modules.clipboard
import qs.services
import qs.dev
import qs.modules.background
import qs.modules.controlcenter
import qs.modules.decorations
import qs.modules.desktop
import qs.modules.dock
import qs.modules.launchpad
import qs.modules.lock
import qs.modules.menubar
import qs.modules.missioncontrol
import qs.modules.notifications
import qs.modules.osd
import qs.modules.osk
import qs.modules.overview
import qs.modules.quicklook
import qs.modules.overlay
import qs.modules.polkit
import qs.modules.screentools
import qs.modules.screentools.editor
import qs.modules.session
import qs.modules.settings
import qs.modules.share
import qs.modules.sidebar
import qs.modules.spotlight
import qs.modules.stagemanager
import qs.modules.tabs
import qs.modules.tiling
import qs.modules.today
import qs.modules.wallpaper
import qs.modules.switcher
import qs.modules.emoji
import qs.modules.notes
import qs.modules.hotcorners

ShellRoot {
    id: root

    // Don't restart the whole shell whenever a file in its folder changes
    // (a session running straight from the repo would reload on every edit);
    // reload by hand instead (irice repair). IRICE_LIVE_RELOAD=1 turns it on.
    settings.watchFiles: Quickshell.env("IRICE_LIVE_RELOAD") === "1"

    // Escape hatch (Super+Escape): close every panel, overlay and selection,
    // in case something is stuck holding the keyboard or pointer
    function panicReset() {
        Panels.close();
        Panels.settingsOpen = false;
        ScreenTools.cancelSelect();
        ScreenTools.translation = null;
    }
    // Services that must run without a UI referencing them
    readonly property var services: [Power, Weather, Decorations, KeyboardLayout, Tools, Tiling, StageManager, Focus, Calendar, Devices, Privacy, NetworkAlerts, Sounds, Transfers, Calls, Badges, ScreenTime, Todos, AppTheme, LocalSend, QuickShare, WindowRules, IriceUpdate, Displays, Mouse, WorkspaceHomes]
    FontLoader { source: Qt.resolvedUrl("assets/fonts/Phosphor.ttf") }
    FontLoader { source: Qt.resolvedUrl("assets/fonts/Phosphor-Fill.ttf") }
    FontLoader { source: Qt.resolvedUrl("assets/fonts/Phosphor-Bold.ttf") }

    Background {}
    ScreenCorners {}
    MenuBar {}
    Dock {}
    StageStrip {}
    AppLibrary {}
    ControlCenter {}
    NotificationCenter {}
    NotificationPopups {}
    Spotlight {}
    Menus {}
    NowPlaying {}
    StatusMenus {}
    WindowControls {}
    SessionDialog {}
    AboutDialog {}
    Lock {}
    Osd {}
    DisplayOverlay {}
    DisplayConnect {}
    SpaceSwitch {}
    MissionControl {}
    Overview {}
    Switcher {}
    EmojiPicker {}
    QuickNote {}
    HotCorners {}
    SnapPreview {}
    TabStrip {}
    QuickLook {}
    ShareSheet {}
    WidgetGallery {}
    Sidebar {}
    Polkit {}
    WallpaperPicker {}
    Settings {}
    Cheatsheet {}
    ClipboardPanel {}
    Overlay {}
    Keyboard {}
    RegionSelector {}
    ScreenToolbar {}
    RecordCountdown {}
    TodayScreen {}
    TranslationCard {}
    ScreenshotEditor {}

    // Compositor keybinds: bind = hl.dsp.global("irice:<name>")
    component Shortcut: GlobalShortcut {
        required property string panel
        appid: "irice"
        name: panel
        onPressed: Panels.toggle(panel, Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name))
    }
    Shortcut { panel: "spotlight"; description: "Open Spotlight" }
    Shortcut { panel: "controlCenter"; description: "Open Control Center" }
    Shortcut { panel: "notificationCenter"; description: "Open Notification Center" }
    Shortcut { panel: "missionControl"; description: "Open Mission Control" }
    Shortcut { panel: "sidebar"; description: "Open the AI / tools sidebar" }
    Shortcut { panel: "wallpaper"; description: "Choose a wallpaper" }
    Shortcut { panel: "session"; description: "Power options" }
    Shortcut { panel: "cheatsheet"; description: "Keyboard shortcuts" }
    Shortcut { panel: "overlay"; description: "Overlay widgets" }
    Shortcut { panel: "keyboard"; description: "On-screen keyboard" }
    Shortcut { panel: "clipboard"; description: "Clipboard history" }
    Shortcut { panel: "launchpad"; description: "App Library" }
    Shortcut { panel: "quickNote"; description: "Quick note" }
    GlobalShortcut {
        appid: "irice"
        name: "appExpose"
        description: "App Exposé: the focused app's windows"
        onPressed: {
            const t = Hyprland.activeToplevel;
            const cls = t?.lastIpcObject?.class ?? t?.wayland?.appId ?? "";
            Panels.appExpose(Apps.entryFor(cls)?.id ?? cls, Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name));
        }
    }
    GlobalShortcut { appid: "irice"; name: "showDesktop"; description: "Show Desktop (again to bring the windows back)"; onPressed: Desktop.showDesktop() }
    GlobalShortcut { appid: "irice"; name: "settings"; description: "Open Settings"; onPressed: Panels.settingsOpen ? Panels.settingsOpen = false : Panels.openSettings() }
    Shortcut { panel: "screenTools"; description: "Screenshot and recording tools" }
    GlobalShortcut { appid: "irice"; name: "screenshotScreen"; description: "Screenshot of the screen"; onPressed: ScreenTools.screenshotScreen() }
    GlobalShortcut { appid: "irice"; name: "screenshotRegion"; description: "Screenshot of an area"; onPressed: ScreenTools.select("screenshot") }
    GlobalShortcut { appid: "irice"; name: "ocr"; description: "Copy text from the screen"; onPressed: ScreenTools.select("ocr") }
    GlobalShortcut { appid: "irice"; name: "screenTranslate"; description: "Translate text on the screen"; onPressed: ScreenTools.select("translate") }
    GlobalShortcut { appid: "irice"; name: "record"; description: "Start or stop recording"; onPressed: ScreenTools.recording ? ScreenTools.stopRecording() : ScreenTools.select("record") }
    GlobalShortcut { appid: "irice"; name: "lens"; description: "Search an area with Google Lens"; onPressed: Launch.run([decodeURIComponent(Qt.resolvedUrl("assets/scripts/lens-search.sh").toString().replace("file://", ""))]) }
    GlobalShortcut { appid: "irice"; name: "darkMode"; description: "Light or dark appearance"; onPressed: System.toggleDarkMode() }
    GlobalShortcut { appid: "irice"; name: "caffeine"; description: "Caffeine on or off"; onPressed: System.setKeepAwake(!System.keepAwake, 0) }
    GlobalShortcut { appid: "irice"; name: "shareSelection"; description: "Share the selected text with a nearby device"; onPressed: Sharing.shareSelection() }
    GlobalShortcut { appid: "irice"; name: "askScreen"; description: "Ask AI about part of the screen"; onPressed: ScreenTools.select("ask") }
    component Snap: GlobalShortcut {
        required property string layout
        appid: "irice"
        name: "snap-" + layout
        description: "Move window: " + Tiling.names[layout]
        onPressed: Tiling.snap(layout)
    }
    Snap { layout: "left" }
    Snap { layout: "right" }
    Snap { layout: "top" }
    Snap { layout: "bottom" }
    Snap { layout: "fill" }
    Snap { layout: "center" }
    GlobalShortcut {
        appid: "irice"
        name: "tabs"
        description: "Merge windows into tabs, or move a tab out"
        onPressed: (Hyprland.activeToplevel?.lastIpcObject?.grouped?.length ?? 0) > 1 ? Hyprland.dispatch("hl.dsp.window.move({ out_of_group = true })") : Apps.mergeIntoTabs()
    }
    GlobalShortcut { appid: "irice"; name: "pip"; description: "Picture in Picture"; onPressed: Tiling.pip() }
    GlobalShortcut { appid: "irice"; name: "snap-restore"; description: "Return window to previous size"; onPressed: Tiling.restore() }
    GlobalShortcut {
        appid: "irice"
        name: "overview"
        description: "Overview"
        onPressed: Panels.toggle("overview", Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name))
    }
    // Tap Super alone: Overview (or Spotlight, Settings → Keyboard
    // Shortcuts). Pressing Super arms it, letting go within a moment acts,
    // unless Super went into a shortcut (superTapCancel, from every other
    // Super bind and Super + mouse) or a window or space changed meanwhile.
    property real superDown: 0
    GlobalShortcut {
        appid: "irice"
        name: "superTap"
        description: "Tap Super (Overview, or as set)"
        onPressed: root.superDown = Date.now()
        onReleased: {
            const armed = root.superDown > 0 && Date.now() - root.superDown < 700;
            root.superDown = 0;
            if (!armed)
                return;
            const screen = Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name);
            const what = Config.options.keyboard.superTap;
            if (what === "overview")
                Panels.toggle("overview", screen);
            else if (what === "spotlight")
                Panels.toggle("spotlight", screen);
        }
    }
    GlobalShortcut {
        appid: "irice"
        name: "superTapCancel"
        description: "(Super used in a shortcut: not a tap)"
        onPressed: root.superDown = 0
    }
    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (root.superDown > 0 && ["workspacev2", "openwindow", "movewindowv2", "changefloatingmode", "fullscreen", "closewindow", "focusedmonv2"].includes(event.name))
                root.superDown = 0;
        }
    }
    GlobalShortcut { appid: "irice"; name: "reset"; description: "Close everything (if something is stuck)"; onPressed: root.panicReset() }
    GlobalShortcut { appid: "irice"; name: "lock"; description: "Lock the screen"; onPressed: System.power("lock") }
    // (screenshot first, then the report page: Settings → Report a Bug)
    GlobalShortcut { appid: "irice"; name: "reportBug"; description: "Report a bug"; onPressed: BugReport.start() }

    // qs ipc call panels toggle controlCenter [screenName]
    // The desktop has drawn (the wallpaper is up, the bar and dock follow):
    // the update screen waits for this before it fades out over it
    IpcHandler {
        target: "shell"
        function ready(): string {
            return Wallpaper.started && Config.ready ? "yes" : "no";
        }
    }
    IpcHandler {
        target: "panels"
        function toggle(name: string, screen: string): void {
            Panels.toggle(name, Quickshell.screens.find(s => s.name === screen));
        }
        function open(name: string, screen: string): void {
            Panels.show(name, Quickshell.screens.find(s => s.name === screen));
        }
        function close(): void {
            Panels.close();
        }
        function reset(): void {
            root.panicReset();
        }
        function page(name: string): void {
            Panels.ccPage = name;
        }
        function settings(page: string): void {
            Panels.openSettings(page);
        }
        // qs ipc call panels widgets true|false  (Edit Widgets on the first screen)
        function widgets(on: bool): void {
            Panels.close();
            Panels.screen = Quickshell.screens[0];
            Panels.widgetEditing = on;
        }
        function tab(index: int): void {
            Panels.sidebarTab = index;
        }
        function nctab(index: int): void {
            Panels.ncTab = index;
        }
        // qs ipc call panels shareFiles "$(printf '%s\n' /path/a /path/b)"
        // (one path per line; file managers' Share… entries use this, through
        // assets/scripts/share-files.sh)
        function shareFiles(paths: string): void {
            // (a file:// URI is decoded; a plain path is taken as is: "100%.png")
            const list = paths.split("\n").filter(p => p !== "").map(p => p.startsWith("file://") ? decodeURIComponent(p.slice(7)) : p);
            if (list.length > 0)
                Panels.share(list);
        }
        // qs ipc call panels shareSelection  (the selected text, else what's copied)
        function shareSelection(): void {
            Sharing.shareSelection();
        }
        // qs ipc call panels quicklook /path/to/file  ("" closes it)
        function quicklook(path: string): void {
            Panels.quickLookPath = path;
        }
    }

    LazyLoader {
        active: Quickshell.env("IRICE_LAB") === "1"
        component: GlassLab {}
    }
}
