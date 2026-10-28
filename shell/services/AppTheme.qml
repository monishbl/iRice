pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Match apps (Settings → Appearance): GTK, Qt, terminal and browser colours
// follow iRice's accent or the wallpaper, light or dark, through matugen
// (assets/scripts/app-theme.sh). Re-applied a moment after any of those
// change; turning it off empties the generated colour files. Desktop light/dark
// preferences and Dolphin's palette follow iRice even when Match apps is off.
Singleton {
    id: root

    readonly property var cfg: Config.options.appearance
    readonly property string source: cfg.appTheme // off | accent | wallpaper
    readonly property bool available: Tools.has("matugen")
    readonly property string key: [Config.ready, source, available, Theme.dark, Theme.accent, source === "wallpaper" ? Wallpaper.path : "", cfg.appThemeTargets.join(",")].join("|")
    property string applied: ""

    onKeyChanged: if (Tools.ready) settle.restart()
    Connections {
        target: Tools
        function onReadyChanged() { settle.restart(); }
    }
    Timer {
        id: settle
        interval: 1500 // after a slider or a wallpaper crossfade settles
        onTriggered: {
            if (!Config.ready || !Tools.ready || root.key === root.applied)
                return;
            run.running = false;
            run.command = Launch.wrap([decodeURIComponent(Qt.resolvedUrl("../assets/scripts/app-theme.sh").toString().replace("file://", "")),
                root.source === "off" || !root.available ? "off" : Theme.dark ? "dark" : "light", String(Theme.accent).slice(0, 7), root.source, Wallpaper.path, root.cfg.appThemeTargets.join(","), Theme.dark ? "dark" : "light"]);
            run.running = true;
            root.applied = root.key;
        }
    }
    Process {
        id: run
        onExited: code => UiState.data.appThemeOn = code === 0 && root.available && root.source !== "off"
    }
}
