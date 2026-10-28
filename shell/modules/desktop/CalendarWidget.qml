import QtQuick
import Quickshell
import qs.common
import qs.modules.notifications as Nc
import qs.services

// Calendar.
//   Small: the day and what's next; click for the calendar in Notification
//   Center.
//   Medium: the month (dots on busy days) beside the chosen day's events;
//   click a date to see its day.
//   Large: Notification Center's calendar itself: Month / Week / To-do,
//   events and to-dos to add, edit, move and delete, and the quick add.
Item {
    id: root

    property string size: "s"
    property bool preview: false // the gallery's picture of it
    SystemClock { id: clock; precision: SystemClock.Minutes }
    readonly property date today: clock.date
    property date selected: today
    readonly property bool onToday: sameDay(selected, today)
    readonly property var events: {
        Calendar.local;
        Calendar.remote;
        clock.date;
        return size === "s" ? Calendar.upcoming(1) : Calendar.eventsOn(selected).filter(e => onToday ? (e.allDay || e.end > today) : true).slice(0, 3);
    }

    function sameDay(a, b) {
        return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
    }
    function when(e) {
        const day = d => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
        const diff = Math.round((day(e.start) - day(today)) / 86400000);
        const prefix = diff === 0 || size !== "s" ? "" : diff === 1 ? "Tomorrow " : Qt.formatDate(e.start, "ddd") + " ";
        return e.allDay ? (prefix === "" ? "All day" : prefix + "all day") : prefix + Qt.formatTime(e.start, "h:mm AP");
    }

    // ── Large: the full calendar ────────────────────────────────────────
    Loader {
        anchors.fill: parent
        active: root.size === "l"
        sourceComponent: Nc.CalendarWidget {
            color: "transparent"
            padding: 0
            cellHeight: 22
            fitHeight: true
            todoTab: true
            enabled: !root.preview
        }
    }

    // ── Small and medium ────────────────────────────────────────────────
    Item {
        anchors.fill: parent
        visible: root.size !== "l"

        // Small: the whole widget opens Notification Center's calendar
        MouseArea {
            anchors.fill: parent
            enabled: root.size === "s" && !root.preview
            cursorShape: Qt.PointingHandCursor
            onClicked: {
                Panels.ncTab = 0;
                Panels.show("notificationCenter", Quickshell.screens[0]);
            }
        }

        // The day: today, or the date picked in the month
        Column {
            id: head
            width: root.size === "m" ? parent.width * 0.42 : parent.width
            spacing: 0
            Text {
                text: (root.onToday ? Qt.formatDate(root.selected, "dddd") : Qt.formatDate(root.selected, "ddd d MMM")).toUpperCase()
                color: Theme.accentInk
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Bold; letterSpacing: 0.5 }
            }
            Text {
                visible: root.size === "s" || root.onToday
                text: root.today.getDate()
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: 40; weight: Font.Normal; features: { "tnum": 1 } }
            }
        }

        // Its events (small: what's next)
        Column {
            id: eventList
            anchors { left: parent.left; bottom: parent.bottom }
            width: root.size === "m" ? parent.width * 0.42 : parent.width
            spacing: 6
            Text {
                visible: root.events.length === 0
                width: parent.width
                wrapMode: Text.Wrap
                text: root.size === "s" || root.onToday ? "No more events today" : "No events"
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.small }
            }
            Repeater {
                model: root.events.slice(0, root.size === "s" ? 1 : root.onToday ? 2 : 3)
                Row {
                    id: ev
                    required property var modelData
                    spacing: 6
                    width: eventList.width
                    Rectangle {
                        width: 3
                        height: 30
                        radius: 1.5
                        color: ev.modelData.color || Theme.accent
                    }
                    Column {
                        width: parent.width - 9
                        Text {
                            width: parent.width
                            text: ev.modelData.title
                            elide: Text.ElideRight
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                        }
                        Text {
                            width: parent.width
                            text: root.when(ev.modelData)
                            elide: Text.ElideRight
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.caption }
                        }
                    }
                }
            }
        }

        // Medium: the month, dots on days with events or to-dos; click a date
        Item {
            id: month
            visible: root.size === "m"
            anchors { right: parent.right; top: parent.top }
            width: parent.width * 0.54
            height: parent.height
            readonly property date first: new Date(root.today.getFullYear(), root.today.getMonth(), 1)
            readonly property int lead: (first.getDay() + 6) % 7 // weeks start on Monday
            readonly property int days: new Date(root.today.getFullYear(), root.today.getMonth() + 1, 0).getDate()
            readonly property real cw: width / 7
            readonly property real ch: height / (2 + Math.ceil((lead + days) / 7))
            // "y-m-d" of days with events, and with to-dos due
            readonly property var busy: {
                Calendar.local;
                Calendar.remote;
                const set = Calendar.busyDays(first, new Date(first.getFullYear(), first.getMonth(), days));
                for (const t of Todos.list)
                    if (t.due && !t.done) {
                        const d = new Date(t.due);
                        set[d.getFullYear() + "-" + d.getMonth() + "-" + d.getDate()] = true;
                    }
                return set;
            }
            Text {
                text: Qt.formatDate(root.today, "MMMM").toUpperCase()
                color: Theme.accentInk
                font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Bold }
            }
            Repeater {
                model: 7
                Text {
                    required property int index
                    x: index * month.cw
                    y: month.ch
                    width: month.cw
                    horizontalAlignment: Text.AlignHCenter
                    text: ["M", "T", "W", "T", "F", "S", "S"][index]
                    color: Theme.color.tertiary
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
                }
            }
            Repeater {
                model: month.days
                Item {
                    id: dayCell
                    required property int index
                    readonly property int cell: index + month.lead
                    readonly property date day: new Date(month.first.getFullYear(), month.first.getMonth(), index + 1)
                    readonly property bool isToday: index + 1 === root.today.getDate()
                    readonly property bool isSelected: root.sameDay(day, root.selected)
                    readonly property bool busy: !!month.busy[day.getFullYear() + "-" + day.getMonth() + "-" + day.getDate()]
                    x: (cell % 7) * month.cw
                    y: (Math.floor(cell / 7) + 2) * month.ch
                    width: month.cw
                    height: month.ch
                    Rectangle {
                        anchors.centerIn: parent
                        width: Math.min(parent.width, parent.height) + 2
                        height: width
                        radius: width / 2
                        visible: dayCell.isToday || dayCell.isSelected || dayMouse.containsMouse
                        color: dayCell.isToday ? Theme.accent : dayCell.isSelected ? Theme.color.fillPressed : Theme.color.fill
                    }
                    Text {
                        anchors.centerIn: parent
                        text: dayCell.index + 1
                        color: dayCell.isToday ? Theme.color.onAccent : Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: dayCell.isToday ? Font.Bold : Font.Medium; features: { "tnum": 1 } }
                    }
                    // Something on this day
                    Rectangle {
                        visible: dayCell.busy && !dayCell.isToday
                        anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: -1 }
                        width: 3
                        height: 3
                        radius: 1.5
                        color: Theme.accent
                    }
                    MouseArea {
                        id: dayMouse
                        anchors.fill: parent
                        enabled: !root.preview
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        // A second click on the chosen date goes back to today
                        onClicked: root.selected = dayCell.isSelected && !dayCell.isToday ? root.today : dayCell.day
                    }
                }
            }
        }
    }
}
