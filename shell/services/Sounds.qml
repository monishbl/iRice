pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Short UI sounds, like macOS: a chime when the session starts (not when the
// shell reloads), the shutter for screenshots, paper for emptying the Trash.
// From the freedesktop sound theme; each can be switched off (Settings →
// Notifications → Sound effects).
Singleton {
    id: root

    readonly property var cfg: Config.options.sounds
    readonly property string dir: "/usr/share/sounds/freedesktop/stereo/"
    readonly property var files: ({ startup: "service-login.oga", screenshot: "camera-shutter.oga", trash: "trash-empty.oga" })

    function play(name) {
        if (!cfg.enabled || cfg[name] === false || !files[name])
            return;
        const f = dir + files[name];
        const v = Math.max(0, Math.min(1, cfg.volume));
        if (Tools.has("pw-play"))
            Quickshell.execDetached(["pw-play", "--volume", v.toFixed(2), f]);
        else if (Tools.has("paplay"))
            Quickshell.execDetached(["paplay", "--volume", String(Math.round(v * 65536)), f]);
        else if (Tools.has("canberra-gtk-play"))
            Quickshell.execDetached(["canberra-gtk-play", "-f", f]);
    }

    // Startup: once per login session, marked in the runtime dir (cleared at logout)
    Connections {
        target: Tools
        function onReadyChanged() {
            if (Tools.ready)
                startup.running = true;
        }
    }
    Process {
        id: startup
        command: ["sh", "-c", "m=\"${XDG_RUNTIME_DIR:-/tmp}/irice-session-started\"; [ -e \"$m\" ] && exit 1; : > \"$m\""]
        onExited: code => {
            if (code === 0)
                root.play("startup");
        }
    }
}
