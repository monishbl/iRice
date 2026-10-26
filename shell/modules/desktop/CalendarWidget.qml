import QtQuick
import Quickshell
import qs.common
import qs.services

// Calendar. Small: the day and what's next. Medium: the day and the next
// events beside the month. Large: the month above the next events.
Item {
    id: root

    property string size: "s"
    SystemClock { id: clock; precision: SystemClock.Minutes }
    readonly property date today: clock.date
    readonly property var events: { Calendar.local; Calendar.remote; clock.date; return Calendar.upcoming(size === "s" ? 1 : 2); }

    function when(e) {
        const day = d => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
        const diff = Math.round((day(e.start) - day(today)) / 86400000);
        const prefix = diff === 0 ? "" : diff === 1 ? "Tomorrow " : Qt.formatDate(e.start, "ddd") + " ";
        return e.allDay ? (diff === 0 ? "All day" : prefix + "all day") : prefix + Qt.formatTime(e.start, "h:mm AP");
    }

    // Day header
    Column {
        id: head
        width: root.size === "m" ? parent.width * 0.42 : parent.width
        spacing: 0
        Text {
            text: Qt.formatDate(root.today, "dddd").toUpperCase()
            color: Theme.accents.red
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Bold; letterSpacing: 0.5 }
        }
        Text {
            text: root.today.getDate()
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: root.size === "l" ? 30 : 40; weight: Font.Normal; features: { "tnum": 1 } }
        }
    }

    // Upcoming events (small and medium: under the day; large: at the bottom)
    Column {
        id: list
        anchors { left: parent.left; right: root.size === "m" ? undefined : parent.right; bottom: parent.bottom }
        width: root.size === "m" ? parent.width * 0.42 : parent.width
        spacing: 6
        Text {
            visible: root.events.length === 0
            text: "No events"
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
        Repeater {
            model: root.events
            Row {
                id: ev
                required property var modelData
                spacing: 6
                width: list.width
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

    // Month grid (medium: right side; large: top)
    Item {
        visible: root.size !== "s"
        anchors { right: parent.right; top: root.size === "l" ? head.bottom : parent.top; topMargin: root.size === "l" ? 4 : 0 }
        width: root.size === "m" ? parent.width * 0.54 : parent.width
        height: root.size === "m" ? parent.height : parent.height - head.height - list.height - 16
        readonly property date first: new Date(root.today.getFullYear(), root.today.getMonth(), 1)
        readonly property int lead: (first.getDay() + 6) % 7 // weeks start on Monday
        readonly property int days: new Date(root.today.getFullYear(), root.today.getMonth() + 1, 0).getDate()
        readonly property int headRows: root.size === "m" ? 2 : 1 // month name (medium) and weekday rows
        readonly property real cw: width / 7
        readonly property real ch: height / (headRows + Math.ceil((lead + days) / 7))
        Text {
            text: Qt.formatDate(root.today, "MMMM").toUpperCase()
            color: Theme.accents.red
            font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Bold }
            visible: root.size === "m"
        }
        Repeater {
            model: 7
            Text {
                required property int index
                x: index * parent.cw
                y: (parent.headRows - 1) * parent.ch
                width: parent.cw
                horizontalAlignment: Text.AlignHCenter
                text: ["M", "T", "W", "T", "F", "S", "S"][index]
                color: Theme.color.tertiary
                font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold }
            }
        }
        Repeater {
            model: parent.days
            Item {
                required property int index
                readonly property int cell: index + parent.lead
                readonly property bool isToday: index + 1 === root.today.getDate()
                x: (cell % 7) * parent.cw
                y: (Math.floor(cell / 7) + parent.headRows) * parent.ch
                width: parent.cw
                height: parent.ch
                Rectangle {
                    anchors.centerIn: parent
                    width: Math.min(parent.width, parent.height) + 2
                    height: width
                    radius: width / 2
                    color: Theme.accents.red
                    visible: parent.isToday
                }
                Text {
                    anchors.centerIn: parent
                    text: parent.index + 1
                    color: parent.isToday ? "white" : Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: parent.isToday ? Font.Bold : Font.Medium; features: { "tnum": 1 } }
                }
            }
        }
    }
}
