pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Window decorations: none, window controls, or hyprbars title bars
// (optional plugin). Window controls are drawn by the irice-glass plugin when
// it's loaded (they move with the window on the same frame); otherwise the
// shell draws them (WindowControls.qml). Title bars switch live via hyprctl.
Singleton {
    id: root

    readonly property string mode: Config.options.appearance.decorations
    property bool pluginLoaded: false
    readonly property bool titlebarsActive: mode === "titlebars" && pluginLoaded
    readonly property bool pluginControls: mode === "controls" && GlassPlugin.available

    onModeChanged: apply()
    onPluginLoadedChanged: apply()
    onPluginControlsChanged: applyControls()
    Connections {
        target: Theme
        function onDarkChanged() { root.applyControls(); }
    }
    Connections {
        target: GlassPlugin
        function onReady() { root.applyControls(); }
    }

    function applyControls() {
        if (GlassPlugin.available)
            GlassPlugin.command("controls", "irice-glass controls " + (pluginControls ? "on " + (Theme.dark ? "dark" : "light") : "off"));
    }

    function apply() {
        if (pluginLoaded)
            Quickshell.execDetached(["hyprctl", "eval", "hl.config({ plugin = { hyprbars = { enabled = " + (mode === "titlebars") + " } } })"]);
    }

    function refresh() {
        check.running = true;
    }

    // Opens a terminal that builds the plugin (the user runs it)
    function installPlugin() {
        const script = decodeURIComponent(Qt.resolvedUrl("../assets/scripts/install-hyprbars.sh").toString().replace("file://", ""));
        Quickshell.execDetached(["sh", "-c", "xdg-terminal-exec \"$1\" || foot \"$1\" || kitty \"$1\" || alacritty -e \"$1\"", "sh", script]);
    }

    Component.onCompleted: refresh()
    Timer {
        interval: 30000
        running: root.mode === "titlebars" && !root.pluginLoaded
        repeat: true
        onTriggered: root.refresh()
    }

    Process {
        id: check
        command: ["sh", "-c", "hyprctl plugin list 2>/dev/null | grep -qi hyprbars && echo yes"]
        stdout: StdioCollector {
            onStreamFinished: root.pluginLoaded = text.trim() === "yes"
        }
    }
}
