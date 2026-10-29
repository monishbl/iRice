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
    readonly property var list: localList.concat(Google.tasks)

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
        const item = { text: p.text, done: false };
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
            return Google.run({ cmd: "taskAdd", listId: destination, patch: {
                text: item.text, done: !!item.done, date: item.due ? Qt.formatDate(new Date(item.due), "yyyy-MM-dd") : "" } });
        }
        UiState.data.todos = localList.concat([item]);
        return true;
    }

    // Change or remove the to-do at index (remove returns it, for Undo)
    function update(i, patch) {
        if (list[i]?.google)
            return Google.updateTask(list[i], patch);
        const l = localList.slice();
        if (!l[i])
            return;
        l[i] = Object.assign({}, l[i], patch);
        if (patch.due !== undefined)
            delete l[i].notified; // a new time notifies again
        UiState.data.todos = l;
        return true;
    }
    function remove(i) {
        const t = list[i];
        if (t?.google) {
            Google.run({ cmd: "taskDelete", listId: t.listId, id: t.id });
            return null; // Google deletions cannot restore the same task ID
        }
        UiState.data.todos = localList.filter((_, j) => j !== i);
        return t;
    }
    function restore(t, i) {
        if (!t)
            return;
        const l = localList.slice();
        l.splice(Math.min(i ?? l.length, l.length), 0, t);
        UiState.data.todos = l;
    }
    // On a given day: [{ i, ...todo }]
    function on(day) {
        const a = new Date(day.getFullYear(), day.getMonth(), day.getDate()).getTime(), b = a + 86400000;
        return list.map((t, i) => Object.assign({ i: i }, t)).filter(t => t.due >= a && t.due < b);
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
