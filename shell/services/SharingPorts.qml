pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// The firewall's way in for nearby sharing: open only while something needs
// it, closed otherwise (never for good). Needed while you're visible to
// others (Quick Share: its ports, DHCP for its hotspot, mDNS; LocalSend
// receiving: 53317), while the share sheet looks for devices (their answers
// come in: LocalSend's 53317, mDNS), and while a QR-code share or a cast
// serves a file (53319-53320). Sending itself needs nothing open: the
// connection goes out. sharing-check.py `ports` asks Tessera's root helper
// (`tessera-root sharing-ports open <sets>|close`: fixed ports, no password
// for an administrator at this computer); with no firewall on, or nothing
// changed, it asks nothing. Opening goes at once, closing a few seconds
// later (a quick off and on asks once), and once at startup.
// Never in the dev sandbox: it shares the host's system bus and pkexec, so
// it would change the real firewall.
Singleton {
    id: root

    readonly property bool sandbox: (Quickshell.env("TESSERA_SANDBOX") || Quickshell.env("IRICE_SANDBOX")) === "1"
    readonly property string script: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/sharing-check.py").toString().replace("file://", ""))

    // What's needed now: the root helper's set names (sorted, joined), and why
    readonly property var need: {
        const sets = [], why = [];
        const add = (names, reason) => {
            for (const n of names)
                if (!sets.includes(n))
                    sets.push(n);
            why.push(reason);
        };
        if (QuickShare.visible || LocalSend.receiving)
            add((QuickShare.visible ? ["quickshare", "mdns"] : []).concat(LocalSend.receiving ? ["localsend"] : []), "visible to Everyone");
        if (Sharing.looking)
            add(["localsend", "mdns"], "looking for devices");
        if (LinkShare.stage === "loading" || LinkShare.stage === "ready")
            add(["links"], "sharing with a QR code");
        if (Cast.servesFile)
            add(["links"], "casting a file");
        return { sets: sets.sort().join(","), why: why.join(", ") };
    }

    property string asked: "?" // what's open now, as last asked ("" closed; "?" not asked yet)
    property string error: ""
    // The last ask's exit code when it failed: 64 an older helper (until an
    // update), 126 cancelled, 127 not allowed (until Try again); anything
    // else, until what's needed changes
    property int failedWith: 0
    readonly property bool held: failedWith === 64 || failedWith === 126 || failedWith === 127
    readonly property bool busy: job.running

    onNeedChanged: {
        if (!held)
            failedWith = 0;
        schedule();
    }
    Component.onCompleted: schedule()
    function schedule() {
        if (sandbox || failedWith !== 0 || job.running)
            return;
        if (need.sets === asked) {
            later.stop();
            return;
        }
        const now = asked === "?" ? [] : asked.split(",");
        later.interval = need.sets.split(",").some(s => s !== "" && !now.includes(s)) ? 300 : 5000;
        later.restart();
    }
    Timer {
        id: later
        onTriggered: root.ask()
    }
    function ask() {
        if (sandbox || job.running)
            return;
        job.sets = need.sets;
        job.command = ["python3", "-I", script, "ports", need.sets || "none", need.why];
        job.running = true;
    }
    // (Settings' Try again)
    function retry() {
        failedWith = 0;
        error = "";
        ask();
    }
    Process {
        id: job
        property string sets: "" // (what it asks for)
        stdout: StdioCollector { id: jobOut }
        onExited: code => {
            let r = {};
            try {
                r = JSON.parse(jobOut.text);
            } catch (e) {}
            if (code === 0) {
                root.asked = job.sets;
                root.error = "";
            } else {
                root.failedWith = code;
                root.error = (job.sets ? "Sharing's ports didn't open. " : "Sharing's ports didn't close. ")
                    + (r.error || (code === 64 ? "Tessera's system helper is from before this. Run Software Update once." : "Try again."));
            }
            root.schedule(); // (what's needed may have changed meanwhile)
        }
    }
    // An update installs the newer helper
    Connections {
        target: Updates
        function onFinished(j) {
            if (root.failedWith === 64) {
                root.failedWith = 0;
                root.error = "";
                root.schedule();
            }
        }
    }
}
