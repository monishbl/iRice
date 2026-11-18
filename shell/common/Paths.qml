pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Where Tessera keeps your files: $XDG_CONFIG_HOME/tessera (settings),
// $XDG_DATA_HOME/tessera, $XDG_STATE_HOME/tessera and $XDG_CACHE_HOME/tessera.
// (Folders under Tessera's old name move over before the shell starts:
// launch-shell.sh, assets/scripts/migrate-names.py.)
Singleton {
    id: root

    readonly property string configHome: Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config"
    readonly property string dataHome: Quickshell.env("XDG_DATA_HOME") || Quickshell.env("HOME") + "/.local/share"
    readonly property string stateHome: Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state"
    readonly property string cacheHome: Quickshell.env("XDG_CACHE_HOME") || Quickshell.env("HOME") + "/.cache"

    readonly property string name: "tessera"

    readonly property string config: configHome + "/" + name
    readonly property string data: dataHome + "/" + name
    readonly property string state: stateHome + "/" + name
    readonly property string cache: cacheHome + "/" + name
}
