pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Match apps (Settings → Appearance): GTK, Qt, terminal and browser colours
// follow iRice's accent or the wallpaper, light or dark, through matugen
// (assets/scripts/app-theme.sh). Re-applied a moment after any of those
// change; turning it off empties the generated colour files.
Singleton {
    id: root

    readonly property var cfg: Config.options.appearance
    readonly property string source: cfg.appTheme // off | accent | wallpaper
    readonly property bool available: Tools.has("matugen")
    readonly property string key: [source, Theme.dark, Theme.accent, source === "wallpaper" ? Wallpaper.path : "", cfg.appThemeTargets.join(",")].join("|")
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
            // Nothing to undo if it was never on
            if (!root.available || root.key === root.applied || (root.source === "off" && root.applied === "" && !root.wasOn))
                return;
            run.running = false;
            run.command = [decodeURIComponent(Qt.resolvedUrl("../assets/scripts/app-theme.sh").toString().replace("file://", "")),
                root.source === "off" ? "off" : Theme.dark ? "dark" : "light", String(Theme.accent).slice(0, 7), root.source, Wallpaper.path, root.cfg.appThemeTargets.join(",")];
            run.running = true;
            root.applied = root.key;
        }
    }
    // It was on in an earlier session (so "off" has files to empty)
    readonly property bool wasOn: UiState.data.appThemeOn === true
    Process {
        id: run
        onExited: UiState.data.appThemeOn = root.source !== "off"
    }
}
