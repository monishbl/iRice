import QtQuick
import Quickshell
import qs.common
import qs.modules.controlcenter
import qs.services

// The calendar in Notification Center.
//   Month or Week (the switch at the top); scroll to move through them.
//   Dots under a date: events (in their calendar's colour) and to-dos (orange).
//   Click a date: its events and to-dos below. Double-click: add an event or
//   a to-do there. Click an item: edit it, or delete it (with Undo).
//   Drag an item onto another date to move it (your own events and to-dos;
//   subscribed calendars are read-only).
//   The field at the bottom adds in plain words: "Call mum at 5pm".
Platter {
    id: root

    property date shown: new Date()
    property date selected: new Date()
    property bool week: false
    readonly property date today: clock.date
    readonly property int firstWeekday: 1 // Monday
    readonly property var days: {
        if (week) {
            const s = new Date(selected.getFullYear(), selected.getMonth(), selected.getDate());
            s.setDate(s.getDate() - (s.getDay() - firstWeekday + 7) % 7);
            return [...Array(7).keys()].map(i => new Date(s.getFullYear(), s.getMonth(), s.getDate() + i));
        }
        const y = shown.getFullYear(), m = shown.getMonth();
        const first = new Date(y, m, 1);
        const lead = (first.getDay() - firstWeekday + 7) % 7;
        return [...Array(42).keys()].map(i => new Date(y, m, 1 - lead + i));
    }
    // What's on the shown days: "y-m-d" -> { colors: [event colours], todos: n }
    readonly property var marks: {
        Calendar.local;
        Calendar.remote;
        const out = {};
        const key = d => d.getFullYear() + "-" + d.getMonth() + "-" + d.getDate();
        const first = days[0], last = days[days.length - 1];
        for (const e of Calendar.between(first, new Date(last.getFullYear(), last.getMonth(), last.getDate() + 1))) {
            const d = new Date(e.start.getFullYear(), e.start.getMonth(), e.start.getDate());
            let end = e.allDay ? new Date(e.end.getTime() - 1) : e.end;
            end = new Date(end.getFullYear(), end.getMonth(), end.getDate());
            for (; d <= end; d.setDate(d.getDate() + 1)) {
                const k = key(d);
                out[k] = out[k] ?? { colors: [], todos: 0 };
                const c = String(e.color || Theme.accent);
                if (!out[k].colors.includes(c))
                    out[k].colors.push(c);
            }
        }
        for (const t of Todos.list) {
            if (!t.due || t.done)
                continue;
            const k = key(new Date(t.due));
            out[k] = out[k] ?? { colors: [], todos: 0 };
            out[k].todos++;
        }
        return out;
    }
    // The selected day: events, then to-dos due that day
    readonly property var items: {
        Calendar.local;
        Calendar.remote;
        const evs = Calendar.eventsOn(selected).map(e => ({ kind: "event", e: e }));
        const tds = Todos.on(selected).map(t => ({ kind: "todo", t: t }));
        return evs.concat(tds);
    }
    property var undo: null // { kind, item, index? }, offered back for a few seconds
    // While dragging an item: the date under the pointer
    property var dropDay: null
    function dayAt(scenePos) {
        const view = week ? weekList : grid;
        const p = view.mapFromItem(null, scenePos.x, scenePos.y);
        if (p.x < 0 || p.y < 0 || p.x >= view.width || p.y >= view.height)
            return null;
        if (week) {
            const c = weekList.childAt(p.x, p.y);
            return c?.modelData ?? null;
        }
        const header = grid.height - 6 * 32; // the M T W row
        const row = Math.floor((p.y - header) / 32), col = Math.floor(p.x / grid.cell);
        return p.y < header ? null : days[row * 7 + col] ?? null;
    }

    implicitHeight: column.implicitHeight + 24

    SystemClock { id: clock; precision: SystemClock.Hours }

    function sameDay(a, b) {
        return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
    }
    function dayKey(d) {
        return d.getFullYear() + "-" + d.getMonth() + "-" + d.getDate();
    }
    function step(n) {
        if (week) {
            selected = new Date(selected.getFullYear(), selected.getMonth(), selected.getDate() + 7 * n);
            shown = selected;
        } else {
            shown = new Date(shown.getFullYear(), shown.getMonth() + n, 1);
        }
    }
    function dayTitle(d) {
        return sameDay(d, today) ? "Today" : sameDay(d, new Date(today.getFullYear(), today.getMonth(), today.getDate() + 1)) ? "Tomorrow" : Qt.formatDate(d, "dddd d MMMM");
    }

    // Move an item to another day, keeping its time
    function moveTo(item, day) {
        if (item.kind === "event" && item.e.local) {
            const src = Calendar.local.find(x => x.id === item.e.id);
            if (!src)
                return;
            const s = new Date(src.start);
            const shift = new Date(day.getFullYear(), day.getMonth(), day.getDate(), s.getHours(), s.getMinutes()).getTime() - src.start;
            Calendar.update(src.id, { start: src.start + shift, end: (src.end || src.start) + shift });
        } else if (item.kind === "todo") {
            const d = new Date(item.t.due);
            Todos.update(item.t.i, { due: new Date(day.getFullYear(), day.getMonth(), day.getDate(), d.getHours(), d.getMinutes()).getTime() });
        }
        selected = day;
    }
    function remove(item) {
        if (item.kind === "event")
            undo = { kind: "event", item: Calendar.remove(item.e.id) };
        else
            undo = { kind: "todo", item: Todos.remove(item.t.i), index: item.t.i };
        undoTimer.restart();
        editor.close();
    }

    Column {
        id: column
        anchors { left: parent.left; right: parent.right; top: parent.top; margins: 12 }
        spacing: 8

        // Month and year, Month / Week, arrows
        Item {
            width: parent.width
            height: 26
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.week ? Qt.formatDate(root.days[0], "d MMM") + " – " + Qt.formatDate(root.days[6], "d MMM") : Qt.formatDate(root.shown, "MMMM yyyy")
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
            }
            Row {
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                spacing: 4
                Segmented {
                    width: 104
                    height: 24
                    options: ["Month", "Week"]
                    current: root.week ? 1 : 0
                    onSelected: i => {
                        root.week = i === 1;
                        if (!root.week)
                            root.shown = root.selected;
                    }
                }
                Repeater {
                    model: [{ icon: "caret-left", step: -1 }, { icon: "caret-right", step: 1 }]
                    IconButton {
                        required property var modelData
                        width: 24
                        height: 24
                        icon: modelData.icon
                        iconSize: 12
                        onClicked: root.step(modelData.step)
                    }
                }
            }
        }

        // The days: a month grid, or the week as rows with what's on
        Item {
            width: parent.width
            height: root.week ? weekList.height : grid.height
            WheelHandler {
                property real acc: 0
                onWheel: e => {
                    acc += e.angleDelta.y;
                    if (Math.abs(acc) >= 120) {
                        root.step(acc < 0 ? 1 : -1);
                        acc = 0;
                    }
                }
            }

            Grid {
                id: grid
                visible: !root.week
                columns: 7
                width: parent.width
                readonly property real cell: width / 7
                Repeater {
                    model: ["M", "T", "W", "T", "F", "S", "S"]
                    Text {
                        required property string modelData
                        width: grid.cell
                        horizontalAlignment: Text.AlignHCenter
                        text: modelData
                        color: Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
                    }
                }
                Repeater {
                    model: root.week ? [] : root.days
                    DayCell {
                        width: grid.cell
                        height: 32
                    }
                }
            }

            Column {
                id: weekList
                visible: root.week
                width: parent.width
                spacing: 2
                Repeater {
                    model: root.week ? root.days : []
                    Rectangle {
                        id: wday
                        required property var modelData
                        readonly property bool isSelected: root.sameDay(modelData, root.selected)
                        readonly property var evs: { Calendar.local; Calendar.remote; return Calendar.eventsOn(modelData); }
                        readonly property var tds: { Todos.list; return Todos.on(modelData).filter(t => !t.done); }
                        width: weekList.width
                        height: Math.max(34, weekCol.implicitHeight + 10)
                        radius: 8
                        color: root.dropDay && root.sameDay(root.dropDay, modelData) ? Theme.accentAlpha(0.25) : isSelected ? Theme.color.fill : wdayMouse.containsMouse ? Qt.alpha(Theme.color.fill, 0.5) : "transparent"
                        Column {
                            x: 8
                            width: 44
                            anchors.verticalCenter: parent.verticalCenter
                            Text {
                                text: Qt.formatDate(wday.modelData, "ddd")
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
                            }
                            // Today's date in the accent circle, like the month grid
                            Rectangle {
                                readonly property bool isToday: root.sameDay(wday.modelData, root.today)
                                width: 26
                                height: 26
                                radius: 13
                                x: -4
                                color: isToday ? Theme.accent : "transparent"
                                Text {
                                    anchors.centerIn: parent
                                    text: wday.modelData.getDate()
                                    color: parent.isToday ? Theme.color.onAccent : Theme.color.label
                                    font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold; features: { "tnum": 1 } }
                                }
                            }
                        }
                        Column {
                            id: weekCol
                            x: 56
                            width: parent.width - 64
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 2
                            Text {
                                visible: wday.evs.length + wday.tds.length === 0
                                text: "—"
                                color: Theme.color.tertiary
                                font { family: Theme.font.family; pixelSize: Theme.font.small }
                            }
                            Repeater {
                                model: wday.evs.slice(0, 4)
                                Row {
                                    required property var modelData
                                    spacing: 6
                                    Rectangle { anchors.verticalCenter: parent.verticalCenter; width: 6; height: 6; radius: 3; color: parent.modelData.color || Theme.accent }
                                    Text {
                                        width: weekCol.width - 12
                                        text: (parent.modelData.allDay ? "" : Qt.formatTime(parent.modelData.start, "h:mm AP") + "  ") + parent.modelData.title
                                        elide: Text.ElideRight
                                        color: Theme.color.label
                                        font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                                    }
                                }
                            }
                            Repeater {
                                model: wday.tds.slice(0, 3)
                                Row {
                                    required property var modelData
                                    spacing: 6
                                    Rectangle { anchors.verticalCenter: parent.verticalCenter; width: 6; height: 6; radius: 1.5; color: Theme.accents.orange }
                                    Text {
                                        width: weekCol.width - 12
                                        text: parent.modelData.text
                                        elide: Text.ElideRight
                                        color: Theme.color.label
                                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                                    }
                                }
                            }
                        }
                        MouseArea {
                            id: wdayMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: root.selected = wday.modelData
                            onDoubleClicked: editor.create(wday.modelData)
                        }
                    }
                }
            }
        }

        // The selected day
        Item {
            width: parent.width
            height: 22
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.dayTitle(root.selected)
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
            }
            IconButton {
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                width: 22
                height: 22
                icon: "plus"
                iconSize: 12
                tooltip: "Add to this day"
                onClicked: editor.create(root.selected)
            }
        }
        Text {
            visible: root.items.length === 0 && !root.undo
            text: "Nothing on this day. Double-click a date to add something."
            width: parent.width
            wrapMode: Text.Wrap
            color: Theme.color.tertiary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
        Repeater {
            model: root.items.slice(0, 8)
            Rectangle {
                id: row
                required property var modelData
                readonly property bool isEvent: modelData.kind === "event"
                readonly property bool movable: !isEvent || modelData.e.local
                width: column.width
                height: 36
                radius: 8
                color: rowMouse.containsMouse ? Theme.color.fill : "transparent"
                z: rowDrag.active ? 10 : 0
                opacity: rowDrag.active ? 0.5 : 1

                // Event: its calendar's colour bar; to-do: a tick box
                Rectangle {
                    id: lead
                    visible: row.isEvent
                    x: 6
                    y: 5
                    width: 3
                    height: parent.height - 10
                    radius: 2
                    color: row.isEvent ? (row.modelData.e.color || Theme.accent) : "transparent"
                }
                Rectangle {
                    visible: !row.isEvent
                    x: 4
                    anchors.verticalCenter: parent.verticalCenter
                    width: 16
                    height: 16
                    radius: 8
                    color: !row.isEvent && row.modelData.t.done ? Theme.accents.orange : "transparent"
                    border.width: !row.isEvent && row.modelData.t.done ? 0 : 1.5
                    border.color: Theme.accents.orange
                    Icon { anchors.centerIn: parent; visible: !row.isEvent && row.modelData.t.done; name: "check"; bold: true; size: 9; color: "white" }
                    MouseArea {
                        anchors { fill: parent; margins: -6 }
                        onClicked: Todos.update(row.modelData.t.i, { done: !row.modelData.t.done })
                    }
                }
                Column {
                    anchors { left: parent.left; leftMargin: row.isEvent ? 16 : 28; right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                    Text {
                        width: parent.width
                        text: row.isEvent ? row.modelData.e.title : row.modelData.t.text
                        elide: Text.ElideRight
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium; strikeout: !row.isEvent && !!row.modelData.t.done }
                    }
                    Text {
                        width: parent.width
                        text: row.isEvent ? Calendar.timeText(row.modelData.e) + (row.modelData.e.location ? " · " + row.modelData.e.location : "") + " · " + row.modelData.e.calendar : "To-do" + (row.modelData.t.allDay ? "" : " · " + Qt.formatTime(new Date(row.modelData.t.due), "h:mm AP"))
                        elide: Text.ElideRight
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
                    }
                }
                MouseArea {
                    id: rowMouse
                    anchors { fill: parent; leftMargin: row.isEvent ? 0 : 24 }
                    hoverEnabled: true
                    cursorShape: row.movable ? Qt.OpenHandCursor : Qt.PointingHandCursor
                    onClicked: editor.edit(row.modelData)
                }
                Component.onDestruction: if (rowDrag.active) Panels.holdPull = Math.max(0, Panels.holdPull - 1)
                DragHandler {
                    id: rowDrag
                    enabled: row.movable
                    target: null // the row stays; a ghost follows the pointer
                    onCentroidChanged: if (active) root.dropDay = root.dayAt(centroid.scenePosition)
                    onActiveChanged: {
                        // The panel holds still while something is dragged in it
                        Panels.holdPull += active ? 1 : -1;
                        if (!active) {
                            const d = root.dropDay;
                            root.dropDay = null;
                            if (d)
                                root.moveTo(row.modelData, d);
                        }
                    }
                }
                // While dragging: the item follows the pointer, and is what drops
                Rectangle {
                    id: ghost
                    visible: rowDrag.active
                    parent: root
                    readonly property point at: root.mapFromItem(null, rowDrag.centroid.scenePosition.x, rowDrag.centroid.scenePosition.y)
                    x: at.x - 20
                    y: at.y - 18
                    width: 180
                    height: 30
                    radius: 8
                    z: 50
                    color: Theme.dark ? "#2C2C31" : "#FFFFFF"
                    border.width: 1
                    border.color: Theme.accent
                    Text {
                        anchors { fill: parent; leftMargin: 10; rightMargin: 10 }
                        verticalAlignment: Text.AlignVCenter
                        text: row.isEvent ? row.modelData.e.title : row.modelData.t.text
                        elide: Text.ElideRight
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                    }
                }
            }
        }

        // Deleted: offer it back
        Row {
            visible: root.undo !== null
            spacing: 8
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Deleted “" + (root.undo?.item?.title ?? root.undo?.item?.text ?? "") + "”"
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.small }
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Undo"
                color: Theme.accent
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        if (root.undo.kind === "event")
                            Calendar.restoreEvent(root.undo.item);
                        else
                            Todos.restore(root.undo.item, root.undo.index);
                        root.undo = null;
                    }
                }
            }
        }
        Timer {
            id: undoTimer
            interval: 8000
            onTriggered: root.undo = null
        }

        // Quick add: plain words, on the selected day
        TextField {
            id: quickAdd
            width: parent.width
            height: 32
            placeholder: "Add: “Call mum at 5pm”"
            commitOnBlur: false
            onCommitted: t => {
                if (t.trim() === "")
                    return;
                const q = Calendar.parseQuick(t);
                const sel = root.selected;
                if (q) {
                    // A time without a day word goes on the selected day
                    if (!/\b(today|tonight|tomorrow|mon|tue|wed|thu|fri|sat|sun)\w*\b|\bin\s+\d/i.test(t) && !root.sameDay(sel, root.today))
                        q.start = new Date(sel.getFullYear(), sel.getMonth(), sel.getDate(), q.start.getHours(), q.start.getMinutes());
                    Calendar.add(q.title, q.start, q.start, false, 0, "");
                    root.selected = q.start;
                } else {
                    // No time: an all-day event on the selected day
                    const d = new Date(sel.getFullYear(), sel.getMonth(), sel.getDate());
                    Calendar.add(t.trim().charAt(0).toUpperCase() + t.trim().slice(1), d, new Date(d.getTime() + 86400000), true, -1, "");
                }
                text = "";
            }
        }
    }

    // One date in the month grid
    component DayCell: Item {
        id: cell
        required property var modelData
        readonly property bool isToday: root.sameDay(modelData, root.today)
        readonly property bool inMonth: modelData.getMonth() === root.shown.getMonth()
        readonly property bool isSelected: root.sameDay(modelData, root.selected)
        readonly property var mark: root.marks[root.dayKey(modelData)] ?? null
        Rectangle {
            anchors.centerIn: parent
            anchors.verticalCenterOffset: -3
            width: 26
            height: 26
            radius: 13
            readonly property bool target: !!root.dropDay && root.sameDay(root.dropDay, cell.modelData)
            visible: cell.isToday || cell.isSelected || dayMouse.containsMouse || target
            color: cell.isToday ? Theme.accent : target ? Theme.accentAlpha(0.35) : cell.isSelected ? Theme.color.fillPressed : Theme.color.fill
        }
        Text {
            anchors.centerIn: parent
            anchors.verticalCenterOffset: -3
            text: cell.modelData.getDate()
            color: cell.isToday ? Theme.color.onAccent : cell.inMonth ? Theme.color.label : Theme.color.tertiary
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: cell.isToday ? Font.Bold : Font.Normal; features: { "tnum": 1 } }
        }
        // Up to three event colours, then an orange square for to-dos
        Row {
            anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 1 }
            spacing: 2
            opacity: cell.inMonth ? 1 : 0.5
            Repeater {
                model: (cell.mark?.colors ?? []).slice(0, 3)
                Rectangle { required property string modelData; width: 4; height: 4; radius: 2; color: modelData }
            }
            Rectangle { visible: (cell.mark?.todos ?? 0) > 0; width: 4; height: 4; radius: 1; color: Theme.accents.orange }
        }
        MouseArea {
            id: dayMouse
            anchors.fill: parent
            hoverEnabled: true
            onClicked: root.selected = cell.modelData
            onDoubleClicked: editor.create(cell.modelData)
        }
    }

    // ── New / edit: an event or a to-do ─────────────────────────────────
    Rectangle {
        id: editor
        anchors.fill: parent
        radius: root.shapeRadius
        color: Qt.rgba(0, 0, 0, 0.35)
        visible: opacity > 0.01
        opacity: open ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 150 } }
        z: 100

        property bool open: false
        property var editing: null // the item being edited, or null for new
        property date day: new Date()
        property bool isTodo: false
        property bool readOnly: false
        property string error: ""

        function create(d) {
            root.selected = d;
            editing = null;
            readOnly = false;
            day = d;
            isTodo = false;
            title.text = "";
            allDay.checked = false;
            startField.text = "9:00 AM";
            endField.text = "10:00 AM";
            alertDrop.value = Calendar.defaultAlert;
            error = "";
            open = true;
            title.focusInput();
        }
        function edit(item) {
            editing = item;
            error = "";
            if (item.kind === "event") {
                const e = item.e;
                isTodo = false;
                readOnly = !e.local;
                day = e.start;
                title.text = e.title;
                allDay.checked = e.allDay;
                startField.text = Qt.formatTime(e.start, "h:mm AP");
                endField.text = Qt.formatTime(e.end, "h:mm AP");
                alertDrop.value = e.alert ?? Calendar.defaultAlert;
            } else {
                const t = item.t;
                isTodo = true;
                readOnly = false;
                day = new Date(t.due);
                title.text = t.text;
                allDay.checked = !!t.allDay;
                startField.text = t.allDay ? "" : Qt.formatTime(new Date(t.due), "h:mm AP");
            }
            open = true;
        }
        function close() {
            open = false;
            editing = null;
        }
        // "9", "9:30", "5pm", "17:00", "5.30 pm" -> minutes since midnight, or -1
        function minutes(s) {
            const m = String(s).trim().toLowerCase().match(/^(\d{1,2})(?:[:.](\d{2}))?\s*(am|pm|a|p)?$/);
            if (!m)
                return -1;
            let h = Number(m[1]);
            const min = Number(m[2] ?? 0);
            if (m[3])
                h = (h % 12) + (m[3].startsWith("p") ? 12 : 0);
            return h > 23 || min > 59 ? -1 : h * 60 + min;
        }
        function save() {
            const name = title.text.trim();
            if (!name) {
                error = "Give it a title.";
                return;
            }
            const base = new Date(day.getFullYear(), day.getMonth(), day.getDate());
            if (isTodo) {
                const t = startField.text.trim() === "" ? -1 : minutes(startField.text);
                if (startField.text.trim() !== "" && t < 0) {
                    error = "Use a time like 9:30 or 5pm, or leave it empty.";
                    return;
                }
                const due = t < 0 ? new Date(base.getTime() + (23 * 60 + 59) * 60000) : new Date(base.getTime() + t * 60000);
                if (editing)
                    Todos.update(editing.t.i, { text: name, due: due.getTime(), allDay: t < 0 });
                else
                    UiState.data.todos = Todos.list.concat([{ text: name, done: false, due: due.getTime(), allDay: t < 0 }]);
            } else {
                let start, end;
                if (allDay.checked) {
                    start = base;
                    end = new Date(base.getTime() + 86400000);
                } else {
                    const s = minutes(startField.text), e = minutes(endField.text);
                    if (s < 0 || e < 0) {
                        error = "Use times like 9:30 or 5pm.";
                        return;
                    }
                    start = new Date(base.getTime() + s * 60000);
                    end = new Date(base.getTime() + Math.max(e, s) * 60000);
                }
                const alert = allDay.checked ? -1 : alertDrop.value;
                if (editing)
                    Calendar.update(editing.e.id, { title: name, start: start.getTime(), end: end.getTime(), allDay: allDay.checked, alert: alert });
                else
                    Calendar.add(name, start, end, allDay.checked, alert, "");
            }
            root.selected = base;
            close();
        }

        MouseArea { anchors.fill: parent; onClicked: editor.close() } // outside the card closes it
        Rectangle {
            id: card
            anchors.centerIn: parent
            width: parent.width - 24
            height: form.implicitHeight + 28
            radius: 16
            // (frosted glass: Notification Center's layer is blurred)
            color: Theme.glass.tier === "solid" ? Theme.glass.solid : Theme.dark ? Qt.rgba(0.11, 0.11, 0.13, 0.78) : Qt.rgba(0.97, 0.97, 0.99, 0.82)
            border.width: 1
            border.color: Theme.dark ? Qt.rgba(1, 1, 1, 0.14) : Qt.rgba(0, 0, 0, 0.1)
            MouseArea { anchors.fill: parent } // clicks inside stay inside
            Keys.onEscapePressed: editor.close()

            Column {
                id: form
                x: 14
                y: 14
                width: parent.width - 28
                spacing: 10

                Text {
                    text: editor.readOnly ? "Event" : editor.editing ? (editor.isTodo ? "Edit To-do" : "Edit Event") : "New on " + Qt.formatDate(editor.day, "ddd d MMM")
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                }
                Segmented {
                    visible: !editor.editing
                    width: parent.width
                    options: ["Event", "To-do"]
                    current: editor.isTodo ? 1 : 0
                    onSelected: i => {
                        editor.isTodo = i === 1;
                        startField.text = editor.isTodo ? "" : "9:00 AM";
                    }
                }
                TextField {
                    id: title
                    width: parent.width
                    height: 32
                    enabled: !editor.readOnly
                    placeholder: editor.isTodo ? "What to do" : "Title"
                    commitOnBlur: false
                    onCommitted: editor.save()
                }
                // Event: all day, or from–to; to-do: an optional time
                Row {
                    visible: !editor.isTodo
                    spacing: 8
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: "All day"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                    Switch {
                        id: allDay
                        enabled: !editor.readOnly
                        onToggled: checked = !checked
                    }
                }
                Row {
                    visible: editor.isTodo || !allDay.checked
                    width: parent.width
                    spacing: 8
                    TextField {
                        id: startField
                        width: editor.isTodo ? parent.width : (parent.width - 24) / 2
                        height: 32
                        enabled: !editor.readOnly
                        placeholder: editor.isTodo ? "Time (optional), e.g. 5pm" : "Starts"
                        commitOnBlur: false
                        onCommitted: editor.save()
                    }
                    Text {
                        visible: !editor.isTodo
                        anchors.verticalCenter: parent.verticalCenter
                        text: "–"
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.body }
                    }
                    TextField {
                        id: endField
                        visible: !editor.isTodo
                        width: (parent.width - 24) / 2
                        height: 32
                        enabled: !editor.readOnly
                        placeholder: "Ends"
                        commitOnBlur: false
                        onCommitted: editor.save()
                    }
                }
                Row {
                    visible: !editor.isTodo && !allDay.checked && !editor.readOnly
                    spacing: 8
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Alert"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                    Dropdown {
                        id: alertDrop
                        width: 150
                        options: [{ value: -1, label: "None" }, { value: 0, label: "At the time" }, { value: 5, label: "5 min before" }, { value: 10, label: "10 min before" }, { value: 30, label: "30 min before" }, { value: 60, label: "1 hour before" }, { value: 1440, label: "1 day before" }]
                        onChosen: v => value = v
                    }
                }
                Text {
                    visible: editor.readOnly
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: editor.editing?.e ? Calendar.timeText(editor.editing.e) + (editor.editing.e.location ? " · " + editor.editing.e.location : "") + "\nFrom " + editor.editing.e.calendar + ", a subscribed calendar (read-only)." : ""
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
                Text {
                    visible: editor.error !== ""
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: editor.error
                    color: Theme.color.danger
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
                // Delete (when editing) on the left; Cancel and Save on the right
                Item {
                    width: parent.width
                    height: 32
                    Rectangle {
                        visible: !!editor.editing && !editor.readOnly
                        width: delText.implicitWidth + 24
                        height: 32
                        radius: Theme.radius.control
                        color: delMouse.containsMouse ? Qt.alpha(Theme.color.danger, 0.15) : "transparent"
                        Text { id: delText; anchors.centerIn: parent; text: editor.isTodo ? "Delete To-do" : "Delete Event"; color: Theme.color.danger; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                        MouseArea { id: delMouse; anchors.fill: parent; hoverEnabled: true; onClicked: root.remove(editor.editing) }
                    }
                    Row {
                        anchors.right: parent.right
                        spacing: 8
                        Rectangle {
                            width: cancelText.implicitWidth + 24
                            height: 32
                            radius: Theme.radius.control
                            color: cancelMouse.pressed ? Theme.color.fillPressed : cancelMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                            Text { id: cancelText; anchors.centerIn: parent; text: editor.readOnly ? "Close" : "Cancel"; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                            MouseArea { id: cancelMouse; anchors.fill: parent; hoverEnabled: true; onClicked: editor.close() }
                        }
                        Rectangle {
                            visible: !editor.readOnly
                            width: saveText.implicitWidth + 24
                            height: 32
                            radius: Theme.radius.control
                            color: saveMouse.pressed ? Qt.darker(Theme.accent, 1.2) : saveMouse.containsMouse ? Qt.darker(Theme.accent, 1.1) : Theme.accent
                            Text { id: saveText; anchors.centerIn: parent; text: editor.editing ? "Save Changes" : editor.isTodo ? "Add To-do" : "Add Event"; color: Theme.color.onAccent; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold } }
                            MouseArea { id: saveMouse; anchors.fill: parent; hoverEnabled: true; onClicked: editor.save() }
                        }
                    }
                }
            }
        }
    }
}
