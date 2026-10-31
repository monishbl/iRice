pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.common

// iCloud (calendars, contacts) and Microsoft 365 (Outlook calendars,
// Microsoft To Do), through assets/scripts/cloud-accounts.py: one request per
// run, its state back. Their events join the calendar (Calendar.between),
// their to-dos join To-do (Todos.list), iCloud contacts show in Spotlight.
//
// Changes show at once and go to the helper in turn; deleting hides first
// and is sent when its Undo (8 s) has run out, as with Google. A change the
// server turns away (it changed on the phone meanwhile) leaves the server's
// version, and a notification says so.
// Syncs: at login, on unlock, when Notification Center opens after 2
// minutes, and every 15 minutes in use; never while locked or idle.
Singleton {
    id: root

    readonly property string script: Quickshell.shellDir + "/assets/scripts/cloud-accounts.py"
    property var state_: ({}) // the helper's last answer
    readonly property var accounts: state_.accounts ?? []
    readonly property bool hasICloud: accounts.some(a => a.provider === "icloud")
    readonly property bool hasMicrosoft: accounts.some(a => a.provider === "microsoft")
    readonly property var calendars: state_.calendars ?? []
    readonly property var taskLists: state_.taskLists ?? []
    readonly property var contacts: state_.contacts ?? []
    readonly property real syncedAt: state_.syncedAt ?? 0
    readonly property bool busy: helper.running
    property string lastError: ""
    property bool offline: false

    function account(id) {
        return accounts.find(a => a.id === id) ?? null;
    }
    function providerName(id) {
        return account(id)?.provider === "icloud" ? "iCloud" : "Microsoft 365";
    }
    function calendar(accountId, id) {
        return calendars.find(c => c.account === accountId && c.id === id) ?? null;
    }
    function shown(c) {
        return !(Config.options.cloud.hiddenCalendars ?? []).includes(c.account + "|" + c.id);
    }

    // ── Events: iCloud's as iCalendar text (read by Calendar), Microsoft's
    // already one per occurrence ─────────────────────────────────────────
    readonly property var parsed: (state_.events ?? []).map(e => {
        const cal = calendar(e.account, e.calendarId);
        if (!cal || !shown(cal))
            return null;
        if (e.ics !== undefined)
            return { raw: e, cal: cal, evs: Calendar.parseIcs(e.ics), notes: descriptionOf(e.ics) };
        return { raw: e, cal: cal, evs: null, notes: e.notes ?? "" };
    }).filter(x => x !== null)
    function descriptionOf(ics) {
        const m = ics.replace(/\r?\n[ \t]/g, "").match(/^DESCRIPTION[^:\n]*:(.*)$/m);
        return m ? m[1].replace(/\\n/gi, "\n").replace(/\\([,;\\])/g, "$1").trim() : "";
    }
    function keyOf(raw) {
        return "cloud:" + raw.account + ":" + (raw.href ?? raw.id);
    }
    // Shown before the server has them: key -> { hidden } | { patch }
    property var overrides: ({})

    // Occurrences overlapping [from, to), as Calendar.between() has them
    function between(from, to) {
        const out = [];
        for (const p of parsed) {
            const raw = p.raw;
            const key = keyOf(raw);
            const o = overrides[key];
            if (o?.hidden)
                continue;
            const base = { id: raw.href ?? raw.id, location: "", color: p.cal.color || "", calendar: p.cal.title, local: false, alert: -1,
                notes: p.notes, cloud: true, source: raw, writable: !!raw.writable, recurring: false, accountName: providerName(raw.account) };
            if (p.evs) {
                for (const ev of p.evs)
                    for (const inst of Calendar.expand(ev, from, to))
                        out.push(Object.assign({}, base, { key: key + "|" + inst.start.getTime(), title: ev.title, allDay: ev.allDay, location: ev.location,
                            alert: ev.alert ?? -1, recurring: !!ev.rrule }, inst, o?.patch ? shownPatch(o.patch) : {}));
            } else {
                const date = x => x.dateTime ? new Date(x.dateTime) : new Date(Number(x.date.slice(0, 4)), Number(x.date.slice(5, 7)) - 1, Number(x.date.slice(8, 10)));
                const start = date(raw.start), end = date(raw.end);
                if (start < to && end > from)
                    out.push(Object.assign({}, base, { key: key, title: raw.title, start: start, end: end, allDay: !!raw.start.date,
                        location: raw.location ?? "", recurring: !!raw.series }, o?.patch ? shownPatch(o.patch) : {}));
            }
        }
        return out;
    }
    function shownPatch(f) {
        const out = {};
        for (const k of ["title", "location", "notes", "allDay"])
            if (f[k] !== undefined)
                out[k] = f[k];
        if (f.start !== undefined) {
            out.start = new Date(f.start);
            out.end = new Date(f.end ?? f.start);
        }
        return out;
    }

    // ── Changing events ─────────────────────────────────────────────────
    function addEvent(accountId, calendarId, fields) {
        const cal = calendar(accountId, calendarId);
        if (!cal?.writable) {
            lastError = "That calendar can't be changed from here. Choose another one.";
            return false;
        }
        send({ cmd: "eventInsert", account: accountId, calendarId: calendarId, fields: fields });
        return true;
    }
    function updateEvent(e, fields) {
        const raw = e.source;
        if (!raw?.writable) {
            lastError = e.recurring ? "Change repeating events on your phone or on the web." : "This calendar can't be changed from here.";
            return false;
        }
        const key = keyOf(raw);
        setOverride(key, { patch: Object.assign({}, overrides[key]?.patch ?? {}, fields) });
        send({ cmd: "eventPatch", account: raw.account, calendarId: raw.calendarId, id: raw.href ?? raw.id, etag: raw.etag ?? "", fields: fields }, key);
        return true;
    }
    // Hidden now, sent when its Undo has run out
    property var deleting: [] // [{ key, request, at }]
    function removeEvent(e) {
        const raw = e.source;
        if (!raw?.writable) {
            lastError = e.recurring ? "Delete repeating events on your phone or on the web." : "This calendar can't be changed from here.";
            return false;
        }
        hide(keyOf(raw), { cmd: "eventDelete", account: raw.account, calendarId: raw.calendarId, id: raw.href ?? raw.id, etag: raw.etag ?? "" });
        return true;
    }
    function restoreEvent(e) {
        unhide(keyOf(e.source));
    }
    function hide(key, request) {
        setOverride(key, { hidden: true });
        deleting = deleting.filter(d => d.key !== key).concat([{ key: key, request: request, at: Date.now() }]);
        deleteTimer.start();
    }
    function unhide(key) {
        deleting = deleting.filter(d => d.key !== key);
        setOverride(key, null);
    }
    function setOverride(key, o) {
        const next = Object.assign({}, overrides);
        if (o)
            next[key] = o;
        else
            delete next[key];
        overrides = next;
    }
    Timer {
        id: deleteTimer
        interval: 1000
        repeat: true
        onTriggered: {
            const due = root.deleting.filter(d => Date.now() - d.at >= 8000);
            root.deleting = root.deleting.filter(d => Date.now() - d.at < 8000);
            for (const d of due)
                root.send(d.request, d.key);
            if (root.deleting.length === 0)
                stop();
        }
    }

    // ── Microsoft To Do ─────────────────────────────────────────────────
    readonly property var tasks: (state_.tasks ?? []).map(t => {
        const key = "cloud/" + t.account + "/" + t.listId + "/" + t.id;
        const o = overrides[key];
        if (o?.hidden)
            return null;
        const p = t.date ? t.date.split("-").map(Number) : [];
        return Object.assign({}, t, { key: key, cloud: true, allDay: true, due: p.length ? new Date(p[0], p[1] - 1, p[2], 23, 59).getTime() : 0 }, o?.patch ?? {});
    }).filter(t => t !== null)
    function taskKey(t) {
        return "cloud/" + t.account + "/" + t.listId + "/" + t.id;
    }
    function addTask(ref, item) {
        const [acc, list] = ref.split("|");
        send({ cmd: "taskInsert", account: acc, listId: list, fields: { text: item.text, done: !!item.done, date: item.due ? Qt.formatDate(new Date(item.due), "yyyy-MM-dd") : "" } });
        return true;
    }
    function updateTask(t, patch) {
        const f = {};
        if (patch.text !== undefined)
            f.text = patch.text;
        if (patch.done !== undefined)
            f.done = patch.done;
        if (patch.due !== undefined)
            f.date = patch.due ? Qt.formatDate(new Date(patch.due), "yyyy-MM-dd") : "";
        const key = taskKey(t);
        setOverride(key, { patch: Object.assign({}, overrides[key]?.patch ?? {}, patch) });
        send({ cmd: "taskPatch", account: t.account, listId: t.listId, id: t.id, etag: t.etag ?? "", fields: f }, key);
        return true;
    }
    function removeTask(t) {
        hide(taskKey(t), { cmd: "taskDelete", account: t.account, listId: t.listId, id: t.id });
    }
    function restoreTask(t) {
        unhide(taskKey(t));
    }

    // ── Contacts (Spotlight) ────────────────────────────────────────────
    function searchContacts(q, limit) {
        const ql = q.toLowerCase();
        return contacts.filter(c => c.name.toLowerCase().includes(ql) || c.emails.some(e => e.toLowerCase().includes(ql))).slice(0, limit);
    }

    // ── Accounts ────────────────────────────────────────────────────────
    function connectICloud(appleId, password) {
        signingIn = true;
        send({ cmd: "connectICloud", appleId: appleId, password: password });
    }
    // Microsoft: GNOME Online Accounts' Microsoft 365 sign-in (goa-signin
    // ms_graph), then the account it made
    function connectMicrosoft() {
        signingIn = true;
        lastError = "";
        msSignin.running = true;
    }
    Process {
        id: msSignin
        command: [Quickshell.shellDir + "/native/build/goa-signin", "ms_graph"]
        stdout: StdioCollector {
            onStreamFinished: {
                let r = {};
                try {
                    r = JSON.parse(text.trim().split("\n").pop());
                } catch (e) {}
                if (r.account) {
                    root.send({ cmd: "connectMicrosoft", goa: r.account, identity: r.identity ?? "" });
                } else {
                    root.signingIn = false;
                    if (!r.cancelled)
                        root.lastError = r.error || "Couldn't open the Microsoft sign-in. Update iRice, then try again.";
                }
            }
        }
    }
    property bool signingIn: false
    function disconnect(id) {
        send({ cmd: "disconnect", account: id });
    }
    function sync() {
        if (accounts.length > 0)
            send({ cmd: "sync" });
    }
    function freshen(age) {
        if (accounts.length === 0 || busy || queue.length > 0 || System.locked)
            return;
        if (Date.now() - syncedAt >= age)
            sync();
    }

    // ── The helper: one request at a time ───────────────────────────────
    property var queue: [] // [{ request, key }]
    property var current: null
    function send(request, key) {
        queue = queue.concat([{ request: request, key: key ?? "" }]);
        pump();
    }
    function pump() {
        if (helper.running || queue.length === 0)
            return;
        current = queue[0];
        queue = queue.slice(1);
        helper.running = true; // (the request goes once it has started)
    }
    Process {
        id: helper
        command: ["python3", "-I", root.script]
        stdinEnabled: true
        // (written once it's running: a write before that is lost)
        onStarted: write(JSON.stringify(root.current.request) + "\n")
        stdout: StdioCollector {
            onStreamFinished: {
                let s = null;
                try {
                    s = JSON.parse(text.trim().split("\n").pop());
                } catch (e) {}
                const done = root.current;
                if (s) {
                    root.state_ = s;
                    root.offline = !!s.offline;
                    root.lastError = s.error ?? "";
                    if (s.error && done && /^(event|task)/.test(done.request.cmd))
                        Quickshell.execDetached(["notify-send", "-a", "iRice", "Change not saved", s.error]);
                } else {
                    root.lastError = "The calendar helper didn't answer. Check that Python 3 is installed.";
                }
                // the server's version is shown now
                if (done?.key)
                    root.setOverride(done.key, null);
                if (["connectICloud", "connectMicrosoft"].includes(done?.request.cmd))
                    root.signingIn = false;
                root.current = null;
                Qt.callLater(root.pump);
            }
        }
    }
    Component.onCompleted: {
        send({ cmd: "status" });
        startSync.start();
    }
    Timer { id: startSync; interval: 20000; onTriggered: root.freshen(0) }

    IdleMonitor {
        id: idle
        timeout: 300
        respectInhibitors: true
    }
    Timer {
        interval: 60 * 1000
        repeat: true
        running: root.accounts.length > 0 && !System.locked && !idle.isIdle
        onTriggered: root.freshen(15 * 60 * 1000)
    }
    Connections {
        target: System
        function onLockedChanged() {
            if (!System.locked)
                root.freshen(2 * 60 * 1000);
        }
    }
    Connections {
        target: Panels
        function onOpenChanged() {
            if (Panels.open === "notificationCenter")
                root.freshen(2 * 60 * 1000);
        }
    }
}
