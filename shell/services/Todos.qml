pragma Singleton
import QtQuick
import Quickshell
import qs.common

// To-dos with due dates (Notification Center's To-do and the Reminders
// widget). Type the when after the what: "pay rent tomorrow 5pm", "call mum
// at 6", "stretch in 20 min", "submit report friday". Timed ones notify when
// they come due.
Singleton {
    id: root

    readonly property var localList: UiState.data.todos ?? []
    // Each to-do carries a key ("local/<id>", "google/<list>/<id>"), and
    // changes find their to-do by it when they run. A position in the list
    // isn't enough: a Google sync can add or reorder tasks while a row or
    // the editor is open, and the change would land on another task.
    readonly property var list: localList.map(t => Object.assign({ key: "local/" + t.id }, t)).concat(Google.tasks)
    function find(ref) {
        const key = typeof ref === "string" ? ref : ref?.key;
        return key ? list.find(t => t.key === key) ?? null : null;
    }
    // To-dos saved before keys existed get theirs once
    function newId() {
        return Date.now().toString(36) + Math.random().toString(36).slice(2, 8);
    }
    function giveIds() {
        if (localList.some(t => !t.id))
            UiState.data.todos = localList.map(t => t.id ? t : Object.assign({ id: newId() }, t));
    }
    Connections {
        target: UiState
        function onLoadedChanged() {
            if (UiState.loaded)
                root.giveIds();
        }
    }
    Component.onCompleted: if (UiState.loaded) giveIds()

    // "text [when]" -> { text, due (ms) | 0, allDay }
    function parse(text) {
        let t = text.trim();
        // "tomorrow 5pm" -> "tomorrow at 5pm" (the calendar's parser wants "at")
        if (!/\bat\s+\d{1,2}(?:[:.]\d{2})?\s*(?:am|pm)?$/i.test(t))
            t = t.replace(/\s+(\d{1,2}(?:[:.]\d{2})?\s*(?:am|pm))$/i, " at $1");
        const q = Calendar.parseQuick(t);
        if (q)
            return { text: q.title, due: q.start.getTime(), allDay: false };
        // A day alone: due by the end of it
        const m = t.match(/\s+(?:by\s+|on\s+)?(today|tonight|tomorrow|mon|tue|wed|thu|fri|sat|sun)[a-z]*$/i);
        if (m) {
            const d = new Date();
            d.setHours(23, 59, 0, 0);
            const w = m[1].toLowerCase();
            if (w === "tomorrow")
                d.setDate(d.getDate() + 1);
            else if (!["today", "tonight"].includes(w)) {
                const want = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"].indexOf(w);
                d.setDate(d.getDate() + ((want - d.getDay() + 7) % 7 || 7));
            }
            const rest = t.slice(0, m.index).trim();
            if (rest)
                return { text: rest.charAt(0).toUpperCase() + rest.slice(1), due: d.getTime(), allDay: true };
        }
        return { text: text.trim(), due: 0, allDay: false };
    }
    function add(text) {
        const p = parse(text);
        if (!p.text)
            return;
        const item = { id: newId(), text: p.text, done: false };
        if (p.due) {
            item.due = p.due;
            item.allDay = p.allDay;
        }
        return addItem(item);
    }

    function addItem(item) {
        const destination = Config.options.google.taskListId;
        if (destination) {
            if (!Google.connected || !Google.taskLists.some(l => l.id === destination)) {
                Google.lastError = "Connect Google and select an available task list, or switch new tasks to Local in Settings → Google.";
                Quickshell.execDetached(["notify-send", "-a", "Reminders", "Task not added", Google.lastError]);
                return false;
            }
            if (item.due && !item.allDay) {
                Google.lastError = "Google Tasks cannot store reminder times. Choose a date-only task or Local in Settings → Google.";
                Quickshell.execDetached(["notify-send", "-a", "Reminders", "Task not added", Google.lastError]);
                return false;
            }
            return Google.addTask(destination, {
                text: item.text, done: !!item.done, date: item.due ? Qt.formatDate(new Date(item.due), "yyyy-MM-dd") : "" });
        }
        UiState.data.todos = localList.concat([item.id ? item : Object.assign({ id: newId() }, item)]);
        return true;
    }

    // Change or remove a to-do (one from `list`, or its key). remove()
    // returns it for Undo; a Google task is only deleted once Undo runs out.
    function update(ref, patch) {
        const t = find(ref);
        if (!t)
            return false;
        if (t.google)
            return Google.updateTask(t, patch);
        const l = localList.slice();
        const i = l.findIndex(x => x.id === t.id);
        l[i] = Object.assign({}, l[i], patch);
        if (patch.due !== undefined)
            delete l[i].notified; // a new time notifies again
        UiState.data.todos = l;
        return true;
    }
    function remove(ref) {
        const t = find(ref);
        if (!t)
            return null;
        if (t.google) {
            Google.removeTask(t);
            return t;
        }
        const at = localList.findIndex(x => x.id === t.id);
        UiState.data.todos = localList.filter(x => x.id !== t.id);
        return Object.assign({ at: at }, t);
    }
    function restore(t) {
        if (!t)
            return;
        if (t.google) {
            Google.restoreTask(t);
            return;
        }
        const item = Object.assign({}, t);
        for (const k of ["key", "i", "at"])
            delete item[k];
        const l = localList.slice();
        l.splice(Math.min(t.at ?? l.length, l.length), 0, item);
        UiState.data.todos = l;
    }
    // On a given day
    function on(day) {
        const a = new Date(day.getFullYear(), day.getMonth(), day.getDate()).getTime(), b = a + 86400000;
        return list.filter(t => t.due >= a && t.due < b);
    }

    // "Today 5:00 PM", "Tomorrow", "Fri", "12 Nov"; "" when undated
    function dueLabel(t) {
        if (!t?.due)
            return "";
        const d = new Date(t.due), now = new Date();
        const day = x => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
        const diff = Math.round((day(d) - day(now)) / 86400000);
        const date = diff === 0 ? "Today" : diff === 1 ? "Tomorrow" : diff === -1 ? "Yesterday" : diff > 1 && diff < 7 ? Qt.formatDate(d, "ddd") : Qt.formatDate(d, "d MMM");
        return t.allDay ? date : date + " " + Qt.formatTime(d, "h:mm AP");
    }
    function overdue(t) {
        return !!t?.due && !t.done && t.due < Date.now();
    }
    // Open to-dos, soonest due first, then the undated in the order added
    function sorted(items) {
        return items.slice().sort((a, b) => (a.due || Infinity) - (b.due || Infinity));
    }

    // Timed to-dos notify once when they come due
    Timer {
        interval: 30000
        repeat: true
        running: root.list.some(t => t.due && !t.done && !t.allDay && !t.notified)
        triggeredOnStart: true
        onTriggered: {
            const now = Date.now();
            let changed = false;
            const l = root.localList.map(t => {
                if (t.due && !t.done && !t.allDay && !t.notified && t.due <= now) {
                    changed = true;
                    if (now - t.due < 3600000) // not for ones long past
                        Quickshell.execDetached(["notify-send", "-a", "Reminders", "-i", "checkbox", t.text, "Due now"]);
                    return Object.assign({}, t, { notified: true });
                }
                return t;
            });
            if (changed)
                UiState.data.todos = l;
        }
    }
}
