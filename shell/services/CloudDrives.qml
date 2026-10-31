pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Google Drive and OneDrive as folders (~/Google Drive, ~/OneDrive) with
// rclone (assets/scripts/cloud-drive.py). Connected drives are mounted again
// at every login.
Singleton {
    id: root

    readonly property string script: Quickshell.shellDir + "/assets/scripts/cloud-drive.py"
    property var info: ({})
    readonly property bool hasRclone: info.rclone ?? false
    function drive(name) {
        return info.drives?.[name] ?? { connected: false, mounted: false, folder: "" };
    }
    readonly property var names: ({ gdrive: "Google Drive", onedrive: "OneDrive" })

    function refresh() {
        statusProc.running = true;
    }
    property bool first: true
    Process {
        id: statusProc
        command: ["python3", "-I", root.script, "status"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.info = JSON.parse(text);
                } catch (e) {}
                // At login: connected drives come back
                if (root.first) {
                    root.first = false;
                    for (const n of ["gdrive", "onedrive"])
                        if (root.drive(n).connected && !root.drive(n).mounted)
                            root.act("mount", n);
                }
            }
        }
    }
    Component.onCompleted: refresh()

    // One job at a time (the rest wait their turn): connect (the browser
    // opens) | mount | unmount | disconnect
    property string busy: "" // "<job> <drive>"
    property var waiting: [] // ["<job> <drive>"]
    property var errors: ({}) // drive -> message
    function act(job, name) {
        if (job_.running || busy !== "") {
            if (busy !== job + " " + name && !waiting.includes(job + " " + name))
                waiting = waiting.concat([job + " " + name]);
            return;
        }
        busy = job + " " + name;
        const e = Object.assign({}, errors);
        delete e[name];
        errors = e;
        job_.command = ["python3", "-I", script, job, name];
        job_.running = true;
    }
    // Connecting waits for the browser (up to 5 minutes): this stops waiting
    function cancel() {
        if (job_.running && busy.startsWith("connect "))
            job_.signal(15);
    }
    function next() {
        if (waiting.length === 0)
            return;
        const [job, name] = waiting[0].split(" ");
        waiting = waiting.slice(1);
        act(job, name);
    }
    Process {
        id: job_
        stdout: StdioCollector {
            onStreamFinished: {
                let r = {};
                try {
                    r = JSON.parse(text);
                } catch (e) {}
                const name = root.busy.split(" ")[1];
                if (!r.ok && r.error !== "") { // ("": cancelled)
                    const e = Object.assign({}, root.errors);
                    e[name] = r.error || "That didn't work. Try again.";
                    root.errors = e;
                }
                root.busy = "";
                root.refresh();
                root.next();
            }
        }
    }

    function open(name) {
        Files.openFolder(drive(name).folder);
    }

    // rclone (and FUSE, for the folders) through Software Update's job
    readonly property bool installing: Updates.running && installAsked
    property bool installAsked: false
    property string installError: ""
    function install() {
        if (Updates.running)
            return;
        installAsked = true;
        installError = "";
        Updates.update(["--install", (info.fuse ? ["rclone"] : ["rclone", "fuse3"]).join(",")]);
    }
    Connections {
        target: Updates
        function onFinished(j) {
            if (!root.installAsked)
                return;
            root.installAsked = false;
            root.installError = j.state === "done" ? "" : (j.error || j.text || "The install didn't finish.");
            root.refresh();
        }
    }
}
