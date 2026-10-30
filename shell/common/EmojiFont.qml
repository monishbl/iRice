pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// The emoji style the shell draws with (Settings → Appearance → Emoji):
// Android (Noto Color Emoji), Apple (Apple Color Emoji) or Windows (Microsoft
// Fluent Emoji). Installed fonts are found once with fc-list; a style whose
// font isn't installed falls back to Noto, and Settings offers to install it.
Singleton {
    id: root

    readonly property var styles: [
        { id: "android", label: "Android", pkg: "noto-fonts-emoji", test: /^noto color emoji$/i },
        { id: "apple", label: "Apple", pkg: "ttf-apple-emoji", test: /^apple color emoji$/i },
        { id: "windows", label: "Windows", pkg: "ttf-fluent-emoji", test: /^fluent ?emoji ?color$/i }
    ]
    property var installed: ({}) // style id -> its font family
    property var files: ({}) // style id -> its font file
    property var atStart: null // what the first scan found
    readonly property string style: Config.options.emoji?.style ?? "android"
    readonly property bool available: installed[style] !== undefined
    // Qt reads the installed fonts once, at start: a font installed since
    // (from Settings) is loaded from its file so it draws without a restart
    readonly property bool lateFont: available && atStart !== null && atStart[style] === undefined && files[style] !== undefined
    readonly property string family: lateFont ? (late.status === FontLoader.Ready ? late.name : (installed.android ?? "Noto Color Emoji")) : (installed[style] ?? installed.android ?? "Noto Color Emoji")
    FontLoader {
        id: late
        source: root.lateFont ? "file://" + root.files[root.style].split("/").map(encodeURIComponent).join("/") : ""
    }
    function packageFor(id) {
        return styles.find(s => s.id === id)?.pkg ?? "";
    }

    function refresh() {
        scan.running = true;
    }
    // Installs a style's font from the AUR in a terminal, then looks again
    // every few seconds for two minutes, so Settings updates when it's done
    function install(id) {
        const pkg = packageFor(id);
        if (pkg === "")
            return;
        // (yay, else paru: whichever AUR helper is installed)
        Launch.terminal(["sh", "-c", "h=$(command -v yay || command -v paru) || { echo 'Install yay or paru first (an AUR helper), then try again.'; read -p 'Press Enter to close' _; exit 1; }; \"$h\" -S --needed \"$1\" && fc-cache -f; echo; read -p 'Done. Press Enter to close' _", "sh", pkg], false);
        recheck.left = 24;
        recheck.restart();
    }
    Timer {
        id: recheck
        property int left: 0
        interval: 5000
        repeat: true
        onTriggered: {
            root.refresh();
            if (--left <= 0)
                stop();
        }
    }
    Process {
        id: scan
        running: true
        command: ["fc-list", ":", "family", "file"]
        stdout: StdioCollector {
            onStreamFinished: {
                const found = {}, files = {};
                for (const line of text.split("\n")) {
                    // "/path/Font.ttf: Family,Other name"
                    const sep = line.lastIndexOf(": ");
                    if (sep < 0)
                        continue;
                    for (const fam of line.slice(sep + 2).split(",")) {
                        const f = fam.trim();
                        for (const s of root.styles)
                            if (found[s.id] === undefined && s.test.test(f)) {
                                found[s.id] = f;
                                files[s.id] = line.slice(0, sep);
                            }
                    }
                }
                root.files = files;
                root.installed = found;
                if (root.atStart === null)
                    root.atStart = found;
            }
        }
    }
}
