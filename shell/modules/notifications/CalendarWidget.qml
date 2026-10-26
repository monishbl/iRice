import QtQuick
import Quickshell
import qs.common
import qs.modules.controlcenter
import qs.services

// Month calendar with today highlighted and dots on days with events; arrows
// page through months. Click a day to list its events. The field below adds a
// reminder in plain words: "Call mum at 5pm", "Stretch in 20 minutes".
Platter {
    id: root

    property date shown: new Date()
    property date selected: new Date()
    readonly property date today: clock.date
    readonly property var busy: Calendar.busyDays(days[0], days[41])
    readonly property var dayEvents: {
        Calendar.local;
        Calendar.remote;
        return Calendar.eventsOn(selected);
    }
    property var undoEvent: null // just deleted, offered back for a few seconds
    readonly property int firstWeekday: 1 // Monday
    readonly property var days: {
        const y = shown.getFullYear(), m = shown.getMonth();
        const first = new Date(y, m, 1);
        const lead = (first.getDay() - firstWeekday + 7) % 7;
        const out = [];
        for (let i = 0; i < 42; i++)
            out.push(new Date(y, m, 1 - lead + i));
        return out;
    }

    implicitHeight: column.implicitHeight + 24

    SystemClock { id: clock; precision: SystemClock.Hours }

    function sameDay(a, b) {
        return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
    }

    Column {
        id: column
        anchors { left: parent.left; right: parent.right; top: parent.top; margins: 12 }
        spacing: 8

        Item {
            width: parent.width
            height: 24
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: Qt.formatDate(root.shown, "MMMM yyyy")
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
            }
            Row {
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                Repeater {
                    model: [{ icon: "caret-left", step: -1 }, { icon: "caret-right", step: 1 }]
                    Rectangle {
                        required property var modelData
                        width: 24
                        height: 24
                        radius: 12
                        color: navMouse.containsMouse ? Theme.color.fillHover : "transparent"
                        Icon { anchors.centerIn: parent; name: parent.modelData.icon; bold: true; size: 12; color: Theme.color.secondary }
                        MouseArea {
                            id: navMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: root.shown = new Date(root.shown.getFullYear(), root.shown.getMonth() + parent.modelData.step, 1)
                        }
                    }
                }
            }
        }

        Grid {
            columns: 7
            width: parent.width
            readonly property real cell: width / 7

            Repeater {
                model: ["M", "T", "W", "T", "F", "S", "S"]
                Text {
                    required property string modelData
                    width: parent.cell
                    horizontalAlignment: Text.AlignHCenter
                    text: modelData
                    color: Theme.color.tertiary
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
                }
            }
            Repeater {
                model: root.days
                Item {
                    required property var modelData
                    readonly property bool isToday: root.sameDay(modelData, root.today)
                    readonly property bool inMonth: modelData.getMonth() === root.shown.getMonth()
                    readonly property bool isSelected: root.sameDay(modelData, root.selected)
                    readonly property bool hasEvents: !!root.busy[modelData.getFullYear() + "-" + modelData.getMonth() + "-" + modelData.getDate()]
                    width: parent.cell
                    height: 30
                    Rectangle {
                        anchors.centerIn: parent
                        anchors.verticalCenterOffset: -2
                        width: 24
                        height: 24
                        radius: 12
                        visible: parent.isToday || parent.isSelected || dayMouse.containsMouse
                        color: parent.isToday ? Theme.accent : parent.isSelected ? Theme.color.fillPressed : Theme.color.fill
                    }
                    Text {
                        anchors.centerIn: parent
                        anchors.verticalCenterOffset: -2
                        text: parent.modelData.getDate()
                        color: parent.isToday ? Theme.color.onAccent : parent.inMonth ? Theme.color.label : Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: parent.isToday ? Font.Bold : Font.Normal; features: { "tnum": 1 } }
                    }
                    Rectangle {
                        visible: parent.hasEvents
                        anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom }
                        width: 4
                        height: 4
                        radius: 2
                        color: parent.inMonth ? Theme.color.secondary : Theme.color.tertiary
                    }
                    MouseArea {
                        id: dayMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: root.selected = parent.modelData
                    }
                                }
            }
        }

        // Events on the selected day
        Text {
            topPadding: 6
            text: root.sameDay(root.selected, root.today) ? "Today" : root.sameDay(root.selected, new Date(root.today.getFullYear(), root.today.getMonth(), root.today.getDate() + 1)) ? "Tomorrow" : Qt.formatDate(root.selected, "dddd d MMMM")
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
        }

        Text {
            visible: root.dayEvents.length === 0 && !root.undoEvent
            text: "No events"
            color: Theme.color.tertiary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }

        Repeater {
            model: root.dayEvents.slice(0, 6)
            Item {
                id: ev
                required property var modelData
                width: parent.width
                height: 34
                Rectangle {
                    id: bar
                    y: 3
                    width: 3
                    height: parent.height - 6
                    radius: 2
                    color: ev.modelData.color || Theme.accent
                }
                Column {
                    anchors { left: bar.right; leftMargin: 8; right: del.left; rightMargin: 4; verticalCenter: parent.verticalCenter }
                    Text {
                        width: parent.width
                        text: ev.modelData.title
                        elide: Text.ElideRight
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                    }
                    Text {
                        width: parent.width
                        text: Calendar.timeText(ev.modelData) + (ev.modelData.location ? " · " + ev.modelData.location : "") + " · " + ev.modelData.calendar
                        elide: Text.ElideRight
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
                    }
                }
                IconButton {
                    id: del
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    visible: ev.modelData.local && evHover.hovered
                    icon: "trash"
                    iconSize: 14
                    onClicked: {
                        root.undoEvent = Calendar.remove(ev.modelData.id);
                        undoTimer.restart();
                    }
                }
                HoverHandler { id: evHover }
            }
        }

        // Deleted: offer it back
        Row {
            visible: root.undoEvent !== null
            spacing: 8
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Deleted “" + (root.undoEvent?.title ?? "") + "”"
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
                        Calendar.restoreEvent(root.undoEvent);
                        root.undoEvent = null;
                    }
                }
            }
        }
        Timer {
            id: undoTimer
            interval: 8000
            onTriggered: root.undoEvent = null
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
}
