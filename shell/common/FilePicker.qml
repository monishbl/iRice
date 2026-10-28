pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Asks for a file or folder (assets/scripts/pick-file.py: zenity, kdialog or the
// desktop portal). One at a time; the callback gets the path, or nothing
// when the user cancels.
//   FilePicker.pick({ folder: false, title: "…", start: "~/Pictures", filter: "Images|*.jpg *.png" }, path => …)
//   FilePicker.pick({ multiple: true, title: "Send files" }, paths => …)
Singleton {
    id: root

    readonly property bool busy: proc.running
    property var callback: null
    property bool multiple: false

    function pick(opts, cb) {
        if (proc.running)
            return;
        const o = opts || {};
        const home = Quickshell.env("HOME");
        const cmd = ["python3", Quickshell.shellDir + "/assets/scripts/pick-file.py"];
        if (o.folder)
            cmd.push("--folder");
        multiple = o.multiple === true && !o.folder;
        if (multiple)
            cmd.push("--multiple");
        if (o.title)
            cmd.push("--title", o.title);
        if (o.start)
            cmd.push("--start", String(o.start).replace(/^~/, home));
        if (o.filter)
            cmd.push("--filter", o.filter);
        callback = cb;
        proc.command = Launch.wrap(cmd);
        proc.running = true;
    }

    Process {
        id: proc
        stdout: StdioCollector { id: out }
        onExited: code => {
            const cb = root.callback;
            root.callback = null;
            const path = out.text.trim();
            if (code === 0 && path !== "" && cb) {
                if (root.multiple) {
                    try {
                        const paths = JSON.parse(path);
                        if (Array.isArray(paths) && paths.length > 0)
                            cb(paths);
                    } catch (e) {}
                } else {
                    cb(path);
                }
            }
        }
    }
}
