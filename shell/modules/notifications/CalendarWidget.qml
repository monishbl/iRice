import QtQuick
import QtQuick.Effects
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
//   The + button opens the event/task editor for the selected day.
Platter {
    id: root

    property date shown: new Date()
    // Reused by the desktop Calendar widget: no padding of its own, shorter
    // date cells, and a fixed height (the calendar and agenda share one scroll surface)
    property int padding: 12
    property int cellHeight: 32
    property bool fitHeight: false
    // A third tab beside Month and Week: your to-do list (the desktop widget;
    // Notification Center has its own To-do tab)
    property bool todoTab: false
    property bool todoMode: false
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
        if (daysView.overlap >= 0.1) return null;
        const view = week ? weekList : grid;
        const p = view.mapFromItem(null, scenePos.x, scenePos.y);
        if (p.x < 0 || p.y < 0 || p.x >= view.width || p.y >= view.height)
            return null;
        if (week) {
            const c = weekList.childAt(p.x, p.y);
            return c?.modelData ?? null;
        }
        const header = grid.height - 6 * cellHeight; // the M T W row
        const row = Math.floor((p.y - header) / cellHeight), col = Math.floor(p.x / grid.cell);
        return p.y < header ? null : days[row * 7 + col] ?? null;
    }

    implicitHeight: headRow.height + 8 + column.implicitHeight + 2 * padding
    readonly property real scrollOffset: viewport.contentY
    function scrollToCalendar() { returnAnimation.restart(); }
    onWeekChanged: viewport.contentY = 0
    onTodoModeChanged: viewport.contentY = 0
    onSelectedChanged: if (fitHeight) viewport.contentY = 0
    NumberAnimation { id: returnAnimation; target: viewport; property: "contentY"; to: 0; duration: 220; easing.type: Easing.OutCubic }

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
        if (item.kind === "event" && (item.e.google || item.e.cloud)) {
            const e = item.e;
            const s = e.start;
            const start = e.allDay ? new Date(day.getFullYear(), day.getMonth(), day.getDate()).getTime()
                : new Date(day.getFullYear(), day.getMonth(), day.getDate(), s.getHours(), s.getMinutes()).getTime();
            const fields = { start: start, end: start + (e.end - e.start), allDay: e.allDay };
            if (e.recurring)
                editor.ask(item, every => Calendar.change(e, fields, every)); // this one, or every one?
            else
                Calendar.change(e, fields, false);
        } else if (item.kind === "event" && item.e.local) {
            const src = Calendar.local.find(x => x.id === item.e.id);
            if (!src)
                return;
            const s = new Date(src.start);
            const shift = new Date(day.getFullYear(), day.getMonth(), day.getDate(), s.getHours(), s.getMinutes()).getTime() - src.start;
            Calendar.update(src.id, { start: src.start + shift, end: (src.end || src.start) + shift });
        } else if (item.kind === "todo") {
            const d = new Date(item.t.due);
            Todos.update(item.t, { due: new Date(day.getFullYear(), day.getMonth(), day.getDate(), d.getHours(), d.getMinutes()).getTime() });
        }
        selected = day;
    }
    function remove(item, every) {
        if (item.kind === "event" && item.e.recurring && every === undefined) {
            editor.ask(item, all => root.remove(item, all));
            return;
        }
        if (item.kind === "event")
            undo = { kind: "event", item: Calendar.discard(item.e, !!every) };
        else
            undo = { kind: "todo", item: Todos.remove(item.t) };
        if (!undo?.item)
            undo = null;
        undoTimer.restart();
        editor.close();
    }

    // Month and year, Month / Week, arrows
    Item {
        id: headRow
        anchors { left: parent.left; right: parent.right; top: parent.top; margins: root.padding }
        height: 26
        z: 5
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.todoMode ? "To-do" : root.week ? Qt.formatDate(root.days[0], "d MMM") + " – " + Qt.formatDate(root.days[6], "d MMM") : Qt.formatDate(root.shown, root.todoTab ? "MMM yyyy" : "MMMM yyyy") // (shorter beside three tabs)
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
        Row {
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            spacing: 4
            // Scrolled down to the agenda: back up to the dates (here in the
            // header, so it never covers the agenda's own buttons)
            IconButton {
                objectName: "calendarScrollUp"
                visible: root.fitHeight && viewport.contentY > 24
                width: 24
                height: 24
                icon: "arrow-up"
                iconSize: 12
                tooltip: "Back to calendar"
                onClicked: root.scrollToCalendar()
            }
            Segmented {
                width: root.todoTab ? 150 : 104
                height: 24
                options: root.todoTab ? ["Month", "Week", "To-do"] : ["Month", "Week"]
                current: root.todoMode ? 2 : root.week ? 1 : 0
                onSelected: i => {
                    root.todoMode = i === 2;
                    if (i === 2)
                        return;
                    root.week = i === 1;
                    if (!root.week)
                        root.shown = root.selected;
                }
            }
            Repeater {
                model: root.todoMode ? [] : [{ icon: "caret-left", step: -1 }, { icon: "caret-right", step: 1 }]
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

    // One bounded scroll surface owns the fixed-height widget. A minimum
    // agenda height must never push the footer outside the glass frame.
    Flickable {
        id: viewport
        objectName: "calendarViewport"
        anchors { left: parent.left; right: parent.right; top: parent.top; margins: root.padding; topMargin: root.padding + headRow.height + 8 }
        height: root.fitHeight ? Math.max(0, root.height - 2 * root.padding - headRow.height - 8) : column.implicitHeight
        contentWidth: width
        contentHeight: column.implicitHeight
        interactive: root.fitHeight && contentHeight > height
        clip: root.fitHeight
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.VerticalFlick
        onMovementStarted: returnAnimation.stop()

        Column {
            id: column
            width: viewport.width
            spacing: 8

            // The days: a month grid, or the week as rows with what's on
            Item {
                id: daysView
                visible: !root.todoMode
                width: parent.width
                height: root.week ? weekList.height : grid.height
                objectName: "calendarDates"
                // Let long weeks scroll fully before leaving a faint, blurred
                // calendar behind the agenda. The widget's existing glass is
                // the backdrop; no opaque panel is painted over it.
                readonly property real pinStart: Math.max(0, height - viewport.height)
                readonly property real overlap: root.fitHeight ? Math.min(1, Math.max(0, viewport.contentY - pinStart) / Math.max(1, viewport.height * 0.75)) : 0
                opacity: 1 - 0.94 * overlap
                transform: Translate { y: root.fitHeight ? Math.max(0, viewport.contentY - daysView.pinStart) : 0 }
                layer.enabled: overlap > 0
                layer.effect: MultiEffect { blurEnabled: true; blurMax: 24; blur: daysView.overlap * 0.7 }
                enabled: overlap < 0.1
                WheelHandler {
                    enabled: !root.fitHeight
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
                            height: root.cellHeight
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
                        Squircle {
                            id: wday
                            required property var modelData
                            readonly property bool isSelected: root.sameDay(modelData, root.selected)
                            readonly property var allEvs: { Calendar.local; Calendar.remote; return Calendar.eventsOn(modelData); }
                            readonly property var allTds: { Todos.list; return Todos.on(modelData).filter(t => !t.done); }
                            // The desktop widget scrolls, so it lists everything; in
                            // Notification Center a busy week mustn't stretch it: a few
                            // per day, then "+N more"
                            readonly property var evs: root.fitHeight ? allEvs : allEvs.slice(0, 4)
                            readonly property var tds: root.fitHeight ? allTds : allTds.slice(0, 3)
                            readonly property int more: allEvs.length + allTds.length - evs.length - tds.length
                            width: weekList.width
                            height: Math.max(40, weekCol.implicitHeight + 10)
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
                                    height: root.fitHeight ? 22 : 26
                                    radius: 13
                                    x: -4
                                    color: isToday ? Theme.accent : "transparent"
                                    Text {
                                        anchors.centerIn: parent
                                        text: wday.modelData.getDate()
                                        color: parent.isToday ? Theme.color.onAccent : Theme.color.label
                                        font { family: Theme.font.family; pixelSize: root.fitHeight ? Theme.font.body : Theme.font.title; weight: Font.DemiBold; features: { "tnum": 1 } }
                                    }
                                }
                            }
                            Column {
                                id: weekCol
                                objectName: "calendarWeekItems"
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
                                    model: wday.evs
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
                                    model: wday.tds
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
                                Text {
                                    visible: wday.more > 0
                                    leftPadding: 12
                                    text: "+" + wday.more + " more"
                                    color: Theme.color.secondary
                                    font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
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

            Item {
                id: agenda
                objectName: "calendarAgenda"
                z: 1
                width: parent.width
                height: agendaContent.implicitHeight
                Column {
                    id: agendaContent
                    width: parent.width
                    spacing: 8
                    // The selected day
                    Item {
                        id: dayTitle
                        objectName: "calendarDayTitle"
                        visible: !root.todoMode
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
                        id: emptyText
                        visible: !root.todoMode && root.items.length === 0 && !root.undo
                        text: "Nothing on this day. Double-click a date to add something."
                        width: parent.width
                        wrapMode: Text.Wrap
                        color: Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                    // (The outer viewport does the scrolling, footer included)
                    Column {
                        id: itemsView
                        visible: !root.todoMode && root.items.length > 0
                        width: parent.width
                        Column {
                            id: itemsCol
                            objectName: "calendarItems"
                            width: parent.width
                            spacing: 8
                            Repeater {
                                model: root.fitHeight ? root.items : root.items.slice(0, 8)
                                Squircle {
                                    id: row
                                    required property var modelData
                                    readonly property bool isEvent: modelData.kind === "event"
                                    readonly property bool movable: !isEvent || Calendar.editable(modelData.e)
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
                                            onClicked: Todos.update(row.modelData.t, { done: !row.modelData.t.done })
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
                                    Squircle {
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
                        }
                    }

                    // To-do tab: the same list as Notification Center's
                    Loader {
                        active: root.todoTab
                        visible: root.todoMode
                        width: parent.width
                        height: item?.implicitHeight ?? 0
                        sourceComponent: TodoWidget {
                            color: "transparent"
                            padding: 0
                            fitHeight: false
                        }
                    }

                    // Deleted: offer it back
                    Row {
                        id: undoRow
                        visible: !root.todoMode && root.undo !== null
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
                            color: Theme.accentInk
                            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    if (root.undo.kind === "event")
                                        Calendar.restoreEvent(root.undo.item);
                                    else
                                        Todos.restore(root.undo.item);
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

                }

            } // agenda
        } // column
    } // viewport

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
            anchors.verticalCenterOffset: -Math.min(3, (root.cellHeight - 20) / 2)
            width: Math.min(26, root.cellHeight)
            height: width
            radius: width / 2
            readonly property bool target: !!root.dropDay && root.sameDay(root.dropDay, cell.modelData)
            visible: cell.isToday || cell.isSelected || dayMouse.containsMouse || target
            color: cell.isToday ? Theme.accent : target ? Theme.accentAlpha(0.35) : cell.isSelected ? Theme.color.fillPressed : Theme.color.fill
        }
        Text {
            anchors.centerIn: parent
            anchors.verticalCenterOffset: -Math.min(3, (root.cellHeight - 20) / 2)
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
    Squircle {
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
        // Where a new event goes: "" this computer, or "<account>|<calendar>"
        property string target: ""
        // (an account's calendar: Google, iCloud, Microsoft 365; they keep a location)
        readonly property bool google: editing ? !!(editing.e?.google || editing.e?.cloud) : !isTodo && target !== ""
        readonly property var targets: [{ value: "", label: "This computer" }].concat(Google.calendars
            .filter(c => c.writable && Google.shown(c))
            .map(c => ({ value: c.account + "|" + c.id, label: c.title + (Google.accounts.length > 1 ? " · " + Google.identity(c.account) : "") })))
            .concat(CloudAccounts.calendars.filter(c => c.writable && CloudAccounts.shown(c))
                .map(c => ({ value: "cloud|" + c.account + "|" + c.id, label: c.title + " · " + CloudAccounts.providerName(c.account) })))
        // A repeating event: this occurrence or all of them? (then `asking(every)`)
        property var asking: null
        function ask(item, then) {
            if (!open)
                edit(item);
            asking = then;
        }

        function create(d) {
            root.selected = d;
            editing = null;
            readOnly = false;
            asking = null;
            day = d;
            isTodo = false;
            target = targets.some(t => t.value === Config.options.google.newEventCalendar) ? Config.options.google.newEventCalendar : "";
            locationField.text = "";
            notesField.text = "";
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
            asking = null;
            if (item.kind === "event") {
                const e = item.e;
                isTodo = false;
                readOnly = !Calendar.editable(e);
                locationField.text = e.location ?? "";
                notesField.text = e.notes ?? "";
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
            asking = null;
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
        function save(every) {
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
                const saved = editing
                    ? Todos.update(editing.t, { text: name, due: due.getTime(), allDay: t < 0 })
                    : Todos.addItem({ text: name, done: false, due: due.getTime(), allDay: t < 0 });
                if (!saved) {
                    error = Google.lastError || "Could not save the task.";
                    return;
                }
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
                const fields = { title: name, start: start.getTime(), end: end.getTime(), allDay: allDay.checked, alert: alert, notes: notesField.text };
                if (google)
                    fields.location = locationField.text;
                if (editing?.e.recurring && every === undefined) {
                    asking = all => editor.save(all);
                    return;
                }
                let saved = true;
                if (editing)
                    saved = Calendar.change(editing.e, fields, !!every);
                else if (google && target.startsWith("cloud|")) {
                    delete fields.alert;
                    const bar = target.indexOf("|", 6);
                    saved = CloudAccounts.addEvent(target.slice(6, bar), target.slice(bar + 1), fields);
                    if (saved)
                        Config.options.google.newEventCalendar = target;
                } else if (google) {
                    delete fields.alert;
                    saved = Google.addEvent(target, fields);
                    if (saved)
                        Config.options.google.newEventCalendar = target;
                } else {
                    Calendar.add(name, start, end, allDay.checked, alert, notesField.text);
                    Config.options.google.newEventCalendar = "";
                }
                if (!saved) {
                    // (an edit is the event's own service's; a new one the chosen calendar's)
                    error = ((editing ? !!editing.e?.cloud : target.startsWith("cloud|")) ? CloudAccounts.lastError : Google.lastError) || "Could not save the event.";
                    asking = null;
                    return;
                }
            }
            root.selected = base;
            close();
        }

        MouseArea { anchors.fill: parent; onClicked: editor.close() } // outside the card closes it
        Squircle {
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
                Dropdown {
                    visible: !editor.editing && !editor.isTodo && editor.targets.length > 1
                    width: parent.width
                    options: editor.targets
                    value: editor.target
                    onChosen: v => editor.target = v
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
                TextField {
                    id: locationField
                    visible: editor.google && !editor.readOnly
                    width: parent.width
                    height: 32
                    placeholder: "Location"
                    commitOnBlur: false
                    onCommitted: editor.save()
                }
                TextField {
                    id: notesField
                    visible: !editor.isTodo && !editor.readOnly
                    width: parent.width
                    height: 32
                    placeholder: "Notes"
                    commitOnBlur: false
                    onCommitted: editor.save()
                }
                Row {
                    visible: !editor.isTodo && !allDay.checked && !editor.readOnly && !editor.google
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
                    text: editor.editing?.e ? Calendar.timeText(editor.editing.e) + (editor.editing.e.location ? " · " + editor.editing.e.location : "")
                        + (editor.editing.e.cloud && editor.editing.e.recurring ? "\nFrom " + editor.editing.e.calendar + " (" + editor.editing.e.accountName + "). Change repeating events on your phone or on the web."
                            : editor.editing.e.google || editor.editing.e.cloud ? "\nFrom " + editor.editing.e.calendar + " (" + editor.editing.e.accountName + "). You can view this calendar but not change it."
                            : "\nFrom " + editor.editing.e.calendar + ", a subscribed calendar (read-only).")
                        + (editor.editing.e.repeatUnsupported ? "\nThis event repeats in a way iRice can't show yet, so only its first date is here. Your calendar app has the rest." : "") : ""
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
                Text {
                    visible: !editor.readOnly && !!(editor.editing?.e?.google || editor.editing?.e?.cloud)
                    width: parent.width
                    elide: Text.ElideRight
                    text: editor.editing?.e?.google || editor.editing?.e?.cloud ? editor.editing.e.calendar + " · " + editor.editing.e.accountName : ""
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
                Text {
                    visible: editor.asking !== null
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: "This is a repeating event. Change only this event, or all events in the series?"
                    color: Theme.color.label
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
                Row {
                    visible: editor.asking !== null
                    anchors.right: parent.right
                    spacing: 8
                    Repeater {
                        model: [{ label: "Cancel", every: null }, { label: "This event", every: false }, { label: "All events", every: true }]
                        Squircle {
                            id: choice
                            required property var modelData
                            width: choiceText.implicitWidth + 24
                            height: 32
                            radius: Theme.radius.control
                            color: choiceMouse.pressed ? Theme.color.fillPressed : choiceMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                            Text { id: choiceText; anchors.centerIn: parent; text: choice.modelData.label; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                            MouseArea {
                                id: choiceMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                onClicked: {
                                    const then = editor.asking;
                                    editor.asking = null;
                                    if (choice.modelData.every === null)
                                        return;
                                    then(choice.modelData.every);
                                    if (editor.open && editor.error === "")
                                        editor.close();
                                }
                            }
                        }
                    }
                }
                Item {
                    visible: editor.asking === null
                    width: parent.width
                    height: 32
                    Squircle {
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
                        Squircle {
                            width: cancelText.implicitWidth + 24
                            height: 32
                            radius: Theme.radius.control
                            color: cancelMouse.pressed ? Theme.color.fillPressed : cancelMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                            Text { id: cancelText; anchors.centerIn: parent; text: editor.readOnly ? "Close" : "Cancel"; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                            MouseArea { id: cancelMouse; anchors.fill: parent; hoverEnabled: true; onClicked: editor.close() }
                        }
                        Squircle {
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
