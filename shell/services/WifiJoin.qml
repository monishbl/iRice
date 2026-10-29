pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Joining the Wi-Fi networks Quickshell's own API can't: enterprise
// (802.1X: eduroam, office networks, with their certificates) and hidden
// ones. The work is in assets/scripts/wifi-enterprise.py, through
// NetworkManager. The settings, passwords included, go to it on stdin, never
// on a command line other programs could read. Here, not in the Wi-Fi page,
// so a join carries on when the menu that started it closes.
Singleton {
    id: root

    property string ssid: "" // the network being joined (or last tried)
    property string stage: "" // "" | saving | connecting | connected | error
    property string error: "" // what went wrong and how to fix it
    readonly property bool busy: joinProc.running
    function joining(name) {
        return busy && ssid === name;
    }
    function failedFor(name) {
        return stage === "error" && ssid === name ? error : "";
    }

    // req: see wifi-enterprise.py (ssid, hidden, security, eap, phase2,
    // identity, anonymous, domain, ca, clientCert, privateKey, password,
    // keyPassword, psk)
    function join(req) {
        if (joinProc.running)
            return false;
        ssid = req.ssid ?? "";
        stage = "saving";
        error = "";
        joinProc.request = JSON.stringify(req);
        joinProc.running = true;
        return true;
    }
    function dismiss() {
        if (!busy) {
            stage = "";
            error = "";
        }
    }

    readonly property string script: Quickshell.shellDir + "/assets/scripts/wifi-enterprise.py"
    Process {
        id: joinProc
        property string request: ""
        command: ["python3", "-I", root.script, "join"]
        stdinEnabled: true
        onStarted: {
            write(request + "\n");
            request = ""; // (the password isn't kept once it's sent)
            stdinEnabled = false;
        }
        stdout: SplitParser {
            onRead: line => {
                try {
                    const m = JSON.parse(line);
                    root.stage = m.state ?? root.stage;
                    if (m.state === "error")
                        root.error = m.message ?? "Couldn't join the network.";
                } catch (e) {}
            }
        }
        onExited: code => {
            stdinEnabled = true;
            if (code !== 0 && root.stage !== "error") {
                root.stage = "error";
                root.error = "Couldn't join the network. Check that python3 and NetworkManager are installed.";
            }
        }
    }

    // A saved profile's settings (no secrets), to fill the form in again:
    // cb gets { exists, eap, phase2, identity, anonymous, domain, caFile,
    // systemCa, clientCert, privateKey, keyMgmt, hidden }
    function saved(name, cb) {
        if (showProc.running)
            return;
        showProc.cb = cb;
        showProc.command = ["python3", "-I", root.script, "show", name];
        showProc.running = true;
    }
    Process {
        id: showProc
        property var cb: null
        stdout: StdioCollector {
            onStreamFinished: {
                let r = { exists: false };
                try {
                    r = JSON.parse(text.trim().split("\n").pop() || "{}");
                } catch (e) {}
                const cb = showProc.cb;
                showProc.cb = null;
                if (cb)
                    cb(r);
            }
        }
    }

    // Saved networks, in range or not (Settings → Network → Known networks),
    // and forgetting one: every saved profile for that name goes
    property var knownNetworks: [] // [{ ssid, name }]
    property string forgetError: ""
    function loadKnown() {
        if (!knownProc.running)
            knownProc.running = true;
    }
    function forget(name) {
        if (forgetProc.running || !name)
            return;
        forgetError = "";
        forgetProc.command = ["python3", "-I", root.script, "forget", name];
        forgetProc.running = true;
    }
    Process {
        id: knownProc
        command: ["python3", "-I", root.script, "known"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.knownNetworks = JSON.parse(text.trim().split("\n").pop() || "{}").known ?? [];
                } catch (e) {}
            }
        }
    }
    Process {
        id: forgetProc
        stdout: SplitParser {
            onRead: line => {
                try {
                    const m = JSON.parse(line);
                    if (m.state === "error")
                        root.forgetError = m.message ?? "Couldn't forget the network.";
                } catch (e) {}
            }
        }
        onExited: code => {
            if (code !== 0 && root.forgetError === "")
                root.forgetError = "Couldn't forget the network. Check that NetworkManager is running.";
            root.loadKnown();
        }
    }
}
