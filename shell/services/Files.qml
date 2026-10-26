pragma Singleton
import QtQuick
import Quickshell
import qs.common

// Opening files and folders. Folders and the Trash open in a file manager
// (Settings → Desktop → Files app; Automatic picks the first installed of
// Nautilus, Dolphin, Thunar, Nemo...), never in whatever app has claimed
// folders system-wide (often a terminal). Files open in their default app.
Singleton {
    id: root

    readonly property var managers: [
        { bin: "nautilus", name: "Files (Nautilus)" }, { bin: "dolphin", name: "Dolphin" }, { bin: "thunar", name: "Thunar" },
        { bin: "nemo", name: "Nemo" }, { bin: "caja", name: "Caja" }, { bin: "pcmanfm-qt", name: "PCManFM-Qt" }, { bin: "pcmanfm", name: "PCManFM" }
    ]
    readonly property var installed: managers.filter(m => Tools.has(m.bin))
    readonly property string manager: {
        const want = Config.options.desktop.fileManager;
        return want && Tools.has(want) ? want : (installed[0]?.bin ?? "");
    }

    function q(s) {
        return "'" + String(s).replace(/'/g, "'\\''") + "'";
    }
    // A folder in the file manager
    function openFolder(path) {
        if (manager)
            Launch.run([manager, path]);
        else
            Launch.open("file://" + path);
    }
    // A folder in the file manager, anything else in its app
    function open(path) {
        Launch.run(["sh", "-c", "if [ -d \"$1\" ]; then exec " + (manager ? q(manager) : "xdg-open") + " \"$1\"; else exec xdg-open \"$1\"; fi", "sh", path]);
    }
    // The folder it's in, with it selected where the file manager can
    function reveal(path) {
        const dir = path.replace(/\/[^/]*$/, "") || "/";
        if (manager === "nautilus" || manager === "dolphin" || manager === "nemo")
            Launch.run([manager, "--select", path]);
        else
            openFolder(dir);
    }
    function openTrash() {
        if (manager)
            Launch.run([manager, manager === "dolphin" ? "trash:/" : "trash:///"]);
        else
            Launch.open("trash:///");
    }
}
