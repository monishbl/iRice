import QtQuick
import qs.common
import qs.services

// What the morning screen shows (TodayScreen): the greeting and date, today's
// events and to-dos on the left, the weather on the right, Pin for today and
// Close at the bottom. `rows` (0 → 1) brings the rows in one after another.
// Esc and Enter close; Tab moves between the buttons.
FocusScope {
    id: root

    property real rows: 1
    readonly property int pad: 32

    // Today's events (past ones dimmed) and open to-dos, soonest first
    readonly property date now: clock.now
    readonly property var events: Calendar.eventsOn(now)
    readonly property var todos: Todos.sorted((UiState.data.todos ?? []).map((t, i) => Object.assign({ i: i }, t)).filter(t => !t.done)).slice(0, 5)
    QtObject {
        id: clock
        property date now: new Date()
    }
    Timer {
        interval: 60 * 1000
        running: true
        repeat: true
        onTriggered: clock.now = new Date()
    }

    function toggle(i) {
        const l = (UiState.data.todos ?? []).slice();
        if (!l[i])
            return;
        l[i] = Object.assign({}, l[i], { done: !l[i].done });
        UiState.data.todos = l;
    }

    // Row n of the stagger: 40 ms apart, each rising 8 px as it fades in
    readonly property real span: 0.62 // seconds the stagger runs over
    function appear(n) {
        const t = rows * span * 1000 - n * 40;
        return Math.max(0, Math.min(1, t / 300));
    }
    component Rise: Item {
        property int order: 0
        readonly property real k: root.appear(order)
        opacity: k
        transform: Translate { y: (1 - (1 - Math.pow(1 - k, 3))) * 8 }
    }

    Keys.onEscapePressed: Today.close()
    Keys.onReturnPressed: Today.close()
    Keys.onEnterPressed: Today.close()

    // ── Greeting ────────────────────────────────────────────────────────
    Rise {
        id: head
        order: 0
        x: root.pad
        y: root.pad
        width: root.width - 2 * root.pad - weatherNow.width - 16
        height: hello.implicitHeight + 8 + date.implicitHeight
        Text {
            id: hello
            width: parent.width
            elide: Text.ElideRight
            text: Today.greeting(root.now)
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: 24; weight: Font.DemiBold; letterSpacing: -0.2 }
        }
        Text {
            id: date
            y: hello.implicitHeight + 8
            text: Qt.formatDate(root.now, "dddd, d MMMM")
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: 14 }
        }
    }
    // The weather now, top right
    Rise {
        id: weatherNow
        order: 0
        visible: Weather.ready
        anchors { right: parent.right; rightMargin: root.pad; top: parent.top; topMargin: root.pad }
        width: nowRow.implicitWidth
        height: nowRow.implicitHeight
        Row {
            id: nowRow
            spacing: 10
            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: Weather.icon
                fill: true
                size: 24
                color: Theme.color.label
            }
            Column {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2
                Text {
                    text: Weather.temp + Weather.unit
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: 24; weight: Font.DemiBold; features: { "tnum": 1 } }
                }
                Text {
                    text: Weather.condition
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
            }
        }
    }

    // ── Left: events and to-dos ─────────────────────────────────────────
    Column {
        id: leftCol
        x: root.pad
        y: head.y + head.height + 32
        width: root.width - 2 * root.pad - rightCol.width - 32
        spacing: 4

        Rise {
            order: 1
            width: parent.width
            height: 20
            Text {
                text: "Today"
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
            }
        }
        Rise {
            order: 2
            visible: root.events.length === 0
            width: parent.width
            height: 36
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Nothing on your calendar today"
                color: Theme.color.tertiary
                font { family: Theme.font.family; pixelSize: 14 }
            }
        }
        Repeater {
            model: root.events.slice(0, 5)
            Rise {
                id: ev
                required property var modelData
                required property int index
                order: 2 + index
                width: leftCol.width
                height: 36
                readonly property bool past: !modelData.allDay && modelData.end < root.now
                opacity: k * (past ? 0.5 : 1)
                Text {
                    id: evTime
                    anchors.verticalCenter: parent.verticalCenter
                    width: 132
                    text: Calendar.timeText(ev.modelData)
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.body; features: { "tnum": 1 } }
                }
                Rectangle {
                    id: evBar
                    anchors { left: evTime.right; verticalCenter: parent.verticalCenter }
                    width: 3
                    height: 18
                    radius: 1.5
                    color: ev.modelData.color || Theme.accent
                }
                Text {
                    anchors { left: evBar.right; leftMargin: 10; right: parent.right; verticalCenter: parent.verticalCenter }
                    elide: Text.ElideRight
                    text: ev.modelData.title
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: 14 }
                }
            }
        }

        Item { width: 1; height: 20 }
        Rise {
            order: 3 + Math.min(5, root.events.length)
            width: parent.width
            height: 20
            Text {
                text: "To-dos"
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
            }
        }
        Rise {
            order: 4 + Math.min(5, root.events.length)
            visible: root.todos.length === 0
            width: parent.width
            height: 36
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: (UiState.data.todos ?? []).length === 0 ? "No to-dos. Add them in Notification Center." : "All done"
                color: Theme.color.tertiary
                font { family: Theme.font.family; pixelSize: 14 }
            }
        }
        Repeater {
            model: root.todos
            Rise {
                id: todo
                required property var modelData
                required property int index
                order: 4 + Math.min(5, root.events.length) + index
                width: leftCol.width
                height: 36
                Rectangle {
                    id: tick
                    anchors.verticalCenter: parent.verticalCenter
                    width: 18
                    height: 18
                    radius: 9
                    color: "transparent"
                    border.width: 1.5
                    border.color: tickMouse.containsMouse ? Theme.accent : Theme.color.tertiary
                    MouseArea {
                        id: tickMouse
                        anchors { fill: parent; margins: -8 }
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.toggle(todo.modelData.i)
                    }
                }
                Text {
                    anchors { left: tick.right; leftMargin: 12; right: due.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                    elide: Text.ElideRight
                    text: todo.modelData.text
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: 14 }
                }
                Text {
                    id: due
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    visible: !!todo.modelData.due
                    readonly property date at: new Date(todo.modelData.due ?? 0)
                    text: todo.modelData.allDay ? (Today.dayKey(at) === Today.day ? "Today" : Qt.formatDate(at, "ddd d MMM")) : Qt.formatTime(at, "h:mm AP")
                    color: (todo.modelData.due ?? Infinity) < root.now.getTime() ? Theme.color.danger : Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.body; features: { "tnum": 1 } }
                }
            }
        }
    }

    // ── Right: the weather through the day ──────────────────────────────
    Column {
        id: rightCol
        anchors { right: parent.right; rightMargin: root.pad }
        y: leftCol.y
        width: 240
        spacing: 4

        Rise {
            order: 1
            width: parent.width
            height: 20
            Text {
                text: "Weather"
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
            }
        }
        Rise {
            order: 2
            visible: !Weather.ready
            width: parent.width
            height: 36
            Text {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width
                wrapMode: Text.Wrap
                text: Weather.error === "Offline" ? "Offline. The weather shows once you're connected." : "The weather isn't available right now."
                color: Theme.color.tertiary
                font { family: Theme.font.family; pixelSize: 14 }
            }
        }
        Rise {
            order: 2
            visible: Weather.ready
            width: parent.width
            height: 36
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "High " + Weather.high + Weather.unit + " · Low " + Weather.low + Weather.unit
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: 14; features: { "tnum": 1 } }
            }
            Text {
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                visible: Weather.rainChance > 0
                text: Weather.rainChance + "% rain"
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.body; features: { "tnum": 1 } }
            }
        }
        Repeater {
            model: Weather.ready ? Weather.hourly : []
            Rise {
                id: hour
                required property var modelData
                required property int index
                order: 3 + index
                width: rightCol.width
                height: 32
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    width: 64
                    text: hour.modelData.time
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.body; features: { "tnum": 1 } }
                }
                Icon {
                    x: 72
                    anchors.verticalCenter: parent.verticalCenter
                    name: hour.modelData.icon
                    fill: true
                    size: 16
                    color: Theme.color.label
                }
                Text {
                    x: 104
                    anchors.verticalCenter: parent.verticalCenter
                    visible: (hour.modelData.rain ?? 0) >= 20
                    text: hour.modelData.rain + "%"
                    color: Theme.accents.blue
                    font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                }
                Text {
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    text: hour.modelData.temp + Weather.unit
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: 14; features: { "tnum": 1 } }
                }
            }
        }
    }

    // ── Buttons ─────────────────────────────────────────────────────────
    Rise {
        order: 10
        anchors { right: parent.right; rightMargin: root.pad; bottom: parent.bottom; bottomMargin: root.pad }
        width: buttons.implicitWidth
        height: buttons.implicitHeight
        Row {
            id: buttons
            spacing: 12
            TodayButton {
                // the widget is on the desktop every day anyway
                visible: !Today.widgetAlways
                text: "Pin for today"
                icon: "push-pin"
                onActivated: Today.pin()
            }
            TodayButton {
                text: "Close"
                primary: true
                focus: true
                onActivated: Today.close()
            }
        }
    }
}
