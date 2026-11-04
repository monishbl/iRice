pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Where Tessera keeps your files: $XDG_CONFIG_HOME/tessera (settings),
// $XDG_DATA_HOME/tessera, $XDG_STATE_HOME/tessera and $XDG_CACHE_HOME/tessera.
// They were …/irice while Tessera was called iRice: launch-shell.sh moves them
// over before each start (assets/scripts/migrate-names.py) and leaves the old
// names as links. A shell that reloaded its files in place right after the
// update (no restart yet, so nothing was moved) still finds everything in
// the old folders: until its next start it keeps using those.
Singleton {
    id: root

    readonly property string configHome: Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config"
    readonly property string dataHome: Quickshell.env("XDG_DATA_HOME") || Quickshell.env("HOME") + "/.local/share"
    readonly property string stateHome: Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state"
    readonly property string cacheHome: Quickshell.env("XDG_CACHE_HOME") || Quickshell.env("HOME") + "/.cache"

    // Not moved yet: the old settings file is there and the new one isn't
    // (read once, at start: what this shell uses never changes under it)
    readonly property bool unmoved: oldProbe.text() !== "" && newProbe.text() === ""
    readonly property string name: unmoved ? "irice" : "tessera"

    readonly property string config: configHome + "/" + name
    readonly property string data: dataHome + "/" + name
    readonly property string state: stateHome + "/" + name
    readonly property string cache: cacheHome + "/" + name

    FileView {
        id: newProbe
        blockAllReads: true
        printErrors: false
        path: root.configHome + "/tessera/config.json"
    }
    FileView {
        id: oldProbe
        blockAllReads: true
        printErrors: false
        path: root.configHome + "/irice/config.json"
    }
}
