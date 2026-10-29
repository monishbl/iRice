pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.common

// Google Calendar and Tasks, through assets/scripts/google-account.py: one
// request per run, JSON in on stdin, JSON lines back (the account's state,
// errors, "the change went through").
//
// Several accounts can be connected; their calendars and task lists show
// together (each item carries its `account`, shown in its details and in
// Settings, never as groups).
//
// Changes to tasks and events never wait on a sync or get turned away while
// one runs: they show at once (`overrides`), queue, and go to the helper as
// soon as it's free. The helper writes each to its outbox on disk before
// sending it, so a change made offline (or just before a restart) is kept,
// shown as made, and sent when Google can be reached again (tried again
// after 30 s, then less often, and before every sync). Deleting hides the
// item first and sends the deletion once its Undo (8 s) has passed, so Undo
// just shows it again. Edits carry the version they were made against: if
// it changed on Google meanwhile (the phone), Google's version is kept and
// a notification says what you had, with Copy.
//
// Syncs are incremental (only what changed; a few hundred bytes when nothing
// did), and happen when they're useful, not on a clock alone: at login, on
// unlock or resume (the morning Today screen opens then too), when
// Notification Center opens and the last sync is over 2 minutes old, and
// every 15 minutes while you're using the computer. Never while it's locked
// or idle. After a failure the next try waits longer (1 minute, then up to
// 2 hours); Sync now in Settings always goes.
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
    // Connected accounts: [{ id, identity, backend, syncedAt }]
    readonly property var accounts: account.accounts ?? []
    readonly property int outbox: account.outbox ?? 0 // changes not on Google yet
    function identity(aid) {
        return accounts.find(a => a.id === aid)?.identity ?? "";
    }

    // Shown before Google has them: key -> { hidden } | { patch }
    property var overrides: ({})
    function keyOf(t) {
        return "google/" + t.account + "/" + t.listId + "/" + t.id;
    }
    function eventKey(e) {
        return "google:" + e.account + ":" + e.calendarId + ":" + e.id;
    }
    // A task list or calendar in Settings: "<account>|<id>" (a bare id, from
    // before several accounts, means the first account that has it)
    function find(rows, ref) {
        if (!ref)
            return null;
        const bar = ref.indexOf("|");
        return bar < 0 ? rows.find(r => r.id === ref) ?? null
            : rows.find(r => r.account === ref.slice(0, bar) && r.id === ref.slice(bar + 1)) ?? null;
    }
    function shown(c) {
        const hidden = Config.options.google.hiddenCalendars ?? [];
        return !hidden.includes(c.account + "|" + (c.calendarId ?? c.id)) && !hidden.includes(c.calendarId ?? c.id);
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
    // Calendars switched off in Settings → Google stay synced (it's cheap)
    // and are just not shown
    readonly property var events: (account.events ?? []).filter(e => shown(e)).map(e => {
        const key = eventKey(e);
        const o = overrides[key];
        if (o?.hidden)
            return null;
        return Object.assign({}, e, { key: key, accountName: identity(e.account) }, o?.patch ?? {});
    }).filter(e => e !== null)
    readonly property var calendars: account.calendars ?? []
    readonly property real syncedAt: account.syncedAt ?? 0

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

    function addTask(listRef, patch) {
        const list = find(taskLists, listRef);
        if (!list) {
            lastError = "That task list isn't available. Choose another in Settings → Google.";
            return false;
        }
        return write({ cmd: "taskAdd", account: list.account, listId: list.id, patch: patch });
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
        return write({ cmd: "taskUpdate", account: task.account, listId: task.listId, id: task.id, etag: task.etag ?? "",
            title: task.text, patch: change }, key, { patch: shown });
    }

    // ── Events ──────────────────────────────────────────────────────────
    // Fields: { title, start, end (ms), allDay, location, notes }, any subset
    // when editing. A repeating event changes this occurrence (`every`
    // false) or the whole series (true; a new time moves every occurrence by
    // as much as this one moved).
    function when(ms, allDay) {
        return allDay ? { date: Qt.formatDate(new Date(ms), "yyyy-MM-dd") } : { dateTime: new Date(ms).toISOString() };
    }
    function shownFields(f) {
        const out = {};
        for (const k of ["title", "location", "notes"])
            if (f[k] !== undefined)
                out[k] = f[k];
        if (f.start !== undefined) {
            out.start = when(f.start, f.allDay);
            out.end = when(f.end ?? f.start, f.allDay);
        }
        return out;
    }
    function startOf(e) {
        const x = e.start;
        return x.dateTime ? new Date(x.dateTime).getTime() : new Date(Number(x.date.slice(0, 4)), Number(x.date.slice(5, 7)) - 1, Number(x.date.slice(8, 10))).getTime();
    }
    function addEvent(calendarRef, fields) {
        const cal = find(calendars, calendarRef);
        if (!cal) {
            lastError = "That calendar isn't available. Choose another one.";
            return false;
        }
        if (!cal.writable) {
            lastError = "This calendar is read-only for your account. Choose a calendar you own.";
            return false;
        }
        return write({ cmd: "eventInsert", account: cal.account, calendarId: cal.id, title: fields.title, fields: fields });
    }
    // `e` is a Google event as `events` has it
    function updateEvent(e, fields, every) {
        if (!e.writable) {
            lastError = "This calendar is read-only for your account. Change it in a calendar you own, or ask its owner for edit access.";
            return false;
        }
        const request = { cmd: "eventPatch", account: e.account, calendarId: e.calendarId, id: e.id, etag: e.etag ?? "",
            title: e.title, fields: fields };
        if (every && e.series) {
            request.series = e.series;
            request.was = startOf(e);
            // Every occurrence shown changes now too
            const shift = fields.start !== undefined ? fields.start - request.was : 0;
            const length = fields.start !== undefined ? (fields.end ?? fields.start) - fields.start : 0;
            for (const other of (account.events ?? []).filter(x => x.account === e.account && x.series === e.series)) {
                const f = Object.assign({}, fields);
                if (fields.start !== undefined) {
                    f.start = startOf(other) + shift;
                    f.end = f.start + length;
                }
                const key = eventKey(other);
                setOverride(key, { patch: Object.assign({}, overrides[key]?.patch ?? {}, shownFields(f)) });
            }
            return write(request);
        }
        const key = eventKey(e);
        return write(request, key, { patch: Object.assign({}, overrides[key]?.patch ?? {}, shownFields(fields)) });
    }

    // Deleting: hidden now, sent when its Undo has run out
    property var deleting: [] // [{ key, request, at }]
    function hide(key, request) {
        setOverride(key, { hidden: true });
        deleting = deleting.filter(d => d.key !== key).concat([{ key: key, request: request, at: Date.now() }]);
        deleteTimer.start();
    }
    function unhide(key) {
        deleting = deleting.filter(d => d.key !== key);
        setOverride(key, null);
    }
    function removeTask(task) {
        hide(keyOf(task), { cmd: "taskDelete", account: task.account, listId: task.listId, id: task.id, etag: task.etag ?? "", title: task.text });
    }
    function restoreTask(task) {
        unhide(keyOf(task));
    }
    function removeEvent(e, every) {
        if (!e.writable) {
            lastError = "This calendar is read-only for your account.";
            return false;
        }
        const request = { cmd: "eventDelete", account: e.account, calendarId: e.calendarId, id: e.id, etag: e.etag ?? "", title: e.title };
        if (every && e.series) {
            request.series = e.series;
            delete request.etag;
            for (const other of (account.events ?? []).filter(x => x.account === e.account && x.series === e.series && x.id !== e.id))
                setOverride(eventKey(other), { hidden: true });
        }
        hide(eventKey(e), request);
        return true;
    }
    function restoreEvent(e) {
        const d = deleting.find(x => x.key === eventKey(e));
        if (d?.request.series)
            for (const other of (account.events ?? []).filter(x => x.account === e.account && x.series === d.request.series))
                setOverride(eventKey(other), null);
        unhide(eventKey(e));
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
                root.write(d.request, d.key, { hidden: true });
            if (root.deleting.length === 0)
                stop();
        }
    }

    function openKeep() { Qt.openUrlExternally("https://keep.google.com/"); }
    Component.onCompleted: run({ cmd: "status" })

    // ── When to sync ────────────────────────────────────────────────────
    // Connect: one click; the browser opens for a new account, an account
    // already in Online Accounts is used as it is
    function connect() {
        return run({ cmd: "signin" });
    }
    // A sync if the last one is older than `age` ms (and nothing's waiting
    // out a failure)
    function freshen(age) {
        if (!connected || busy || queue.length > 0 || System.locked || Date.now() < retryAt)
            return;
        if (Date.now() - syncedAt >= age)
            run({ cmd: "sync" });
    }
    property int failures: 0
    property real retryAt: 0

    // Changes that couldn't reach Google: tried again after 30 s, then 2,
    // 8 and 30 minutes (and before every sync)
    property int sendFailures: 0
    Timer {
        id: resend
        repeat: false
        interval: Math.min(30 * 60 * 1000, 30 * 1000 * Math.pow(4, Math.max(0, root.sendFailures - 1)))
        onTriggered: {
            if (root.outbox > 0 && !root.busy && root.queue.length === 0)
                root.start({ cmd: "flush" });
            else if (root.outbox > 0)
                restart();
        }
    }

    // Google kept its own version of something you changed: say what you had
    function conflict(c) {
        const why = c.reason === "deleted" ? "it was deleted on Google"
            : c.reason === "changed" ? "it changed on Google" : "Google refused it (" + c.reason + ")";
        const body = "Your change to " + c.title + " couldn't be saved because " + why + ". Here's what you had: " + c.what;
        Quickshell.execDetached(["sh", "-c", '[ "$(notify-send -a iRice --action=copy=Copy "$1" "$2")" = copy ] && printf %s "$3" | wl-copy',
            "sh", "Google change not saved", body, c.what]);
    }
    IdleMonitor {
        id: idle
        timeout: 300
        respectInhibitors: true // (a video playing counts as use)
    }
    Timer {
        interval: 60 * 1000 // looks every minute; syncs when 15 have passed
        repeat: true
        running: root.connected && !System.locked && !idle.isIdle
        onTriggered: root.freshen(15 * 60 * 1000)
    }
    Connections {
        target: System
        function onLockedChanged() {
            if (!System.locked) // unlocked, or woke up (sleep locks first)
                root.freshen(2 * 60 * 1000);
        }
    }
    Connections {
        target: idle
        function onIsIdleChanged() {
            if (!idle.isIdle)
                root.freshen(15 * 60 * 1000);
        }
    }
    Connections {
        target: Panels
        function onOpenChanged() {
            if (Panels.open === "notificationCenter")
                root.freshen(2 * 60 * 1000);
        }
    }

    property bool replied: false // the helper said anything (else it crashed or couldn't start)
    property bool offline: false // the last run couldn't reach Google
    // A sign-in window or the browser is up for signing in (Settings steps
    // aside meanwhile, and comes back when this goes false: done, failed or
    // cancelled)
    property bool signingIn: false
    Process {
        id: helper
        onRunningChanged: if (running) root.offline = false
        command: ["python3", "-I", decodeURIComponent(Qt.resolvedUrl("../assets/scripts/google-account.py").toString().replace("file://", "")), Config.dir + "/google"]
        stdinEnabled: true
        onStarted: {
            helper.write(JSON.stringify(root.pending) + "\n"); // (the Process's own write, not the queue's)
            stdinEnabled = false;
        }
        onExited: code => {
            stdinEnabled = true;
            root.signingIn = false;
            if (code !== 0 && !root.replied)
                root.lastError = "Google helper failed. Check that python3 and python-gobject are installed.";
            // The write is in Google's data now (or failed, and is shown as
            // Google has it): its override goes either way
            if (root.current?.key)
                root.setOverride(root.current.key, null);
            root.current = null;
            // A failed sync waits longer each time before the automatic ones try again
            if (root.operation === "sync" || root.operation === "status") {
                if (root.lastError && root.connected) {
                    root.failures++;
                    root.retryAt = Date.now() + Math.min(2 * 3600 * 1000, 60 * 1000 * Math.pow(4, root.failures - 1)); // 1, 4, 16, 64 min, then 2 h (no network yet at login: soon)
                } else if (!root.lastError) {
                    root.failures = 0;
                    root.retryAt = 0;
                }
            }
            // Changes still waiting (offline): tried again later
            if (root.outbox > 0 && root.queue.length === 0) {
                if (root.offline)
                    root.sendFailures++;
                resend.restart();
            } else if (root.outbox === 0) {
                root.sendFailures = 0;
                resend.stop();
            }
            if (root.operation === "status" && root.connected && root.queue.length === 0)
                Qt.callLater(() => root.freshen(2 * 60 * 1000));
            else
                Qt.callLater(root.pump);
        }
        stdout: SplitParser {
            onRead: line => {
                root.replied = true;
                try {
                    const message = JSON.parse(line);
                    if (message.signingIn)
                        root.signingIn = true;
                    if (message.authUrl && message.authUrl.startsWith("https://accounts.google.com/o/oauth2/v2/auth?")) {
                        root.signingIn = true;
                        Qt.openUrlExternally(message.authUrl);
                    }
                    if (message.data) {
                        root.account = message.data;
                        // A destination whose account went away: back to this computer
                        if (["disconnect", "forget"].includes(root.operation)) {
                            if (!root.find(root.taskLists, Config.options.google.taskListId))
                                Config.options.google.taskListId = "";
                            if (!root.find(root.calendars, Config.options.google.newEventCalendar))
                                Config.options.google.newEventCalendar = "";
                        }
                    }
                    if (message.mutationComplete)
                        root.mutationComplete();
                    if (message.conflict)
                        root.conflict(message.conflict);
                    if (message.error) {
                        root.lastError = message.error;
                        root.offline = !!message.offline;
                        // (offline: the change is kept and shown; no alarm for that)
                        if (!message.offline && !["sync", "status", "flush"].includes(root.operation))
                            Quickshell.execDetached(["notify-send", "-a", "iRice", "Google", message.error]);
                    }
                } catch (e) { root.lastError = "Could not read the Google helper response."; }
            }
        }
    }
}
