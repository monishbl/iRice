pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// The Tessera login screen (greetd; ./install.sh --greeter keeps its own copy
// in /usr/share/tessera): its look follows yours. The greeter can't read your
// home, so the little it needs (the style from Settings → Lock Screen →
// Login screen, the look you're using, light or dark, Material's colours
// and the clock's style) goes to /usr/share/tessera/greeter.json through the
// root helper (tessera-root login-look, no password for an administrator),
// a few seconds after a change, and only when it differs from what's there.
// Never from the dev sandbox; never before an update has installed a helper
// that knows it (its policy names org.tessera.root.login-look: otherwise it
// would ask for the password).
//   LoginScreen.installed  LoginScreen.preview()
Singleton {
    id: root

    readonly property bool installed: hasCopy
    property bool hasCopy: false
    property bool canWrite: false
    property string status: ""

    // What the greeter gets, its keys in order (greeter.json is written
    // sorted, so the two compare as text)
    readonly property string look: {
        const a = Config.options.appearance, l = Config.options.lock;
        const s = Theme.materialSeed ?? (/^#[0-9a-fA-F]{6}$/.test(a.materialSeed) ? a.materialSeed : String(Wallpaper.tone).slice(0, 7));
        return JSON.stringify({
            clock24: a.clock24 === true,
            clockFace: l.clockFace,
            clockRound: l.clockRound,
            clockWeight: Math.round(l.clockWeight),
            clockWidth: Math.round(l.clockWidth),
            look: a.look === "material" ? "material" : "glass",
            materialStyle: a.materialStyle,
            mode: Theme.dark ? "dark" : "light",
            seed: /^#[0-9a-fA-F]{6}$/.test(s) ? s : "#6750A4",
            style: l.loginStyle ?? "look"
        });
    }
    onLookChanged: if (Config.ready) later.restart()

    Process {
        running: true
        // (/usr/share/irice: the login screen installed before iRice became
        // Tessera, until ./install.sh --only=login runs again)
        command: ["sh", "-c", "[ -d /usr/share/tessera/shell ] || [ -d /usr/share/irice/shell ] || exit 1; " + RootHelper.knows("login-look") + " && exit 0; exit 2"]
        onExited: code => {
            root.hasCopy = code !== 1;
            root.canWrite = code === 0;
            if (root.canWrite)
                later.restart();
        }
    }
    // The login screen's own copy: Tessera's, or iRice's until it's installed again
    property string share: "/usr/share/tessera"
    Process {
        running: true
        command: ["sh", "-c", "[ -d /usr/share/tessera ] || [ ! -d /usr/share/irice ] || echo old"]
        stdout: StdioCollector {
            onStreamFinished: if (text.trim() === "old")
                root.share = "/usr/share/irice"
        }
    }
    FileView {
        id: current
        path: root.share + "/greeter.json"
        watchChanges: true
        onFileChanged: reload()
    }
    Timer {
        id: later
        interval: 3000
        onTriggered: {
            if (!root.canWrite || System.sandboxed || write.running)
                return;
            if (current.text().trim() === root.look)
                return;
            write.command = RootHelper.command(["login-look", root.look]);
            write.running = true;
        }
    }
    Process {
        id: write
        onExited: code => {
            root.status = code === 0 ? "The login screen has your look." : "Couldn't give the login screen your look. It's tried again after your next change.";
            current.reload();
        }
    }

    // Shows the login screen here, as it would look, with a stand-in
    // password ("tessera"); Escape closes it. The same folder as this shell
    // (Glass, or the Material root), so "look" shows the look you're in.
    function preview() {
        const s = Config.options.lock.loginStyle ?? "look";
        Quickshell.execDetached(["sh", "-c", "QS=qs; for d in \"${XDG_DATA_HOME:-}/tessera\" \"$HOME/.local/share/tessera\" \"${XDG_DATA_HOME:-}/irice\" \"$HOME/.local/share/irice\"; do if [ -x \"$d/quickshell/bin/qs\" ]; then QS=\"$d/quickshell/bin/qs\"; export QML_IMPORT_PATH=\"$d/quickshell/lib/qt6/qml${QML_IMPORT_PATH:+:$QML_IMPORT_PATH}\"; break; fi; done; TESSERA_GREETER_MOCK=1 TESSERA_GREETER_STYLE=\"$1\" TESSERA_GREETER_WALLPAPER=\"$2\" TESSERA_GREETER_CLOCK24=\"$4\" exec \"$QS\" -p \"$3/greeter.qml\"", "sh", s, Wallpaper.path, Quickshell.shellDir, Config.options.appearance.clock24 ? "1" : "0"]);
    }
}
