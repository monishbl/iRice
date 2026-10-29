pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Google Calendar and Tasks, through assets/scripts/google-account.py: one
// request per run, JSON in on stdin, JSON lines back (the account's state,
// errors, "the change went through").
//
// Changes to tasks never wait on a sync or get turned away while one runs:
// they show at once (`overrides`), queue, and go out as soon as the helper
// is free; afterwards only that task list is fetched again. Deleting hides
// the task first and sends the deletion once its Undo (8 s) has passed, so
// Undo just shows it again. Two changes to the same task from two places
// (here and the phone) are last-writer-wins: Google keeps whichever came last.
Singleton {
    id: root
    property var account: ({}) // the helper's last report: connection, calendars' events, tasks
    readonly property bool connected: account.connected ?? false
    readonly property bool configured: account.configured ?? false
    readonly property bool busy: helper.running
    property string lastError: ""
    property string operation: ""
    property var pending: ({})
    signal mutationComplete()
    readonly property var taskLists: account.taskLists ?? []

    // Shown before Google has them: key -> { hidden } | { patch }
    property var overrides: ({})
    function keyOf(t) {
        return "google/" + t.listId + "/" + t.id;
    }
    readonly property var tasks: (account.tasks ?? []).map(t => {
        const key = keyOf(t);
        const o = overrides[key];
        if (o?.hidden)
            return null;
        const p = t.date ? t.date.split("-").map(Number) : [];
        return Object.assign({}, t, { key: key, google: true, allDay: true,
            due: p.length ? new Date(p[0], p[1] - 1, p[2], 23, 59).getTime() : 0 }, o?.patch ?? {});
    }).filter(t => t !== null)
    readonly property var events: account.events ?? []

    // Account actions (Settings): one at a time, turned away while busy
    function run(request) {
        if (busy || queue.length > 0) {
            lastError = "Google is busy. Wait for the current operation, then try again.";
            return false;
        }
        start(request);
        return true;
    }
    function start(request) {
        lastError = "";
        replied = false;
        pending = request;
        operation = request.cmd;
        helper.running = true;
    }

    // ── Writes: shown at once, queued, sent in order ────────────────────
    property var queue: []
    function write(request, key, override) {
        if (key && override)
            setOverride(key, override);
        queue = queue.concat([{ request: request, key: key ?? "" }]);
        pump();
        return true;
    }
    function pump() {
        if (helper.running || queue.length === 0)
            return;
        const next = queue[0];
        queue = queue.slice(1);
        current = next;
        start(next.request);
    }
    property var current: null // the queued write being sent
    function setOverride(key, o) {
        const m = Object.assign({}, overrides);
        if (o)
            m[key] = o;
        else
            delete m[key];
        overrides = m;
    }

    function addTask(listId, patch) {
        return write({ cmd: "taskAdd", listId: listId, patch: patch });
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
        const key = keyOf(task);
        const shown = Object.assign({}, overrides[key]?.patch ?? {}, patch);
        return write({ cmd: "taskUpdate", listId: task.listId, id: task.id, patch: change }, key, { patch: shown });
    }

    // Deleting: hidden now, sent when its Undo has run out
    property var deleting: [] // [{ task, at }]
    function removeTask(task) {
        setOverride(keyOf(task), { hidden: true });
        deleting = deleting.concat([{ task: task, at: Date.now() }]);
        deleteTimer.start();
    }
    function restoreTask(task) {
        const key = keyOf(task);
        deleting = deleting.filter(d => keyOf(d.task) !== key);
        setOverride(key, null);
    }
    Timer {
        id: deleteTimer
        interval: 1000
        repeat: true
        running: false
        onTriggered: {
            const due = root.deleting.filter(d => Date.now() - d.at >= 8000);
            root.deleting = root.deleting.filter(d => Date.now() - d.at < 8000);
            for (const d of due)
                root.write({ cmd: "taskDelete", listId: d.task.listId, id: d.task.id }, root.keyOf(d.task), { hidden: true });
            if (root.deleting.length === 0)
                stop();
        }
    }

    function openKeep() { Qt.openUrlExternally("https://keep.google.com/"); }
    Component.onCompleted: run({ cmd: "status" })
    Timer {
        interval: 15 * 60 * 1000
        repeat: true
        running: root.connected
        onTriggered: if (!root.busy && root.queue.length === 0) root.run({ cmd: "sync" })
    }
    property bool replied: false // the helper said anything (else it crashed or couldn't start)
    Process {
        id: helper
        command: ["python3", "-I", decodeURIComponent(Qt.resolvedUrl("../assets/scripts/google-account.py").toString().replace("file://", "")), Config.dir + "/google"]
        stdinEnabled: true
        onStarted: {
            helper.write(JSON.stringify(root.pending) + "\n"); // (the Process's own write, not the queue's)
            stdinEnabled = false;
        }
        onExited: code => {
            stdinEnabled = true;
            if (code !== 0 && !root.replied)
                root.lastError = "Google helper failed. Check that python3 and python-gobject are installed.";
            // The write is in Google's data now (or failed, and is shown as
            // Google has it): its override goes either way
            if (root.current?.key)
                root.setOverride(root.current.key, null);
            root.current = null;
            if (root.operation === "status" && root.connected && root.queue.length === 0)
                Qt.callLater(() => root.run({ cmd: "sync" }));
            else
                Qt.callLater(root.pump);
        }
        stdout: SplitParser {
            onRead: line => {
                root.replied = true;
                try {
                    const message = JSON.parse(line);
                    if (message.authUrl && message.authUrl.startsWith("https://accounts.google.com/o/oauth2/v2/auth?"))
                        Qt.openUrlExternally(message.authUrl);
                    if (message.data) {
                        root.account = message.data;
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
