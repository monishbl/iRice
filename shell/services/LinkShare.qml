pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Sharing with a phone that has no Quick Share (an iPhone), through its
// browser (assets/scripts/link-share.py): a QR code its camera opens, on
// this computer's own network. Send: the page has the files to save.
// Receive: the page takes photos and files, saved in the Quick Share folder.
// Up only while the card (modules/share/LinkShareCard) shows, at most 10
// minutes; the link holds a secret.
//   LinkShare.send(paths)   LinkShare.receive()   LinkShare.close()
Singleton {
    id: root

    readonly property string script: Quickshell.shellDir + "/assets/scripts/link-share.py"
    readonly property string folder: QuickShare.folder || (Quickshell.env("XDG_DOWNLOAD_DIR") || Quickshell.env("HOME") + "/Downloads")

    property string mode: "" // "" | send | receive
    property string stage: "" // "" | loading | ready | error
    property string url: ""
    property string qr: ""
    property string error: ""
    property var names: [] // what's being sent
    property int count: 0 // files saved by the phone (send) or received
    property real done: 0
    property real total: 0
    readonly property bool busy: stage === "ready" && total > 0 && done < total

    function send(paths) {
        names = paths.map(p => p.split("/").pop());
        start("send", paths);
    }
    function receive() {
        names = [];
        start("receive", [folder]);
    }
    function start(m, args) {
        close();
        mode = m;
        stage = "loading";
        Panels.close();
        job = jobComp.createObject(root, { command: ["python3", "-I", script, "--name", QuickShare.name, m].concat(args) });
        job.running = true;
    }
    property Process job: null
    function close() {
        if (job) {
            // (stopped, it removes its code and stops serving)
            const old = job;
            job = null;
            old.running = false;
            old.destroy(1000);
        }
        mode = "";
        stage = "";
        url = "";
        qr = "";
        error = "";
        count = 0;
        done = 0;
        total = 0;
        flush();
    }

    // What arrived, in one alert per moment rather than one per file
    property var arrived: []
    Timer {
        id: gather
        interval: 1500
        onTriggered: root.flush()
    }
    function flush() {
        const paths = arrived;
        if (paths.length === 0)
            return;
        arrived = [];
        Sounds.play("received");
        const first = paths[0];
        Island.post({ kind: "custom", icon: "check-circle", title: paths.length === 1 ? "Received “" + first.split("/").pop() + "”" : "Received " + paths.length + " files",
            subtitle: "From a phone's browser", color: Theme.accents.green,
            action: paths.length === 1 ? "Open" : "Show in Folder", onAction: () => paths.length === 1 ? Files.open(first) : Files.reveal(first),
            action2: paths.length === 1 ? "Show in Folder" : "", onAction2: () => Files.reveal(first) }, 8000);
    }

    Component {
        id: jobComp
        Process {
            id: proc
            stdinEnabled: true
            stdout: SplitParser {
                onRead: line => {
                    let ev;
                    try {
                        ev = JSON.parse(line);
                    } catch (e) {
                        return;
                    }
                    if (proc !== root.job)
                        return; // (one that was closed)
                    switch (ev.ev) {
                    case "ready":
                        root.url = ev.url;
                        root.qr = ev.qr;
                        root.stage = "ready";
                        break;
                    case "progress":
                        root.done = ev.done;
                        root.total = ev.total;
                        break;
                    case "sent":
                        root.count++;
                        break;
                    case "received":
                        root.count += ev.paths.length;
                        root.arrived = root.arrived.concat(ev.paths);
                        gather.restart();
                        break;
                    case "error":
                        root.error = ev.message;
                        root.stage = "error";
                        break;
                    case "closed":
                        if (ev.why === "expired" && root.stage === "ready") {
                            root.error = "The code expired after 10 minutes. Make a new one to share again.";
                            root.stage = "error";
                        }
                        break;
                    }
                }
            }
            onExited: code => {
                if (proc === root.job && root.stage === "loading") {
                    root.error = root.error || "Sharing couldn't start. Check that Python 3 is installed, then try again.";
                    root.stage = "error";
                }
            }
        }
    }

    IpcHandler {
        target: "linkshare"
        // (opens the card that lets a phone's browser send files here)
        function receive(): void {
            root.receive();
        }
        function close(): void {
            root.close();
        }
        // The dev sandbox only: the card in a state, nothing served
        // (qs ipc call linkshare demo ready|loading|error|busy <qr png>)
        function demo(stage: string, qr: string): void {
            if ((Quickshell.env("TESSERA_SANDBOX") || Quickshell.env("IRICE_SANDBOX")) !== "1")
                return;
            root.mode = "send";
            root.names = ["Holiday.jpg"];
            root.qr = qr;
            root.error = stage === "error" ? "Making the code needs qrencode." : "";
            root.count = stage === "busy" ? 1 : 0;
            root.total = stage === "busy" ? 100 : 0;
            root.done = stage === "busy" ? 40 : 0;
            root.stage = stage === "busy" ? "ready" : stage;
        }
    }
}
