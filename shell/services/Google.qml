pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

Singleton {
    id: root
    property var data: ({})
    readonly property bool connected: data.connected ?? false
    readonly property bool configured: data.configured ?? false
    readonly property bool busy: helper.running
    property string lastError: ""
    property string operation: ""
    property var pending: ({})
    signal mutationComplete()
    readonly property var taskLists: data.taskLists ?? []
    readonly property var tasks: (data.tasks ?? []).map(t => {
        const p = t.date ? t.date.split("-").map(Number) : [];
        return Object.assign({}, t, { google: true, allDay: true,
            due: p.length ? new Date(p[0], p[1] - 1, p[2], 23, 59).getTime() : 0 });
    })
    readonly property var events: data.events ?? []

    function run(request) {
        if (busy) {
            lastError = "Google is busy. Wait for the current operation, then try again.";
            return false;
        }
        lastError = "";
        pending = request;
        operation = request.cmd;
        helper.running = true;
        return true;
    }
    function updateTask(task, patch) {
        const change = Object.assign({}, patch);
        if (patch.due !== undefined) {
            if (patch.due && patch.allDay === false) {
                lastError = "Google Tasks supports due dates, not reminder times. Choose an all-day date.";
                return false;
            }
            change.date = patch.due ? Qt.formatDate(new Date(patch.due), "yyyy-MM-dd") : "";
        }
        return run({ cmd: "taskUpdate", listId: task.listId, id: task.id, patch: change });
    }
    function openKeep() { Qt.openUrlExternally("https://keep.google.com/"); }
    Component.onCompleted: run({ cmd: "status" })
    Timer {
        interval: 15 * 60 * 1000
        repeat: true
        running: root.connected
        onTriggered: if (!root.busy) root.run({ cmd: "sync" })
    }
    Process {
        id: helper
        command: ["python3", "-I", decodeURIComponent(Qt.resolvedUrl("../assets/scripts/google-account.py").toString().replace("file://", "")), Config.dir + "/google"]
        stdinEnabled: true
        onStarted: {
            write(JSON.stringify(root.pending) + "\n");
            stdinEnabled = false;
        }
        onExited: {
            stdinEnabled = true;
            if (root.operation === "status" && root.connected)
                Qt.callLater(() => root.run({ cmd: "sync" }));
        }
        stdout: SplitParser {
            onRead: line => {
                try {
                    const message = JSON.parse(line);
                    if (message.authUrl && message.authUrl.startsWith("https://accounts.google.com/o/oauth2/v2/auth?"))
                        Qt.openUrlExternally(message.authUrl);
                    if (message.data) {
                        root.data = message.data;
                        if (["disconnect", "forget"].includes(root.operation) && !root.connected)
                            Config.options.google.taskListId = "";
                    }
                    if (message.mutationComplete)
                        root.mutationComplete();
                    if (message.error) {
                        root.lastError = message.error;
                        if (root.operation !== "sync" && root.operation !== "status")
                            Quickshell.execDetached(["notify-send", "-a", "iRice", "Google", message.error]);
                    }
                } catch (e) { root.lastError = "Could not read the Google helper response."; }
            }
        }
    }
}
