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
import qs.modules.overlay
import qs.modules.polkit
import qs.modules.screentools
import qs.modules.session
import qs.modules.settings
import qs.modules.sidebar
import qs.modules.spotlight
import qs.modules.stagemanager
import qs.modules.wallpaper

ShellRoot {
    id: root

    // Escape hatch (Super+Escape): close every panel, overlay and selection,
    // in case something is stuck holding the keyboard or pointer
    function panicReset() {
        Panels.close();
        Panels.settingsOpen = false;
        ScreenTools.cancelSelect();
        ScreenTools.translation = null;
    }
    // Services that must run without a UI referencing them
    readonly property var services: [Power, Weather, Decorations, KeyboardLayout, Tools, Tiling, StageManager, Focus, Calendar, Devices]
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
    MissionControl {}
    Overview {}
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
    TranslationCard {}

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
    GlobalShortcut { appid: "irice"; name: "settings"; description: "Open Settings"; onPressed: Panels.settingsOpen ? Panels.settingsOpen = false : Panels.openSettings() }
    Shortcut { panel: "screenTools"; description: "Screenshot and recording tools" }
    GlobalShortcut { appid: "irice"; name: "screenshotScreen"; description: "Screenshot of the screen"; onPressed: ScreenTools.screenshotScreen() }
    GlobalShortcut { appid: "irice"; name: "screenshotRegion"; description: "Screenshot of an area"; onPressed: ScreenTools.select("screenshot") }
    GlobalShortcut { appid: "irice"; name: "ocr"; description: "Copy text from the screen"; onPressed: ScreenTools.select("ocr") }
    GlobalShortcut { appid: "irice"; name: "screenTranslate"; description: "Translate text on the screen"; onPressed: ScreenTools.select("translate") }
    GlobalShortcut { appid: "irice"; name: "record"; description: "Start or stop recording"; onPressed: ScreenTools.recording ? ScreenTools.stopRecording() : ScreenTools.select("record") }
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
    GlobalShortcut { appid: "irice"; name: "snap-restore"; description: "Return window to previous size"; onPressed: Tiling.restore() }
    // Tap Super: overview. Hyprland sends this on release, and only when no
    // other key was pressed while Super was held.
    GlobalShortcut {
        appid: "irice"
        name: "overview"
        description: "Overview (tap Super)"
        onPressed: Panels.toggle("overview", Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name))
    }
    GlobalShortcut { appid: "irice"; name: "reset"; description: "Close everything (if something is stuck)"; onPressed: root.panicReset() }
    GlobalShortcut { appid: "irice"; name: "lock"; description: "Lock the screen"; onPressed: System.power("lock") }

    // qs ipc call panels toggle controlCenter [screenName]
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
    }

    LazyLoader {
        active: Quickshell.env("IRICE_LAB") === "1"
        component: GlassLab {}
    }
}
