pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// tessera-ui (native/tessera-ui, built by tools/build-native.sh --ui): Tessera's
// interface in Rust, drawing the surfaces Settings lists in `rust.surfaces`
// in place of this shell's (docs/rust-ui-plan.md). This shell hides its own
// copy of a surface only while tessera-ui says it draws it (drawing(name));
// when tessera-ui isn't built, ends, or never says so, this shell draws it.
// It ends when this shell does (its stdin closes).
Singleton {
    id: root

    readonly property list<string> wanted: Config.ready ? Config.options.rust.surfaces : []
    readonly property string binary: Quickshell.env("TESSERA_UI") || Paths.data + "/bin/tessera-ui"
    property var drawn: [] // what it said it draws
    function drawing(name) {
        return proc.running && drawn.includes(name);
    }

    // (armed a moment after the list changes, once `command` has the new
    // list: started in the same instant, it could run with the old one)
    property bool armed: false
    KeptProcess {
        id: proc
        wanted: root.armed
        command: [root.binary, "--surfaces", root.wanted.join(",")]
        stdinEnabled: true // (it ends when this closes: the shell quit)
        onRunningChanged: if (!running)
            root.drawn = []
        stdout: SplitParser {
            onRead: line => root.handle(line)
        }
    }
    // A new list: start it again with that one (what's drawn waits for it to
    // say). An empty list just stops it (a restart then would leave it
    // waiting out a back-off the next time it's wanted).
    onWantedChanged: Qt.callLater(() => {
        const on = root.wanted.length > 0;
        if (on && root.armed && proc.running) {
            root.drawn = [];
            proc.restart();
        }
        root.armed = on;
    })

    function handle(line) {
        let e;
        try {
            e = JSON.parse(line);
        } catch (x) {
            return;
        }
        if (e.src !== "ui")
            return;
        if (e.kind === "ready")
            drawn = e.surfaces ?? [];
        else if (e.kind === "unknown")
            console.info("tessera-ui: no " + e.surface + " in Rust yet: this shell draws it");
    }
}
