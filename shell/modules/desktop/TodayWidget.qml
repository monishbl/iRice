import QtQuick
import qs.common
import qs.services

// Today on the desktop (pinned from the morning screen, or there every day:
// Settings → Desktop & Widgets → Today). Medium: the greeting, the weather
// and the next events. Large adds to-dos and the day's high, low and rain.
// Click the greeting to open the morning screen again.
Item {
    id: root

    property string size: "l"
    property bool preview: false
    readonly property bool large: size === "l"

    QtObject {
        id: clock
        property date now: new Date()
    }
    Timer {
        interval: 60 * 1000
        running: !root.preview
        repeat: true
        onTriggered: clock.now = new Date()
    }
    // Events still to come today (all-day ones first)
    readonly property var events: Calendar.eventsOn(clock.now).filter(e => e.allDay || e.end > clock.now)
    readonly property var todos: Todos.sorted(Todos.list.filter(t => !t.done))

    function toggle(t) {
        if (preview)
            return;
        Todos.update(t, { done: !t.done });
    }

    // ── Greeting and weather ────────────────────────────────────────────
    Column {
        id: head
        width: parent.width - weather.width - 8
        spacing: 2
        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.preview ? "Good morning" : Today.greeting(clock.now)
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: root.large ? 18 : 16; weight: Font.DemiBold }
            MouseArea {
                anchors.fill: parent
                enabled: !root.preview
                cursorShape: Qt.PointingHandCursor
                onClicked: Today.show("")
            }
        }
        Text {
            text: Qt.formatDate(clock.now, root.large ? "dddd, d MMMM" : "ddd, d MMM")
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
    }
    Row {
        id: weather
        anchors.right: parent.right
        visible: Weather.ready
        spacing: 6
        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: Weather.icon
            fill: true
            size: 18
            color: Theme.color.label
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: Weather.temp + Weather.unit
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: 18; weight: Font.DemiBold; features: { "tnum": 1 } }
        }
    }

    Column {
        anchors { left: parent.left; right: parent.right; top: head.bottom; topMargin: 12 }
        spacing: 6

        Text {
            visible: root.large && Weather.ready
            text: "High " + Weather.high + Weather.unit + " · Low " + Weather.low + Weather.unit + (Weather.rainChance > 0 ? " · " + Weather.rainChance + "% rain" : "")
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
        }

        // ── Events ──
        Text {
            visible: root.events.length === 0
            text: "Nothing else today"
            color: Theme.color.tertiary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
        Repeater {
            model: root.events.slice(0, root.large ? 3 : 2)
            Item {
                id: ev
                required property var modelData
                width: parent.width
                height: 18
                Rectangle {
                    id: bar
                    anchors.verticalCenter: parent.verticalCenter
                    width: 3
                    height: 14
                    radius: 1.5
                    color: ev.modelData.color || Theme.accent
                }
                Text {
                    anchors { left: bar.right; leftMargin: 8; right: time.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
                    elide: Text.ElideRight
                    text: ev.modelData.title
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
                Text {
                    id: time
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    text: ev.modelData.allDay ? "All day" : Qt.formatTime(ev.modelData.start, "h:mm AP")
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                }
            }
        }

        // ── To-dos (large) ──
        Item { visible: root.large; width: 1; height: 4 }
        Text {
            visible: root.large && root.todos.length === 0
            text: "No to-dos"
            color: Theme.color.tertiary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
        Repeater {
            model: root.large ? root.todos.slice(0, 3) : []
            Item {
                id: todo
                required property var modelData
                width: parent.width
                height: 18
                Rectangle {
                    id: tick
                    anchors.verticalCenter: parent.verticalCenter
                    width: 14
                    height: 14
                    radius: 7
                    color: "transparent"
                    border.width: 1.5
                    border.color: Theme.color.tertiary
                    MouseArea {
                        anchors { fill: parent; margins: -6 }
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.toggle(todo.modelData)
                    }
                }
                Text {
                    anchors { left: tick.right; leftMargin: 8; right: parent.right; verticalCenter: parent.verticalCenter }
                    elide: Text.ElideRight
                    text: todo.modelData.text
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
            }
        }
    }
}
