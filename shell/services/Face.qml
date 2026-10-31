pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Face unlock for the lock screen only (assets/scripts/face.py): Howdy
// (howdy-next, from the AUR) with the laptop's infrared camera, next to the
// password and the fingerprint. Offered only with an IR camera: a photo or
// a screen can't fool one the way it fools a plain webcam. Setting it up,
// adding and removing faces go through iRice's root helper (the password).
Singleton {
    id: root

    readonly property string script: Quickshell.shellDir + "/assets/scripts/face.py"
    property var info: ({})
    readonly property var camera: info.camera ?? null
    readonly property bool hasCamera: camera !== null
    readonly property bool installed: (info.howdy ?? false) && (info.pam ?? false)
    readonly property bool setUp: installed && (info.device ?? "") !== "" && (info.models ?? false)
    // Faces added (kept here: Howdy's own list is root's to read)
    readonly property int faces: Config.options.lock.faceCount ?? 0
    readonly property bool lockScreen: setUp && faces > 0 && (Config.options.lock.face ?? true)

    function refresh() {
        statusProc.running = true;
    }
    Process {
        id: statusProc
        command: ["python3", "-I", root.script, "status"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.info = JSON.parse(text);
                } catch (e) {}
            }
        }
    }
    Component.onCompleted: refresh()

    // ── Installing howdy-next (AUR, through Software Update) ────────────
    readonly property bool installing: Updates.running && installAsked
    property bool installAsked: false
    property string installError: ""
    function install() {
        if (Updates.running || installed)
            return;
        installAsked = true;
        installError = "";
        Updates.update(["--aur-install", "howdy-next"]);
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

    // ── Root jobs: setup, add, clear (one password each) ────────────────
    property string busy: "" // setup | add | clear
    property string error: ""
    function run(job) {
        if (rootJob.running)
            return;
        error = "";
        if (Tools.ready && !Tools.has("pkexec")) {
            error = "Changing this needs polkit (pkexec), which isn't installed. Install polkit, then try again.";
            return;
        }
        busy = job;
        rootJob.command = ["pkexec", "/usr/lib/irice/irice-root", "face"].concat(job === "setup" ? ["setup", camera.path] : [job]);
        rootJob.running = true;
    }
    function setUpCamera() {
        if (camera)
            run("setup");
    }
    function addFace() {
        run("add");
    }
    function clearFaces() {
        run("clear");
    }
    Process {
        id: rootJob
        stdout: StdioCollector { id: jobOut }
        stderr: StdioCollector { id: jobErr }
        onExited: code => {
            const job = root.busy;
            root.busy = "";
            if (code === 0) {
                if (job === "add")
                    Config.options.lock.faceCount = root.faces + 1;
                else if (job === "clear")
                    Config.options.lock.faceCount = 0;
            } else if (code !== 126) { // (126: the password prompt was cancelled)
                const said = (jobErr.text + "\n" + jobOut.text).trim().split("\n").filter(l => l.trim() !== "").pop() ?? "";
                root.error = code === 127 ? "Not allowed (an administrator's password is needed)."
                    : job === "add" ? "No face was found. Look at the camera, then try again. If the camera's infrared light stays off, it needs linux-enable-ir-emitter (AUR) set up once in a terminal: sudo linux-enable-ir-emitter configure." + (said ? " (" + said.replace(/^irice-root: /, "") + ")" : "")
                    : said.replace(/^irice-root: /, "") || "That didn't work. Update iRice, then try again.";
            }
            root.refresh();
        }
    }
}
