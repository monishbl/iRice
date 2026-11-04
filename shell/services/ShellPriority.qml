pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Keeping the shell smooth when apps are busy: ananicy-cpp's rule that runs
// it as an interactive process (Tessera's root helper: `tessera-root priority
// on|off`, no password for an administrator at this computer). Only where
// ananicy-cpp is installed; off stays off through updates.
Singleton {
    id: root

    property bool available: false // ananicy-cpp is installed
    property bool on: false
    property bool busy: false
    property string error: ""

    Component.onCompleted: refresh()
    function refresh() {
        if (!check.running)
            check.running = true;
    }
    Process {
        id: check
        // (irice.rules: installed before iRice became Tessera, until the root setup replaces it)
        command: ["sh", "-c", "test -d /etc/ananicy.d || exit 2; test -f /etc/ananicy.d/tessera.rules || test -f /etc/ananicy.d/irice.rules"]
        onExited: code => {
            root.available = code !== 2;
            root.on = code === 0;
        }
    }

    function set(on) {
        if (job.running)
            return;
        error = "";
        if (Tools.ready && !Tools.has("pkexec")) {
            error = "Changing this needs polkit (pkexec), which isn't installed. Install polkit, then try again.";
            return;
        }
        busy = true;
        job.command = RootHelper.command(["priority", on ? "on" : "off"]);
        job.running = true;
    }
    Process {
        id: job
        stderr: StdioCollector { id: jobErr }
        onExited: code => {
            root.busy = false;
            if (code === 64)
                root.error = "Tessera's system helper is from before this setting. Run Software Update once, then try again.";
            else if (code !== 0 && code !== 126)
                root.error = code === 127 ? "Not allowed (an administrator's password is needed)."
                    : (jobErr.text.replace(/^(tessera|irice)-root: /, "").trim() || "That didn't change. Update Tessera, then try again.");
            root.refresh();
        }
    }
}
